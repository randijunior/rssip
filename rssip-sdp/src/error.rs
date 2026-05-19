pub type Result<T> = std::result::Result<T, Error>;

use thiserror::Error;
use utils::ScannerError;

#[derive(Debug, Error, PartialEq)]
pub enum Error {
    #[error("ParseSdpError : {0}")]
    ParseSdpError(#[from] ParseSdpError),

    #[error("empty time description")]
    SdpTimeDescriptionNotFound,
}

#[derive(Debug, Error, PartialEq)]
pub enum ParseSdpError {
    #[error("invalid protocol")]
    SdpInvalidProtocolVersion,

    #[error("unknow media type")]
    SdpUnknowMediaType,

    #[error("unknow sdp transport protocol")]
    SdpUnknowTransport,

    #[error("scanner error: {:#?}", 0)]
    ScannerError(ScannerError),

    #[error("syntax error: {}", s)]
    SyntaxError { s: String },
}

impl From<ScannerError> for Error {
    fn from(err: ScannerError) -> Self {
        Self::ParseSdpError(ParseSdpError::ScannerError(err))
    }
}
