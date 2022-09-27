//! Object store
//!

use serde::{Deserialize, Serialize};
use std::cmp::min;
use std::path::Path;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::types::*;
use lofire::types::*;
use rkv::backend::{
    BackendDatabaseFlags, BackendFlags, BackendIter, BackendWriteFlags, DatabaseFlags, Lmdb,
    LmdbDatabase, LmdbDatabaseFlags, LmdbEnvironment, LmdbRwTransaction, LmdbWriteFlags,
};
use rkv::{
    Manager, MultiIntegerStore, Rkv, SingleStore, StoreError, StoreError::DataError, StoreOptions,
    Value, WriteFlags, Writer,
};
use serde_bare::error::Error;

pub struct Store {
    /// the main store where all the repo objects are stored
    main_store: SingleStore<LmdbDatabase>,
    /// store for the pin boolean, recently_used timestamp, and synced boolean
    meta_store: SingleStore<LmdbDatabase>,
    /// store for the expiry timestamp
    expiry_store: MultiIntegerStore<LmdbDatabase, u32>,
    /// store for the LRU list
    recently_used_store: MultiIntegerStore<LmdbDatabase, u32>,
    /// the opened environment so we can create new transactions
    environment: Arc<RwLock<Rkv<LmdbEnvironment>>>,
}

// TODO: versioning V0
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
struct ObjectMeta {
    pub pin: bool,
    pub last_used: Timestamp,
    pub synced: bool,
}

impl Store {
    /// returns the Lofire Timestamp of now.
    fn now_timestamp() -> Timestamp {
        ((SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - EPOCH_AS_UNIX_TIMESTAMP)
            / 60)
            .try_into()
            .unwrap()
    }

    /// Opens the store and returns a Store object that should be kept and used to call put/get/delete/pin
    /// The key is the encryption key for the data at rest.
    pub fn open<'a>(path: &Path, key: [u8; 32]) -> Store {
        let mut manager = Manager::<LmdbEnvironment>::singleton().write().unwrap();
        let shared_rkv = manager
            .get_or_create(path, |path| {
                //Rkv::new::<Lmdb>(path) // use this instead to disable encryption
                Rkv::with_encryption_key_and_mapsize::<Lmdb>(path, key, 2 * 1024 * 1024 * 1024)
            })
            .unwrap();
        let env = shared_rkv.read().unwrap();

        println!("created env with LMDB Version: {}", env.version());

        let main_store = env.open_single("main", StoreOptions::create()).unwrap();
        let meta_store = env.open_single("meta", StoreOptions::create()).unwrap();
        let mut opts = StoreOptions::<LmdbDatabaseFlags>::create();
        opts.flags.set(DatabaseFlags::DUP_FIXED, true);
        let expiry_store = env.open_multi_integer("expiry", opts).unwrap();
        let recently_used_store = env.open_multi_integer("recently_used", opts).unwrap();

