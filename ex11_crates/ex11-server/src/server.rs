use std::collections::HashMap;
use std::net::{SocketAddr, TcpListener};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use log::{debug, info};
use serde::de::DeserializeOwned;
use serde::Serialize;

use ex11_shared::err::BoxDynError;
use ex11_shared::message_tcp_stream::MessageTcpStream;

static DEFAULT_TIMEOUT: Duration = Duration::from_millis(500);

type ClientType<T> = Arc<Mutex<MessageTcpStream<T>>>;
type ClientsMap<T> = HashMap<SocketAddr, ClientType<T>>;
type ClientsMapSync<T> = Arc<Mutex<ClientsMap<T>>>;

pub struct Server<T: Serialize + DeserializeOwned + Send + 'static> {
    listener: TcpListener,
    clients: ClientsMapSync<T>,
}

impl<T: Serialize + DeserializeOwned + Send + 'static> Server<T> {
    pub fn new(socket_addr: &SocketAddr) -> Result<Server<T>, BoxDynError> {
        info!("Listening on {}", socket_addr);
        Ok(Server {
            listener: TcpListener::bind(socket_addr)?,
            clients: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub fn listen_blocking(&mut self) -> Result<(), BoxDynError> {
        let clients = Arc::clone(&self.clients);
        for stream_result in self.listener.incoming() {
            let stream = stream_result?;
            let addr = stream.peer_addr()?;
            info!("New client {} connected.", addr);
            let message_stream = MessageTcpStream::from_tcp_stream(stream, None)?;
            let stream_arc = Arc::new(Mutex::new(message_stream));
            let clients_send_arc = Arc::clone(&clients);
            clients
                .lock()
                .unwrap()
                .insert(addr, Arc::clone(&stream_arc));
            thread::spawn(move || {
                Server::handle_client(clients_send_arc, stream_arc, addr.clone());
            });
        }
        Ok(())
    }

    fn handle_client(clients: ClientsMapSync<T>, stream: ClientType<T>, socket_addr: SocketAddr) {
        loop {
            let message_option = match stream.lock().unwrap().read_next_message() {
                Err(err) => {
                    debug!("Err from client {}: {}", &socket_addr, err);
                    Server::remove_dead_clients(&clients, &vec![socket_addr]);
                    break;
                }
                Ok(Some(msg)) => Some(msg),
                _ => None,
            };

            if let Some(message) = message_option {
                Server::process_client_message(&clients, &socket_addr, &message);
            }
        }
    }

    fn process_client_message(
        clients: &ClientsMapSync<T>,
        client_socket_addr: &SocketAddr,
        message: &T,
    ) {
        info!("Message from client {}", client_socket_addr);
        let mut clients_to_remove = Vec::<SocketAddr>::new();
        clients
            .lock()
            .unwrap()
            .iter_mut()
            .filter(|(&it_socket_addr, _)| it_socket_addr == *client_socket_addr)
            .for_each(|(sock_addr, message_stream)| {
                let result = message_stream.lock().unwrap().send_message(message);
                if let Err(_) = result {
                    clients_to_remove.push(sock_addr.clone());
                }
            });

        Server::remove_dead_clients(&clients, &clients_to_remove);
    }

    fn remove_dead_clients(clients: &ClientsMapSync<T>, dead_socket_addrs: &Vec<SocketAddr>) {
        if !dead_socket_addrs.is_empty() {
            let mut guard = clients.lock().unwrap();

            for socket_addr in dead_socket_addrs {
                info!("Client {} dropped", &socket_addr);
                guard.remove(socket_addr);
            }
        }
    }
}
