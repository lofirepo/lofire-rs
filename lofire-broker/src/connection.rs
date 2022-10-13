//! Connection to a Broker, can be local or remote.
//! If remote, it will use a Stream and Sink of framed messages
use async_std::task;
use std::fmt::Debug;
use std::pin::Pin;

use crate::server::BrokerServer;
use async_broadcast::{broadcast, Receiver};
use async_oneshot::oneshot;
use debug_print::*;
use futures::{stream, Sink, SinkExt, Stream, StreamExt};
use lofire::object::*;
use lofire::types::*;
use lofire::utils::*;
use lofire_net::errors::*;
use lofire_net::types::*;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use xactor::{message, spawn, Actor, Addr, Context, Handler, WeakAddr};

#[message]
struct BrokerMessageXActor(BrokerMessage);

struct BrokerMessageActor {
    r: Option<async_oneshot::Receiver<BrokerMessage>>,
    s: async_oneshot::Sender<BrokerMessage>,
}

impl Actor for BrokerMessageActor {}

impl BrokerMessageActor {
    fn new() -> BrokerMessageActor {
        let (s, r) = oneshot::<BrokerMessage>();
        BrokerMessageActor { r: Some(r), s }
    }
    fn resolve(&mut self, msg: BrokerMessage) {
        self.s.send(msg).unwrap()
    }

    fn receiver(&mut self) -> async_oneshot::Receiver<BrokerMessage> {
        self.r.take().unwrap()
    }
}

#[async_trait::async_trait]
impl Handler<BrokerMessageXActor> for BrokerMessageActor {
    async fn handle(&mut self, ctx: &mut Context<Self>, msg: BrokerMessageXActor) {
        println!("handling {:?}", msg.0);
        self.resolve(msg.0);
        ctx.stop(None);
    }
}

// pub struct OverlayConnectionServer<'a, T> {
//     broker: &'a T,
// }

// impl<'a, T> OverlayConnectionServer<'a, T> {
//     pub fn sync_branch(&self) {}

//     pub fn leave(&self) {}

//     pub fn topic_connect(&self, id: TopicId) -> TopicSubscription<T> {
//         unimplemented!()
//     }

//     pub fn get_block(&self, id: BlockId) {}
// }

pub struct OverlayConnectionClient<'a, T>
where
    T: BrokerConnection,
{
    broker: &'a mut T,
    overlay: OverlayId,
    repo_link: RepoLink,
}

impl<'a, T> OverlayConnectionClient<'a, T>
where
    T: BrokerConnection,
{
    pub fn sync_branch(&self) {}

    pub fn leave(&self) {}

    pub fn topic_connect(&self, id: TopicId) -> TopicSubscription<T> {
        let (s, mut r1) = broadcast(128); // FIXME this should be done only once, in the Broker
        TopicSubscription {
            id,
            overlay_cnx: self,
            event_stream: r1.clone(),
        }
    }

    pub fn get_block(&self, id: BlockId) {}

    pub async fn put_block(&mut self, block: &Block) -> Result<BlockId, ProtocolError> {
        let res = self
            .broker
            .process_overlay_request(
                self.overlay,
                BrokerOverlayRequestContentV0::BlockPut(BlockPut::V0(block.clone())),
            )
            .await?;
        // TODO compute the ObjectId here or receive it from broker
        Ok(Digest::Blake3Digest32([9; 32]))
    }

    pub async fn put_object(
        &mut self,
        content: ObjectContent,
        deps: Vec<ObjectId>,
        expiry: Option<Timestamp>,
        max_object_size: usize,
        repo_pubkey: PubKey,
        repo_secret: SymKey,
    ) -> Result<ObjectId, ProtocolError> {
        let obj = Object::new(
            content,
            deps,
            expiry,
            max_object_size,
            repo_pubkey,
            repo_secret,
        );
        for block in obj.blocks() {
            let _ = self.put_block(block).await?;
        }
        Ok(obj.id())
    }
}

pub struct TopicSubscription<'a, T>
where
    T: BrokerConnection,
{
    id: TopicId,
    overlay_cnx: &'a OverlayConnectionClient<'a, T>,
    event_stream: Receiver<Event>,
}