        Store {
            environment: shared_rkv.clone(),
            main_store,
            meta_store,
            expiry_store,
            recently_used_store,
        }
    }

    /// Pins the object
    pub fn pin(&self, object_id: &ObjectId) -> Result<(), Error> {
        self.set_pin(object_id, true)
    }

    /// Unpins the object
    pub fn unpin(&self, object_id: &ObjectId) -> Result<(), Error> {
        self.set_pin(object_id, false)
    }

    /// Sets the pin for that Object. if add is true, will add the pin. if false, will remove the pin.
    /// A pin on an object prevents it from being removed when the store is making some disk space by using the LRU.
    /// A pin does not override the expiry. If expiry is set and is reached, the obejct will be deleted, no matter what.
    pub fn set_pin(&self, object_id: &ObjectId, add: bool) -> Result<(), Error> {
        let lock = self.environment.read().unwrap();
        let mut writer = lock.write().unwrap();
        let obj_id_ser = serde_bare::to_vec(&object_id).unwrap();
        let meta_ser = self.meta_store.get(&writer, &obj_id_ser).unwrap();
        let mut meta;

        // if adding a pin, if there is a meta (if already pinned, return) and is synced, remove the last_used timestamp from recently_used_store
        // if no meta, create it with pin:true, synced: false
        // if removing a pin (if pin already removed, return), if synced, add an entry to recently_used_store with the last_used timestamp (as found in meta, dont use now)

        match meta_ser {
            Some(meta_value) => {
                meta =
                    serde_bare::from_slice::<ObjectMeta>(&meta_value.to_bytes().unwrap()).unwrap();

                if add == meta.pin {
                    // pinning while already pinned, or unpinning while already unpinned. NOP
                    return Ok(());
                };

                meta.pin = add;

                if meta.synced {
                    if add {
                        // we remove the previous timestamp (last_used) from recently_used_store
                        self.remove_from_lru(&mut writer, &obj_id_ser, &meta.last_used)
                            .unwrap();
                    } else {
                        // we add an entry to recently_used_store with last_used
                        self.add_to_lru(&mut writer, &obj_id_ser, &meta.last_used)
                            .unwrap();
                    }
                }
            }
            None => {
                if add {
                    meta = ObjectMeta {
                        pin: true,
                        synced: false,
                        last_used: 0,
                    }
                } else {
                    // there is no meta, and user wants to unpin, so let's leave everything as it is.
                    return Ok(());
                }
            }
        }
        let new_meta_ser = serde_bare::to_vec(&meta).unwrap();
        self.meta_store
            .put(
                &mut writer,
                obj_id_ser,
                &Value::Blob(new_meta_ser.as_slice()),
            )
            .unwrap();
        // commit
        writer.commit().unwrap();

        Ok(())
    }

    /// the broker calls this method when the object has been retrieved/synced by enough peers and it
    /// can now be included in the LRU for potential garbage collection.
    /// If this method has not been called on an object, it will be kept in the store and will not enter LRU.
    pub fn has_been_synced(&self, object_id: &ObjectId, when: Option<u32>) -> Result<(), Error> {
        let lock = self.environment.read().unwrap();
        let mut writer = lock.write().unwrap();
        let obj_id_ser = serde_bare::to_vec(&object_id).unwrap();
        let meta_ser = self.meta_store.get(&writer, obj_id_ser.clone()).unwrap();
        let mut meta;
        let now = match when {
            None => Self::now_timestamp(),
            Some(w) => w,
        };
        // get the meta. if no meta, it is ok, we will create it after (with pin:false and synced:true)
        // if already synced, return
        // update the meta with last_used:now and synced:true
        // if pinned, save and return
        // otherwise add an entry to recently_used_store with now

        match meta_ser {
            Some(meta_value) => {
                meta =
                    serde_bare::from_slice::<ObjectMeta>(&meta_value.to_bytes().unwrap()).unwrap();

                if meta.synced {
                    // already synced. NOP
                    return Ok(());
                };

                meta.synced = true;
                meta.last_used = now;

                if !meta.pin {
                    // we add an entry to recently_used_store with now
                    println!("adding to LRU");
                    self.add_to_lru(&mut writer, &obj_id_ser, &now).unwrap();
                }
            }
            None => {
                meta = ObjectMeta {
                    pin: false,
                    synced: true,
                    last_used: now,
                };
                println!("adding to LRU also");
                self.add_to_lru(&mut writer, &obj_id_ser, &now).unwrap();
            }
        }
        let new_meta_ser = serde_bare::to_vec(&meta).unwrap();
        self.meta_store
            .put(
                &mut writer,
                obj_id_ser,
                &Value::Blob(new_meta_ser.as_slice()),
            )
            .unwrap();
        // commit
        writer.commit().unwrap();

        Ok(())
    }

    /// Retrieves an object from the storage backend.
    pub fn get(&self, object_id: &ObjectId) -> Result<Object, StoreError> {
        let lock = self.environment.read().unwrap();
        let reader = lock.read().unwrap();
        let obj_id_ser = serde_bare::to_vec(&object_id).unwrap();
        let obj_ser_res = self.main_store.get(&reader, obj_id_ser.clone());
        match obj_ser_res {
            Err(e) => Err(e),
            Ok(None) => Err(StoreError::FileInvalid),
            Ok(Some(obj_ser)) => {
                // updating recently_used
                // first getting the meta for this objectId
                let meta_ser = self.meta_store.get(&reader, obj_id_ser.clone()).unwrap();
                match meta_ser {
                    Some(meta_value) => {
                        let mut meta =
                            serde_bare::from_slice::<ObjectMeta>(&meta_value.to_bytes().unwrap())
                                .unwrap();
                        if meta.synced {
                            let mut writer = lock.write().unwrap();
                            let now = Self::now_timestamp();
                            if !meta.pin {
                                // we remove the previous timestamp (last_used) from recently_used_store
                                self.remove_from_lru(&mut writer, &obj_id_ser, &meta.last_used)
                                    .unwrap();
                                // we add an entry to recently_used_store with now
                                self.add_to_lru(&mut writer, &obj_id_ser, &now).unwrap();
                            }
                            // we save the new meta (with last_used:now)
                            meta.last_used = now;
                            let new_meta_ser = serde_bare::to_vec(&meta).unwrap();
                            self.meta_store
                                .put(
                                    &mut writer,
                                    obj_id_ser,
                                    &Value::Blob(new_meta_ser.as_slice()),
                                )
                                .unwrap();
                            // commit
                            writer.commit().unwrap();
                        }
                    }
                    _ => {} // there is no meta. we do nothing since we start to record LRU only once synced == true.
                }

                match serde_bare::from_slice::<Object>(&obj_ser.to_bytes().unwrap()) {
                    Err(e) => Err(StoreError::FileInvalid),
                    Ok(o) => Ok(o),
                }
            }
        }
    }

    /// Adds an object in the storage backend. The object is persisted to disk. Returns the ObjectId of the Object.
    pub fn put(&self, object: &Object) -> ObjectId {
        let obj_ser = serde_bare::to_vec(&object).unwrap();

        let hash = blake3::hash(obj_ser.as_slice());
        let obj_id = Digest::Blake3Digest32(hash.as_bytes().clone());
        let obj_id_ser = serde_bare::to_vec(&obj_id).unwrap();

        let lock = self.environment.read().unwrap();
        let mut writer = lock.write().unwrap();
        self.main_store
            .put(&mut writer, &obj_id_ser, &Value::Blob(obj_ser.as_slice()))
            .unwrap();

        // if it has an expiry, adding the objectId to the expiry_store
        match object.get_expiry() {
            Some(expiry) => {
                self.expiry_store
                    .put(&mut writer, expiry, &Value::Blob(obj_id_ser.as_slice()))
                    .unwrap();
            }
            _ => {}
        }
        writer.commit().unwrap();

        obj_id
    }

    /// Removes the object from the storage backend. The removed object is returned so it can be inspected.
    /// Also returned is the approximate size of of free space that was reclaimed.
    pub fn del(&self, object_id: &ObjectId) -> Result<(Object, usize), StoreError> {
        let lock = self.environment.read().unwrap();
        let mut writer = lock.write().unwrap();
        let obj_id_ser = serde_bare::to_vec(&object_id).unwrap();
        // retrieving the object itself (we need the expiry)
        let obj_ser = self.main_store.get(&writer, obj_id_ser.clone())?;
        if obj_ser.is_none() {
            return Err(StoreError::FileInvalid); //FIXME with propoer Error type
        }
        let slice = obj_ser.unwrap().to_bytes().unwrap();
        let obj = serde_bare::from_slice::<Object>(&slice).unwrap(); //FIXME propagate error?
        let meta_res = self.meta_store.get(&writer, obj_id_ser.clone())?;
        if meta_res.is_some() {
            let meta = serde_bare::from_slice::<ObjectMeta>(&meta_res.unwrap().to_bytes().unwrap())
                .unwrap();
            if meta.last_used != 0 {
                self.remove_from_lru(&mut writer, &obj_id_ser.clone(), &meta.last_used)?;
            }
            // removing the meta
            self.meta_store.delete(&mut writer, obj_id_ser.clone())?;
        }
        // deleting object from main_store
        self.main_store.delete(&mut writer, obj_id_ser.clone())?;
        // removing objectId from expiry_store, if any expiry
        match obj.get_expiry() {
            Some(expiry) => {
                self.expiry_store.delete(
                    &mut writer,
                    expiry,
                    &Value::Blob(obj_id_ser.clone().as_slice()),
                )?;
            }
            _ => {}
        }

        writer.commit().unwrap();
        Ok((obj, slice.len()))
    }

    /// Removes all the objects that have expired. The broker should call this method periodically.
    pub fn remove_expired(&self) -> Result<(), Error> {
        let lock = self.environment.read().unwrap();
        let reader = lock.read().unwrap();

        let mut iter = self
            .expiry_store
            .iter_prev_dup_from(&reader, Self::now_timestamp())
            .unwrap();

        while let Some(Ok(mut sub_iter)) = iter.next() {
            while let Some(Ok(k)) = sub_iter.next() {
                //println!("removing {:?} {:?}", k.0, k.1);
                let obj_id = serde_bare::from_slice::<ObjectId>(k.1).unwrap();
                self.del(&obj_id).unwrap();
            }
        }
        Ok(())
    }

    /// Returns a valid/optimal value size for the entries of the storage backend.
    pub fn get_valid_value_size(size: usize) -> usize {
        const MIN_SIZE: usize = 4072;
        const PAGE_SIZE: usize = 4096;
        const HEADER: usize = PAGE_SIZE - MIN_SIZE;
        const MAX_FACTOR: usize = 512;
        min(
            ((size + HEADER) as f32 / PAGE_SIZE as f32).ceil() as usize,
            MAX_FACTOR,
        ) * PAGE_SIZE
            - HEADER
    }

    /// Removes some objects that haven't been used for a while, reclaiming some room on disk.
    /// The oldest are removed first, until the total amount of data removed is at least equal to size,
    /// or the LRU list became empty. The approximate size of the storage space that was reclaimed is returned.
    pub fn remove_least_used(&self, size: usize) -> usize {
        let lock = self.environment.read().unwrap();
        let reader = lock.read().unwrap();

        let mut iter = self.recently_used_store.iter_start(&reader).unwrap();

        let mut total: usize = 0;

        while let Some(Ok(entry)) = iter.next() {
            let obj_id =
                serde_bare::from_slice::<ObjectId>(entry.1.to_bytes().unwrap().as_slice()).unwrap();
            let obj = self.del(&obj_id).unwrap();
            println!("removed {:?}", obj_id);
            total += obj.1;
            if total >= size {
                break;
            }
        }
        total
    }

    fn remove_from_lru(
        &self,
        writer: &mut Writer<LmdbRwTransaction>,
        obj_id_ser: &Vec<u8>,
        time: &Timestamp,
    ) -> Result<(), StoreError> {
        self.recently_used_store
            .delete(writer, *time, &Value::Blob(obj_id_ser.as_slice()))
    }

    fn add_to_lru(
        &self,
        writer: &mut Writer<LmdbRwTransaction>,
        obj_id_ser: &Vec<u8>,
        time: &Timestamp,
    ) -> Result<(), StoreError> {
        let mut flag = LmdbWriteFlags::empty();
        flag.set(WriteFlags::APPEND_DUP, true);
        self.recently_used_store.put_with_flags(
            writer,
            *time,
            &Value::Blob(obj_id_ser.as_slice()),
            flag,
        )
    }

    fn list_all(&self) {
        let lock = self.environment.read().unwrap();
        let reader = lock.read().unwrap();
        println!("MAIN");
        let mut iter = self.main_store.iter_start(&reader).unwrap();
        while let Some(Ok(entry)) = iter.next() {
            println!("{:?} {:?}", entry.0, entry.1)
        }
        println!("META");
        let mut iter2 = self.meta_store.iter_start(&reader).unwrap();
        while let Some(Ok(entry)) = iter2.next() {
            println!("{:?} {:?}", entry.0, entry.1)
        }
        println!("EXPIRY");
        let mut iter3 = self.expiry_store.iter_start(&reader).unwrap();
        while let Some(Ok(entry)) = iter3.next() {
            println!("{:?} {:?}", entry.0, entry.1)
        }
        println!("LRU");
        let mut iter4 = self.recently_used_store.iter_start(&reader).unwrap();
        while let Some(Ok(entry)) = iter4.next() {
            println!("{:?} {:?}", entry.0, entry.1)
        }
    }
}
#[cfg(test)]
mod test {

