//! LoFiRe broker protocol types
//!
//! Corresponds to the BARE schema

use lofire::types::*;
use lofire_p2p::types::*;
use serde::{Deserialize, Serialize};

//
// BROKER PROTOCOL
//

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

/// Content of `AddClientV0`
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct AddClientContentV0 {
    /// Client pub key
    pub client: PubKey,
}
/// Add a client
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct AddClientV0 {
    pub content: AddClientContentV0,

    /// Signature by user key
    pub sig: Sig,
}

/// Add a client
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum AddClient {
    V0(AddClientV0),
}

/// Content of `DelClientV0`
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct DelClientContentV0 {
    /// Client pub key
    pub client: PubKey,
}

/// Remove a client
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct DelClientV0 {
    pub content: DelClientContentV0,

    /// Signature by user key
    pub sig: Sig,
}

/// Remove a client
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum DelClient {
    V0(DelClientV0),
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

/// Request a Block by ID
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockGetV0 {
    /// List of Block IDs to request
    pub ids: Vec<BlockId>,

    /// Whether or not to include all children recursively
    pub include_children: bool,

    /// Topic the object is referenced from
    pub topic: Option<PubKey>,
}

/// Request an object by ID
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum BlockGet {
    V0(BlockGetV0),
}

/// Request to store an object
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum BlockPut {
    V0(Block),
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

    /// Signed `TopicAdvert` for the PeerId of the broker
    pub advert: Option<TopicAdvert>,
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

/// Content of BrokerRequestV0
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum BrokerRequestContentV0 {
    OverlayJoin(OverlayJoin),
    OverlayLeave(OverlayLeave),
    TopicSub(TopicSub),
    TopicUnsub(TopicUnsub),
    Event(Event),
    ObjectGet(BlockGet),
    ObjectPut(BlockPut),
    ObjectPin(ObjectPin),
    ObjectUnpin(ObjectUnpin),
    ObjectCopy(ObjectCopy),
    ObjectDel(ObjectDel),
    BranchHeadsReq(BranchHeadsReq),
    BranchSyncReq(BranchSyncReq),
}
/// Broker request
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BrokerRequestV0 {
    /// Request ID
    pub id: u64,

    /// Request content
    pub content: BrokerRequestContentV0,
}

/// Broker request
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum BrokerRequest {
    V0(BrokerRequestV0),
}

/// Result codes
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Result {
    Ok,
    Error,
}

/// Content of BrokerResponseV0
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum BrokerResponseContentV0 {
    Block(Block),
}

/// Response to an BrokerRequest
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BrokerResponseV0 {
    /// Request ID
    pub id: u64,

    /// Result (including but not limited to Result)
    pub result: u16,

    /// Response content
    pub content: Option<BrokerResponseContentV0>,
}

/// Response to an BrokerRequest
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum BrokerResponse {
    V0(BrokerResponseV0),
}

/// Content of BrokerOverlayMessageV0
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum BrokerOverlayMessageContentV0 {
    BrokerRequest(BrokerRequest),
    BrokerResponse(BrokerResponse),
    Event(Event),
}
/// Broker message for an overlay
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BrokerOverlayMessageV0 {
    pub overlay: OverlayId,
    pub content: BrokerOverlayMessageContentV0,
}

/// Broker message for an overlay
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum BrokerOverlayMessage {
    V0(BrokerOverlayMessageV0),
}

/// Content of BrokerMessageV0
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum BrokerMessageContentV0 {
    ServerHello(ServerHello),
    ClientAuth(ClientAuth),
    AddUser(AddUser),
    DelUser(DelUser),
    AddClient(AddClient),
    DelClient(DelClient),
    BrokerOverlayMessage(BrokerOverlayMessage),
}

/// Broker message
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BrokerMessageV0 {
    /// Message content
    pub content: BrokerMessageContentV0,

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
    pub result: u16,

    /// Response content
    pub content: Option<ExtResponseContentV0>,
}

/// Response to an ExtRequest
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ExtResponse {
    V0(ExtResponseV0),
}

///
/// AUTHENTICATION MESSAGES
///

/// Client Hello
pub type ClientHelloV0 = ();

/// Client Hello
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ClientHello {
    V0(ClientHelloV0),
}

/// Initiate connection - choose broker or ext protocol
/// First message sent by the client
pub enum InitConnection {
    Broker(ClientHello),
    Ext(ExtRequest),
}

/// Server hello sent upon a client connection
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServerHelloV0 {
    /// Nonce for ClientAuth
    #[serde(with = "serde_bytes")]
    pub nonce: Vec<u8>,
}

/// Server hello sent upon a client connection
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ServerHello {
    V0(ServerHelloV0),
}

/// Content of ClientAuthV0
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClientAuthContentV0 {
    /// User pub key
    pub user: PubKey,

    /// Client pub key
    pub client: PubKey,

    /// Nonce from ServerHello
    #[serde(with = "serde_bytes")]
    pub nonce: Vec<u8>,
}

/// Client authentication
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClientAuthV0 {
    /// Authentication data
    pub content: ClientAuthContentV0,

    /// Signature by client key
    pub sig: Sig,
}

/// Client authentication
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ClientAuth {
    V0(ClientAuthV0),
}

/// Authentication result
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthResultV0 {
    pub result: u16,
}

/// Authentication result
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AuthResult {
    V0(AuthResultV0),
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

    /// Keys for the root blocks of the requested objects
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

    /// Signed `TopicAdvert` for publishers
    pub advert: Option<TopicAdvert>,

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
    pub topics: Vec<TopicId>,

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
    /// Authorized client pub keys
    pub authorized_keys: Vec<PubKey>,

    /// Admins can add/remove user accounts
    pub admin: bool,

    /// Overlays joined
    pub overlays: Vec<OverlayId>,

    /// Topics joined, with publisher flag
    pub topics: Vec<TopicId>,
}

/// User account
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Account {
    V0(AccountV0),
}
