use core::fmt;
use std::str;

use crate::MediaType;
use crate::error::Result;
use crate::parser::{HeaderParse, SipParser};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ContentType(MediaType);

impl HeaderParse for ContentType {
    const NAME: &'static str = "Content-Type";
    const SHORT_NAME: &'static str = "c";

    fn parse(parser: &mut SipParser) -> Result<Self> {
        let media_type = MediaType::parse(parser)?;

        Ok(Self(media_type))
    }
}

impl ContentType {
    pub fn new_sdp() -> Self {
        Self(MediaType {
            mimetype: crate::MimeType {
                mtype: "application".into(),
                subtype: "sdp".into(),
            },
            params: Default::default(),
        })
    }
}

impl fmt::Display for ContentType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", Self::NAME, self.0)
    }
}
