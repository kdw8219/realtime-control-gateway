use serde::Deserialize;
use config::{Config, File as ConfigFile};

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
    Config::builder().add_source(ConfigFile::with_name("config/default").required(true))
    .build()
    .unwrap()
    .try_deserialize()
    .unwrap()
}
