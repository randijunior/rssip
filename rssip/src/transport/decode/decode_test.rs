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
