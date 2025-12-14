use futures_util::Stream;
use tonic::transport::Channel;
use serde_json::Value;
use tokio::sync::mpsc;
use tonic::Status;
use tokio_stream::wrappers::UnboundedReceiverStream;

use crate::protocol::{
    robot::control::robot_control_service_client::RobotControlServiceClient,
    robot::signaling::robot_signal_service_client::RobotSignalServiceClient,
    robot::signaling::SignalMessage,
    robot::control::{CommandRequest, CommandResponse},
};

pub struct GrpcClient {
    pub control: RobotControlServiceClient<Channel>,
    pub signal_tx: mpsc::UnboundedSender<SignalMessage>,
}

impl GrpcClient {
    pub async fn connect(addr: String) -> anyhow::Result<(Self, impl Stream<Item = Result<SignalMessage, tonic::Status>>)> {
        let channel = tonic::transport::Endpoint::from_shared(addr)?.connect().await?;
        
        let mut signal =
            RobotSignalServiceClient::new(channel.clone());

        let control =
            RobotControlServiceClient::new(channel);

        // Gateway → grpc-robot-api
        let (tx, rx) = mpsc::unbounded_channel::<SignalMessage>();
        let outbound = UnboundedReceiverStream::new(rx);

        // open shared bidi-stream (1회)
        let response = signal
            .open_signal_stream(outbound)
            .await?;

        let inbound = response.into_inner();

        Ok((
            Self {
                control: control,
                signal_tx: tx,
            },
            inbound,
        ))
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



