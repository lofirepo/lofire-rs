//! Overlay

use lofire::types::*;
use lofire_net::types::*;
use serde::{Deserialize, Serialize};

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
