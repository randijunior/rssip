use std::{fmt, str};

use crate::error::Result;
use crate::parser::{HeaderParse, SipParser};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[repr(transparent)]
pub struct MinExpires(u32);

impl HeaderParse for MinExpires {
    const NAME: &'static str = "Min-Expires";

    fn parse(parser: &mut SipParser) -> Result<Self> {
        let expires = parser.parse_u32()?;

        Ok(Self(expires))
    }
}

impl MinExpires {
    #[inline]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    #[inline]
    pub const fn as_u32(&self) -> u32 {
        self.0
    }
}

impl fmt::Display for MinExpires {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", Self::NAME, self.0)
    }
}
