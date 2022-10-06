//! Topic

use lofire::types::*;
use lofire_net::types::*;
use serde::{Deserialize, Serialize};

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
