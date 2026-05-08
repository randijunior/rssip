use tokio::sync::mpsc;

use crate::endpoint::Endpoint;
use crate::error::{DialogError, Error, Result};
use crate::message::headers::{CallId, Contact, From, Header, Headers, To};
use crate::message::method::SipMethod;
use crate::message::param::Params;
use crate::message::sip_uri::{Scheme, Uri};
use crate::transaction::Role;
use crate::transport::incoming::{IncomingRequest, IncomingResponse};
use crate::{OutgoingRequest, find_map_header};

/// Represents a SIP Dialog.
pub struct Dialog {
    id: DialogId,
    state: DialogState,
    remote_cseq: Option<u32>,
    local_cseq: Option<u32>,
    local: From,
    remote: To,
    target: Contact,
    secure: bool,
    route_set: Vec<RouteSet>,
    role: Role,
    endpoint: Endpoint,
    receiver: mpsc::Receiver<DialogMessage>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DialogState {
    Early,
    Confirmed,
}

impl Dialog {
    pub fn create_uas(
        request: &IncomingRequest,
        contact: Contact,
        endpoint: Endpoint,
        state: DialogState,
    ) -> Result<Self> {
        if !matches!(request.req_line.method, SipMethod::Invite) {
            return Err(DialogError::InvalidMethod.into());
        }
        let mandatory_headers = &request.incoming_info.mandatory_headers;

        if mandatory_headers.to.tag().is_some() {
            return Err(DialogError::ToCannotHaveTag.into());
        };

        let to = mandatory_headers.to.clone();
        let from = mandatory_headers.from.clone();

        let remote_cseq = Some(mandatory_headers.cseq.cseq());
        let local_cseq = None;

        let route_set = RouteSet::from_headers(&request.headers);
        let secure = request.incoming_info.transport_info.transport.is_secure()
            && request.req_line.uri.scheme == Scheme::Sips;

        let id = DialogId {
            call_id: mandatory_headers.call_id.clone(),
            remote_tag: from.tag().map(|t| t.to_owned()),
            local_tag: crate::generate_tag_n(8),
        };

        let (sender, receiver) = mpsc::channel(10);

        endpoint.ua_plugin().register_dialog(id.clone(), sender);

        let dialog = Self {
            endpoint,
            id,
            state,
            remote_cseq,
            local_cseq,
            local: from,
            remote: to,
            target: contact,
            secure,
            route_set,
            receiver,
            role: Role::UAS,
        };

        log::trace!("UAS dialog created");

        Ok(dialog)
    }

    pub fn create_uac(
        request: &OutgoingRequest,
        response: &IncomingResponse,
        endpoint: Endpoint,
    ) -> Result<Self> {
        let Some(contact) = find_map_header!(response.headers, Contact).cloned() else {
            return Err(Error::Other("Missing Contact header!".to_owned()));
        };
        let Some(cseq) = find_map_header!(request.headers, CSeq) else {
            return Err(Error::Other("Missing CSeq header!".to_owned()));
        };
        let Some(call_id) = find_map_header!(request.headers, CallId).cloned() else {
            return Err(Error::Other("Missing Call-ID header!".to_owned()));
        };

        let Some(from) = find_map_header!(request.headers, From).cloned() else {
            return Err(Error::Other("Missing From header!".to_owned()));
        };

        let Some(to) = find_map_header!(request.headers, To).cloned() else {
            return Err(Error::Other("Missing To header!".to_owned()));
        };

        let Some(local_tag) = from.tag.clone() else {
            return Err(Error::Other("Missing From tag!".to_owned()));
        };

        let Some(remote_tag) = response.incoming_info.mandatory_headers.to.tag.clone() else {
            return Err(Error::Other("Missing To tag!".to_owned()));
        };

        let local_cseq = Some(cseq.cseq());
        let remote_cseq: Option<u32> = None;

        let id = DialogId {
            call_id,
            remote_tag: Some(remote_tag),
            local_tag,
        };

        let secure = request.target_info.transport.is_secure()
            && request.request.req_line.uri.scheme == Scheme::Sips;
        let route_set = RouteSet::from_headers(&response.headers);

        let state = if response.status_line.code.as_u16() < 200 {
            DialogState::Early
        } else {
            DialogState::Confirmed
        };

        let (sender, receiver) = mpsc::channel(10);

        endpoint.ua_plugin().register_dialog(id.clone(), sender);

        let dialog = Self {
            endpoint,
            id,
            state,
            remote_cseq,
            local_cseq,
            local: from,
            remote: to,
            target: contact,
            secure,
            route_set,
            receiver,
            role: Role::UAC,
        };

        log::trace!("UAC dialog created");

        Ok(dialog)
    }
}

impl Drop for Dialog {
    fn drop(&mut self) {
        self.endpoint.ua_plugin().remove_dialog(&self.id);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DialogId {
    pub call_id: CallId,
    pub local_tag: String,
    pub remote_tag: Option<String>,
}

impl DialogId {
    pub fn from_request(request: &IncomingRequest) -> Option<Self> {
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

pub enum DialogMessage {
    Request(IncomingRequest),
    Response(IncomingResponse),
}

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
