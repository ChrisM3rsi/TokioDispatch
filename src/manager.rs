use std::collections::HashMap;

use tokio::sync::{broadcast, mpsc};

use crate::event::Event;

pub struct Manager {
    topics: HashMap<String, broadcast::Sender<String>>,
    event_rx: mpsc::Receiver<Event>,
}

impl Manager {
    pub fn new(event_rx: mpsc::Receiver<Event>) -> Self {
        Self {
            topics: HashMap::new(),
            event_rx,
        }
    }

    pub async fn run(mut self) {
        while let Some(evnt) = self.event_rx.recv().await {
            match evnt {
                Event::Subscribe { topic, resp } => {
                    let tx = self
                        .topics
                        .entry(topic)
                        .or_insert_with(|| broadcast::channel(100).0);

                    resp.send(tx.subscribe()).unwrap(); //TODO: remove unwrap
                }
                Event::Publish { topic, message } => {
                    if let Some(tx) = self.topics.get(&topic) {
                        tx.send(message).unwrap(); //TODO: remove unwrap
                    }
                }
                Event::ListTopics { resp } => {
                    let topics = self.topics.keys().cloned().collect();
                    resp.send(topics).unwrap(); //TODO: remove unwrap
                }
            }
        }
    }
}
