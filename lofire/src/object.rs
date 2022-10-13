//! Merkle hash tree of Objects

use std::collections::HashMap;

use debug_print::*;

use chacha20::cipher::{KeyIvInit, StreamCipher};
use chacha20::ChaCha20;

use crate::store::*;
use crate::types::*;

/// Size of a serialized empty Block
const EMPTY_BLOCK_SIZE: usize = 12;
/// Size of a serialized BlockId
const BLOCK_ID_SIZE: usize = 33;
/// Size of serialized SymKey
const BLOCK_KEY_SIZE: usize = 33;
/// Size of serialized Object with deps reference.
const EMPTY_ROOT_SIZE_DEPSREF: usize = 77;
/// Extra size needed if depsRef used instead of deps list.
const DEPSREF_OVERLOAD: usize = EMPTY_ROOT_SIZE_DEPSREF - EMPTY_BLOCK_SIZE;
/// Varint extra bytes when reaching the maximum value we will ever use
const BIG_VARINT_EXTRA: usize = 3;
/// Varint extra bytes when reaching the maximum size of data byte arrays.
const DATA_VARINT_EXTRA: usize = 4;
/// Max extra space used by the deps list
const MAX_DEPS_SIZE: usize = 8 * BLOCK_ID_SIZE;

pub struct Object {
    /// ID of root block
    id: ObjectId,

    /// Key for root block
    key: Option<SymKey>,

    /// Blocks of the Object (nodes of the tree)
    blocks: Vec<Block>,
}

/// Object parsing errors
#[derive(Debug)]
pub enum ObjectParseError {
    /// Missing root key
    MissingRootKey,
    /// Invalid BlockId encountered in the tree
    InvalidBlockId,
    /// Too many or too few children of a block
    InvalidChildren,
    /// Number of keys does not match number of children of a block
    InvalidKeys,
    /// Error deserializing content of a block
    BlockDeserializeError,
    /// Error deserializing content of the object
    ObjectDeserializeError,
}

