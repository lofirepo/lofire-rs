//! Topic

use lofire::brokerstore::BrokerStore;
use lofire::store::*;
use lofire::types::*;
use lofire_net::types::*;
use serde::{Deserialize, Serialize};
use serde_bare::{from_slice, to_vec};

// TODO: versioning V0
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TopicMeta {
    pub users: u32,
}

pub struct Topic<'a> {
    /// Topic ID
    id: TopicId,
    store: &'a dyn BrokerStore,
}

impl<'a> Topic<'a> {
    const PREFIX: u8 = b"t"[0];

    // propertie's suffixes
    const ADVERT: u8 = b"a"[0];
    const HEAD: u8 = b"h"[0];
    const META: u8 = b"m"[0];

    const ALL_PROPERTIES: [u8; 3] = [Self::ADVERT, Self::HEAD, Self::META];

    const SUFFIX_FOR_EXIST_CHECK: u8 = Self::META;

    pub fn open(id: &TopicId, store: &'a dyn BrokerStore) -> Result<Topic<'a>, StorageError> {
        let opening = Topic {
            id: id.clone(),
            store,
        };
        if !opening.exists() {
            return Err(StorageError::NotFound);
        }
        Ok(opening)
    }
    pub fn create(id: &TopicId, store: &'a dyn BrokerStore) -> Result<Topic<'a>, StorageError> {
        let acc = Topic {
            id: id.clone(),
            store,
        };
        if acc.exists() {
            return Err(StorageError::BackendError);
        }
        let meta = TopicMeta { users: 0 };
        store.put(
            Self::PREFIX,
            &to_vec(&id)?,
            Some(Self::META),
            to_vec(&meta)?,
        )?;
        Ok(acc)
    }
    pub fn exists(&self) -> bool {
        self.store
            .get(
                Self::PREFIX,
                &to_vec(&self.id).unwrap(),
                Some(Self::SUFFIX_FOR_EXIST_CHECK),
            )
            .is_ok()
    }
    pub fn id(&self) -> TopicId {
        self.id
    }
    pub fn add_head(&self, head: &ObjectId) -> Result<(), StorageError> {
        if !self.exists() {
            return Err(StorageError::BackendError);
        }
        self.store.put(
            Self::PREFIX,
            &to_vec(&self.id)?,
            Some(Self::HEAD),
            to_vec(head)?,
        )
    }
    pub fn remove_head(&self, head: &ObjectId) -> Result<(), StorageError> {
        self.store.del_property_value(
            Self::PREFIX,
            &to_vec(&self.id)?,
            Some(Self::HEAD),
            to_vec(head)?,
        )
    }

    pub fn has_head(&self, head: &ObjectId) -> Result<(), StorageError> {
        self.store.has_property_value(
            Self::PREFIX,
            &to_vec(&self.id)?,
            Some(Self::HEAD),
            to_vec(head)?,
        )
    }

    pub fn metadata(&self) -> Result<TopicMeta, StorageError> {
        match self
            .store
            .get(Self::PREFIX, &to_vec(&self.id)?, Some(Self::META))
        {
            Ok(meta) => Ok(from_slice::<TopicMeta>(&meta)?),
            Err(e) => Err(e),
        }
    }
    pub fn set_metadata(&self, meta: &TopicMeta) -> Result<(), StorageError> {
        if !self.exists() {
            return Err(StorageError::BackendError);
        }
        self.store.replace(
            Self::PREFIX,
            &to_vec(&self.id)?,
            Some(Self::META),
            to_vec(meta)?,
        )
    }

    pub fn del(&self) -> Result<(), StorageError> {
        self.store
            .del_all(Self::PREFIX, &to_vec(&self.id)?, &Self::ALL_PROPERTIES)
    }
}
