pub mod client;
pub mod manager;
pub mod topic_tree;
pub mod types;

use tokio::{
    net::TcpListener,
    signal,
    sync::{
        broadcast::{self},
        mpsc::{self},
    },
};
use tokio_stream::{StreamMap, wrappers::BroadcastStream};
use tokio_util::{sync::CancellationToken, task::TaskTracker};
use tracing::{Instrument, debug_span, error, info};

use crate::{
    client::Client,
    manager::Manager,
    types::{ManagerEvent, ServerResponse, SystemEvents},
};
const SERVER_SOCKET: &str = "127.0.0.1:8080";
const TRACING_LEVEL: tracing::Level = tracing::Level::DEBUG;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    //TODO: add timestamp to payloads
    tracing_subscriber::fmt()
        .with_max_level(TRACING_LEVEL)
        .init();

    let ct = CancellationToken::new();

    tokio::spawn(cancellation_listener(ct.clone()));

    let (event_tx, event_rx) = mpsc::channel::<ManagerEvent>(32);
    let (system_tx, _) = broadcast::channel::<SystemEvents>(5);

    let manager = Manager::new(event_rx, system_tx.clone());

    tokio::spawn(
        manager
            .run(ct.clone())
            .instrument(debug_span!("Manager")),
    );

    let listener = TcpListener::bind(SERVER_SOCKET).await?;
    info!("Server running on {}", SERVER_SOCKET);

    let tracker = TaskTracker::new();

    loop {
        tokio::select! {
            res = listener.accept() => {
                match res {
                    Ok((socket, addr)) => {
                        info!("New client connected: {}", addr);

                        let client = Client::new(
                            StreamMap::<String, BroadcastStream<ServerResponse>>::new(),
                            event_tx.clone(),
                            system_tx.subscribe());

                        tracker.spawn(client.run(socket));
                    }

                    Err(e) => { error!("Failed to accept connection: {}", e); }
                }
            }
             _ = ct.cancelled() => {
                info!("Shutting down accept loop");
                break;
            }

        };
    }
    tracker.close();
    tracker.wait().await;
    info!("Handled all pending messages");
    Ok(())
}

async fn cancellation_listener(ct: CancellationToken) {
    match signal::ctrl_c().await {
        Ok(()) => {
            ct.cancel();
        }
        Err(err) => {
            error!("Unable to listen for shutdown signal: {}", err);
            ct.cancel();
        }
    }
}
