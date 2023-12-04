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
            let mut session = UserSession {
                logged_user: None,
                socket_addr,
                tcp_stream: message_tcp_stream,
                broadcaster,
                user_service: Arc::clone(&self.user_service),
            };

            tokio::spawn(async move {
                match session.run().await {
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
}

struct UserSession {
    socket_addr: SocketAddr,
    tcp_stream: MessageTcpStream<Message>,
    broadcaster: Sender<Arc<BroadcastMessage>>,
    user_service: Arc<UserService>,
    logged_user: Option<User>,
}

impl UserSession {
    pub async fn run(&mut self) -> Result<(), ServerError> {
        let mut broadcast_sub = self.broadcaster.subscribe();
        loop {
            select! {
                broadcast_msg_try = broadcast_sub.recv() => {
                    let msg = broadcast_msg_try.unwrap();
                    if self.socket_addr != msg.from_addr && self.logged_user.is_some() {
                        self.tcp_stream.send_message(&msg.message).await?;
                    }
                }
                stream_msg_try = self.tcp_stream.read_next_message() => {
                    match stream_msg_try {
                        Err(stream_err) => { return Err(ServerError::from(stream_err)); }
                        Ok(Some(msg)) if self.logged_user.is_some() => {
                            self.process_message_from_authenticated_client(msg).await?
                        },
                        Ok(Some(Message::Signup(login, passwd))) => {
                            match self.user_service.signup(&login, &passwd).await {
                                Ok(user) => {
                                    self.logged_user = Some(user);
                                    self.send_text_reply(&format!("Welcome, {}", login)).await?;
                                },
                                Err(UserError::UserAlreadyExistsError(_)) => {
                                    self.send_text_reply(&format!("Username {} already exists!", login)).await?;
                                }
                                Err(err) => {
                                    error!("{}", err);
                                }
                            }
                        },
                        Ok(Some(Message::Login(login, passwd))) => {
                            match self.user_service.authenticate(&login, &passwd).await {
                                Ok(user) => {
                                    self.logged_user = Some(user);
                                    self.send_text_reply(&format!("Welcome, {}", login)).await?;
                                },
                                Err(UserError::AuthenticationFailedError) => {
                                    self.send_text_reply("Authentication failure").await?
                                }
                                Err(err) => {
                                    error!("{}", err);
                                    self.send_text_reply("Server error").await?
                                }
                            }
                        }
                        Ok(Some(_)) => {
                            self.send_text_reply("Permission denied, login first using .login <username> <password>").await?;
                        }
                        Err(stream_err) => {return Err(ServerError::from(stream_err));},
                        _ => (),
                    }
                }
            }
        }
    }

    async fn process_message_from_authenticated_client(
        &mut self,
        message: Message,
    ) -> Result<(), ServerError> {
        let user = self.logged_user.as_ref().unwrap();
        match message {
            Message::Login(_, _) | Message::Signup(_, _) => {
                self.send_text_reply("Already logged in!").await
            }
            Message::Passwd(new_passwd) => {
                self.user_service.change_password(user, &new_passwd).await?;
                self.send_text_reply("Password updated successfully").await
            }
            _ => {
                self.user_service.save_user_message(user, &message).await?;
                self.broadcaster
                    .send(Arc::new(BroadcastMessage {
                        from_addr: self.socket_addr.clone(),
                        message,
                    }))
                    .map(|_| ())
                    .map_err(|err| ServerError::GeneralError(err.to_string()))
            }
        }
    }

    async fn send_text_reply(&mut self, text: &str) -> Result<(), ServerError> {
        let message = Message::Text(text.to_string());
        self.tcp_stream
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
