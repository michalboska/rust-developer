use std::cell::RefCell;
use std::collections::HashMap;
use std::net::{SocketAddr, TcpListener};
use std::rc::Rc;
use std::time::Duration;

use log::info;
use serde::de::DeserializeOwned;
use serde::Serialize;

use ex11_shared::err::BoxDynError;
use ex11_shared::message_tcp_stream::MessageTcpStream;

static DEFAULT_TIMEOUT: Duration = Duration::from_millis(500);

type ClientType<T> = Rc<RefCell<MessageTcpStream<T>>>;
type ClientsMap<T> = HashMap<SocketAddr, ClientType<T>>;

pub struct Server<T: Serialize + DeserializeOwned> {
    listener: TcpListener,
    clients: ClientsMap<T>,
}

impl<T: Serialize + DeserializeOwned> Server<T> {
    pub fn new(socket_addr: &SocketAddr) -> Result<Server<T>, BoxDynError> {
        info!("Listening on {}", socket_addr);
        Ok(Server {
            listener: TcpListener::bind(socket_addr)?,
            clients: HashMap::new(),
        })
    }

    pub fn listen_blocking(&mut self) -> Result<(), BoxDynError> {
        for stream_result in self.listener.incoming() {
            let stream = stream_result?;
            let addr = stream.peer_addr()?;
            let message_stream = MessageTcpStream::from_tcp_stream(stream, DEFAULT_TIMEOUT)?;

            let rc = Rc::new(RefCell::new(message_stream));

            self.clients.insert(addr, Rc::clone(&rc));
            Server::handle_client(&mut self.clients, Rc::clone(&rc), &addr);
        }
        Ok(())
    }

    fn handle_client(clients: &mut ClientsMap<T>, stream: ClientType<T>, socket_addr: &SocketAddr) {
        loop {
            let message_option = match stream.borrow_mut().read_next_message() {
                Err(_) => {
                    break;
                }
                Ok(Some(msg)) => Some(msg),
                _ => None,
            };

            if let Some(message) = message_option {
                Server::process_client_message(clients, socket_addr, &message);
            }
        }
    }

    fn process_client_message(
        clients: &mut ClientsMap<T>,
        client_socket_addr: &SocketAddr,
        message: &T,
    ) {
        let mut clients_to_remove = Vec::<SocketAddr>::new();
        clients
            .iter_mut()
            .filter(|(&it_socket_addr, _)| it_socket_addr == *client_socket_addr)
            .for_each(|(sock_addr, message_stream)| {
                let result = message_stream.borrow_mut().send_message_to_server(message);
                if let Err(_) = result {
                    clients_to_remove.push(sock_addr.clone());
                }
            });

        for socket_addr in clients_to_remove {
            clients.remove(&socket_addr);
        }
    }
}
