use tokio::sync::mpsc;

pub mod ua;

pub use ua::Ua;

use crate::endpoint::Endpoint;
use crate::error::{DialogError, Result};
use crate::message::ReasonPhrase;
use crate::message::headers::{CallId, Contact, From, Header, Headers, To};
use crate::message::method::SipMethod;
use crate::message::param::Params;
use crate::message::sip_uri::{Scheme, Uri};
use crate::message::status_code::StatusCode;
use crate::transaction::Role;
use crate::transport::incoming::{IncomingRequest, IncomingResponse};

/// Returns `true` if this method can establish a dialog
const fn can_establish_a_dialog(method: &SipMethod) -> bool {
    matches!(method, SipMethod::Invite)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DialogState {
    Initial,
    Early,
    Confirmed,
}

/// Represents a SIP Dialog.
pub struct Dialog {
    endpoint: Endpoint,
    pub(crate) id: DialogId,
    state: DialogState,
    remote_cseq: u32,
    local_cseq: Option<u32>,
    local: From,
    remote: To,
    pub(crate) target: Contact,
    secure: bool,
    route_set: Vec<RouteSet>,
    receiver: utils::PeekableReceiver<DialogMessage>,
    role: Role,
}

impl Dialog {
    pub fn new_uas(
        request: &IncomingRequest,
        contact: Contact,
        endpoint: Endpoint,
    ) -> Result<Self> {
        if !can_establish_a_dialog(&request.req_line.method) {
            return Err(DialogError::InvalidMethod.into());
        }
        let mandatory_headers = &request.incoming_info.mandatory_headers;

        if mandatory_headers.to.tag().is_some() {
            return Err(DialogError::ToCannotHaveTag.into());
        };

        let to = mandatory_headers.to.clone();
        let from = mandatory_headers.from.clone();

        let remote_cseq = mandatory_headers.cseq.cseq();
        let local_cseq = None;

        let route_set = RouteSet::from_headers(&request.request.headers);
        let secure = request.incoming_info.transport_info.transport.is_secure()
            && request.request.req_line.uri.scheme == Scheme::Sips;

        let dialog_id = DialogId {
            call_id: mandatory_headers.call_id.clone(),
            remote_tag: from.tag().map(|t| t.to_owned()),
            local_tag: crate::generate_tag_n(8),
        };

        let (sender, receiver) = mpsc::channel(10);

        endpoint
            .dialogs()
            .register_dialog(dialog_id.clone(), sender);

        let dialog = Self {
            endpoint,
            id: dialog_id,
            state: DialogState::Initial,
            remote_cseq,
            local_cseq,
            local: from,
            remote: to,
            target: contact,
            secure,
            route_set,
            receiver: receiver.into(),
            role: Role::UAS,
        };

        Ok(dialog)
    }

    pub(crate) fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }

    pub(crate) fn set_state(&mut self, state: DialogState) {
        self.state = state;
    }

    pub async fn on_request(&mut self) -> Result<Option<IncomingRequest>> {
        match self.receiver.recv().await {
            Some(DialogMessage::Request(request)) => {
                // Check CSeq.
                let request_cseq = request.incoming_info.mandatory_headers.cseq.cseq();

                if request_cseq <= self.remote_cseq
                    && !matches!(request.req_line.method, SipMethod::Ack | SipMethod::Cancel)
                {
                    let st_text = ReasonPhrase::from("Invalid Cseq");
                    let mut response = self.endpoint.create_outgoing_response(
                        &request,
                        StatusCode::ServerInternalError,
                        Some(st_text),
                    );
                    self.endpoint.send_outgoing_response(&mut response).await?;
                    return Ok(Some(request));
                }
                self.remote_cseq = request_cseq;

                Ok(Some(request))
            }
            Some(DialogMessage::Response(incoming_response)) => todo!(),
            None => Ok(None),
        }
    }
}

pub enum DialogMessage {
    Request(IncomingRequest),
    Response(IncomingResponse),
}

#[async_trait::async_trait]
pub trait DialogUsage: Sync + Send + 'static {
    async fn on_receive(&self, request: &mut Option<IncomingRequest>);
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

pub(super) struct RouteSet {
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

impl Drop for Dialog {
    fn drop(&mut self) {
        self.endpoint.dialogs().remove_dialog(&self.id);
    }
}
