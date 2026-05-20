use core::fmt;
use std::str::{self, FromStr};

use crate::error::Result;
use crate::message::SipMethod;
use crate::parser::{HeaderParse, SipParser};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct CSeq {
    cseq: u32,
    method: SipMethod,
}

impl CSeq {
    pub fn new(cseq: u32, method: SipMethod) -> Self {
        Self { cseq, method }
    }

    pub fn cseq(&self) -> u32 {
        self.cseq
    }

    pub fn method(&self) -> SipMethod {
        self.method
    }
}

impl HeaderParse for CSeq {
    const NAME: &'static str = "CSeq";

    fn parse(parser: &mut SipParser) -> Result<CSeq> {
        let cseq = parser.parse_u32()?;

        parser.skip_ws();

        let method = parser.take_alphabetic();
        let method = SipMethod::from(method);

        Ok(Self { cseq, method })
    }
}

impl fmt::Display for CSeq {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {} {}", Self::NAME, self.cseq, self.method)
    }
}

impl FromStr for CSeq {
    type Err = crate::error::Error;

    fn from_str(s: &str) -> Result<Self> {
        Self::parse(&mut SipParser::new(s))
    }
}