impl<'a, T> TopicSubscription<'a, T>
where
    T: BrokerConnection,
{
    pub fn unsubscribe(&self) {}

    pub fn disconnect(&self) {}

    pub fn get_branch_heads(&self) {}

    pub fn get_event_stream(&self) -> &Receiver<Event> {
        &self.event_stream
    }
}

#[async_trait::async_trait]
pub trait BrokerConnection {
    type OC: BrokerConnection;

    async fn add_user(
        &mut self,
        user_id: PubKey,
        admin_user_pk: PrivKey,
    ) -> Result<(), ProtocolError>;

    async fn del_user(&mut self);

    async fn add_client(&mut self);

    async fn del_client(&mut self);

    async fn overlay_connect(
        &mut self,
        repo: &RepoLink,
        public: bool,
    ) -> Result<OverlayConnectionClient<Self::OC>, ProtocolError>;

    async fn process_overlay_request(
        &mut self,
        overlay: OverlayId,
        request: BrokerOverlayRequestContentV0,
    ) -> Result<(), ProtocolError>;
}

pub struct BrokerConnectionLocal<'a> {
    server: &'a mut BrokerServer,
    user: PubKey,
}

#[async_trait::async_trait]
impl<'a> BrokerConnection for BrokerConnectionLocal<'a> {
    type OC = BrokerConnectionLocal<'a>;

    async fn add_user(
        &mut self,
        user_id: PubKey,
        admin_user_pk: PrivKey,
    ) -> Result<(), ProtocolError> {
        let op_content = AddUserContentV0 { user: user_id };
        let sig = sign(admin_user_pk, self.user, &serde_bare::to_vec(&op_content)?)?;

        self.server.add_user(user_id, self.user, sig)
    }

    async fn process_overlay_request(
        &mut self,
        overlay: OverlayId,
        request: BrokerOverlayRequestContentV0,
    ) -> Result<(), ProtocolError> {
        Ok(())
    }

    async fn del_user(&mut self) {}

    async fn add_client(&mut self) {}

    async fn del_client(&mut self) {}

    async fn overlay_connect(
        &mut self,
        repo: &RepoLink,
        public: bool,
    ) -> Result<OverlayConnectionClient<BrokerConnectionLocal<'a>>, ProtocolError> {
        unimplemented!();
        //OverlayConnectionClient { broker: self }
    }
}

