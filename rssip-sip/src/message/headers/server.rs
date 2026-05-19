use std::{fmt, str};

use crate::error::Result;
use crate::parser::{HeaderParse, SipParser};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Server(String);

impl HeaderParse for Server {
    const NAME: &'static str = "Server";

    fn parse(parser: &mut SipParser) -> Result<Self> {
        let server = parser.read_line()?;

        Ok(Server(server.to_owned()))
    }
}

impl fmt::Display for Server {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", Server::NAME, self.0)
    }
}
