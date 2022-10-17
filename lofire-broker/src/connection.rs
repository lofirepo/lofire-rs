//! Connection to a Broker, can be local or remote.
//! If remote, it will use a Stream and Sink of framed messages
use async_std::task;
use futures::{
    ready,
    stream::Stream,
    task::{Context, Poll},
    Future,
};
use std::pin::Pin;
use std::{collections::HashSet, fmt::Debug};

use crate::server::BrokerServer;
use async_broadcast::{broadcast, Receiver};
use async_oneshot::oneshot;
use debug_print::*;
use futures::{pin_mut, stream, Sink, SinkExt, StreamExt};
use lofire::object::*;
use lofire::store::*;
use lofire::types::*;
use lofire::utils::*;
use lofire_net::errors::*;
use lofire_net::types::*;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use xactor::{message, spawn, Actor, Addr, Handler, WeakAddr};

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

struct BrokerMessageStreamActor {
    r: Option<async_channel::Receiver<Block>>,
    s: async_channel::Sender<Block>,
    error_r: Option<async_oneshot::Receiver<Option<ProtocolError>>>,
    error_s: Option<async_oneshot::Sender<Option<ProtocolError>>>,
}

impl Actor for BrokerMessageStreamActor {}

impl BrokerMessageStreamActor {
    fn new() -> BrokerMessageStreamActor {
        let (s, r) = async_channel::unbounded::<Block>();
        let (error_s, error_r) = oneshot::<Option<ProtocolError>>();
        BrokerMessageStreamActor {
            r: Some(r),
            s,
            error_r: Some(error_r),
            error_s: Some(error_s),
        }
    }
    async fn partial(&mut self, block: Block) -> Result<(), ProtocolError> {
        //debug_println!("GOT PARTIAL {:?}", block.id());
        self.s
            .send(block)
            .await
            .map_err(|e| ProtocolError::CannotSend)
    }

    fn receiver(&mut self) -> async_channel::Receiver<Block> {
        self.r.take().unwrap()
    }

    fn error_receiver(&mut self) -> async_oneshot::Receiver<Option<ProtocolError>> {
        self.error_r.take().unwrap()
    }

    fn send_error(&mut self, err: Option<ProtocolError>) {
        if self.error_s.is_some() {
            let _ = self.error_s.take().unwrap().send(err);
            self.error_s = None;
        }
    }

    fn close(&mut self) {
        self.s.close();
    }
}

#[async_trait::async_trait]
impl Handler<BrokerMessageXActor> for BrokerMessageActor {
    async fn handle(&mut self, ctx: &mut xactor::Context<Self>, msg: BrokerMessageXActor) {
        //println!("handling {:?}", msg.0);
        self.resolve(msg.0);
        ctx.stop(None);
    }
}

