use std::sync::Arc;

use anyhow::anyhow;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::{
    accept_hdr_async,
    tungstenite::handshake::server::{Request, Response},
    tungstenite::Message,
    WebSocketStream,
};

use crate::domain::signal::{CommandType, ControlPayload, WsSignalMessage};
use crate::protocol::grpc::GrpcClient;
use crate::protocol::robot::signaling::SignalMessage;
use crate::session::manager::SharedSessions;

type WsSink = futures_util::stream::SplitSink<WebSocketStream<TcpStream>, Message>;

pub struct WebSocketHandler {
    grpc: Arc<GrpcClient>,
    sessions: SharedSessions,
}

impl WebSocketHandler {
    pub fn new(grpc: Arc<GrpcClient>, sessions: SharedSessions) -> Self {
        Self { grpc, sessions }
    }

    pub async fn handle_connection(&self, stream: TcpStream) -> anyhow::Result<()> {
        let peer = stream
            .peer_addr()
            .map(|addr| addr.to_string())
            .unwrap_or_else(|_| "unknown_peer".to_string());

        let (path_tx, path_rx) = oneshot::channel();

        // Capture the HTTP path during the WebSocket handshake so we can route control/screen channels.
        let ws_stream = accept_hdr_async(stream, move |req: &Request, resp: Response| {
            let _ = path_tx.send(req.uri().path().to_string());
            Ok(resp)
        })
        .await?;

        let path = path_rx.await.unwrap_or_else(|_| "/".to_string());
        println!("[ws] handshake peer={peer} path={path}");

        if let Some(robot_id) = extract_robot_id(&path, "/ws/screen/") {
            println!("[ws] route=screen robot_id={robot_id}");
            self.handle_screen_channel(robot_id, ws_stream).await
        } else if let Some(robot_id) = extract_robot_id(&path, "/ws/control/") {
            println!("[ws] route=control robot_id={robot_id}");
            self.handle_control_channel(robot_id, ws_stream).await
        } else {
            anyhow::bail!("unsupported websocket path: {path}");
        }
    }

    async fn handle_screen_channel(
        &self,
        robot_id: String,
        ws_stream: WebSocketStream<TcpStream>,
    ) -> anyhow::Result<()> {
        let (ws_sink, mut ws_stream) = ws_stream.split();

        // gRPC -> WS 송신 큐 (WebRTC signaling)
        let (ws_tx, mut ws_rx) = mpsc::unbounded_channel::<SignalMessage>();

        {
            let mut guard = self.sessions.write().await;
            guard.insert(robot_id.clone(), ws_tx);
        }

        // gRPC signal stream이 아직 없으면 지금(최초) 오픈 + inbound receiver spawn
        if let Err(e) = self.grpc.ensure_signal_stream(self.sessions.clone()).await {
            let mut guard = self.sessions.write().await;
            guard.remove(&robot_id);
            return Err(e);
        }

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

    async fn handle_control_channel(
        &self,
        robot_id: String,
        ws_stream: WebSocketStream<TcpStream>,
    ) -> anyhow::Result<()> {
        let (mut ws_sink, mut ws_stream) = ws_stream.split();

        // Control도 signaling stream을 통해 robot-api로 전달한다.
        if let Err(e) = self.grpc.ensure_signal_stream(self.sessions.clone()).await {
            eprintln!(
                "[control] failed to ensure signaling stream for {robot_id}: {e}"
            );
            let _ = send_control_error(
                &mut ws_sink,
                "signaling stream not ready; command not processed",
            )
            .await;
            return Err(e);
        }

        println!("[control] ws connected for robot_id={robot_id}, signaling stream ready");
        if let Err(e) = send_control_ack(&mut ws_sink, "control channel ready").await {
            eprintln!("[control] failed to send ready ack for {robot_id}: {e}");
        }

        while let Some(msg) = ws_stream.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    println!("[control] recv raw text for {robot_id}: {text}");

                    let ws_signal = match parse_control_request(&text, &robot_id) {
                        Ok(msg) => msg,
                        Err(e) => {
                            let _ = send_control_error(&mut ws_sink, e.to_string()).await;
                            eprintln!("[control] parse error for {robot_id}: {e}");
                            continue;
                        }
                    };

                    println!("[control] parsed WsSignalMessage for {robot_id}: {:?}", ws_signal);

                    match SignalMessage::try_from(ws_signal) {
                        Ok(signal) => {
                            println!(
                                "[control] mapped to SignalMessage for {robot_id}: {:?}",
                                signal.payload
                            );

                            let sender = match self.grpc.signal_sender() {
                                Ok(s) => s,
                                Err(e) => {
                                    eprintln!(
                                        "[control] signal sender missing for {robot_id}: {e}"
                                    );
                                    let _ = send_control_error(
                                        &mut ws_sink,
                                        "signaling sender unavailable",
                                    )
                                    .await;
                                    break;
                                }
                            };

                            if let Err(e) = sender.send(signal) {
                                let _ = send_control_error(
                                    &mut ws_sink,
                                    format!("failed to deliver command: {e}"),
                                )
                                .await;
                                eprintln!("[control] failed to send over gRPC channel for {robot_id}: {e}");
                                break;
                            }

                            println!("[control] sent to gRPC channel for {robot_id}");
                            if let Err(e) = send_control_ack(&mut ws_sink, "command accepted").await {
                                eprintln!("[control] failed to send ack to client for {robot_id}: {e}");
                            }
                        }
                        Err(e) => {
                            eprintln!("[control] signal conversion error for {robot_id}: {e}");
                            if let Err(err) = send_control_error(
                                &mut ws_sink,
                                format!("invalid control command: {e}"),
                            )
                            .await
                            {
                                eprintln!("[control] failed to send error to client for {robot_id}: {err}");
                            }
                        }
                    }
                }
                Ok(Message::Close(frame)) => {
                    println!("[control] close frame for {robot_id}: {:?}", frame);
                    break;
                }
                Ok(other) => {
                    println!("[control] ignore ws message for {robot_id}: {:?}", other);
                }
                Err(e) => {
                    eprintln!("[control] websocket error for {robot_id}: {e}");
                    break;
                }
            }
        }

        println!("[control] loop finished for {robot_id}");

        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ControlRequestType {
    Move,
    Stop,
    EStop,
    SetSpeed,
    Dock,
    PathFollow,
}

