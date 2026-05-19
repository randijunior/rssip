use std::ops;
use std::result::Result as StdResult;

use crate::error::{Error, Result};
use crate::message::headers::{CSeq, CallId, From as FromHeader, Header, Headers, To, Via};
use crate::message::{Request, Response};
use crate::parser::HeaderParse;

/// This type represents an received SIP request.
#[derive(Clone)]
pub struct IncomingRequest {
    /// The SIP message.
    pub request: Request,
    /// Incoming message info.
    pub incoming_info: Box<IncomingInfo>,
}

impl ops::Deref for IncomingRequest {
    type Target = Request;
    fn deref(&self) -> &Self::Target {
        &self.request
    }
}

/// This type represents an received SIP response.
#[derive(Clone)]
pub struct IncomingResponse {
    /// The SIP message.
    pub response: Response,
    /// Incoming message info.
    pub incoming_info: Box<IncomingInfo>,
}

impl ops::Deref for IncomingResponse {
    type Target = Response;
    fn deref(&self) -> &Self::Target {
        &self.response
    }
}

/// Incoming message info.
#[derive(Clone)]
pub struct IncomingInfo {
    /// The mandatory headers extracted from the message.
    pub mandatory_headers: MandatoryHeaders,
    /// The received transport packet.
    pub transport_msg: super::TransportMessage,
}

/// Represents the mandatory headers that every SIP message must contain.
#[derive(Clone)]
pub struct MandatoryHeaders {
    /// The topmost `Via` header.
    pub via: Via,
    /// The `From` header.
    pub from: FromHeader,
    /// The `To` header.
    pub to: To,
    /// The `Call-ID` header.
    pub call_id: CallId,
    /// The `CSeq` header.
    pub cseq: CSeq,
}

impl MandatoryHeaders {
    pub fn from_headers(headers: &Headers) -> Result<Self> {
        Self::try_from(headers)
    }
    pub fn into_headers(self) -> Headers {
        let mut headers = Headers::with_capacity(5);
        headers.push(Header::Via(self.via));
        headers.push(Header::From(self.from));
        headers.push(Header::To(self.to));
        headers.push(Header::CallId(self.call_id));
        headers.push(Header::CSeq(self.cseq));
        headers
    }
    /// Extracts a mandatory header.
    pub fn required<T>(header: Option<T>, name: &'static str) -> Result<T> {
        header.ok_or(Error::MissingHeader(name))
    }
}

impl TryFrom<&Headers> for MandatoryHeaders {
    type Error = Error;

    fn try_from(headers: &Headers) -> StdResult<Self, Self::Error> {
        let mut via: Option<Via> = None;
        let mut cseq: Option<CSeq> = None;
        let mut from: Option<FromHeader> = None;
        let mut call_id: Option<CallId> = None;
        let mut to: Option<To> = None;

        for header in headers.iter() {
            match header {
                Header::Via(v) if via.is_none() => via = Some(v.clone()),
                Header::From(f) => from = Some(f.clone()),
                Header::To(t) => to = Some(t.clone()),
                Header::CallId(c) => call_id = Some(c.clone()),
                Header::CSeq(c) => cseq = Some(*c),
                _ => (),
            }
        }
        let via = Self::required(via, Via::NAME)?;
        let from = Self::required(from, FromHeader::NAME)?;
        let to = Self::required(to, To::NAME)?;
        let call_id = Self::required(call_id, CallId::NAME)?;
        let cseq = Self::required(cseq, CSeq::NAME)?;

        Ok(MandatoryHeaders {
            via,
            from,
            to,
            call_id,
            cseq,
        })
    }
}
