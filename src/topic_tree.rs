use crate::types::ServerResponse;
use std::collections::HashMap;
use tokio::sync::broadcast;
use tracing::debug;

#[derive(Debug)]
pub struct TopicTree {
    root: Node,
}

#[derive(Debug)]
struct Node {
    children: HashMap<String, Node>,
    wildcard_child: Option<Box<Node>>,
    channel: Option<broadcast::Sender<ServerResponse>>, //TODO: ServerResponse should be decoupled from TopicTree and Manager structs
}
impl Node {
    fn new() -> Self {
        Self {
            children: HashMap::new(),
            wildcard_child: None,
            channel: None,
        }
    }
}

impl TopicTree {
    pub fn new() -> Self {
        Self { root: Node::new() }
    }
    pub fn get_or_create_channel(&mut self, pattern: &str) -> broadcast::Sender<ServerResponse> {
        let parts: Vec<&str> = pattern
            .split('/')
            .filter(|s| !s.is_empty())
            .collect();

        let mut current = &mut self.root;

        for part in parts {
            if part == "*" {
                if current.wildcard_child.is_none() {
                    current.wildcard_child = Some(Box::new(Node::new()));
                }
                current = current.wildcard_child.as_mut().unwrap();
            } else {
                current = current
                    .children
                    .entry(part.to_string())
                    .or_insert_with(Node::new);
            }
        }

        if current.channel.is_none() {
            let (tx, _) = broadcast::channel(100);
            current.channel = Some(tx);
        }

        current
            .channel
            .as_ref()
            .unwrap()
            .clone()
    }
    pub fn find_matches(&self, topic: &str) -> Vec<broadcast::Sender<ServerResponse>> {
        let parts: Vec<&str> = topic
            .split('/')
            .filter(|s| !s.is_empty())
            .collect();

        let mut matches = Vec::new();
        self.collect_matches(&self.root, &parts, 0, &mut matches);

        matches.dedup_by(|x, y| x.same_channel(y)); // TODO: find a smarter way then using dedup
        matches
    }
    fn collect_matches(
        &self,
        node: &Node,
        parts: &[&str],
        index: usize,
        matches: &mut Vec<broadcast::Sender<ServerResponse>>,
    ) {
        debug!("index: {}", index);
        // If we reached the end of the topic, check if this node has a subscription
        if index == parts.len() {
            debug!("latest match");
            if let Some(tx) = &node.channel {
                debug!("node: {:?}", node);
                matches.push(tx.clone());
            }
            return;
        }

        let part = parts[index];

        // 1. Exact match
        if let Some(child) = node.children.get(part) {
            debug!("exact match");
            self.collect_matches(child, parts, index + 1, matches);
        }

        if let Some(child) = &node.wildcard_child {
            debug!("wildcard match match");
            if let Some(tx) = &child.channel {
                debug!("wildcard node: {:?}", node);

                matches.push(tx.clone());
            }

            self.collect_matches(child, parts, index + 1, matches);
        }

        debug!("no match");
    }
}
