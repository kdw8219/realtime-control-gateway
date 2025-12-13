use futures_util::Stream;
use tonic::transport::Channel;
use serde_json::Value;
use tokio::sync::mpsc;
use tonic::Status;


use crate::protocol::{
    robot::control::robot_control_service_client::RobotControlServiceClient,
    robot::signaling::robot_signal_service_client::RobotSignalServiceClient,
    robot::signaling::SignalMessage,
    robot::control::{CommandRequest, CommandResponse},
};

pub struct GrpcClient {
    control: RobotControlServiceClient<Channel>,
    signal: RobotSignalServiceClient<Channel>,
}

impl GrpcClient {
    pub async fn connect(addr: String) -> anyhow::Result<Self> {
        let channel = tonic::transport::Endpoint::from_shared(addr)?.connect().await?;
        
        let control = RobotControlServiceClient::new(channel.clone());
        let signal = RobotSignalServiceClient::new(channel); //둘 중 하나는 원본을 써도...

        Ok(Self {
            control,
            signal,
        })
    }

    pub async fn open_signal_stream(
        &self,
        robot_id: String,
    ) -> anyhow::Result<(
        mpsc::UnboundedSender<SignalMessage>,
        impl Stream<Item = Result<SignalMessage, Status>>,
    )> {
        // client → robot 송신 채널
        let (tx, rx) = mpsc::unbounded_channel::<SignalMessage>();
        let outbound = tokio_stream::wrappers::UnboundedReceiverStream::new(rx);
        
        let response = self
            .signal
            .clone()
            .open_signal_stream(outbound)
            .await?;

        let inbound = response.into_inner();

        // 첫 메시지로 "세션 시작" 알림 (선택)
        tx.send(SignalMessage {
            robot_id,
            payload: None,
        })?;

        Ok((tx, inbound))
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



