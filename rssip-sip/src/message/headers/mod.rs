//! SIP Headers types

mod accept;
mod accept_encoding;
mod accept_language;
mod alert_info;
mod allow;
mod authentication_info;
mod authorization;
mod call_id;
mod call_info;
mod contact;
mod content_disposition;
mod content_encoding;
mod content_language;
mod content_length;
mod content_type;
mod cseq;
mod date;
mod error_info;
mod expires;
mod from;
mod header;
mod in_reply_to;
mod max_fowards;
mod mime_version;
mod min_expires;
mod organization;
mod priority;
mod proxy_authenticate;
mod proxy_authorization;
mod proxy_require;
mod raw_header;
mod record_route;
mod reply_to;
mod require;
mod retry_after;
mod route;
mod server;
mod subject;
mod supported;
mod timestamp;
mod to;
mod unsupported;
mod user_agent;
pub mod via;
mod warning;
mod www_authenticate;

use core::fmt;
use std::convert;
use std::ops::{Index, IndexMut, Range, RangeBounds, RangeFrom};
use std::vec::Splice;

pub use accept::Accept;
pub use accept_encoding::*;
pub use accept_language::*;
pub use alert_info::AlertInfo;
pub use allow::Allow;
pub use authentication_info::AuthenticationInfo;
pub use authorization::Authorization;
pub use call_id::CallId;
pub use call_info::CallInfo;
pub use contact::Contact;
pub use content_disposition::ContentDisposition;
pub use content_encoding::ContentEncoding;
pub use content_language::ContentLanguage;
pub use content_length::ContentLength;
pub use content_type::ContentType;
pub use cseq::CSeq;
pub use date::Date;
pub use error_info::ErrorInfo;
pub use expires::Expires;
pub use from::From;
pub use header::*;
pub use in_reply_to::InReplyTo;
pub use max_fowards::MaxForwards;
pub use mime_version::MimeVersion;
pub use min_expires::MinExpires;
pub use organization::Organization;
pub use priority::Priority;
pub use proxy_authenticate::ProxyAuthenticate;
pub use proxy_authorization::ProxyAuthorization;
pub use proxy_require::ProxyRequire;
pub use raw_header::RawHeader;
pub use record_route::RecordRoute;
pub use reply_to::ReplyTo;
pub use require::Require;
pub use retry_after::RetryAfter;
pub use route::Route;
pub use server::Server;
pub use subject::Subject;
pub use supported::Supported;
pub use timestamp::Timestamp;
pub use to::To;
pub use unsupported::Unsupported;
pub use user_agent::UserAgent;
pub use via::Via;
pub use warning::Warning;
pub use www_authenticate::WWWAuthenticate;

/// A colection of SIP Headers.
#[derive(Default, Debug, PartialEq, Clone)]
pub struct Headers(Vec<Header>);

impl Headers {
    #[inline]
    pub const fn new() -> Self {
        Self(Vec::new())
    }

    pub fn last(&self) -> Option<&Header> {
        self.0.last()
    }

    pub fn last_mut(&mut self) -> Option<&mut Header> {
        self.0.last_mut()
    }

    pub fn first_mut(&mut self) -> Option<&mut Header> {
        self.0.first_mut()
    }

    pub fn remove(&mut self, index: usize) -> Header {
        self.0.remove(index)
    }

    pub fn insert(&mut self, index: usize, header: Header) {
        self.0.insert(index, header);
    }

    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self(Vec::with_capacity(capacity))
    }

    #[inline]
    pub fn extend<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = Header>,
    {
        self.0.extend(iter);
    }

    pub fn splice<R, I>(&mut self, range: R, replace_with: I) -> Splice<'_, I::IntoIter>
    where
        R: RangeBounds<usize>,
        I: IntoIterator<Item = Header>,
    {
        self.0.splice(range, replace_with)
    }

    #[inline]
    pub fn iter(&self) -> core::slice::Iter<'_, Header> {
        self.0.iter()
    }

    #[inline]
    pub fn iter_mut(&mut self) -> core::slice::IterMut<'_, Header> {
        self.0.iter_mut()
    }

    #[inline]
    pub fn append(&mut self, other: &mut Self) {
        self.0.append(&mut other.0);
    }

    #[inline]
    pub fn push(&mut self, hdr: Header) {
        self.0.push(hdr);
    }

    pub fn insert_mut(&mut self, index: usize, hdr: Header) -> &mut Header {
        self.0.insert_mut(index, hdr)
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn get(&self, index: usize) -> Option<&Header> {
        self.0.get(index)
    }

    #[inline]
    pub fn pop(&mut self) -> Option<Header> {
        self.0.pop()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.0.capacity()
    }
}

impl IntoIterator for Headers {
    type Item = Header;

    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl Index<usize> for Headers {
    type Output = Header;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl IndexMut<usize> for Headers {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.0[index]
    }
}

impl Index<Range<usize>> for Headers {
    type Output = [Header];

    fn index(&self, range: Range<usize>) -> &Self::Output {
        &self.0[range]
    }
}

impl Index<RangeFrom<usize>> for Headers {
    type Output = [Header];

    fn index(&self, range: RangeFrom<usize>) -> &Self::Output {
        &self.0[range]
    }
}

impl<Header, const N: usize> convert::From<[Header; N]> for Headers
where
    Headers: FromIterator<Header>,
{
    fn from(array: [Header; N]) -> Self {
        array.into_iter().collect()
    }
}

impl convert::From<Vec<Header>> for Headers {
    fn from(headers: Vec<Header>) -> Self {
        Self(headers)
    }
}

impl FromIterator<Header> for Headers {
    fn from_iter<I: IntoIterator<Item = Header>>(iter: I) -> Self {
        Headers(iter.into_iter().collect())
    }
}

impl fmt::Display for Headers {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for hdr in self.iter() {
            write!(f, "{hdr}\r\n")?;
        }
        Ok(())
    }
}
