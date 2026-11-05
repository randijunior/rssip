use std::io;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RtcpHeader {
    pub version: u8,     /* protocol version */
    pub padding: bool,   /* padding flag */
    pub packet_type: u8, /* RTCP packet type */
    pub length: u16,
    pub count: u8,
}

impl RtcpHeader {
    pub fn parse(raw_packet: &mut impl bytes::Buf) -> io::Result<Self> {
        let b0 = raw_packet.try_get_u8()?;

        let version = b0 >> 6;

        let padding = (b0 & 0b0010_0000) != 0;
        let count = b0 & 0b0001_1111;

        let packet_type = raw_packet.try_get_u8()?;
        let length = raw_packet.try_get_u16()?;

        Ok(Self {
            version,
            padding,
            packet_type,
            length,
            count,
        })
    }
}
