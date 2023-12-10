use std::fs::File;
use std::io::Write;
use std::net::SocketAddr;
use std::ops::Deref;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use log::info;
use rocket::tokio;
use thiserror::Error;
use tokio::net::TcpStream;
use tokio::sync::watch::Receiver;
use tokio::{fs, select};

use ex17_shared::message::Message;
use ex17_shared::message_tcp_stream::{MessageTcpStream, MessageTcpStreamError};

use crate::client::ClientError::{
    ConnectError, IllegalArgumentError, IncorrectTransmitByteCountError,
};

pub struct Client {
    message_stream: MessageTcpStream<Message>,
    stdin_input_rx: Receiver<Option<Message>>,
}

impl Client {
    pub async fn new(
        socket_addr: &SocketAddr,
        stdin_input_rx: Receiver<Option<Message>>,
    ) -> Result<Client, ClientError> {
        fs::create_dir_all("files").await?;
        fs::create_dir_all("images").await?;
        info!("Connecting to {}", socket_addr);
        Ok(Client {
            message_stream: MessageTcpStream::from_tcp_stream(
                TcpStream::connect(socket_addr)
                    .await
                    .map_err(|_| ConnectError(socket_addr.clone()))?,
            )?,
            stdin_input_rx,
        })
    }

    pub async fn process_messages(&mut self) -> Result<(), ClientError> {
        loop {
            select! {
                stdin_event = self.stdin_input_rx.changed() => {
                    if stdin_event.is_err() {
                        return Ok(());
                    }
                    let message_ref = self.stdin_input_rx.borrow_and_update();
                    match message_ref.deref() {
                        Some(Message::Quit) => {return Ok(());},
                        Some(message) => {
                            self.message_stream.send_message(message).await?;
                        },
                        None => {},
                    }
                }
                server_event = self.message_stream.read_next_message() => {
                    match server_event {
                        Ok(Some(message)) => {
                            Client::process_message(&message)?;
                        }
                        Err(err) => { return Err(ClientError::from(err));}
                        _ => {}
                    }
                }
            }
        }
    }

    fn process_message(message: &Message) -> Result<(), ClientError> {
        match message {
            Message::File(_, _) | Message::Image(_) => Client::save_file(message),
            Message::Text(ref text) => {
                println!("{}", text);
                Ok(())
            }
            _ => Err(IllegalArgumentError("Unknown message type".to_string())),
        }
    }

    fn save_file(message: &Message) -> Result<(), ClientError> {
        let path_str = match message {
            Message::File(file_path, _) => Ok(format!(
                "files/{}",
                Client::get_file_name_from_path(file_path)?
            )),
            Message::Image(_) => {
                let duration = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
                Ok(format!("images/{}", duration.as_millis()))
            }
            _ => Err(IllegalArgumentError(
                "Cannot save this message type as file".to_string(),
            )),
        }?;
        let content = match message {
            Message::File(_, vec) => Ok(vec),
            Message::Image(vec) => Ok(vec),
            _ => Err(IllegalArgumentError(
                "Cannot save this message type as file".to_string(),
            )),
        }?;
        let mut file = File::options()
            .write(true)
            .truncate(true)
            .create(true)
            .open(path_str)?;
        let bytes_written = file.write(content)?;
        if bytes_written == content.len() {
            Ok(())
        } else {
            Err(IncorrectTransmitByteCountError(
                content.len(),
                bytes_written,
            ))
        }
    }

    fn get_file_name_from_path(path_str: &str) -> Result<&str, ClientError> {
        let path = Path::new(path_str);
        let file_path_error =
            || IllegalArgumentError(format!("Invalid path received: {}", path_str));
        let file_name = path
            .file_name()
            .ok_or_else(file_path_error)
            .and_then(|x| x.to_str().ok_or_else(file_path_error))?;
        Ok(file_name)
    }
}

#[derive(Error, Debug)]
pub enum ClientError {
    #[error("Could not connect to {0}")]
    ConnectError(SocketAddr),
    #[error(transparent)]
    IOError(#[from] std::io::Error),
    #[error(transparent)]
    TcpStreamError(#[from] MessageTcpStreamError),
    #[error("Invalid filesystem path {0}")]
    InvalidFsPathError(Box<Path>),
    #[error("{0}")]
    IllegalArgumentError(String),
    #[error("Expected to read {0} bytes, actually read {1} bytes")]
    IncorrectTransmitByteCountError(usize, usize),
}
