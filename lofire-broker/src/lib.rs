#[macro_export]
macro_rules! before {
    ( $self:expr, $request_id:ident, $addr:ident, $receiver:ident ) => {
        let mut actor = BrokerMessageActor::new();
        let $receiver = actor.receiver();
        let mut $addr = actor
            .start()
            .await
            .map_err(|_e| ProtocolError::ActorError)?;

        let $request_id = $addr.actor_id();
        debug_println!("actor ID {}", $request_id);

        {
            let mut map = $self.actors.write().expect("RwLock poisoned");
            map.insert($request_id, $addr.downgrade());
        }
    };
}

macro_rules! after {
    ( $self:expr, $request_id:ident, $addr:ident, $receiver:ident, $reply:ident ) => {
        debug_println!("waiting for reply");

        $addr.wait_for_stop().await; // TODO add timeout and close connection if there's no reply
        let $reply = $receiver.await.unwrap();

        debug_println!("reply arrived {:?}", $reply);
        {
            let mut map = $self.actors.write().expect("RwLock poisoned");
            map.remove(&$request_id);
        }
    };
}

pub mod account;

pub mod overlay;

pub mod topic;

pub mod connection;

pub mod server;

pub mod auth;
