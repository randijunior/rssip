#![warn(clippy::undocumented_unsafe_blocks)]

//! # voip-sip
//!
//! A rust library that implements the SIP protocol.
//!

pub mod endpoint;
pub(crate) mod error;
pub mod macros;
pub mod message;
pub(crate) mod parser;
pub mod resolver;
pub mod transaction;
pub(crate) mod transport;
pub mod ua;

pub use endpoint::Endpoint;
pub use error::Result;
pub use transport::outgoing::{OutgoingRequest, OutgoingResponse};
pub mod utils {
    pub use utils::local_ip;
}

#[cfg(test)]
#[macro_use]
extern crate assert_matches;

#[cfg(test)]
pub(crate) mod test_utils;

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
/// use voip::Q;
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