#[async_trait::async_trait]
impl Handler<BrokerMessageXActor> for BrokerMessageStreamActor {
    async fn handle(&mut self, ctx: &mut xactor::Context<Self>, msg: BrokerMessageXActor) {
        //println!("handling {:?}", msg.0);
        let res: Result<Option<Block>, ProtocolError> = msg.0.into();
        match res {
            Err(e) => {
                self.send_error(Some(e));
                ctx.stop(None);
                self.close();
            }
            Ok(Some(b)) => {
                self.send_error(None);
                // it must be a partial content
                let res = self.partial(b).await;
                if let Err(e) = res {
                    ctx.stop(None);
                    self.close();
                }
            }
            Ok(None) => {
                self.send_error(None);
                ctx.stop(None);
                self.close();
            }
        }
    }
}

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
    pub fn overlay(repo_link: &RepoLink, public: bool) -> OverlayId {
        let overlay: OverlayId = match public {
            true => Digest::Blake3Digest32(*blake3::hash(repo_link.id().slice()).as_bytes()),
            false => {
                let key: [u8; blake3::OUT_LEN] =
                    blake3::derive_key("LoFiRe OverlayId BLAKE3 key", repo_link.secret().slice());
                let keyed_hash = blake3::keyed_hash(&key, repo_link.id().slice());
                Digest::Blake3Digest32(*keyed_hash.as_bytes())
            }
        };
        overlay
    }

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

    pub async fn delete_object(&mut self, id: ObjectId) -> Result<(), ProtocolError> {
        self.broker
            .process_overlay_request(
                self.overlay,
                BrokerOverlayRequestContentV0::ObjectDel(ObjectDel::V0(ObjectDelV0 { id })),
            )
            .await
    }

    pub async fn pin_object(&mut self, id: ObjectId) -> Result<(), ProtocolError> {
        self.broker
            .process_overlay_request(
                self.overlay,
                BrokerOverlayRequestContentV0::ObjectPin(ObjectPin::V0(ObjectPinV0 { id })),
            )
            .await
    }

    pub async fn unpin_object(&mut self, id: ObjectId) -> Result<(), ProtocolError> {
        self.broker
            .process_overlay_request(
                self.overlay,
                BrokerOverlayRequestContentV0::ObjectUnpin(ObjectUnpin::V0(ObjectUnpinV0 { id })),
            )
            .await
    }

    pub async fn copy_object(
        &mut self,
        id: ObjectId,
        expiry: Option<Timestamp>,
    ) -> Result<ObjectId, ProtocolError> {
        self.broker
            .process_overlay_request_objectid_response(
                self.overlay,
                BrokerOverlayRequestContentV0::ObjectCopy(ObjectCopy::V0(ObjectCopyV0 {
                    id,
                    expiry,
                })),
            )
            .await
    }

    pub async fn get_block(
        &mut self,
        id: BlockId,
        include_children: bool,
        topic: Option<PubKey>,
    ) -> Result<Pin<Box<T::BlockStream>>, ProtocolError> {
        self.broker
            .process_overlay_request_stream_response(
                self.overlay,
                BrokerOverlayRequestContentV0::BlockGet(BlockGet::V0(BlockGetV0 {
                    id,
                    include_children,
                    topic,
                })),
            )
            .await
    }

    pub async fn get_object(
        &mut self,
        id: ObjectId,
        topic: Option<PubKey>,
    ) -> Result<Object, ProtocolError> {
        let mut blockstream = self.get_block(id, true, topic).await?;
        let mut store = HashMapRepoStore::new();
        while let Some(block) = blockstream.next().await {
            store.put(&block).unwrap();
        }
        Object::load(id, None, &store).map_err(|e| match e {
            ObjectParseError::MissingBlocks(_missing) => ProtocolError::MissingBlocks,
            _ => ProtocolError::ObjectParseError,
        })
    }

    pub async fn put_block(&mut self, block: &Block) -> Result<BlockId, ProtocolError> {
        self.broker
            .process_overlay_request(
                self.overlay,
                BrokerOverlayRequestContentV0::BlockPut(BlockPut::V0(block.clone())),
            )
            .await?;
        Ok(block.id())
    }

    // TODO maybe implement a put_block_with_children ? that would behave like put_object, but taking in a parent Blockk instead of a content

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
        debug_println!("object has {} blocks", obj.blocks().len());
        let mut deduplicated: HashSet<ObjectId> = HashSet::new();
        for block in obj.blocks() {
            let id = block.id();
            if deduplicated.get(&id).is_none() {
                let _ = self.put_block(block).await?;
                deduplicated.insert(id);
            }
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
    type BlockStream: Stream<Item = Block>;

    async fn close(&mut self);

    async fn add_user(
        &mut self,
        user_id: PubKey,
        admin_user_pk: PrivKey,
    ) -> Result<(), ProtocolError>;

    async fn del_user(&mut self, user_id: PubKey, admin_user_pk: PrivKey);

    async fn add_client(&mut self, client_id: ClientId, user_pk: PrivKey);

    async fn del_client(&mut self, client_id: ClientId, user_pk: PrivKey);

    async fn overlay_connect(
        &mut self,
        repo: &RepoLink,
        public: bool,
    ) -> Result<OverlayConnectionClient<Self::OC>, ProtocolError>;

    // TODO: remove those 3 functions from trait. they are used internally only. should not be exposed to end-user
    async fn process_overlay_request(
        &mut self,
        overlay: OverlayId,
        request: BrokerOverlayRequestContentV0,
    ) -> Result<(), ProtocolError>;

    async fn process_overlay_request_stream_response(
        &mut self,
        overlay: OverlayId,
        request: BrokerOverlayRequestContentV0,
    ) -> Result<Pin<Box<Self::BlockStream>>, ProtocolError>;

    async fn process_overlay_request_objectid_response(
        &mut self,
        overlay: OverlayId,
        request: BrokerOverlayRequestContentV0,
    ) -> Result<ObjectId, ProtocolError>;

    async fn process_overlay_connect(
        &mut self,
        repo_link: &RepoLink,
        public: bool,
    ) -> Result<OverlayId, ProtocolError> {
        let overlay: OverlayId = match public {
            true => Digest::Blake3Digest32(*blake3::hash(repo_link.id().slice()).as_bytes()),
            false => {
                let key: [u8; blake3::OUT_LEN] =
                    blake3::derive_key("LoFiRe OverlayId BLAKE3 key", repo_link.secret().slice());
                let keyed_hash = blake3::keyed_hash(&key, repo_link.id().slice());
                Digest::Blake3Digest32(*keyed_hash.as_bytes())
            }
        };

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
                                    secret: repo_link.secret(),
                                    peers: repo_link.peers(),
                                    repo_pubkey: Some(repo_link.id()), //TODO if we know we are connecting to a core node, we can pass None here
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
        Ok(overlay)
    }
}

