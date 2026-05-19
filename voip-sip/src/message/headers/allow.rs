use std::fmt;

use itertools::Itertools;

use crate::error::Result;
use crate::macros;
use crate::message::SipMethod;
use crate::parser::{HeaderParse, SipParser};

#[derive(Debug, PartialEq, Eq, Default, Clone)]
pub struct Allow(Vec<SipMethod>);

impl HeaderParse for Allow {
    const NAME: &'static str = "Allow";

    fn parse(parser: &mut SipParser) -> Result<Self> {
        let allow = macros::collect_elems_separated_by_comma!(parser, {
            let method = parser.take_alphabetic();

            SipMethod::from(method)
        });

        Ok(Self(allow))
    }
}

impl Allow {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn push(&mut self, method: SipMethod) {
        self.0.push(method);
    }
}

impl fmt::Display for Allow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", Self::NAME, self.0.iter().format(", "))
    }
}
