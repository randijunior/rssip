use std::{fmt, str};

use crate::error::Result;
use crate::parser::{HeaderParse, SipParser};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Warning {
    code: u32,
    host: String,
    text: String,
}

impl HeaderParse for Warning {
    const NAME: &'static str = "Warning";

    fn parse(parser: &mut SipParser) -> Result<Self> {
        let code = parser.parse_u32()?;
        parser.skip_ws();
        let host = parser.read_host().to_owned();
        parser.skip_ws();
        parser.must_read(b'"')?;
        let text = parser.take_until(b'"');
        parser.advance()?;
        let text = str::from_utf8(text)?.to_owned();

        Ok(Self { code, host, text })
    }
}

impl fmt::Display for Warning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {} {} {}",
            Self::NAME,
            self.code,
            self.host,
            self.text
        )
    }
}
