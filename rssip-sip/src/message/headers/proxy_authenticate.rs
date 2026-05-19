use std::fmt;

use crate::error::Result;
use crate::message::sip_auth::Challenge;
use crate::parser::{HeaderParse, SipParser};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ProxyAuthenticate(Challenge);

impl HeaderParse for ProxyAuthenticate {
    const NAME: &'static str = "Proxy-Authenticate";

    fn parse(parser: &mut SipParser) -> Result<Self> {
        let challenge = parser.parse_auth_challenge()?;

        Ok(Self(challenge))
    }
}

impl fmt::Display for ProxyAuthenticate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", Self::NAME, self.0)
    }
}
