pub mod websocket;
pub mod grpc;

pub mod robot {
    pub mod control {
        tonic::include_proto!("robot.control");
    }

    pub mod signaling {
        tonic::include_proto!("robot.signaling");
    }
}