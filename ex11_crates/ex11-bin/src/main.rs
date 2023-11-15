use std::io::BufRead;
use std::net::{IpAddr, SocketAddr};
use std::process::exit;
use std::str::FromStr;
use std::string::ToString;
use std::{io, thread};

use clap::Parser;
use flume::Sender;
use log::error;
use log::LevelFilter::Info;

use ex11_client::client::Client;
use ex11_server::server::Server;
use ex11_shared::err::BoxDynError;
use ex11_shared::message::Message;

use crate::cli::{Cli, Modes};

mod cli;

const DEFAULT_HOST: &str = "127.0.0.1";
const DEFAULT_PORT: u16 = 11111;

fn main() {
    env_logger::builder().filter_level(Info).init();

    let cli = Cli::parse();
    let address = cli.hostname.unwrap_or(DEFAULT_HOST.to_string());
    let port = cli.port.unwrap_or(DEFAULT_PORT);
    let socket_addr = get_socket_addr(&address, port);
    if let Err(e) = socket_addr {
        error!("Invalid address {}: {}", address, e);
        exit(1);
    }
    let exec_result = match cli.mode {
        Modes::CLIENT => client(&socket_addr.unwrap()),
        Modes::SERVER => Server::<Message>::new(&socket_addr.unwrap())
            .unwrap()
            .listen_blocking(),
    };
    if let Err(err) = exec_result {
        error!("{}", err);
        exit(1);
    }
}

fn get_socket_addr(ip_addr_str: &str, port: u16) -> Result<SocketAddr, BoxDynError> {
    let ip_addr = IpAddr::from_str(ip_addr_str)?;
    return Ok(SocketAddr::new(ip_addr, port));
}

fn client(socket_addr: &SocketAddr) -> Result<(), BoxDynError> {
    let (tx, rx) = flume::unbounded();
    thread::spawn(move || {
        client_stdin_reader(tx);
    });
    let mut client = Client::new(socket_addr, rx)?;
    client.process_messages()?;
    Ok(())
}

fn client_stdin_reader(message_tx: Sender<Message>) {
    let mut stdin_lock = io::stdin().lock();
    let mut buf = String::new();
    loop {
        buf.clear();
        match stdin_lock.read_line(&mut buf) {
            Ok(0) => {
                break;
            }
            Ok(_) => match Message::from_str(&buf.trim()) {
                Ok(message) => {
                    if matches!(message, Message::Quit) {
                        break;
                    } else {
                        message_tx.send(message).unwrap();
                    }
                }
                Err(err) => {
                    eprintln!("{}", err);
                }
            },
            Err(err) => {
                eprintln!("{}", err);
                break;
            }
        }
    }
}
