//! Merkle hash tree of Objects

use debug_print::*;

use chacha20::cipher::{KeyIvInit, StreamCipher};
use chacha20::ChaCha20;

use crate::store::*;
use crate::types::*;
use lofire::types::*;

/// Size of a serialized empty Object
pub const EMPTY_OBJECT_SIZE: usize = 12;
/// Size of a serialized ObjectId
pub const OBJECT_ID_SIZE: usize = 33;
/// Size of serialized SymKey
pub const OBJECT_KEY_SIZE: usize = 33;
/// Size of serialized Oject with deps reference.
pub const EMPTY_ROOT_SIZE_DEPSREF: usize = 77;
/// Extra size needed if depsRef used instead of deps list.
pub const DEPSREF_OVERLOAD: usize = EMPTY_ROOT_SIZE_DEPSREF - EMPTY_OBJECT_SIZE;
/// Varint extra bytes when reaching the maximum value we will ever use
pub const BIG_VARINT_EXTRA: usize = 3;
/// Varint extra bytes when reaching the maximum size of data byte arrays.
pub const DATA_VARINT_EXTRA: usize = 4;
/// Max extra space used by the deps list
pub const MAX_DEPS_SIZE: usize = 8 * OBJECT_ID_SIZE;

pub struct Tree {
    /// ID of root object
    root_id: ObjectId,

    /// Key for root object
    root_key: Option<SymKey>,

    /// Nodes of the tree
    nodes: Vec<Object>,
}

/// Tree parsing errors
#[derive(Debug)]
pub enum TreeParseError {
    /// Missing root key
    MissingRootKey,
    /// Invalid object ID encountered in the tree
    InvalidObjectId,
    /// Too many or too few children of a node
    InvalidChildren,
    /// Number of keys does not match number of children of a node
    InvalidKeys,
    /// Error deserializing content of a node
    DeserializeError,
}

