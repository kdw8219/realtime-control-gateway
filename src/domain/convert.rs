use anyhow::{anyhow, Result};

use crate::domain::signal::{WsSignalMessage, WsIceCandidate, WsSessionDescription};

// gRPC(prost) 타입들
use crate::protocol::robot::signaling::{
    signal_message, ClientAnswer, IceCandidate, RobotOffer, ScreenRequest, SignalMessage, WebrtcError,
};

impl TryFrom<WsSignalMessage> for SignalMessage {
    type Error = anyhow::Error;

    fn try_from(ws: WsSignalMessage) -> Result<Self> {
        match ws {
            WsSignalMessage::ScreenRequest { robot_id } => Ok(SignalMessage {
                robot_id,
                payload: Some(signal_message::Payload::ScreenRequest(ScreenRequest {})),
            }),

            WsSignalMessage::RobotOffer { robot_id, offer } => Ok(SignalMessage {
                robot_id,
                payload: Some(signal_message::Payload::RobotOffer(RobotOffer {
                    sdp: offer.sdp,
                    r#type: offer.sdp_type,
                })),
            }),

            WsSignalMessage::ClientAnswer { robot_id, answer } => Ok(SignalMessage {
                robot_id,
                payload: Some(signal_message::Payload::ClientAnswer(ClientAnswer {
                    sdp: answer.sdp,
                    r#type: answer.sdp_type,
                })),
            }),

            WsSignalMessage::RobotIce { robot_id, ice } => Ok(SignalMessage {
                robot_id,
                payload: Some(signal_message::Payload::RobotIce(IceCandidate {
                    candidate: ice.candidate,
                    sdp_mid: ice.sdp_mid,
                    sdp_mline_index: ice.sdp_mline_index,
                })),
            }),

            WsSignalMessage::ClientIce { robot_id, ice } => Ok(SignalMessage {
                robot_id,
                payload: Some(signal_message::Payload::ClientIce(IceCandidate {
                    candidate: ice.candidate,
                    sdp_mid: ice.sdp_mid,
                    sdp_mline_index: ice.sdp_mline_index,
                })),
            }),

            WsSignalMessage::WebrtcError { robot_id, error } => Ok(SignalMessage {
                robot_id,
                payload: Some(signal_message::Payload::WebrtcError(WebrtcError { error })),
            }),
        }
    }
}


impl TryFrom<SignalMessage> for WsSignalMessage {
    type Error = anyhow::Error;

    fn try_from(msg: SignalMessage) -> Result<Self> {
        let robot_id = msg.robot_id;

        match msg.payload {
            Some(signal_message::Payload::ScreenRequest(_)) => {
                Ok(WsSignalMessage::ScreenRequest { robot_id })
            }

            Some(signal_message::Payload::RobotOffer(o)) => {
                Ok(WsSignalMessage::RobotOffer {
                    robot_id,
                    offer: WsSessionDescription {
                        sdp: o.sdp,
                        sdp_type: o.r#type,
                    },
                })
            }

            Some(signal_message::Payload::ClientAnswer(a)) => {
                Ok(WsSignalMessage::ClientAnswer {
                    robot_id,
                    answer: WsSessionDescription {
                        sdp: a.sdp,
                        sdp_type: a.r#type,
                    },
                })
            }

            Some(signal_message::Payload::RobotIce(i)) => {
                Ok(WsSignalMessage::RobotIce {
                    robot_id,
                    ice: WsIceCandidate {
                        candidate: i.candidate,
                        sdp_mid: i.sdp_mid,
                        sdp_mline_index: i.sdp_mline_index,
                    },
                })
            }

            Some(signal_message::Payload::ClientIce(i)) => {
                Ok(WsSignalMessage::ClientIce {
                    robot_id,
                    ice: WsIceCandidate {
                        candidate: i.candidate,
                        sdp_mid: i.sdp_mid,
                        sdp_mline_index: i.sdp_mline_index,
                    },
                })
            }

            Some(signal_message::Payload::WebrtcError(e)) => {
                Ok(WsSignalMessage::WebrtcError {
                    robot_id,
                    error: e.error,
                })
            }

            None => Err(anyhow!("empty SignalMessage payload")),
        }
    }
}
