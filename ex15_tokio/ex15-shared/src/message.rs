use std::fmt::Debug;
use std::fs::File;
use std::io::Read;

use anyhow::{Context, Result};
use lazy_static::lazy_static;
use regex::Regex;
use serde_derive::{Deserialize, Serialize};

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
    pub fn from_str(str: &str) -> Result<Message> {
        if let Some(caps) = REGEX.captures(str) {
            let arg = caps.get(2).unwrap().as_str();
            return match caps.get(1).unwrap().as_str() {
                "file" => Ok(Message::File(arg.to_string(), Message::buf_from_file(arg)?)),
                "image" => Ok(Message::Image(Message::buf_from_file(arg)?)),
                "quit" => Ok(Message::Quit),
                _ => Ok(Message::Text(arg.to_string())),
            };
        }
        Ok(Message::Text(str.to_string()))
    }

    fn buf_from_file(path_str: &str) -> Result<Vec<u8>> {
        let mut file = File::open(path_str).context(format!("Cannot open file {}", path_str))?;
        let file_len = file
            .metadata()
            .context(format!("Cannot get metadata for file {}", path_str))?
            .len() as usize;
        let mut buf = Vec::with_capacity(file_len);
        file.read_to_end(&mut buf)
            .context(format!("Cannot read file {}", path_str))?;
        Ok(buf)
    }
}
