use std::{fmt, str};

use crate::error::Result;
use crate::parser::{HeaderParse, SipParser};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[repr(transparent)]
pub struct MaxForwards(u32);

impl HeaderParse for MaxForwards {
    const NAME: &'static str = "Max-Forwards";

    fn parse(parser: &mut SipParser) -> Result<MaxForwards> {
        let fowards = parser.parse_u32()?;

        Ok(Self(fowards))
    }
}

impl MaxForwards {
    pub const fn new(fowards: u32) -> Self {
        Self(fowards)
    }
}

impl fmt::Display for MaxForwards {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", Self::NAME, self.0)
    }
}
