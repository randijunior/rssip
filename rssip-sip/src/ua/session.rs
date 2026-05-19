use super::dialog::Dialog;
use crate::message::headers::Contact;
use crate::message::status_code::StatusCode;
use crate::transaction::{Role, ServerTransaction};
use crate::transport::incoming::IncomingRequest;
use crate::{Endpoint, Result};

struct InvSession<S> {
    state: S,
    endpoint: Endpoint,
    role: Role,
}

struct Incoming {
    dialog: Dialog,
    server_tsx: ServerTransaction,
}

struct Established {
    dialog: Dialog,
}

impl InvSession<Incoming> {
    pub fn from_invitation(
        request: IncomingRequest,
        contact: Contact,
        endpoint: Endpoint,
    ) -> Result<Self> {
        let dialog = Dialog::new_uas(&endpoint, &request, contact);
        let server_tsx = endpoint.accept_request(request);

        Ok(InvSession {
            state: Incoming { dialog, server_tsx },
            endpoint: endpoint,
            role: Role::UAS,
        })
    }

    pub async fn progress(&mut self, status_code: StatusCode) -> Result<()> {
        let Incoming { dialog, server_tsx } = &mut self.state;

        dialog.respond_provisional(server_tsx, status_code).await?;

        Ok(())
    }

    pub async fn accept(self, status_code: StatusCode) -> Result<InvSession<Established>> {
        let Incoming { dialog, server_tsx } = self.state;

        todo!()

        // dialog.respond_final(server_tsx, status_code).await?;

        // Ok(InvSession {
        //     state: Established { dialog },
        //     endpoint: self.endpoint,
        //     role: self.role,
        // })
    }
}

pub enum SessionEvent {}

impl InvSession<Established> {
    pub async fn receive_event(&mut self) -> Option<SessionEvent> {
        todo!()
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

        let mut session = InvSession::from_invitation(req, contact, endpoint).unwrap();

        session.progress(StatusCode::Trying).await.unwrap();
        session.progress(StatusCode::Ringing).await.unwrap();

        let mut session = session.accept(StatusCode::Ok).await.unwrap();

        while let Some(evt) = session.receive_event().await {}
    }
}
