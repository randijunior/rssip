#[cfg(test)]
mod session_test;

use std::ops;

use tokio::sync::mpsc;
use tokio::time;

use crate::dialog::{Dialog, DialogState};
use crate::message::headers::Contact;
use crate::message::method::SipMethod;
use crate::message::status_code::StatusCode;
use crate::transaction::{ServerTransaction, timers};
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

    pub async fn progress(&mut self, status_code: StatusCode) -> Result<()> {
        let Incoming {
            server_tsx, dialog, ..
        } = &mut self.state;

        let response = dialog.create_response(&server_tsx, status_code);

        server_tsx.send_provisional_response(response).await?;

        dialog.set_state(DialogState::Early);

        Ok(())
    }

    pub async fn accept(self, status_code: StatusCode) -> Result<Session<Established>> {
        let Incoming {
            server_tsx,
            mut dialog,
            endpoint,
        } = self.state;

        let response = dialog.create_response(&server_tsx, status_code);
        server_tsx.send_final_response(response).await?;

        let ack_timer = timers::T1 * 64;
        loop {
            match time::timeout(ack_timer, dialog.recv_request())
                .await
                .map_err(|_elapsed| Error::Other("No ACK received".into()))??
            {
                req if req.req_line.method == SipMethod::Ack => {
                    break;
                }
                req => {
                    log::debug!(
                        "received request(NoAck): {} (ignoring)",
                        req.req_line.method
                    );
                    continue;
                }
            }
        }

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
        while let Ok(msg) = dialog.recv_request().await {
            match msg {
                request => match request.req_line.method {
                    SipMethod::Invite => {
                        tx.send(SessionEvent::ReInvite(request))
                            .await
                            .map_err(|_| Error::ChannelClosed)?;
                        break;
                    }
                    SipMethod::Bye => {
                        let bye_tsx = ServerTransaction::from_request(request, endpoint);

                        let response = dialog.create_response(&bye_tsx, StatusCode::Ok);
                        bye_tsx.send_final_response(response).await?;

                        tx.send(SessionEvent::Terminated(Cause::ByeReceived))
                            .await
                            .map_err(|_| Error::ChannelClosed)?;

                        break;
                    }
                    method => {
                        log::debug!("received request: {} (ignoring)", method);
                        continue;
                    }
                },
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
