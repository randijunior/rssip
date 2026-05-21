mod dialog;
use std::sync::RwLock;

use dialog::DialogId;
use rustc_hash::FxHashMap;

pub use self::dialog::{Dialog, DialogState};
use crate::Endpoint;
use crate::endpoint::{self, ReceivedRequest, ReceivedResponse};
use crate::message::method::SipMethod;
use crate::message::status_code::StatusCode;
use crate::transport::incoming::IncomingMessage;

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

        if !matches!(request.req_line.method, SipMethod::Cancel | SipMethod::Ack) {
            if dialog.update_remote_cseq(request_cseq).is_err() {
                let mut response = endpoint.create_outgoing_response(
                    &request,
                    StatusCode::ServerInternalError,
                    Some("Invalid Cseq".into()),
                );
                if let Err(err) = endpoint.send_outgoing_response(&mut response).await {
                    log::error!("Error sending response = {err:?}");
                }
            }
        }

        if let DialogState::Established(sender) = dialog.state() {
            let _res = sender.send(IncomingMessage::Request(request)).await;
        }
    }

    async fn on_receive_response(&self, _response: ReceivedResponse<'_>, _endpoint: &Endpoint) {}
}
