use std::sync::RwLock;

use rustc_hash::FxHashMap;
use tokio::sync::mpsc;

use crate::endpoint::{self, ReceivedRequest, ReceivedResponse};
use crate::error::{Error, Result};
use crate::message::headers::{CallId, Contact, From, Header, Headers, To};
use crate::message::method::SipMethod;
use crate::message::param::Params;
use crate::message::sip_uri::{Scheme, Uri};
use crate::message::status_code::StatusCode;
use crate::transaction::{Role, ServerTransaction};
use crate::transport::incoming::{IncomingMessage, IncomingRequest};
use crate::{Endpoint, OutgoingResponse, find_map_mut_header};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogState {
    Initial,
    Early,
    Confirmed,
}

/// Represents a SIP Dialog.
pub struct Dialog {
    dialog_id: DialogId,
    state: DialogState,
    remote_cseq: u32,
    local_cseq: u32,
    from: From,
    to: To,
    contact: Contact,
    secure: bool,
    route_set: Vec<RouteSet>,
    role: Role,
    receiver: mpsc::Receiver<IncomingMessage>,
    endpoint: Endpoint,
}

impl Dialog {
    pub fn create_uas(
        endpoint: Endpoint,
        request: &IncomingRequest,
        contact: Contact,
    ) -> Result<Dialog> {
        if !matches!(
            request.req_line.method,
            SipMethod::Invite
                | SipMethod::Subscribe
                | SipMethod::Refer
                | SipMethod::Notify
                | SipMethod::Update
        ) {
            return Err(Error::Dialog(format!(
                "The {} method cannot establish a dialog",
                request.req_line.method
            )));
        }
        let mandatory_headers = &request.incoming_info.mandatory_headers;
        if mandatory_headers.to.tag.is_some() {
            return Err(Error::Dialog(
                "The To header tag is added only by the server (UAS) in the response.".to_owned(),
            ));
        }
        let dialog_id = DialogId {
            call_id: mandatory_headers.call_id.clone(),
            remote_tag: mandatory_headers.from.tag().map(|t| t.to_owned()),
            local_tag: crate::generate_tag_n(8),
        };

        let (sender, receiver) = mpsc::channel(10);

        endpoint
            .ua_plugin()
            .register_dialog(dialog_id.clone(), sender);

        let dialog = Dialog {
            dialog_id,
            remote_cseq: mandatory_headers.cseq.cseq(),
            local_cseq: 0,
            from: mandatory_headers.from.clone(),
            to: mandatory_headers.to.clone(),
            secure: request.incoming_info.transport_msg.transport.is_secure()
                && request.req_line.uri.scheme == Scheme::Sips,
            route_set: RouteSet::from_headers(&request.headers),
            state: DialogState::Initial,
            role: Role::Uas,
            contact,
            receiver,
            endpoint,
        };

        Ok(dialog)
    }

    pub fn create_response(
        &self,
        server_tsx: &ServerTransaction,
        status_code: StatusCode,
    ) -> OutgoingResponse {
        let mut response = server_tsx.create_response(status_code, None);
        let headers = &mut response.headers;

        let allow = self.endpoint.allow();
        let supported = self.endpoint.supported();

        let code = status_code.as_u16();

        if matches!(code, 101..=399 | 485) && !headers.iter().any(|hdr| hdr.is_contact()) {
            headers.push(Header::Contact(self.contact.clone()));
        }

        if matches!(code,180..=189 | 200..=299 | 405)
            && !allow.is_empty()
            && !headers.iter().any(|hdr| hdr.is_allow())
        {
            headers.push(Header::Allow(allow.clone()));
        }

        if matches!(code, 200..=299)
            && !supported.is_empty()
            && !headers.iter().any(|hdr| hdr.is_supported())
        {
            headers.push(Header::Supported(supported.clone()));
        }

        if code != 100 {
            let to = find_map_mut_header!(headers, To).expect("missing to header!");
            to.tag = Some(self.dialog_id.local_tag.clone());
        }

        response
    }

    pub async fn recv(&mut self) -> Option<IncomingMessage> {
        let msg = self.receiver.recv().await?;

        if let IncomingMessage::Request(incoming_request) = &msg {
            self.process_incoming_request(incoming_request).await;
        }

        Some(msg)
    }

