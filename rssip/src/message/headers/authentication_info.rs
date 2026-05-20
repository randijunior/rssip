use std::{fmt, str};

use crate::error::Result;
use crate::message::sip_auth;
use crate::parser::{HeaderParse, SipParser};

#[derive(Debug, Default, PartialEq, Eq, Clone)]
pub struct AuthenticationInfo {
    nextnonce: Option<String>,
    qop: Option<String>,
    rspauth: Option<String>,
    cnonce: Option<String>,
    nc: Option<String>,
}

impl HeaderParse for AuthenticationInfo {
    const NAME: &'static str = "Authentication-Info";

    fn parse(parser: &mut SipParser) -> Result<Self> {
        let mut auth_info = Self::default();

        loop {
            parser.skip_ws();
            let (name, value) = parser.param_ref()?;
            match name {
                sip_auth::NEXTNONCE => auth_info.nextnonce = value.map(ToOwned::to_owned),
                sip_auth::QOP => auth_info.qop = value.map(ToOwned::to_owned),
                sip_auth::RSPAUTH => auth_info.rspauth = value.map(ToOwned::to_owned),
                sip_auth::CNONCE => auth_info.cnonce = value.map(ToOwned::to_owned),
                sip_auth::NC => auth_info.nc = value.map(ToOwned::to_owned),
                // TODO: error here
                _ => (),
            };

            if parser.take_if_eq(b',').is_none() {
                break;
            }
        }

        Ok(auth_info)
    }
}

impl fmt::Display for AuthenticationInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: ", Self::NAME)?;

        if let Some(nextnonce) = &self.nextnonce {
            write!(f, "nextnonce={}", nextnonce)?;
        }
        if let Some(qop) = &self.qop {
            write!(f, ", qop={}", qop)?;
        }
        if let Some(rspauth) = &self.rspauth {
            write!(f, ", rspauth={}", rspauth)?;
        }
        if let Some(cnonce) = &self.cnonce {
            write!(f, ", cnonce={}", cnonce)?;
        }
        if let Some(nc) = &self.nc {
            write!(f, ", nc={}", nc)?;
        }

        Ok(())
    }
}
