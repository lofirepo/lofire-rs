//! LoFiRe broker protocol types
//!
//! Corresponds to the BARE schema

use lofire::types::*;
use lofire_p2p::types::*;
use serde::{Deserialize, Serialize};

//
// BROKER PROTOCOL
//

/// Server hello sent upon a client connection
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServerHelloV0 {
    /// Nonce for ClientAuth
    #[serde(with = "serde_bytes")]
    pub nonce: Vec<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ServerHello {
    V0(ServerHelloV0),
}

/// Content of ClientAuthV0
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClientAuthContentV0 {
    /// User pub key
    pub user: PubKey,

    /// Device pub key
    pub device: PubKey,

    /// Nonce from ServerHello
    #[serde(with = "serde_bytes")]
    pub nonce: Vec<u8>,
}

/// Client authentication
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClientAuthV0 {
    /// Authentication data
    pub content: ClientAuthContentV0,

    /// Signature by device key
    pub sig: Sig,
}

/// Client authentication
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ClientAuth {
    V0(ClientAuthV0),
}

/// Content of AddUserV0
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct AddUserContentV0 {
    /// User pub key
    pub user: PubKey,
}

/// Add user account
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct AddUserV0 {
    pub content: AddUserContentV0,

    /// Signature by admin key
    pub sig: Sig,
}

/// Add user account
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum AddUser {
    V0(AddUserV0),
}
/// Content of DelUserV0
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct DelUserContentV0 {
    /// User pub key
    pub user: PubKey,
}

/// Delete user account
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct DelUserV0 {
    pub content: DelUserContentV0,

    /// Signature by admin key
    pub sig: Sig,
}

/// Delete user account
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum DelUser {
    V0(DelUserV0),
}

/// Content of AuthorizeDeviceKeyV0
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct AuthorizeDeviceKeyContentV0 {
    /// Device pub key
    pub device: PubKey,
}
/// Authorize device key
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct AuthorizeDeviceKeyV0 {
    pub content: AuthorizeDeviceKeyContentV0,

    /// Signature by user key
    pub sig: Sig,
}

/// Authorize device key
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum AuthorizeDeviceKey {
    V0(AuthorizeDeviceKeyV0),
}

/// Content of RevokeDeviceKeyV0
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct RevokeDeviceKeyContentV0 {
    /// Device pub key
    pub device: PubKey,
}

/// Revoke device key
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct RevokeDeviceKeyV0 {
    pub content: RevokeDeviceKeyContentV0,

    /// Signature by user key
    pub sig: Sig,
}

/// Revoke device key
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum RevokeDeviceKey {
    V0(RevokeDeviceKeyV0),
}

/// Request to join an overlay
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OverlayJoinV0 {
    /// Overlay secret
    pub secret: SymKey,

    /// Peers to connect to
    pub peers: Vec<PeerAdvert>,
}

/// Request to join an overlay
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum OverlayJoin {
    V0(OverlayJoinV0),
}

/// Request to leave an overlay
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum OverlayLeave {
    V0(),
}

/// Request an object by ID
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ObjectGetV0 {
    /// List of Object IDs to request
    pub ids: Vec<ObjectId>,

    /// Whether or not to include all children recursively
    pub include_children: bool,

    /// Topic the object is referenced from
    pub topic: Option<PubKey>,
}

/// Request an object by ID
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ObjectGet {
    V0(ObjectGetV0),
}

/// Request to store an object
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ObjectPutV0 {
    pub object: Block,
}

/// Request to store an object
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ObjectPut {
    V0(ObjectPutV0),
}

/// Request to pin an object
///
/// Brokers maintain an LRU cache of objects,
/// where old, unused objects might get deleted to free up space for new ones.
/// Pinned objects are retained, regardless of last access.
/// Note that expiry is still observed in case of pinned objects.
/// To make an object survive its expiry,
/// it needs to be copied with a different expiry time.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct ObjectPinV0 {
    pub id: ObjectId,
}

/// Request to pin an object
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum ObjectPin {
    ObjectPinV0,
}

/// Request to unpin an object
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct ObjectUnpinV0 {
    pub id: ObjectId,
}

