use crate::protocol::websocket::WebSocketHandler;
use crate::protocol::grpc::GrpcClient;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::net::TcpListener;
use crate::session::manager::{SessionManager, SharedSessions};

pub struct GatewayApp {
    grpc: Arc<GrpcClient>,
    sessions: SharedSessions,
}

impl GatewayApp {
    pub async fn new(grpc_endpoint: String) -> anyhow::Result<Self> {
        let grpc_client = GrpcClient::connect(grpc_endpoint).await?;
        let grpc = Arc::new(grpc_client);

        let sessions: SharedSessions = Arc::new(RwLock::new(SessionManager::new()));

        Ok(Self { grpc, sessions })
    }

    pub async fn run(&self, bind_addr: &str) -> anyhow::Result<()> {
        let listener = TcpListener::bind(bind_addr).await?;
        println!("Gateway listening start : {}", bind_addr);

        loop {
            let (stream, _) = listener.accept().await?;

            let grpc = self.grpc.clone();
            let sessions = self.sessions.clone();

            tokio::spawn(async move {
                let handler = WebSocketHandler::new(grpc, sessions);

                if let Err(e) = handler.handle_connection(stream).await {
                    eprintln!("WebSocket error: {:?}", e);
                }
            });
        }
    }
}
