#![warn(clippy::undocumented_unsafe_blocks)]

//! # rssip
//!
//! A rust library that implements the SIP protocol.
//!

mod core;
pub(crate) mod error;
pub mod macros;
pub mod message;
pub(crate) mod parser;
pub mod transaction;
pub(crate) mod transport;
pub mod ua_layer;

pub use core::endpoint::Endpoint;
pub use core::{endpoint, resolver};

pub use error::Result;
pub use sdp;
pub use transport::incoming::{IncomingMessage, IncomingRequest, IncomingResponse};
pub use transport::outgoing::{OutgoingRequest, OutgoingResponse};
pub mod utils {
    pub use utils::local_ip;
}

use std::fmt::{self, Debug};
use std::str::{
    FromStr, {self},
};

use error::Error;

/// Branch parameter prefix defined in RFC3261.
pub(crate) const RFC3261_BRANCH_ID: &str = "z9hG4bK";

use rand::distr::{Alphanumeric, SampleString};

use crate::message::param::Params;

pub(crate) fn generate_branch() -> String {
    generate_branch_n(8)
}

pub(crate) fn generate_branch_n(n: usize) -> String {
    let mut branch = String::with_capacity(RFC3261_BRANCH_ID.len() + n);
    branch.push_str(RFC3261_BRANCH_ID);
    Alphanumeric.append_string(&mut rand::rng(), &mut branch, n);
    branch
}

pub(crate) fn generate_tag_n(n: usize) -> String {
    random_str(n)
}

pub(crate) fn random_str(n: usize) -> String {
    Alphanumeric.sample_string(&mut rand::rng(), n)
}

#[must_use]
pub(crate) fn is_valid_port(v: u16) -> bool {
    matches!(v, 0..=65535)
}

/// Represents a quality value (q-value) used in SIP
/// headers.
///
/// The `Q` struct provides a method to parse a string
/// representation of a q-value into a `Q` instance. The
/// q-value is typically used to indicate the preference
/// of certain SIP headers.
///
/// # Examples
///
/// ```
/// use rssip::Q;
///
/// let q_value = "0.5".parse();
/// assert_eq!(q_value, Ok(Q(0, 5)));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub struct Q(pub u8, pub u8);

impl Q {
    pub fn new(a: u8, b: u8) -> Self {
        Self(a, b)
    }
}
impl From<u8> for Q {
    fn from(value: u8) -> Self {
        Self(value, 0)
    }
}
#[derive(Debug, PartialEq, Eq)]
pub struct ParseQError;

impl From<ParseQError> for Error {
    fn from(value: ParseQError) -> Self {
        Self::Other(format!("{:#?}", value))
    }
}

impl FromStr for Q {
    type Err = ParseQError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.rsplit_once('.') {
            Some((a, b)) => {
                let a = a.parse().map_err(|_| ParseQError)?;
                let b = b.parse().map_err(|_| ParseQError)?;
                Ok(Q(a, b))
            }
            None => match s.parse() {
                Ok(n) => Ok(Q(n, 0)),
                Err(_) => Err(ParseQError),
            },
        }
    }
}

impl fmt::Display for Q {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, ";q={}.{}", self.0, self.1)
    }
}

/// This type reprents an MIME type that indicates an
/// content format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MimeType {
    pub mtype: String,
    pub subtype: String,
}

/// The `media-type` that appears in `Accept` and
/// `Content-Type` SIP headers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaType {
    pub mimetype: MimeType,
    pub params: Params,
}

impl MediaType {
    /// Constructs a `MediaType` from a type and a subtype.
    pub fn new(mtype: String, subtype: String) -> Self {
        Self {
            mimetype: MimeType { mtype, subtype },
            params: Default::default(),
        }
    }

    pub fn parse(parser: &mut parser::SipParser) -> Result<Self> {
        let mtype = parser.token()?;
        parser.advance()?;
        let subtype = parser.token()?;
        let param = macros::parse_params!(parser);

        Ok(Self::from_parts(mtype, subtype, param))
    }

