//! A Broker server

use std::sync::Arc;

use crate::auth::*;
use crate::connection::BrokerConnectionLocal;
use debug_print::*;
use futures::Stream;
use lofire::store::Store;
use lofire::types::*;
use lofire::utils::*;
use lofire_net::errors::*;
use lofire_net::types::*;
use lofire_store_lmdb::store::LmdbStore;

#[derive(Debug)]
enum ProtocolType {
    Start,
    Auth,
    Broker,
    Ext,
    P2P,
}

pub struct ProtocolHandler {
    broker: Arc<BrokerServer>,
    protocol: ProtocolType,
    auth_protocol: Option<AuthProtocolHandler>,
    broker_protocol: Option<BrokerProtocolHandler>,
    ext_protocol: Option<ExtProtocolHandler>,
}

impl ProtocolHandler {
    /// Handle incoming message
    // FIXME return ProtocolError instead of panic via unwrap()
    pub fn handle_incoming(&mut self, frame: Vec<u8>) -> Vec<Result<Vec<u8>, ProtocolError>> {
        debug_println!("SERVER PROTOCOL {:?}", &self.protocol);
        match &self.protocol {
            ProtocolType::Start => {
                let message = serde_bare::from_slice::<StartProtocol>(&frame).unwrap();
                match message {
                    StartProtocol::Auth(b) => {
                        self.protocol = ProtocolType::Auth;
                        self.auth_protocol = Some(AuthProtocolHandler::new());
                        vec![self.auth_protocol.as_mut().unwrap().handle_init(b)]
                    }
                    StartProtocol::Ext(ext) => {
                        self.protocol = ProtocolType::Ext;
                        self.ext_protocol = Some(ExtProtocolHandler {});
                        let reply = self.ext_protocol.as_ref().unwrap().handle_incoming(ext);
                        vec![Ok(serde_bare::to_vec(&reply).unwrap())]
                    }
                }
            }
            ProtocolType::Auth => {
                let res = self.auth_protocol.as_mut().unwrap().handle_incoming(frame);
                if res.last().unwrap().is_ok() {
                    // we switch to Broker protocol
                    self.protocol = ProtocolType::Broker;
                    self.broker_protocol = Some(BrokerProtocolHandler {
                        user: self.auth_protocol.as_ref().unwrap().get_user().unwrap(),
                        broker: Arc::clone(&self.broker),
                    });
                    self.auth_protocol = None;
                }
                res
            }
            ProtocolType::Broker => {
                let message = serde_bare::from_slice::<BrokerMessage>(&frame).unwrap();
                let reply = self
                    .broker_protocol
                    .as_ref()
                    .unwrap()
                    .handle_incoming(message);
                vec![Ok(serde_bare::to_vec(&reply).unwrap())]
            }
            ProtocolType::Ext => {
                // Ext protocol is not accepting 2 extrequest in the same connection.
                // TODO, close the connection
                vec![Err(ProtocolError::InvalidState)]
            }
            ProtocolType::P2P => {
                unimplemented!()
            }
        }
    }
}

pub struct ExtProtocolHandler {}

impl ExtProtocolHandler {
    pub fn handle_incoming(&self, msg: ExtRequest) -> ExtResponse {
        unimplemented!()
    }
}

pub struct BrokerProtocolHandler {
    broker: Arc<BrokerServer>,
    user: PubKey,
}

impl BrokerProtocolHandler {
    fn prepare_reply_broker_message(
        res: Result<(), ProtocolError>,
        id: u64,
        padding_size: usize,
    ) -> BrokerMessage {
        let result = match res {
            Ok(_) => 0,
            Err(e) => e.into(),
        };
        let msg = BrokerMessage::V0(BrokerMessageV0 {
            padding: vec![0; padding_size],
            content: BrokerMessageContentV0::BrokerResponse(BrokerResponse::V0(BrokerResponseV0 {
                id,
                result,
            })),
        });
        msg
    }

    fn prepare_reply_broker_overlay_message(
        res: Result<(), ProtocolError>,
        id: u64,
        overlay: OverlayId,
        block: Option<Block>,
        padding_size: usize,
    ) -> BrokerMessage {
        let result = match res {
            Ok(_) => 0,
            Err(e) => e.into(),
        };
        let content = match block {
            Some(b) => Some(BrokerOverlayResponseContentV0::Block(b)),
            None => None,
        };
        let msg = BrokerMessage::V0(BrokerMessageV0 {
            padding: vec![0; padding_size],
            content: BrokerMessageContentV0::BrokerOverlayMessage(BrokerOverlayMessage::V0(
                BrokerOverlayMessageV0 {
                    overlay,
                    content: BrokerOverlayMessageContentV0::BrokerOverlayResponse(
                        BrokerOverlayResponse::V0(BrokerOverlayResponseV0 {
                            id,
                            result,
                            content,
                        }),
                    ),
                },
            )),
        });
        msg
    }

    fn prepare_reply_broker_overlay_message_stream(
        res: Result<Block, ProtocolError>,
        id: u64,
        overlay: OverlayId,
        padding_size: usize,
    ) -> BrokerMessage {
        let result: u16 = match &res {
            Ok(r) => ProtocolError::PartialContent.into(),
            Err(e) => (*e).clone().into(),
        };
        let content = match res {
            Ok(r) => Some(BrokerOverlayResponseContentV0::Block(r)),
            Err(_) => None,
        };
        let msg = BrokerMessage::V0(BrokerMessageV0 {
            padding: vec![0; padding_size],
            content: BrokerMessageContentV0::BrokerOverlayMessage(BrokerOverlayMessage::V0(
                BrokerOverlayMessageV0 {
                    overlay,
                    content: BrokerOverlayMessageContentV0::BrokerOverlayResponse(
                        BrokerOverlayResponse::V0(BrokerOverlayResponseV0 {
                            id,
                            result,
                            content,
                        }),
                    ),
                },
            )),
        });
        msg
    }

