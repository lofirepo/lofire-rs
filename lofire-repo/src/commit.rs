//! Commit

use ed25519_dalek::{Signature as DalekSig, *};

use crate::store::*;
use crate::tree::*;
use crate::types::*;
use lofire::types::{Signature as Sig, *};

#[derive(Debug)]
pub enum CommitVerifyError {
    InvalidSignature,
    MissingObjects(Vec<ObjectId>),
    BodyLoadError,
    BodyDeserializeError,
    PermissionDenied,
}
impl CommitBody {
    /// Get CommitType corresponding to CommitBody
    pub fn to_type(&self) -> CommitType {
        match self {
            CommitBody::Ack(_) => CommitType::Ack,
            CommitBody::AddBranch(_) => CommitType::AddBranch,
            CommitBody::AddMembers(_) => CommitType::AddMembers,
            CommitBody::Branch(_) => CommitType::Branch,
            CommitBody::EndOfBranch(_) => CommitType::EndOfBranch,
            CommitBody::RemoveBranch(_) => CommitType::RemoveBranch,
            CommitBody::Repository(_) => CommitType::Repository,
            CommitBody::Snapshot(_) => CommitType::Snapshot,
            CommitBody::Transaction(_) => CommitType::Transaction,
        }
    }
}

impl Commit {
    /// New commit
    pub fn new(
        author_privkey: &PrivKey,
        author_pubkey: &PubKey,
        seq: u32,
        branch: &ObjectRef,
        deps: &Vec<ObjectRef>,
        acks: &Vec<ObjectRef>,
        refs: &Vec<ObjectRef>,
        metadata: &Vec<u8>,
        body: &ObjectRef,
        expiry: Option<Timestamp>,
    ) -> Result<Commit, SignatureError> {
        let content = CommitContentV0 {
            author: author_pubkey.clone(),
            seq,
            branch: branch.clone(),
            deps: deps.clone(),
            acks: acks.clone(),
            refs: refs.clone(),
            metadata: metadata.clone(),
            body: body.clone(),
            expiry,
        };
        let content_ser = serde_bare::to_vec(&content).unwrap();

        // sign commit
        let kp = match (author_privkey, author_pubkey) {
            (PrivKey::Ed25519PrivKey(sk), PubKey::Ed25519PubKey(pk)) => [*sk, *pk].concat(),
        };
        let keypair = Keypair::from_bytes(kp.as_slice())?;
        let sig_bytes = keypair.sign(content_ser.as_slice()).to_bytes();
        let mut it = sig_bytes.chunks_exact(32);
        let mut ss: Ed25519Sig = [[0; 32], [0; 32]];
        ss[0].copy_from_slice(it.next().unwrap());
        ss[1].copy_from_slice(it.next().unwrap());
        let sig = Sig::Ed25519Sig(ss);
        Ok(Commit::V0(CommitV0 { content, sig }))
    }

    /// Get commit signature
    pub fn sig(&self) -> &Sig {
        match self {
            Commit::V0(c) => &c.sig,
        }
    }

    /// Get commit content
    pub fn content(&self) -> &CommitContentV0 {
        match self {
            Commit::V0(c) => &c.content,
        }
    }

    /// Verify commit signature
    pub fn verify_sig(&self) -> Result<(), SignatureError> {
        let c = match self {
            Commit::V0(c) => c,
        };
        let content_ser = serde_bare::to_vec(&c.content).unwrap();
        let pubkey = match c.content.author {
            PubKey::Ed25519PubKey(pk) => pk,
        };
        let pk = PublicKey::from_bytes(&pubkey)?;
        let sig_bytes = match c.sig {
            Sig::Ed25519Sig(ss) => [ss[0], ss[1]].concat(),
        };
        let sig = DalekSig::from_bytes(&sig_bytes)?;
        pk.verify_strict(&content_ser, &sig)
    }

    /// Verify commit permissions
    pub fn verify_perm(&self, body: &CommitBody, branch: &Branch) -> Result<(), CommitVerifyError> {
        let content = self.content();
        match branch.get_member(&content.author) {
            Some(m) => {
                if m.has_perm(body.to_type()) {
                    return Ok(());
                }
            }
            None => (),
        }
        Err(CommitVerifyError::PermissionDenied)
    }

