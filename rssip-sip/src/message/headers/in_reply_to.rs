use std::{fmt, str};

use itertools::Itertools;
use utils::scanner;

use crate::error::Result;
use crate::macros;
use crate::message::headers::CallId;
use crate::parser::{HeaderParse, SipParser};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct InReplyTo(Vec<CallId>);

impl HeaderParse for InReplyTo {
    const NAME: &'static str = "In-Reply-To";

    fn parse(parser: &mut SipParser) -> Result<Self> {
        let ids = macros::collect_elems_separated_by_comma!(parser, {
            let id = parser.consume_while(|b| !scanner::is_newline(b) && b != b',');
            let id = str::from_utf8(id)?;

            CallId::from(id)
        });

        Ok(Self(ids))
    }
}

impl fmt::Display for InReplyTo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", Self::NAME, self.0.iter().format(", "))
    }
}
