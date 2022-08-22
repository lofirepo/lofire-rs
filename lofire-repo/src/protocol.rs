//! LoFiRe protocol
//!
//! Corresponds to the BARE schema

use serde::{Deserialize, Serialize};

//
// COMMON DATA TYPES
//

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

/// Object reference
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct ObjectRef {
    /// Object ID
    pub id: ObjectId,

    /// Key for decrypting the Object
    pub key: SymKey,
}

/// Internal node of a Merkle tree
pub type InternalNode = Vec<SymKey>;

/// Data chunk at a leaf of a Merkle tree
pub type DataChunk = Vec<u8>;

/// Content of an Object
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum ObjectContent {
    /// Internal node with references to children
    InternalNode,

    /// Leaf node with encrypted data chunk
    DataChunk,
}

pub enum DepList {
    DepListV0(Vec<ObjectId>),
}

/// Dependencies of an Object
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ObjectDeps {
    /// List of Object IDs (max. 8),
    ObjectIdList(Vec<ObjectId>),

    /// Reference to an Object that contains an DepList
    DepListRef(ObjectRef),
}

/// Immutable object with an encrypted chunk of data.
/// Data is chunked and stored in a Merkle tree.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Object {
    ObjectV0 {
        /// Objects IDs for child nodes in the Merkle tree
        children: Vec<ObjectId>,

        /// Other objects this object depends on (e.g. Commit deps & acks)
        deps: ObjectDeps,

        /// Expiry time of this object and all of its children
        /// when the object should be deleted by all replicas
        expiry: Option<Timestamp>,

        /// Object content
        /// Encrypted using convergent encryption with ChaCha20:
        /// - convergence_key: BLAKE3 derive_key ("LoFiRe Data BLAKE3 key",
        ///                                        repo_pubkey + repo_secret)
        /// - key: BLAKE3 keyed hash (convergence_key, plain_object)
        /// - nonce: 0
        content: ObjectContent,
    },
}

/// Repository definition
/// Published in root branch, where:
/// - branch_pubkey: repo_pubkey
/// - branch_secret: BLAKE3 derive_key ("LoFiRe Root Branch secret",
///                                     repo_pubkey + repo_secret)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Repository {
    RepositoryV0 {
        /// Repo public key ID
        id: PubKey,

        /// List of branches
        branches: Vec<ObjectRef>,

        /// Whether or not to allow external requests
        allow_ext_requests: bool,

        /// App-specific metadata
        metadata: Vec<u8>,
    },
}

/// Add a branch to the repository
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum AddBranch {
    AddBranchV0 {
        /// Reference to the Branch
        branch: ObjectRef,
    },
}

/// Remove a branch from the repository
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum RemoveBranch {
    RemoveBranchV0 {
        /// Reference to the Branch
        branch: ObjectRef,
    },
}

/// Commit object types
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum CommitType {
    RepositoryCommit,
    AddBranchCommit,
    RemoveBranchCommit,
    BranchCommit,
    AddMembersCommit,
    EndOfBranchCommit,
    TransactionCommit,
    SnapshotCommit,
    AckCommit,
}