    async fn process_incoming_request(&mut self, request: &IncomingRequest) {
        let request_cseq = request.incoming_info.mandatory_headers.cseq.cseq();

        if !matches!(request.req_line.method, SipMethod::Cancel | SipMethod::Ack)
            && request_cseq <= self.remote_cseq
        {
            let mut response = self.endpoint.create_outgoing_response(
                &request,
                StatusCode::ServerInternalError,
                Some("Invalid Cseq".into()),
            );
            if let Err(err) = self.endpoint.send_outgoing_response(&mut response).await {
                log::error!("Error sending response = {err:?}");
            }
        }

        self.remote_cseq = request_cseq;

        if request.req_line.method == SipMethod::Ack && self.state != DialogState::Confirmed {
            self.state = DialogState::Confirmed;
        }
    }

    pub(crate) fn set_state(&mut self, dialog_state: DialogState) {
        self.state = dialog_state;
    }
}

#[derive(Default)]
pub struct DialogPlugin {
    dialogs: RwLock<FxHashMap<DialogId, mpsc::Sender<IncomingMessage>>>,
}

impl DialogPlugin {
    pub(crate) fn register_dialog(
        &self,
        dialog_id: DialogId,
        sender: mpsc::Sender<IncomingMessage>,
    ) {
        let mut dialogs = self.dialogs.write().expect("Lock failed");

        dialogs.insert(dialog_id, sender);
    }

    pub(crate) fn remove_dialog(&self, dialog_id: &DialogId) {
        let mut dialogs = self.dialogs.write().expect("Lock failed");

        dialogs.remove(dialog_id);
    }

    pub(crate) fn get_dialog(&self, dialog_id: &DialogId) -> Option<mpsc::Sender<IncomingMessage>> {
        let dialogs = self.dialogs.read().expect("Lock failed");

        dialogs.get(dialog_id).cloned()
    }
}

#[async_trait::async_trait]
impl endpoint::Plugin for DialogPlugin {
    fn name(&self) -> &'static str {
        "dialog"
    }

    async fn on_receive_request(&self, mut request: ReceivedRequest<'_>, endpoint: &Endpoint) {
        let Some(dialog_id) = DialogId::from_incoming_request(&request) else {
            return;
        };

        let request = request.take();

        let Some(channel) = self.get_dialog(&dialog_id) else {
            if request.req_line.method != SipMethod::Ack {
                let mut response = endpoint.create_outgoing_response(
                    &request,
                    StatusCode::CallOrTransactionDoesNotExist,
                    None,
                );
                if let Err(err) = endpoint.send_outgoing_response(&mut response).await {
                    log::error!("Error sending response = {err:?}");
                }
            }
            return;
        };

        if channel
            .send(IncomingMessage::Request(request))
            .await
            .is_err()
        {
            log::error!("Error sending request to dialog");
        }
    }

    async fn on_receive_response(&self, _response: ReceivedResponse<'_>, _endpoint: &Endpoint) {}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DialogId {
    pub call_id: CallId,
    pub local_tag: String,
    pub remote_tag: Option<String>,
}

impl DialogId {
    pub fn from_incoming_request(request: &IncomingRequest) -> Option<Self> {
        let headers = &request.incoming_info.mandatory_headers;
        let call_id = headers.call_id.clone();

        let local_tag = match headers.to.tag() {
            Some(tag) => tag.to_owned(),
            None => return None,
        };

        let remote_tag = headers.from.tag().map(|t| t.to_owned());

        Some(Self {
            call_id,
            local_tag,
            remote_tag,
        })
    }
}

#[derive(Default)]
pub struct RouteSet {
    uri: Uri,
    params: Params,
}

impl RouteSet {
    pub fn from_headers(headers: &Headers) -> Vec<RouteSet> {
        headers
            .iter()
            .filter_map(|header| {
                if let Header::RecordRoute(route) = header {
                    Some(RouteSet {
                        uri: route.name_addr().uri.clone(),
                        params: route.params().clone(),
                    })
                } else {
                    None
                }
            })
            .collect()
    }
}
