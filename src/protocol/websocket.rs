use tokio_tungstenite::{accept_async, tungstenite::accept};
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};
use std::sync::{Arc, Mutex};
use crate::protocol::grpc::GrpcClient;
use crate::session::manager::{SessionManager, SharedSessions};
use tokio::net::TcpStream;

pub struct WebSocketHandler {
    grpc: Arc<crate::protocol::grpc::GrpcClient>,
    sessions: SharedSessions,
}

impl WebSocketHandler {
    pub fn new(
        grpc: Arc<GrpcClient>,
        sessions: SharedSessions,
    ) -> Self {
        Self { grpc, sessions }
    }

    pub async fn handle_connection(
        &self,
        stream: TcpStream,
    ) -> anyhow::Result<()> {
        // 여기서부터:
        // - WebSocket handshake
        // - robot_id 추출
        // - WS read/write task
        // - gRPC subscribe_signal
        // - SessionManager insert/remove
        Ok(())
    }
}