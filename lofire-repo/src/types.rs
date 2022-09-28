//! LoFiRe repository data types
//!
//! Corresponds to the BARE schema

use lofire::types::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Object reference
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct ObjectRef {
    /// Object ID
    pub id: ObjectId,

    /// Key for decrypting the Object
    pub key: SymKey,
}

/// Internal node of a Merkle tree
pub type InternalNode = Vec<SymKey>;

/// Content of ObjectV0: a Merkle tree node
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ObjectContentV0 {
    /// Internal node with references to children
    InternalNode(InternalNode),

    #[serde(with = "serde_bytes")]
    DataChunk(Vec<u8>),
}

/// List of ObjectId dependencies as encrypted Object content
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DepList {
    V0(Vec<ObjectId>),
}

/// Dependencies of an Object
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ObjectDeps {
    /// List of Object IDs (max. 8),
    ObjectIdList(Vec<ObjectId>),

    /// Reference to an Object that contains a DepList
    DepListRef(ObjectRef),
}

/// Immutable object with encrypted content
///
/// Data is chunked and stored in a Merkle tree.
/// An Object stores a Merkle tree node.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ObjectV0 {
    /// Objects IDs for child nodes in the Merkle tree
    pub children: Vec<ObjectId>,

    /// Other objects this object depends on (e.g. Commit deps & acks)
    pub deps: ObjectDeps,

    /// Expiry time of this object and all of its children
    /// when the object should be deleted by all replicas
    pub expiry: Option<Timestamp>,

    /// Encrypted ObjectContentV0
    ///
    /// Encrypted using convergent encryption with ChaCha20:
    /// - convergence_key: BLAKE3 derive_key ("LoFiRe Data BLAKE3 key",
    ///                                        repo_pubkey + repo_secret)
    /// - key: BLAKE3 keyed hash (convergence_key, plain_object_content)
    /// - nonce: 0
    #[serde(with = "serde_bytes")]
    pub content: Vec<u8>,
}

/// Immutable object with encrypted content
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Object {
    V0(ObjectV0),
}

/// Repository definition
///
/// Published in root branch, where:
/// - branch_pubkey: repo_pubkey
/// - branch_secret: BLAKE3 derive_key ("LoFiRe Root Branch secret",
///                                     repo_pubkey + repo_secret)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RepositoryV0 {
    /// Repo public key ID
    pub id: PubKey,

    /// List of branches
    pub branches: Vec<ObjectRef>,

    /// Whether or not to allow external requests
    pub allow_ext_requests: bool,

    /// App-specific metadata
    #[serde(with = "serde_bytes")]
    pub metadata: Vec<u8>,
}

/// Repository definition
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Repository {
    V0(RepositoryV0),
}

/// Add a branch to the repository
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum AddBranch {
    V0(ObjectRef),
}

/// Remove a branch from the repository
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum RemoveBranch {
    V0(ObjectRef),
}

/// Commit object types
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum CommitType {
    Repository,
    AddBranch,
    RemoveBranch,
    Branch,
    AddMembers,
    EndOfBranch,
    Transaction,
    Snapshot,
    Ack,
}

/// Member of a Branch
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MemberV0 {
    /// Member public key ID
    pub id: PubKey,

    /// Commit types the member is allowed to publish in the branch
    pub commit_types: Vec<CommitType>,

    /// Application-specific metadata
    /// (role, permissions, cryptographic material, etc)
    #[serde(with = "serde_bytes")]
    pub metadata: Vec<u8>,
}

/// Member of a branch
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Member {
    V0(MemberV0),
}

/// Branch definition
///
/// First commit in a branch, signed by branch key
/// In case of a fork, the commit deps indicat
/// the previous branch heads.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BranchV0 {
    /// Branch public key ID
    pub id: PubKey,

    /// Pub/sub topic for publishing events
    pub topic: PubKey,

    /// Branch secret key
    pub secret: SymKey,

    /// Members with permissions
    pub members: Vec<MemberV0>,

    /// Number of acks required for a commit to be valid
    pub quorum: HashMap<CommitType, u32>,

    /// Delay to send explicit acks,
    /// if not enough implicit acks arrived by then
    pub ack_delay: RelTime,

    /// Tags for organizing branches within the repository
    #[serde(with = "serde_bytes")]
    pub tags: Vec<u8>,

    /// App-specific metadata (validation rules, etc)
    #[serde(with = "serde_bytes")]
    pub metadata: Vec<u8>,
}

