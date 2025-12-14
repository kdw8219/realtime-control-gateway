use crate::protocol::websocket::WebSocketHandler;
use crate::protocol::grpc::GrpcClient;
use std::sync::{Arc};
use tokio::sync::RwLock;
use tokio::net::TcpListener;
use crate::session::manager::{SessionManager, SharedSessions};
use crate::protocol::robot::signaling::SignalMessage;

pub struct GatewayApp {
    grpc: Arc<GrpcClient>,
    sessions: SharedSessions,
}

impl GatewayApp {
    pub async fn new(grpc_endpoint: String) -> anyhow::Result<Self> {
        let (grpc_client, signal_rx) = GrpcClient::connect(grpc_endpoint).await?; //grpc 연결은 하나면 됨
        let grpc = Arc::new(grpc_client);

        let sessions: SharedSessions =
            Arc::new(RwLock::new(SessionManager::new()));

        Self::spawn_signal_receiver(signal_rx, sessions.clone());

        Ok(Self {
            grpc,
            sessions,
        })
    }

    fn spawn_signal_receiver(
        mut signal_rx: impl futures_util::Stream<
            Item = Result<SignalMessage, tonic::Status>
        > + Send + 'static,
        sessions: SharedSessions,
    ) {
        let mut signal_rx = Box::pin(signal_rx);
        
        tokio::spawn(async move {
            use futures_util::StreamExt;

            while let Some(Ok(msg)) = signal_rx.next().await {
                let robot_id = msg.robot_id.clone();

                let guard = sessions.read().await;
                if let Some(ws_tx) = guard.get_ws_sender(&robot_id) {
                    let _ = ws_tx.send(msg);
                }
            }

            // stream 종료 → 여기서 reconnect 로직 넣을 수 있음
            eprintln!("gRPC signaling stream closed");
        });
    }

    pub async fn run(&self, bind_addr: &str) -> anyhow::Result<()> {
        let listener = TcpListener::bind(bind_addr).await?;
        println!("Gateway listening start : {}", bind_addr);

        loop {
            let (stream, _) = listener.accept().await?;

            // 의존성 clone (cheap)
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

    // Additional methods for GatewayApp
}