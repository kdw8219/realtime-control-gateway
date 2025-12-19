use anyhow::{anyhow, Result};

use crate::domain::signal::{
    WsSignalMessage, WsIceCandidate, WsSessionDescription, CommandType, ControlPayload,
};

// gRPC(prost) 타입
use crate::protocol::robot::signaling::{
    signal_message,
    ClientAnswer,
    ControlCommand as GrpcControlCommand,
    IceCandidate,
    RobotOffer,
    ScreenRequest,
    SignalMessage,
    WebrtcError,
    CommandType as GrpcCommandType,
    MovePayload,
    SetSpeedPayload,
    PathFollowPayload,
};

impl From<CommandType> for GrpcCommandType {
    fn from(cmd: CommandType) -> Self {
        match cmd {
            CommandType::Move => GrpcCommandType::Move,
            CommandType::Stop => GrpcCommandType::Stop,
            CommandType::EmergencyStop => GrpcCommandType::EmergencyStop,
            CommandType::SetSpeed => GrpcCommandType::SetSpeed,
            CommandType::Dock => GrpcCommandType::Dock,
            CommandType::PathFollow => GrpcCommandType::PathFollow,
        }
    }
}

impl TryFrom<GrpcCommandType> for CommandType {
    type Error = anyhow::Error;

    fn try_from(cmd: GrpcCommandType) -> Result<Self> {
        match cmd {
            GrpcCommandType::Move => Ok(CommandType::Move),
            GrpcCommandType::Stop => Ok(CommandType::Stop),
            GrpcCommandType::EmergencyStop => Ok(CommandType::EmergencyStop),
            GrpcCommandType::SetSpeed => Ok(CommandType::SetSpeed),
            GrpcCommandType::Dock => Ok(CommandType::Dock),
            GrpcCommandType::PathFollow => Ok(CommandType::PathFollow),
            GrpcCommandType::CommandUnknown => Err(anyhow!("unknown control command type")),
        }
    }
}

impl TryFrom<WsSignalMessage> for SignalMessage {
    type Error = anyhow::Error;

    fn try_from(ws: WsSignalMessage) -> Result<Self> {
        match ws {
            /* ---------- WebRTC ---------- */

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

            /* ---------- Control ---------- */

            WsSignalMessage::ControlCommand {
                robot_id,
                command,
                payload,
            } => {
                let grpc_payload = match payload {
                    Some(ControlPayload::Move { direction, speed }) => {
                        Some(crate::protocol::robot::signaling::control_command::Payload::Move(
                            MovePayload { direction, speed },
                        ))
                    }
                    Some(ControlPayload::SetSpeed { speed }) => {
                        Some(crate::protocol::robot::signaling::control_command::Payload::SetSpeed(
                            SetSpeedPayload { speed },
                        ))
                    }
                    Some(ControlPayload::PathFollow { path_id }) => {
                        Some(crate::protocol::robot::signaling::control_command::Payload::PathFollow(
                            PathFollowPayload { path_id },
                        ))
                    }
                    None => None,
                };

                let grpc_command: i32 = GrpcCommandType::from(command) as i32;

                Ok(SignalMessage {
                    robot_id,
                    payload: Some(signal_message::Payload::ControlCommand(GrpcControlCommand {
                        command: grpc_command,
                        payload: grpc_payload,
                    })),
                })
            }
        }
    }
}

impl TryFrom<SignalMessage> for WsSignalMessage {
    type Error = anyhow::Error;

    fn try_from(msg: SignalMessage) -> Result<Self> {
        let robot_id = msg.robot_id;

        match msg.payload {
            /* ---------- WebRTC ---------- */

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

            /* ---------- Control ---------- */

            Some(signal_message::Payload::ControlCommand(cmd)) => {
                let grpc_cmd = GrpcCommandType::from_i32(cmd.command)
                    .ok_or_else(|| anyhow!("unknown control command type value: {}", cmd.command))?;

                let payload = match cmd.payload {
                    Some(crate::protocol::robot::signaling::control_command::Payload::Move(m)) => {
                        Some(ControlPayload::Move {
                            direction: m.direction,
                            speed: m.speed,
                        })
                    }
                    Some(crate::protocol::robot::signaling::control_command::Payload::SetSpeed(s)) => {
                        Some(ControlPayload::SetSpeed { speed: s.speed })
                    }
                    Some(crate::protocol::robot::signaling::control_command::Payload::PathFollow(p)) => {
                        Some(ControlPayload::PathFollow {
                            path_id: p.path_id,
                        })
                    }
                    None => None,
                };

                Ok(WsSignalMessage::ControlCommand {
                    robot_id,
                    command: grpc_cmd.try_into()?,
                    payload,
                })
            }

            None => Err(anyhow!("empty SignalMessage payload")),
        }
    }
}