    use crate::store::Store;
    use crate::types::*;
    use lofire::types::*;
    use rkv::backend::{BackendInfo, BackendStat, Lmdb, LmdbEnvironment};
    use rkv::{Manager, Rkv, StoreOptions, Value};
    #[allow(unused_imports)]
    use std::time::Duration;
    #[allow(unused_imports)]
    use std::{fs, thread};
    use tempfile::Builder;

    #[test]
    pub fn test_remove_least_used() {
        let path_str = "test-env";
        let root = Builder::new().prefix(path_str).tempdir().unwrap();
        let key: [u8; 32] = [0; 32];
        fs::create_dir_all(root.path()).unwrap();
        println!("{}", root.path().to_str().unwrap());
        let store = Store::open(root.path(), key);
        let mut now = Store::now_timestamp();
        now -= 200;
        // TODO: fix the LMDB bug that is triggered with x max set to 86 !!!
        for x in 1..85 {
            let obj = ObjectV0 {
                children: Vec::new(),
                deps: ObjectDeps::ObjectIdList(Vec::new()),
                expiry: None,
                content: vec![x; 10],
            };
            let obj_id = store.put(&Object::V0(obj.clone()));
            println!("#{} -> objId {:?}", x, obj_id);
            store
                .has_been_synced(&obj_id, Some(now + x as u32))
                .unwrap();
        }

        let ret = store.remove_least_used(200);
        println!("removed {}", ret);
        assert_eq!(ret, 208)

        //store.list_all();
    }

