use crate::dialog::{Dialog, DialogState};
use crate::message::headers::Contact;
use crate::message::method::SipMethod;
use crate::message::status_code::StatusCode;
use crate::transaction::{Role, ServerTransaction};
use crate::{Endpoint, IncomingMessage, IncomingRequest, Result};

pub struct InviteSession<State> {
    state: State,
    role: Role,
    dialog: Dialog,
    endpoint: Endpoint,
}

pub struct Incoming {
    server_tsx: ServerTransaction,
}

pub struct Established;

impl InviteSession<Incoming> {
    pub fn incoming(
        request: IncomingRequest,
        contact: Contact,
        endpoint: Endpoint,
    ) -> Result<Self> {
        let dialog = Dialog::create_uas(endpoint.clone(), &request, contact)?;
        let server_tsx = endpoint.accept_request(request);

        Ok(InviteSession {
            dialog,
            state: Incoming { server_tsx },
            role: Role::Uas,
            endpoint,
        })
    }

    pub async fn progress(&mut self, status_code: StatusCode) -> Result<()> {
        let Incoming { server_tsx } = &mut self.state;

        let response = self.dialog.create_response(&server_tsx, status_code);
        server_tsx.send_provisional_response(response).await?;

        self.dialog.set_state(DialogState::Early);

        Ok(())
    }

    pub async fn accept(self, status_code: StatusCode) -> Result<InviteSession<Established>> {
        let Incoming { server_tsx } = self.state;

        let response = self.dialog.create_response(&server_tsx, status_code);
        server_tsx.send_final_response(response).await?;

        Ok(InviteSession {
            state: Established,
            role: self.role,
            dialog: self.dialog,
            endpoint: self.endpoint,
        })
    }
}

impl InviteSession<Established> {
    async fn handle_bye(&mut self, bye: IncomingRequest) -> Result<()> {
        let bye_tsx = self.endpoint.accept_request(bye);

        let response = self.dialog.create_response(&bye_tsx, StatusCode::Ok);

        bye_tsx.send_final_response(response).await?;

        Ok(())
    }
    pub async fn recv(&mut self) -> Option<SessionEvent> {
        loop {
            match self.dialog.recv().await? {
                IncomingMessage::Request(request) => match request.req_line.method {
                    SipMethod::Invite => break Some(SessionEvent::ReInvite(request)),
                    SipMethod::Bye => {
                        let _res = self.handle_bye(request).await;
                        break Some(SessionEvent::Terminated);
                    }
                    method => {
                        log::info!("received other request: {}", method);
                        continue;
                    }
                },
                IncomingMessage::Response(incoming_response) => todo!(),
            }
        }
    }
}

pub enum SessionEvent {
    Terminated,
    ReInvite(IncomingRequest),
    Options(IncomingRequest),
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

        let mut session = InviteSession::incoming(req, contact, endpoint).unwrap();

        session.progress(StatusCode::Trying).await.unwrap();
        session.progress(StatusCode::Ringing).await.unwrap();

        let mut session = session.accept(StatusCode::Ok).await.unwrap();

        while let Some(evt) = session.recv().await {}
    }
}
