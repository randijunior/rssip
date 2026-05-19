pub mod dialog;

use std::sync::RwLock;
use std::sync::atomic::Ordering;

use dialog::DialogId;
use rustc_hash::FxHashMap;

use self::dialog::Dialog;
use crate::Endpoint;
use crate::endpoint::{self, ReceivedRequest, ReceivedResponse};
use crate::message::method::SipMethod;
use crate::message::status_code::StatusCode;

#[derive(Default)]
pub struct UaPlugin {
    dialogs: RwLock<FxHashMap<DialogId, Dialog>>,
}

impl UaPlugin {
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
impl endpoint::Plugin for UaPlugin {
    fn name(&self) -> &'static str {
        "dialog-ua"
    }

    async fn on_receive_request(&self, mut request: ReceivedRequest<'_>, endpoint: &Endpoint) {
        if request.req_line.method == SipMethod::Cancel {
            return;
        }
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
                let _res = endpoint.send_outgoing_response(&mut response).await;
            }
            return;
        };
        let request_cseq = request.incoming_info.mandatory_headers.cseq.cseq();

        if dialog.remote_cseq().try_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            if request_cseq > current {
                Some(request_cseq)
            } else {
                None
            }
        }).is_err() {
            let warn_text = Some("Invalid Cseq".into());
            let mut response = endpoint.create_outgoing_response(
                &request,
                StatusCode::ServerInternalError,
                warn_text,
            );
            endpoint.send_outgoing_response(&mut response).await;

        }

        // handle
    }

    async fn on_receive_response(&self, _response: ReceivedResponse<'_>, _endpoint: &Endpoint) {}
}
