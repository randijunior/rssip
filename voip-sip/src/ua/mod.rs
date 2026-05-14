pub mod dialog;

use std::collections::HashMap;
use std::sync::Mutex;

use dialog::{DialogId, DialogMessage};
use tokio::sync::mpsc;

use crate::Endpoint;
use crate::endpoint::{self, ReceivedRequest, ReceivedResponse};
use crate::message::method::SipMethod;
use crate::message::status_code::StatusCode;
use crate::transport::incoming::IncomingRequest;

type Dialogs = HashMap<DialogId, mpsc::Sender<DialogMessage>>;

#[derive(Default)]
pub struct UaPlugin {
    dialogs: Mutex<Dialogs>,
}

impl UaPlugin {
    pub(crate) fn register_dialog(&self, dialog_id: DialogId, dialog: mpsc::Sender<DialogMessage>) {
        let mut dialogs = self.dialogs.lock().expect("Lock failed");

        dialogs.insert(dialog_id, dialog);
    }

    pub(crate) fn remove_dialog(&self, dialog_id: &DialogId) {
        let mut dialogs = self.dialogs.lock().expect("Lock failed");

        dialogs.remove(dialog_id);
    }

    pub(crate) fn get_dialog_from_request(
        &self,
        request: &IncomingRequest,
    ) -> Option<mpsc::Sender<DialogMessage>> {
        let dialog_id = DialogId::from_request(request)?;

        let dialogs = self.dialogs.lock().expect("Lock failed");

        dialogs.get(&dialog_id).cloned()
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
        let request = request.take();

        let Some(sender) = self.get_dialog_from_request(&request) else {
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

        if let Err(err) = sender.send(DialogMessage::Request(request)).await {
            log::error!("failed to send message to dialog! {err}");
        }
    }

    async fn on_receive_response(&self, _response: ReceivedResponse<'_>, _endpoint: &Endpoint) {}
}
