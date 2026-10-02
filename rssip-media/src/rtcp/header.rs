
#[derive(Default, Clone)]
pub struct RtcpHeaderCommon {
    version: u8, /* protocol version */
    padding: bool, /* padding flag */
    packet_type: u8, /* RTCP packet type */
    packet_len: u16
}