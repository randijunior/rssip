use std::{fmt, str};

use crate::error::Result;
use crate::parser::{HeaderParse, SipParser};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct MimeVersion {
    major: u8,
    minor: u8,
}

impl HeaderParse for MimeVersion {
    const NAME: &'static str = "MIME-Version";

    fn parse(parser: &mut SipParser) -> Result<Self> {
        let (major, _, minor) = (
            parser.advance()? - b'0',
            parser.must_read(b'.')?,
            parser.advance()? - b'0',
        );

        Ok(Self { major, minor })
    }
}

impl fmt::Display for MimeVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}.{}", Self::NAME, self.major, self.minor)
    }
}
