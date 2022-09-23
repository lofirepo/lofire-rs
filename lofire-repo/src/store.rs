//! Object store
//!

use std::path::Path;
use std::sync::{Arc, RwLock};

use crate::types::*;
use lofire::types::*;
use rkv::backend::{Lmdb, LmdbDatabase, LmdbEnvironment};
use rkv::{Manager, Rkv, SingleStore, StoreOptions, Value};
use serde_bare::error::Error;

struct Store {
    // the main store where all the repo objects are stored
    main_store: SingleStore<LmdbDatabase>,
    // the opened environment so we can create new transactions
    environment: Arc<RwLock<Rkv<LmdbEnvironment>>>,
}

impl Store {
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

        let store = env.open_single("main", StoreOptions::create()).unwrap();

        Store {
            environment: shared_rkv.clone(),
            main_store: store,
        }
    }

    pub fn get(&self, object_id: ObjectId) -> Result<ObjectV0, Error> {
        let lock = self.environment.read().unwrap();
        let reader = lock.read().unwrap();

        let obj_id_ser = serde_bare::to_vec(&object_id).unwrap();

        let obj_ser = self.main_store.get(&reader, obj_id_ser).unwrap();

        serde_bare::from_slice::<ObjectV0>(&obj_ser.unwrap().to_bytes().unwrap())
    }

    pub fn put(&self, object: ObjectV0) -> ObjectId {
        let obj_ser = serde_bare::to_vec(&object).unwrap();

        let hash = blake3::hash(obj_ser.as_slice());
        let obj_id = Digest::Blake3Digest32(hash.as_bytes().clone());
        let obj_id_ser = serde_bare::to_vec(&obj_id).unwrap();

        let lock = self.environment.read().unwrap();
        let mut writer = lock.write().unwrap();
        self.main_store
            .put(&mut writer, obj_id_ser, &Value::Blob(obj_ser.as_slice()))
            .unwrap();

        writer.commit().unwrap();
        obj_id
    }
}
#[cfg(test)]
mod test {

    use crate::store::Store;
    use crate::types::*;
    use rkv::backend::{Lmdb, LmdbEnvironment};
    use rkv::{Manager, Rkv, StoreOptions, Value};
    #[allow(unused_imports)]
    use std::time::Duration;
    #[allow(unused_imports)]
    use std::{fs, thread};
    use tempfile::Builder;

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

        let obj_id = store.put(obj);

        println!("ObjectId: {:?}", obj_id);

        let objres = store.get(obj_id);

        println!("Object: {:?}", objres.unwrap());
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
                    //Rkv::new::<Lmdb>(path) // use this instead to disable encryption
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
                store
                    .put(&mut writer, "blob", &Value::Blob(b"blob"))
                    .unwrap();

                // You must commit a write transaction before the writer goes out of scope, or the
                // transaction will abort and the data won't persist.
                writer.commit().unwrap();
            }

            {
                // Use a read transaction to query the store via a `Reader`. There can be multiple
                // concurrent readers for a store, and readers never block on a writer nor other
                // readers.
                let reader = env.read().expect("reader");

                // Keys are `AsRef<u8>`, and the return value is `Result<Option<Value>, StoreError>`.
                // println!("Get int {:?}", store.get(&reader, "int").unwrap());
                // println!("Get uint {:?}", store.get(&reader, "uint").unwrap());
                // println!("Get float {:?}", store.get(&reader, "float").unwrap());
                // println!("Get instant {:?}", store.get(&reader, "instant").unwrap());
                // println!("Get boolean {:?}", store.get(&reader, "boolean").unwrap());
                // println!("Get string {:?}", store.get(&reader, "string").unwrap());
                // println!("Get json {:?}", store.get(&reader, "json").unwrap());
                println!("Get blob {:?}", store.get(&reader, "blob").unwrap());

                // Retrieving a non-existent value returns `Ok(None)`.
                println!(
                    "Get non-existent value {:?}",
                    store.get(&reader, "non-existent").unwrap()
                );

                // A read transaction will automatically close once the reader goes out of scope,
                // so isn't necessary to close it explicitly, although you can do so by calling
                // `Reader.abort()`.
            }

            {
                // Aborting a write transaction rolls back the change(s).
                let mut writer = env.write().unwrap();
                store.put(&mut writer, "foo", &Value::Blob(b"bar")).unwrap();
                writer.abort();
                let reader = env.read().expect("reader");
                println!(
                    "It should be None! ({:?})",
                    store.get(&reader, "foo").unwrap()
                );
            }

            {
                // Explicitly aborting a transaction is not required unless an early abort is
                // desired, since both read and write transactions will implicitly be aborted once
                // they go out of scope.
                {
                    let mut writer = env.write().unwrap();
                    store.put(&mut writer, "foo", &Value::Blob(b"bar")).unwrap();
                }
                let reader = env.read().expect("reader");
                println!(
                    "It should be None! ({:?})",
                    store.get(&reader, "foo").unwrap()
                );
            }

            {
                // Deleting a key/value pair also requires a write transaction.
                let mut writer = env.write().unwrap();
                store.put(&mut writer, "foo", &Value::Blob(b"bar")).unwrap();
                store.put(&mut writer, "bar", &Value::Blob(b"baz")).unwrap();
                store.delete(&mut writer, "foo").unwrap();

                // A write transaction also supports reading, and the version of the store that it
                // reads includes the changes it has made regardless of the commit state of that
                // transaction.
                // In the code above, "foo" and "bar" were put into the store, then "foo" was
                // deleted so only "bar" will return a result when the database is queried via the
                // writer.
                println!(
                    "It should be None! ({:?})",
                    store.get(&writer, "foo").unwrap()
                );
                println!("Get bar ({:?})", store.get(&writer, "bar").unwrap());

                // But a reader won't see that change until the write transaction is committed.
                {
                    let reader = env.read().expect("reader");
                    println!("Get foo {:?}", store.get(&reader, "foo").unwrap());
                    println!("Get bar {:?}", store.get(&reader, "bar").unwrap());
                }
                writer.commit().unwrap();
                {
                    let reader = env.read().expect("reader");
                    println!(
                        "It should be None! ({:?})",
                        store.get(&reader, "foo").unwrap()
                    );
                    println!("Get bar {:?}", store.get(&reader, "bar").unwrap());
                }

                // Committing a transaction consumes the writer, preventing you from reusing it by
                // failing at compile time with an error. This line would report "error[E0382]:
                // borrow of moved value: `writer`".
                // store.put(&mut writer, "baz", &Value::Str("buz")).unwrap();
            }

            {
                // Clearing all the entries in the store with a write transaction.
                {
                    let mut writer = env.write().unwrap();
                    store.put(&mut writer, "foo", &Value::Blob(b"bar")).unwrap();
                    store.put(&mut writer, "bar", &Value::Blob(b"baz")).unwrap();
                    writer.commit().unwrap();
                }

                // {
                //     let mut writer = env.write().unwrap();
                //     store.clear(&mut writer).unwrap();
                //     writer.commit().unwrap();
                // }

                // {
                //     let reader = env.read().expect("reader");
                //     println!(
                //         "It should be None! ({:?})",
                //         store.get(&reader, "foo").unwrap()
                //     );
                //     println!(
                //         "It should be None! ({:?})",
                //         store.get(&reader, "bar").unwrap()
                //     );
                // }
            }
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
