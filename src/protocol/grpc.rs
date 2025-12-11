use crate::protocol::robot::control::robot_control_service_client::RobotControlServiceClient;
use crate::protocol::robot::signaling::robot_signal_service_client::RobotSignalServiceClient;

pub struct GrpcClientHandler {
    pub to_ip: String,
    pub to_port: String,
    pub control_client: Option<RobotControlServiceClient<tonic::transport::Channel>>,
    pub signal_client: Option<RobotSignalServiceClient<tonic::transport::Channel>>,
}

impl GrpcClientHandler {
    pub async fn connect(&mut self) -> anyhow::Result<()> {

        let addr = format!("http://{}:{}"
        , self.to_ip
        , self.to_port
        );

        let channel = tonic::transport::Channel::from_shared(addr)?
            .connect()
            .await?;

        let control_client = RobotControlServiceClient::new(channel.clone());
        let signal_client = RobotSignalServiceClient::new(channel.clone());
        self.control_client = Some(control_client);
        self.signal_client = Some(signal_client);
        Ok(())
    }

    pub async fn handle_request(&self) -> anyhow::Result<()> {
        // Implementation goes here
        Ok(())
    }
}