use std::{fmt, io};

use crate::rtp::header::RtpHeader;

#[derive(Default, Clone)]
pub struct RtpPacket {
    pub header: RtpHeader,
    pub payload: bytes::Bytes,
}

impl fmt::Display for RtpPacket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let header = &self.header;
        let payload_len = self.payload.len();

        writeln!(f, "RTP PACKET:")?;
        writeln!(f, "\tVersion: {}", header.version)?;
        writeln!(f, "\tMarker: {}", header.marker)?;
        writeln!(f, "\tPayload Type: {}", header.payload_type)?;
        writeln!(f, "\tSequence Number: {}", header.sequence_number)?;
        writeln!(f, "\tTimestamp: {}", header.timestamp)?;
        writeln!(f, "\tSSRC: {} ({:x})", header.ssrc, header.ssrc)?;
        writeln!(f, "\tPayload Length: {}", payload_len)?;
        writeln!(f, "\tCSRC Count: {}", header.csrc_list.len())?;

        Ok(())
    }
}

impl RtpPacket {
    pub fn parse(raw_packet: &mut impl bytes::Buf) -> io::Result<Self> {
        let header = RtpHeader::parse(raw_packet)?;
        let payload_len = raw_packet.remaining();
        let payload = raw_packet.copy_to_bytes(payload_len);

        if header.padding {
            todo!()
        } else {
            Ok(Self { header, payload })
        }
    }
}
