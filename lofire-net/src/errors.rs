use crate::types::BrokerMessage;
use core::fmt;
use lofire::object::ObjectParseError;
use lofire::types::Block;
use lofire::types::ObjectId;
use num_enum::IntoPrimitive;
use num_enum::TryFromPrimitive;
use std::convert::From;
use std::convert::TryFrom;
use std::error::Error;

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
    OverlayNotFound,
    BrokerError,
    NotFound,
    EndOfStream,
    StoreError,
    MissingBlocks,
    ObjectParseError,
    InvalidValue,
    UserAlreadyExists,
    RepoIdRequired,
    Closing,
}

impl ProtocolError {
    pub fn is_stream(&self) -> bool {
        *self == ProtocolError::PartialContent || *self == ProtocolError::EndOfStream
    }
}

impl Error for ProtocolError {}

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl From<lofire::errors::LofireError> for ProtocolError {
    fn from(e: lofire::errors::LofireError) -> Self {
        match e {
            lofire::errors::LofireError::InvalidSignature => ProtocolError::InvalidSignature,
            lofire::errors::LofireError::SerializationError => ProtocolError::SerializationError,
        }
    }
}

impl From<ObjectParseError> for ProtocolError {
    fn from(e: ObjectParseError) -> Self {
        ProtocolError::ObjectParseError
    }
}

impl From<lofire::store::StoreGetError> for ProtocolError {
    fn from(e: lofire::store::StoreGetError) -> Self {
        match e {
            lofire::store::StoreGetError::NotFound => ProtocolError::NotFound,
            lofire::store::StoreGetError::InvalidValue => ProtocolError::InvalidValue,
            _ => ProtocolError::StoreError,
        }
    }
}

impl From<lofire::store::StorePutError> for ProtocolError {
    fn from(e: lofire::store::StorePutError) -> Self {
        match e {
            _ => ProtocolError::StoreError,
        }
    }
}

impl From<lofire::store::StoreDelError> for ProtocolError {
    fn from(e: lofire::store::StoreDelError) -> Self {
        match e {
            lofire::store::StoreDelError::NotFound => ProtocolError::NotFound,
            _ => ProtocolError::StoreError,
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

impl From<BrokerMessage> for Result<ObjectId, ProtocolError> {
    fn from(msg: BrokerMessage) -> Self {
        if !msg.is_response() {
            panic!("BrokerMessage is not a response");
        }
        match msg.result() {
            0 => Ok(msg.response_object_id()),
            err => Err(ProtocolError::try_from(err).unwrap()),
        }
    }
}

/// Option represents if a Block is available. cannot be returned here. call BrokerMessage.response_block() to get a reference to it.
impl From<BrokerMessage> for Result<Option<u16>, ProtocolError> {
    fn from(msg: BrokerMessage) -> Self {
        if !msg.is_response() {
            panic!("BrokerMessage is not a response");
        }
        //let partial: u16 = ProtocolError::PartialContent.into();
        let res = msg.result();
        if res == 0 || ProtocolError::try_from(res).unwrap().is_stream() {
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

/// Option represents if a Block is available. returns a clone.
impl From<BrokerMessage> for Result<Option<Block>, ProtocolError> {
    fn from(msg: BrokerMessage) -> Self {
        if !msg.is_response() {
            panic!("BrokerMessage is not a response");
        }
        //let partial: u16 = ProtocolError::PartialContent.into();
        let res = msg.result();
        if res == 0 || ProtocolError::try_from(res).unwrap().is_stream() {
            if msg.is_overlay() {
                match msg.response_block() {
                    Some(b) => Ok(Some(b.clone())),
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
