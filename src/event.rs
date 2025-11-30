use tokio::sync::{broadcast, oneshot};

pub enum Event {
    // "I want to subscribe to topic 'rust'"
    Subscribe {
        topic: String,
        // The Manager uses this to send the success/failure response back
        resp: oneshot::Sender<broadcast::Receiver<String>>,
    },
    // "Send 'Hello' to everyone on topic 'rust'"
    Publish {
        topic: String,
        message: String,
    },

    ListTopics {
        resp: oneshot::Sender<Vec<String>>
    }
}