#[derive(Debug, Deserialize)]
struct ControlRequest {
    #[serde(rename = "type")]
    kind: ControlRequestType,

    #[serde(default)]
    payload: Value,
}

fn parse_control_request(text: &str, robot_id: &str) -> anyhow::Result<WsSignalMessage> {
    let req: ControlRequest = serde_json::from_str(text)?;

    let payload = match req.payload {
        Value::Object(map) => map,
        Value::Null => Map::new(),
        other => {
            return Err(anyhow!(
                "control payload must be an object, got: {other}"
            ))
        }
    };

    let (command, payload) = match req.kind {
        ControlRequestType::Move => {
            let direction = payload
                .get("direction")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("direction is required for move command"))?;
            let speed = payload
                .get("speed")
                .and_then(Value::as_f64)
                .unwrap_or(1.0);

            (
                CommandType::Move,
                Some(ControlPayload::Move {
                    direction: direction.to_string(),
                    speed,
                }),
            )
        }
        ControlRequestType::Stop => (CommandType::Stop, None),
        ControlRequestType::EStop => (CommandType::EmergencyStop, None),
        ControlRequestType::SetSpeed => {
            let speed = payload
                .get("speed")
                .and_then(Value::as_f64)
                .ok_or_else(|| anyhow!("speed is required for set_speed command"))?;

            (
                CommandType::SetSpeed,
                Some(ControlPayload::SetSpeed { speed }),
            )
        }
        ControlRequestType::Dock => (CommandType::Dock, None),
        ControlRequestType::PathFollow => {
            let path_id = payload
                .get("path_id")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("path_id is required for path_follow command"))?;

            (
                CommandType::PathFollow,
                Some(ControlPayload::PathFollow {
                    path_id: path_id.to_string(),
                }),
            )
        }
    };

    Ok(WsSignalMessage::ControlCommand {
        robot_id: robot_id.to_string(),
        command,
        payload,
    })
}

fn extract_robot_id(path: &str, prefix: &str) -> Option<String> {
    path.strip_prefix(prefix)
        .map(|rest| rest.trim_end_matches('/').to_string())
}

async fn send_control_ack(ws_sink: &mut WsSink, message: impl Into<String>) -> anyhow::Result<()> {
    let payload = json!({
        "type": "control_ack",
        "message": message.into(),
    });

    ws_sink.send(Message::Text(payload.to_string().into())).await?;
    Ok(())
}

async fn send_control_error(
    ws_sink: &mut WsSink,
    message: impl Into<String>,
) -> anyhow::Result<()> {
    let payload = json!({
        "type": "control_error",
        "message": message.into(),
    });

    ws_sink.send(Message::Text(payload.to_string().into())).await?;
    Ok(())
}
