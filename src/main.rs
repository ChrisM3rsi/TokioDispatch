pub mod manager;
pub mod message;
pub mod types;

use std::{io::Lines, net::SocketAddr};

use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
    sync::{
        mpsc::{self, Sender},
        oneshot,
    },
};
use tokio_stream::{StreamExt, StreamMap, wrappers::BroadcastStream};

use crate::{
    manager::Manager,
    message::Message,
    types::{ClientEvent, ManagerEvent},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    //TODO: add gracefull handling using cancellation token passing it inside manager and tokio Select the listening loop
    const SERVER_SOCKET: &str = "127.0.0.1:8080";

    let (event_tx, event_rx) = mpsc::channel::<ManagerEvent>(32);

    let manager = Manager::new(event_rx);

    tokio::spawn(async move {
        manager.run().await;
    });

    let listener = TcpListener::bind(SERVER_SOCKET).await?;
    println!("Server running on {}", SERVER_SOCKET); //TODO: add logging, investigate logging for best performance on tasks

    loop {
        let (socket, addr) = listener.accept().await?;
        println!("New client connected: {}", addr);

        let client_tx = event_tx.clone();

        tokio::spawn(async move {
            handle_client(socket, client_tx).await;
        });
    }
}

async fn handle_client(socket: TcpStream, manager_tx: Sender<ManagerEvent>) {
    let (read_half, mut write_half) = socket.into_split();

    let mut reader = BufReader::new(read_half);
    let mut line = String::new();

    let mut subscriptions = StreamMap::<String, BroadcastStream<String>>::new(); //TODO: create model

    loop {
        tokio::select! {
            // BRANCH A: Read from user's keyboard
            result = reader.read_line(&mut line) => {
                match result {
                    Ok(0) => break, // EOF - client disconnected
                    Ok(_) => {
                        let trimmed = line.trim();
                        if !trimmed.is_empty() {
                            handle_event(trimmed, &manager_tx, &mut subscriptions, &mut write_half).await;
                        }
                        line.clear();
                    }
                    Err(msg) =>  {
                        println!("error: {}", msg);
                        break
                    } // Read error
                }
            }

            // BRANCH B: Messages to send to the user
            Some((topic, msg_result)) = subscriptions.next() => {
                 match msg_result  {
                     Ok(msg) =>  {
                        let client_msg = format!("From: {}: {}",topic, msg);

                        if write_half.write_all(client_msg.as_bytes()).await.is_err() {
                            break; // Write error - client gone
                        }
                     },
                     Err(e) => {
                        println!("error: {}", e);
                      }
                 }
            }
        }
    }
}

async fn handle_event(
    input: &str,
    manager_tx: &mpsc::Sender<ManagerEvent>,
    subscriptions: &mut StreamMap<String, BroadcastStream<String>>,
    write_half: &mut tokio::net::tcp::OwnedWriteHalf,
) {
    let client_event = deserialize_input(input);

    match client_event {
        Some(evnt) => match evnt {
            ClientEvent::Subscribe { topic } => {
                if subscriptions.contains_key(&topic) {
                    let msg = format!("Already subscribed to {}\n", topic);
                    let _ = write_half.write_all(msg.as_bytes()).await;
                    return;
                }

                let (resp_tx, resp_rx) = oneshot::channel();

                let evnt = ManagerEvent::Subscribe {
                    topic: topic.clone(),
                    resp: resp_tx,
                };

                if manager_tx.send(evnt).await.is_ok() {
                    if let Ok(broadcast_rx) = resp_rx.await {
                        subscriptions.insert(topic.clone(), BroadcastStream::new(broadcast_rx));

                        let msg = format!("Subscribed to {}\n", topic);
                        let _ = write_half.write_all(msg.as_bytes()).await;
                    }
                }
            }
            ClientEvent::Publish { message } => {
                let cmd = ManagerEvent::Publish { message };
                let _ = manager_tx.send(cmd).await;
            }
            ClientEvent::ListTopics => todo!(),
        },
        None => {
            let msg = "Unknown command. Use SUB <topic> or PUB <topic> <msg>\n";
            let _ = write_half.write_all(msg.as_bytes()).await;
        }
    }

    fn deserialize_input(input: &str) -> Option<ClientEvent> {
        let mut parts = input.splitn(3, ' ');
        let cmd_name = parts.next().unwrap(); //TODO: convert to upper case 

        match cmd_name {
            "SUB" => {
                let topic = parts.next().unwrap_or("default").to_string();
                Some(ClientEvent::Subscribe { topic })
            }
            "PUB" => {
                let topic = parts.next().unwrap_or("default").to_string();
                let text = parts.next().unwrap_or("").to_string();

                Some(ClientEvent::Publish {
                    message: Message { text, topic },
                })
            }
            "LIST" => {
                todo!();
            }
            _ => None,
        }
    }
}
