use std::{fmt, str};

use crate::error::Result;
use crate::parser::{HeaderParse, SipParser};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Priority(String);

impl HeaderParse for Priority {
    const NAME: &'static str = "Priority";

    fn parse(parser: &mut SipParser) -> Result<Self> {
        let priority = parser.token()?;

        Ok(Self(priority.to_owned()))
    }
}

impl fmt::Display for Priority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", Self::NAME, self.0)
    }
}
