use std::fmt::{Debug, Display};
use std::net::SocketAddr;
use std::sync::Arc;

use log::{error, info};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use thiserror::Error;
use tokio::net::TcpListener;
use tokio::select;
use tokio::sync::broadcast::{channel, Sender};

use ex15_shared::message::Message;
use ex15_shared::message_tcp_stream::{MessageTcpStream, MessageTcpStreamError};

use crate::server::ServerError::AddressInUseError;
use crate::users::{User, UserError, UserService};

const CAPACITY: usize = 20;
const ECONNRESET: i32 = 54;
const SQLITE_DB_FILE: &str = "server.db";

pub struct Server {
    listener: TcpListener,
    broadcaster: Sender<Arc<BroadcastMessage>>,
    user_service: Arc<UserService>,
}

#[derive(Debug)]
struct BroadcastMessage {
    from_addr: SocketAddr,
    message: Message,
}

struct UserSession {
    logged_user: Option<User>,
}

impl Server {
    pub async fn new(socket_addr: SocketAddr) -> Result<Server, ServerError> {
        info!("Listening on {}", socket_addr);

        let listener = TcpListener::bind(socket_addr)
            .await
            .map_err(|_| AddressInUseError(socket_addr))?;

        let connect_options = SqliteConnectOptions::new()
            .filename(SQLITE_DB_FILE)
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .connect_with(connect_options)
            .await?;
        let user_service = UserService::new(pool).await?;

        Ok(Server {
            listener,
            broadcaster: channel(CAPACITY).0,
            user_service: Arc::new(user_service),
        })
    }

    pub async fn listen(&self) -> Result<(), ServerError> {
        loop {
            let (tcp_stream, socket_addr) = self.listener.accept().await?;
            let broadcaster = self.broadcaster.clone();
            let message_tcp_stream = MessageTcpStream::<Message>::from_tcp_stream(tcp_stream)?;
            let user_service = Arc::clone(&self.user_service);
            tokio::spawn(async move {
                match Server::handle_client(
                    socket_addr,
                    message_tcp_stream,
                    broadcaster,
                    user_service,
                )
                .await
                {
                    Err(ServerError::TcpStreamError(MessageTcpStreamError::IOError(err)))
                        if err.raw_os_error() == Some(ECONNRESET) =>
                    {
                        info!("Client {} disconnected", socket_addr);
                    }
                    Err(err) => {
                        error!("{}", err);
                    }
                    _ => {}
                }
            });
        }
    }

    async fn handle_client(
        socket_addr: SocketAddr,
        mut tcp_stream: MessageTcpStream<Message>,
        broadcaster: Sender<Arc<BroadcastMessage>>,
        user_service: Arc<UserService>,
    ) -> Result<(), ServerError> {
        let mut user_session = UserSession { logged_user: None };
        let mut broadcast_sub = broadcaster.subscribe();
        loop {
            select! {
                broadcast_msg_try = broadcast_sub.recv() => {
                    let msg = broadcast_msg_try.unwrap();
                    if socket_addr != msg.from_addr && user_session.logged_user.is_some() {
                        tcp_stream.send_message(&msg.message).await?;
                    }
                }
                stream_msg_try = tcp_stream.read_next_message() => {
                    match stream_msg_try {
                        Err(stream_err) => { return Err(ServerError::from(stream_err)); }
                        Ok(Some(msg)) if user_session.logged_user.is_some() => {
                            Server::process_message_from_authenticated_client(
                                &socket_addr,
                                &user_session,
                                &mut tcp_stream,
                                &broadcaster,
                                &user_service,
                                msg
                            ).await?
                        },
                        Ok(Some(Message::Signup(login, passwd))) => {
                            match user_service.signup(&login, &passwd).await {
                                Ok(user) => {
                                    user_session.logged_user = Some(user);
                                    Server::send_text_reply(&format!("Welcome, {}", login), &mut tcp_stream).await?;
                                },
                                Err(UserError::UserAlreadyExistsError(_)) => {
                                    Server::send_text_reply(&format!("Username {} already exists!", login), &mut tcp_stream).await?;
                                }
                                Err(err) => {
                                    error!("{}", err);
                                }
                            }
                        },
                        Ok(Some(Message::Login(login, passwd))) => {
                            match user_service.authenticate(&login, &passwd).await {
                                Ok(user) => {
                                    user_session.logged_user = Some(user);
                                    Server::send_text_reply(&format!("Welcome, {}", login), &mut tcp_stream).await?;
                                },
                                Err(UserError::AuthenticationFailedError) => {
                                    Server::send_text_reply("Authentication failure", &mut tcp_stream).await?
                                }
                                Err(err) => {
                                    error!("{}", err);
                                    Server::send_text_reply("Server error", &mut tcp_stream).await?
                                }
                            }
                        }
                        Ok(Some(_)) => {
                            Server::send_text_reply("Permission denied, login first using .login <username> <password>", &mut tcp_stream).await?;
                        }
                        Err(stream_err) => {return Err(ServerError::from(stream_err));},
                        _ => (),
                    }
                }
            }
        }
    }

    async fn process_message_from_authenticated_client(
        socket_addr: &SocketAddr,
        user_session: &UserSession,
        tcp_stream: &mut MessageTcpStream<Message>,
        broadcaster: &Sender<Arc<BroadcastMessage>>,
        user_service: &Arc<UserService>,
        message: Message,
    ) -> Result<(), ServerError> {
        let user = user_session.logged_user.as_ref().unwrap();
        match message {
            Message::Login(_, _) | Message::Signup(_, _) => {
                Server::send_text_reply("Already logged in!", tcp_stream).await
            }
            Message::Passwd(new_passwd) => {
                user_service.change_password(user, &new_passwd).await?;
                Server::send_text_reply("Password updated successfully", tcp_stream).await
            }
            _ => {
                user_service.save_user_message(user, &message).await?;
                broadcaster
                    .send(Arc::new(BroadcastMessage {
                        from_addr: socket_addr.clone(),
                        message,
                    }))
                    .map(|_| ())
                    .map_err(|err| ServerError::GeneralError(err.to_string()))
            }
        }
    }

    async fn send_text_reply(
        text: &str,
        message_tcp_stream: &mut MessageTcpStream<Message>,
    ) -> Result<(), ServerError> {
        let message = Message::Text(text.to_string());
        message_tcp_stream
            .send_message(&message)
            .await
            .map_err(|err| ServerError::from(err))
    }
}

#[derive(Error, Debug)]
pub enum ServerError {
    #[error(transparent)]
    IOError(#[from] std::io::Error),
    #[error(transparent)]
    SqlError(#[from] sqlx::Error),
    #[error(transparent)]
    UserError(#[from] UserError),
    #[error(transparent)]
    TcpStreamError(#[from] MessageTcpStreamError),
    #[error("Listen address {0} already in use")]
    AddressInUseError(SocketAddr),
    #[error("{0}")]
    GeneralError(String),
}
