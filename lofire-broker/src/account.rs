//! User account

use lofire::types::*;
use lofire_net::types::*;
use serde::{Deserialize, Serialize};

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
