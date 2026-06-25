use std::ops;

use media::sdp::SessionDescription;
use media::sdp::negotiator::Negotiator;
use media::sdp::parser::SdpParser;
use tokio::sync::mpsc;

use crate::message::SipBody;
use crate::message::headers::Contact;
use crate::message::method::SipMethod;
use crate::message::status_code::StatusCode;
use crate::transaction::ServerTransaction;
use crate::ua_layer::dialog::Dialog;
use crate::{Endpoint, Error, IncomingRequest, Result};

// Offer                Answer             RFC    Ini Est Early
// -------------------------------------------------------------------
// 1. INVITE Req.          2xx INVITE Resp.     RFC 3261  Y   Y    N
// 2. 2xx INVITE Resp.     ACK Req.             RFC 3261  Y   Y    N

pub struct Session<S> {
    state: S,
    nego: Negotiator,
}

pub struct Incoming {
    dialog: Dialog,
    endpoint: Endpoint,
    server_tsx: ServerTransaction,
}

pub struct Established {
    rx: mpsc::Receiver<SessionEvent>,
}

pub enum SessionEvent {
    Terminated(Cause),
    ReInvite(IncomingRequest),
    Options(IncomingRequest),
}

#[derive(Debug)]
pub enum Cause {
    ByeReceived,
}

impl Session<Incoming> {
    pub fn from_invite_tsx(
        server_tsx: ServerTransaction,
        contact: Contact,
        endpoint: Endpoint,
    ) -> Result<Self> {
        let invite = server_tsx.request();
        let nego = if let Some(body) = &invite.body {
            // EarlyOffer
            let sdp = Self::get_sdp(body)?;
            Negotiator::from_remote(sdp)
        } else {
            // DelayedOffer
            Negotiator::default()
        };
        let dialog = Dialog::create_uas(invite, contact, endpoint.clone())?;
        Ok(Session {
            state: Incoming {
                dialog,
                endpoint,
                server_tsx,
            },
            nego,
        })
    }

    pub fn set_local_offer(&mut self, offer: SessionDescription) -> Result<()> {
        self.nego.set_local_sdp(offer)?;
        Ok(())
    }

    // RFC 3261 13.3.1.1
    pub async fn progress(&mut self, status_code: StatusCode) -> Result<()> {
        let Incoming {
            server_tsx, dialog, ..
        } = &mut self.state;

        dialog.provisional_response(server_tsx, status_code).await?;

        Ok(())
    }
}

impl<S> Session<S> {
    fn get_sdp(body: &SipBody) -> Result<SessionDescription> {
        let sdp = SdpParser::parse(body.as_ref())?;
        Ok(sdp)
    }
}

impl Established {
    fn new(dialog: Dialog, endpoint: Endpoint) -> Self {
        let (tx, rx) = mpsc::channel::<SessionEvent>(10);

        tokio::spawn(async move {
            if let Err(err) = Self::session_loop(dialog, endpoint, tx).await {
                log::error!("Failed to handle dialog msg: {}", err);
            }
        });

        Self { rx }
    }

    async fn session_loop(
        mut dialog: Dialog,
        endpoint: Endpoint,
        tx: mpsc::Sender<SessionEvent>,
    ) -> Result<()> {
        while let Ok(request) = dialog.recv_request().await {
            match request.req_line.method {
                SipMethod::Invite => {
                    tx.send(SessionEvent::ReInvite(request))
                        .await
                        .map_err(|_| Error::ChannelClosed)?;
                    break;
                }
                SipMethod::Bye => {
                    let bye_tsx = ServerTransaction::from_request(request, endpoint);
                    dialog.final_response(bye_tsx, StatusCode::Ok).await?;

                    tx.send(SessionEvent::Terminated(Cause::ByeReceived))
                        .await
                        .map_err(|_| Error::ChannelClosed)?;

                    break;
                }
                method => {
                    log::debug!("received request: {} (ignoring)", method);
                    continue;
                }
            }
        }

        Ok(())
    }
}

impl ops::Deref for Session<Established> {
    type Target = mpsc::Receiver<SessionEvent>;

    fn deref(&self) -> &Self::Target {
        &self.state.rx
    }
}

impl ops::DerefMut for Session<Established> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.state.rx
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::method::SipMethod;
    use crate::test_utils::{create_test_endpoint, create_test_request};
    use crate::transport::{MockTransport, TransportHandle};

    fn create_test_invite() -> IncomingRequest {
        let transport = TransportHandle::new(MockTransport::new_udp());
        create_test_request(SipMethod::Invite, transport)
    }

    #[tokio::test]
    async fn test_session_with_late_offer() {
        let endpoint = create_test_endpoint().await;
        let request = create_test_invite();
        let contact = "test <sip:localhost:5969>".parse().unwrap();
        let server_tsx = ServerTransaction::from_request(request, endpoint.clone());

        let session = Session::from_invite_tsx(server_tsx, contact, endpoint);

        assert!(session.is_ok());
    }
}