/// Request to unpin an object
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum ObjectUnpin {
    V0(ObjectUnpinV0),
}

/// Request to copy an object with a different expiry time
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct ObjectCopyV0 {
    /// Object ID to copy
    pub id: ObjectId,

    /// New expiry time
    pub expiry: Option<Timestamp>,
}

/// Request to copy an object with a different expiry time
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum ObjectCopy {
    V0(ObjectCopyV0),
}

/// Request to delete an object
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct ObjectDelV0 {
    pub id: ObjectId,
}

/// Request to delete an object
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum ObjectDel {
    V0(ObjectDelV0),
}

/// Request subscription to a topic
///
/// For publishers a private key also needs to be provided.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct TopicSubV0 {
    /// Topic to subscribe
    pub topic: PubKey,

    /// Topic private key for publishers
    pub key: Option<PrivKey>,
}

/// Request subscription to a topic
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum TopicSub {
    V0(TopicSubV0),
}

/// Request unsubscription from a topic
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct TopicUnsubV0 {
    /// Topic to unsubscribe
    pub topic: PubKey,
}

/// Request unsubscription from a topic
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum TopicUnsub {
    V0(TopicUnsubV0),
}

/// Content of AppRequestV0
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AppRequestContentV0 {
    OverlayJoin(OverlayJoin),
    OverlayLeave(OverlayLeave),
    TopicSub(TopicSub),
    TopicUnsub(TopicUnsub),
    Event(Event),
    ObjectGet(ObjectGet),
    ObjectPut(ObjectPut),
    ObjectPin(ObjectPin),
    ObjectUnpin(ObjectUnpin),
    ObjectCopy(ObjectCopy),
    ObjectDel(ObjectDel),
    BranchHeadsReq(BranchHeadsReq),
    BranchSyncReq(BranchSyncReq),
}
/// Application request
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppRequestV0 {
    /// Request ID
    pub id: u64,

    /// Request content
    pub content: AppRequestContentV0,
}

/// Application request
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AppRequest {
    V0(AppRequestV0),
}

/// Result codes
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Result {
    Ok,
    Error,
}

/// Content of AppResponseV0
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AppResponseContentV0 {
    Block(Block),
}

/// Response to an AppRequest
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppResponseV0 {
    /// Request ID
    pub id: u64,

    /// Result (incl but not limited to Result)
    pub result: u8,
    pub content: Option<AppResponseContentV0>,
}

/// Response to an AppRequest
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AppResponse {
    V0(AppResponseV0),
}

/// Content of AppOverlayMessageV0
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AppOverlayMessageContentV0 {
    AppRequest(AppRequest),
    AppResponse(AppResponse),
    Event(Event),
}
/// Application message for an overlay
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppOverlayMessageV0 {
    pub overlay: OverlayId,
    pub content: AppOverlayMessageContentV0,
}

/// Application message for an overlay
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AppOverlayMessage {
    V0(AppOverlayMessageV0),
}

/// Content of AppMessageV0
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AppMessageContentV0 {
    ServerHello(ServerHello),
    ClientAuth(ClientAuth),
    AddUser(AddUser),
    DelUser(DelUser),
    AuthorizeDeviceKey(AuthorizeDeviceKey),
    RevokeDeviceKey(RevokeDeviceKey),
    AppOverlayMessage(AppOverlayMessage),
}

/// Application message
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppMessageV0 {
    /// Message content
    pub content: AppMessageContentV0,

    /// Optional padding
    #[serde(with = "serde_bytes")]
    pub padding: Vec<u8>,
}

//
// EXTERNAL REQUESTS
//

/// Request object(s) by ID from a repository by non-members
///
/// The request is sent by a non-member to an overlay member node,
/// which has a replica of the repository.
///
/// The response includes the requested objects and all their children recursively,
/// and optionally all object dependencies recursively.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExtObjectGetV0 {
    /// Repository to request the objects from
    pub repo: PubKey,

    /// List of Object IDs to request, including their children
    pub ids: Vec<ObjectId>,

    /// Whether or not to include all children recursively
    pub include_children: bool,

    /// Expiry time after which the link becomes invalid
    pub expiry: Option<Timestamp>,
}

