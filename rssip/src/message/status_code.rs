use std::borrow;

use crate::{error, message};

/// Status Code enum for SIP messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd)]
#[repr(u16)]
pub enum StatusCode {
    ///`Trying` status code.
    Trying = 100,
    ///`Ringing` status code.
    Ringing = 180,
    ///`Call Is Being Forwarded` status code.
    CallIsBeingForwarded = 181,
    ///`Queued` status code.
    Queued = 182,
    ///`InvSession Progress` status code.
    SessionProgress = 183,
    ///`Early Dialog Terminated` status code.
    EarlyDialogTerminated = 199,
    ///`OK` status code.
    Ok = 200,
    ///`Accepted` status code.
    Accepted = 202,
    ///`No Notification` status code.
    NoNotification = 204,
    ///`Multiple Choices` status code.
    MultipleChoices = 300,
    ///`Moved Permanently` status code.
    MovedPermanently = 301,
    ///`Moved Temporarily` status code.
    MovedTemporarily = 302,
    ///`Use Proxy` status code.
    UseProxy = 305,
    ///`Alternative Service` status code.
    AlternativeService = 380,
    ///`Bad Request` status code.
    BadRequest = 400,
    ///`Unauthorized` status code.
    Unauthorized = 401,
    ///`Payment Required` status code.
    PaymentRequired = 402,
    ///`Forbidden` status code.
    Forbidden = 403,
    ///`Not Found` status code.
    NotFound = 404,
    ///`Method Not Allowed` status code.
    MethodNotAllowed = 405,
    ///`Not Acceptable` status code.
    NotAcceptable = 406,
    ///`Proxy Authentication Required` status code.
    ProxyAuthenticationRequired = 407,
    ///`Request Timeout` status code.
    RequestTimeout = 408,
    ///`Conflict` status code.
    Conflict = 409,
    ///`Gone` status code.
    Gone = 410,
    ///`Length Required` status code.
    LengthRequired = 411,
    ///`Conditional Request Failed` status code.
    ConditionalRequestFailed = 412,
    ///`Request Entity Too Large` status code.
    RequestEntityTooLarge = 413,
    ///`Request URI Too Long` status code.
    RequestUriTooLong = 414,
    ///`Unsupported Media Type` status code.
    UnsupportedMediaType = 415,
    ///`Unsupported URI Scheme` status code.
    UnsupportedUriScheme = 416,
    ///`Unknown Resource Priority` status code.
    UnknownResourcePriority = 417,
    ///`Bad Extension` status code.
    BadExtension = 420,
    ///`Extension Required` status code.
    ExtensionRequired = 421,
    ///`InvSession Timer Too Small` status code.
    SessionIntervalTooSmall = 422,
    ///`Interval Too Brief` status code.
    IntervalTooBrief = 423,
    ///`Bad Location Information` status code.
    BadLocationInformation = 424,
    ///`Use Identity Header` status code.
    UseIdentityHeader = 428,
    ///`Provide Referrer Header` status code.
    ProvideReferrerIdentity = 429,
    ///`Flow Failed` status code.
    FlowFailed = 430,
    ///`Anonymity Disallowed` status code.
    AnonymityDisallowed = 433,
    ///`Bad Identity Info` status code.
    BadIdentityInfo = 436,
    ///`Unsupported Certificate` status code.
    UnsupportedCertificate = 437,
    ///`Invalid Identity Header` status code.
    InvalidIdentityHeader = 438,
    ///`First Hop Lacks Outbound Support` status code.
    FirstHopLacksOutboundSupport = 439,
    ///`Max Breadth Exceeded` status code.
    MaxBreadthExceeded = 440,
    ///`Bad Info Package` status code.
    BadInfoPackage = 469,
    ///`Consent Needed` status code.
    ConsentNeeded = 470,
    ///`Temporarily Unavailable` status code.
    TemporarilyUnavailable = 480,
    ///`Call or Transaction Does Not Exist` status code.
    CallOrTransactionDoesNotExist = 481,
    ///`Loop Detected` status code.
    LoopDetected = 482,
    ///`Too Many Hops` status code.
    TooManyHops = 483,
    ///`Address Incomplete` status code.
    AddressIncomplete = 484,
    ///`Ambiguous` status code.
    Ambiguous = 485,
    ///`Busy Here` status code.
    BusyHere = 486,
    ///`Request Terminated` status code.
    RequestTerminated = 487,
    ///`Not Acceptable Here` status code.
    NotAcceptableHere = 488,
    ///`Bad Event` status code.
    BadEvent = 489,
    ///`Request Updated` status code.
    RequestUpdated = 490,
    ///`Request Pending` status code.
    RequestPending = 491,
    ///`Undecipherable` status code.
    Undecipherable = 493,
    ///`Security Agreement Needed` status code.
    SecurityAgreementRequired = 494,
    ///`Server Internal Error` status code.
    ServerInternalError = 500,
    ///`Not Implemented` status code.
    NotImplemented = 501,
    ///`Bad Gateway` status code.
    BadGateway = 502,
    ///`Service Unavailable` status code.
    ServiceUnavailable = 503,
    ///`Server Timeout` status code.
    ServerTimeout = 504,
    ///`Version Not Supported` status code.
    VersionNotSupported = 505,
    ///`SipMessage Too Large` status code.
    MessageTooLarge = 513,
    ///`Push Notification Service Not Supported` status code.
    PushNotificationServiceNotSupported = 555,
    ///`Precondition Failure` status code.
    PreconditionFailure = 580,
    ///`Busy Everywhere` status code.
    BusyEverywhere = 600,
    ///`Decline` status code.
    Decline = 603,
    ///`Does Not Exist Anywhere` status code.
    DoesNotExistAnywhere = 604,
    ///`Not Acceptable Anywhere` status code.
    NotAcceptableAnywhere = 606,
    ///`Unwanted` status code.
    Unwanted = 607,
    ///`Rejected` status code.
    Rejected = 608,
}

