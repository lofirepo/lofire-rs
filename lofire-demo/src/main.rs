use async_tungstenite::async_std::connect_async;
use async_tungstenite::client_async;
use async_tungstenite::tungstenite::{Error, Message};
use debug_print::*;
use futures::{future, pin_mut, stream, SinkExt, StreamExt};
use lofire::store::{store_max_value_size, store_valid_value_size};
use lofire_broker::config::ConfigMode;
use lofire_store_lmdb::brokerstore::LmdbBrokerStore;
use lofire_store_lmdb::repostore::LmdbRepoStore;
use std::thread;

use lofire::types::*;
use lofire::utils::{generate_keypair, now_timestamp};
use lofire_broker::connection::*;
use lofire_broker::server::*;
use lofire_net::errors::*;
use lofire_net::types::*;

fn block_size() -> usize {
    store_max_value_size()
    //store_valid_value_size(0)
}

async fn test(cnx: &mut impl BrokerConnection, pub_key: PubKey, priv_key: PrivKey) {
    let _ = cnx.add_user(PubKey::Ed25519PubKey([1; 32]), priv_key).await;

    let _ = cnx.add_user(pub_key, priv_key).await;
    //.expect("add_user 2 (myself) failed");

    assert_eq!(
        cnx.add_user(PubKey::Ed25519PubKey([1; 32]), priv_key)
            .await
            .err()
            .unwrap(),
        ProtocolError::UserAlreadyExists
    );

    let repo = RepoLink::V0(RepoLinkV0 {
        id: PubKey::Ed25519PubKey([1; 32]),
        secret: SymKey::ChaCha20Key([0; 32]),
        peers: vec![],
    });
    let mut public_overlay_cnx = cnx
        .overlay_connect(&repo, true)
        .await
        .expect("overlay_connect failed");

    let my_block_id = public_overlay_cnx
        .put_block(&Block::new(
            vec![],
            ObjectDeps::ObjectIdList(vec![]),
            None,
            vec![27; 150],
            None,
        ))
        .await
        .expect("put_block failed");

    debug_println!("added block_id to store {}", my_block_id);

    let object_id = public_overlay_cnx
        .put_object(
            ObjectContent::File(File::V0(FileV0 {
                content_type: vec![],
                metadata: vec![],
                content: vec![48; 69000],
            })),
            vec![],
            None,
            block_size(),
            repo.id(),
            repo.secret(),
        )
        .await
        .expect("put_object failed");

    debug_println!("added object_id to store {}", object_id);

    let mut my_block_stream = public_overlay_cnx
        .get_block(my_block_id, true, None)
        .await
        .expect("get_block failed");

    while let Some(b) = my_block_stream.next().await {
        debug_println!("GOT BLOCK {}", b.id());
    }

    let mut my_object_stream = public_overlay_cnx
        .get_block(object_id, true, None)
        .await
        .expect("get_block for object failed");

    while let Some(b) = my_object_stream.next().await {
        debug_println!("GOT BLOCK {}", b.id());
    }

    let object = public_overlay_cnx
        .get_object(object_id, None)
        .await
        .expect("get_object failed");

    debug_println!("GOT OBJECT with ID {}", object.id());

    // let object_id = public_overlay_cnx
    //     .copy_object(object_id, Some(now_timestamp() + 60))
    //     .await
    //     .expect("copy_object failed");

    // debug_println!("COPIED OBJECT to OBJECT ID {}", object_id);

    public_overlay_cnx
        .delete_object(object_id)
        .await
        .expect("delete_object failed");

    let res = public_overlay_cnx
        .get_object(object_id, None)
        .await
        .unwrap_err();
    debug_println!("result from get object after delete: {}", res);

    //TODO test pin/unpin
}

async fn test_local_connection() {
    debug_println!("===== TESTING LOCAL API =====");

    let root = tempfile::Builder::new()
        .prefix("node-daemon")
        .tempdir()
        .unwrap();
    let master_key: [u8; 32] = [0; 32];
    std::fs::create_dir_all(root.path()).unwrap();
    println!("{}", root.path().to_str().unwrap());
    let store = LmdbBrokerStore::open(root.path(), master_key);

    let mut server = BrokerServer::new(store, ConfigMode::Local).expect("starting broker");

    let (priv_key, pub_key) = generate_keypair();

    let mut cnx = server.local_connection(pub_key);

    test(&mut cnx, pub_key, priv_key).await;
}

#[xactor::main]
async fn main() -> std::io::Result<()> {
    debug_println!("Starting LoFiRe app demo...");

    test_local_connection().await;

    debug_println!("===== TESTING REMOTE API =====");

    let res = connect_async("ws://127.0.0.1:3012").await;

    match (res) {
        Ok((ws, _)) => {
            debug_println!("WebSocket handshake completed");

            let (write, read) = ws.split();
            let mut frames_stream_read = read.map(|msg_res| match msg_res {
                Err(e) => {
                    debug_println!("ERROR {:?}", e);
                    vec![]
                }
                Ok(message) => {
                    if message.is_close() {
                        vec![]
                    } else {
                        message.into_data()
                    }
                }
            });
            async fn transform(message: Vec<u8>) -> Result<Message, Error> {
                if message.len() == 0 {
                    Ok(Message::Close(None))
                } else {
                    Ok(Message::binary(message))
                }
            }
            let frames_stream_write = write
                .with(|message| transform(message))
                .sink_map_err(|e| ProtocolError::WriteError);

            let (priv_key, pub_key) = generate_keypair();
            let master_key: [u8; 32] = [0; 32];
            let mut cnx_res = ConnectionRemote::open_broker_connection(
                frames_stream_write,
                frames_stream_read,
                pub_key,
                priv_key,
                PubKey::Ed25519PubKey([1; 32]),
            )
            .await;

            match cnx_res {
                Ok(mut cnx) => {
                    test(&mut cnx, pub_key, priv_key).await;
                    cnx.close().await;
                }
                Err(e) => {
                    debug_println!("cannot connect {:?}", e);
                }
            }
        }
        Err(e) => {
            debug_println!("Cannot connect: {:?}", e);
        }
    }

    Ok(())
}
