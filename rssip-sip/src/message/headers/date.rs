use std::{fmt, str};

use crate::error::Result;
use crate::parser::{HeaderParse, SipParser};

#[derive(Debug, PartialEq, Eq, Clone)]
#[repr(transparent)]
pub struct Date(String);

impl HeaderParse for Date {
    const NAME: &'static str = "Date";

    fn parse(parser: &mut SipParser) -> Result<Self> {
        let date = parser.read_line()?;

        Ok(Self(date.to_owned()))
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", Self::NAME, self.0)
    }
}
