use std::io::{Cursor, Read, Write};
use std::marker::PhantomData;
use std::mem::size_of;
use std::net::TcpStream;
use std::time::Duration;

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
        read_interval: Duration,
    ) -> Result<MessageTcpStream<T>, BoxDynError> {
        tcp_stream.set_read_timeout(Some(read_interval))?;
        tcp_stream.set_nonblocking(false)?;
        Ok(MessageTcpStream {
            tcp_stream,
            _phantom: PhantomData,
        })
    }

    pub fn read_next_message(&mut self) -> Result<Option<T>, BoxDynError> {
        let mut buf_size: [u8; 4] = [0; 4];
        match self.tcp_stream.read(&mut buf_size) {
            Ok(_) => {
                let message_size = u32::from_le_bytes(buf_size);
                let message_bytes = self.read_next_n_bytes(message_size as usize)?;
                Ok(bincode::deserialize(&message_bytes[..])?)
            }
            Err(_) => Ok(None),
        }
    }

    pub fn send_message_to_server(&mut self, message: &T) -> Result<(), BoxDynError> {
        let vec = bincode::serialize(message)?;
        let size = vec.len() as u32;
        let written_bytes =
            self.tcp_stream.write(&u32::to_le_bytes(size))? + self.tcp_stream.write(&vec)?;
        self.tcp_stream.flush()?;
        if written_bytes == size as usize + size_of::<u32>() {
            Ok(())
        } else {
            Err(BoxDynError::from("Incorrect amount of bytes written"))
        }
    }

    fn read_next_n_bytes(&mut self, n: usize) -> Result<Vec<u8>, BoxDynError> {
        let mut cursor = Cursor::new(vec![0u8; n]);
        let mut total_bytes = 0usize;
        while total_bytes < n {
            total_bytes += self.tcp_stream.read(cursor.get_mut())?;
        }
        Ok(cursor.into_inner())
    }
}
