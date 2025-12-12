use futures_util::Stream;
use tonic::transport::Channel;
use serde_json::Value;
use tokio::sync::mpsc;
use tonic::Status;


use crate::protocol::{
    robot::control::robot_control_service_client::RobotControlServiceClient,
    robot::signaling::robot_signal_service_client::RobotSignalServiceClient,
    robot::signaling::SignalMessage
};

pub struct GrpcClient {
    control: RobotControlServiceClient<Channel>,
    signal: RobotSignalServiceClient<Channel>,
}

impl GrpcClient {
    pub async fn connect(addr: String) -> anyhow::Result<Self> {
        let channel = tonic::transport::Endpoint::from_shared(addr)?.connect().await?;
        
        let control = RobotControlServiceClient::new(channel.clone());
        let signal = RobotSignalServiceClient::new(channel.clone());

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
            .signal(outbound)
            .await?;

        let inbound = response.into_inner();

        // 첫 메시지로 "세션 시작" 알림 (선택)
        tx.send(SignalMessage {
            robot_id,
            payload: None,
        })?;

        Ok((tx, inbound))
    }

    pub async fn send_control(
        &self,
        robot_id: String,
        command: String,
        payload: Value,
    ) -> anyhow::Result<Value> {
        // 실제 구현
        Ok(serde_json::json!({"status": "ok"}))
    }

    pub async fn signal(
        &self,
        robot_id: String,
        sig_type: String,
        payload: Value,
    ) -> anyhow::Result<()> {
    
        Ok(())
    }

    pub async fn subscribe_signal(
        &self,
        robot_id: String,
    ) -> anyhow::Result<impl Stream<Item = Result<SignalMessage, tonic::Status>>> {
        let req = tonic::Request::new(crate::protocol::robot::signaling::SignalMessage { robot_id, payload:None });
        let stream = self.signal.clone().subscribe(req).await?.into_inner();
        Ok(stream)
    }

}



