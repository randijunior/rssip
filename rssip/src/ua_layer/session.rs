use std::ops;

use sdp::msg::SessionDescription;
use tokio::sync::mpsc;

use crate::message::SipBody;
use crate::message::headers::Contact;
use crate::message::method::SipMethod;
use crate::message::status_code::StatusCode;
use crate::transaction::ServerTransaction;
use crate::ua_layer::dialog::Dialog;
use crate::ua_layer::negotiator::{Done, LocalOffer, Negotiator, RemoteOffer, WaitNego};
use crate::{Endpoint, Error, IncomingRequest, Result};

// Offer                Answer             RFC    Ini Est Early
// -------------------------------------------------------------------
// 1. INVITE Req.          2xx INVITE Resp.     RFC 3261  Y   Y    N
// 2. 2xx INVITE Resp.     ACK Req.             RFC 3261  Y   Y    N

pub struct Session<S, N = ()> {
    state: S,
    nego: Negotiator<N>,
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

impl<N> Session<Incoming, N> {
    // RFC 3261 13.3.1.1
    pub async fn progress(&mut self, status_code: StatusCode) -> Result<()> {
        let Incoming {
            server_tsx, dialog, ..
        } = &mut self.state;

        dialog.provisional_response(server_tsx, status_code).await?;

        Ok(())
    }
}

impl Session<Incoming> {
    pub fn from_invitation(
        server_tsx: ServerTransaction,
        contact: Contact,
        endpoint: Endpoint,
    ) -> Result<Self> {
        let request = server_tsx.request();

        if request.body.is_some() {
            return Err(Error::ErrUnexpectedSdpBody);
        }
        let dialog = Dialog::create_uas(request, contact, endpoint.clone())?;
        let nego = Negotiator::new();

        Ok(Session {
            state: Incoming {
                dialog,
                endpoint,
                server_tsx,
            },
            nego,
        })
    }

    pub fn set_local_offer(self, offer: LocalOffer) -> Session<Incoming, LocalOffer> {
        Session {
            nego: Negotiator::from_local(offer),
            state: self.state,
        }
    }
}

// offer may be sent in INVITE (EarlyOffer)
impl Session<Incoming, RemoteOffer> {
    pub fn from_invitation_with_sdp(
        server_tsx: ServerTransaction,
        contact: Contact,
        endpoint: Endpoint,
    ) -> Result<Self> {
        let dialog = Dialog::create_uas(server_tsx.request(), contact, endpoint.clone())?;
        let invite = server_tsx.request();

        let sdp = match &invite.body {
            Some(body) => Self::get_sdp(body)?,
            None => {
                return Err(Error::ErrMissingSdpBody);
            }
        };
        let nego = Negotiator::from_remote(RemoteOffer::new(sdp));

        Ok(Session {
            state: Incoming {
                dialog,
                endpoint,
                server_tsx,
            },
            nego,
        })
    }

    pub fn set_local_offer(self, offer: LocalOffer) -> Session<Incoming, WaitNego> {
        Session {
            nego: self.nego.set_local_offer(offer),
            state: self.state,
        }
    }
}

impl Session<Incoming, LocalOffer> {
    pub async fn accept(self, status_code: StatusCode) -> Result<Session<Established, Done>> {
        let Incoming {
            server_tsx,
            mut dialog,
            endpoint,
        } = self.state;

        dialog.final_response(server_tsx, status_code).await?;

        let ack = dialog.wait_for_ack().await?;
        let sdp = match &ack.body {
            Some(body) => Self::get_sdp(body)?,
            None => return Err(Error::Other(format!("Missing body on ACK request",))),
        };

        let nego = self.nego.process_answer(sdp)?;

        Ok(Session {
            state: Established::new(dialog, endpoint),
            nego,
        })
    }
}

impl Session<Incoming, WaitNego> {
    pub async fn accept(self, status_code: StatusCode) -> Result<Session<Established, Done>> {
        let Incoming {
            server_tsx,
            mut dialog,
            endpoint,
        } = self.state;

        let nego = self.nego.negotiate()?;

        // TODO: Add sdp to final response
        dialog.final_response(server_tsx, status_code).await?;
        dialog.wait_for_ack().await?;

        Ok(Session {
            state: Established::new(dialog, endpoint),
            nego,
        })
    }
}

impl<S, N> Session<S, N> {
    fn get_sdp(body: &SipBody) -> Result<SessionDescription> {
        let sdp = sdp::parser::SdpParser::parse(body.as_ref())
            .map_err(|err| Error::Other(err.to_string()))?;

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

impl<N> ops::Deref for Session<Established, N> {
    type Target = mpsc::Receiver<SessionEvent>;

    fn deref(&self) -> &Self::Target {
        &self.state.rx
    }
}

impl<N> ops::DerefMut for Session<Established, N> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.state.rx
    }
}
