//! RepoStore information about each RepoStore
//! It contains the symKeys to open the RepoStores
//! A repoStore is identified by its repo pubkey if in local mode
//! In core mode, it is identified by the overlayid.

use lofire::brokerstore::BrokerStore;
use lofire::store::*;
use lofire::types::*;
use lofire_net::types::*;
use serde::{Deserialize, Serialize};
use serde_bare::{from_slice, to_vec};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum RepoStoreId {
    Overlay(OverlayId),
    Repo(PubKey),
}

impl From<RepoStoreId> for String {
    fn from(id: RepoStoreId) -> Self {
        hex::encode(to_vec(&id).unwrap())
    }
}

pub struct RepoStoreInfo<'a> {
    /// RepoStore ID
    id: RepoStoreId,
    store: &'a dyn BrokerStore,
}

impl<'a> RepoStoreInfo<'a> {
    const PREFIX: u8 = b"r"[0];

    // propertie's suffixes
    const KEY: u8 = b"k"[0];

    const ALL_PROPERTIES: [u8; 1] = [Self::KEY];

    const SUFFIX_FOR_EXIST_CHECK: u8 = Self::KEY;

    pub fn open(
        id: &RepoStoreId,
        store: &'a dyn BrokerStore,
    ) -> Result<RepoStoreInfo<'a>, StoreGetError> {
        let opening = RepoStoreInfo {
            id: id.clone(),
            store,
        };
        if !opening.exists() {
            return Err(StoreGetError::NotFound);
        }
        Ok(opening)
    }
    pub fn create(
        id: &RepoStoreId,
        key: &SymKey,
        store: &'a dyn BrokerStore,
    ) -> Result<RepoStoreInfo<'a>, StorePutError> {
        let acc = RepoStoreInfo {
            id: id.clone(),
            store,
        };
        if acc.exists() {
            return Err(StorePutError::BackendError);
        }
        store.put(Self::PREFIX, &to_vec(&id)?, Some(Self::KEY), to_vec(key)?)?;
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
    pub fn id(&self) -> &RepoStoreId {
        &self.id
    }
    pub fn key(&self) -> Result<SymKey, StoreGetError> {
        match self
            .store
            .get(Self::PREFIX, &to_vec(&self.id)?, Some(Self::KEY))
        {
            Ok(k) => Ok(from_slice::<SymKey>(&k)?),
            Err(e) => Err(e),
        }
    }
    pub fn del(&self) -> Result<(), StoreDelError> {
        self.store
            .del_all(Self::PREFIX, &to_vec(&self.id)?, &Self::ALL_PROPERTIES)
    }
}
