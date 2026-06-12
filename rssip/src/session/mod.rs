#[cfg(test)]
mod session_test;

use std::ops;

use tokio::sync::mpsc;

use crate::dialog::Dialog;
use crate::message::headers::Contact;
use crate::message::method::SipMethod;
use crate::message::status_code::StatusCode;
use crate::transaction::ServerTransaction;
use crate::{Endpoint, Error, IncomingRequest, Result};

pub enum SessionEvent {
    Terminated(Cause),
    ReInvite(IncomingRequest),
    Options(IncomingRequest),
}

#[derive(Debug)]
pub enum Cause {
    ByeReceived,
}

pub struct Session<S> {
    state: S,
}

pub struct Incoming {
    dialog: Dialog,
    endpoint: Endpoint,
    server_tsx: ServerTransaction,
}

pub struct Established {
    rx: mpsc::Receiver<SessionEvent>,
}

impl Session<Incoming> {
    pub fn init_incoming(
        request: IncomingRequest,
        contact: Contact,
        endpoint: Endpoint,
    ) -> Result<Self> {
        let dialog = Dialog::create_uas(&request, contact, endpoint.clone())?;
        let server_tsx = ServerTransaction::from_request(request, endpoint.clone());
        Ok(Session {
            state: Incoming::new(dialog, server_tsx, endpoint),
        })
    }

    // RFC 3261 13.3.1.1
    pub async fn progress(&mut self, status_code: StatusCode) -> Result<()> {
        let Incoming {
            server_tsx, dialog, ..
        } = &mut self.state;

        dialog.provisional_response(server_tsx, status_code).await?;

        Ok(())
    }

    // RFC 3261 13.3.1.2
    pub async fn redirect(self, status_code: StatusCode) -> Result<()> {
        if !matches!(status_code.as_u16(), 300..=399) {
            return Err(Error::Other(format!(
                "Invalid status code (expected 3xx) got {:?}",
                status_code
            )));
        }
        let Incoming {
            server_tsx,
            mut dialog,
            ..
        } = self.state;

        dialog.final_response(server_tsx, status_code).await?;

        Ok(())
    }

    // RFC 3261 13.3.1.3
    pub async fn reject(self, status_code: StatusCode) -> Result<()> {
        if !matches!(status_code.as_u16(), 400..=699) {
            return Err(Error::Other(format!(
                "Invalid status code (expected 4xx-6xx) got {:?}",
                status_code
            )));
        }
        let Incoming {
            server_tsx,
            mut dialog,
            ..
        } = self.state;

        dialog.final_response(server_tsx, status_code).await?;

        Ok(())
    }

    // RFC 3261 13.3.1.4
    pub async fn accept(self, status_code: StatusCode) -> Result<Session<Established>> {
        let Incoming {
            server_tsx,
            mut dialog,
            endpoint,
        } = self.state;

        dialog.final_response(server_tsx, status_code).await?;
        dialog.wait_for_ack().await?;

        Ok(Session {
            state: Established::new(dialog, endpoint),
        })
    }
}

impl Incoming {
    fn new(dialog: Dialog, server_tsx: ServerTransaction, endpoint: Endpoint) -> Self {
        Self {
            dialog,
            endpoint,
            server_tsx,
        }
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