/// Branch definition
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Branch {
    V0(BranchV0),
}

/// Add members to an existing branch
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AddMembersV0 {
    /// Members to add, with permissions
    pub members: Vec<MemberV0>,

    /// New quorum
    pub quorum: Option<HashMap<CommitType, u32>>,

    /// New ackDelay
    pub ack_delay: Option<RelTime>,
}

/// Add members to an existing branch
///
/// If a member already exists, it overwrites the previous definition,
/// in that case this can only be used for adding new permissions,
/// not to remove existing ones.
/// The quorum and ackDelay can be changed as well
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AddMembers {
    V0(AddMembersV0),
}

/// ObjectRef for EndOfBranch
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PlainOrEncryptedObjectRef {
    Plain(ObjectRef),
    Encrypted(Vec<u8>),
}

/// End of branch
///
/// No more commits accepted afterwards, only acks of this commit
/// May reference a fork where the branch continues
/// with possibly different members, permissions, validation rules.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EndOfBranchV0 {
    /// (Encrypted) reference to forked branch (optional)
    pub fork: Option<PlainOrEncryptedObjectRef>,

    /// Expiry time when all commits in the branch should be deleted
    pub expiry: Timestamp,
}

/// End of branch
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum EndOfBranch {
    V0(EndOfBranchV0),
}

/// Transaction with CRDT operations
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Transaction {
    #[serde(with = "serde_bytes")]
    V0(Vec<u8>),
}

/// Snapshot of a Branch
///
/// Contains a data structure
/// computed from the commits at the specified head.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SnapshotV0 {
    /// Branch heads the snapshot was made from
    pub heads: Vec<ObjectId>,

    /// Snapshot data structure
    #[serde(with = "serde_bytes")]
    pub content: Vec<u8>,
}

/// Snapshot of a Branch
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Snapshot {
    V0(SnapshotV0),
}

/// Acknowledgement of another Commit
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Ack {
    V0(),
}

/// Commit body, corresponds to CommitType
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CommitBody {
    Repository(Repository),
    AddBranch(AddBranch),
    RemoveBranch(RemoveBranch),
    Branch(Branch),
    AddMembers(AddMembers),
    EndOfBranch(EndOfBranch),
    Transaction(Transaction),
    Snapshot(Snapshot),
    Ack(Ack),
}

/// Content of a Commit
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommitContentV0 {
    /// Commit author
    pub author: PubKey,

    /// Author's commit sequence number in this branch
    pub seq: u32,

    /// Branch the commit belongs to
    pub branch: ObjectRef,

    /// Direct dependencies of this commit
    pub deps: Vec<ObjectRef>,

    /// Not directly dependent heads to acknowledge
    pub acks: Vec<ObjectRef>,

    /// Files the commit references
    pub refs: Vec<ObjectRef>,

    /// App-specific metadata (commit message, creation time, etc)
    #[serde(with = "serde_bytes")]
    pub metadata: Vec<u8>,

    /// Object with a CommitBody inside
    pub body: ObjectRef,

    /// Expiry time of the body object
    pub expiry: Option<Timestamp>,
}

/// Commit object
/// Signed by branch key, or a member key authorized to publish this commit type
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommitV0 {
    /// Commit content
    pub content: CommitContentV0,

    /// Signature over the content by the author
    pub sig: Signature,
}

/// Commit object
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Commit {
    V0(CommitV0),
}

/// A file stored in an Object
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileV0 {
    #[serde(with = "serde_bytes")]
    pub content_type: Vec<u8>,

    #[serde(with = "serde_bytes")]
    pub metadata: Vec<u8>,

    #[serde(with = "serde_bytes")]
    pub content: Vec<u8>,
}

/// A file stored in an Object
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum File {
    V0(FileV0),
}

/// Immutable data stored encrypted in a Merkle tree
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Data {
    Commit(Commit),
    CommitBody(CommitBody),
    File(File),
    DepList(DepList),
}
