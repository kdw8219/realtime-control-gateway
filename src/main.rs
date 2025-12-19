mod protocol;
mod config;
mod app;
mod session;
mod domain;

#[tokio::main]
async fn main() -> anyhow::Result<()> {

    // Default to info-level logs so we always see connection traces even if RUST_LOG is unset.
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    log::info!("server starting");

    let settings = config::configs::load_settings();
    let grpc_endpoint = format!("http://{}:{}", settings.grpc_client.to_ip, settings.grpc_client.to_port);
    let ws_bind_addr = format!("{}:{}", settings.websocket_server.self_ip, settings.websocket_server.self_port);
    
    let app = app::gateway_app::GatewayApp::new(grpc_endpoint).await?;
    app.run(ws_bind_addr.as_str()).await?;

    Ok(())

}
