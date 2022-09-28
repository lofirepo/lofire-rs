//! Branch of a Repository

use crate::types::*;
use lofire::types::*;

use std::collections::HashMap;

impl MemberV0 {
    pub fn new(id: &PubKey, commit_types: &Vec<CommitType>, metadata: &Vec<u8>) -> MemberV0 {
        MemberV0 {
            id: id.clone(),
            commit_types: commit_types.clone(),
            metadata: metadata.clone(),
        }
    }

    /// Check whether this member has permission for the given commit type
    pub fn has_perm(&self, commit_type: CommitType) -> bool {
        self.commit_types.contains(&commit_type)
    }
}

impl Member {
    pub fn new(id: &PubKey, commit_types: &Vec<CommitType>, metadata: &Vec<u8>) -> Member {
        Member::V0(MemberV0::new(id, commit_types, metadata))
    }

    /// Check whether this member has permission for the given commit type
    pub fn has_perm(&self, commit_type: CommitType) -> bool {
        match self {
            Member::V0(m) => m.has_perm(commit_type),
        }
    }
}

impl BranchV0 {
    pub fn new(
        id: &PubKey,
        topic: &PubKey,
        secret: &SymKey,
        members: &Vec<MemberV0>,
        quorum: &HashMap<CommitType, u32>,
        ack_delay: RelTime,
        tags: &Vec<u8>,
        metadata: &Vec<u8>,
    ) -> BranchV0 {
        BranchV0 {
            id: id.clone(),
            topic: topic.clone(),
            secret: secret.clone(),
            members: members.clone(),
            quorum: quorum.clone(),
            ack_delay: ack_delay,
            tags: tags.clone(),
            metadata: metadata.clone(),
        }
    }
}

impl Branch {
    pub fn new(
        id: &PubKey,
        topic: &PubKey,
        secret: &SymKey,
        members: &Vec<MemberV0>,
        quorum: &HashMap<CommitType, u32>,
        ack_delay: RelTime,
        tags: &Vec<u8>,
        metadata: &Vec<u8>,
    ) -> Branch {
        Branch::V0(BranchV0::new(
            id, topic, secret, members, quorum, ack_delay, tags, metadata,
        ))
    }

    /// Get member by ID
    pub fn get_member(&self, id: &PubKey) -> Option<&MemberV0> {
        match self {
            Branch::V0(b) => {
                for m in b.members.iter() {
                    if m.id == *id {
                        return Some(m);
                    }
                }
            }
        }
        None
    }
}
