use rssip::dialog::{Dialog, DialogState};
use rssip::message::headers::Contact;
use rssip::message::method::SipMethod;
use rssip::message::status_code::StatusCode;
use rssip::transaction::{Role, ServerTransaction};
use rssip::{Endpoint, IncomingMessage, IncomingRequest, Result};
use tokio::sync::mpsc;

pub struct InvSession<S> {
    state: S,
    endpoint: Endpoint,
    role: Role,
}

pub struct Incoming {
    dialog: Dialog,
    server_tsx: ServerTransaction,
}

pub struct Completed {
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

    pub async fn accept(self, status_code: StatusCode) -> Result<InvSession<Completed>> {
        let Incoming { dialog, server_tsx } = self.state;
        let response = dialog.create_response(&server_tsx, status_code);

        server_tsx.send_final_response(response).await?;

        let (sender, receiver) = mpsc::channel(5);

        dialog.set_state(DialogState::Established(sender));

        Ok(InvSession {
            state: Completed { receiver },
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

impl InvSession<Completed> {
    pub async fn recv(&mut self) -> Option<SessionEvent> {
        let msg = self.state.receiver.recv().await?;

        match msg {
            IncomingMessage::Request(request) => match request.req_line.method {
                SipMethod::Invite => Some(SessionEvent::ReInvite(request)),
                SipMethod::Bye => Some(SessionEvent::Bye(request)),
                other => None,
            },
            IncomingMessage::Response(incoming_response) => todo!(),
        }
    }
}
