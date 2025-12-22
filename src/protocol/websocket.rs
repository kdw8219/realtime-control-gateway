use std::sync::Arc;

use anyhow::anyhow;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Map, Value};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::{
    accept_hdr_async,
    tungstenite::handshake::server::{Request, Response},
    tungstenite::Message,
    WebSocketStream,
};

use crate::domain::control::{ControlRequest, ControlRequestType};
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
        log::info!("[ws] handshake peer={peer} path={path}");

        if let Some(robot_id) = extract_robot_id(&path, "/ws/screen/") {
            log::info!("[ws] route=screen robot_id={robot_id}");
            self.handle_screen_channel(robot_id, ws_stream).await
        } else if let Some(robot_id) = extract_robot_id(&path, "/ws/control/") {
            log::info!("[ws] route=control robot_id={robot_id}");
            self.handle_control_channel(robot_id, ws_stream).await
        } else {
            anyhow::bail!("unsupported websocket path: {path}");
        }
    }

    async fn init_signaling(&self, robot_id: &str) -> anyhow::Result<()> {
        let hello = SignalMessage {
            robot_id: robot_id.to_string(),
            payload: None,
        };

        // Ensure the bi-di stream is open and immediately send a handshake message
        // carrying only the robot_id so the gRPC server can bind the session.
        self.grpc
            .ensure_signal_stream(self.sessions.clone(), Some(hello))
            .await?;
        log::info!("[ws] sent initial signaling handshake for {robot_id}");

        Ok(())
    }

    async fn handle_screen_channel(
        &self,
        robot_id: String,
        ws_stream: WebSocketStream<TcpStream>,
    ) -> anyhow::Result<()> {
        log::info!("[screen] handle_screen_channel enter robot_id={robot_id}");
        let (ws_sink, mut ws_stream) = ws_stream.split();

        // gRPC -> WS 송신 큐 (WebRTC signaling)
        let (ws_tx, mut ws_rx) = mpsc::unbounded_channel::<SignalMessage>();

        {
            let mut guard = self.sessions.write().await;
            guard.insert(robot_id.clone(), ws_tx);
        }

        // gRPC signal stream을 즉시 준비시키고 handshake 메시지를 전송
        self.init_signaling(&robot_id)
            .await
            .map_err(|e| {
                log::error!("[screen] failed to init signaling for {robot_id}: {e:?}");
                e
            })?;
        log::info!("[screen] signaling stream ready for {robot_id}");

        // WS로 내려보내는 task (SignalMessage -> WsSignalMessage -> JSON)
        let robot_id_for_task = robot_id.clone();
        tokio::spawn(async move {
            let mut ws_sink = ws_sink;

            while let Some(msg) = ws_rx.recv().await {
                log::info!(
                    "[screen] outbound to client robot_id={}: {:?}",
                    robot_id_for_task,
                    msg.payload
                );
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
                    log::info!("[screen] inbound text from client robot_id={}: {text}", robot_id);
                    let ws_msg: WsSignalMessage = serde_json::from_str(text.as_str())?;
                    let signal: SignalMessage = ws_msg.try_into()?;

                    let mut sent = false;
                    // 1차 시도
                    if let Ok(sender) = self.grpc.signal_sender().await {
                        if sender.send(signal.clone()).is_ok() {
                            sent = true;
                        } else {
                            log::warn!("[screen] failed to send signal to gRPC for {}: channel closed (retrying)", robot_id);
                        }
                    }

                    // 재시도: 스트림 재연결 후 다시 전송
                    if !sent {
                        if let Err(e) = self.init_signaling(&robot_id).await {
                            log::warn!("[screen] retry init signaling failed for {}: {}", robot_id, e);
                        } else if let Ok(sender) = self.grpc.signal_sender().await {
                            if sender.send(signal).is_ok() {
                                sent = true;
                                log::info!("[screen] resent signal after reconnect for {}", robot_id);
                            }
                        }
                    }

                    if !sent {
                        log::warn!(
                            "[screen] gRPC signal sender not ready for {} (will keep WS open)",
                            robot_id
                        );
                    }
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
        log::info!("[control] handle_control_channel enter robot_id={robot_id}");
        let (mut ws_sink, mut ws_stream) = ws_stream.split();

        // Control도 signaling stream을 통해 robot-api로 전달한다. (비동기 준비)
        self.init_signaling(&robot_id)
            .await
            .map_err(|e| {
                log::error!("[control] failed to init signaling for {robot_id}: {e:?}");
                e
            })?;
        log::info!("[control] signaling stream ready for {robot_id}");

        log::info!("[control] ws connected for robot_id={robot_id}, signaling stream ready");
        if let Err(e) = send_control_ack(&mut ws_sink, "control channel ready").await {
            eprintln!("[control] failed to send ready ack for {robot_id}: {e}");
        }

        while let Some(msg) = ws_stream.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    log::info!("[control] recv raw text for {robot_id}: {text}");

                    let ws_signal = match parse_control_request(&text, &robot_id) {
                        Ok(msg) => msg,
                        Err(e) => {
                            let _ = send_control_error(&mut ws_sink, e.to_string()).await;
                            eprintln!("[control] parse error for {robot_id}: {e}");
                            continue;
                        }
                    };

                    log::info!("[control] parsed WsSignalMessage for {robot_id}: {:?}", ws_signal);

                    match SignalMessage::try_from(ws_signal) {
                        Ok(signal) => {
                            log::info!(
                                "[control] mapped to SignalMessage for {robot_id}: {:?}",
                                signal.payload
                            );

                            let mut delivered = false;
                            if let Ok(sender) = self.grpc.signal_sender().await {
                                if sender.send(signal.clone()).is_ok() {
                                    delivered = true;
                                } else {
                                    log::warn!("[control] gRPC channel closed for {robot_id}, retrying");
                                }
                            }

                            if !delivered {
                                if let Err(e) = self.init_signaling(&robot_id).await {
                                    log::warn!("[control] retry init signaling failed for {robot_id}: {e}");
                                } else if let Ok(sender) = self.grpc.signal_sender().await {
                                    if sender.send(signal.clone()).is_ok() {
                                        delivered = true;
                                        log::info!("[control] resent signal after reconnect for {robot_id}");
                                    }
                                }
                            }

                            if !delivered {
                                let _ = send_control_error(
                                    &mut ws_sink,
                                    "signaling sender unavailable",
                                )
                                .await;
                                eprintln!("[control] failed to send over gRPC channel for {robot_id}");
                                break;
                            }

                            log::info!("[control] sent to gRPC channel for {robot_id}");
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
                    log::info!("[control] close frame for {robot_id}: {:?}", frame);
                    break;
                }
                Ok(other) => {
                    log::info!("[control] ignore ws message for {robot_id}: {:?}", other);
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
