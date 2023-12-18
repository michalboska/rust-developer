use std::io::Write;
use std::net::{IpAddr, SocketAddr};
use std::process::exit;
use std::str::FromStr;
use std::string::ToString;

use anyhow::{Context, Error};
use clap::Parser;
use log::LevelFilter::Info;
use log::{debug, error};
use rocket::tokio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::watch::Sender;

use ex18_client::client::Client;
use ex18_server::server::Server;
use ex18_server::web::serve_web;
use ex18_shared::message::Message;

use crate::cli::{Cli, Modes};

mod cli;

const DEFAULT_HOST: &str = "0.0.0.0";
const DEFAULT_PORT: u16 = 11111;
const DEFAULT_PORT_WEB: u16 = 8080;

#[tokio::main]
async fn main() {
    env_logger::builder()
        .filter_level(Info)
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
    let web_port = cli.web_port.unwrap_or(DEFAULT_PORT_WEB);
    let exec_fn = |cli_mode: Modes| async move {
        let socket_addr =
            get_socket_addr(&address, port).context(format!("Invalid address {}", address))?;
        match cli_mode {
            Modes::CLIENT => client(&socket_addr).await,
            Modes::SERVER => {
                let socket_addr_web = get_socket_addr(&address, web_port)
                    .context(format!("Invalid address {}", address))?;
                server(socket_addr, socket_addr_web).await
            }
        }
    };
    if let Err(err) = exec_fn(cli.mode).await {
        error!("{}", err);
        exit(1);
    }
}

fn get_socket_addr(ip_addr_str: &str, port: u16) -> Result<SocketAddr, Error> {
    let ip_addr = IpAddr::from_str(ip_addr_str)?;
    Ok(SocketAddr::new(ip_addr, port))
}

async fn server(chat_listen_addr: SocketAddr, web_listen_addr: SocketAddr) -> Result<(), Error> {
    tokio::spawn(async move {
        return Server::new(chat_listen_addr)
            .await?
            .listen()
            .await
            .context(format!("Listening on address {} failed", chat_listen_addr));
    });
    let web_server_handle = tokio::spawn(async move {
        return serve_web(web_listen_addr).await.context("Web server error");
    });

    tokio::try_join!(web_server_handle)
        .map(|_| ())
        .context("Web server failed")
}

async fn client(socket_addr: &SocketAddr) -> Result<(), Error> {
    let (tx, rx) = tokio::sync::watch::channel(None);

    tokio::spawn(async {
        client_stdin_reader(tx).await.unwrap();
    });

    let mut client = Client::new(socket_addr, rx).await?;
    client.process_messages().await?;
    Ok(())
}

async fn client_stdin_reader(message_tx: Sender<Option<Message>>) -> Result<(), Error> {
    loop {
        let mut buf = String::new();
        let mut reader = BufReader::new(tokio::io::stdin());
        let read_bytes = reader
            .read_line(&mut buf)
            .await
            .context("Could not read from stdin")?;
        if read_bytes == 0 {
            debug!("stdin is empty, exitting...");
            return Ok(());
        }
        let message_result = Message::from_str(buf.trim()).await;
        match message_result {
            Ok(message) => {
                message_tx.send(Some(message))?;
            }
            Err(err) => {
                eprintln!("{}", err);
            }
        }
    }
}