impl StatusCode {
    pub fn from_u16(input: u16) -> Option<Self> {
        match input {
            100 => Some(Self::Trying),
            180 => Some(Self::Ringing),
            181 => Some(Self::CallIsBeingForwarded),
            182 => Some(Self::Queued),
            183 => Some(Self::SessionProgress),
            199 => Some(Self::EarlyDialogTerminated),
            200 => Some(Self::Ok),
            202 => Some(Self::Accepted),
            204 => Some(Self::NoNotification),
            300 => Some(Self::MultipleChoices),
            301 => Some(Self::MovedPermanently),
            302 => Some(Self::MovedTemporarily),
            305 => Some(Self::UseProxy),
            380 => Some(Self::AlternativeService),
            400 => Some(Self::BadRequest),
            401 => Some(Self::Unauthorized),
            402 => Some(Self::PaymentRequired),
            403 => Some(Self::Forbidden),
            404 => Some(Self::NotFound),
            405 => Some(Self::MethodNotAllowed),
            406 => Some(Self::NotAcceptable),
            407 => Some(Self::ProxyAuthenticationRequired),
            408 => Some(Self::RequestTimeout),
            409 => Some(Self::Conflict),
            410 => Some(Self::Gone),
            411 => Some(Self::LengthRequired),
            412 => Some(Self::ConditionalRequestFailed),
            413 => Some(Self::RequestEntityTooLarge),
            414 => Some(Self::RequestUriTooLong),
            415 => Some(Self::UnsupportedMediaType),
            416 => Some(Self::UnsupportedUriScheme),
            417 => Some(Self::UnknownResourcePriority),
            420 => Some(Self::BadExtension),
            421 => Some(Self::ExtensionRequired),
            422 => Some(Self::SessionIntervalTooSmall),
            423 => Some(Self::IntervalTooBrief),
            424 => Some(Self::BadLocationInformation),
            428 => Some(Self::UseIdentityHeader),
            429 => Some(Self::ProvideReferrerIdentity),
            430 => Some(Self::FlowFailed),
            433 => Some(Self::AnonymityDisallowed),
            436 => Some(Self::BadIdentityInfo),
            437 => Some(Self::UnsupportedCertificate),
            438 => Some(Self::InvalidIdentityHeader),
            439 => Some(Self::FirstHopLacksOutboundSupport),
            440 => Some(Self::MaxBreadthExceeded),
            469 => Some(Self::BadInfoPackage),
            470 => Some(Self::ConsentNeeded),
            480 => Some(Self::TemporarilyUnavailable),
            481 => Some(Self::CallOrTransactionDoesNotExist),
            482 => Some(Self::LoopDetected),
            483 => Some(Self::TooManyHops),
            484 => Some(Self::AddressIncomplete),
            485 => Some(Self::Ambiguous),
            486 => Some(Self::BusyHere),
            487 => Some(Self::RequestTerminated),
            488 => Some(Self::NotAcceptableHere),
            489 => Some(Self::BadEvent),
            490 => Some(Self::RequestUpdated),
            491 => Some(Self::RequestPending),
            493 => Some(Self::Undecipherable),
            494 => Some(Self::SecurityAgreementRequired),
            500 => Some(Self::ServerInternalError),
            501 => Some(Self::NotImplemented),
            502 => Some(Self::BadGateway),
            503 => Some(Self::ServiceUnavailable),
            504 => Some(Self::ServerTimeout),
            505 => Some(Self::VersionNotSupported),
            513 => Some(Self::MessageTooLarge),
            555 => Some(Self::PushNotificationServiceNotSupported),
            580 => Some(Self::PreconditionFailure),
            600 => Some(Self::BusyEverywhere),
            603 => Some(Self::Decline),
            604 => Some(Self::DoesNotExistAnywhere),
            606 => Some(Self::NotAcceptableAnywhere),
            607 => Some(Self::Unwanted),
            608 => Some(Self::Rejected),
            _ => None,
        }
    }