    #[test]
    pub fn test_set_pin() {
        let path_str = "test-env";
        let root = Builder::new().prefix(path_str).tempdir().unwrap();
        let key: [u8; 32] = [0; 32];
        fs::create_dir_all(root.path()).unwrap();
        println!("{}", root.path().to_str().unwrap());
        let store = Store::open(root.path(), key);
        let mut now = Store::now_timestamp();
        now -= 200;
        // TODO: fix the LMDB bug that is triggered with x max set to 86 !!!
        for x in 1..85 {
            let obj = ObjectV0 {
                children: Vec::new(),
                deps: ObjectDeps::ObjectIdList(Vec::new()),
                expiry: None,
                content: vec![x; 10],
            };
            let obj_id = store.put(&Object::V0(obj.clone()));
            println!("#{} -> objId {:?}", x, obj_id);
            store.set_pin(&obj_id, true).unwrap();
            store
                .has_been_synced(&obj_id, Some(now + x as u32))
                .unwrap();
        }

        let ret = store.remove_least_used(200);
        println!("removed {}", ret);
        assert_eq!(ret, 0);

        store.list_all();
    }

    #[test]
    pub fn test_get_valid_value_size() {
        assert_eq!(Store::get_valid_value_size(0), 4072);
        assert_eq!(Store::get_valid_value_size(2), 4072);
        assert_eq!(Store::get_valid_value_size(4072), 4072);
        assert_eq!(Store::get_valid_value_size(4072 + 1), 4072 + 4096);
        assert_eq!(Store::get_valid_value_size(4072 + 4096), 4072 + 4096);
        assert_eq!(
            Store::get_valid_value_size(4072 + 4096 + 1),
            4072 + 4096 + 4096
        );
        assert_eq!(
            Store::get_valid_value_size(4072 + 4096 + 4096),
            4072 + 4096 + 4096
        );
        assert_eq!(
            Store::get_valid_value_size(4072 + 4096 + 4096 + 1),
            4072 + 4096 + 4096 + 4096
        );
        assert_eq!(
            Store::get_valid_value_size(4072 + 4096 * 511),
            4072 + 4096 * 511
        );
        assert_eq!(
            Store::get_valid_value_size(4072 + 4096 * 511 + 1),
            4072 + 4096 * 511
        );
    }

