//! Overlay

use lofire::store::*;
use lofire::types::*;
use lofire_net::types::*;
use serde::{Deserialize, Serialize};

/// An overlay this node joined
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OverlayV0 {
    /// Overlay ID
    pub id: Digest,

    /// Overlay secret
    pub secret: SymKey,

    /// Public key of repository the overlay belongs to
    pub repo_pubkey: Option<PubKey>,

    /// Secret of repository the overlay belongs to
    pub repo_secret: Option<SymKey>,

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

pub enum OverlayLoadError {
    NotFound,
    StoreError,
    ValueError,
}

pub enum OverlaySaveError {
    StoreError,
    ValueError,
}

impl OverlayV0 {
    pub fn new(
        id: Digest,
        secret: SymKey,
        repo_pubkey: Option<PubKey>,
        repo_secret: Option<SymKey>,
        peers: Vec<PeerAdvert>,
        topics: Vec<TopicId>,
        users: u32,
        last_access: Timestamp,
    ) -> OverlayV0 {
        OverlayV0 {
            id,
            secret,
            repo_pubkey,
            repo_secret,
            peers,
            topics,
            users,
            last_access,
        }
    }
}

impl Overlay {
    pub fn new(
        id: Digest,
        secret: SymKey,
        repo_pubkey: Option<PubKey>,
        repo_secret: Option<SymKey>,
        peers: Vec<PeerAdvert>,
        topics: Vec<TopicId>,
        users: u32,
        last_access: Timestamp,
    ) -> Overlay {
        Overlay::V0(OverlayV0::new(
            id,
            secret,
            repo_pubkey,
            repo_secret,
            peers,
            topics,
            users,
            last_access,
        ))
    }

    pub fn id(&self) -> Digest {
        match self {
            Overlay::V0(o) => o.id,
        }
    }

    pub fn load(id: Digest, store: &impl Store) -> Result<Overlay, OverlayLoadError> {
        let overlay_ser = store.get_overlay(&id).map_err(|e| match e {
            StoreGetError::NotFound => OverlayLoadError::NotFound,
            _ => OverlayLoadError::StoreError,
        })?;
        serde_bare::from_slice::<Overlay>(overlay_ser.as_slice()).map_err(|e| match e {
            _ => OverlayLoadError::ValueError,
        })
    }

    pub fn save(&self, store: &mut impl Store) -> Result<(), OverlaySaveError> {
        let overlay_ser = serde_bare::to_vec(&self).map_err(|_| OverlaySaveError::ValueError)?;
        store
            .put_overlay(self.id(), overlay_ser)
            .map_err(|_| OverlaySaveError::StoreError)
    }
}
