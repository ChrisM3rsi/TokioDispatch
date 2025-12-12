use tokio::sync::{broadcast, oneshot};


#[derive(Debug)]
pub enum ManagerEvent {
    // "I want to subscribe to topic 'rust'"
    Subscribe {
        topic: String,
        // The Manager uses this to send the success/failure response back
        resp: oneshot::Sender<broadcast::Receiver<String>>,
    },
    // "Send 'Hello' to everyone on topic 'rust'"
    Publish {
        message: Message,
    },

    ListTopics {
        resp: oneshot::Sender<Vec<String>>,
    },
}

pub enum ClientEvent {
    Subscribe { topic: String },
    Publish { message: Message },
    ListTopics,
}


#[derive(Debug)]
pub struct Message {
    pub topic: String,
    pub text: String
}
