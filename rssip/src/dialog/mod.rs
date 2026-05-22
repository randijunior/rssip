use std::sync::RwLock;

use rustc_hash::FxHashMap;

use crate::Endpoint;
use crate::endpoint::{self, ReceivedRequest, ReceivedResponse};
use crate::message::method::SipMethod;
use crate::message::status_code::StatusCode;
use crate::transport::incoming::IncomingMessage;
use std::result::Result as StdResult;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc;

use crate::error::{Error, Result};
use crate::message::headers::{CallId, Contact, From, Header, Headers, To};

use crate::message::param::Params;
use crate::message::sip_uri::{Scheme, Uri};

use crate::transaction::{Role, ServerTransaction};
use crate::transport::incoming::IncomingRequest;
use crate::{OutgoingResponse, find_map_mut_header};

#[derive(Debug, Clone)]
pub enum DialogState {
    Initial,
    Early,
    Established(mpsc::Sender<IncomingMessage>),
}

/// Represents a SIP Dialog.
#[derive(Clone)]
pub struct Dialog {
    inner: Arc<Inner>,
}

struct Inner {
    dialog_id: DialogId,
    state: Mutex<DialogState>,
    remote_cseq: AtomicU32,
    local_cseq: AtomicU32,
    from: From,
    to: To,
    contact: Contact,
    secure: bool,
    route_set: Vec<RouteSet>,
    role: Role,
}

impl Dialog {
    pub fn create_uas(
        endpoint: &Endpoint,
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
        let inner = Arc::new(Inner {
            dialog_id: dialog_id.clone(),
            remote_cseq: AtomicU32::new(mandatory_headers.cseq.cseq()),
            local_cseq: AtomicU32::new(0),
            from: mandatory_headers.from.clone(),
            to: mandatory_headers.to.clone(),
            secure: request.incoming_info.transport_msg.transport.is_secure()
                && request.req_line.uri.scheme == Scheme::Sips,
            route_set: RouteSet::from_headers(&request.headers),
            state: Mutex::new(DialogState::Initial),
            role: Role::Uas,
            contact,
        });
        let dialog = Dialog { inner };

        endpoint
            .ua_plugin()
            .register_dialog(dialog_id, dialog.clone());

        Ok(dialog)
    }

    pub fn create_response(
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

    pub fn set_state(&self, dialog_state: DialogState) {
        *self.inner.state.lock().expect("poisoned") = dialog_state;
    }

    pub fn state(&self) -> DialogState {
        self.inner.state.lock().expect("poisoned").clone()
    }

    pub fn update_remote_cseq(&self, new_value: u32) -> StdResult<u32, u32> {
        self.inner
            .remote_cseq
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |dialog_cseq| {
                if new_value > dialog_cseq {
                    Some(new_value)
                } else {
                    None
                }
            })
    }
}

#[derive(Default)]
pub struct DialogPlugin {
    dialogs: RwLock<FxHashMap<DialogId, Dialog>>,
}

impl DialogPlugin {
    pub(crate) fn register_dialog(&self, dialog_id: DialogId, dialog: Dialog) {
        let mut dialogs = self.dialogs.write().expect("Lock failed");

        dialogs.insert(dialog_id, dialog);
    }

    pub(crate) fn remove_dialog(&self, dialog_id: &DialogId) {
        let mut dialogs = self.dialogs.write().expect("Lock failed");

        dialogs.remove(dialog_id);
    }

    pub(crate) fn get_dialog(&self, dialog_id: &DialogId) -> Option<Dialog> {
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

        let Some(dialog) = self.get_dialog(&dialog_id) else {
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
        let request_cseq = request.incoming_info.mandatory_headers.cseq.cseq();

        if !matches!(request.req_line.method, SipMethod::Cancel | SipMethod::Ack)
            && dialog.update_remote_cseq(request_cseq).is_err()
        {
            let mut response = endpoint.create_outgoing_response(
                &request,
                StatusCode::ServerInternalError,
                Some("Invalid Cseq".into()),
            );
            if let Err(err) = endpoint.send_outgoing_response(&mut response).await {
                log::error!("Error sending response = {err:?}");
            }
        }

        if let DialogState::Established(sender) = dialog.state() {
            let _res = sender.send(IncomingMessage::Request(request)).await;
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