impl Tree {
    /// Create new Tree from given content
    ///
    /// The arity of the tree is the maximum that fits in the given `max_object_size`
    ///
    /// Arguments:
    /// * `content`: Object content
    /// * `root_deps`: Dependencies for the root object
    /// * `max_object_size`: Max object size used for chunking content
    /// * `repo_pubkey`: Repository public key
    /// * `repo_secret`: Repository secret
    pub fn new(
        content: Vec<u8>,
        root_deps: Vec<ObjectId>,
        expiry: Option<Timestamp>,
        max_object_size: usize,
        repo_pubkey: PubKey,
        repo_secret: SymKey,
    ) -> Tree {
        fn convergence_key(repo_pubkey: PubKey, repo_secret: SymKey) -> [u8; blake3::OUT_LEN] {
            let key_material = match (repo_pubkey, repo_secret) {
                (PubKey::Ed25519PubKey(pubkey), SymKey::ChaCha20Key(secret)) => {
                    [pubkey, secret].concat()
                }
            };
            blake3::derive_key("LoFiRe Data BLAKE3 key", key_material.as_slice())
        }

        fn make_object(
            content: &[u8],
            conv_key: &[u8; blake3::OUT_LEN],
            children: Vec<ObjectId>,
            deps: ObjectDeps,
            expiry: Option<Timestamp>,
        ) -> (Object, SymKey) {
            let key_hash = blake3::keyed_hash(conv_key, content);
            let nonce = [0u8; 12];
            let key = key_hash.as_bytes();
            let mut cipher = ChaCha20::new(key.into(), &nonce.into());
            let mut content_enc = Vec::from(content);
            let mut content_enc_slice = &mut content_enc.as_mut_slice();
            cipher.apply_keystream(&mut content_enc_slice);
            let obj = Object::V0(ObjectV0 {
                children,
                deps,
                expiry,
                content: content_enc,
            });
            let key = SymKey::ChaCha20Key(key.clone());
            debug_println!(">>> make_object:");
            debug_println!("!! id: {:?}", obj.id());
            //debug_println!("!! children: ({}) {:?}", children.len(), children);
            (obj, key)
        }

        fn make_deps(
            deps_vec: Vec<ObjectId>,
            object_size: usize,
            repo_pubkey: PubKey,
            repo_secret: SymKey,
        ) -> ObjectDeps {
            let deps: ObjectDeps;
            if deps_vec.len() <= 8 {
                deps = ObjectDeps::ObjectIdList(deps_vec);
            } else {
                let dep_list = DepList::V0(deps_vec);
                let dep_list_ser = serde_bare::to_vec(&dep_list).unwrap();
                let dep_tree = Tree::new(
                    dep_list_ser,
                    vec![],
                    None,
                    object_size,
                    repo_pubkey,
                    repo_secret,
                );
                let dep_ref = ObjectRef {
                    id: dep_tree.root_id,
                    key: dep_tree.root_key.unwrap(),
                };
                deps = ObjectDeps::DepListRef(dep_ref);
            }
            deps
        }

        /// Build tree from leaves, returns parent nodes
        fn make_tree(
            leaves: &[(Object, SymKey)],
            conv_key: &ChaCha20Key,
            root_deps: ObjectDeps,
            expiry: Option<Timestamp>,
            arity: usize,
        ) -> Vec<(Object, SymKey)> {
            let mut parents = vec![];
            let chunks = leaves.chunks(arity);
            let mut it = chunks.peekable();
            while let Some(nodes) = it.next() {
                let keys = nodes.iter().map(|(_obj, key)| key.clone()).collect();
                let children = nodes.iter().map(|(obj, _key)| obj.id()).collect();
                let content = ObjectContentV0::InternalNode(keys);
                let content_ser = serde_bare::to_vec(&content).unwrap();
                let child_deps = ObjectDeps::ObjectIdList(vec![]);
                let deps = if parents.is_empty() && it.peek().is_none() {
                    root_deps.clone()
                } else {
                    child_deps
                };
                parents.push(make_object(
                    content_ser.as_slice(),
                    conv_key,
                    children,
                    deps,
                    expiry,
                ));
            }
            debug_println!("parents += {}", parents.len());

            if 1 < parents.len() {
                let mut great_parents = make_tree(
                    parents.as_slice(),
                    conv_key,
                    root_deps.clone(),
                    expiry,
                    arity,
                );
                parents.append(&mut great_parents);
            }
            parents
        }

        // create Objects by chunking + encrypting content
        let object_size = Store::get_valid_value_size(max_object_size);
        let data_chunk_size = object_size - EMPTY_OBJECT_SIZE - DATA_VARINT_EXTRA;

        let mut nodes: Vec<(Object, SymKey)> = vec![];
        let conv_key = convergence_key(repo_pubkey, repo_secret);

        let deps = make_deps(root_deps.clone(), object_size, repo_pubkey, repo_secret);

        if EMPTY_OBJECT_SIZE + DATA_VARINT_EXTRA + OBJECT_ID_SIZE * root_deps.len() + content.len()
            <= object_size
        {
            // content fits in root node
            let data_chunk = ObjectContentV0::DataChunk(content.clone());
            let content_ser = serde_bare::to_vec(&data_chunk).unwrap();
            nodes.push(make_object(
                content_ser.as_slice(),
                &conv_key,
                vec![],
                ObjectDeps::ObjectIdList(vec![]),
                expiry,
            ));
        } else {
            // leaf nodes
            for chunk in content.chunks(data_chunk_size) {
                let data_chunk = ObjectContentV0::DataChunk(chunk.to_vec());
                let content_ser = serde_bare::to_vec(&data_chunk).unwrap();
                nodes.push(make_object(
                    content_ser.as_slice(),
                    &conv_key,
                    vec![],
                    ObjectDeps::ObjectIdList(vec![]),
                    expiry,
                ));
            }

            // internal nodes
            // arity: max number of ObjectRefs that fit inside an InternalNode Object within the object_size limit
            let arity: usize =
                (object_size - EMPTY_OBJECT_SIZE - BIG_VARINT_EXTRA * 2 - MAX_DEPS_SIZE)
                    / (OBJECT_ID_SIZE + OBJECT_KEY_SIZE);
            let mut parents = make_tree(nodes.as_slice(), &conv_key, deps.clone(), expiry, arity);
            nodes.append(&mut parents);
        }
        // root node
        let (root_obj, root_key) = nodes.last().unwrap();
        let root_id = root_obj.id();

        Tree {
            root_id,
            root_key: Some(root_key.clone()),
            nodes: nodes.into_iter().map(|(obj, _key)| obj).collect(),
        }
    }

