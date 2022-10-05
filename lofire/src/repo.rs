//! Repository

use crate::types::*;

impl RepositoryV0 {
    pub fn new(
        id: &PubKey,
        branches: &Vec<ObjectRef>,
        allow_ext_requests: bool,
        metadata: &Vec<u8>,
    ) -> RepositoryV0 {
        RepositoryV0 {
            id: id.clone(),
            branches: branches.clone(),
            allow_ext_requests,
            metadata: metadata.clone(),
        }
    }
}

impl Repository {
    pub fn new(
        id: &PubKey,
        branches: &Vec<ObjectRef>,
        allow_ext_requests: bool,
        metadata: &Vec<u8>,
    ) -> Repository {
        Repository::V0(RepositoryV0::new(
            id,
            branches,
            allow_ext_requests,
            metadata,
        ))
    }
}
