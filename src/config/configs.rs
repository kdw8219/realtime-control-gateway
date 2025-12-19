use serde::Deserialize;
use config::{Config, File as ConfigFile};
use std::env;

#[derive(Deserialize, Debug)]
pub struct WebsocketConfig {
    pub self_ip: String,
    pub self_port: String,
}

#[derive(Deserialize, Debug)]
pub struct GRPCConfig {
    pub to_ip: String,
    pub to_port: String,
}

#[derive(Deserialize, Debug)]
pub struct Settings {
    pub websocket_server: WebsocketConfig,
    pub grpc_client: GRPCConfig,
}

pub fn load_settings() -> Settings {
    let mut settings: Settings = Config::builder()
        .add_source(ConfigFile::with_name("config/default").required(true))
        .build()
        .unwrap()
        .try_deserialize()
        .unwrap();

    // 환경변수로 덮어쓰기 (docker env 파일 self_ip/self_port/to_ip/to_port)
    if let Ok(v) = env::var("self_ip") {
        settings.websocket_server.self_ip = v;
    }
    if let Ok(v) = env::var("self_port") {
        settings.websocket_server.self_port = v;
    }
    if let Ok(v) = env::var("to_ip") {
        settings.grpc_client.to_ip = v;
    }
    if let Ok(v) = env::var("to_port") {
        settings.grpc_client.to_port = v;
    }

    settings
}