    /// Returns the reason text related to the status code.
    pub const fn reason(&self) -> message::ReasonPhrase {
        let reason_str = match self {
            Self::Trying => "Trying",
            Self::Ringing => "Ringing",
            Self::CallIsBeingForwarded => "Call Is Being Forwarded",
            Self::Queued => "Queued",
            Self::SessionProgress => "InviteSession Progress",
            Self::EarlyDialogTerminated => "Early Dialog Terminated",
            Self::Ok => "OK",
            Self::Accepted => "Accepted",
            Self::NoNotification => "No Notification",
            Self::MultipleChoices => "Multiple Choices",
            Self::MovedPermanently => "Moved Permanently",
            Self::MovedTemporarily => "Moved Temporarily",
            Self::UseProxy => "Use Proxy",
            Self::AlternativeService => "Alternative Service",
            Self::BadRequest => "Bad Request",
            Self::Unauthorized => "Unauthorized",
            Self::PaymentRequired => "Payment Required",
            Self::Forbidden => "Forbidden",
            Self::NotFound => "Not Found",
            Self::MethodNotAllowed => "Method Not Allowed",
            Self::NotAcceptable => "Not Acceptable",
            Self::ProxyAuthenticationRequired => "Proxy Authentication Required",
            Self::RequestTimeout => "Request Timeout",
            Self::Conflict => "Conflict",
            Self::Gone => "Gone",
            Self::LengthRequired => "Length Required",
            Self::ConditionalRequestFailed => "Conditional Request Failed",
            Self::RequestEntityTooLarge => "Request Entity Too Large",
            Self::RequestUriTooLong => "Request URI Too Long",
            Self::UnsupportedMediaType => "Unsupported Media Type",
            Self::UnsupportedUriScheme => "Unsupported URI Scheme",
            Self::UnknownResourcePriority => "Unknown Resource Priority",
            Self::BadExtension => "Bad Extension",
            Self::ExtensionRequired => "Extension Required",
            Self::SessionIntervalTooSmall => "InviteSession Interval Too Small",
            Self::IntervalTooBrief => "Interval Too Brief",
            Self::BadLocationInformation => "Bad Location Information",
            Self::UseIdentityHeader => "Use Identity Header",
            Self::ProvideReferrerIdentity => "Provide Referrer Identity",
            Self::FlowFailed => "Flow Failed",
            Self::AnonymityDisallowed => "Anonymity Disallowed",
            Self::BadIdentityInfo => "Bad Identity Info",
            Self::UnsupportedCertificate => "Unsupported Certificate",
            Self::InvalidIdentityHeader => "Invalid Identity Header",
            Self::FirstHopLacksOutboundSupport => "First Hop Lacks Outbound Support",
            Self::MaxBreadthExceeded => "Max Breadth Exceeded",
            Self::BadInfoPackage => "Bad Info Package",
            Self::ConsentNeeded => "Consent Needed",
            Self::TemporarilyUnavailable => "Temporarily Unavailable",
            Self::CallOrTransactionDoesNotExist => "Call or Transaction Does Not Exist",
            Self::LoopDetected => "Loop Detected",
            Self::TooManyHops => "Too Many Hops",
            Self::AddressIncomplete => "Address Incomplete",
            Self::Ambiguous => "Ambiguous",
            Self::BusyHere => "Busy Here",
            Self::RequestTerminated => "Request Terminated",
            Self::NotAcceptableHere => "Not Acceptable Here",
            Self::BadEvent => "Bad Event",
            Self::RequestUpdated => "Request Updated",
            Self::RequestPending => "Request Pending",
            Self::Undecipherable => "Undecipherable",
            Self::SecurityAgreementRequired => "Security Agreement Required",
            Self::ServerInternalError => "Server Internal Error",
            Self::NotImplemented => "Not Implemented",
            Self::BadGateway => "Bad Gateway",
            Self::ServiceUnavailable => "Service Unavailable",
            Self::ServerTimeout => "Server Timeout",
            Self::VersionNotSupported => "Version Not Supported",
            Self::MessageTooLarge => "Message Too Large",
            Self::PushNotificationServiceNotSupported => "Push Notification Service Not Supported",
            Self::PreconditionFailure => "Precondition Failure",
            Self::BusyEverywhere => "Busy Everywhere",
            Self::Decline => "Decline",
            Self::DoesNotExistAnywhere => "Does Not Exist Anywhere",
            Self::NotAcceptableAnywhere => "Not Acceptable Anywhere",
            Self::Unwanted => "Unwanted",
            Self::Rejected => "Rejected",
        };

        message::ReasonPhrase(borrow::Cow::Borrowed(reason_str))
    }

