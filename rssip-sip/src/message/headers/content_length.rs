use std::{fmt, str};

use crate::error::Result;
use crate::parser::{HeaderParse, SipParser};

#[derive(Default, Debug, PartialEq, Eq, Clone, Copy)]
#[repr(transparent)]
pub struct ContentLength(u32);

impl HeaderParse for ContentLength {
    const NAME: &'static str = "Content-Length";
    const SHORT_NAME: &'static str = "l";

    fn parse(parser: &mut SipParser) -> Result<ContentLength> {
        let l = parser.parse_u32()?;

        Ok(Self(l))
    }
}

impl fmt::Display for ContentLength {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", Self::NAME, self.0)
    }
}

impl From<u32> for ContentLength {
    fn from(c_len: u32) -> Self {
        Self(c_len)
    }
}
