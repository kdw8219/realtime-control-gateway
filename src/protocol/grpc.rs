use futures_util::StreamExt;
use anyhow::anyhow;
use tonic::transport::{Channel, Endpoint};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio_stream::wrappers::UnboundedReceiverStream;
use log::{info, error, debug};

use crate::session::manager::SharedSessions;

use crate::protocol::robot::signaling::{
    robot_signal_service_client::RobotSignalServiceClient,
    SignalMessage,
};

pub struct GrpcClient {
    signal: RobotSignalServiceClient<Channel>,

    // lazy-init된 outbound sender (Gateway -> grpc-robot-api), 재연결 가능
    signal_tx: Arc<Mutex<Option<mpsc::UnboundedSender<SignalMessage>>>>,

    // init 경쟁 방지
    init_lock: Mutex<()>,
}

impl GrpcClient {
    pub async fn connect(addr: String) -> anyhow::Result<Self> {
        let channel = Endpoint::from_shared(addr)?.connect().await?;

        Ok(Self {
            signal: RobotSignalServiceClient::new(channel),
            signal_tx: Arc::new(Mutex::new(None)),
            init_lock: Mutex::new(()),
        })
    }

    pub async fn ensure_signal_stream(
        &self,
        sessions: SharedSessions,
        initial: Option<SignalMessage>,
    ) -> anyhow::Result<()> {
        if let Some(sender) = self.signal_tx.lock().await.clone() {
            if let Some(ref msg) = initial {
                sender
                    .send(msg.clone())
                    .map_err(|e| anyhow!("failed to send initial signal: {e}"))?;
            }
            debug!("[grpc] ensure_signal_stream: already initialized");
            return Ok(());
        }

        let _g = self.init_lock.lock().await;
        if let Some(sender) = self.signal_tx.lock().await.clone() {
            if let Some(ref msg) = initial {
                sender
                    .send(msg.clone())
                    .map_err(|e| anyhow!("failed to send initial signal: {e}"))?;
            }
            debug!("[grpc] ensure_signal_stream: already initialized (after lock)");
            return Ok(());
        }

        info!("[grpc] ensure_signal_stream: opening bi-di stream...");

        // Gateway -> grpc-robot-api outbound
        let (tx, rx) = mpsc::unbounded_channel::<SignalMessage>();
        if let Some(msg) = initial {
            tx.send(msg)
                .map_err(|e| anyhow!("failed to send initial signal before open: {e}"))?;
        }
        let outbound = UnboundedReceiverStream::new(rx);

        // 실제 RPC 호출은 여기서 발생 (lazy-init)
        let mut client = self.signal.clone();
        let response = match client.open_signal_stream(outbound).await {
            Ok(resp) => resp,
            Err(e) => {
                error!("[grpc] failed to open signal stream: {:?}", e);
                return Err(e.into());
            }
        };

        let inbound = response.into_inner();

        // sender 저장
        {
            let mut guard = self.signal_tx.lock().await;
            *guard = Some(tx.clone());
        }
        info!("[grpc] ensure_signal_stream: stream opened and sender stored");

        // inbound receiver spawn (1회)
        let signal_tx = self.signal_tx.clone();
        tokio::spawn(async move {
            let mut inbound = Box::pin(inbound);
            let mut last_err: Option<tonic::Status> = None;

            while let Some(item) = inbound.next().await {
                match item {
                    Ok(msg) => {
                        debug!("[grpc] inbound msg for robot_id={}", msg.robot_id);
                        let robot_id = msg.robot_id.clone();

                        let guard = sessions.read().await;
                        if let Some(ws_tx) = guard.get_ws_sender(&robot_id) {
                            let _ = ws_tx.send(msg);
                        }
                    }
                    Err(e) => {
                        error!("[grpc] inbound stream error: {:?}", e);
                        last_err = Some(e);
                        break;
                    }
                }
            }

            match inbound.trailers().await {
                Ok(Some(md)) => {
                    error!(
                        "[grpc] signaling stream closed with trailers: {:?} last_err={:?}",
                        md, last_err
                    );
                }
                Ok(None) => {
                    error!("[grpc] signaling stream closed cleanly last_err={:?}", last_err);
                }
                Err(status) => {
                    error!(
                        "[grpc] signaling stream trailers error: {:?} last_err={:?}",
                        status, last_err
                    );
                }
            }
            // 연결이 종료되면 sender를 비워 재연결을 허용
            let mut guard = signal_tx.lock().await;
            *guard = None;
        });

        Ok(())
    }

    pub async fn signal_sender(&self) -> anyhow::Result<mpsc::UnboundedSender<SignalMessage>> {
        self.signal_tx
            .lock()
            .await
            .clone()
            .ok_or_else(|| anyhow::anyhow!("signal stream not initialized (call ensure_signal_stream first)"))
    }

    /// gRPC bi-di signaling 스트림을 명시적으로 닫는다.
    /// sender를 drop 하면 tonic outbound 스트림이 종료되어 서버도 정리된다.
    pub async fn close_signal_stream(&self) {
        let mut guard = self.signal_tx.lock().await;
        if guard.is_some() {
            debug!("[grpc] closing signal stream (drop sender)");
        }
        *guard = None;
    }
}
