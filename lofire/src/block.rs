//! Immutable Block

use crate::store::*;
use crate::types::*;

impl BlockV0 {
    pub fn new(
        children: Vec<BlockId>,
        deps: ObjectDeps,
        expiry: Option<Timestamp>,
        content: Vec<u8>,
        key: Option<SymKey>,
    ) -> BlockV0 {
        let mut b = BlockV0 {
            id: None,
            key,
            children,
            deps,
            expiry,
            content,
        };
        let block = Block::V0(b.clone());
        b.id = Some(block.get_id());
        b
    }
}

impl Block {
    pub fn new(
        children: Vec<BlockId>,
        deps: ObjectDeps,
        expiry: Option<Timestamp>,
        content: Vec<u8>,
        key: Option<SymKey>,
    ) -> Block {
        Block::V0(BlockV0::new(children, deps, expiry, content, key))
    }

    /// Copy the block with a different expiry
    pub fn copy(
        &self,
        expiry: Option<Timestamp>,
        store: &mut impl Store,
    ) -> Result<BlockId, StoreGetError> {
        store.copy(self.id(), expiry)
    }

    /// Compute the ID
    pub fn get_id(&self) -> BlockId {
        let ser = serde_bare::to_vec(self).unwrap();
        let hash = blake3::hash(ser.as_slice());
        Digest::Blake3Digest32(hash.as_bytes().clone())
    }

    /// Get the already computed ID
    pub fn id(&self) -> BlockId {
        match self {
            Block::V0(b) => match b.id {
                Some(id) => id,
                None => self.get_id(),
            },
        }
    }

    /// Get the content
    pub fn content(&self) -> &Vec<u8> {
        match self {
            Block::V0(b) => &b.content,
        }
    }

    /// Get the children
    pub fn children(&self) -> &Vec<BlockId> {
        match self {
            Block::V0(b) => &b.children,
        }
    }

    /// Get the dependencies
    pub fn deps(&self) -> &ObjectDeps {
        match self {
            Block::V0(b) => &b.deps,
        }
    }

    /// Get the expiry
    pub fn expiry(&self) -> Option<Timestamp> {
        match self {
            Block::V0(b) => b.expiry,
        }
    }

    pub fn set_expiry(&mut self, expiry: Option<Timestamp>) {
        match self {
            Block::V0(b) => {
                b.id = None;
                b.expiry = expiry
            }
        }
    }

    /// Get the key
    pub fn key(&self) -> Option<SymKey> {
        match self {
            Block::V0(b) => b.key,
        }
    }

    /// Set the key
    pub fn set_key(&mut self, key: SymKey) {
        match self {
            Block::V0(b) => b.key = Some(key),
        }
    }
}
