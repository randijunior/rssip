#[cfg(test)]
mod decode_test;

use std::io::{self, Result};

use bytes::BytesMut;
use tokio_util::bytes::Buf;
use tokio_util::codec::Decoder;

use crate::message::headers::ContentLength;
use crate::parser::HeaderParse;
use crate::transport::{KEEPALIVE_REQUEST, KEEPALIVE_RESPONSE, MSG_HEADERS_END};

pub struct StreamingDecoder {}

impl Default for StreamingDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamingDecoder {
    pub fn new() -> Self {
        Self {}
    }
}

impl Decoder for StreamingDecoder {
    type Error = std::io::Error;
    type Item = FramedMessage;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>> {
        // Check if is keep-alive.
        if src.len() >= 4 && &src[0..4] == KEEPALIVE_REQUEST {
            src.advance(4);
            return Ok(Some(FramedMessage::KeepaliveRequest));
        }
        if src.len() >= 2 && &src[0..2] == KEEPALIVE_RESPONSE {
            src.advance(2);
            return Ok(Some(FramedMessage::KeepaliveResponse));
        }

        // Find header end.
        let Some(hdr_end) = src
            .windows(MSG_HEADERS_END.len())
            .position(|window| window == MSG_HEADERS_END)
        else {
            return Ok(None);
        };

        let body_start = hdr_end + MSG_HEADERS_END.len();
        // Find "Content-Length" header
        let mut content_length = None;
        let lines = src[..hdr_end].split(|&b| b == b'\n');
        for line in lines {
            let mut split = line.splitn(2, |&c| c == b':');
            let Some(name) = split.next() else {
                continue;
            };

            if name.eq_ignore_ascii_case(ContentLength::NAME.as_bytes())
                || name.eq_ignore_ascii_case(ContentLength::SHORT_NAME.as_bytes())
            {
                let Some(value) = split.next() else {
                    continue;
                };
                let Ok(value_str) = std::str::from_utf8(value) else {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Invalid UTF-8 in Content-Length header",
                    ));
                };
                if let Ok(parsed_value) = value_str.trim().parse::<usize>() {
                    content_length = Some(parsed_value);
                }
            }
        }

        if let Some(c_len) = content_length {
            let expected_msg_size = body_start + c_len;
            if src.len() < expected_msg_size {
                src.reserve(expected_msg_size - src.len());
                return Ok(None);
            }
            let src_bytes = src.split_to(expected_msg_size);
            let completed_bytes = src_bytes.freeze();

            Ok(Some(FramedMessage::Complete(completed_bytes)))
        } else {
            // Return Error
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                "Content-Length not found",
            ))
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum FramedMessage {
    Complete(bytes::Bytes),
    KeepaliveRequest,
    KeepaliveResponse,
}

impl std::fmt::Display for FramedMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Complete(msg) => write!(f, "{:?}", msg),
            Self::KeepaliveRequest => write!(f, "Keepalive Request"),
            Self::KeepaliveResponse => write!(f, "Keepalive Response"),
        }
    }
}
