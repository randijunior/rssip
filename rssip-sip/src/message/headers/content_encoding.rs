use core::fmt;
use std::str;

use itertools::Itertools;

use crate::error::Result;
use crate::macros;
use crate::parser::{HeaderParse, SipParser};

#[derive(Default, Debug, PartialEq, Eq, Clone)]
pub struct ContentEncoding(Vec<String>);

impl HeaderParse for ContentEncoding {
    const NAME: &'static str = "Content-Encoding";
    const SHORT_NAME: &'static str = "e";

    fn parse(parser: &mut SipParser) -> Result<Self> {
        let codings =
            macros::collect_elems_separated_by_comma!(parser, { parser.token()?.to_owned() });

        Ok(Self(codings))
    }
}

impl fmt::Display for ContentEncoding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", Self::NAME, self.0.iter().format(", "))
    }
}