/// Request object(s) by ID from a repository by non-members
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ExtObjectGet {
    V0(ExtObjectGetV0),
}

/// Branch heads request
pub type ExtBranchHeadsReq = BranchHeadsReq;

/// Branch synchronization request
pub type ExtBranchSyncReq = BranchSyncReq;

/// Content of ExtRequestV0
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ExtRequestContentV0 {
    ExtObjectGet(ExtObjectGet),
    ExtBranchHeadsReq(ExtBranchHeadsReq),
    ExtBranchSyncReq(ExtBranchSyncReq),
}

/// External request authenticated by a MAC
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExtRequestV0 {
    /// Request ID
    pub id: u64,

    /// Request content
    pub content: ExtRequestContentV0,

    /// BLAKE3 MAC over content
    /// BLAKE3 keyed hash:
    /// - key: BLAKE3 derive_key ("LoFiRe ExtRequest BLAKE3 key",
    ///                           repo_pubkey + repo_secret)
    pub mac: Digest,
}

/// External request authenticated by a MAC
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ExtRequest {
    V0(ExtRequestV0),
}

/// Content of ExtResponseV0
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ExtResponseContentV0 {
    Block(Block),
    EventResp(EventResp),
    Event(Event),
}

/// Response to an ExtRequest
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExtResponseV0 {
    /// Request ID
    pub id: u64,

    /// Result code
    pub result: u8,

    /// Response content
    pub content: Option<ExtResponseContentV0>,
}

/// Response to an ExtRequest
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ExtResponse {
    V0(ExtResponseV0),
}

//
// DIRECT MESSAGES
//

/// Link/invitation to the repository
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RepoLinkV0 {
    /// Repository public key ID
    pub id: PubKey,

    /// Repository secret
    pub secret: SymKey,

    /// Peers to connect to
    pub peers: Vec<PeerAdvert>,
}

/// Link/invitation to the repository
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum RepoLink {
    V0(RepoLinkV0),
}

/// Owned repository with private key
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RepoKeysV0 {
    /// Repository private key
    pub key: PrivKey,

    /// Repository secret
    pub secret: SymKey,

    /// Peers to connect to
    pub peers: Vec<PeerAdvert>,
}

/// Owned repository with private key
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum RepoKeys {
    V0(RepoKeysV0),
}

/// Link to object(s) or to a branch from a repository
/// that can be shared to non-members
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ObjectLinkV0 {
    /// Request to send to an overlay peer
    pub req: ExtRequest,

    /// Keys for the requested objects
    pub keys: Vec<ObjectRef>,
}

/// Link to object(s) or to a branch from a repository
/// that can be shared to non-members
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ObjectLink {
    V0(ObjectLinkV0),
}

//
// BROKER STORAGE
//

/// A topic this node subscribed to in an overlay
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TopicV0 {
    /// Topic public key ID
    pub id: PubKey,

    /// Topic private key for publishers
    pub priv_key: Option<PrivKey>,

    /// Set of branch heads
    pub heads: Vec<ObjectId>,

    /// Number of local users that subscribed to the topic
    pub users: u32,
}

/// A topic this node subscribed to in an overlay
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Topic {
    V0(TopicV0),
}

/// An overlay this node joined
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OverlayV0 {
    /// Overlay ID
    pub id: OverlayId,

    /// Overlay secret
    pub secret: SymKey,

    /// Known peers with connected flag
    pub peers: Vec<PeerAdvert>,

    /// Topics this node subscribed to in the overlay
    pub topics: Vec<Topic>,

    /// Number of local users that joined the overlay
    pub users: u32,

    /// Last access by any user
    pub last_access: Timestamp,
}

/// An overlay this node joined
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Overlay {
    V0(OverlayV0),
}

/// User account
///
/// Stored as user_pubkey -> Account
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccountV0 {
    /// Authorized device pub keys
    pub authorized_keys: Vec<PubKey>,

    /// Admins can add/remove user accounts
    pub admin: bool,

    /// Overlays joined
    pub overlays: Vec<Overlay>,

    /// Topics joined, with publisher flag
    pub topics: Vec<Topic>,
}

/// User account
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Account {
    V0(AccountV0),
}
