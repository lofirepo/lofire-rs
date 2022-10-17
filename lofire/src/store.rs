//! Block store

use crate::types::*;

use std::{cmp::min, collections::HashMap, mem::size_of_val};

pub trait RepoStore {
    /// Load a block from the store.
    fn get(&self, id: &BlockId) -> Result<Block, StoreGetError>;

    /// Save a block to the store.
    fn put(&mut self, block: &Block) -> Result<BlockId, StorePutError>;

    /// Delete a block from the store.
    fn del(&mut self, id: &BlockId) -> Result<(Block, usize), StoreDelError>;

    /// Copy an object with a different expiry time, or no expiry time.
    fn copy(&mut self, id: ObjectId, expiry: Option<Timestamp>) -> Result<ObjectId, StoreGetError>;
}

#[derive(Debug, PartialEq)]
pub enum StoreGetError {
    NotFound,
    InvalidValue,
    BackendError,
    SerializationError,
}

impl From<serde_bare::error::Error> for StoreGetError {
    fn from(e: serde_bare::error::Error) -> Self {
        StoreGetError::SerializationError
    }
}

#[derive(Debug, PartialEq)]
pub enum StorePutError {
    BackendError,
    SerializationError,
    InvalidValue,
}

impl From<serde_bare::error::Error> for StorePutError {
    fn from(e: serde_bare::error::Error) -> Self {
        StorePutError::SerializationError
    }
}

#[derive(Debug, PartialEq)]
pub enum StoreDelError {
    NotFound,
    InvalidValue,
    BackendError,
    SerializationError,
}

impl From<serde_bare::error::Error> for StoreDelError {
    fn from(e: serde_bare::error::Error) -> Self {
        StoreDelError::SerializationError
    }
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
pub struct HashMapRepoStore {
    blocks: HashMap<BlockId, Block>,
}

impl HashMapRepoStore {
    pub fn new() -> HashMapRepoStore {
        HashMapRepoStore {
            blocks: HashMap::new(),
        }
    }
}

impl RepoStore for HashMapRepoStore {
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

    // fn get_account(&self, id: &PubKey) -> Result<Vec<u8>, StoreGetError> {
    //     match self.accounts.get(id) {
    //         Some(value) => Ok(value.clone()),
    //         None => Err(StoreGetError::NotFound),
    //     }
    // }

    // fn put_account(&mut self, id: PubKey, account: Vec<u8>) -> Result<(), StorePutError> {
    //     self.accounts.insert(id, account);
    //     Ok(())
    // }

    // fn del_account(&mut self, id: &PubKey) -> Result<Vec<u8>, StoreDelError> {
    //     let value = self.accounts.remove(id).ok_or(StoreDelError::NotFound)?;
    //     Ok(value)
    // }

    // fn get_overlay(&self, id: &Digest) -> Result<Vec<u8>, StoreGetError> {
    //     match self.overlays.get(id) {
    //         Some(value) => Ok(value.clone()),
    //         None => Err(StoreGetError::NotFound),
    //     }
    // }

    // fn put_overlay(&mut self, id: Digest, overlay: Vec<u8>) -> Result<(), StorePutError> {
    //     self.overlays.insert(id, overlay);
    //     Ok(())
    // }

    // fn del_overlay(&mut self, id: &Digest) -> Result<Vec<u8>, StoreDelError> {
    //     let value = self.overlays.remove(id).ok_or(StoreDelError::NotFound)?;
    //     Ok(value)
    // }

    // fn get_topic(&self, id: &PubKey) -> Result<Vec<u8>, StoreGetError> {
    //     match self.topics.get(id) {
    //         Some(value) => Ok(value.clone()),
    //         None => Err(StoreGetError::NotFound),
    //     }
    // }

    // fn put_topic(&mut self, id: PubKey, topic: Vec<u8>) -> Result<(), StorePutError> {
    //     self.topics.insert(id, topic);
    //     Ok(())
    // }

    // fn del_topic(&mut self, id: &PubKey) -> Result<Vec<u8>, StoreDelError> {
    //     let value = self.topics.remove(id).ok_or(StoreDelError::NotFound)?;
    //     Ok(value)
    // }
}
