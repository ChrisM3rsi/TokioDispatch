use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, oneshot};

#[derive(Debug)]
pub enum ManagerEvent {
    Subscribe {
        topic: String,
        resp: oneshot::Sender<broadcast::Receiver<ServerResponse>>,
    },
    Publish {
        message: Message,
    },
}

pub enum ClientEvent {
    Subscribe { topic: String },
    Unsubscribe {topic: String},
    Publish { message: Message },
    ListTopics,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Message {
    pub topic: String,
    pub text: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "type", content = "payload")]
pub enum ServerResponse {
    Ok,
    Error(String), //TODO: use ServerError for code specific errors
    Subscribed(String),
    Unsubscribed(String),
    NewMessage(Message),
    ListTopics(Vec<String>),
    Gn,
}

#[derive(Clone)]
pub enum SystemEvents {
    Gn
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ServerError {
    pub message: String,
    pub code: u32,
}
