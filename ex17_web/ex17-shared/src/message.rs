use std::fmt::Debug;

use anyhow::{bail, Context, Result};
use lazy_static::lazy_static;
use regex::Regex;
use rocket::tokio;
use serde_derive::{Deserialize, Serialize};
use tokio::fs::File;
use tokio::io::AsyncReadExt;

use crate::message::Message::Signup;

lazy_static! {
    static ref REGEX_COMPLEX: Regex = Regex::new(r"^\.(\S+) (\S+ )?(\S+)$").unwrap();
    static ref REGEX_SIMPLE: Regex = Regex::new(r"^\.(\S+)$").unwrap();
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Message {
    File(String, Vec<u8>),
    Image(Vec<u8>),
    Text(String),
    Login(String, String),
    Signup(String, String),
    Passwd(String),
    Quit,
}

impl Message {
    /// Creates an instance of `Message` from provided `&str`
    pub async fn from_str(str: &str) -> Result<Message> {
        if let Some(caps) = REGEX_COMPLEX.captures(str) {
            let arg = caps.get(3).unwrap().as_str();
            let optional_arg_option = caps.get(2).map(|m| m.as_str().trim());
            return match caps.get(1).unwrap().as_str() {
                "file" => Ok(Message::File(
                    arg.to_string(),
                    Message::buf_from_file(arg).await?,
                )),
                "image" => Ok(Message::Image(Message::buf_from_file(arg).await?)),
                "login" => match optional_arg_option {
                    None => {
                        bail!("Login requires two arguments - username and password")
                    }
                    Some(optional_arg) => {
                        Ok(Message::Login(optional_arg.to_string(), arg.to_string()))
                    }
                },
                "signup" => match optional_arg_option {
                    None => {
                        bail!("Use .signup <new_username> <new_password>")
                    }
                    Some(optional_arg) => Ok(Signup(optional_arg.to_string(), arg.to_string())),
                },
                "passwd" => match optional_arg_option {
                    None => {
                        bail!("Passwd requires the new password to be typed two times")
                    }
                    Some(optional_arg) => {
                        if optional_arg == arg {
                            Ok(Message::Passwd(arg.to_string()))
                        } else {
                            bail!("Passwords don't match!")
                        }
                    }
                },
                _ => Ok(Message::Text(arg.to_string())),
            };
        } else if let Some(caps) = REGEX_SIMPLE.captures(str) {
            let arg = caps.get(1).unwrap().as_str();
            return match arg {
                "quit" => Ok(Message::Quit),
                _ => bail!("Unknown command"),
            };
        }
        Ok(Message::Text(str.to_string()))
    }

    async fn buf_from_file(path_str: &str) -> Result<Vec<u8>> {
        let mut file = File::open(path_str)
            .await
            .context(format!("Cannot open file {}", path_str))?;
        let file_len = file
            .metadata()
            .await
            .context(format!("Cannot get metadata for file {}", path_str))?
            .len() as usize;
        let mut buf = Vec::with_capacity(file_len);
        file.read_to_end(&mut buf)
            .await
            .context(format!("Cannot read file {}", path_str))?;
        Ok(buf)
    }
}

#[cfg(test)]
mod tests {
    use rocket::tokio;

    use crate::message::Message;

    #[tokio::test]
    async fn from_str_creates_a_text_message() {
        let text = "text";

        let message = Message::from_str(text).await.unwrap();

        match message {
            Message::Text(string) => {
                assert_eq!(text.to_string(), string);
            }
            _ => {
                panic!("Not text message with proper text: {:?}", message);
            }
        }
    }

    #[tokio::test]
    async fn from_str_creates_a_quit_message() {
        let text = ".quit";

        let message = Message::from_str(text).await.unwrap();

        assert!(matches!(message, Message::Quit));
    }

    #[tokio::test]
    async fn from_str_creates_a_login_message() {
        let user = "user";
        let pass = "pass";
        let text = format!(".login {} {}", user, pass);

        let message = Message::from_str(&text).await.unwrap();

        match message {
            Message::Login(msg_user, msg_pass) => {
                assert_eq!(user, msg_user);
                assert_eq!(pass, msg_pass);
            }
            _ => {
                panic!("{:?} is not Login", message);
            }
        }
    }

    #[tokio::test]
    async fn from_str_creates_a_signup_message() {
        let user = "user";
        let pass = "pass";
        let text = format!(".signup {} {}", user, pass);

        let message = Message::from_str(&text).await.unwrap();

        match message {
            Message::Signup(msg_user, msg_pass) => {
                assert_eq!(user, msg_user);
                assert_eq!(pass, msg_pass);
            }
            _ => {
                panic!("{:?} is not Signup", message);
            }
        }
    }

    #[tokio::test]
    async fn from_str_creates_a_password_message_if_passwords_match() {
        let pass = "pass";
        let text = format!(".passwd {} {}", pass, pass);

        let message = Message::from_str(&text).await.unwrap();

        match message {
            Message::Passwd(msg_pass) => {
                assert_eq!(pass, msg_pass);
            }
            _ => {
                panic!("{:?} is not passwd", message);
            }
        }
    }

    #[tokio::test]
    async fn from_str_returns_error_if_passwords_dont_match() {
        let pass = "pass";
        let pass2 = "!pass";
        let text = format!(".passwd {} {}", pass, pass2);

        let message = Message::from_str(&text).await;

        assert!(matches!(message, Err(_)));
    }
}
