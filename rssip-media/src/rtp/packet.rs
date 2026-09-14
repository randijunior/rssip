use std::io;

use crate::rtp::header::RtpHeader;

pub struct RtpPacket {
    header: RtpHeader,
    payload: RtpPayload,
}

pub struct RtpPayload(bytes::Bytes);

impl RtpPacket {
    pub fn parse(buff: &[u8]) -> io::Result<Self> {
        todo!()
    }
}
