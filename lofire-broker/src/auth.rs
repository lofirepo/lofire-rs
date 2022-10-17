use std::pin::Pin;

use debug_print::*;
use futures::future::BoxFuture;
use futures::future::OptionFuture;
use futures::FutureExt;
use lofire::types::*;
use lofire::utils::*;
use lofire_net::errors::*;
use lofire_net::types::*;
use rust_fsm::*;

state_machine! {
    derive(Debug)
    AuthProtocolClient(Ready)

    Ready(ClientHelloSent) => ClientHelloSent,
    ClientHelloSent(ServerHelloReceived) => ServerHelloReceived,
    ServerHelloReceived(ClientAuthSent) => ClientAuthSent,
    ClientAuthSent(AuthResultReceived) => AuthResult,
    AuthResult => {
        Ok => BrokerProtocol,
        Error => Closed,
    },
}

state_machine! {
    derive(Debug)
    AuthProtocolServer(Ready)

    Ready(ClientHelloReceived) => ClientHelloReceived,
    ClientHelloReceived(ServerHelloSent) => ServerHelloSent,
    ServerHelloSent(ClientAuthReceived) => ClientAuthReceived,
    ClientAuthReceived => {
        Ok => AuthResultOk,
        Error => AuthResultError,
    },
    AuthResultOk(AuthResultSent) => BrokerProtocol,
    AuthResultError(AuthResultSent) => Closed,
}

pub struct AuthProtocolHandler {
    machine: StateMachine<AuthProtocolServer>,
    nonce: Option<Vec<u8>>,
    user: Option<PubKey>,
}

impl AuthProtocolHandler {
    pub fn new() -> AuthProtocolHandler {
        AuthProtocolHandler {
            machine: StateMachine::new(),
            nonce: None,
            user: None,
        }
    }

    pub fn get_user(&self) -> Option<PubKey> {
        self.user
    }

    // FIXME return ProtocolError instead of panic via unwrap()
    pub fn handle_init(&mut self, client_hello: ClientHello) -> Result<Vec<u8>, ProtocolError> {
        let _ = self
            .machine
            .consume(&AuthProtocolServerInput::ClientHelloReceived)
            .map_err(|_e| ProtocolError::InvalidState)?;

        let mut random_buf = [0u8; 32];
        getrandom::getrandom(&mut random_buf).unwrap();
        let nonce = random_buf.to_vec();
        let reply = ServerHello::V0(ServerHelloV0 {
            nonce: nonce.clone(),
        });
        self.nonce = Some(nonce);

        let _ = self
            .machine
            .consume(&AuthProtocolServerInput::ServerHelloSent)
            .map_err(|_e| ProtocolError::InvalidState)?;

        //debug_println!("sending nonce to client: {:?}", self.nonce);

        Ok(serde_bare::to_vec(&reply).unwrap())
    }

    // FIXME return ProtocolError instead of panic via unwrap()
    pub fn handle_incoming(
        &mut self,
        frame: Vec<u8>,
    ) -> (
        Result<Vec<u8>, ProtocolError>,
        Pin<Box<OptionFuture<BoxFuture<'static, u16>>>>,
    ) {
        fn prepare_reply(res: Result<Vec<u8>, ProtocolError>) -> AuthResult {
            let (result, metadata) = match res {
                Ok(m) => (0, m),
                Err(e) => (e.into(), vec![]),
            };
            AuthResult::V0(AuthResultV0 { result, metadata })
        }

        // FIXME return ProtocolError instead of panic via unwrap()
        fn process_state(
            handler: &mut AuthProtocolHandler,
            frame: Vec<u8>,
        ) -> Result<Vec<u8>, ProtocolError> {
            match handler.machine.state() {
                &AuthProtocolServerState::ServerHelloSent => {
                    let message = serde_bare::from_slice::<ClientAuth>(&frame).unwrap();
                    let _ = handler
                        .machine
                        .consume(&AuthProtocolServerInput::ClientAuthReceived)
                        .map_err(|_e| ProtocolError::InvalidState)?;

                    // verifying client auth

                    debug_println!("verifying client auth");

                    let _ = verify(
                        &serde_bare::to_vec(&message.content_v0()).unwrap(),
                        message.sig(),
                        message.user(),
                    )
                    .map_err(|_e| ProtocolError::AccessDenied)?;

                    // debug_println!(
                    //     "matching nonce : {:?} {:?}",
                    //     message.nonce(),
                    //     handler.nonce.as_ref().unwrap()
                    // );

                    if message.nonce() != handler.nonce.as_ref().unwrap() {
                        let _ = handler
                            .machine
                            .consume(&AuthProtocolServerInput::Error)
                            .map_err(|_e| ProtocolError::InvalidState);

                        return Err(ProtocolError::AccessDenied);
                    }

                    // TODO check that the device has been registered for this user. if not, return AccessDenied

                    // all is good, we advance the FSM and send back response
                    let _ = handler
                        .machine
                        .consume(&AuthProtocolServerInput::Ok)
                        .map_err(|_e| ProtocolError::InvalidState)?;

                    handler.user = Some(message.user());

                    Ok(vec![]) // without any metadata
                }
                _ => Err(ProtocolError::InvalidState),
            }
        }

        let res = process_state(self, frame);
        let is_err = res.as_ref().err().cloned();
        let reply = prepare_reply(res);
        let reply_ser: Result<Vec<u8>, ProtocolError> = Ok(serde_bare::to_vec(&reply).unwrap());
        if is_err.is_some() {
            (
                reply_ser,
                Box::pin(OptionFuture::from(Some(
                    async move { reply.result() }.boxed(),
                ))),
            )
        } else {
            (reply_ser, Box::pin(OptionFuture::from(None)))
        }
    }
}
