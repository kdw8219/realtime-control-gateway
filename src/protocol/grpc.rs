use crate::protocol::robot::control::robot_control_service_client::RobotControlServiceClient;

pub struct GrpcClientHandler {
    pub to_ip: String,
    pub to_port: String,
}

impl GrpcClientHandler {
    pub async fn connect(&self) -> anyhow::Result<()> {
        let addr = format!("http://{}:{}"
        , self.to_ip
        , self.to_port
        );
        let mut client = RobotControlServiceClient::connect(addr).await?;
        Ok(())
    }
    pub async fn handle_request(&self) -> anyhow::Result<()> {
        // Implementation goes here
        Ok(())
    }
}