    /// Load tree from store
    ///
    /// Returns Ok(Tree) or a Err(Vec<ObjectId>) of missing Object IDs
    pub fn load(
        root_id: ObjectId,
        root_key: Option<SymKey>,
        store: &Store,
    ) -> Result<Tree, Vec<ObjectId>> {
        fn load_tree(
            parents: Vec<ObjectId>,
            store: &Store,
            nodes: &mut Vec<Object>,
            missing: &mut Vec<ObjectId>,
        ) {
            let mut children: Vec<ObjectId> = vec![];
            for id in parents {
                match store.get(&id) {
                    Ok(obj) => {
                        nodes.insert(0, obj.clone());
                        match obj {
                            Object::V0(o) => {
                                children.extend(o.children.iter().rev());
                            }
                        }
                    }
                    Err(_) => missing.push(id.clone()),
                }
            }
            if !children.is_empty() {
                load_tree(children, store, nodes, missing);
            }
        }

        let mut nodes: Vec<Object> = vec![];
        let mut missing: Vec<ObjectId> = vec![];

        load_tree(vec![root_id], store, &mut nodes, &mut missing);

        if missing.is_empty() {
            Ok(Tree {
                root_id,
                root_key,
                nodes,
            })
        } else {
            Err(missing)
        }
    }

    /// Save objects of the tree in the store
    pub fn save(&self, store: &Store) {
        for node in &self.nodes {
            store.put(node);
        }
    }

    /// Get the ID of the root object
    pub fn root_id(&self) -> ObjectId {
        self.root_id
    }

    /// Get the key of the root object
    pub fn root_key(&self) -> Option<SymKey> {
        self.root_key
    }

    /// Get an `ObjectRef` for the root object
    pub fn root_ref(&self) -> Option<ObjectRef> {
        if self.root_key.is_some() {
            Some(ObjectRef {
                id: self.root_id,
                key: self.root_key.unwrap(),
            })
        } else {
            None
        }
    }

    pub fn root(&self) -> &Object {
        self.nodes.last().unwrap()
    }

    pub fn nodes(&self) -> &Vec<Object> {
        &self.nodes
    }

