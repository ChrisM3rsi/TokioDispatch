use std::collections::HashMap;

use tokio::sync::{broadcast, mpsc};
use tokio_util::sync::CancellationToken;
use tracing::debug;

use crate::types::{ManagerEvent, ServerResponse};

pub struct Manager {
    topics: HashMap<String, broadcast::Sender<ServerResponse>>,
    event_rx: mpsc::Receiver<ManagerEvent>,
}

impl Manager {
    pub fn new(event_rx: mpsc::Receiver<ManagerEvent>) -> Self {
        Self {
            topics: HashMap::new(),
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
                    tx.send(ServerResponse::NewMessage(message)).unwrap(); //TODO: remove unwrap
                }
            }
            ManagerEvent::ListTopics { resp } => {
                let topics = self.topics.keys().cloned().collect();
                resp.send(topics).unwrap(); //TODO: remove unwrap
            }
        }
    }

    pub async fn run(mut self, ct: CancellationToken) {
        loop {
            tokio::select! {
                Some(evnt) = self.event_rx.recv() =>  { //TODO: should this be parallel? Each client communicates with manager via a single channel, is this considered bottleneck?
                    debug!("handling event: {:?}", evnt);
                    self.handle_event(evnt).await;
            }

                _ = ct.cancelled() => {
                    break;
                }

            }
        }
        self.event_rx.close();

        while let Some(evnt) = self.event_rx.recv().await {
           self.handle_event(evnt).await;
        }

        for (_, broadcast) in self.topics {
            let _ = broadcast.send(ServerResponse::Gn);
        }
    }
}
