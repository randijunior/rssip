use std::{fmt, str};

use crate::error::Result;
use crate::parser::{HeaderParse, SipParser};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Organization(String);

impl HeaderParse for Organization {
    const NAME: &'static str = "Organization";

    fn parse(parser: &mut SipParser) -> Result<Self> {
        let organization = parser.read_line()?;

        Ok(Self(organization.to_owned()))
    }
}

impl fmt::Display for Organization {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", Self::NAME, self.0)
    }
}
