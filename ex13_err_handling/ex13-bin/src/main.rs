use std::io::{BufRead, Write};
use std::net::{IpAddr, SocketAddr};
use std::process::exit;
use std::str::FromStr;
use std::string::ToString;
use std::{io, thread};

use anyhow::{Context, Error};
use clap::Parser;
use flume::Sender;
use log::LevelFilter::Debug;
use log::{debug, error};

use ex13_client::client::Client;
use ex13_server::server::Server;
use ex13_shared::message::Message;

use crate::cli::{Cli, Modes};

mod cli;

const DEFAULT_HOST: &str = "127.0.0.1";
const DEFAULT_PORT: u16 = 11111;

fn main() {
    env_logger::builder()
        .filter_level(Debug)
        .format(|buf, record| {
            buf.write_fmt(format_args!(
                "{}:{} {} [{}] - {}\n",
                record.file().unwrap_or("unknown"),
                record.line().unwrap_or(0),
                chrono::Local::now().format("%Y-%m-%dT%H:%M:%S"),
                record.level(),
                record.args()
            ))?;
            Ok(())
        })
        .init();

    let cli = Cli::parse();
    let address = cli.hostname.unwrap_or(DEFAULT_HOST.to_string());
    let port = cli.port.unwrap_or(DEFAULT_PORT);
    let exec_fn = |cli_mode: Modes| {
        let socket_addr =
            get_socket_addr(&address, port).context(format!("Invalid address {}", address))?;
        match cli_mode {
            Modes::CLIENT => client(&socket_addr),
            Modes::SERVER => Server::<Message>::new(socket_addr)?
                .listen_blocking()
                .context("asd"),
        }
    };
    if let Err(err) = exec_fn(cli.mode) {
        error!("{}", err);
        exit(1);
    }
}

fn get_socket_addr(ip_addr_str: &str, port: u16) -> Result<SocketAddr, Error> {
    let ip_addr = IpAddr::from_str(ip_addr_str)?;
    return Ok(SocketAddr::new(ip_addr, port));
}

fn client(socket_addr: &SocketAddr) -> Result<(), Error> {
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
        let read_result = stdin_lock.read_line(&mut buf);
        let buf_trim = buf.trim();
        match read_result {
            Ok(0) => {
                debug!("stdin is empty, exitting...");
                exit(0);
            }
            Ok(_) => match Message::from_str(buf_trim) {
                Ok(message) => {
                    debug!("Read valid message: {}", buf_trim);
                    if matches!(message, Message::Quit) {
                        break;
                    } else {
                        message_tx.send(message).unwrap();
                    }
                }
                Err(err) => {
                    error!("{}", err);
                }
            },
            Err(err) => {
                error!("{}", err);
                break;
            }
        }
    }
}
