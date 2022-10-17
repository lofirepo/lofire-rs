use async_std::net::{TcpListener, TcpStream};
use async_std::sync::Mutex;
use async_std::task;
use async_tungstenite::accept_async;
use async_tungstenite::tungstenite::protocol::Message;
use debug_print::*;
use futures::{SinkExt, StreamExt};
use lofire_broker::config::ConfigMode;
use lofire_broker::server::*;
use lofire_store_lmdb::brokerstore::LmdbBrokerStore;
use lofire_store_lmdb::repostore::LmdbRepoStore;
use std::fs;
use std::sync::Arc;
use tempfile::Builder;

async fn connection_loop(tcp: TcpStream, mut handler: ProtocolHandler) -> std::io::Result<()> {
    let mut ws = accept_async(tcp).await.unwrap();
    let (mut tx, mut rx) = ws.split();

    let mut tx_mutex = Arc::new(Mutex::new(tx));

    // setup the async frames task
    let receiver = handler.async_frames_receiver();
    let ws_in_task = Arc::clone(&tx_mutex);
    task::spawn(async move {
        while let Ok(frame) = receiver.recv().await {
            if ws_in_task
                .lock()
                .await
                .send(Message::binary(frame))
                .await
                .is_err()
            {
                //deal with sending errors (close the connection)
                break;
            }
        }
        debug_println!("end of async frames loop");

        let mut lock = ws_in_task.lock().await;
        let _ = lock.send(Message::Close(None)).await;
        let _ = lock.close();
    });

    while let Some(msg) = rx.next().await {
        let msg = match msg {
            Err(e) => {
                debug_println!("Error on server stream: {:?}", e);
                // Errors returned directly through the AsyncRead/Write API are fatal, generally an error on the underlying
                // transport.
                // TODO close connection
                break;
            }
            Ok(m) => m,
        };
        //TODO implement PING and CLOSE messages
        if msg.is_close() {
            debug_println!("CLOSE from client");
            break;
        } else if msg.is_binary() {
            //debug_println!("server received binary: {:?}", msg);

            let replies = handler.handle_incoming(msg.into_data()).await;

            match replies.0 {
                Err(e) => {
                    debug_println!("Protocol Error: {:?}", e);
                    // dealing with ProtocolErrors (close the connection)
                    break;
                }
                Ok(r) => {
                    if tx_mutex
                        .lock()
                        .await
                        .send(Message::binary(r))
                        .await
                        .is_err()
                    {
                        //deaingl with sending errors (close the connection)
                        break;
                    }
                }
            }
            match replies.1.await {
                Some(errcode) => {
                    if errcode > 0 {
                        debug_println!("Close due to error code : {:?}", errcode);
                        //close connection
                        break;
                    }
                }
                None => {}
            }
        }
    }
    let mut lock = tx_mutex.lock().await;
    let _ = lock.send(Message::Close(None)).await;
    let _ = lock.close();
    debug_println!("end of sync read+write loop");
    Ok(())
}

#[async_std::main]
async fn main() -> std::io::Result<()> {
    println!("Starting LoFiRe node daemon...");

    let root = tempfile::Builder::new()
        .prefix("node-daemon")
        .tempdir()
        .unwrap();
    let master_key: [u8; 32] = [0; 32];
    std::fs::create_dir_all(root.path()).unwrap();
    println!("{}", root.path().to_str().unwrap());
    let store = LmdbBrokerStore::open(root.path(), master_key);

    let server: BrokerServer =
        BrokerServer::new(store, ConfigMode::Local).expect("starting broker");

    let socket = TcpListener::bind("127.0.0.1:3012").await?;
    let mut connections = socket.incoming();
    let server_arc = Arc::new(server);
    while let Some(tcp) = connections.next().await {
        let proto_handler = Arc::clone(&server_arc).protocol_handler();
        let _handle = task::spawn(connection_loop(tcp.unwrap(), proto_handler));
    }
    Ok(())
}
