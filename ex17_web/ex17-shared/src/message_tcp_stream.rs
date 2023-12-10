use std::io::Cursor;
use std::marker::PhantomData;
use std::mem;

use bincode::{deserialize, serialize};
use log::{debug, error};
use rocket::tokio;
use serde::de::DeserializeOwned;
use serde::Serialize;
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::message_tcp_stream::MessageTcpStreamError::IncorrectTransmitByteCountError;

pub struct MessageTcpStream<T> {
    tcp_stream: TcpStream,
    _phantom: PhantomData<T>,
}

impl<T: Serialize + DeserializeOwned> MessageTcpStream<T> {
    pub fn from_tcp_stream(
        tcp_stream: TcpStream,
    ) -> Result<MessageTcpStream<T>, MessageTcpStreamError> {
        Ok(MessageTcpStream {
            tcp_stream,
            _phantom: PhantomData,
        })
    }

    pub async fn read_next_message(&mut self) -> Result<Option<T>, MessageTcpStreamError> {
        let read_fn = async {
            let mut size_buf = [0u8; 4];
            let expected_read = 4 * mem::size_of::<u8>();
            let bytes_read = self.tcp_stream.read(&mut size_buf).await?;
            if bytes_read != expected_read {
                return Err(IncorrectTransmitByteCountError(expected_read, bytes_read));
            }
            let message_size = u32::from_le_bytes(size_buf);
            if message_size == 0 {
                return Ok::<Option<Vec<u8>>, MessageTcpStreamError>(None);
            }
            Ok(Some(self.read_next_n_bytes(message_size as usize).await?))
        };
        match read_fn.await {
            Ok(Some(message_bytes)) => {
                debug!("Read binary message: {:?}", message_bytes);
                Ok(Some(deserialize(&message_bytes[..])?))
            }
            Err(MessageTcpStreamError::IOError(io_err)) if io_err.raw_os_error() == Some(35) => {
                Ok(None)
            }
            Err(e) => Err(e),
            Ok(None) => Ok(None),
        }
    }

    pub async fn send_message(&mut self, message: &T) -> Result<(), MessageTcpStreamError> {
        let vec = serialize(message)?;
        debug!("Serialized data: {:?}", vec);
        let size = vec.len() as u32;
        let size_byte_slice = u32::to_le_bytes(size);
        let expected_bytes_written = mem::size_of::<u32>() + vec.len() * mem::size_of::<u8>();
        let bytes_written =
            self.tcp_stream.write(&size_byte_slice).await? + self.tcp_stream.write(&vec).await?;
        self.tcp_stream.flush().await?;
        if bytes_written == expected_bytes_written {
            Ok(())
        } else {
            Err(IncorrectTransmitByteCountError(
                expected_bytes_written,
                bytes_written,
            ))
        }
    }

    async fn read_next_n_bytes(&mut self, n: usize) -> Result<Vec<u8>, MessageTcpStreamError> {
        let mut cursor = Cursor::new(vec![0u8; n]);
        let mut total_bytes = 0usize;
        while total_bytes < n {
            total_bytes += self.tcp_stream.read(&mut cursor.get_mut()).await?;
        }
        Ok(cursor.into_inner())
    }
}

#[derive(Error, Debug)]
pub enum MessageTcpStreamError {
    #[error(transparent)]
    SerdeError(#[from] bincode::Error),
    #[error(transparent)]
    IOError(#[from] std::io::Error),
    #[error("Expected to read {0} bytes, actually read {1} bytes")]
    IncorrectTransmitByteCountError(usize, usize),
}
