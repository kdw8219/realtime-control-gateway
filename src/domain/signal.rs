use serde::{Deserialize, Serialize};

/// WebSocket <-> Gateway signaling 메시지
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WsSignalMessage {
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
}

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
