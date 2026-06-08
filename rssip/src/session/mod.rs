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
use crate::{Endpoint, IncomingRequest, Result};

pub struct InviteSession<S> {
    state: S,
}

pub struct Incoming {
    dialog: Dialog,
    endpoint: Endpoint,
    server_tsx: ServerTransaction,
}

pub struct Accepted {
    dialog: Dialog,
    endpoint: Endpoint,
}

pub struct Confirmed {
    rx: mpsc::Receiver<SessionEvent>,
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
            state: Incoming {
                server_tsx,
                dialog,
                endpoint,
            },
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

    pub async fn accept(self, status_code: StatusCode) -> Result<InviteSession<Accepted>> {
        let Incoming {
            server_tsx,
            dialog,
            endpoint,
        } = self.state;

        let response = dialog.create_response(&server_tsx, status_code);
        server_tsx.send_final_response(response).await?;

        Ok(InviteSession {
            state: Accepted { dialog, endpoint },
        })
    }
}

impl InviteSession<Accepted> {
    async fn recv_ack(&mut self) -> Result<IncomingRequest> {
        let ack_timer = timers::T1 * 64;
        loop {
            match time::timeout(ack_timer, self.state.dialog.recv_request()).await {
                Ok(Ok(req)) if req.req_line.method == SipMethod::Ack => {
                    return Ok(req);
                }
                Ok(Ok(req)) => {
                    log::debug!("received request: {} (ignoring)", req.req_line.method);
                    continue;
                }
                Ok(Err(err)) => {
                    return Err(err);
                }
                Err(_elapsed) => return Err(crate::Error::Other("No ACK received".into())),
            }
        }
    }

    pub async fn wait_for_ack(mut self) -> Result<InviteSession<Confirmed>> {
        let _ = self.recv_ack().await?;

        let (tx, rx) = mpsc::channel::<SessionEvent>(10);

        tokio::spawn(async move {
            let Accepted {
                mut dialog,
                endpoint,
            } = self.state;

            while let Ok(msg) = dialog.recv_request().await {
                match msg {
                    request => match request.req_line.method {
                        SipMethod::Invite => {
                            let _result = tx.send(SessionEvent::ReInvite(request));
                            break;
                        }
                        SipMethod::Bye => {
                            let bye_tsx =
                                ServerTransaction::from_request(request, endpoint.clone());

                            let response = dialog.create_response(&bye_tsx, StatusCode::Ok);

                            let _result = bye_tsx.send_final_response(response).await;
                            let _result = tx.send(SessionEvent::Terminated(Cause::ByeReceived));

                            break;
                        }
                        method => {
                            log::debug!("received request: {} (ignoring)", method);
                            continue;
                        }
                    },
                }
            }
        });

        Ok(InviteSession {
            state: Confirmed { rx },
        })
    }
}

impl ops::Deref for Confirmed {
    type Target = mpsc::Receiver<SessionEvent>;

    fn deref(&self) -> &Self::Target {
        &self.rx
    }
}

impl ops::DerefMut for Confirmed {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.rx
    }
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