pub struct BrokerConnectionLocal<'a> {
    broker: &'a mut BrokerServer,
    user: PubKey,
}

#[async_trait::async_trait]
impl<'a> BrokerConnection for BrokerConnectionLocal<'a> {
    type OC = BrokerConnectionLocal<'a>;
    type BlockStream = async_channel::Receiver<Block>;

    async fn close(&mut self) {}

    async fn add_user(
        &mut self,
        user_id: PubKey,
        admin_user_pk: PrivKey,
    ) -> Result<(), ProtocolError> {
        let op_content = AddUserContentV0 { user: user_id };
        let sig = sign(admin_user_pk, self.user, &serde_bare::to_vec(&op_content)?)?;

        self.broker.add_user(self.user, user_id, sig)
    }

    async fn process_overlay_request(
        &mut self,
        overlay: OverlayId,
        request: BrokerOverlayRequestContentV0,
    ) -> Result<(), ProtocolError> {
        match request {
            BrokerOverlayRequestContentV0::OverlayConnect(_) => {
                self.broker.overlay_connect(self.user, overlay)
            }
            BrokerOverlayRequestContentV0::OverlayJoin(j) => {
                self.broker
                    .overlay_join(self.user, overlay, j.repo_pubkey(), j.secret(), j.peers())
            }
            BrokerOverlayRequestContentV0::ObjectPin(op) => {
                self.broker.pin_object(self.user, overlay, op.id())
            }
            BrokerOverlayRequestContentV0::ObjectUnpin(op) => {
                self.broker.unpin_object(self.user, overlay, op.id())
            }
            BrokerOverlayRequestContentV0::ObjectDel(op) => {
                self.broker.del_object(self.user, overlay, op.id())
            }
            BrokerOverlayRequestContentV0::BlockPut(b) => {
                self.broker.block_put(self.user, overlay, b.block())
            }
            _ => Err(ProtocolError::InvalidState),
        }
    }

