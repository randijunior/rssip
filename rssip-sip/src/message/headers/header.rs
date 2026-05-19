use std::fmt;

use enum_as_inner::EnumAsInner;

use crate::message::headers::*;

/// A SIP Header.
///
/// This enum contain the SIP headers, as defined in `RFC3261`.
#[derive(Debug, PartialEq, EnumAsInner, Clone)]
pub enum Header {
    /// `Accept` Header
    Accept(Accept),
    /// `Accept-Enconding` Header
    AcceptEncoding(AcceptEncoding),
    /// `Accept-Language` Header
    AcceptLanguage(AcceptLanguage),
    /// `Alert-Info` Header.
    AlertInfo(AlertInfo),
    /// `Allow` Header
    Allow(Allow),
    /// `Authentication-Info` Header
    AuthenticationInfo(AuthenticationInfo),
    /// `Authorization` Header
    Authorization(Authorization),
    /// `Call-ID` Header
    CallId(CallId),
    /// `Call-Info` Header
    CallInfo(CallInfo),
    /// `Contact` Header
    Contact(Contact),
    /// `Content-Disposition` Header
    ContentDisposition(ContentDisposition),
    /// `Content-Encoding` Header
    ContentEncoding(ContentEncoding),
    /// `Content-Language` Header
    ContentLanguage(ContentLanguage),
    /// `Content-Length` Header
    ContentLength(ContentLength),
    /// `Content-Type` Header
    ContentType(ContentType),
    /// `CSeq` Header
    CSeq(CSeq),
    /// `Date` Header
    Date(Date),
    /// `Error-Info` Header
    ErrorInfo(ErrorInfo),
    /// `Expires` Header
    Expires(Expires),
    /// `From` Header
    From(From),
    /// `In-Reply-To` Header
    InReplyTo(InReplyTo),
    /// `Max-Fowards` Header
    MaxForwards(MaxForwards),
    /// `Min-Expires` Header
    MinExpires(MinExpires),
    /// `MIME-Version` Header
    MimeVersion(MimeVersion),
    /// `Organization` Header
    Organization(Organization),
    /// `Priority` Header
    Priority(Priority),
    /// `Proxy-Authenticate` Header
    ProxyAuthenticate(ProxyAuthenticate),
    /// `Proxy-Authorization` Header
    ProxyAuthorization(ProxyAuthorization),
    /// `Proxy-Require` Header
    ProxyRequire(ProxyRequire),
    /// `Retry-After` Header
    RetryAfter(RetryAfter),
    /// `Route` Header
    Route(Route),
    /// `Record-Route` Header
    RecordRoute(RecordRoute),
    /// `Reply-To` Header
    ReplyTo(ReplyTo),
    /// `Require` Header
    Require(Require),
    /// `Server` Header
    Server(Server),
    /// `Subject` Header
    Subject(Subject),
    /// `Supported` Header
    Supported(Supported),
    /// `Timestamp` Header
    Timestamp(Timestamp),
    /// `To` Header
    To(To),
    /// `Unsupported` Header
    Unsupported(Unsupported),
    /// `User-Agent` Header
    UserAgent(UserAgent),
    /// `Via` Header
    Via(Via),
    /// `Warning` Header
    Warning(Warning),
    /// `WWW-Authenticate` Header
    WWWAuthenticate(WWWAuthenticate),
    /// Other Generic Header
    RawHeader(RawHeader),
}

impl fmt::Display for Header {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Header::Accept(accept) => accept.fmt(f),
            Header::AcceptEncoding(accept_encoding) => accept_encoding.fmt(f),
            Header::AcceptLanguage(accept_language) => accept_language.fmt(f),
            Header::AlertInfo(alert_info) => alert_info.fmt(f),
            Header::Allow(allow) => allow.fmt(f),
            Header::AuthenticationInfo(authentication_info) => authentication_info.fmt(f),
            Header::Authorization(authorization) => authorization.fmt(f),
            Header::CallId(call_id) => call_id.fmt(f),
            Header::CallInfo(call_info) => call_info.fmt(f),
            Header::Contact(contact) => contact.fmt(f),
            Header::ContentDisposition(content_disposition) => content_disposition.fmt(f),
            Header::ContentEncoding(content_encoding) => content_encoding.fmt(f),
            Header::ContentLanguage(content_language) => content_language.fmt(f),
            Header::ContentLength(content_length) => content_length.fmt(f),
            Header::ContentType(content_type) => content_type.fmt(f),
            Header::CSeq(cseq) => cseq.fmt(f),
            Header::Date(date) => date.fmt(f),
            Header::ErrorInfo(error_info) => error_info.fmt(f),
            Header::Expires(expires) => expires.fmt(f),
            Header::From(from) => from.fmt(f),
            Header::InReplyTo(in_reply_to) => in_reply_to.fmt(f),
            Header::MaxForwards(max_forwards) => max_forwards.fmt(f),
            Header::MinExpires(min_expires) => min_expires.fmt(f),
            Header::MimeVersion(mime_version) => mime_version.fmt(f),
            Header::Organization(organization) => organization.fmt(f),
            Header::Priority(priority) => priority.fmt(f),
            Header::ProxyAuthenticate(proxy_authenticate) => proxy_authenticate.fmt(f),
            Header::ProxyAuthorization(proxy_authorization) => proxy_authorization.fmt(f),
            Header::ProxyRequire(proxy_require) => proxy_require.fmt(f),
            Header::RetryAfter(retry_after) => retry_after.fmt(f),
            Header::Route(route) => route.fmt(f),
            Header::RecordRoute(record_route) => record_route.fmt(f),
            Header::ReplyTo(reply_to) => reply_to.fmt(f),
            Header::Require(require) => require.fmt(f),
            Header::Server(server) => server.fmt(f),
            Header::Subject(subject) => subject.fmt(f),
            Header::Supported(supported) => supported.fmt(f),
            Header::Timestamp(timestamp) => timestamp.fmt(f),
            Header::To(to) => to.fmt(f),
            Header::Unsupported(unsupported) => unsupported.fmt(f),
            Header::UserAgent(user_agent) => user_agent.fmt(f),
            Header::Via(via) => via.fmt(f),
            Header::Warning(warning) => warning.fmt(f),
            Header::WWWAuthenticate(wwwauthenticate) => wwwauthenticate.fmt(f),
            Header::RawHeader(raw_header) => raw_header.fmt(f),
        }
    }
}
