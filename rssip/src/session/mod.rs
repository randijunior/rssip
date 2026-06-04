#[cfg(test)]
mod session_test;

use tokio::sync::mpsc;
use tokio::time;

use crate::dialog::{Dialog, DialogState};
use crate::message::headers::Contact;
use crate::message::method::SipMethod;
use crate::message::status_code::StatusCode;
use crate::transaction::{ServerTransaction, timers};
use crate::{Endpoint, IncomingMessage, IncomingRequest, Result};

pub struct InviteSession<S> {
    state: S,
}

pub struct Incoming {
    dialog: Dialog,
    endpoint: Endpoint,
    server_tsx: ServerTransaction,
}

pub struct Confirmed {
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

    pub async fn accept(self, status_code: StatusCode) -> Result<InviteSession<Confirmed>> {
        let Incoming {
            server_tsx,
            mut dialog,
            endpoint,
        } = self.state;

        let response = dialog.create_response(&server_tsx, status_code);
        server_tsx.send_final_response(response).await?;

        let ack_timer = time::sleep(timers::T1 * 64);
        tokio::pin!(ack_timer);

        loop {
            tokio::select! {
                msg = dialog.recv() => {
                    match msg {
                        Ok(Some(IncomingMessage::Request(req))) if req.req_line.method == SipMethod::Ack => {
                            break;
                        },
                        Ok(Some(IncomingMessage::Request(req))) => {
                            log::debug!("received request: {} (ignoring)", req.req_line.method);
                            continue;
                        }
                        Err(err)=> {
                            return Err(err);
                        },
                        Ok(None) =>  { return Err(crate::Error::ChannelClosed); },
                        _=>  {
                            continue;
                        }
                    }
                }
                _ = &mut ack_timer => {
                    return Err(crate::Error::Other("No ACK received".into()))
                }
            }
        }

        let (tx, rx) = mpsc::channel::<SessionEvent>(10);

        tokio::spawn(async move {
            loop {
                let Ok(Some(msg)) = dialog.recv().await else {
                    break;
                };
                match msg {
                    IncomingMessage::Request(request) => match request.req_line.method {
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
                    IncomingMessage::Response(_incoming_response) => unimplemented!(),
                }
            }
        });

        Ok(InviteSession {
            state: Confirmed { rx },
        })
    }
}


impl InviteSession<Confirmed> {
    pub async fn recv(&mut self) -> Option<SessionEvent> {
        self.state.rx.recv().await
    }
}