    async fn process_overlay_request_objectid_response(
        &mut self,
        overlay: OverlayId,
        request: BrokerOverlayRequestContentV0,
    ) -> Result<ObjectId, ProtocolError> {
        match request {
            BrokerOverlayRequestContentV0::ObjectCopy(oc) => {
                self.broker
                    .copy_object(self.user, overlay, oc.id(), oc.expiry())
            }
            _ => Err(ProtocolError::InvalidState),
        }
    }

    async fn process_overlay_request_stream_response(
        &mut self,
        overlay: OverlayId,
        request: BrokerOverlayRequestContentV0,
    ) -> Result<Pin<Box<Self::BlockStream>>, ProtocolError> {
        match request {
            // TODO BranchSyncReq
            BrokerOverlayRequestContentV0::BlockGet(b) => self
                .broker
                .block_get(self.user, overlay, b.id(), b.include_children(), b.topic())
                .map(|r| Box::pin(r)),
            _ => Err(ProtocolError::InvalidState),
        }
    }

    async fn del_user(&mut self, user_id: PubKey, admin_user_pk: PrivKey) {}

    async fn add_client(&mut self, user_id: PubKey, admin_user_pk: PrivKey) {}

    async fn del_client(&mut self, user_id: PubKey, admin_user_pk: PrivKey) {}

    async fn overlay_connect(
        &mut self,
        repo_link: &RepoLink,
        public: bool,
    ) -> Result<OverlayConnectionClient<BrokerConnectionLocal<'a>>, ProtocolError> {
        let overlay = self.process_overlay_connect(repo_link, public).await?;
        Ok(OverlayConnectionClient {
            broker: self,
            repo_link: repo_link.clone(),
            overlay,
        })
    }
}

impl<'a> BrokerConnectionLocal<'a> {
    pub fn new(broker: &'a mut BrokerServer, user: PubKey) -> BrokerConnectionLocal<'a> {
        BrokerConnectionLocal { broker, user }
    }
}

pub struct ConnectionRemote {}

impl ConnectionRemote {
    pub async fn ext_request<
        B: Stream<Item = Vec<u8>> + StreamExt + Send + Sync,
        A: Sink<Vec<u8>, Error = ProtocolError> + Send,
    >(
        w: A,
        r: B,
        request: ExtRequest,
    ) -> Result<ExtResponse, ProtocolError> {
        unimplemented!();
    }

    async fn close<S>(w: S, err: ProtocolError) -> ProtocolError
    where
        S: Sink<Vec<u8>, Error = ProtocolError>,
    {
        let mut writer = Box::pin(w);
        let _ = writer.send(vec![]);
        let _ = writer.close();
        err
    }

    // FIXME return ProtocolError instead of panic via unwrap()
    pub async fn open_broker_connection<
        B: Stream<Item = Vec<u8>> + StreamExt + Send + Sync + 'static,
        A: Sink<Vec<u8>, Error = ProtocolError> + Send,
    >(
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
            return Err(Self::close(writer, ProtocolError::InvalidState).await);
        }

        let server_hello = serde_bare::from_slice::<ServerHello>(&answer.unwrap()).unwrap();

        //debug_println!("received nonce from server: {:?}", server_hello.nonce());

        let content = ClientAuthContentV0 {
            user,
            client,
            nonce: server_hello.nonce().clone(),
        };

        let sig = sign(user_pk, user, &serde_bare::to_vec(&content).unwrap())
            .map_err(|_e| ProtocolError::SignatureError)?;

        let auth_ser = serde_bare::to_vec(&ClientAuth::V0(ClientAuthV0 { content, sig })).unwrap();
        //debug_println!("AUTH SENT {:?}", auth_ser);
        writer
            .send(auth_ser)
            .await
            .map_err(|_e| ProtocolError::CannotSend)?;

        let answer = reader.next().await;
        if answer.is_none() {
            //return Err(ProtocolError::InvalidState);
            return Err(Self::close(writer, ProtocolError::InvalidState).await);
        }