    pub fn from_static(s: &'static str) -> Result<Self> {
        Self::parse(&mut parser::SipParser::new(s.as_bytes()))
    }

    /// Constructs a `MediaType` with an optional
    /// parameters.
    pub fn from_parts(mtype: &str, subtype: &str, params: Params) -> Self {
        Self {
            mimetype: MimeType {
                mtype: mtype.into(),
                subtype: subtype.into(),
            },
            params,
        }
    }
}

impl fmt::Display for MediaType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let MediaType { mimetype, params } = self;
        write!(f, "{}/{}", mimetype.mtype, mimetype.subtype)?;
        write!(f, "{}", params)?;
        Ok(())
    }
}

/// Test utilities
#[cfg(test)]
mod test_utils {
    use std::str::FromStr;

    use bytes::Bytes;

    use crate::endpoint::{Endpoint, EndpointBuilder};
    use crate::message::Request;
    use crate::message::headers::{CSeq, CallId, From, Header, Headers, MaxForwards, To, Via};
    use crate::message::method::SipMethod;
    use crate::message::uri::Uri;
    use crate::transaction::{TsxPlugin};
    use crate::transport::incoming::{IncomingInfo, IncomingRequest, MandatoryHeaders};
    use crate::transport::{Packet, TransportHandle, TransportMessage};
    use crate::ua_layer::dialog::DialogPlugin;

    #[macro_export]
    macro_rules! assert_eq_tsx_state {
        ($watcher:expr, $state:expr $(,)?) => {
            $crate::assert_eq_state!($watcher, $state,)
        };
        ($watcher:expr, $state:expr, $($arg:tt)+) => {{
            let new_state =  {
                match tokio::time::timeout(std::time::Duration::from_millis(50), $watcher.recv()).await {
                    Ok(Err(err)) => panic!("{}", format!("The channel has been closed: {err}")),
                    Err(_) => panic!("timeout!"),
                    Ok(Ok(state)) => state,
                }
            };
            assert_eq!(new_state, $state, $($arg)+);
        }};
    }

    pub async fn create_test_endpoint() -> Endpoint {
        EndpointBuilder::new()
            .with_udp_addr("127.0.0.1:0")
            .with_plugin(DialogPlugin::default())
            .with_plugin(TsxPlugin::default())
            .build()
            .await
            .unwrap()
    }

    fn create_test_headers(method: SipMethod) -> Headers {
        let branch = crate::generate_branch();

        let via = Via::from_str(&format!(
            "SIP/2.0/UDP localhost:5060;branch={branch};received=127.0.0.1"
        ))
        .unwrap();
        let from = From::from_str("Alice <sip:alice@localhost>;tag=1928301774").unwrap();
        let to = To::from_str("Bob <sip:bob@localhost>").unwrap();
        let cid = CallId::from("a84b4c76e66710@pc33.atlanta.com");
        let mfowards = MaxForwards::new(70);
        let cseq = CSeq::new(1, method);

        crate::headers! {
            Header::Via(via),
            Header::From(from),
            Header::To(to),
            Header::CallId(cid),
            Header::CSeq(cseq),
            Header::MaxForwards(mfowards)
        }
    }

    pub fn create_test_request(method: SipMethod, transport: TransportHandle) -> IncomingRequest {
        let headers = create_test_headers(method);
        let target = format!("sip:{}", transport.local_addr());
        let uri = Uri::from_str(&target).unwrap();

        let mandatory_headers = MandatoryHeaders::from_headers(&headers).unwrap();

        let request = Request::with_headers(method, uri, headers);
        let packet = Packet::new(Bytes::new(), transport.local_addr());

        let transport_msg = TransportMessage { packet, transport };

        let incoming_info = IncomingInfo {
            transport_msg,
            mandatory_headers,
        };

        IncomingRequest {
            request,
            incoming_info: Box::new(incoming_info),
        }
    }
}