    /// Parse tree and return decrypted content assembled from chunks
    pub fn content(&self) -> Result<Vec<u8>, TreeParseError> {
        /// Collect decrypted leaves from the tree
        fn collect_leaves(
            nodes: &Vec<Object>,
            parents: &Vec<(ObjectId, SymKey)>,
            parent_index: usize,
            leaves: &mut Vec<u8>,
        ) -> Result<(), TreeParseError> {
            /*debug_println!(
                ">>> collect_leaves: #{}..{}",
                parent_index,
                parent_index + parents.len() - 1
            );*/
            let mut children: Vec<(ObjectId, SymKey)> = vec![];
            let mut i = parent_index;

            for (id, key) in parents {
                //debug_println!("!!! parent: #{}", i);
                let node = &nodes[i];
                i += 1;

                // verify object ID
                if *id != node.id() {
                    debug_println!("Invalid ObjectId.\nExp: {:?}\nGot: {:?}", *id, node.id());
                    return Err(TreeParseError::InvalidObjectId);
                }

                match node {
                    Object::V0(obj) => {
                        // decrypt content
                        let mut content_dec = obj.content.clone();
                        match key {
                            SymKey::ChaCha20Key(key) => {
                                let nonce = [0u8; 12];
                                let mut cipher = ChaCha20::new(key.into(), &nonce.into());
                                let mut content_dec_slice = &mut content_dec.as_mut_slice();
                                cipher.apply_keystream(&mut content_dec_slice);
                            }
                        }

                        // deserialize content
                        let obj_content: ObjectContentV0;
                        match serde_bare::from_slice(content_dec.as_slice()) {
                            Ok(oc) => obj_content = oc,
                            Err(e) => {
                                debug_println!("Deserialize error: {}", e);
                                return Err(TreeParseError::DeserializeError);
                            }
                        }

                        // parse object content
                        match obj_content {
                            ObjectContentV0::InternalNode(keys) => {
                                if keys.len() != obj.children.len() {
                                    debug_println!(
                                        "Invalid keys length: got {}, expected {}",
                                        keys.len(),
                                        obj.children.len()
                                    );
                                    debug_println!("!!! children: {:?}", obj.children);
                                    debug_println!("!!! keys: {:?}", keys);
                                    return Err(TreeParseError::InvalidKeys);
                                }

                                for (id, key) in obj.children.iter().zip(keys.iter()) {
                                    children.push((id.clone(), key.clone()));
                                }
                            }
                            ObjectContentV0::DataChunk(chunk) => {
                                leaves.extend_from_slice(chunk.as_slice());
                            }
                        }
                    }
                }
            }
            if !children.is_empty() {
                if parent_index < children.len() {
                    return Err(TreeParseError::InvalidChildren);
                }
                match collect_leaves(nodes, &children, parent_index - children.len(), leaves) {
                    Ok(_) => (),
                    Err(e) => return Err(e),
                }
            }
            Ok(())
        }

        if self.root_key.is_none() {
            return Err(TreeParseError::MissingRootKey);
        }

        let mut leaves: Vec<u8> = vec![];
        let parents = vec![(self.root_id, self.root_key.unwrap())];
        match collect_leaves(&self.nodes, &parents, self.nodes.len() - 1, &mut leaves) {
            Ok(_) => Ok(leaves),
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod test {

    use crate::store::*;
    use crate::tree::*;
    use crate::types::*;
    use lofire::types::*;

    // Those constants are calculated with Store::get_max_value_size

    /// Maximum arity of branch containing max number of leaves
    const MAX_ARITY_LEAVES: usize = 31774;
    /// Maximum arity of root branch
    const MAX_ARITY_ROOT: usize = 31770;
    /// Maximum data that can fit in object.content
    const MAX_DATA_PAYLOAD_SIZE: usize = 2097112;

    /// Test tree API
    #[test]
    pub fn test_tree() {
        let c: Vec<u8> = (0..255).collect();
        let content: Vec<u8> = [c.as_slice(); 320].concat();
        let deps: Vec<ObjectId> = vec![Digest::Blake3Digest32([9; 32])];
        let expiry = Some(2u32.pow(31));
        let max_object_size = 0;

        let repo_secret = SymKey::ChaCha20Key([0; 32]);
        let repo_pubkey = PubKey::Ed25519PubKey([1; 32]);

        let tree = Tree::new(
            content.clone(),
            deps,
            expiry,
            max_object_size,
            repo_pubkey,
            repo_secret,
        );

        println!("root_id: {:?}", tree.root_id());
        println!("root_key: {:?}", tree.root_key().unwrap());
        println!("nodes.len: {:?}", tree.nodes().len());
        //println!("nodes: {:?}", tree.nodes());
        let mut i = 0;
        for node in tree.nodes() {
            println!("#{}: {:?}", i, node.id());
            i += 1;
        }

        match tree.content() {
            Ok(cnt) => {
                assert_eq!(content, cnt);
            }
            Err(e) => panic!("Tree parse error: {:?}", e),
        }

        let root = tempfile::Builder::new()
            .prefix("test-tree")
            .tempdir()
            .unwrap();
        let key: [u8; 32] = [0; 32];
        std::fs::create_dir_all(root.path()).unwrap();
        println!("{}", root.path().to_str().unwrap());
        let store = Store::open(root.path(), key);

        tree.save(&store);

        let tree2 = Tree::load(tree.root_id(), tree.root_key(), &store).unwrap();

        println!("nodes2.len: {:?}", tree2.nodes().len());
        //println!("nodes2: {:?}", tree2.nodes());
        let mut i = 0;
        for node in tree2.nodes() {
            println!("#{}: {:?}", i, node.id());
            i += 1;
        }

        match tree2.content() {
            Ok(cnt) => {
                assert_eq!(content, cnt);
            }
            Err(e) => panic!("Tree2 parse error: {:?}", e),
        }
    }

    /// Checks that a content that fits the root node, will not be chunked into children nodes
    #[test]
    pub fn test_depth_1() {
        let deps: Vec<ObjectId> = vec![Digest::Blake3Digest32([9; 32])];

        let size = Store::get_max_value_size()
            - EMPTY_OBJECT_SIZE
            - DATA_VARINT_EXTRA
            - OBJECT_ID_SIZE * deps.len();
        let content: Vec<u8> = vec![99; size];

        let expiry = Some(2u32.pow(31));
        let max_object_size = Store::get_max_value_size();

        let repo_secret = SymKey::ChaCha20Key([0; 32]);
        let repo_pubkey = PubKey::Ed25519PubKey([1; 32]);

        let tree = Tree::new(
            content,
            deps,
            expiry,
            max_object_size,
            repo_pubkey,
            repo_secret,
        );

        println!("root_id: {:?}", tree.root_id());
        println!("root_key: {:?}", tree.root_key().unwrap());
        println!("nodes.len: {:?}", tree.nodes().len());
        //println!("root: {:?}", tree.root());
        //println!("nodes: {:?}", tree.nodes());
        assert_eq!(tree.nodes.len(), 1);
    }

    #[test]
    pub fn test_object_size() {
        let max_object_size = Store::get_max_value_size();
        println!("max_object_size: {}", max_object_size);

        let id = Digest::Blake3Digest32([0u8; 32]);
        let key = SymKey::ChaCha20Key([0u8; 32]);

        let one_key = ObjectContentV0::InternalNode(vec![key]);
        let one_key_ser = serde_bare::to_vec(&one_key).unwrap();

        let two_keys = ObjectContentV0::InternalNode(vec![key, key]);
        let two_keys_ser = serde_bare::to_vec(&two_keys).unwrap();

        let max_keys = ObjectContentV0::InternalNode(vec![key; MAX_ARITY_LEAVES]);
        let max_keys_ser = serde_bare::to_vec(&max_keys).unwrap();

        let data = ObjectContentV0::DataChunk(vec![]);
        let data_ser = serde_bare::to_vec(&data).unwrap();

        let data_full = ObjectContentV0::DataChunk(vec![0; MAX_DATA_PAYLOAD_SIZE]);
        let data_full_ser = serde_bare::to_vec(&data_full).unwrap();

        let leaf_empty = Object::V0(ObjectV0 {
            children: vec![],
            deps: ObjectDeps::ObjectIdList(vec![]),
            expiry: Some(2342),
            content: data_ser.clone(),
        });
        let leaf_empty_ser = serde_bare::to_vec(&leaf_empty).unwrap();

        let leaf_full_data = Object::V0(ObjectV0 {
            children: vec![],
            deps: ObjectDeps::ObjectIdList(vec![]),
            expiry: Some(2342),
            content: data_full_ser.clone(),
        });
        let leaf_full_data_ser = serde_bare::to_vec(&leaf_full_data).unwrap();

        let root_depsref = Object::V0(ObjectV0 {
            children: vec![],
            deps: ObjectDeps::DepListRef(ObjectRef { id: id, key: key }),
            expiry: Some(2342),
            content: data_ser.clone(),
        });

        let root_depsref_ser = serde_bare::to_vec(&root_depsref).unwrap();

        let internal_max = Object::V0(ObjectV0 {
            children: vec![id; MAX_ARITY_LEAVES],
            deps: ObjectDeps::ObjectIdList(vec![]),
            expiry: Some(2342),
            content: max_keys_ser.clone(),
        });
        let internal_max_ser = serde_bare::to_vec(&internal_max).unwrap();

        let internal_one = Object::V0(ObjectV0 {
            children: vec![id; 1],
            deps: ObjectDeps::ObjectIdList(vec![]),
            expiry: Some(2342),
            content: one_key_ser.clone(),
        });
        let internal_one_ser = serde_bare::to_vec(&internal_one).unwrap();

        let internal_two = Object::V0(ObjectV0 {
            children: vec![id; 2],
            deps: ObjectDeps::ObjectIdList(vec![]),
            expiry: Some(2342),
            content: two_keys_ser.clone(),
        });
        let internal_two_ser = serde_bare::to_vec(&internal_two).unwrap();

        let root_one = Object::V0(ObjectV0 {
            children: vec![id; 1],
            deps: ObjectDeps::ObjectIdList(vec![id; 8]),
            expiry: Some(2342),
            content: one_key_ser.clone(),
        });
        let root_one_ser = serde_bare::to_vec(&root_one).unwrap();

        let root_two = Object::V0(ObjectV0 {
            children: vec![id; 2],
            deps: ObjectDeps::ObjectIdList(vec![id; 8]),
            expiry: Some(2342),
            content: two_keys_ser.clone(),
        });
        let root_two_ser = serde_bare::to_vec(&root_two).unwrap();

        println!(
            "range of valid value sizes {} {}",
            Store::get_valid_value_size(0),
            Store::get_max_value_size()
        );

        println!(
            "max_data_payload_of_object: {}",
            max_object_size - EMPTY_OBJECT_SIZE - DATA_VARINT_EXTRA
        );

        println!(
            "max_data_payload_depth_1: {}",
            max_object_size - EMPTY_OBJECT_SIZE - DATA_VARINT_EXTRA - MAX_DEPS_SIZE
        );

        println!(
            "max_data_payload_depth_2: {}",
            MAX_ARITY_ROOT * MAX_DATA_PAYLOAD_SIZE
        );

        println!(
            "max_data_payload_depth_3: {}",
            MAX_ARITY_ROOT * MAX_ARITY_LEAVES * MAX_DATA_PAYLOAD_SIZE
        );

        let max_arity_leaves = (max_object_size - EMPTY_OBJECT_SIZE - BIG_VARINT_EXTRA * 2)
            / (OBJECT_ID_SIZE + OBJECT_KEY_SIZE);
        println!("max_arity_leaves: {}", max_arity_leaves);
        assert_eq!(max_arity_leaves, MAX_ARITY_LEAVES);
        assert_eq!(
            max_object_size - EMPTY_OBJECT_SIZE - DATA_VARINT_EXTRA,
            MAX_DATA_PAYLOAD_SIZE
        );
        let max_arity_root =
            (max_object_size - EMPTY_OBJECT_SIZE - MAX_DEPS_SIZE - BIG_VARINT_EXTRA * 2)
                / (OBJECT_ID_SIZE + OBJECT_KEY_SIZE);
        println!("max_arity_root: {}", max_arity_root);
        assert_eq!(max_arity_root, MAX_ARITY_ROOT);
        println!("store_max_value_size: {}", leaf_full_data_ser.len());
        assert_eq!(leaf_full_data_ser.len(), max_object_size);
        println!("leaf_empty: {}", leaf_empty_ser.len());
        assert_eq!(leaf_empty_ser.len(), EMPTY_OBJECT_SIZE);
        println!("root_depsref: {}", root_depsref_ser.len());
        assert_eq!(root_depsref_ser.len(), EMPTY_ROOT_SIZE_DEPSREF);
        println!("internal_max: {}", internal_max_ser.len());
        assert_eq!(
            internal_max_ser.len(),
            EMPTY_OBJECT_SIZE
                + BIG_VARINT_EXTRA * 2
                + MAX_ARITY_LEAVES * (OBJECT_ID_SIZE + OBJECT_KEY_SIZE)
        );
        assert!(internal_max_ser.len() < max_object_size);
        println!("internal_one: {}", internal_one_ser.len());
        assert_eq!(
            internal_one_ser.len(),
            EMPTY_OBJECT_SIZE + 1 * OBJECT_ID_SIZE + 1 * OBJECT_KEY_SIZE
        );
        println!("internal_two: {}", internal_two_ser.len());
        assert_eq!(
            internal_two_ser.len(),
            EMPTY_OBJECT_SIZE + 2 * OBJECT_ID_SIZE + 2 * OBJECT_KEY_SIZE
        );
        println!("root_one: {}", root_one_ser.len());
        assert_eq!(
            root_one_ser.len(),
            EMPTY_OBJECT_SIZE + 8 * OBJECT_ID_SIZE + 1 * OBJECT_ID_SIZE + 1 * OBJECT_KEY_SIZE
        );
        println!("root_two: {}", root_two_ser.len());
        assert_eq!(
            root_two_ser.len(),
            EMPTY_OBJECT_SIZE + 8 * OBJECT_ID_SIZE + 2 * OBJECT_ID_SIZE + 2 * OBJECT_KEY_SIZE
        );

        // let object_size_1 = 4096 * 1 - VALUE_HEADER_SIZE;
        // let object_size_512 = 4096 * MAX_PAGES_PER_VALUE - VALUE_HEADER_SIZE;
        // let arity_1: usize =
        //     (object_size_1 - 8 * OBJECT_ID_SIZE) / (OBJECT_ID_SIZE + OBJECT_KEY_SIZE);
        // let arity_512: usize =
        //     (object_size_512 - 8 * OBJECT_ID_SIZE) / (OBJECT_ID_SIZE + OBJECT_KEY_SIZE);

        // println!("1-page object_size: {}", object_size_1);
        // println!("512-page object_size: {}", object_size_512);
        // println!("max arity of 1-page object: {}", arity_1);
        // println!("max arity of 512-page object: {}", arity_512);
    }
}
