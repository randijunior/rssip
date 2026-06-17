//! Transaction Layer.

pub use client::ClientTransaction;
pub use manager::TsxPlugin;
pub use server::ServerTransaction;

pub mod client;
pub mod server;

pub(crate) mod fsm;
pub(crate) mod manager;
pub(crate) mod timers;

#[derive(PartialEq, Eq, Hash, Clone, Debug, Copy)]
pub enum Role {
    Uac,
    Uas,
}
