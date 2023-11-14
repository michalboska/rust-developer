use fs::File;
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{BufRead, Read, Write};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::path::Path;
use std::process::exit;
use std::rc::Rc;
use std::str::FromStr;
use std::string::ToString;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::{fs, io, thread};

use clap::Parser;
use flume::Sender;

use crate::cli::{Cli, Modes};
use crate::err::{BoxDynError, RuntimeError};
use crate::message::Message;
use crate::message_tcp_stream::MessageTcpStream;

mod cli;
mod err;
mod message;
mod message_tcp_stream;

const DEFAULT_HOST: &str = "127.0.0.1";
const DEFAULT_PORT: u16 = 11111;
const DEFAULT_TIMEOUT: Duration = Duration::from_millis(500);

fn main() {
    let cli = Cli::parse();
    let address = cli.hostname.unwrap_or(DEFAULT_HOST.to_string());
    let port = cli.port.unwrap_or(DEFAULT_PORT);
    let socket_addr = get_socket_addr(&address, port);
    if let Err(e) = socket_addr {
        eprintln!("Invalid address {}: {}", address, e);
        exit(1);
    }
    let exec_result = match cli.mode {
        Modes::CLIENT => client(&socket_addr.unwrap()),
        Modes::SERVER => server(&socket_addr.unwrap()),
    };
    if let Err(err) = exec_result {
        eprintln!("{}", err);
        exit(1);
    }
}

fn get_socket_addr(ip_addr_str: &str, port: u16) -> Result<SocketAddr, BoxDynError> {
    let ip_addr = IpAddr::from_str(ip_addr_str)?;
    return Ok(SocketAddr::new(ip_addr, port));
}

fn server(listen_addr: &SocketAddr) -> Result<(), BoxDynError> {
    println!("Listening on {}", listen_addr);
    let listener = TcpListener::bind(listen_addr)?;
    let mut clients: HashMap<SocketAddr, Rc<RefCell<MessageTcpStream<Message>>>> = HashMap::new();

    for stream_result in listener.incoming() {
        let stream = stream_result?;
        let addr = stream.peer_addr()?;
        let message_stream = MessageTcpStream::from_tcp_stream(stream, DEFAULT_TIMEOUT)?;

        let stream_rc = Rc::new(RefCell::new(message_stream));
        clients.insert(addr.clone(), Rc::clone(&stream_rc));
        server_handle_client(Rc::clone(&stream_rc), &addr, &mut clients);
    }

    Ok(())
}

fn server_handle_client(
    stream: Rc<RefCell<MessageTcpStream<Message>>>,
    socket_addr: &SocketAddr,
    clients: &mut HashMap<SocketAddr, Rc<RefCell<MessageTcpStream<Message>>>>,
) {
    loop {
        let message_option = match stream.borrow_mut().read_next_message() {
            Err(_) => {
                break;
            }
            Ok(Some(msg)) => Some(msg),
            _ => None,
        };

        if let Some(message) = message_option {
            server_process_client_message(clients, socket_addr, &message);
        }
    }
}

fn server_process_client_message(
    clients: &mut HashMap<SocketAddr, Rc<RefCell<MessageTcpStream<Message>>>>,
    client_socket_addr: &SocketAddr,
    message: &Message,
) {
    let mut clients_to_remove = Vec::<SocketAddr>::new();

    clients
        .iter()
        .filter(|(&it_socket_addr, _)| it_socket_addr == *client_socket_addr)
        .for_each(|(sock_addr, rc)| {
            let result = rc.borrow_mut().send_message_to_server(message);
            if let Err(_) = result {
                clients_to_remove.push(sock_addr.clone());
            }
        });

    for socket_addr in clients_to_remove {
        clients.remove(&socket_addr);
    }
}

fn client(server_addr: &SocketAddr) -> Result<(), BoxDynError> {
    println!("Connecting to {}", server_addr);
    let mut tcp_stream = MessageTcpStream::<Message>::from_tcp_stream(
        TcpStream::connect(server_addr)?,
        DEFAULT_TIMEOUT,
    )?;
    let (tx, rx) = flume::unbounded();
    thread::spawn(move || client_stdin_reader(tx));
    loop {
        // was there any input from stdin?
        let result = rx.recv_timeout(DEFAULT_TIMEOUT);
        if let Ok(message) = result {
            tcp_stream.send_message_to_server(&message)?;
            if matches!(message, Message::Quit) {
                return Ok(());
            }
        }

        // was there any message from server?
        let result_option = tcp_stream.read_next_message()?;
        if let Some(message) = result_option {
            if matches!(message, Message::Quit) {
                return Ok(());
            }
            client_process_message(&message)?;
        }
    }
}

fn client_process_message(message: &Message) -> Result<(), BoxDynError> {
    match message {
        Message::File(_, _) | Message::Image(_) => save_file(message),
        Message::Text(ref text) => {
            println!("{}", text);
            Ok(())
        }
        _ => Err(Box::new(RuntimeError::from_str("Unknown message type"))),
    }
}

fn save_file(message: &Message) -> Result<(), BoxDynError> {
    let path_str = match message {
        Message::File(file_path, _) => Ok(format!("files/{}", get_file_name_from_path(file_path)?)),
        Message::Image(_) => {
            let duration = SystemTime::now().duration_since(UNIX_EPOCH)?;
            Ok(format!("images/{}", duration.as_millis()))
        }
        _ => Err(RuntimeError::box_from_str(
            "Cannot save this message type as file",
        )),
    }?;
    let content = match message {
        Message::File(_, vec) => Ok(vec),
        Message::Image(vec) => Ok(vec),
        _ => Err(RuntimeError::box_from_str(
            "Cannot save this message type as file",
        )),
    }?;
    let mut file = File::options().write(true).truncate(true).open(path_str)?;
    file.write(content)
        .and(Ok(()))
        .map_err(|err| RuntimeError::box_from_string(err.to_string()))
}

fn get_file_name_from_path(path_str: &str) -> Result<&str, BoxDynError> {
    let path = Path::new(path_str);
    let file_path_error =
        || RuntimeError::box_from_string(format!("Invalid path received: {}", path_str));
    let file_name = path
        .file_name()
        .ok_or_else(file_path_error)
        .and_then(|x| x.to_str().ok_or_else(file_path_error))?;
    Ok(file_name)
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
