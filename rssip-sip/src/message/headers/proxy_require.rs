use std::{fmt, str};

use itertools::Itertools;

use crate::error::Result;
use crate::macros;
use crate::parser::{HeaderParse, SipParser};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ProxyRequire(Vec<String>);

impl HeaderParse for ProxyRequire {
    const NAME: &'static str = "Proxy-Require";

    fn parse(parser: &mut SipParser) -> Result<Self> {
        let tags =
            macros::collect_elems_separated_by_comma!(parser, { parser.token()?.to_owned() });

        Ok(Self(tags))
    }
}

impl fmt::Display for ProxyRequire {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", Self::NAME, self.0.iter().format(", "))
    }
}
