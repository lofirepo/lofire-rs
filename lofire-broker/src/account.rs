//! User account

use lofire::store::*;
use lofire::types::*;
use lofire_net::types::*;
use serde::{Deserialize, Serialize};

pub enum AccountLoadError {
    NotFound,
    StoreError,
    DeserializeError,
}

pub enum AccountSaveError {
    StoreError,
    SerializeError,
}

/// User account
///
/// Stored as user_pubkey -> Account
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccountV0 {
    /// User ID
    pub id: PubKey,

    /// Authorized client pub keys
    pub clients: Vec<PubKey>,

    /// Admins can add/remove user accounts
    pub admin: bool,

    /// Overlays joined
    pub overlays: Vec<PubKey>,

    /// Topics joined, with publisher flag
    pub topics: Vec<PubKey>,
}

/// User account
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Account {
    V0(AccountV0),
}

impl AccountV0 {
    pub fn new(
        id: PubKey,
        clients: Vec<PubKey>,
        admin: bool,
        overlays: Vec<PubKey>,
        topics: Vec<PubKey>,
    ) -> AccountV0 {
        AccountV0 {
            id,
            clients,
            admin,
            overlays,
            topics,
        }
    }
}

impl Account {
    pub fn new(
        id: PubKey,
        clients: Vec<PubKey>,
        admin: bool,
        overlays: Vec<PubKey>,
        topics: Vec<PubKey>,
    ) -> Account {
        Account::V0(AccountV0::new(id, clients, admin, overlays, topics))
    }

    pub fn id(&self) -> PubKey {
        match self {
            Account::V0(a) => a.id,
        }
    }

    pub fn load(id: PubKey, store: &Store) -> Result<Account, AccountLoadError> {
        let account_ser = store.get_account(&id).map_err(|e| match e {
            StoreGetError::NotFound => AccountLoadError::NotFound,
            StoreGetError::StoreError => AccountLoadError::StoreError,
        })?;
        serde_bare::from_slice::<Account>(account_ser.as_slice()).map_err(|e| match e {
            _ => AccountLoadError::DeserializeError,
        })
    }

    pub fn save(&self, store: &Store) -> Result<(), AccountSaveError> {
        let account_ser =
            serde_bare::to_vec(&self).map_err(|_| AccountSaveError::SerializeError)?;
        store
            .put_account(&self.id(), &account_ser)
            .map_err(|_| AccountSaveError::StoreError)
    }
}
