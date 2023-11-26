use std::fs;
use std::fs::File;
use std::io::Write;
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use flume::Receiver;
use log::info;
use thiserror::Error;

use crate::client::ClientError::{ConnectError, IllegalArgumentError};
use ex13_shared::message::Message;
use ex13_shared::message_tcp_stream::{MessageTcpStream, MessageTcpStreamError};

static DEFAULT_TIMEOUT: Duration = Duration::from_millis(500);

pub struct Client {
    message_stream: MessageTcpStream<Message>,
    stdin_input_rx: Receiver<Message>,
}

impl Client {
    pub fn new(
        socket_addr: &SocketAddr,
        stdin_input_rx: Receiver<Message>,
    ) -> Result<Client, ClientError> {
        fs::create_dir_all("files")?;
        fs::create_dir_all("images")?;
        info!("Connecting to {}", socket_addr);
        Ok(Client {
            message_stream: MessageTcpStream::from_tcp_stream(
                TcpStream::connect(socket_addr).map_err(|_| ConnectError(socket_addr.clone()))?,
                Some(DEFAULT_TIMEOUT),
            )?,
            stdin_input_rx,
        })
    }

    pub fn process_messages(&mut self) -> Result<(), ClientError> {
        loop {
            // was there any input from stdin?
            let result = self.stdin_input_rx.recv_timeout(DEFAULT_TIMEOUT);
            if let Ok(message) = result {
                self.message_stream.send_message(&message)?;
                if matches!(message, Message::Quit) {
                    return Ok(());
                }
            }

            // was there any message from server?
            let result_option = self.message_stream.read_next_message()?;
            if let Some(message) = result_option {
                if matches!(message, Message::Quit) {
                    return Ok(());
                }
                Client::process_message(&message)?;
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
        file.write(content)?;
        Ok(())
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
}