        let auth_result = serde_bare::from_slice::<AuthResult>(&answer.unwrap()).unwrap();

        match auth_result.result() {
            0 => {
                async fn transform(message: BrokerMessage) -> Result<Vec<u8>, ProtocolError> {
                    if message.is_close() {
                        Ok(vec![])
                    } else {
                        Ok(serde_bare::to_vec(&message).unwrap())
                    }
                }
                let messages_stream_write = writer.with(|message| transform(message));

                let mut messages_stream_read = reader.map(|message| {
                    if message.len() == 0 {
                        BrokerMessage::Close
                    } else {
                        serde_bare::from_slice::<BrokerMessage>(&message).unwrap()
                    }
                });

                let cnx =
                    BrokerConnectionRemote::open(messages_stream_write, messages_stream_read, user);

                Ok(cnx)
            }
            err => Err(Self::close(writer, ProtocolError::try_from(err).unwrap()).await),
        }
    }
}

pub struct BrokerConnectionRemote<T>
where
    T: Sink<BrokerMessage> + Send,
{
    writer: Pin<Box<T>>,
    user: PubKey,
    actors: Arc<RwLock<HashMap<u64, WeakAddr<BrokerMessageActor>>>>,
    stream_actors: Arc<RwLock<HashMap<u64, WeakAddr<BrokerMessageStreamActor>>>>,
}

