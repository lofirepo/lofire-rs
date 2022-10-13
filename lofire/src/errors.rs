pub enum LofireError {
    InvalidSignature,
    SerializationError,
}

impl From<serde_bare::error::Error> for LofireError {
    fn from(e: serde_bare::error::Error) -> Self {
        LofireError::SerializationError
    }
}

impl From<ed25519_dalek::ed25519::Error> for LofireError {
    fn from(e: ed25519_dalek::ed25519::Error) -> Self {
        LofireError::InvalidSignature
    }
}