    #[test]
    pub fn test_remove_expired() {
        let path_str = "test-env";
        let root = Builder::new().prefix(path_str).tempdir().unwrap();
        let key: [u8; 32] = [0; 32];
        fs::create_dir_all(root.path()).unwrap();
        println!("{}", root.path().to_str().unwrap());
        let store = Store::open(root.path(), key);

        let now = Store::now_timestamp();
        let list = [
            now - 10,
            now - 6,
            now - 6,
            now - 3,
            now - 2,
            now - 1, //#5 should be removed, and above
            now + 3,
            now + 4,
            now + 4,
            now + 5,
            now + 10,
        ];
        let mut listObjId: Vec<ObjectId> = Vec::with_capacity(11);
        println!("now {}", now);

        let mut i = 0u8;
        for expiry in list {
            //let i: u8 = (expiry + 10 - now).try_into().unwrap();
            let obj = ObjectV0 {
                children: Vec::new(),
                deps: ObjectDeps::ObjectIdList(Vec::new()),
                expiry: Some(expiry),
                content: [i].to_vec(),
            };
            let obj_id = store.put(&Object::V0(obj.clone()));
            println!("#{} -> objId {:?}", i, obj_id);
            listObjId.push(obj_id);
            i += 1;
        }

        store.remove_expired();

        assert!(store.get(listObjId.get(0).unwrap()).is_err());
        assert!(store.get(listObjId.get(1).unwrap()).is_err());
        assert!(store.get(listObjId.get(2).unwrap()).is_err());
        assert!(store.get(listObjId.get(5).unwrap()).is_err());
        assert!(store.get(listObjId.get(6).unwrap()).is_ok());
        assert!(store.get(listObjId.get(7).unwrap()).is_ok());

        //store.list_all();
    }

