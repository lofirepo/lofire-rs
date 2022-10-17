//! Broker Config, persisted to store

use lofire::brokerstore::BrokerStore;
use lofire::store::*;
use lofire::types::*;
use lofire_net::types::*;
use serde::{Deserialize, Serialize};
use serde_bare::{from_slice, to_vec};

// TODO: versioning V0
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ConfigMode {
    Local,
    Core,
}

pub struct Config<'a> {
    store: &'a dyn BrokerStore,
}

impl<'a> Config<'a> {
    const PREFIX: u8 = b"c"[0];

    const KEY: [u8; 5] = *b"onfig";

    // propertie's suffixes
    const MODE: u8 = b"m"[0];

    const ALL_PROPERTIES: [u8; 1] = [Self::MODE];

    const SUFFIX_FOR_EXIST_CHECK: u8 = Self::MODE;

    pub fn open(store: &'a dyn BrokerStore) -> Result<Config<'a>, StoreGetError> {
        let opening = Config { store };
        if !opening.exists() {
            return Err(StoreGetError::NotFound);
        }
        Ok(opening)
    }
    pub fn get_or_create(
        mode: &ConfigMode,
        store: &'a dyn BrokerStore,
    ) -> Result<Config<'a>, StorePutError> {
        match Self::open(store) {
            Err(e) => {
                if e == StoreGetError::NotFound {
                    Self::create(mode, store)
                } else {
                    Err(StorePutError::BackendError)
                }
            }
            Ok(p) => {
                if &p.mode().unwrap() != mode {
                    return Err(StorePutError::InvalidValue);
                }
                Ok(p)
            }
        }
    }
    pub fn create(
        mode: &ConfigMode,
        store: &'a dyn BrokerStore,
    ) -> Result<Config<'a>, StorePutError> {
        let acc = Config { store };
        if acc.exists() {
            return Err(StorePutError::BackendError);
        }
        store.put(
            Self::PREFIX,
            &to_vec(&Self::KEY)?,
            Some(Self::MODE),
            to_vec(&mode)?,
        )?;
        Ok(acc)
    }
    pub fn exists(&self) -> bool {
        self.store
            .get(
                Self::PREFIX,
                &to_vec(&Self::KEY).unwrap(),
                Some(Self::SUFFIX_FOR_EXIST_CHECK),
            )
            .is_ok()
    }
    pub fn mode(&self) -> Result<ConfigMode, StoreGetError> {
        match self
            .store
            .get(Self::PREFIX, &to_vec(&Self::KEY)?, Some(Self::MODE))
        {
            Ok(ver) => Ok(from_slice::<ConfigMode>(&ver)?),
            Err(e) => Err(e),
        }
    }
}
