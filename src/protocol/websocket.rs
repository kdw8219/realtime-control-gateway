use tokio_tungstenite::{accept_async, tungstenite::Message};
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use crate::protocol::grpc::GrpcClient;
use crate::session::manager::SharedSessions;
use tokio::net::TcpStream;
use tokio::sync::mpsc;

use crate::protocol::robot::signaling::SignalMessage;
use crate::domain::signal::WsSignalMessage;

pub struct WebSocketHandler {
    grpc: Arc<GrpcClient>,
    sessions: SharedSessions,
}

impl WebSocketHandler {
    pub fn new(grpc: Arc<GrpcClient>, sessions: SharedSessions) -> Self {
        Self { grpc, sessions }
    }

    pub async fn handle_connection(&self, stream: TcpStream) -> anyhow::Result<()> {
        let ws_stream = accept_async(stream).await?;
        let (ws_sink, mut ws_stream) = ws_stream.split();

        // 첫 메시지로 robot_id 확보
        let robot_id = match ws_stream.next().await {
            Some(Ok(Message::Text(text))) => {
                let v: serde_json::Value = serde_json::from_str(text.as_str())?;
                v["robot_id"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("robot_id missing in first message"))?
                    .to_string()
            }
            _ => anyhow::bail!("robot_id not provided"),
        };

        // 세션별 WS 송신 큐 (gRPC inbound 라우팅 대상)
        let (ws_tx, mut ws_rx) = mpsc::unbounded_channel::<SignalMessage>();

        {
            let mut guard = self.sessions.write().await;
            guard.insert(robot_id.clone(), ws_tx);
        }

        // gRPC signal stream이 아직 없으면 지금(최초) 오픈 + inbound receiver spawn
        self.grpc.ensure_signal_stream(self.sessions.clone()).await?;

        // WS로 내려보내는 task (SignalMessage -> WsSignalMessage -> JSON)
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

        // WS -> gRPC (WsSignalMessage -> SignalMessage -> signal_tx send)
        while let Some(Ok(msg)) = ws_stream.next().await {
            match msg {
                Message::Text(text) => {
                    let ws_msg: WsSignalMessage = serde_json::from_str(text.as_str())?;
                    let signal: SignalMessage = ws_msg.try_into()?;

                    // ensure_signal_stream을 이미 호출했으므로 sender는 존재
                    let _ = self.grpc.signal_sender()?.send(signal);
                }
                Message::Close(_) => break,
                _ => {}
            }
        }

        // 세션 제거
        {
            let mut guard = self.sessions.write().await;
            guard.remove(&robot_id);
        }

        Ok(())
    }
}
