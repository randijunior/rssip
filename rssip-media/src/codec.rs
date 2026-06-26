use std::borrow::Cow;

pub struct Codec {
    name: Cow<'static, str>,
    clock_rate: u32,               // clock rate in Hz
    payload_type: u8,              // pt
    channels: u16,                 // Number of audio channels (0 for video codecs)
    sdp_fmtp_line: Option<String>, // Format-specific parameters as SDP fmtp line
}

impl Codec {
    pub const ULAW: Self = Self::new("PCMU", 8000, 0, 1);
    pub const ALAW: Self = Self::new("PCMA", 8000, 8, 1);
    pub const OPUS: Self = Self::new("opus", 48_000, 96, 2);

    pub const fn new(name: &'static str, clock_rate: u32, payload_type: u8, channels: u16) -> Self {
        Self {
            name: Cow::Borrowed(name),
            clock_rate,
            payload_type,
            channels,
            sdp_fmtp_line: None,
        }
    }
}
