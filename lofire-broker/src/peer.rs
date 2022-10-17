//! Peer

use lofire::brokerstore::BrokerStore;
use lofire::store::*;
use lofire::types::*;
use lofire_net::types::*;
use serde::{Deserialize, Serialize};
use serde_bare::{from_slice, to_vec};

pub struct Peer<'a> {
    /// Topic ID
    id: PeerId,
    store: &'a dyn BrokerStore,
}

impl<'a> Peer<'a> {
    const PREFIX: u8 = b"p"[0];

    // propertie's suffixes
    const VERSION: u8 = b"v"[0];
    const ADVERT: u8 = b"a"[0];

    const ALL_PROPERTIES: [u8; 2] = [Self::VERSION, Self::ADVERT];

    const SUFFIX_FOR_EXIST_CHECK: u8 = Self::VERSION;

    pub fn open(id: &PeerId, store: &'a dyn BrokerStore) -> Result<Peer<'a>, StoreGetError> {
        let opening = Peer {
            id: id.clone(),
            store,
        };
        if !opening.exists() {
            return Err(StoreGetError::NotFound);
        }
        Ok(opening)
    }
    pub fn update_or_create(
        advert: &PeerAdvert,
        store: &'a dyn BrokerStore,
    ) -> Result<Peer<'a>, StorePutError> {
        let id = advert.peer();
        match Self::open(id, store) {
            Err(e) => {
                if e == StoreGetError::NotFound {
                    Self::create(advert, store)
                } else {
                    Err(StorePutError::BackendError)
                }
            }
            Ok(p) => {
                p.update_advert(advert)?;
                Ok(p)
            }
        }
    }
    pub fn create(
        advert: &PeerAdvert,
        store: &'a dyn BrokerStore,
    ) -> Result<Peer<'a>, StorePutError> {
        let id = advert.peer();
        let acc = Peer {
            id: id.clone(),
            store,
        };
        if acc.exists() {
            return Err(StorePutError::BackendError);
        }
        store.put(
            Self::PREFIX,
            &to_vec(&id)?,
            Some(Self::VERSION),
            to_vec(&advert.version())?,
        )?;
        store.put(
            Self::PREFIX,
            &to_vec(&id)?,
            Some(Self::ADVERT),
            to_vec(&advert)?,
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
    pub fn id(&self) -> PeerId {
        self.id
    }
    pub fn version(&self) -> Result<u32, StoreGetError> {
        match self
            .store
            .get(Self::PREFIX, &to_vec(&self.id)?, Some(Self::VERSION))
        {
            Ok(ver) => Ok(from_slice::<u32>(&ver)?),
            Err(e) => Err(e),
        }
    }
    pub fn set_version(&self, version: u32) -> Result<(), StorePutError> {
        if !self.exists() {
            return Err(StorePutError::BackendError);
        }
        self.store.replace(
            Self::PREFIX,
            &to_vec(&self.id)?,
            Some(Self::VERSION),
            to_vec(&version)?,
        )
    }
    pub fn update_advert(&self, advert: &PeerAdvert) -> Result<(), StorePutError> {
        if advert.peer() != &self.id {
            return Err(StorePutError::InvalidValue);
        }
        let current_advert = self.advert().map_err(|e| StorePutError::BackendError)?;
        if current_advert.version() >= advert.version() {
            return Ok(());
        }
        self.store.replace(
            Self::PREFIX,
            &to_vec(&self.id)?,
            Some(Self::ADVERT),
            to_vec(advert)?,
        )
    }
    pub fn advert(&self) -> Result<PeerAdvert, StoreGetError> {
        match self
            .store
            .get(Self::PREFIX, &to_vec(&self.id)?, Some(Self::ADVERT))
        {
            Ok(advert) => Ok(from_slice::<PeerAdvert>(&advert)?),
            Err(e) => Err(e),
        }
    }
    pub fn set_advert(&self, advert: &PeerAdvert) -> Result<(), StorePutError> {
        if !self.exists() {
            return Err(StorePutError::BackendError);
        }
        self.store.replace(
            Self::PREFIX,
            &to_vec(&self.id)?,
            Some(Self::ADVERT),
            to_vec(advert)?,
        )
    }

    pub fn del(&self) -> Result<(), StoreDelError> {
        self.store
            .del_all(Self::PREFIX, &to_vec(&self.id)?, &Self::ALL_PROPERTIES)
    }
}
