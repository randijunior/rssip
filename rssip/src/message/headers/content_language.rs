use std::{fmt, str};

use itertools::Itertools;

use crate::error::Result;
use crate::macros;
use crate::message::headers::LanguageTag;
use crate::parser::{HeaderParse, SipParser};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ContentLanguage(Vec<LanguageTag>);

impl HeaderParse for ContentLanguage {
    const NAME: &'static str = "Content-Language";

    fn parse(parser: &mut SipParser) -> Result<Self> {
        let languages =
            macros::collect_elems_separated_by_comma!(parser, { LanguageTag::parse(parser) });

        Ok(Self(languages))
    }
}

impl fmt::Display for ContentLanguage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", Self::NAME, self.0.iter().format(", "))
    }
}
