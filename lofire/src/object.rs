//! Immutable Object

use crate::types::*;

impl ObjectV0 {
    pub fn new(
        children: Vec<ObjectId>,
        deps: ObjectDeps,
        expiry: Option<Timestamp>,
        content: Vec<u8>,
    ) -> ObjectV0 {
        ObjectV0 {
            children,
            deps,
            expiry,
            content,
        }
    }
}

impl Object {
    pub fn new(
        children: Vec<ObjectId>,
        deps: ObjectDeps,
        expiry: Option<Timestamp>,
        content: Vec<u8>,
    ) -> Object {
        Object::V0(ObjectV0::new(children, deps, expiry, content))
    }

    /// Get the ID
    pub fn id(&self) -> ObjectId {
        let ser = serde_bare::to_vec(self).unwrap();
        let hash = blake3::hash(ser.as_slice());
        Digest::Blake3Digest32(hash.as_bytes().clone())
    }

    /// Get the deps
    pub fn deps(&self) -> ObjectDeps {
        match self {
            Object::V0(o) => o.deps.clone(),
        }
    }

    /// Get the expiry
    pub fn expiry(&self) -> Option<Timestamp> {
        match self {
            Object::V0(o) => o.expiry,
        }
    }
}
