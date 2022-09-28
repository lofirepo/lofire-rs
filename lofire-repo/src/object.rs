//! Immutable Object

use crate::types::*;
use lofire::types::*;

impl ObjectV0 {
    pub fn new(
        children: &Vec<ObjectId>,
        deps: &ObjectDeps,
        expiry: Option<Timestamp>,
        content: &Vec<u8>,
    ) -> ObjectV0 {
        ObjectV0 {
            children: children.clone(),
            deps: deps.clone(),
            expiry: expiry.clone(),
            content: content.clone(),
        }
    }
}

impl Object {
    pub fn new(
        children: &Vec<ObjectId>,
        deps: &ObjectDeps,
        expiry: Option<Timestamp>,
        content: &Vec<u8>,
    ) -> Object {
        Object::V0(ObjectV0::new(children, deps, expiry, content))
    }

    /// Get the ID of an Object
    pub fn id(&self) -> ObjectId {
        let ser = serde_bare::to_vec(self).unwrap();
        let hash = blake3::hash(ser.as_slice());
        Digest::Blake3Digest32(hash.as_bytes().clone())
    }

    pub fn expiry(&self) -> Option<Timestamp> {
        match self {
            Object::V0(o) => o.expiry,
        }
    }
}
