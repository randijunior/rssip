use tokio::time;
use tokio_util::either::Either;

use crate::dialog::{Dialog, DialogState};
use crate::message::headers::Contact;
use crate::message::method::SipMethod;
use crate::message::status_code::StatusCode;
use crate::transaction::{ServerTransaction, timers};
use crate::{Endpoint, IncomingMessage, IncomingRequest, Result};

pub struct InviteSession<State> {
    state: State,
    dialog: Dialog,
    endpoint: Endpoint,
}

pub struct Incoming {
    server_tsx: ServerTransaction,
}

pub struct Accepted {
    ack: Option<IncomingRequest>,
    terminated: bool,
}

impl InviteSession<Incoming> {
    pub fn create_incoming(
        request: IncomingRequest,
        contact: Contact,
        endpoint: Endpoint,
    ) -> Result<Self> {
        let dialog = Dialog::create_uas(endpoint.clone(), &request, contact)?;
        let server_tsx = ServerTransaction::from_request(request, endpoint.clone());
        Ok(InviteSession {
            dialog,
            endpoint,
            state: Incoming { server_tsx },
        })
    }

    pub async fn progress(&mut self, status_code: StatusCode) -> Result<()> {
        let Incoming { server_tsx } = &mut self.state;

        let response = self.dialog.create_response(&server_tsx, status_code);
        server_tsx.send_provisional_response(response).await?;

        self.dialog.set_state(DialogState::Early);

        Ok(())
    }

    pub async fn accept(self, status_code: StatusCode) -> Result<InviteSession<Accepted>> {
        let Incoming { server_tsx } = self.state;

        let response = self.dialog.create_response(&server_tsx, status_code);
        server_tsx.send_final_response(response).await?;

        Ok(InviteSession {
            state: Accepted {
                ack: None,
                terminated: false,
            },
            dialog: self.dialog,
            endpoint: self.endpoint,
        })
    }
}

impl InviteSession<Accepted> {
    pub async fn receive(&mut self) -> Result<Option<SessionEvent>> {
        if self.state.terminated {
            return Ok(None);
        };
        loop {
            let ack_timer = if self.state.ack.is_some() {
                Either::Left(std::future::pending())
            } else {
                Either::Right(time::sleep(timers::T1 * 64))
            };
            tokio::pin!(ack_timer);

            tokio::select! {
                msg = self.dialog.recv() => {
                    match msg {
                        Ok(Some(IncomingMessage::Request(request))) => match request.req_line.method {
                            SipMethod::Invite => return Ok(Some(SessionEvent::ReInvite(request))),
                            SipMethod::Bye => {
                                self.handle_bye(request).await?;
                                self.terminate();
                                return Ok(Some(SessionEvent::Terminated(Cause::ByeReceived)));
                            }
                            SipMethod::Ack => {
                                self.state.ack = Some(request);
                                continue;
                            }
                            method => {
                                log::debug!("received request: {} (ignoring)", method);
                                continue;
                            }
                        },
                        Ok(Some(IncomingMessage::Response(_incoming_response))) => unimplemented!(),
                        Ok(None) =>  {
                            self.terminate();
                            return Ok(None)
                        },
                        Err(err) =>  {
                            self.terminate();
                            return Err(err);
                        },
                    }
                }
                _ = &mut ack_timer => {
                    log::warn!("no ACK received, terminating the session");
                    self.terminate();
                    return Ok(Some(SessionEvent::Terminated(Cause::NoACK)));
                }
            }
        }
    }

    async fn handle_bye(&mut self, bye: IncomingRequest) -> Result<()> {
        let bye_tsx = ServerTransaction::from_request(bye, self.endpoint.clone());

        let response = self.dialog.create_response(&bye_tsx, StatusCode::Ok);

        bye_tsx.send_final_response(response).await?;

        Ok(())
    }

    fn terminate(&mut self) {
        self.state.terminated = true;
        self.endpoint.ua_plugin().remove_dialog(self.dialog.id());
    }
}

pub enum SessionEvent {
    Terminated(Cause),
    ReInvite(IncomingRequest),
    Options(IncomingRequest),
}

#[derive(Debug)]
pub enum Cause {
    NoACK,
    ByeReceived,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::method::SipMethod;
    use crate::message::status_code::StatusCode;
    use crate::test_utils::transport::MockTransport;
    use crate::test_utils::{create_test_endpoint, create_test_request};
    use crate::transport::TransportHandle;

    fn create_test_invite() -> IncomingRequest {
        let transport = TransportHandle::new(MockTransport::new_udp());
        create_test_request(SipMethod::Invite, transport)
    }

    #[tokio::test]
    async fn test_create_session() {
        let endpoint = create_test_endpoint().await;
        let req = create_test_invite();
        let contact = "test <sip:localhost:5969>".parse().unwrap();

        let mut session = InviteSession::create_incoming(req, contact, endpoint).unwrap();

        session.progress(StatusCode::Trying).await.unwrap();
        session.progress(StatusCode::Ringing).await.unwrap();

        let _session = session.accept(StatusCode::Ok).await.unwrap();
    }
}
