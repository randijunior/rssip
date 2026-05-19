use std::fmt;

use crate::error::Result;
use crate::message::sip_auth::Credential;
use crate::parser::{HeaderParse, SipParser};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ProxyAuthorization(Credential);

impl HeaderParse for ProxyAuthorization {
    const NAME: &'static str = "Proxy-Authorization";

    fn parse(parser: &mut SipParser) -> Result<Self> {
        let credential = parser.parse_auth_credential()?;

        Ok(Self(credential))
    }
}

impl fmt::Display for ProxyAuthorization {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", Self::NAME, self.0)
    }
}