    /// Verify if the commit dependencies (`deps` and `acks`) are available in the store
    pub fn verify_deps(&self, store: &Store) -> Result<(), CommitVerifyError> {
        let content = self.content();
        let mut missing: Vec<ObjectId> = vec![];
        for obj_ref in [
            vec![content.body],
            content.acks.clone(),
            content.deps.clone(),
        ]
        .concat()
        {
            match Tree::load(store, obj_ref.id, Some(obj_ref.key)) {
                Ok(_) => (),
                Err(m) => missing.extend(m.iter()),
            }
        }
        if !missing.is_empty() {
            return Err(CommitVerifyError::MissingObjects(missing)); // TODO dedup
        }
        Ok(())
    }

    /// Verify signature, permissions, and dependencies
    pub fn verify(&self, branch: &Branch, store: &Store) -> Result<(), CommitVerifyError> {
        self.verify_sig()
            .map_err(|_e| CommitVerifyError::InvalidSignature)?;
        self.verify_deps(store)?;
        let body = self.body(store)?;
        self.verify_perm(&body, branch)?;
        Ok(())
    }

    /// Get commit body object from store
    pub fn body(&self, store: &Store) -> Result<CommitBody, CommitVerifyError> {
        let content = self.content();
        let tree = Tree::load(
            store,
            content.body.id.clone(),
            Some(content.body.key.clone()),
        )
        .map_err(|missing| CommitVerifyError::MissingObjects(missing))?;
        let body_ser = tree
            .content()
            .map_err(|_e| CommitVerifyError::BodyLoadError)?;
        serde_bare::from_slice(body_ser.as_slice())
            .map_err(|_e| CommitVerifyError::BodyDeserializeError)
    }
}

mod test {
    use std::collections::HashMap;

    use ed25519_dalek::*;
    use rand::rngs::OsRng;
    use rkv::EncodableKey;

    use crate::branch::*;
    use crate::commit::*;
    use crate::store::*;
    use crate::types::*;
    use lofire::types::*;

    #[test]
    pub fn test_commit() {
        let mut csprng = OsRng {};
        let keypair: Keypair = Keypair::generate(&mut csprng);
        println!(
            "private key: ({}) {:?}",
            keypair.secret.as_bytes().len(),
            keypair.secret.as_bytes()
        );
        println!(
            "public key: ({}) {:?}",
            keypair.public.as_bytes().len(),
            keypair.public.as_bytes()
        );
        let ed_priv_key = keypair.secret.to_bytes();
        let ed_pub_key = keypair.public.to_bytes();
        let priv_key = PrivKey::Ed25519PrivKey(ed_priv_key);
        let pub_key = PubKey::Ed25519PubKey(ed_pub_key);
        let seq = 3;
        let obj_ref = ObjectRef {
            id: ObjectId::Blake3Digest32([1; 32]),
            key: SymKey::ChaCha20Key([2; 32]),
        };
        let obj_refs = vec![obj_ref];
        let branch = obj_ref.clone();
        let deps = obj_refs.clone();
        let acks = obj_refs.clone();
        let refs = obj_refs.clone();
        let metadata = vec![1, 2, 3];
        let body_ref = obj_ref.clone();
        let expiry = Some(2342);

        let commit = Commit::new(
            &priv_key, &pub_key, seq, &branch, &deps, &acks, &refs, &metadata, &body_ref, expiry,
        )
        .unwrap();
        println!("commit: {:?}", commit);

        let root = tempfile::Builder::new()
            .prefix("test-tree")
            .tempdir()
            .unwrap();
        let key: [u8; 32] = [0; 32];
        std::fs::create_dir_all(root.path()).unwrap();
        println!("{}", root.path().to_str().unwrap());
        let store = Store::open(root.path(), key);

        let metadata = [66u8; 64].to_vec();
        let commit_types = vec![CommitType::Ack, CommitType::Transaction];
        let secret = SymKey::ChaCha20Key(key);
        let member = MemberV0::new(&pub_key, &commit_types, &metadata);
        let members = vec![member];
        let mut quorum = HashMap::new();
        quorum.insert(CommitType::Transaction, 3);
        let ack_delay = RelTime::Minutes(3);
        let tags = [99u8; 32].to_vec();
        let branch = Branch::new(
            &pub_key, &pub_key, &secret, &members, &quorum, ack_delay, &tags, &metadata,
        );
        println!("branch: {:?}", branch);
        let body = CommitBody::Ack(Ack::V0());
        println!("body: {:?}", body);

        commit.verify_sig().expect("Invalid signature");
        commit
            .verify_perm(&body, &branch)
            .expect("Permission denied");

        match commit.verify(&branch, &store) {
            Ok(_) => panic!("Commit should not be Ok"),
            Err(CommitVerifyError::MissingObjects(missing)) => {
                assert_eq!(missing.len(), 3);
            }
            Err(e) => panic!("Commit verify error: {:?}", e),
        }
    }
}
