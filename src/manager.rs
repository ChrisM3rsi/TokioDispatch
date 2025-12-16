use std::sync::Arc;

use dashmap::DashMap;
use tokio::sync::{broadcast, mpsc};
use tokio_util::{sync::CancellationToken, task::TaskTracker};
use tracing::{debug, error, info};

use crate::types::{ManagerEvent, ServerResponse};

pub struct Manager {
    topics: Arc<DashMap<String, broadcast::Sender<ServerResponse>>>,
    event_rx: mpsc::Receiver<ManagerEvent>,
}

impl Manager {
    pub fn new(event_rx: mpsc::Receiver<ManagerEvent>) -> Self {
        Self {
            topics: Arc::new(DashMap::new()),
            event_rx,
        }
    }

    async fn handle_event(&mut self, evnt: ManagerEvent) {
        match evnt {
            ManagerEvent::Subscribe { topic, resp } => {
                let tx = self
                    .topics
                    .entry(topic)
                    .or_insert_with(|| broadcast::channel(100).0);

                resp.send(tx.subscribe()).unwrap(); //TODO: remove unwrap
            }
            ManagerEvent::Publish { message } => {
                if let Some(tx) = self.topics.get(&message.topic) {
                    tx.send(ServerResponse::NewMessage(message))
                        .unwrap(); //TODO: remove unwrap
                }
            }
            ManagerEvent::ListTopics { resp } => {
                let topics: Vec<String> = self
                    .topics
                    .iter()
                    .map(|entry| entry.key().clone())
                    .collect();

                resp.send(topics).unwrap(); //TODO: remove unwrap
            }
        }
    }

    pub async fn run(mut self, ct: CancellationToken) {
        let tracker = TaskTracker::new();

        loop {
            tokio::select! {
                Some(evnt) = self.event_rx.recv() => {
                    debug!("handling event: {:?}", evnt);
                    let topics = self.topics.clone();
                    tracker.spawn(handle_event_parallel(topics, evnt));
                }
                _ = ct.cancelled() => {
                    break;
                }
            }
        }
        
        self.event_rx.close();
        tracker.close();
        tracker.wait().await;

        while let Some(evnt) = self.event_rx.recv().await {
            info!("handling remaining events");
            self.handle_event(evnt).await; //TODO: find a way to parallelize this as well
        }

        for entry in self.topics.iter() {
            let _ = entry.value().send(ServerResponse::Gn);
        }
    }
}

async fn handle_event_parallel(
    topics: Arc<DashMap<String, broadcast::Sender<ServerResponse>>>,
    evnt: ManagerEvent,
) {
    match evnt {
        ManagerEvent::Publish { message } => {
            let tx_opt = topics
                .get(&message.topic)
                .map(|channel| channel.clone());

            match tx_opt {
                Some(res) => {
                    res.send(ServerResponse::NewMessage(message))
                        .unwrap();
                }
                None => {
                    error!("Topic not found")
                }
            }
        }

        ManagerEvent::Subscribe { topic, resp } => {
            let tx_opt = topics
                .get(&topic)
                .map(|guard| guard.clone());

            match tx_opt {
                Some(tx) => {
                    let _ = resp.send(tx.subscribe());
                }
                None => {
                    let tx = topics
                        .entry(topic)
                        .or_insert_with(|| broadcast::channel(100).0);

                    let _ = resp.send(tx.subscribe());
                }
            }
        }

        ManagerEvent::ListTopics { resp } => {
            let topics: Vec<String> = topics
                .iter()
                .map(|entry| entry.key().clone())
                .collect();

            resp.send(topics).unwrap(); //TODO: remove unwrap
        }
    }
}
