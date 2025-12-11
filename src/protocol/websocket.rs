use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::protocol::Message;
use anyhow::Result;
use futures_util::{SinkExt, StreamExt};

pub struct WebSocketHandler {
    grpc_handler: crate::protocol::grpc::GrpcClientHandler,
}

impl WebSocketHandler {
    pub async fn new(to_ip:String, to_port:String) -> Self {
        let mut grpc_handler = crate::protocol::grpc::GrpcClientHandler {
            to_ip,
            to_port,
        };

        grpc_handler.connect().await;

        Self {
            grpc_handler,
        }
    }   
    pub async fn handle_connection(&self, stream: tokio::net::TcpStream) -> Result<()> {
        // Implementation goes here
        Ok(())
    }
}