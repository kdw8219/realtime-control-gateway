use tokio::net::TcpListener;

use crate::protocol::websocket;
mod protocol;
mod config;


#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let settings = config::configs::load_settings();
    let websocket_config = &settings.websocket_server;
    let grpc_config = &settings.grpc_client;

    let addr = format!("{}:{}"
        , websocket_config.self_ip
        , websocket_config.self_port
    );

    let listener = TcpListener::bind(&addr).await?;
    println!("WebSocket server started on ws://{}", addr);

    while let Ok((stream, _)) = listener.accept().await {
        tokio::spawn(async move {
            protocol::websocket::WebSocketHandler::handle_connection(stream).await
        });
    }

    Ok(())
}
