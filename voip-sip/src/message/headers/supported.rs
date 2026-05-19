use std::{fmt, str};

use itertools::Itertools;

use crate::error::Result;
use crate::macros;
use crate::parser::{HeaderParse, SipParser};

#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct Supported(Vec<String>);

impl HeaderParse for Supported {
    const NAME: &'static str = "Supported";
    const SHORT_NAME: &'static str = "k";

    fn parse(parser: &mut SipParser) -> Result<Self> {
        let tags =
            macros::collect_elems_separated_by_comma!(parser, { parser.token()?.to_owned() });

        Ok(Self(tags))
    }
}

impl Supported {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    /// Add a new tag to the list of supported tags.
    pub fn add_tag(&mut self, tag: String) {
        self.0.push(tag);
    }
}

impl fmt::Display for Supported {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", Self::NAME, self.0.iter().format(", "))
    }
}
