use futures_util::StreamExt;
use tonic::transport::{Channel, Endpoint};
use tokio::sync::{mpsc, Mutex, OnceCell};
use tokio_stream::wrappers::UnboundedReceiverStream;
use log::{info, warn, error, debug};

use crate::session::manager::SharedSessions;

use crate::protocol::{
    robot::control::robot_control_service_client::RobotControlServiceClient,
    robot::signaling::robot_signal_service_client::RobotSignalServiceClient,
    robot::signaling::SignalMessage,
    robot::control::{CommandRequest, CommandResponse},
};

pub struct GrpcClient {
    control: RobotControlServiceClient<Channel>,
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
            control: RobotControlServiceClient::new(channel.clone()),
            signal: RobotSignalServiceClient::new(channel),
            signal_tx: OnceCell::new(),
            init_lock: Mutex::new(()),
        })
    }

    pub async fn ensure_signal_stream(&self, sessions: SharedSessions) -> anyhow::Result<()> {
        if self.signal_tx.get().is_some() {
            return Ok(());
        }

        let _g = self.init_lock.lock().await;
        if self.signal_tx.get().is_some() {
            return Ok(());
        }

        // Gateway -> grpc-robot-api outbound
        let (tx, rx) = mpsc::unbounded_channel::<SignalMessage>();
        let outbound = UnboundedReceiverStream::new(rx);

        // 실제 RPC 호출은 여기서 발생 (lazy-init)
        let response = self
            .signal
            .clone()
            .open_signal_stream(outbound)
            .await?;

        let inbound = response.into_inner();

        // sender 저장
        let _ = self.signal_tx.set(tx);

        // inbound receiver spawn (1회)
        tokio::spawn(async move {
            let mut inbound = Box::pin(inbound);

            while let Some(item) = inbound.next().await {
                match item {
                    Ok(msg) => {
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

    pub async fn send_command(
        &self,
        req: CommandRequest,
    ) -> anyhow::Result<CommandResponse> {
        let resp = self
            .control
            .clone()
            .send_command(req)
            .await?
            .into_inner();

        Ok(resp)
    }
}
