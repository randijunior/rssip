use std::{fmt, str};

use crate::error::Result;
use crate::parser::{HeaderParse, SipParser};

#[derive(Debug, PartialEq, Clone)]
pub struct Timestamp {
    time: f32,
    delay: Option<f32>,
}

impl HeaderParse for Timestamp {
    const NAME: &'static str = "Timestamp";

    fn parse(parser: &mut SipParser) -> Result<Self> {
        let time = parser.parse_f32()?;
        parser.skip_ws();

        let delay = if parser.peek().is_some_and(|b| b.is_ascii_digit()) {
            Some(parser.parse_f32()?)
        } else {
            None
        };

        Ok(Self { time, delay })
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", Self::NAME, self.time)?;

        if let Some(delay) = &self.delay {
            write!(f, "{}", delay)?;
        }

        Ok(())
    }
}
