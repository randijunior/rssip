use std::fmt;

use crate::error::Result;
use crate::message::sip_auth::Challenge;
use crate::parser::{HeaderParse, SipParser};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct WWWAuthenticate(Challenge);

impl HeaderParse for WWWAuthenticate {
    const NAME: &'static str = "WWW-Authenticate";

    fn parse(parser: &mut SipParser) -> Result<Self> {
        let challenge = parser.parse_auth_challenge()?;

        Ok(Self(challenge))
    }
}

impl fmt::Display for WWWAuthenticate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", Self::NAME, self.0)
    }
}