    pub fn handle_incoming(&self, msg: BrokerMessage) -> BrokerMessage {
        // TODO check FSM

        let padding_size = 20; // TODO randomize, if config of server contains padding_max

        let id = msg.id();
        let content = msg.content();
        match content {
            BrokerMessageContentV0::BrokerRequest(req) => Self::prepare_reply_broker_message(
                match req.content_v0() {
                    BrokerRequestContentV0::AddUser(cmd) => {
                        self.broker.add_user(cmd.user(), self.user, cmd.sig())
                    }
                    BrokerRequestContentV0::DelUser(cmd) => {
                        self.broker.del_user(cmd.user(), cmd.sig())
                    }
                    BrokerRequestContentV0::AddClient(cmd) => {
                        self.broker.add_client(cmd.client(), cmd.sig())
                    }
                    BrokerRequestContentV0::DelClient(cmd) => {
                        self.broker.del_client(cmd.client(), cmd.sig())
                    }
                },
                id,
                padding_size,
            ),
            BrokerMessageContentV0::BrokerResponse(res) => Self::prepare_reply_broker_message(
                Err(ProtocolError::InvalidState),
                id,
                padding_size,
            ),
            BrokerMessageContentV0::BrokerOverlayMessage(omsg) => {
                let overlay = omsg.overlay_id();
                let block = None;
                let mut res = Err(ProtocolError::InvalidState);

                if omsg.is_request() {
                    match omsg.overlay_request().content_v0() {
                        BrokerOverlayRequestContentV0::OverlayConnect(_) => {
                            res = self.broker.overlay_connect(overlay)
                        }
                        BrokerOverlayRequestContentV0::OverlayJoin(j) => {
                            res = self.broker.overlay_join(overlay, j.secret(), j.peers())
                        }
                        BrokerOverlayRequestContentV0::BlockPut(b) => {
                            res = self.broker.block_put(overlay, b.block())
                        }
                        // TODO BranchSyncReq
                        BrokerOverlayRequestContentV0::BlockGet(b) => {
                            let res = self.broker.block_get(
                                overlay,
                                b.id(),
                                b.include_children(),
                                b.topic(),
                            );
                            // return an error or the first block, and setup a spawner for the remain blocks to be sent.
                            let one_reply = match res {
                                Err(e) => Err(e),
                                Ok(stream) => stream
                                    .recv_blocking()
                                    .map_err(|e| ProtocolError::EndOfStream),
                            };
                            return Self::prepare_reply_broker_overlay_message_stream(
                                one_reply,
                                id,
                                overlay,
                                padding_size,
                            );
                        }
                        _ => {}
                    }
                }

                Self::prepare_reply_broker_overlay_message(res, id, overlay, block, padding_size)
            }
        }
    }
}

pub struct BrokerServer {
    store: LmdbStore,
}

impl BrokerServer {
    pub const fn new(store: LmdbStore) -> BrokerServer {
        BrokerServer { store }
    }

    pub fn local_connection(&mut self, user: PubKey) -> BrokerConnectionLocal {
        BrokerConnectionLocal::new(self, user)
    }

    pub fn protocol_handler(self: Arc<Self>) -> ProtocolHandler {
        return ProtocolHandler {
            broker: Arc::clone(&self),
            protocol: ProtocolType::Start,
            auth_protocol: None,
            broker_protocol: None,
            ext_protocol: None,
        };
    }

    pub fn add_user(
        &self,
        user_id: PubKey,
        admin_user: PubKey,
        sig: Sig,
    ) -> Result<(), ProtocolError> {
        debug_println!("ADDING USER {:?}", user_id);
        // TODO implement add_user

        // check that admin_user is indeed an admin

        // verify signature
        let op_content = AddUserContentV0 { user: user_id };
        let _ = verify(&serde_bare::to_vec(&op_content).unwrap(), sig, admin_user)?;

        // check user_id is not already present

        // if not, add to store
        Ok(())
    }

    pub fn del_user(&self, user_id: PubKey, sig: Sig) -> Result<(), ProtocolError> {
        // TODO implement del_user
        Ok(())
    }
    pub fn add_client(&self, client_id: PubKey, sig: Sig) -> Result<(), ProtocolError> {
        // TODO implement add_client
        Ok(())
    }

    pub fn del_client(&self, client_id: PubKey, sig: Sig) -> Result<(), ProtocolError> {
        // TODO implement del_client
        Ok(())
    }

    pub fn overlay_connect(&self, overlay: Digest) -> Result<(), ProtocolError> {
        // TODO check that the broker has already joined this overlay. if not, send OverlayNotJoined
        Err(ProtocolError::OverlayNotJoined)
    }

    pub fn block_put(&self, overlay: Digest, block: &Block) -> Result<(), ProtocolError> {
        self.store._put(block);
        Ok(())
    }

    pub fn block_get(
        &self,
        overlay: Digest,
        id: BlockId,
        include_children: bool,
        topic: Option<PubKey>,
    ) -> Result<async_channel::Receiver<Block>, ProtocolError> {
        unimplemented!();

        // if !include_children {
        //     for id in ids {
        //         self.store.get()
        //     }
        // }
    }

    pub fn overlay_join(
        &self,
        overlay: Digest,
        secret: SymKey,
        peers: &Vec<PeerAdvert>,
    ) -> Result<(), ProtocolError> {
        // TODO join an overlay
        Ok(())
    }
}
