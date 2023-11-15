use std::error::Error;
use std::fmt;
use std::fmt::{Display, Formatter};
pub type BoxDynError = Box<dyn Error>;

#[derive(Debug)]
pub struct RuntimeError {
    msg: String,
}

impl Display for RuntimeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(&self.msg)
    }
}

impl Error for RuntimeError {}

impl RuntimeError {
    pub fn from_str(str: &str) -> RuntimeError {
        RuntimeError {
            msg: str.to_string(),
        }
    }

    pub fn from_string(string: String) -> RuntimeError {
        RuntimeError { msg: string }
    }

    pub fn box_from_str(str: &str) -> BoxDynError {
        Box::new(RuntimeError::from_str(str))
    }

    pub fn box_from_string(string: String) -> BoxDynError {
        Box::new(RuntimeError::from_string(string))
    }
}