    #[test]
    pub fn test_remove_all_expired() {
        let path_str = "test-env";
        let root = Builder::new().prefix(path_str).tempdir().unwrap();
        let key: [u8; 32] = [0; 32];
        fs::create_dir_all(root.path()).unwrap();
        println!("{}", root.path().to_str().unwrap());
        let store = Store::open(root.path(), key);

        let now = Store::now_timestamp();
        let list = [
            now - 10,
            now - 6,
            now - 6,
            now - 3,
            now - 2,
            now - 2, //#5 should be removed, and above
        ];
        let mut listObjId: Vec<ObjectId> = Vec::with_capacity(6);
        println!("now {}", now);

        let mut i = 0u8;
        for expiry in list {
            //let i: u8 = (expiry + 10 - now).try_into().unwrap();
            let obj = ObjectV0 {
                children: Vec::new(),
                deps: ObjectDeps::ObjectIdList(Vec::new()),
                expiry: Some(expiry),
                content: [i].to_vec(),
            };
            let obj_id = store.put(&Object::V0(obj.clone()));
            println!("#{} -> objId {:?}", i, obj_id);
            listObjId.push(obj_id);
            i += 1;
        }

        store.remove_expired();

        assert!(store.get(listObjId.get(0).unwrap()).is_err());
        assert!(store.get(listObjId.get(1).unwrap()).is_err());
        assert!(store.get(listObjId.get(2).unwrap()).is_err());
        assert!(store.get(listObjId.get(3).unwrap()).is_err());
        assert!(store.get(listObjId.get(4).unwrap()).is_err());
        assert!(store.get(listObjId.get(5).unwrap()).is_err());
    }

    #[test]
    pub fn test_remove_empty_expired() {
        let path_str = "test-env";
        let root = Builder::new().prefix(path_str).tempdir().unwrap();
        let key: [u8; 32] = [0; 32];
        fs::create_dir_all(root.path()).unwrap();
        println!("{}", root.path().to_str().unwrap());
        let store = Store::open(root.path(), key);

        store.remove_expired();
    }

    #[test]
    pub fn test_store_object() {
        let path_str = "test-env";
        let root = Builder::new().prefix(path_str).tempdir().unwrap();

        let key: [u8; 32] = [0; 32];
        fs::create_dir_all(root.path()).unwrap();

        println!("{}", root.path().to_str().unwrap());

        let store = Store::open(root.path(), key);

        let obj = ObjectV0 {
            children: Vec::new(),
            deps: ObjectDeps::ObjectIdList(Vec::new()),
            expiry: None,
            content: b"abc".to_vec(),
        };

        let obj_id = store.put(&Object::V0(obj.clone()));

        println!("ObjectId: {:?}", obj_id);
        assert_eq!(
            obj_id,
            Digest::Blake3Digest32([
                155, 83, 186, 17, 95, 10, 80, 31, 111, 24, 250, 64, 8, 145, 71, 193, 103, 246, 202,
                28, 202, 144, 63, 65, 85, 229, 136, 85, 202, 34, 13, 85
            ])
        );

        let objres = store.get(&obj_id).unwrap();

        println!("Object: {:?}", objres);
        assert_eq!(objres, Object::V0(obj));
    }

