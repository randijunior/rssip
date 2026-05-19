use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// An SIP Method.
///
/// This enum declares SIP methods as described by `RFC3261` and Others.
pub enum SipMethod {
    /// SIP INVITE Method.
    Invite,
    /// SIP ACK Method.
    Ack,
    /// SIP BYE Method.
    Bye,
    /// SIP CANCEL Method.
    Cancel,
    /// SIP REGISTER Method.
    Register,
    /// SIP OPTIONS Method.
    Options,
    /// SIP INFO Method.
    Info,
    /// SIP NOTIFY Method.
    Notify,
    /// SIP SUBSCRIBE Method.
    Subscribe,
    /// SIP UPDATE Method.
    Update,
    /// SIP REFER Method.
    Refer,
    /// SIP PRACK Method.
    Prack,
    /// SIP MESSAGE Method.
    Message,
    /// SIP PUBLISH Method.
    Publish,
    /// An unknown SIP method.
    Unknown,
}

impl SipMethod {
    pub fn is_invite(&self) -> bool {
        matches!(self, Self::Invite)
    }

    pub fn is_ack(&self) -> bool {
        matches!(self, Self::Ack)
    }

    /// Returns the string representation of a method.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            SipMethod::Invite => "INVITE",
            SipMethod::Ack => "ACK",
            SipMethod::Bye => "BYE",
            SipMethod::Cancel => "CANCEL",
            SipMethod::Register => "REGISTER",
            SipMethod::Options => "OPTIONS",
            SipMethod::Info => "INFO",
            SipMethod::Notify => "NOTIFY",
            SipMethod::Subscribe => "SUBSCRIBE",
            SipMethod::Update => "UPDATE",
            SipMethod::Refer => "REFER",
            SipMethod::Prack => "PRACK",
            SipMethod::Message => "MESSAGE",
            SipMethod::Publish => "PUBLISH",
            SipMethod::Unknown => "UNKNOWN-SipMethod",
        }
    }
}

impl From<&str> for SipMethod {
    fn from(value: &str) -> Self {
        value.as_bytes().into()
    }
}

impl From<&[u8]> for SipMethod {
    fn from(value: &[u8]) -> Self {
        match value {
            b"INVITE" => SipMethod::Invite,
            b"CANCEL" => SipMethod::Cancel,
            b"ACK" => SipMethod::Ack,
            b"BYE" => SipMethod::Bye,
            b"REGISTER" => SipMethod::Register,
            b"OPTIONS" => SipMethod::Options,
            b"INFO" => SipMethod::Info,
            b"NOTIFY" => SipMethod::Notify,
            b"SUBSCRIBE" => SipMethod::Subscribe,
            b"UPDATE" => SipMethod::Update,
            b"REFER" => SipMethod::Refer,
            b"PRACK" => SipMethod::Prack,
            b"MESSAGE" => SipMethod::Message,
            b"PUBLISH" => SipMethod::Publish,
            _ => SipMethod::Unknown,
        }
    }
}

impl fmt::Display for SipMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
