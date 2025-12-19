pub mod websocket;
pub mod grpc;

pub mod robot {
    pub mod signaling {
        tonic::include_proto!("robot.signaling");
    }
}
