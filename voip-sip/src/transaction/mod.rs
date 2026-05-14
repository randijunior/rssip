//! Transaction Layer.

use std::time::Duration;

pub use client::ClientTransaction;
pub use manager::TsxPlugin;
pub use server::ServerTransaction;

use crate::transport::incoming::{IncomingRequest, IncomingResponse};

pub mod client;
pub(crate) mod fsm;
pub(crate) mod manager;
pub mod server;
pub(crate) mod timers;

#[derive(PartialEq, Eq, Hash, Clone, Debug, Copy)]
pub enum Role {
    UAS,
    UAC,
}

#[derive(Clone)]
pub enum TransactionMessage {
    Request(IncomingRequest),
    Response(IncomingResponse),
}
