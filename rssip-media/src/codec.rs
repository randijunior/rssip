use std::borrow::Cow;

pub struct  Codec {
    name: Cow<'static, str>,
    payload_type: u8,
    sdp_fmtp_line: String,
    clock_rate: u32,
    channels: u16
}