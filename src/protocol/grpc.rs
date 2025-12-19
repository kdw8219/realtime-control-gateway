use futures_util::StreamExt;
use tonic::transport::{Channel, Endpoint};
use tokio::sync::{mpsc, Mutex, OnceCell};
use tokio_stream::wrappers::UnboundedReceiverStream;
use log::{info, warn, error, debug};
use tokio::time::{timeout, Duration};

use crate::session::manager::SharedSessions;

use crate::protocol::robot::signaling::{
    robot_signal_service_client::RobotSignalServiceClient,
    SignalMessage,
};

pub struct GrpcClient {
    signal: RobotSignalServiceClient<Channel>,

    // lazy-init된 outbound sender (Gateway -> grpc-robot-api)
    signal_tx: OnceCell<mpsc::UnboundedSender<SignalMessage>>,

    // init 경쟁 방지
    init_lock: Mutex<()>,
}

impl GrpcClient {
    pub async fn connect(addr: String) -> anyhow::Result<Self> {
        let channel = Endpoint::from_shared(addr)?.connect().await?;

        Ok(Self {
            signal: RobotSignalServiceClient::new(channel),
            signal_tx: OnceCell::new(),
            init_lock: Mutex::new(()),
        })
    }

    pub async fn ensure_signal_stream(&self, sessions: SharedSessions) -> anyhow::Result<()> {
        if self.signal_tx.get().is_some() {
            debug!("[grpc] ensure_signal_stream: already initialized");
            return Ok(());
        }

        let _g = self.init_lock.lock().await;
        if self.signal_tx.get().is_some() {
            debug!("[grpc] ensure_signal_stream: already initialized (after lock)");
            return Ok(());
        }

        info!("[grpc] ensure_signal_stream: opening bi-di stream...");

        // Gateway -> grpc-robot-api outbound
        let (tx, rx) = mpsc::unbounded_channel::<SignalMessage>();
        let outbound = UnboundedReceiverStream::new(rx);

        // 실제 RPC 호출은 여기서 발생 (lazy-init)
        let response = match self
            .signal
            .clone()
            .open_signal_stream(outbound)
            .await
        {
            Ok(resp) => resp,
            Err(e) => {
                error!("[grpc] failed to open signal stream: {:?}", e);
                return Err(e.into());
            }
        };

        let inbound = response.into_inner();

        // sender 저장
        let _ = self.signal_tx.set(tx);
        info!("[grpc] ensure_signal_stream: stream opened and sender stored");

        // inbound receiver spawn (1회)
        tokio::spawn(async move {
            let mut inbound = Box::pin(inbound);

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
                        error!("gRPC inbound stream error: {:?}", e);
                        break;
                    }
                }
            }

            error!("gRPC signaling stream closed");
        });

        Ok(())
    }

    pub fn signal_sender(&self) -> anyhow::Result<&mpsc::UnboundedSender<SignalMessage>> {
        self.signal_tx
            .get()
            .ok_or_else(|| anyhow::anyhow!("signal stream not initialized (call ensure_signal_stream first)"))
    }
}
