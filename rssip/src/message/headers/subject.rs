use std::{fmt, str};

use crate::error::Result;
use crate::parser::{HeaderParse, SipParser};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Subject(String);

impl HeaderParse for Subject {
    const NAME: &'static str = "Subject";
    const SHORT_NAME: &'static str = "s";

    fn parse(parser: &mut SipParser) -> Result<Self> {
        let subject = parser.read_line()?;

        Ok(Self(subject.to_owned()))
    }
}

impl fmt::Display for Subject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", Self::NAME, self.0)
    }
}