    #[test]
    pub fn test_lmdb() {
        let path_str = "test-env";
        let root = Builder::new().prefix(path_str).tempdir().unwrap();

        // we set an encryption key with all zeros... for test purpose only ;)
        let key: [u8; 32] = [0; 32];
        {
            fs::create_dir_all(root.path()).unwrap();

            println!("{}", root.path().to_str().unwrap());

            let mut manager = Manager::<LmdbEnvironment>::singleton().write().unwrap();
            let shared_rkv = manager
                .get_or_create(root.path(), |path| {
                    // Rkv::new::<Lmdb>(path) // use this instead to disable encryption
                    Rkv::with_encryption_key_and_mapsize::<Lmdb>(path, key, 2 * 1024 * 1024 * 1024)
                })
                .unwrap();
            let env = shared_rkv.read().unwrap();

            println!("LMDB Version: {}", env.version());

            let store = env.open_single("testdb", StoreOptions::create()).unwrap();

            {
                // Use a write transaction to mutate the store via a `Writer`. There can be only
                // one writer for a given environment, so opening a second one will block until
                // the first completes.
                let mut writer = env.write().unwrap();

                // Keys are `AsRef<[u8]>`, while values are `Value` enum instances. Use the `Blob`
                // variant to store arbitrary collections of bytes. Putting data returns a
                // `Result<(), StoreError>`, where StoreError is an enum identifying the reason
                // for a failure.
                // store.put(&mut writer, "int", &Value::I64(1234)).unwrap();
                // store
                //     .put(&mut writer, "uint", &Value::U64(1234_u64))
                //     .unwrap();
                // store
                //     .put(&mut writer, "float", &Value::F64(1234.0.into()))
                //     .unwrap();
                // store
                //     .put(&mut writer, "instant", &Value::Instant(1528318073700))
                //     .unwrap();
                // store
                //     .put(&mut writer, "boolean", &Value::Bool(true))
                //     .unwrap();
                // store
                //     .put(&mut writer, "string", &Value::Str("Héllo, wörld!"))
                //     .unwrap();
                // store
                //     .put(
                //         &mut writer,
                //         "json",
                //         &Value::Json(r#"{"foo":"bar", "number": 1}"#),
                //     )
                //     .unwrap();
                const EXTRA: usize = 2095; // + 4096 * 524280 + 0;
                let key: [u8; 33] = [0; 33];
                let key2: [u8; 33] = [2; 33];
                let key3: [u8; 33] = [3; 33];
                let key4: [u8; 33] = [4; 33];
                //let value: [u8; 1977 + EXTRA] = [1; 1977 + EXTRA];
                let value = vec![1; 1977 + EXTRA];
                let value2: [u8; 1977 + 1] = [1; 1977 + 1];
                let value4: [u8; 953 + 0] = [1; 953 + 0];
                store.put(&mut writer, key, &Value::Blob(&value2)).unwrap();
                store.put(&mut writer, key2, &Value::Blob(&value2)).unwrap();
                // store.put(&mut writer, key3, &Value::Blob(&value)).unwrap();
                // store.put(&mut writer, key4, &Value::Blob(&value4)).unwrap();

                // You must commit a write transaction before the writer goes out of scope, or the
                // transaction will abort and the data won't persist.
                writer.commit().unwrap();
                let reader = env.read().expect("reader");
                let stat = store.stat(&reader).unwrap();

                println!("LMDB stat page_size : {}", stat.page_size());
                println!("LMDB stat depth : {}", stat.depth());
                println!("LMDB stat branch_pages : {}", stat.branch_pages());
                println!("LMDB stat leaf_pages : {}", stat.leaf_pages());
                println!("LMDB stat overflow_pages : {}", stat.overflow_pages());
                println!("LMDB stat entries : {}", stat.entries());
            }

            // {
            //     // Use a read transaction to query the store via a `Reader`. There can be multiple
            //     // concurrent readers for a store, and readers never block on a writer nor other
            //     // readers.
            //     let reader = env.read().expect("reader");

            //     // Keys are `AsRef<u8>`, and the return value is `Result<Option<Value>, StoreError>`.
            //     // println!("Get int {:?}", store.get(&reader, "int").unwrap());
            //     // println!("Get uint {:?}", store.get(&reader, "uint").unwrap());
            //     // println!("Get float {:?}", store.get(&reader, "float").unwrap());
            //     // println!("Get instant {:?}", store.get(&reader, "instant").unwrap());
            //     // println!("Get boolean {:?}", store.get(&reader, "boolean").unwrap());
            //     // println!("Get string {:?}", store.get(&reader, "string").unwrap());
            //     // println!("Get json {:?}", store.get(&reader, "json").unwrap());
            //     println!("Get blob {:?}", store.get(&reader, "blob").unwrap());

            //     // Retrieving a non-existent value returns `Ok(None)`.
            //     println!(
            //         "Get non-existent value {:?}",
            //         store.get(&reader, "non-existent").unwrap()
            //     );

            //     // A read transaction will automatically close once the reader goes out of scope,
            //     // so isn't necessary to close it explicitly, although you can do so by calling
            //     // `Reader.abort()`.
            // }

            // {
            //     // Aborting a write transaction rolls back the change(s).
            //     let mut writer = env.write().unwrap();
            //     store.put(&mut writer, "foo", &Value::Blob(b"bar")).unwrap();
            //     writer.abort();
            //     let reader = env.read().expect("reader");
            //     println!(
            //         "It should be None! ({:?})",
            //         store.get(&reader, "foo").unwrap()
            //     );
            // }

            // {
            //     // Explicitly aborting a transaction is not required unless an early abort is
            //     // desired, since both read and write transactions will implicitly be aborted once
            //     // they go out of scope.
            //     {
            //         let mut writer = env.write().unwrap();
            //         store.put(&mut writer, "foo", &Value::Blob(b"bar")).unwrap();
            //     }
            //     let reader = env.read().expect("reader");
            //     println!(
            //         "It should be None! ({:?})",
            //         store.get(&reader, "foo").unwrap()
            //     );
            // }

            // {
            //     // Deleting a key/value pair also requires a write transaction.
            //     let mut writer = env.write().unwrap();
            //     store.put(&mut writer, "foo", &Value::Blob(b"bar")).unwrap();
            //     store.put(&mut writer, "bar", &Value::Blob(b"baz")).unwrap();
            //     store.delete(&mut writer, "foo").unwrap();

            //     // A write transaction also supports reading, and the version of the store that it
            //     // reads includes the changes it has made regardless of the commit state of that
            //     // transaction.
            //     // In the code above, "foo" and "bar" were put into the store, then "foo" was
            //     // deleted so only "bar" will return a result when the database is queried via the
            //     // writer.
            //     println!(
            //         "It should be None! ({:?})",
            //         store.get(&writer, "foo").unwrap()
            //     );
            //     println!("Get bar ({:?})", store.get(&writer, "bar").unwrap());

            //     // But a reader won't see that change until the write transaction is committed.
            //     {
            //         let reader = env.read().expect("reader");
            //         println!("Get foo {:?}", store.get(&reader, "foo").unwrap());
            //         println!("Get bar {:?}", store.get(&reader, "bar").unwrap());
            //     }
            //     writer.commit().unwrap();
            //     {
            //         let reader = env.read().expect("reader");
            //         println!(
            //             "It should be None! ({:?})",
            //             store.get(&reader, "foo").unwrap()
            //         );
            //         println!("Get bar {:?}", store.get(&reader, "bar").unwrap());
            //     }

            //     // Committing a transaction consumes the writer, preventing you from reusing it by
            //     // failing at compile time with an error. This line would report "error[E0382]:
            //     // borrow of moved value: `writer`".
            //     // store.put(&mut writer, "baz", &Value::Str("buz")).unwrap();
            // }

            // {
            //     // Clearing all the entries in the store with a write transaction.
            //     {
            //         let mut writer = env.write().unwrap();
            //         store.put(&mut writer, "foo", &Value::Blob(b"bar")).unwrap();
            //         store.put(&mut writer, "bar", &Value::Blob(b"baz")).unwrap();
            //         writer.commit().unwrap();
            //     }

            //     // {
            //     //     let mut writer = env.write().unwrap();
            //     //     store.clear(&mut writer).unwrap();
            //     //     writer.commit().unwrap();
            //     // }

            //     // {
            //     //     let reader = env.read().expect("reader");
            //     //     println!(
            //     //         "It should be None! ({:?})",
            //     //         store.get(&reader, "foo").unwrap()
            //     //     );
            //     //     println!(
            //     //         "It should be None! ({:?})",
            //     //         store.get(&reader, "bar").unwrap()
            //     //     );
            //     // }
            // }

            let stat = env.stat().unwrap();
            let info = env.info().unwrap();
            println!("LMDB info map_size : {}", info.map_size());
            println!("LMDB info last_pgno : {}", info.last_pgno());
            println!("LMDB info last_txnid : {}", info.last_txnid());
            println!("LMDB info max_readers : {}", info.max_readers());
            println!("LMDB info num_readers : {}", info.num_readers());
            println!("LMDB stat page_size : {}", stat.page_size());
            println!("LMDB stat depth : {}", stat.depth());
            println!("LMDB stat branch_pages : {}", stat.branch_pages());
            println!("LMDB stat leaf_pages : {}", stat.leaf_pages());
            println!("LMDB stat overflow_pages : {}", stat.overflow_pages());
            println!("LMDB stat entries : {}", stat.entries());
        }
        // We reopen the env and data to see if it was well saved to disk.
        {
            let mut manager = Manager::<LmdbEnvironment>::singleton().write().unwrap();
            let shared_rkv = manager
                .get_or_create(root.path(), |path| {
                    //Rkv::new::<Lmdb>(path) // use this instead to disable encryption
                    Rkv::with_encryption_key_and_mapsize::<Lmdb>(path, key, 2 * 1024 * 1024 * 1024)
                })
                .unwrap();
            let env = shared_rkv.read().unwrap();

            println!("LMDB Version: {}", env.version());

            let store = env.open_single("testdb", StoreOptions::default()).unwrap(); //StoreOptions::create()

            {
                let reader = env.read().expect("reader");
                println!(
                    "It should be baz! ({:?})",
                    store.get(&reader, "bar").unwrap()
                );
            }
        }
        // Here the database and environment is closed, but the files are still present in the temp directory.
        // uncomment this if you need time to copy them somewhere for analysis, before the temp folder get destroyed
        //thread::sleep(Duration::from_millis(20000));
    }
}

pub fn open() {}

pub fn get() {}

pub fn put() {}
