use tokio::sync::mpsc;

use super::dialog::Dialog;
use crate::message::headers::Contact;
use crate::message::method::SipMethod;
use crate::message::status_code::StatusCode;
use crate::transaction::{Role, ServerTransaction};
use crate::transport::incoming::{IncomingMessage, IncomingRequest};
use crate::ua::dialog::DialogState;
use crate::{Endpoint, Result};

pub struct InvSession<S> {
    state: S,
    endpoint: Endpoint,
    role: Role,
}

pub struct Incoming {
    dialog: Dialog,
    server_tsx: ServerTransaction,
}

pub struct Established {
    receiver: mpsc::Receiver<IncomingMessage>,
}

impl InvSession<Incoming> {
    pub fn create_uas(
        request: IncomingRequest,
        contact: Contact,
        endpoint: Endpoint,
    ) -> Result<Self> {
        let dialog = Dialog::new_uas(&endpoint, &request, contact);
        let server_tsx = endpoint.accept_request(request);

        Ok(InvSession {
            state: Incoming { dialog, server_tsx },
            endpoint: endpoint,
            role: Role::Uas,
        })
    }

    pub async fn progress(&mut self, status_code: StatusCode) -> Result<()> {
        let Incoming { dialog, server_tsx } = &mut self.state;
        let response = dialog.create_response(server_tsx, status_code);

        server_tsx.send_provisional_response(response).await?;

        dialog.set_state(DialogState::Early);

        Ok(())
    }

    pub async fn accept(self, status_code: StatusCode) -> Result<InvSession<Established>> {
        let Incoming { dialog, server_tsx } = self.state;
        let response = dialog.create_response(&server_tsx, status_code);

        server_tsx.send_final_response(response).await?;

        let (sender, receiver) = mpsc::channel(5);

        dialog.set_channel(sender);
        dialog.set_state(DialogState::Completed);

        Ok(InvSession {
            state: Established { receiver },
            endpoint: self.endpoint,
            role: self.role,
        })
    }
}

pub enum SessionEvent {
    Bye(IncomingRequest),
    ReInvite(IncomingRequest),
    Options(IncomingRequest),
}

impl InvSession<Established> {
    pub async fn recv(&mut self) -> Option<SessionEvent> {
        let msg = self.state.receiver.recv().await?;

        match msg {
            IncomingMessage::Request(request) => match request.req_line.method {
                SipMethod::Invite => Some(SessionEvent::ReInvite(request)),
                SipMethod::Bye => Some(SessionEvent::Bye(request)),
                other => {
                    log::debug!("Received Other Method: {other}");
                    None
                }
            },
            IncomingMessage::Response(incoming_response) => todo!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::method::SipMethod;
    use crate::message::status_code::StatusCode;
    use crate::test_utils::transport::MockTransport;
    use crate::test_utils::{create_test_endpoint, create_test_request};
    use crate::transport::Transport;

    fn create_test_invite() -> IncomingRequest {
        let transport = Transport::new(MockTransport::new_udp());
        create_test_request(SipMethod::Invite, transport)
    }

    #[tokio::test]
    async fn test_create_session() {
        let endpoint = create_test_endpoint().await;
        let req = create_test_invite();
        let contact = "test <sip:localhost:5969>".parse().unwrap();

        let mut session = InvSession::create_uas(req, contact, endpoint).unwrap();

        session.progress(StatusCode::Trying).await.unwrap();
        session.progress(StatusCode::Ringing).await.unwrap();

        let mut session = session.accept(StatusCode::Ok).await.unwrap();

        while let Some(evt) = session.recv().await {}
    }
}
