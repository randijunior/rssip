use std::fmt;

use itertools::Itertools;

use crate::error::Result;
use crate::parser::{HeaderParse, SipParser};
use crate::{MediaType, macros};

#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct Accept(Vec<MediaType>);

impl HeaderParse for Accept {
    const NAME: &'static str = "Accept";

    fn parse(parser: &mut SipParser) -> Result<Accept> {
        let mtypes =
            macros::collect_elems_separated_by_comma!(parser, { MediaType::parse(parser)? });

        Ok(Self(mtypes))
    }
}

impl Accept {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    #[inline]
    pub fn push(&mut self, mtype: MediaType) {
        self.0.push(mtype);
    }
}

impl fmt::Display for Accept {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", Self::NAME, self.0.iter().format(", "))
    }
}
