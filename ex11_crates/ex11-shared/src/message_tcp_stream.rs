use bincode::{deserialize, serialize};
use std::io::{Cursor, Read, Write};
use std::marker::PhantomData;
use std::net::TcpStream;
use std::time::Duration;

use log::debug;
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::err::BoxDynError;

pub struct MessageTcpStream<T> {
    tcp_stream: TcpStream,
    _phantom: PhantomData<T>,
}

impl<T: Serialize + DeserializeOwned> MessageTcpStream<T> {
    pub fn from_tcp_stream(
        tcp_stream: TcpStream,
        read_interval: Option<Duration>,
    ) -> Result<MessageTcpStream<T>, BoxDynError> {
        tcp_stream.set_read_timeout(read_interval)?;
        Ok(MessageTcpStream {
            tcp_stream,
            _phantom: PhantomData,
        })
    }

    pub fn read_next_message(&mut self) -> Result<Option<T>, BoxDynError> {
        let mut size_buf = [0u8; 4];
        match self.tcp_stream.read(&mut size_buf) {
            Ok(_) => {
                let message_size = u32::from_le_bytes(size_buf);
                if message_size == 0 {
                    return Ok(None);
                }
                let message_bytes = self.read_next_n_bytes(message_size as usize)?;
                debug!("Read binary message: {:?}", message_bytes);
                let deserialized_message = deserialize(&message_bytes[..])?;
                return Ok(Some(deserialized_message));
            }
            Err(_) => Ok(None),
        }
    }

    pub fn send_message(&mut self, message: &T) -> Result<(), BoxDynError> {
        let vec = serialize(message)?;
        debug!("Serialized data: {:?}", vec);
        let size = vec.len() as u32;
        let size_byte_slice = u32::to_le_bytes(size);
        self.tcp_stream.write(&size_byte_slice)?;
        self.tcp_stream.write(&vec)?;
        self.tcp_stream.flush()?;
        Ok(())
    }

    fn read_next_n_bytes(&mut self, n: usize) -> Result<Vec<u8>, BoxDynError> {
        let mut cursor = Cursor::new(vec![0u8; n]);
        let mut total_bytes = 0usize;
        while total_bytes < n {
            total_bytes += self.tcp_stream.read(&mut cursor.get_mut())?;
        }
        Ok(cursor.into_inner())
    }
}
