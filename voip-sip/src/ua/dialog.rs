use std::fmt;
use std::sync::atomic::AtomicU32;
use std::sync::{Arc, Mutex};

use crate::error::Result;
use crate::message::headers::{CallId, Contact, From, Header, Headers, To};
use crate::message::param::Params;
use crate::message::sip_uri::{Scheme, Uri};
use crate::message::status_code::StatusCode;
use crate::transaction::{Role, ServerTransaction};
use crate::transport::incoming::IncomingRequest;
use crate::{Endpoint, OutgoingResponse, find_map_mut_header};

/// Represents a SIP Dialog.
#[derive(Clone)]
pub struct Dialog {
    inner: Arc<Inner>,
}

struct Inner {
    dialog_id: DialogId,
    state: Mutex<DialogState>,
    remote_cseq: Option<AtomicU32>,
    local_cseq: Option<AtomicU32>,
    from: From,
    to: To,
    contact: Contact,
    secure: bool,
    route_set: Vec<RouteSet>,
    role: Role,
}

impl Dialog {
    pub(super) fn new_uas(
        endpoint: &Endpoint,
        request: &IncomingRequest,
        contact: Contact,
    ) -> Dialog {
        let mandatory_headers = &request.incoming_info.mandatory_headers;
        debug_assert!(
            mandatory_headers.to.tag.is_none(),
            "The To header tag is added only by the server (UAS) in the response."
        );
        let dialog_id = DialogId {
            call_id: mandatory_headers.call_id.clone(),
            remote_tag: mandatory_headers.from.tag().map(|t| t.to_owned()),
            local_tag: crate::generate_tag_n(8),
        };
        let inner = Arc::new(Inner {
            dialog_id: dialog_id.clone(),
            remote_cseq: Some(AtomicU32::new(mandatory_headers.cseq.cseq())),
            local_cseq: None,
            from: mandatory_headers.from.clone(),
            to: mandatory_headers.to.clone(),
            secure: request.incoming_info.transport_msg.transport.is_secure()
                && request.req_line.uri.scheme == Scheme::Sips,
            route_set: RouteSet::from_headers(&request.headers),
            state: Mutex::new(DialogState::Initial),
            role: Role::UAS,
            contact,
        });

        let dialog = Dialog { inner };

        endpoint
            .ua_plugin()
            .register_dialog(dialog_id, dialog.clone());

        dialog
    }

    pub(super) async fn respond_provisional(
        &self,
        server_tsx: &mut ServerTransaction,
        status_code: StatusCode,
    ) -> Result<()> {
        let response = self.create_response(server_tsx, status_code);

        server_tsx.send_provisional_response(response).await?;

        self.set_state(DialogState::Early);

        Ok(())
    }

    fn create_response(
        &self,
        server_tsx: &ServerTransaction,
        status_code: StatusCode,
    ) -> OutgoingResponse {
        let mut response = server_tsx.create_response(status_code, None);
        let headers = &mut response.headers;

        let allow = server_tsx.endpoint().allow();
        let supported = server_tsx.endpoint().supported();

        let code = status_code.as_u16();

        if matches!(code, 101..=399 | 485) && !headers.iter().any(|hdr| hdr.is_contact()) {
            headers.push(Header::Contact(self.inner.contact.clone()));
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
            to.tag = Some(self.inner.dialog_id.local_tag.clone());
        }

        response
    }

    fn set_state(&self, dialog_state: DialogState) {
        let mut state = self.inner.state.lock().expect("poisoned");
        *state = dialog_state;
    }
}

enum DialogState {
    Initial,
    Early,
    Confirmed,
}

impl fmt::Display for DialogState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DialogState::Initial => f.write_str("INITIAL"),
            DialogState::Early => f.write_str("EARLY"),
            DialogState::Confirmed => f.write_str("CONFIRMED"),
        }
    }
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
