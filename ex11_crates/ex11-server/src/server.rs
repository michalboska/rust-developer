use std::collections::HashMap;
use std::net::{SocketAddr, TcpListener};
use std::sync::{Arc, Mutex};
use std::thread;

use log::{debug, info};
use serde::de::DeserializeOwned;
use serde::Serialize;

use ex11_shared::err::BoxDynError;
use ex11_shared::message_tcp_stream::MessageTcpStream;

type ClientType<T> = Arc<Mutex<MessageTcpStream<T>>>;
type ClientsMap<T> = HashMap<SocketAddr, RwClient<T>>;
type ClientsMapSync<T> = Arc<Mutex<ClientsMap<T>>>;

pub struct Server<T: Serialize + DeserializeOwned + Send + 'static> {
    listener: TcpListener,
    clients: ClientsMapSync<T>,
}

struct RwClient<T: Send + 'static> {
    reader: ClientType<T>,
    writer: ClientType<T>,
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
            let read_stream = stream_result?;
            let write_stream = read_stream.try_clone()?;
            let addr = read_stream.peer_addr()?;
            let reader = Arc::new(Mutex::new(MessageTcpStream::from_tcp_stream(
                read_stream,
                None,
            )?));
            let writer = Arc::new(Mutex::new(MessageTcpStream::from_tcp_stream(
                write_stream,
                None,
            )?));
            {
                let mut clients_guard = clients.lock().unwrap();
                clients_guard.insert(
                    addr,
                    RwClient {
                        reader: Arc::clone(&reader),
                        writer: Arc::clone(&writer),
                    },
                );
                info!("New client {} connected.", addr);
            }
            let clients_send_arc = Arc::clone(&clients);
            thread::spawn(move || {
                Server::handle_client(clients_send_arc, Arc::clone(&reader), addr.clone());
            });
        }
        Ok(())
    }

    fn handle_client(clients: ClientsMapSync<T>, stream: ClientType<T>, socket_addr: SocketAddr) {
        loop {
            let read_result = {
                let mut stream_guard = stream.lock().unwrap();
                stream_guard.read_next_message()
            };
            let message_option = match read_result {
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
        let mut other_clients_to_send_message = {
            let mut clients_guard = clients.lock().unwrap();
            clients_guard
                .iter_mut()
                .filter(|(&socket_addr, _)| socket_addr != *client_socket_addr)
                .map(|(&socket_addr, client)| (socket_addr.clone(), Arc::clone(&client.writer)))
                .collect::<Vec<(SocketAddr, ClientType<T>)>>()
        };
        other_clients_to_send_message
            .iter_mut()
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
