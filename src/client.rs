use std::net::SocketAddr;

use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpStream,
    sync::{
        broadcast::Receiver,
        mpsc::{self, Sender},
        oneshot,
    },
};
use tokio_stream::{StreamExt, StreamMap, wrappers::BroadcastStream};
use tracing::{error, info};

use crate::types::{ClientEvent, ManagerEvent, Message, ServerResponse, SystemEvents};

type ClientSubscriptions = StreamMap<String, BroadcastStream<ServerResponse>>;

pub struct Client {
    subscriptions: ClientSubscriptions,
    manager_tx: Sender<ManagerEvent>,
    system_rx: Receiver<SystemEvents>,
    
}

impl Client {
    pub fn new(
        subscriptions: ClientSubscriptions,
        manager_tx: Sender<ManagerEvent>,
        system_rx: Receiver<SystemEvents>,
    ) -> Self {
        Self {
            subscriptions,
            manager_tx,
            system_rx,
        }
    }

    pub async fn run(mut self, socket: TcpStream ) {
        let (read_half, mut write_half) = socket.into_split();
        let mut reader = BufReader::new(read_half);
        let mut line = String::new();

        loop {
            // client input
            tokio::select! {
                result = reader.read_line(&mut line) => {
                    match result {
                        Ok(0) => break,
                        Ok(_) => {
                            let trimmed = line.trim();
                            if !trimmed.is_empty() {
                                let response = self.handle_event(trimmed).await;
                                let json = serde_json::to_string(&response).unwrap();

                                if write_half.write_all(json.as_bytes()).await.is_err() {
                                    break;
                                }
                            }
                            line.clear();
                        }
                        Err(msg) =>  {
                            error!("error: {}", msg);
                            break
                        }
                    }
                }

                // client response
                Some((_, msg_result)) = self.subscriptions.next() => {
                     match msg_result  {
                        //TODO:  Create a second model for actual response
                         Ok(msg) =>  {
                            let json = serde_json::to_string(&msg).unwrap();
                            if write_half.write_all(json.as_bytes()).await.is_err() {
                                break; // Write error - client gone
                            }

                         },
                         Err(e) => {
                            let response = ServerResponse::Error("Unexpected error".to_string());
                            let json_response = serde_json::to_string(&response).unwrap();
                            if write_half.write_all(json_response.as_bytes()).await.is_err() {
                                break; // Write error - client gone
                            }
                            error!("error: {}", e);
                          }
                     }
                }

                Ok(msg) = self.system_rx.recv() => {
                    match msg {
                        SystemEvents::Gn => {
                            let json = serde_json::to_string(&ServerResponse::Gn).unwrap();
                            let _ = write_half.write_all(json.as_bytes()).await;
                            break;
                        }
                    }
                }
            }
        }
    }

    async fn handle_event(&mut self, input: &str) -> ServerResponse {
        let client_event = deserialize_input(input);

        match client_event {
            Some(evnt) => match evnt {
                ClientEvent::Subscribe { topic } => {
                    info!("topic: {}", topic);
                    if self.subscriptions.contains_key(&topic) {
                        return ServerResponse::Error(format!("Already subscribed to {}\n", topic));
                    }

                    let (resp_tx, resp_rx) = oneshot::channel();

                    let evnt = ManagerEvent::Subscribe {
                        topic: topic.clone(),
                        resp: resp_tx,
                    };

                    match self.manager_tx.send(evnt).await {
                        Ok(_) => {
                            if let Ok(broadcast_rx) = resp_rx.await {
                                self.subscriptions
                                    .insert(topic.clone(), BroadcastStream::new(broadcast_rx));
                                return ServerResponse::Subscribed(topic);
                            } else {
                                return ServerResponse::Ok;
                            }
                        }
                        Err(e) => ServerResponse::Error(e.to_string()),
                    }
                }
                ClientEvent::Publish { message } => {
                    let cmd = ManagerEvent::Publish { message };
                    let _ = self.manager_tx.send(cmd).await;
                    return ServerResponse::Ok;
                }
                ClientEvent::ListTopics => {
                    let messages: Vec<String> = self
                        .subscriptions
                        .keys()
                        .map(|x| x.clone())
                        .collect();

                    ServerResponse::ListTopics(messages)
                }
                ClientEvent::Unsubscribe { topic } => {
                    self.subscriptions.remove(&topic);
                    ServerResponse::Ok
                }
            },
            None => ServerResponse::Error(
                "Unknown command. Use SUB <topic> or PUB <topic> <msg>\n".to_string(),
            ),
        }
    }
}

fn deserialize_input(input: &str) -> Option<ClientEvent> {
    let mut parts = input.splitn(3, ' ');
    let cmd_name = parts.next().unwrap(); //TODO: convert to upper case 

    match cmd_name {
        "SUB" => {
            let topic = parts
                .next()
                .unwrap_or("default")
                .to_string();

            Some(ClientEvent::Subscribe { topic })
        }
        "PUB" => {
            let topic = parts
                .next()
                .unwrap_or("default")
                .to_string();
            let text = parts.next().unwrap_or("").to_string();

            Some(ClientEvent::Publish {
                message: Message { text, topic },
            })
        }
        "LIST" => Some(ClientEvent::ListTopics),

        "UNSUB" => {
            let topic = parts
                .next()
                .unwrap_or("default")
                .to_string();

            Some(ClientEvent::Unsubscribe { topic })
        }
        _ => None,
    }
}
