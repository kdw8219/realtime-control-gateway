use serde::{Deserialize, Serialize};

/* ============================
 * WebSocket <-> Gateway Signal
 * ============================ */

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WsSignalMessage {
    /* ---------- WebRTC ---------- */

    #[serde(rename = "screen_request")]
    ScreenRequest {
        robot_id: String,
    },

    #[serde(rename = "robot_offer")]
    RobotOffer {
        robot_id: String,
        offer: WsSessionDescription,
    },

    #[serde(rename = "client_answer")]
    ClientAnswer {
        robot_id: String,
        answer: WsSessionDescription,
    },

    #[serde(rename = "robot_ice")]
    RobotIce {
        robot_id: String,
        ice: WsIceCandidate,
    },

    #[serde(rename = "client_ice")]
    ClientIce {
        robot_id: String,
        ice: WsIceCandidate,
    },

    #[serde(rename = "webrtc_error")]
    WebrtcError {
        robot_id: String,
        error: String,
    },

    /* ---------- Control ---------- */

    #[serde(rename = "control_command")]
    ControlCommand {
        robot_id: String,
        command: CommandType,

        #[serde(flatten)]
        payload: Option<ControlPayload>,
    },
}

/* ============================
 * Control Types
 * ============================ */

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CommandType {
    Move,
    Stop,
    EmergencyStop,
    SetSpeed,
    Dock,
    PathFollow,
}

/* ============================
 * Control Payload
 * ============================ */

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "payload_type")]
pub enum ControlPayload {
    #[serde(rename = "move")]
    Move {
        direction: String,
        speed: f64,
    },

    #[serde(rename = "set_speed")]
    SetSpeed {
        speed: f64,
    },

    #[serde(rename = "path_follow")]
    PathFollow {
        path_id: String,
    },
}

/* ============================
 * WebRTC Payloads
 * ============================ */

#[derive(Debug, Serialize, Deserialize)]
pub struct WsSessionDescription {
    pub sdp: String,

    #[serde(rename = "type")]
    pub sdp_type: String, // "offer" | "answer"
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WsIceCandidate {
    pub candidate: String,

    #[serde(rename = "sdpMid")]
    pub sdp_mid: String,

    #[serde(rename = "sdpMLineIndex")]
    pub sdp_mline_index: i32,
}
