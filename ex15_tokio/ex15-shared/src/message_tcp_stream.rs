use std::io::Cursor;
use std::marker::PhantomData;

use bincode::{deserialize, serialize};
use log::{debug, error};
use serde::de::DeserializeOwned;
use serde::Serialize;
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

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
            self.tcp_stream.read(&mut size_buf).await?;

            let message_size = u32::from_le_bytes(size_buf);
            if message_size == 0 {
                return Ok::<Option<Vec<u8>>, MessageTcpStreamError>(None);
            }
            Ok(Some(self.read_next_n_bytes(message_size as usize).await?))
        };
        return match read_fn.await {
            Ok(Some(message_bytes)) => {
                debug!("Read binary message: {:?}", message_bytes);
                Ok(Some(deserialize(&message_bytes[..])?))
            }
            Err(MessageTcpStreamError::IOError(io_err)) if io_err.raw_os_error() == Some(35) => {
                Ok(None)
            }
            Err(e) => Err(e),
            Ok(None) => Ok(None),
        };
    }

    pub async fn send_message(&mut self, message: &T) -> Result<(), MessageTcpStreamError> {
        let vec = serialize(message)?;
        debug!("Serialized data: {:?}", vec);
        let size = vec.len() as u32;
        let size_byte_slice = u32::to_le_bytes(size);
        self.tcp_stream.write(&size_byte_slice).await?;
        self.tcp_stream.write(&vec).await?;
        self.tcp_stream.flush().await?;
        Ok(())
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
}
