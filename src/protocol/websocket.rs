use tokio_tungstenite::{accept_async, tungstenite::Message};
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use crate::protocol::grpc::GrpcClient;
use crate::session::manager::{ SharedSessions};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use crate::protocol::robot::signaling::SignalMessage;
use crate::domain::signal::WsSignalMessage;

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

        let ws_stream = accept_async(stream).await?;
        let (mut ws_sink, mut ws_stream) = ws_stream.split();

        //get robot id from json
        let robot_id = match ws_stream.next().await {
            Some(Ok(Message::Text(text))) => {
                let v: serde_json::Value = serde_json::from_str(&text)?;
                v["robot_id"].as_str().unwrap().to_string()
            }
            _ => anyhow::bail!("robot_id not provided"),
        };

        let (ws_tx, mut ws_rx) =
            mpsc::unbounded_channel::<SignalMessage>();

        {
            let mut guard = self.sessions.write().await;
            guard.insert(robot_id.clone(), ws_tx);
        }

        //let mut ws_sink_clone = ws_sink.clone();
        tokio::spawn(async move {
            let mut ws_sink = ws_sink;

            while let Some(msg) = ws_rx.recv().await {
                let ws_msg = match WsSignalMessage::try_from(msg) {
                    Ok(v) => v,
                    Err(e) => {
                        eprintln!("convert error: {e}");
                        break;
                    }
                };

                let json = match serde_json::to_string(&ws_msg) {
                    Ok(v) => v,
                    Err(e) => {
                        eprintln!("serde error: {e}");
                        break;
                    }
                };

                if ws_sink.send(Message::Text(json.into())).await.is_err() {
                    break;
                }
            }
        });

        // 6️⃣ WS → gRPC 처리 루프
        while let Some(Ok(msg)) = ws_stream.next().await {
            match msg {
                Message::Text(text) => {
                    let s = text.as_str();
                    let ws_msg: WsSignalMessage = serde_json::from_str(s)?;
                    let signal: SignalMessage = ws_msg.try_into()?;
                    let _ = self.grpc.signal_tx.send(signal);
                }

                Message::Close(_) => break,
                _ => {}
            }
        }

        // 7️⃣ 연결 종료 → 세션 정리
        {
            let mut guard = self.sessions.write().await;
            guard.remove(&robot_id);
        }

        Ok(())
    }
}