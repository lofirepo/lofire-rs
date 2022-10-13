use crate::types::BrokerMessage;
use lofire::errors::*;
use lofire::types::Block;
use num_enum::IntoPrimitive;
use num_enum::TryFromPrimitive;
use std::convert::From;
use std::convert::TryFrom;

#[derive(Debug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive, Clone)]
#[repr(u16)]
pub enum ProtocolError {
    CannotSend = 1,
    WriteError,
    ActorError,
    InvalidState,
    SignatureError,
    InvalidSignature,
    SerializationError,
    PartialContent,
    AccessDenied,
    OverlayNotJoined,
}

impl From<LofireError> for ProtocolError {
    fn from(e: LofireError) -> Self {
        match e {
            LofireError::InvalidSignature => ProtocolError::InvalidSignature,
            LofireError::SerializationError => ProtocolError::SerializationError,
        }
    }
}

impl From<serde_bare::error::Error> for ProtocolError {
    fn from(e: serde_bare::error::Error) -> Self {
        ProtocolError::SerializationError
    }
}

impl From<BrokerMessage> for Result<(), ProtocolError> {
    fn from(msg: BrokerMessage) -> Self {
        if !msg.is_response() {
            panic!("BrokerMessage is not a response");
        }
        match msg.result() {
            0 => Ok(()),
            err => Err(ProtocolError::try_from(err).unwrap()),
        }
    }
}

/// Option represents if a Block is available. canot be returned here. call BrokerMessage.response_block() to get a reference to it.
impl From<BrokerMessage> for Result<Option<u16>, ProtocolError> {
    fn from(msg: BrokerMessage) -> Self {
        if !msg.is_response() {
            panic!("BrokerMessage is not a response");
        }
        let partial: u16 = ProtocolError::PartialContent.into();
        let res = msg.result();
        if res == 0 || res == partial {
            if msg.is_overlay() {
                match msg.response_block() {
                    Some(_) => Ok(Some(res)),
                    None => Ok(None),
                }
            } else {
                Ok(None)
            }
        } else {
            Err(ProtocolError::try_from(res).unwrap())
        }
    }
}
