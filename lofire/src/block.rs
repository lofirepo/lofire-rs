//! Immutable Block

use crate::types::*;

impl BlockV0 {
    pub fn new(
        children: Vec<BlockId>,
        deps: ObjectDeps,
        expiry: Option<Timestamp>,
        content: Vec<u8>,
    ) -> BlockV0 {
        BlockV0 {
            children,
            deps,
            expiry,
            content,
        }
    }
}

impl Block {
    pub fn new(
        children: Vec<BlockId>,
        deps: ObjectDeps,
        expiry: Option<Timestamp>,
        content: Vec<u8>,
    ) -> Block {
        Block::V0(BlockV0::new(children, deps, expiry, content))
    }

    /// Get the ID
    pub fn id(&self) -> BlockId {
        let ser = serde_bare::to_vec(self).unwrap();
        let hash = blake3::hash(ser.as_slice());
        Digest::Blake3Digest32(hash.as_bytes().clone())
    }

    /// Get the deps
    pub fn deps(&self) -> ObjectDeps {
        match self {
            Block::V0(o) => o.deps.clone(),
        }
    }

    /// Get the expiry
    pub fn expiry(&self) -> Option<Timestamp> {
        match self {
            Block::V0(o) => o.expiry,
        }
    }
}
