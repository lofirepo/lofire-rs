//! LoFiRe common data types
//!
//! Corresponds to the BARE schema

use serde::{Deserialize, Serialize};
use std::hash::Hash;

/// 32-byte Blake3 hash digest
pub type Blake3Digest32 = [u8; 32];

/// Hash digest
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Digest {
    Blake3Digest32(Blake3Digest32),
}

/// ChaCha20 symmetric key
pub type ChaCha20Key = [u8; 32];

/// Symmetric cryptographic key
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum SymKey {
    ChaCha20Key(ChaCha20Key),
}

/// Curve25519 public key
pub type Ed25519PubKey = [u8; 32];

/// Curve25519 private key
pub type Ed25519PrivKey = [u8; 32];

/// Public key
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub enum PubKey {
    Ed25519PubKey(Ed25519PubKey),
}

/// Private key
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub enum PrivKey {
    Ed25519PrivKey(Ed25519PrivKey),
}

/// Ed25519 signature
pub type Ed25519Sig = [[u8; 32]; 2];

/// Cryptographic signature
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub enum Signature {
    Ed25519Sig(Ed25519Sig),
}

/// Timestamp: absolute time in minutes since 2022-02-22 22:22 UTC
pub type Timestamp = u32;

pub const EPOCH_AS_UNIX_TIMESTAMP: u64 = 1645568520;

/// Relative time (e.g. delay from current time)
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum RelTime {
    Seconds(u8),
    Minutes(u8),
    Hours(u8),
    Days(u8),
}

//
// STORAGE OBJECTS
//

/// Object ID:
/// BLAKE3 hash over the serialized Object with encrypted content
pub type ObjectId = Digest;

/// Object reference
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ObjectRef {
    /// Object ID
    pub id: ObjectId,

    /// Key for decrypting the Object
    pub key: SymKey,
}

//
// COMMON DATA TYPES FOR MESSAGES
//

/// Peer ID: public key of node
pub type PeerId = PubKey;

/// IPv4 address
pub type IPv4 = [u8; 4];

/// IPv6 address
pub type IPv6 = [u8; 16];

/// IP address
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum IP {
    IPv4(IPv4),
    IPv6(IPv6),
}

/// IP transport protocol
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum IPTransportProtocol {
    TLS,
    QUIC,
}

/// IP transport address
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct IPTransportAddr {
    pub ip: IP,
    pub port: u16,
    pub protocol: IPTransportProtocol,
}

/// Network address
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum NetAddr {
    IPTransport(IPTransportAddr),
}

/// Bloom filter (variable size)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BloomFilter {
    /// Number of hash functions
    pub k: u32,

    /// Filter
    #[serde(with = "serde_bytes")]
    pub f: Vec<u8>,
}

/// Bloom filter (128 B)
///
/// (m=1024; k=7; p=0.01; n=107)
pub type BloomFilter128 = [[u8; 32]; 4];

/// Bloom filter (1 KiB)
///
/// (m=8192; k=7; p=0.01; n=855)
pub type BloomFilter1K = [[u8; 32]; 32];
