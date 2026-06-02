use super::*;

#[test]
fn test_must_scan_expected_byte_succeeds() {
    let mut scanner = Scanner::new(b"Hello, World!");
    let result = scanner.must_read(b'H');
    assert_eq!(result, Ok(()));
}

#[test]
fn test_must_scan_fails_on_eof() {
    let mut scanner = Scanner::new(b"");
    let err = scanner.must_read(b'h').unwrap_err();
    assert_eq!(err.kind, ScannerErrorKind::Eof);
}

#[test]
fn test_scan_while_digits_should_return_only_digits() {
    let mut scanner = Scanner::new(b"123hello");
    let digits = scanner.scan_while(|b| b.is_ascii_digit());
    assert_eq!(digits, b"123");
}

#[test]
fn scan_while_as_str_should_return_only_alphabetic() {
    let mut scanner = Scanner::new(b"hello123");
    let string = scanner.scan_while_as_str(|b| b.is_ascii_alphabetic());
    assert_eq!(string, Ok("hello"));
}

#[test]
fn scan_while_as_str_fails_on_invalid_utf8() {
    let mut scanner = Scanner::new(&[0xff, 0xff]);
    let err = scanner.scan_while_as_str(|_| true).unwrap_err();
    std::assert_matches!(err.kind, ScannerErrorKind::InvalidUtf8(_));
}

#[test]
fn test_peek_while_should_return_only_alphabetic() {
    let scanner = Scanner::new(b"hello123");
    let letters = scanner.peek_while(|b| b.is_ascii_alphabetic());
    assert_eq!(letters, b"hello");
    assert_eq!(scanner.remaining(), b"hello123");
}

#[test]
fn test_scan_u32_valid_number_returns_value() {
    let mut scanner = Scanner::new(b"12345hello");
    let result = scanner.scan_u32();
    assert_eq!(result, Ok(12345u32));
}

#[test]
fn test_scan_u32_invalid_number_returns_error() {
    let mut scanner = Scanner::new(b"hello");
    let err = scanner.scan_u32().unwrap_err();
    assert_eq!(err.kind, ScannerErrorKind::InvalidNumber);
}

#[test]
fn test_scan_u16_valid_number_returns_value() {
    let mut scanner = Scanner::new(b"65535xyz");
    let result = scanner.scan_u16();
    assert_eq!(result, Ok(65535u16));
}

#[test]
fn test_scan_f32_valid_number_returns_value() {
    let mut scanner = Scanner::new(b"3.1415hello");
    let result = scanner.scan_f32();
    assert_eq!(result, Ok(3.1415f32));
}

#[test]
fn test_scan_f32_invalid_number_returns_error() {
    let mut scanner = Scanner::new(b"hello");
    let err = scanner.scan_f32().unwrap_err();
    assert_eq!(err.kind, ScannerErrorKind::InvalidNumber);
}
