use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub struct ArgParseError {
    pub msg: String,
}

impl Display for ArgParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.msg)
    }
}

impl Error for ArgParseError {}
