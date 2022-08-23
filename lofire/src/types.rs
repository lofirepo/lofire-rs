//! LoFiRe common data types
//!
//! Corresponds to the BARE schema

use serde::{Deserialize, Serialize};

/// 32-byte Blake3 hash digest
pub type Blake3Digest32 = [u8; 32];

/// Hash digest
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Digest {
    Blake3Digest32(Blake3Digest32),
}

/// ChaCha20 symmetric key
pub type ChaCha20Key = [u8; 32];

/// Symmetric cryptographic key
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum SymKey {
    ChaCha20Key(ChaCha20Key),
}

/// Curve25519 public key
pub type Curve25519PubKey = [u8; 32];

/// Curve25519 private key
pub type Curve25519PrivKey = [u8; 32];

/// Public key
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum PubKey {
    Curve25519PubKey(Curve25519PubKey),
}

/// Private key
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum PrivKey {
    Curve25519PrivKey(Curve25519PrivKey),
}

/// Ed25519 signature
pub type Ed25519Sig = [[u8; 32]; 2];

/// Cryptographic signature
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Signature {
    Ed25519Sig(Ed25519Sig),
}

/// Timestamp: absolute time in minutes since 2022-02-22 22:22 UTC
pub type Timestamp = u32;

/// Relative time (e.g. delay from current time)
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum RelTime {
    Seconds(u8),
    Minutes(u8),
    Hours(u8),
    Days(u8),
}

//
// STORAGE OBJECTS
//

/// Object ID
/// BLAKE3 hash over the serialized Object with encrypted content
pub type ObjectId = Digest;
