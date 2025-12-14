use std::collections::HashMap;
use tokio::sync::RwLock;
use std::sync::Arc;
use tokio::sync::mpsc;
use crate::protocol::robot::signaling::SignalMessage;
pub type WsSender = mpsc::UnboundedSender<SignalMessage>;

pub struct SessionManager {
    sessions: HashMap<String, WsSender>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    pub fn get_ws_sender(&self, robot_id: &str) -> Option<WsSender> {
        self.sessions.get(robot_id).cloned()
    }

    pub fn insert(&mut self, robot_id: String, tx: WsSender) {
        self.sessions.insert(robot_id, tx);
    }

    pub fn remove(&mut self, robot_id: &str) {
        self.sessions.remove(robot_id);
    }
}


pub type SharedSessions = Arc<RwLock<SessionManager>>;