#[async_trait::async_trait]
impl<T> BrokerConnection for BrokerConnectionRemote<T>
where
    T: Sink<BrokerMessage> + Send,
{
    type OC = BrokerConnectionRemote<T>;
    type BlockStream = async_channel::Receiver<Block>;

    async fn close(&mut self) {
        let _ = self.writer.send(BrokerMessage::Close).await;
        let _ = self.writer.close();
    }

    async fn process_overlay_request_stream_response(
        &mut self,
        overlay: OverlayId,
        request: BrokerOverlayRequestContentV0,
    ) -> Result<Pin<Box<Self::BlockStream>>, ProtocolError> {
        let mut actor = BrokerMessageStreamActor::new();
        let receiver = actor.receiver();
        let error_receiver = actor.error_receiver();
        let mut addr = actor
            .start()
            .await
            .map_err(|_e| ProtocolError::ActorError)?;

        let request_id = addr.actor_id();
        //debug_println!("actor ID {}", request_id);

        {
            let mut map = self.stream_actors.write().expect("RwLock poisoned");
            map.insert(request_id, addr.downgrade());
        }

        self.writer
            .send(BrokerMessage::V0(BrokerMessageV0 {
                padding: vec![], //FIXME implement padding
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

        //debug_println!("waiting for first reply");
        let reply = error_receiver.await.unwrap();
        match reply {
            Some(e) => {
                let mut map = self.stream_actors.write().expect("RwLock poisoned");
                map.remove(&request_id);
                return Err(e);
            }
            None => {
                let stream_actors_in_thread = Arc::clone(&self.stream_actors);
                task::spawn(async move {
                    addr.wait_for_stop().await; // TODO add timeout
                    let mut map = stream_actors_in_thread.write().expect("RwLock poisoned");
                    map.remove(&request_id);
                });

                Ok(Box::pin(receiver))
            }
        }
    }

    async fn process_overlay_request_objectid_response(
        &mut self,
        overlay: OverlayId,
        request: BrokerOverlayRequestContentV0,
    ) -> Result<ObjectId, ProtocolError> {
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

    async fn del_user(&mut self, user_id: PubKey, admin_user_pk: PrivKey) {}

    async fn add_client(&mut self, client_id: ClientId, user_pk: PrivKey) {}

    async fn del_client(&mut self, client_id: ClientId, user_pk: PrivKey) {}

    async fn overlay_connect(
        &mut self,
        repo_link: &RepoLink,
        public: bool,
    ) -> Result<OverlayConnectionClient<BrokerConnectionRemote<T>>, ProtocolError> {
        let overlay = self.process_overlay_connect(repo_link, public).await?;

        Ok(OverlayConnectionClient {
            broker: self,
            repo_link: repo_link.clone(),
            overlay,
        })
    }
}

impl<T> BrokerConnectionRemote<T>
where
    T: Sink<BrokerMessage> + Send,
{
    async fn connection_reader_loop<
        U: Stream<Item = BrokerMessage> + StreamExt + Send + Sync + Unpin + 'static,
    >(
        stream: U,
        actors: Arc<RwLock<HashMap<u64, WeakAddr<BrokerMessageActor>>>>,
        stream_actors: Arc<RwLock<HashMap<u64, WeakAddr<BrokerMessageStreamActor>>>>,
    ) -> Result<(), ProtocolError> {
        let mut s = stream;
        while let Some(message) = s.next().await {
            //debug_println!("GOT MESSAGE {:?}", message);

            if message.is_close() {
                return Err(ProtocolError::Closing);
            }

            // TODO check FSM

            if message.is_request() {
                debug_println!("is request {}", message.id());
                return Err(ProtocolError::Closing);
                // close connection. a client is not supposed to receive requests.
            } else if message.is_response() {
                let id = message.id();
                //debug_println!("is response for {}", id);
                {
                    let map = actors.read().expect("RwLock poisoned");
                    match map.get(&id) {
                        Some(weak_addr) => match weak_addr.upgrade() {
                            Some(addr) => {
                                addr.send(BrokerMessageXActor(message))
                                    .map_err(|e| ProtocolError::Closing)?
                                //.expect("sending message back to actor failed");
                            }
                            None => {
                                debug_println!("ERROR. Addr is dead for ID {}", id);
                                return Err(ProtocolError::Closing);
                            }
                        },
                        None => {
                            let map2 = stream_actors.read().expect("RwLock poisoned");
                            match map2.get(&id) {
                                Some(weak_addr) => match weak_addr.upgrade() {
                                    Some(addr) => {
                                        addr.send(BrokerMessageXActor(message))
                                            .map_err(|e| ProtocolError::Closing)?
                                        //.expect("sending message back to stream actor failed");
                                    }
                                    None => {
                                        debug_println!(
                                            "ERROR. Addr is dead for ID {} {:?}",
                                            id,
                                            message
                                        );
                                        return Err(ProtocolError::Closing);
                                    }
                                },
                                None => {
                                    debug_println!("Actor ID not found {} {:?}", id, message);
                                    return Err(ProtocolError::Closing);
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub fn open<U: Stream<Item = BrokerMessage> + StreamExt + Send + Sync + Unpin + 'static>(
        writer: T,
        reader: U,
        user: PubKey,
    ) -> BrokerConnectionRemote<T> {
        let actors: Arc<RwLock<HashMap<u64, WeakAddr<BrokerMessageActor>>>> =
            Arc::new(RwLock::new(HashMap::new()));

        let stream_actors: Arc<RwLock<HashMap<u64, WeakAddr<BrokerMessageStreamActor>>>> =
            Arc::new(RwLock::new(HashMap::new()));

        let actors_in_thread = Arc::clone(&actors);
        let stream_actors_in_thread = Arc::clone(&stream_actors);
        task::spawn(async move {
            if let Err(e) =
                Self::connection_reader_loop(reader, actors_in_thread, stream_actors_in_thread)
                    .await
            {
                debug_println!("closing {}", e);
                // TODO close
                //Box::pin(writer).close();
            }
            debug_println!("end of reader loop");
        });

        BrokerConnectionRemote::<T> {
            writer: Box::pin(writer),
            user,
            actors: Arc::clone(&actors),
            stream_actors: Arc::clone(&stream_actors),
        }
    }
}
