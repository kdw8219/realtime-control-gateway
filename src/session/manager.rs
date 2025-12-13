use std::collections::HashMap;
use tokio::sync::RwLock;
use std::sync::Arc;

pub struct Session {
    pub robot_id: String,
    // ws_sender, grpc_task 등은 나중에 추가
}

pub struct SessionManager {
    sessions: HashMap<String, Session>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    pub fn insert(&mut self, robot_id: String, session: Session) {
        self.sessions.insert(robot_id, session);
    }

    pub fn remove(&mut self, robot_id: &str) {
        self.sessions.remove(robot_id);
    }
}

pub type SharedSessions = Arc<RwLock<SessionManager>>;
