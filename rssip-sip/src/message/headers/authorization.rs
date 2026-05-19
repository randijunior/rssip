use std::fmt;

use crate::error::Result;
use crate::message::sip_auth::Credential;
use crate::parser::{HeaderParse, SipParser};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Authorization {
    credential: Credential,
}

impl HeaderParse for Authorization {
    const NAME: &'static str = "Authorization";

    fn parse(parser: &mut SipParser) -> Result<Self> {
        let credential = parser.parse_auth_credential()?;

        Ok(Self { credential })
    }
}

impl Authorization {
    pub fn new(credential: Credential) -> Self {
        Self { credential }
    }

    pub fn credential(&self) -> &Credential {
        &self.credential
    }
}

impl fmt::Display for Authorization {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", Self::NAME, self.credential)
    }
}
