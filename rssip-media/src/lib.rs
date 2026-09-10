use std::net::SocketAddr;

use tokio::sync::mpsc;

use crate::{
    rtp::{RtpSession, packet::RtpPacket},
    sdp::{MediaDescription, SessionDescription},
};

pub mod codec;
pub mod error;
pub mod negotiator;
pub mod rtp;
pub mod sdp;

pub enum MediaEvent {
    RtpPacket(RtpPacket),
}

// CallSession
// media_address
// Runs setup_rtp + build_answer + dialog.accept(200).

pub struct MediaSession {
    
}

impl MediaSession {
    pub async fn setup(sdp: &SessionDescription) -> std::io::Result<Self> {
        todo!()
    }
    pub async fn receive_event(&mut self) -> std::io::Result<MediaEvent> {
        todo!()
    }
}
