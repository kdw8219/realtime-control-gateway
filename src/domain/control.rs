use serde::{Deserialize, Serialize};

/// WebSocket <-> Gateway control 메시지
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WsControlMessage {
    #[serde(rename = "move")]
    Move {
        robot_id: String,
        payload: MovePayload,
    },

    #[serde(rename = "stop")]
    Stop {
        robot_id: String,
        payload: EmptyPayload,
    },

    #[serde(rename = "e_stop")]
    EmergencyStop {
        robot_id: String,
        payload: EmptyPayload,
    },

    #[serde(rename = "set_speed")]
    SetSpeed {
        robot_id: String,
        payload: SetSpeedPayload,
    },

    #[serde(rename = "dock")]
    Dock {
        robot_id: String,
        payload: EmptyPayload,
    },

    #[serde(rename = "path_follow")]
    PathFollow {
        robot_id: String,
        payload: PathFollowPayload,
    },
}
#[derive(Debug, Serialize, Deserialize)]
pub struct MovePayload {
    pub direction: String, // "forward" | "backward" | "left" | "right"
    pub speed: f32,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct EmptyPayload {}

#[derive(Debug, Serialize, Deserialize)]
pub struct SetSpeedPayload {
    pub speed: f32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PathFollowPayload {
    pub path_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WsControlResponse {
    #[serde(rename = "control_ack")]
    Ack {
        robot_id: String,
        message: String,
    },

    #[serde(rename = "control_error")]
    Error {
        robot_id: String,
        message: String,
    },
}
