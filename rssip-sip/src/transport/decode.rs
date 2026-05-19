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

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_decode_keepalive_request() {
        let buffer = &mut BytesMut::from(KEEPALIVE_REQUEST);
        let result = StreamingDecoder::new().decode(buffer).unwrap();

        assert!(buffer.is_empty());
        assert_eq!(result, Some(FramedMessage::KeepaliveRequest));
    }

    #[test]
    fn test_decode_keepalive_response() {
        let buffer = &mut BytesMut::from(KEEPALIVE_RESPONSE);
        let result = StreamingDecoder::new().decode(buffer).unwrap();

        assert!(buffer.is_empty());
        assert_eq!(result, Some(FramedMessage::KeepaliveResponse));
    }

    #[test]
    fn test_decode_complete_message_for_single_frame() {
        let complete_msg: &[u8] = b"INVITE sip:bob@example.com SIP/2.0\r\n\
        Via: SIP/2.0/UDP pc33.atlanta.com;branch=z9hG4bK776asdhds\r\n\
        Max-Forwards: 70\r\n\
        To: Bob <sip:bob@example.com>\r\n\
        From: Alice <sip:alice@example.com>;tag=1928301774\r\n\
        Call-ID: a84b4c76e66710\r\n\
        CSeq: 314159 INVITE\r\n\
        Contact: <sip:alice@example.com>\r\n\
        Content-Length: 0\r\n\
        \r\n";
        let mut buffer = BytesMut::from(complete_msg);
        let result = StreamingDecoder::new().decode(&mut buffer).unwrap();

        assert!(buffer.is_empty());
        assert_eq!(result, Some(FramedMessage::Complete(complete_msg.into())));
    }

    #[test]
    fn test_decode_complete_message_for_multiple_frames() {
        let complete_msg: &[u8] = b"INVITE sip:bob@example.com SIP/2.0\r\n\
        Via: SIP/2.0/UDP pc33.atlanta.com;branch=z9hG4bK776asdhds\r\n\
        Max-Forwards: 70\r\n\
        To: Bob <sip:bob@example.com>\r\n\
        From: Alice <sip:alice@example.com>;tag=1928301774\r\n\
        Call-ID: a84b4c76e66710\r\n\
        CSeq: 314159 INVITE\r\n\
        Contact: <sip:alice@example.com>\r\n\
        Content-Length: 0\r\n\
        \r\n";
        let mut decoder = StreamingDecoder::new();
        let mut buffer = BytesMut::new();

        let part1 = &complete_msg[..50];
        let part2 = &complete_msg[50..100];
        let part3 = &complete_msg[100..];

        buffer.extend_from_slice(part1);
        let result = decoder.decode(&mut buffer).unwrap();
        assert!(result.is_none(), "message should not be complete yet");

        buffer.extend_from_slice(part2);
        let result = decoder.decode(&mut buffer).unwrap();
        assert!(result.is_none(), "message should not be complete yet");

        buffer.extend_from_slice(part3);
        let result = decoder.decode(&mut buffer).unwrap();
        assert_eq!(result, Some(FramedMessage::Complete(complete_msg.into())));

        assert!(buffer.is_empty());
    }

    #[test]
    fn test_decode_returns_error_for_invalid_utf8_content_length() {
        let invalid_msg: &[u8] = b"INVITE sip:bob@example.com SIP/2.0\r\n\
        Content-Length: \xFF\xFE\r\n\
        \r\n";
        let mut buffer = BytesMut::from(invalid_msg);
        let result = StreamingDecoder::new().decode(&mut buffer);
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        assert_eq!(err.to_string(), "Invalid UTF-8 in Content-Length header");
    }

    #[test]
    fn test_decode_returns_error_when_content_length_missing() {
        let msg: &[u8] = b"INVITE sip:bob@example.com SIP/2.0\r\n\
        \r\n";

        let mut buffer = BytesMut::from(msg);
        let result = StreamingDecoder::new().decode(&mut buffer);
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
        assert_eq!(err.to_string(), "Content-Length not found");
    }
}
