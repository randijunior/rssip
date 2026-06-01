use std::sync;

use rustc_hash::{FxBuildHasher, FxHashMap};
use tokio::sync::mpsc::{self};

use super::Role;
use crate::endpoint::{self, ToTake};
use crate::message::method::SipMethod;
use crate::message::sip_uri::HostPort;
use crate::transport::incoming::{
    IncomingInfo, IncomingMessage, IncomingRequest, IncomingResponse,
};
use crate::{Endpoint, RFC3261_BRANCH_ID};

pub(crate) type TransactionEntry = mpsc::Sender<IncomingMessage>;

#[derive(Default)]
pub struct TsxPlugin {
    transactions: sync::Mutex<FxHashMap<TransactionKey, TransactionEntry>>,
}

impl TsxPlugin {
    pub fn with_capacity(capacity: usize) -> Self {
        let map = FxHashMap::with_capacity_and_hasher(capacity, FxBuildHasher);

        Self {
            transactions: sync::Mutex::new(map),
        }
    }

    #[inline]
    pub(crate) fn add_transaction(&self, key: TransactionKey, entry: TransactionEntry) {
        let mut map = self.transactions.lock().expect("Lock failed");

        map.insert(key, entry);
    }

    #[inline]
    pub(crate) fn remove_transaction(&self, key: &TransactionKey) {
        let mut map = self.transactions.lock().expect("Lock failed");

        map.remove(key);
    }

    #[inline]
    pub(crate) fn get_entry(&self, key: &TransactionKey) -> Option<TransactionEntry> {
        let map = self.transactions.lock().expect("Lock failed");

        map.get(key).cloned()
    }
}

#[async_trait::async_trait]
impl endpoint::Plugin for TsxPlugin {
    fn name(&self) -> &'static str {
        "tsx"
    }

    async fn on_incoming_request(&self, mut req: ToTake<'_, IncomingRequest>, _: &Endpoint) {
        let key = TransactionKey::from_request(&req);

        let Some(channel) = self.get_entry(&key) else {
            return;
        };

        let request = req.take();

        channel
            .send(IncomingMessage::Request(request))
            .await
            .unwrap();
    }

    async fn on_incoming_response(&self, mut res: ToTake<'_, IncomingResponse>, _: &Endpoint) {
        let key = TransactionKey::from_response(&res);

        let Some(channel) = self.get_entry(&key) else {
            return;
        };

        let response = res.take();

        let _res = channel.send(IncomingMessage::Response(response)).await;
    }
}

#[derive(PartialEq, Eq, Hash, Clone, Debug)]
pub enum TransactionKey {
    Rfc2543(Rfc2543),
    Rfc3261(Rfc3261),
}

impl TransactionKey {
    pub fn from_request(request: &IncomingRequest) -> Self {
        Self::from_incoming_info(&request.incoming_info, Role::Uas)
    }

    pub fn from_response(response: &IncomingResponse) -> Self {
        Self::from_incoming_info(&response.incoming_info, Role::Uac)
    }

    fn from_incoming_info(info: &IncomingInfo, role: Role) -> Self {
        match &info.mandatory_headers.via.branch {
            Some(branch) if branch.starts_with(RFC3261_BRANCH_ID) => {
                let branch = branch.to_owned();
                let method = info.mandatory_headers.cseq.method();

                Self::new_key_3261(role, method, branch)
            }
            _ => {
                todo!("create rfc 2543")
            }
        }
    }

    pub fn new_key_3261(role: Role, method: SipMethod, branch: String) -> Self {
        let method = if matches!(method, SipMethod::Invite | SipMethod::Ack) {
            None
        } else {
            Some(method)
        };

        Self::Rfc3261(Rfc3261 {
            role,
            branch,
            method,
        })
    }
}

#[derive(PartialEq, Eq, Hash, Clone, Debug)]
pub struct Rfc2543 {
    pub cseq: u32,
    pub from_tag: Option<String>,
    pub to_tag: Option<String>,
    pub call_id: String,
    pub via_host_port: HostPort,
    pub method: Option<SipMethod>,
}

#[derive(PartialEq, Eq, Hash, Clone, Debug)]
pub struct Rfc3261 {
    role: Role,
    branch: String,
    method: Option<SipMethod>,
}