    #[inline]
    pub const fn as_u16(self) -> u16 {
        self as u16
    }

    #[inline]
    pub fn is_provisional(&self) -> bool {
        matches!(self.as_u16(), 100..=199)
    }

    #[inline]
    pub fn is_final(&self) -> bool {
        !self.is_provisional()
    }
}

impl TryFrom<u16> for StatusCode {
    type Error = error::Error;

    fn try_from(code: u16) -> Result<Self, Self::Error> {
        Self::from_u16(code).ok_or(error::Error::InvalidStatusCode)
    }
}

impl TryFrom<&[u8]> for StatusCode {
    type Error = error::Error;

    fn try_from(code: &[u8]) -> Result<Self, Self::Error> {
        Ok(match code {
            b"100" => Self::Trying,
            b"180" => Self::Ringing,
            b"181" => Self::CallIsBeingForwarded,
            b"182" => Self::Queued,
            b"183" => Self::SessionProgress,
            b"199" => Self::EarlyDialogTerminated,
            b"200" => Self::Ok,
            b"202" => Self::Accepted,
            b"204" => Self::NoNotification,
            b"300" => Self::MultipleChoices,
            b"301" => Self::MovedPermanently,
            b"302" => Self::MovedTemporarily,
            b"305" => Self::UseProxy,
            b"380" => Self::AlternativeService,
            b"400" => Self::BadRequest,
            b"401" => Self::Unauthorized,
            b"402" => Self::PaymentRequired,
            b"403" => Self::Forbidden,
            b"404" => Self::NotFound,
            b"405" => Self::MethodNotAllowed,
            b"406" => Self::NotAcceptable,
            b"407" => Self::ProxyAuthenticationRequired,
            b"408" => Self::RequestTimeout,
            b"409" => Self::Conflict,
            b"410" => Self::Gone,
            b"411" => Self::LengthRequired,
            b"412" => Self::ConditionalRequestFailed,
            b"413" => Self::RequestEntityTooLarge,
            b"414" => Self::RequestUriTooLong,
            b"415" => Self::UnsupportedMediaType,
            b"416" => Self::UnsupportedUriScheme,
            b"417" => Self::UnknownResourcePriority,
            b"420" => Self::BadExtension,
            b"421" => Self::ExtensionRequired,
            b"422" => Self::SessionIntervalTooSmall,
            b"423" => Self::IntervalTooBrief,
            b"424" => Self::BadLocationInformation,
            b"428" => Self::UseIdentityHeader,
            b"429" => Self::ProvideReferrerIdentity,
            b"430" => Self::FlowFailed,
            b"433" => Self::AnonymityDisallowed,
            b"436" => Self::BadIdentityInfo,
            b"437" => Self::UnsupportedCertificate,
            b"438" => Self::InvalidIdentityHeader,
            b"439" => Self::FirstHopLacksOutboundSupport,
            b"440" => Self::MaxBreadthExceeded,
            b"469" => Self::BadInfoPackage,
            b"470" => Self::ConsentNeeded,
            b"480" => Self::TemporarilyUnavailable,
            b"481" => Self::CallOrTransactionDoesNotExist,
            b"482" => Self::LoopDetected,
            b"483" => Self::TooManyHops,
            b"484" => Self::AddressIncomplete,
            b"485" => Self::Ambiguous,
            b"486" => Self::BusyHere,
            b"487" => Self::RequestTerminated,
            b"488" => Self::NotAcceptableHere,
            b"489" => Self::BadEvent,
            b"490" => Self::RequestUpdated,
            b"491" => Self::RequestPending,
            b"493" => Self::Undecipherable,
            b"494" => Self::SecurityAgreementRequired,
            b"500" => Self::ServerInternalError,
            b"501" => Self::NotImplemented,
            b"502" => Self::BadGateway,
            b"503" => Self::ServiceUnavailable,
            b"504" => Self::ServerTimeout,
            b"505" => Self::VersionNotSupported,
            b"513" => Self::MessageTooLarge,
            b"555" => Self::PushNotificationServiceNotSupported,
            b"580" => Self::PreconditionFailure,
            b"600" => Self::BusyEverywhere,
            b"603" => Self::Decline,
            b"604" => Self::DoesNotExistAnywhere,
            b"606" => Self::NotAcceptableAnywhere,
            b"607" => Self::Unwanted,
            b"608" => Self::Rejected,
            _ => return Err(error::Error::InvalidStatusCode),
        })
    }
}
