use std::{fmt, str};

use crate::error::Result;
use crate::parser::{HeaderParse, SipParser};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[repr(transparent)]
pub struct Expires(u32);

impl HeaderParse for Expires {
    const NAME: &'static str = "Expires";

    fn parse(parser: &mut SipParser) -> Result<Expires> {
        let expires = parser.parse_u32()?;

        Ok(Self(expires))
    }
}

impl fmt::Display for Expires {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", Expires::NAME, self.0)
    }
}
