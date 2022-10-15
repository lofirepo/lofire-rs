//! Block store

use crate::types::*;

use std::{cmp::min, collections::HashMap, mem::size_of_val};

pub trait Store {
    /// Load a block from the store.
    fn get(&self, id: &BlockId) -> Result<Block, StoreGetError>;

    /// Save a block to the store.
    fn put(&mut self, block: &Block) -> Result<BlockId, StorePutError>;

    /// Delete a block from the store.
    fn del(&mut self, id: &BlockId) -> Result<(Block, usize), StoreDelError>;

    /// Copy an object with a different expiry time, or no expiry time.
    fn copy(&mut self, id: ObjectId, expiry: Option<Timestamp>) -> Result<ObjectId, StoreGetError>;

    /// Load an account from the store.
    fn get_account(&self, id: &PubKey) -> Result<Vec<u8>, StoreGetError>;

    /// Save an account to the store.
    fn put_account(&mut self, id: PubKey, account: Vec<u8>) -> Result<(), StorePutError>;

    /// Delete an account from the store.
    fn del_account(&mut self, id: &PubKey) -> Result<Vec<u8>, StoreDelError>;

    /// Load an account from the store.
    fn get_overlay(&self, id: &Digest) -> Result<Vec<u8>, StoreGetError>;

    /// Save an account to the store.
    fn put_overlay(&mut self, id: Digest, account: Vec<u8>) -> Result<(), StorePutError>;

    /// Delete an account from the store.
    fn del_overlay(&mut self, id: &Digest) -> Result<Vec<u8>, StoreDelError>;

    /// Load an account from the store.
    fn get_topic(&self, id: &PubKey) -> Result<Vec<u8>, StoreGetError>;

    /// Save an account to the store.
    fn put_topic(&mut self, id: PubKey, account: Vec<u8>) -> Result<(), StorePutError>;

    /// Delete an account from the store.
    fn del_topic(&mut self, id: &PubKey) -> Result<Vec<u8>, StoreDelError>;
}

#[derive(Debug)]
pub enum StoreGetError {
    NotFound,
    InvalidValue,
    BackendError,
}

#[derive(Debug)]
pub enum StorePutError {
    BackendError,
}

#[derive(Debug)]
pub enum StoreDelError {
    NotFound,
    InvalidValue,
    BackendError,
}

const MIN_SIZE: usize = 4072;
const PAGE_SIZE: usize = 4096;
const HEADER: usize = PAGE_SIZE - MIN_SIZE;
const MAX_FACTOR: usize = 512;

/// Returns a valid/optimal value size for the entries of the storage backend.
pub fn store_valid_value_size(size: usize) -> usize {
    min(
        ((size + HEADER) as f32 / PAGE_SIZE as f32).ceil() as usize,
        MAX_FACTOR,
    ) * PAGE_SIZE
        - HEADER
}

/// Returns the maximum value size for the entries of the storage backend.
pub const fn store_max_value_size() -> usize {
    MAX_FACTOR * PAGE_SIZE - HEADER
}

/// Store with a HashMap backend
pub struct HashMapStore {
    blocks: HashMap<BlockId, Block>,
    accounts: HashMap<PubKey, Vec<u8>>,
    overlays: HashMap<Digest, Vec<u8>>,
    topics: HashMap<PubKey, Vec<u8>>,
}

impl HashMapStore {
    pub fn new() -> HashMapStore {
        HashMapStore {
            blocks: HashMap::new(),
            accounts: HashMap::new(),
            overlays: HashMap::new(),
            topics: HashMap::new(),
        }
    }
}

impl Store for HashMapStore {
    fn get(&self, id: &BlockId) -> Result<Block, StoreGetError> {
        match self.blocks.get(id) {
            Some(block) => Ok(block.clone()),
            None => Err(StoreGetError::NotFound),
        }
    }

    fn put(&mut self, block: &Block) -> Result<BlockId, StorePutError> {
        let id = block.id();
        self.blocks.insert(id, block.clone());
        Ok(id)
    }

    fn del(&mut self, id: &BlockId) -> Result<(Block, usize), StoreDelError> {
        let block = self.blocks.remove(id).ok_or(StoreDelError::NotFound)?;
        let size = size_of_val(&block);
        Ok((block, size))
    }

    fn copy(&mut self, id: ObjectId, expiry: Option<Timestamp>) -> Result<ObjectId, StoreGetError> {
        todo!();
    }

    fn get_account(&self, id: &PubKey) -> Result<Vec<u8>, StoreGetError> {
        match self.accounts.get(id) {
            Some(value) => Ok(value.clone()),
            None => Err(StoreGetError::NotFound),
        }
    }

    fn put_account(&mut self, id: PubKey, account: Vec<u8>) -> Result<(), StorePutError> {
        self.accounts.insert(id, account);
        Ok(())
    }

    fn del_account(&mut self, id: &PubKey) -> Result<Vec<u8>, StoreDelError> {
        let value = self.accounts.remove(id).ok_or(StoreDelError::NotFound)?;
        Ok(value)
    }

    fn get_overlay(&self, id: &Digest) -> Result<Vec<u8>, StoreGetError> {
        match self.overlays.get(id) {
            Some(value) => Ok(value.clone()),
            None => Err(StoreGetError::NotFound),
        }
    }

    fn put_overlay(&mut self, id: Digest, overlay: Vec<u8>) -> Result<(), StorePutError> {
        self.overlays.insert(id, overlay);
        Ok(())
    }

    fn del_overlay(&mut self, id: &Digest) -> Result<Vec<u8>, StoreDelError> {
        let value = self.overlays.remove(id).ok_or(StoreDelError::NotFound)?;
        Ok(value)
    }

    fn get_topic(&self, id: &PubKey) -> Result<Vec<u8>, StoreGetError> {
        match self.topics.get(id) {
            Some(value) => Ok(value.clone()),
            None => Err(StoreGetError::NotFound),
        }
    }

    fn put_topic(&mut self, id: PubKey, topic: Vec<u8>) -> Result<(), StorePutError> {
        self.topics.insert(id, topic);
        Ok(())
    }

    fn del_topic(&mut self, id: &PubKey) -> Result<Vec<u8>, StoreDelError> {
        let value = self.topics.remove(id).ok_or(StoreDelError::NotFound)?;
        Ok(value)
    }
}