impl<'a> BrokerConnectionLocal<'a> {
    pub fn new(server: &'a mut BrokerServer, user: PubKey) -> BrokerConnectionLocal<'a> {
        BrokerConnectionLocal { server, user }
    }
}

pub struct ConnectionRemote<A, B>
where
    A: Sink<Vec<u8>, Error = ProtocolError> + Send,
    B: Stream<Item = Vec<u8>> + StreamExt + Send,
{
    writer: Option<A>, // TODO: remove. not used
    reader: Option<B>, // TODO: remove. not used
}

impl<A, B> ConnectionRemote<A, B>
where
    A: Sink<Vec<u8>, Error = ProtocolError> + Send,
    B: Stream<Item = Vec<u8>> + StreamExt + Send + 'static,
{
    pub async fn ext_request(
        w: A,
        r: B,
        request: ExtRequest,
    ) -> Result<ExtResponse, ProtocolError> {
        unimplemented!();
    }

    // FIXME return ProtocolError instead of panic via unwrap()
    pub async fn open_broker_connection(
        w: A,
        r: B,
        user: PubKey,
        user_pk: PrivKey,
        client: PubKey,
    ) -> Result<impl BrokerConnection, ProtocolError> {
        let mut writer = Box::pin(w);
        writer
            .send(serde_bare::to_vec(&StartProtocol::Auth(ClientHello::V0())).unwrap())
            .await
            .map_err(|_e| ProtocolError::CannotSend)?;

        let mut reader = Box::pin(r);
        let answer = reader.next().await;
        if answer.is_none() {
            return Err(ProtocolError::InvalidState);
        }

        let server_hello = serde_bare::from_slice::<ServerHello>(&answer.unwrap()).unwrap();

        debug_println!("received nonce from server: {:?}", server_hello.nonce());

        let content = ClientAuthContentV0 {
            user,
            client,
            nonce: server_hello.nonce().clone(),
        };

        let sig = sign(user_pk, user, &serde_bare::to_vec(&content).unwrap())
            .map_err(|_e| ProtocolError::SignatureError)?;

        let auth_ser = serde_bare::to_vec(&ClientAuth::V0(ClientAuthV0 { content, sig })).unwrap();
        debug_println!("AUTH SENT {:?}", auth_ser);
        writer
            .send(auth_ser)
            .await
            .map_err(|_e| ProtocolError::CannotSend)?;

        let answer = reader.next().await;
        if answer.is_none() {
            return Err(ProtocolError::InvalidState);
        }

        let auth_result = serde_bare::from_slice::<AuthResult>(&answer.unwrap()).unwrap();

        match auth_result.result() {
            0 => {
                async fn transform(message: BrokerMessage) -> Result<Vec<u8>, ProtocolError> {
                    Ok(serde_bare::to_vec(&message).unwrap())
                }
                let messages_stream_write = writer.with(|message| transform(message));

                let messages_stream_read = reader
                    .map(|message| serde_bare::from_slice::<BrokerMessage>(&message).unwrap());

                let cnx =
                    BrokerConnectionRemote::open(messages_stream_write, messages_stream_read, user);

                Ok(cnx)
            }
            err => Err(ProtocolError::try_from(err).unwrap()),
        }
    }
}

pub struct BrokerConnectionRemote<T, U>
where
    T: Sink<BrokerMessage> + Send,
    U: Stream<Item = BrokerMessage> + StreamExt + Send + 'static,
    <U as Stream>::Item: 'static + Debug + Send,
{
    writer: Pin<Box<T>>,
    reader: Option<U>, // TODO: remove. not used
    user: PubKey,
    actors: Arc<RwLock<HashMap<u64, WeakAddr<BrokerMessageActor>>>>,
}

#[async_trait::async_trait]
impl<T, U> BrokerConnection for BrokerConnectionRemote<T, U>
where
    T: Sink<BrokerMessage> + Send,
    U: Stream<Item = BrokerMessage> + StreamExt + Send + 'static,
    <U as Stream>::Item: 'static + Debug + Send,
{
    type OC = BrokerConnectionRemote<T, U>;

    async fn process_overlay_request(
        &mut self,
        overlay: OverlayId,
        request: BrokerOverlayRequestContentV0,
    ) -> Result<(), ProtocolError> {
        before!(self, request_id, addr, receiver);

        self.writer
            .send(BrokerMessage::V0(BrokerMessageV0 {
                padding: vec![], // FIXME implement padding
                content: BrokerMessageContentV0::BrokerOverlayMessage(BrokerOverlayMessage::V0(
                    BrokerOverlayMessageV0 {
                        overlay,
                        content: BrokerOverlayMessageContentV0::BrokerOverlayRequest(
                            BrokerOverlayRequest::V0(BrokerOverlayRequestV0 {
                                id: request_id,
                                content: request,
                            }),
                        ),
                    },
                )),
            }))
            .await
            .map_err(|_e| ProtocolError::CannotSend)?;

        after!(self, request_id, addr, receiver, reply);
        reply.into()
    }

    // FIXME return ProtocolError instead of panic via unwrap()
    async fn add_user(
        &mut self,
        user_id: PubKey,
        admin_user_pk: PrivKey,
    ) -> Result<(), ProtocolError> {
        before!(self, request_id, addr, receiver);

        let op_content = AddUserContentV0 { user: user_id };

        let sig = sign(
            admin_user_pk,
            self.user,
            &serde_bare::to_vec(&op_content).unwrap(),
        )?;

        self.writer
            .send(BrokerMessage::V0(BrokerMessageV0 {
                padding: vec![], // FIXME implement padding
                content: BrokerMessageContentV0::BrokerRequest(BrokerRequest::V0(
                    BrokerRequestV0 {
                        id: request_id,
                        content: BrokerRequestContentV0::AddUser(AddUser::V0(AddUserV0 {
                            content: op_content,
                            sig,
                        })),
                    },
                )),
            }))
            .await
            .map_err(|_e| ProtocolError::CannotSend)?;

        after!(self, request_id, addr, receiver, reply);
        reply.into()
    }

    async fn del_user(&mut self) {}

    async fn add_client(&mut self) {}

    async fn del_client(&mut self) {}

    async fn overlay_connect(
        &mut self,
        repo: &RepoLink,
        public: bool,
    ) -> Result<OverlayConnectionClient<BrokerConnectionRemote<T, U>>, ProtocolError> {
        let overlay: OverlayId = match public {
            true => Digest::Blake3Digest32(*blake3::hash(repo.id().slice()).as_bytes()),
            false => {
                let key: [u8; blake3::OUT_LEN] =
                    blake3::derive_key("LoFiRe OverlayId BLAKE3 key", repo.secret().slice());
                let keyed_hash = blake3::keyed_hash(&key, repo.id().slice());
                Digest::Blake3Digest32(*keyed_hash.as_bytes())
            }
        };

        // sending OverlayConnect
        let res = self
            .process_overlay_request(
                overlay,
                BrokerOverlayRequestContentV0::OverlayConnect(OverlayConnect::V0()),
            )
            .await;

        match res {
            Err(e) => {
                if e == ProtocolError::OverlayNotJoined {
                    debug_println!("OverlayNotJoined");
                    let res2 = self
                        .process_overlay_request(
                            overlay,
                            BrokerOverlayRequestContentV0::OverlayJoin(OverlayJoin::V0(
                                OverlayJoinV0 {
                                    secret: repo.secret(),
                                    peers: repo.peers(),
                                    repo_pubkey: None,
                                    repo_secret: None,
                                },
                            )),
                        )
                        .await?;
                } else {
                    return Err(e);
                }
            }
            Ok(()) => {}
        }

        debug_println!("OverlayConnectionClient ready");

        Ok(OverlayConnectionClient {
            broker: self,
            repo_link: repo.clone(),
            overlay,
        })
    }
}

impl<T, U> BrokerConnectionRemote<T, U>
where
    T: Sink<BrokerMessage> + Send,
    U: Stream<Item = BrokerMessage> + StreamExt + Send + 'static,
    // FIXME: the 2 static lifetimes here should be avoided. but how? try Arc<>
    // or while let Some(message) = reader.next().await { ... }  instead of reader.for_each
    <U as Stream>::Item: 'static + Debug + Send,
{
    pub fn open(writer: T, reader: U, user: PubKey) -> BrokerConnectionRemote<T, U> {
        let actors: Arc<RwLock<HashMap<u64, WeakAddr<BrokerMessageActor>>>> =
            Arc::new(RwLock::new(HashMap::new()));

        let actors_in_thread = Arc::clone(&actors);
        task::spawn((move || {
            //let actors_in_closure = Arc::clone(&actors_in_thread);
            reader.for_each(move |message| {
                let actors_in_future = Arc::clone(&actors_in_thread);
                async move {
                    debug_println!("GOT MESSAGE {:?}", message);

                    // TODO check FSM

                    if message.is_request() {
                        debug_println!("is request {}", message.id());
                        // TODO close connection. a client is not supposed to receive requests.
                    } else if message.is_response() {
                        let id = message.id();
                        debug_println!("is response {}", id);

                        {
                            let map = actors_in_future.read().expect("RwLock poisoned");
                            match map.get(&id) {
                                Some(weak_addr) => match weak_addr.upgrade() {
                                    Some(addr) => {
                                        addr.send(BrokerMessageXActor(message))
                                            .expect("sending message back to actor failed");
                                    }
                                    None => {
                                        debug_println!("ERROR. Addr is dead for ID {}", id);
                                    }
                                },
                                None => {
                                    debug_println!("ERROR. invalid ID {}", id);
                                }
                            }
                        }
                    }
                }
            })
        })());

        BrokerConnectionRemote::<T, U> {
            writer: Box::pin(writer),
            reader: None, //Some(reader),
            user,
            actors,
        }
    }

    // pub async fn run(&mut self) {
    //     self.reader
    //         .take()
    //         .unwrap()
    //         .for_each(|message| async move {
    //             debug_print!("GOT MESSAGE {:?}", message);
    //         })
    //         .await
    // }
}
