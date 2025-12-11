use tokio::net::TcpListener;

use crate::protocol::websocket;
pub mod protocol;
mod config;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let settings = config::configs::load_settings();
    let websocket_config = &settings.websocket_server;
    let grpc_config = Arc::new(settings.grpc_client);

    let addr = format!("{}:{}"
        , websocket_config.self_ip
        , websocket_config.self_port
    );

    let listener = TcpListener::bind(&addr).await?;
    println!("WebSocket server started on ws://{}", addr);

    while let Ok((stream, _)) = listener.accept().await {
        let grpc_config = Arc::clone(&grpc_config);

        tokio::spawn(async move {
            let ws = protocol::websocket::WebSocketHandler::new(grpc_config.to_ip.clone(), grpc_config.to_port.clone()).await;
            ws.handle_connection(stream).await
        });
    }

    Ok(())
}
