
use std::alloc::System;

use tokio::sync::{broadcast::{self, Sender}, mpsc};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error};

use crate::{
    topic_tree::TopicTree,
    types::{ManagerEvent, ServerResponse, SystemEvents},
};

pub struct Manager {
    topics: TopicTree,
    event_rx: mpsc::Receiver<ManagerEvent>,
    system_tx: Sender<SystemEvents>
}

impl Manager {
    pub fn new(event_rx: mpsc::Receiver<ManagerEvent>, system_tx: Sender<SystemEvents>) -> Self {
        Self {
            topics: TopicTree::new(),
            event_rx,
            system_tx
        }
    }

    async fn handle_event(&mut self, evnt: ManagerEvent) {
        match evnt {
            ManagerEvent::Subscribe { topic, resp } => {
                let tx = self
                    .topics
                    .get_or_create_channel(&topic);

                resp.send(tx.subscribe()).unwrap(); //TODO: remove unwrap
            }
            ManagerEvent::Publish { message } => {
                let matches = self.topics.find_matches(&message.topic);
                for tx in matches {
                    match tx.send(ServerResponse::NewMessage(message.clone())) {
                        Ok(_) => debug!("send to broadcast channel"),
                        Err(_) => error!("Failed to send message to broadcast"),
                    }
                }
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

        let _ = self.system_tx.send(SystemEvents::Gn);
    }
}