impl Object {
    /// Create new Object from given content
    ///
    /// The Object is chunked and stored in a Merkle tree
    /// The arity of the Merkle tree is the maximum that fits in the given `max_object_size`
    ///
    /// Arguments:
    /// * `content`: Object content
    /// * `deps`: Dependencies of the object
    /// * `max_object_size`: Max object size used for chunking content
    /// * `repo_pubkey`: Repository public key
    /// * `repo_secret`: Repository secret
    pub fn new(
        content: ObjectContent,
        deps: Vec<ObjectId>,
        expiry: Option<Timestamp>,
        max_object_size: usize,
        repo_pubkey: PubKey,
        repo_secret: SymKey,
    ) -> Object {
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
        ) -> (Block, SymKey) {
            let key_hash = blake3::keyed_hash(conv_key, content);
            let nonce = [0u8; 12];
            let key = key_hash.as_bytes();
            let mut cipher = ChaCha20::new(key.into(), &nonce.into());
            let mut content_enc = Vec::from(content);
            let mut content_enc_slice = &mut content_enc.as_mut_slice();
            cipher.apply_keystream(&mut content_enc_slice);
            let obj = Block::V0(BlockV0 {
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
                let dep_obj = Object::new(
                    ObjectContent::DepList(dep_list),
                    vec![],
                    None,
                    object_size,
                    repo_pubkey,
                    repo_secret,
                );
                let dep_ref = ObjectRef {
                    id: dep_obj.id,
                    key: dep_obj.key.unwrap(),
                };
                deps = ObjectDeps::DepListRef(dep_ref);
            }
            deps
        }

        /// Build tree from leaves, returns parent nodes
        fn make_tree(
            leaves: &[(Block, SymKey)],
            conv_key: &ChaCha20Key,
            root_deps: ObjectDeps,
            expiry: Option<Timestamp>,
            arity: usize,
        ) -> Vec<(Block, SymKey)> {
            let mut parents = vec![];
            let chunks = leaves.chunks(arity);
            let mut it = chunks.peekable();
            while let Some(nodes) = it.next() {
                let keys = nodes.iter().map(|(_obj, key)| key.clone()).collect();
                let children = nodes.iter().map(|(obj, _key)| obj.id()).collect();
                let content = BlockContentV0::InternalNode(keys);
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

        // create blocks by chunking + encrypting content
        let block_size = store_valid_value_size(max_object_size);
        let data_chunk_size = block_size - EMPTY_BLOCK_SIZE - DATA_VARINT_EXTRA;

        let mut blocks: Vec<(Block, SymKey)> = vec![];
        let conv_key = convergence_key(repo_pubkey, repo_secret);

        let obj_deps = make_deps(deps.clone(), block_size, repo_pubkey, repo_secret);

        let content_ser = serde_bare::to_vec(&content).unwrap();

        if EMPTY_BLOCK_SIZE + DATA_VARINT_EXTRA + BLOCK_ID_SIZE * deps.len() + content_ser.len()
            <= block_size
        {
            // content fits in root node
            let data_chunk = BlockContentV0::DataChunk(content_ser.clone());
            let content_ser = serde_bare::to_vec(&data_chunk).unwrap();
            blocks.push(make_object(
                content_ser.as_slice(),
                &conv_key,
                vec![],
                ObjectDeps::ObjectIdList(vec![]),
                expiry,
            ));
        } else {
            // leaf nodes
            for chunk in content_ser.chunks(data_chunk_size) {
                let data_chunk = BlockContentV0::DataChunk(chunk.to_vec());
                let content_ser = serde_bare::to_vec(&data_chunk).unwrap();
                blocks.push(make_object(
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
                (block_size - EMPTY_BLOCK_SIZE - BIG_VARINT_EXTRA * 2 - MAX_DEPS_SIZE)
                    / (BLOCK_ID_SIZE + BLOCK_KEY_SIZE);
            let mut parents = make_tree(
                blocks.as_slice(),
                &conv_key,
                obj_deps.clone(),
                expiry,
                arity,
            );
            blocks.append(&mut parents);
        }
        // root node
        let (root_block, root_key) = blocks.last().unwrap();
        let root_id = root_block.id();

        Object {
            id: root_id,
            key: Some(root_key.clone()),
            blocks: blocks.into_iter().map(|(obj, _key)| obj).collect(),
        }
    }

    /// Load an Object
    ///
    /// Returns Ok(Object) or an Err(Vec<ObjectId>) of missing BlockIds
    fn load<F>(id: ObjectId, key: Option<SymKey>, get_block: F) -> Result<Object, Vec<BlockId>>
    where
        F: Fn(&BlockId) -> Result<Block, ()>,
    {
        fn load_tree<F>(
            parents: Vec<BlockId>,
            get_block: F,
            blocks: &mut Vec<Block>,
            missing: &mut Vec<BlockId>,
        ) where
            F: Fn(&BlockId) -> Result<Block, ()>,
        {
            let mut children: Vec<BlockId> = vec![];
            for id in parents {
                match get_block(&id) {
                    Ok(obj) => {
                        blocks.insert(0, obj.clone());
                        match obj {
                            Block::V0(o) => {
                                children.extend(o.children.iter().rev());
                            }
                        }
                    }
                    Err(_) => missing.push(id.clone()),
                }
            }
            if !children.is_empty() {
                load_tree(children, get_block, blocks, missing);
            }
        }

        let mut blocks: Vec<Block> = vec![];
        let mut missing: Vec<BlockId> = vec![];

        load_tree(vec![id], get_block, &mut blocks, &mut missing);

        if missing.is_empty() {
            Ok(Object { id, key, blocks })
        } else {
            Err(missing)
        }
    }

    /// Load an Object from HashMap
    ///
    /// Returns Ok(Object) or an Err(Vec<ObjectId>) of missing BlockIds
    pub fn from_hashmap(
        id: ObjectId,
        key: Option<SymKey>,
        blocks: &HashMap<BlockId, Block>,
    ) -> Result<Object, Vec<BlockId>> {
        Self::load(id, key, |id: &BlockId| match blocks.get(id) {
            Some(block) => Ok(block.clone()),
            None => Err(()),
        })
    }

    /// Load an Object from Store
    ///
    /// Returns Ok(Object) or an Err(Vec<ObjectId>) of missing BlockIds
    pub fn from_store(
        id: ObjectId,
        key: Option<SymKey>,
        store: &impl Store,
    ) -> Result<Object, Vec<BlockId>> {
        Self::load(id, key, |id: &BlockId| store.get(id).or(Err(())))
    }

    /// Save blocks of the object in the store
    pub fn save(&self, store: &mut impl Store) -> Result<(), StorePutError> {
        for block in &self.blocks {
            store.put(block.clone())?;
        }
        Ok(())
    }

    /// Get the ID of the Object
    pub fn id(&self) -> ObjectId {
        self.id
    }

    /// Get the key for the Object
    pub fn key(&self) -> Option<SymKey> {
        self.key
    }

    /// Get an `ObjectRef` for the root object
    pub fn reference(&self) -> Option<ObjectRef> {
        if self.key.is_some() {
            Some(ObjectRef {
                id: self.id,
                key: self.key.unwrap(),
            })
        } else {
            None
        }
    }

    pub fn root(&self) -> &Block {
        self.blocks.last().unwrap()
    }

    pub fn blocks(&self) -> &Vec<Block> {
        &self.blocks
    }

    pub fn to_hashmap(&self) -> HashMap<BlockId, Block> {
        let mut map: HashMap<BlockId, Block> = HashMap::new();
        for block in &self.blocks {
            map.insert(block.id(), block.clone());
        }
        map
    }

    /// Parse the Object and return the decrypted content assembled from Blocks
    pub fn content(&self) -> Result<ObjectContent, ObjectParseError> {
        /// Collect decrypted leaves from the tree
        fn collect_leaves(
            nodes: &Vec<Block>,
            parents: &Vec<(ObjectId, SymKey)>,
            parent_index: usize,
            obj_content: &mut Vec<u8>,
        ) -> Result<(), ObjectParseError> {
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
                    return Err(ObjectParseError::InvalidBlockId);
                }

                match node {
                    Block::V0(obj) => {
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
                        let content: BlockContentV0;
                        match serde_bare::from_slice(content_dec.as_slice()) {
                            Ok(c) => content = c,
                            Err(e) => {
                                debug_println!("Block deserialize error: {}", e);
                                return Err(ObjectParseError::BlockDeserializeError);
                            }
                        }

                        // parse content
                        match content {
                            BlockContentV0::InternalNode(keys) => {
                                if keys.len() != obj.children.len() {
                                    debug_println!(
                                        "Invalid keys length: got {}, expected {}",
                                        keys.len(),
                                        obj.children.len()
                                    );
                                    debug_println!("!!! children: {:?}", obj.children);
                                    debug_println!("!!! keys: {:?}", keys);
                                    return Err(ObjectParseError::InvalidKeys);
                                }

                                for (id, key) in obj.children.iter().zip(keys.iter()) {
                                    children.push((id.clone(), key.clone()));
                                }
                            }
                            BlockContentV0::DataChunk(chunk) => {
                                obj_content.extend_from_slice(chunk.as_slice());
                            }
                        }
                    }
                }
            }
            if !children.is_empty() {
                if parent_index < children.len() {
                    return Err(ObjectParseError::InvalidChildren);
                }
                match collect_leaves(nodes, &children, parent_index - children.len(), obj_content) {
                    Ok(_) => (),
                    Err(e) => return Err(e),
                }
            }
            Ok(())
        }

        if self.key.is_none() {
            return Err(ObjectParseError::MissingRootKey);
        }

        let mut obj_content: Vec<u8> = vec![];
        let parents = vec![(self.id, self.key.unwrap())];
        match collect_leaves(
            &self.blocks,
            &parents,
            self.blocks.len() - 1,
            &mut obj_content,
        ) {
            Ok(_) => {
                let content: ObjectContent;
                match serde_bare::from_slice(obj_content.as_slice()) {
                    Ok(c) => Ok(c),
                    Err(e) => {
                        debug_println!("Object deserialize error: {}", e);
                        Err(ObjectParseError::ObjectDeserializeError)
                    }
                }
            }
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod test {

    use crate::object::*;
    use crate::store::*;
    use crate::types::*;

    // Those constants are calculated with Store::get_max_value_size

    /// Maximum arity of branch containing max number of leaves
    const MAX_ARITY_LEAVES: usize = 31774;
    /// Maximum arity of root branch
    const MAX_ARITY_ROOT: usize = 31770;
    /// Maximum data that can fit in object.content
    const MAX_DATA_PAYLOAD_SIZE: usize = 2097112;

    /// Test tree API
    #[test]
    pub fn test_object() {
        let file = File::V0(FileV0 {
            content_type: Vec::from("file/test"),
            metadata: Vec::from("some meta data here"),
            content: [(0..255).collect::<Vec<u8>>().as_slice(); 320].concat(),
        });
        let content = ObjectContent::File(file);

        let deps: Vec<ObjectId> = vec![Digest::Blake3Digest32([9; 32])];
        let expiry = Some(2u32.pow(31));
        let max_object_size = 0;

        let repo_secret = SymKey::ChaCha20Key([0; 32]);
        let repo_pubkey = PubKey::Ed25519PubKey([1; 32]);

        let object = Object::new(
            content.clone(),
            deps,
            expiry,
            max_object_size,
            repo_pubkey,
            repo_secret,
        );

        println!("root_id: {:?}", object.id());
        println!("root_key: {:?}", object.key().unwrap());
        println!("nodes.len: {:?}", object.blocks().len());
        //println!("nodes: {:?}", tree.nodes());
        let mut i = 0;
        for node in object.blocks() {
            println!("#{}: {:?}", i, node.id());
            i += 1;
        }

        match object.content() {
            Ok(cnt) => {
                assert_eq!(content, cnt);
            }
            Err(e) => panic!("Object parse error: {:?}", e),
        }
        let mut store = HashMapStore::new();

        object.save(&mut store).expect("Object save error");

        let object2 = Object::from_store(object.id(), object.key(), &store).unwrap();

        println!("nodes2.len: {:?}", object2.blocks().len());
        //println!("nodes2: {:?}", tree2.nodes());
        let mut i = 0;
        for node in object2.blocks() {
            println!("#{}: {:?}", i, node.id());
            i += 1;
        }

        match object2.content() {
            Ok(cnt) => {
                assert_eq!(content, cnt);
            }
            Err(e) => panic!("Object2 parse error: {:?}", e),
        }

        let map = object.to_hashmap();
        let object3 = Object::from_hashmap(object.id(), object.key(), &map).unwrap();
        match object3.content() {
            Ok(cnt) => {
                assert_eq!(content, cnt);
            }
            Err(e) => panic!("Object3 parse error: {:?}", e),
        }
    }

    /// Checks that a content that fits the root node, will not be chunked into children nodes
    #[test]
    pub fn test_depth_1() {
        let deps: Vec<ObjectId> = vec![Digest::Blake3Digest32([9; 32])];

        let empty_file = ObjectContent::File(File::V0(FileV0 {
            content_type: vec![],
            metadata: vec![],
            content: vec![],
        }));
        let empty_file_ser = serde_bare::to_vec(&empty_file).unwrap();
        println!("empty file size: {}", empty_file_ser.len());

        let size = store_max_value_size()
            - EMPTY_BLOCK_SIZE
            - DATA_VARINT_EXTRA
            - BLOCK_ID_SIZE * deps.len()
            - empty_file_ser.len()
            - DATA_VARINT_EXTRA;
        println!("file size: {}", size);

        let content = ObjectContent::File(File::V0(FileV0 {
            content_type: vec![],
            metadata: vec![],
            content: vec![99; size],
        }));
        let content_ser = serde_bare::to_vec(&content).unwrap();
        println!("content len: {}", content_ser.len());

        let expiry = Some(2u32.pow(31));
        let max_object_size = store_max_value_size();

        let repo_secret = SymKey::ChaCha20Key([0; 32]);
        let repo_pubkey = PubKey::Ed25519PubKey([1; 32]);

        let object = Object::new(
            content,
            deps,
            expiry,
            max_object_size,
            repo_pubkey,
            repo_secret,
        );

        println!("root_id: {:?}", object.id());
        println!("root_key: {:?}", object.key().unwrap());
        println!("nodes.len: {:?}", object.blocks().len());
        //println!("root: {:?}", tree.root());
        //println!("nodes: {:?}", object.blocks);
        assert_eq!(object.blocks.len(), 1);
    }

    #[test]
    pub fn test_block_size() {
        let max_block_size = store_max_value_size();
        println!("max_object_size: {}", max_block_size);

        let id = Digest::Blake3Digest32([0u8; 32]);
        let key = SymKey::ChaCha20Key([0u8; 32]);

        let one_key = BlockContentV0::InternalNode(vec![key]);
        let one_key_ser = serde_bare::to_vec(&one_key).unwrap();

        let two_keys = BlockContentV0::InternalNode(vec![key, key]);
        let two_keys_ser = serde_bare::to_vec(&two_keys).unwrap();

        let max_keys = BlockContentV0::InternalNode(vec![key; MAX_ARITY_LEAVES]);
        let max_keys_ser = serde_bare::to_vec(&max_keys).unwrap();

        let data = BlockContentV0::DataChunk(vec![]);
        let data_ser = serde_bare::to_vec(&data).unwrap();

        let data_full = BlockContentV0::DataChunk(vec![0; MAX_DATA_PAYLOAD_SIZE]);
        let data_full_ser = serde_bare::to_vec(&data_full).unwrap();

        let leaf_empty = Block::V0(BlockV0 {
            children: vec![],
            deps: ObjectDeps::ObjectIdList(vec![]),
            expiry: Some(2342),
            content: data_ser.clone(),
        });
        let leaf_empty_ser = serde_bare::to_vec(&leaf_empty).unwrap();

        let leaf_full_data = Block::V0(BlockV0 {
            children: vec![],
            deps: ObjectDeps::ObjectIdList(vec![]),
            expiry: Some(2342),
            content: data_full_ser.clone(),
        });
        let leaf_full_data_ser = serde_bare::to_vec(&leaf_full_data).unwrap();

        let root_depsref = Block::V0(BlockV0 {
            children: vec![],
            deps: ObjectDeps::DepListRef(ObjectRef { id: id, key: key }),
            expiry: Some(2342),
            content: data_ser.clone(),
        });

        let root_depsref_ser = serde_bare::to_vec(&root_depsref).unwrap();

        let internal_max = Block::V0(BlockV0 {
            children: vec![id; MAX_ARITY_LEAVES],
            deps: ObjectDeps::ObjectIdList(vec![]),
            expiry: Some(2342),
            content: max_keys_ser.clone(),
        });
        let internal_max_ser = serde_bare::to_vec(&internal_max).unwrap();

        let internal_one = Block::V0(BlockV0 {
            children: vec![id; 1],
            deps: ObjectDeps::ObjectIdList(vec![]),
            expiry: Some(2342),
            content: one_key_ser.clone(),
        });
        let internal_one_ser = serde_bare::to_vec(&internal_one).unwrap();

        let internal_two = Block::V0(BlockV0 {
            children: vec![id; 2],
            deps: ObjectDeps::ObjectIdList(vec![]),
            expiry: Some(2342),
            content: two_keys_ser.clone(),
        });
        let internal_two_ser = serde_bare::to_vec(&internal_two).unwrap();

        let root_one = Block::V0(BlockV0 {
            children: vec![id; 1],
            deps: ObjectDeps::ObjectIdList(vec![id; 8]),
            expiry: Some(2342),
            content: one_key_ser.clone(),
        });
        let root_one_ser = serde_bare::to_vec(&root_one).unwrap();

        let root_two = Block::V0(BlockV0 {
            children: vec![id; 2],
            deps: ObjectDeps::ObjectIdList(vec![id; 8]),
            expiry: Some(2342),
            content: two_keys_ser.clone(),
        });
        let root_two_ser = serde_bare::to_vec(&root_two).unwrap();

        println!(
            "range of valid value sizes {} {}",
            store_valid_value_size(0),
            store_max_value_size()
        );

        println!(
            "max_data_payload_of_object: {}",
            max_block_size - EMPTY_BLOCK_SIZE - DATA_VARINT_EXTRA
        );

        println!(
            "max_data_payload_depth_1: {}",
            max_block_size - EMPTY_BLOCK_SIZE - DATA_VARINT_EXTRA - MAX_DEPS_SIZE
        );

        println!(
            "max_data_payload_depth_2: {}",
            MAX_ARITY_ROOT * MAX_DATA_PAYLOAD_SIZE
        );

        println!(
            "max_data_payload_depth_3: {}",
            MAX_ARITY_ROOT * MAX_ARITY_LEAVES * MAX_DATA_PAYLOAD_SIZE
        );

        let max_arity_leaves = (max_block_size - EMPTY_BLOCK_SIZE - BIG_VARINT_EXTRA * 2)
            / (BLOCK_ID_SIZE + BLOCK_KEY_SIZE);
        println!("max_arity_leaves: {}", max_arity_leaves);
        assert_eq!(max_arity_leaves, MAX_ARITY_LEAVES);
        assert_eq!(
            max_block_size - EMPTY_BLOCK_SIZE - DATA_VARINT_EXTRA,
            MAX_DATA_PAYLOAD_SIZE
        );
        let max_arity_root =
            (max_block_size - EMPTY_BLOCK_SIZE - MAX_DEPS_SIZE - BIG_VARINT_EXTRA * 2)
                / (BLOCK_ID_SIZE + BLOCK_KEY_SIZE);
        println!("max_arity_root: {}", max_arity_root);
        assert_eq!(max_arity_root, MAX_ARITY_ROOT);
        println!("store_max_value_size: {}", leaf_full_data_ser.len());
        assert_eq!(leaf_full_data_ser.len(), max_block_size);
        println!("leaf_empty: {}", leaf_empty_ser.len());
        assert_eq!(leaf_empty_ser.len(), EMPTY_BLOCK_SIZE);
        println!("root_depsref: {}", root_depsref_ser.len());
        assert_eq!(root_depsref_ser.len(), EMPTY_ROOT_SIZE_DEPSREF);
        println!("internal_max: {}", internal_max_ser.len());
        assert_eq!(
            internal_max_ser.len(),
            EMPTY_BLOCK_SIZE
                + BIG_VARINT_EXTRA * 2
                + MAX_ARITY_LEAVES * (BLOCK_ID_SIZE + BLOCK_KEY_SIZE)
        );
        assert!(internal_max_ser.len() < max_block_size);
        println!("internal_one: {}", internal_one_ser.len());
        assert_eq!(
            internal_one_ser.len(),
            EMPTY_BLOCK_SIZE + 1 * BLOCK_ID_SIZE + 1 * BLOCK_KEY_SIZE
        );
        println!("internal_two: {}", internal_two_ser.len());
        assert_eq!(
            internal_two_ser.len(),
            EMPTY_BLOCK_SIZE + 2 * BLOCK_ID_SIZE + 2 * BLOCK_KEY_SIZE
        );
        println!("root_one: {}", root_one_ser.len());
        assert_eq!(
            root_one_ser.len(),
            EMPTY_BLOCK_SIZE + 8 * BLOCK_ID_SIZE + 1 * BLOCK_ID_SIZE + 1 * BLOCK_KEY_SIZE
        );
        println!("root_two: {}", root_two_ser.len());
        assert_eq!(
            root_two_ser.len(),
            EMPTY_BLOCK_SIZE + 8 * BLOCK_ID_SIZE + 2 * BLOCK_ID_SIZE + 2 * BLOCK_KEY_SIZE
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
