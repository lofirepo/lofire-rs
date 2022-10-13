use async_std::net::{TcpListener, TcpStream};
use async_std::task;
use async_tungstenite::accept_async;
use async_tungstenite::tungstenite::protocol::Message;
use debug_print::*;
use futures::{SinkExt, StreamExt};
use lofire_broker::server::*;
use lofire_store_lmdb::store::LmdbStore;
use std::{fs, thread};
use tempfile::Builder;

async fn connection_loop(tcp: TcpStream, mut handler: ProtocolHandler<'_>) -> std::io::Result<()> {
    let mut ws = accept_async(tcp).await.unwrap();

    while let Some(msg) = ws.next().await {
        let msg = match msg {
            Err(e) => {
                debug_println!("Error on server stream: {:?}", e);
                // Errors returned directly through the AsyncRead/Write API are fatal, generally an error on the underlying
                // transport.
                // TODO close connection
                continue;
            }
            Ok(m) => m,
        };
        if msg.is_binary() {
            debug_println!("server received binary: {:?}", msg);

            let mut replies = handler.handle_incoming(msg.into_data());
            for reply in replies {
                match reply {
                    Err(e) => {
                        // TODO deal with ProtocolErrors (close the connection?)
                        break;
                    }
                    Ok(r) => {
                        ws.send(Message::binary(r)).await.unwrap() // FIXME deal with sending errors (close the connection?)
                    }
                }
            }
        }
    }
    Ok(())
}

#[async_std::main]
async fn main() -> std::io::Result<()> {
    println!("Starting LoFiRe node daemon...");

    let root = tempfile::Builder::new()
        .prefix("node-daemon")
        .tempdir()
        .unwrap();
    let key: [u8; 32] = [0; 32];
    std::fs::create_dir_all(root.path()).unwrap();
    println!("{}", root.path().to_str().unwrap());
    let store = LmdbStore::open(root.path(), key);

    static server: BrokerServer = BrokerServer::new();

    let socket = TcpListener::bind("127.0.0.1:3012").await?;
    let mut connections = socket.incoming();
    while let Some(tcp) = connections.next().await {
        let _handle = task::spawn(connection_loop(tcp.unwrap(), server.protocol_handler()));
    }
    Ok(())
}
