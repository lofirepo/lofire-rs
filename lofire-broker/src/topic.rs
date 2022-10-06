//! Topic

use lofire::store::*;
use lofire::types::*;
use lofire_net::types::*;
use serde::{Deserialize, Serialize};

pub enum TopicLoadError {
    NotFound,
    StoreError,
    DeserializeError,
}

pub enum TopicSaveError {
    StoreError,
    SerializeError,
}

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

    /// Last access by any user
    pub last_access: Timestamp,
}

/// A topic this node subscribed to in an overlay
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Topic {
    V0(TopicV0),
}

impl TopicV0 {
    pub fn new(
        id: PubKey,
        advert: Option<TopicAdvert>,
        heads: Vec<ObjectId>,
        users: u32,
        last_access: Timestamp,
    ) -> TopicV0 {
        TopicV0 {
            id,
            advert,
            heads,
            users,
            last_access,
        }
    }
}

impl Topic {
    pub fn new(
        id: PubKey,
        advert: Option<TopicAdvert>,
        heads: Vec<ObjectId>,
        users: u32,
        last_access: Timestamp,
    ) -> Topic {
        Topic::V0(TopicV0::new(id, advert, heads, users, last_access))
    }

    pub fn id(&self) -> PubKey {
        match self {
            Topic::V0(a) => a.id,
        }
    }

    pub fn load(id: PubKey, store: &Store) -> Result<Topic, TopicLoadError> {
        let topic_ser = store.get_account(&id).map_err(|e| match e {
            StoreGetError::NotFound => TopicLoadError::NotFound,
            StoreGetError::StoreError => TopicLoadError::StoreError,
        })?;
        serde_bare::from_slice::<Topic>(topic_ser.as_slice()).map_err(|e| match e {
            _ => TopicLoadError::DeserializeError,
        })
    }

    pub fn save(&self, store: &Store) -> Result<(), TopicSaveError> {
        let topic_ser = serde_bare::to_vec(&self).map_err(|_| TopicSaveError::SerializeError)?;
        store
            .put_account(&self.id(), &topic_ser)
            .map_err(|_| TopicSaveError::StoreError)
    }
}
