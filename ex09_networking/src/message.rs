use std::error::Error;
use std::fs::File;
use std::io::Read;

use lazy_static::lazy_static;
use regex::Regex;
use serde_derive::{Deserialize, Serialize};

use crate::message::Message::Image;

lazy_static! {
    static ref REGEX: Regex = Regex::new("^\\.(\\S+) (\\S+)$").unwrap();
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Message {
    File(String, Vec<u8>),
    Image(Vec<u8>),
    Text(String),
    Quit,
}

impl Message {
    pub fn from_str(str: &str) -> Result<Message, Box<dyn Error>> {
        if let Some(caps) = REGEX.captures(str) {
            let arg = caps.get(2).unwrap().as_str();
            return match caps.get(1).unwrap().as_str() {
                "file" => Ok(Message::File(arg.to_string(), Message::buf_from_file(arg)?)),
                "image" => Ok(Image(Message::buf_from_file(arg)?)),
                "quit" => Ok(Message::Quit),
                _ => Ok(Message::Text(arg.to_string())),
            };
        }
        Ok(Message::Text(str.to_string()))
    }

    fn buf_from_file(path_str: &str) -> Result<Vec<u8>, Box<dyn Error>> {
        let mut file = File::open(path_str)?;
        let file_len = file.metadata()?.len() as usize;
        let mut buf = Vec::with_capacity(file_len);
        file.read_to_end(&mut buf)?;
        Ok(buf)
    }
}

// impl Display for Message {
//     fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
//         return write!(
//             f,
//             "Message: {:?} body: [{} bytes]",
//             self.message_type,
//             self.binary_body.len()
//         );
//     }
// }
