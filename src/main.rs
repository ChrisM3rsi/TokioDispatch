pub mod event;
pub mod manager;

use std::{io::Lines, net::SocketAddr};

use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
    sync::{
        mpsc::{self, Sender},
        oneshot,
    },
};

use crate::{event::Event, manager::Manager};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (event_tx, event_rx) = mpsc::channel::<Event>(32);

    let manager = Manager::new(event_rx);

    tokio::spawn(async move {
        manager.run().await;
    });

    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    println!("Server running on 127.0.0.1:8080");

    loop {
        let (socket, addr) = listener.accept().await?;
        println!("New client connected: {}", addr);

        let client_tx = event_tx.clone();

        tokio::spawn(async move {
            handle_client(socket, client_tx, addr).await;
        });
    }
}

async fn handle_client(socket: TcpStream, manager_tx: Sender<Event>, addr: SocketAddr) {
    let (read_half, mut write_half) = socket.into_split();

    let (client_tx, mut client_rx) = mpsc::channel::<String>(100);

    let mut reader = BufReader::new(read_half);
    let mut line = String::new();

    loop {
        tokio::select! {
            // BRANCH A: Read from user's keyboard
            result = reader.read_line(&mut line) => {
                match result {
                    Ok(0) => break, // EOF - client disconnected
                    Ok(_) => {
                        let trimmed = line.trim();
                        if !trimmed.is_empty() {
                            handle_command(trimmed, &manager_tx, &client_tx).await;
                        }
                        line.clear(); // IMPORTANT: Clear for next iteration
                    }
                    Err(msg) =>  {
                        println!("error: {}", msg);
                        break
                    } // Read error
                }
            }

            // BRANCH B: Messages to send to the user
            Some(msg) = client_rx.recv() => {
                if write_half.write_all(msg.as_bytes()).await.is_err() {
                    break; // Write error - client gone
                }
            }
        }
    }
}

async fn handle_command(
    input: &str,
    manager_tx: &mpsc::Sender<Event>,
    client_tx: &mpsc::Sender<String>,
) {
    let mut parts = input.splitn(3, ' ');
    let cmd_name = parts.next().unwrap_or("");

    match cmd_name {
        "SUB" => {
            let topic = parts.next().unwrap_or("default").to_string();
            let topic_clone = topic.clone();

            let (resp_tx, resp_rx) = oneshot::channel();
            let cmd = Event::Subscribe {
                topic: topic.clone(),
                resp: resp_tx,
            };

            if manager_tx.send(cmd).await.is_ok() {
                if let Ok(mut broadcast_rx) = resp_rx.await {
                    // Bridge task: broadcast -> client_tx
                    let my_client_tx = client_tx.clone();
                    tokio::spawn(async move {
                        while let Ok(msg) = broadcast_rx.recv().await {
                            let formatted = format!("From {}: {}\n", topic_clone, msg);
                            if my_client_tx.send(formatted).await.is_err() {
                                break;
                            }
                        }
                    });
                    let _ = client_tx.send(format!("Subscribed to {}\n", topic)).await;
                }
            }
        }
        "PUB" => {
            let topic = parts.next().unwrap_or("default").to_string();
            let message = parts.next().unwrap_or("").to_string();

            let cmd = Event::Publish { topic, message };
            let _ = manager_tx.send(cmd).await;
        }
        _ => {
            let _ = client_tx
                .send("Unknown command. Use SUB <topic> or PUB <topic> <msg>\n".to_string())
                .await;
        }
    }
}
