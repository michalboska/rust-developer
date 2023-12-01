use std::fmt::Debug;
use std::net::SocketAddr;
use std::sync::Arc;

use log::info;
use serde::de::DeserializeOwned;
use serde::Serialize;
use thiserror::Error;
use tokio::net::TcpListener;
use tokio::select;
use tokio::sync::broadcast::{channel, Sender};

use ex15_shared::message_tcp_stream::{MessageTcpStream, MessageTcpStreamError};

use crate::server::ServerError::AddressInUseError;

const CAPACITY: usize = 20;

pub struct Server<T: Serialize + DeserializeOwned + Send + Sync + Debug + 'static> {
    listener: TcpListener,
    broadcaster: Sender<Arc<BroadcastMessage<T>>>,
}

#[derive(Clone, Debug)]
struct BroadcastMessage<T: Send + Sync + Debug + 'static> {
    from_addr: SocketAddr,
    message: T,
}

impl<T: Serialize + DeserializeOwned + Send + Sync + Debug + 'static> Server<T> {
    pub async fn new(socket_addr: SocketAddr) -> Result<Server<T>, ServerError> {
        info!("Listening on {}", socket_addr);

        let listener = TcpListener::bind(socket_addr)
            .await
            .map_err(|_| AddressInUseError(socket_addr))?;

        Ok(Server {
            listener,
            broadcaster: channel(CAPACITY).0,
        })
    }

    pub async fn listen(&self) -> Result<(), ServerError> {
        loop {
            let (tcp_stream, socket_addr) = self.listener.accept().await?;
            let broadcaster = self.broadcaster.clone();
            let message_tcp_stream = MessageTcpStream::<T>::from_tcp_stream(tcp_stream)?;
            tokio::spawn(async move {
                Server::<T>::handle_client(socket_addr, message_tcp_stream, broadcaster)
                    .await
                    .unwrap();
            });
        }
    }

    async fn handle_client(
        socket_addr: SocketAddr,
        mut tcp_stream: MessageTcpStream<T>,
        broadcaster: Sender<Arc<BroadcastMessage<T>>>,
    ) -> Result<(), ServerError> {
        let mut broadcast_sub = broadcaster.subscribe();
        loop {
            select! {
                broadcast_msg_try = broadcast_sub.recv() => {
                    let msg = broadcast_msg_try.unwrap();
                    if socket_addr != msg.from_addr {
                        tcp_stream.send_message(&msg.message).await?;
                    }
                }
                stream_msg_try = tcp_stream.read_next_message() => {
                    match stream_msg_try {
                        Ok(Some(msg)) => { broadcaster.send(Arc::new(BroadcastMessage {
                            from_addr: socket_addr.clone(),
                            message: msg,
                        })).unwrap();
                            },
                        Err(stream_err) => {return Err(ServerError::from(stream_err));},
                        _ => {},
                    }
                }
            }
        }
    }
}

#[derive(Error, Debug)]
pub enum ServerError {
    #[error(transparent)]
    IOError(#[from] std::io::Error),
    #[error(transparent)]
    TcpStreamError(#[from] MessageTcpStreamError),
    #[error("Listen address {0} already in use")]
    AddressInUseError(SocketAddr),
}
