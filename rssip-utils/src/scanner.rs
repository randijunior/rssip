//! Text scanning with the `Scanner` type.

use core::{fmt, str};

type Result<T> = std::result::Result<T, ScannerError>;

#[must_use]
pub fn is_space(b: u8) -> bool {
    matches!(b, b' ' | b'\t')
}

#[must_use]
pub fn is_newline(b: u8) -> bool {
    matches!(b, b'\r' | b'\n')
}

#[must_use]
pub fn is_alphabetic(b: u8) -> bool {
    b.is_ascii_alphabetic()
}

#[must_use]
pub fn is_digit(b: u8) -> bool {
    b.is_ascii_digit()
}

/// A text scanner for sequentially reading bytes from an input slice.
///
/// The `Scanner` provides methods to iterate over the input while
/// tracking the current position in terms of line and column numbers.
pub struct Scanner<'buf> {
    /// The input byte slice being scanned.
    buffer: &'buf [u8],
    /// The current position in the input.
    position: Position,
    /// The current byte index within the buffer.
    index: usize,
    /// The total length of the input buffer.
    len: usize,
}

/// Represents a position within the scanned input.
///
/// Both `line` and `column` are 1-based:
/// - `line = 1` means the first line of the input.
/// - `column = 1` means the first byte of the line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    /// Current line number (starting from 1).
    pub line: usize,
    /// Current column number (starting from 0).
    pub column: usize,
}

#[derive(Debug, PartialEq, Clone, Copy, Eq)]
pub struct ScannerError {
    pub kind: ScannerErrorKind,
    pub position: Position,
}

/// Errors that can occur while reading the buffer.
#[derive(Debug, PartialEq, Clone, Copy, Eq)]
pub enum ScannerErrorKind {
    /// End of the buffer reached unexpectedly.
    Eof,

    NonNewLine,

    /// The byte read did not match the expected value.
    UnexpectedByte {
        /// The byte that was expected.
        expected: u8,
        /// The byte that was actually found.
        found: u8,
    },

    /// A number could not be parsed from the input.
    InvalidNumber,

    /// The bytes are not valid UTF-8.
    InvalidUtf8(str::Utf8Error),
}

impl<'buf> Scanner<'buf> {
    /// Create a `Scanner` from a byte slice.
    pub const fn new(buffer: &'buf [u8]) -> Self {
        Scanner {
            buffer,
            len: buffer.len(),
            position: Position::new(),
            index: 0usize,
        }
    }

    pub fn next_byte(&mut self) -> Result<u8> {
        self.scan()
            .ok_or_else(|| self.scan_error(ScannerErrorKind::Eof))
    }

    pub fn is_eof(&self) -> bool {
        self.index == self.len
    }

    /// Returns a slice of the remaining bytes in the scanner.
    #[inline]
    pub fn remaining(&self) -> &[u8] {
        self.as_ref()
    }

    /// Advances the scanner by `n` bytes.
    ///
    /// Stops early if the end of the buffer is reached.
    pub fn advance_n(&mut self, n: usize) {
        for _ in 0..n {
            if self.scan().is_none() {
                break;
            }
        }
    }

    /// Reads the next byte and advance the scanner position.
    ///
    /// If the scanner has reached the end of the buffer, it returns `None`.
    pub fn scan(&mut self) -> Option<u8> {
        self.peek().copied().inspect(|&c| {
            self.bump(c);
        })
    }

    /// Returns a reference to the next byte without advancing the scanner
    /// position.
    ///
    /// If the scanner has reached the end of the buffer, it returns `None`.
    #[must_use]
    pub fn peek(&self) -> Option<&u8> {
        self.buffer.get(self.index)
    }

    /// Get a reference to the current scanner [`Position`].
    #[inline]
    pub fn position(&self) -> Position {
        self.position
    }

    /// Returns `true` if the upcoming bytes match the given `prefix`.
    #[inline]
    pub fn matches_prefix(&self, prefix: &[u8]) -> bool {
        self.remaining().starts_with(prefix)
    }

    /// Get `n` bytes without advance.
    ///
    /// Returns `None` if there are fewer than `n` bytes remaining.
    #[must_use]
    pub fn peek_n(&self, n: usize) -> Option<&[u8]> {
        self.remaining().get(..n)
    }

    /// Reads a `u32` number until a non-digit is found.
    ///
    /// Returns an error if no valid digits were found or if the number is out
    /// of range.
    pub fn scan_u32(&mut self) -> Result<u32> {
        self.scan_number_str(false)
            .parse()
            .map_err(|_| self.scan_error(ScannerErrorKind::InvalidNumber))
    }

    /// Read a `u16` number until an invalid digit is found.
    ///
    /// Returns an error if no valid digits were found or if the number is out
    /// of range.
    pub fn scan_u16(&mut self) -> Result<u16> {
        self.scan_number_str(false)
            .parse()
            .map_err(|_| self.scan_error(ScannerErrorKind::InvalidNumber))
    }

    /// Read a `u64` number until an invalid digit is found.
    ///
    /// Returns an error if no valid digits were found or if the number is out
    /// of range.
    pub fn scan_u64(&mut self) -> Result<u64> {
        self.scan_number_str(false)
            .parse()
            .map_err(|_| self.scan_error(ScannerErrorKind::InvalidNumber))
    }

    /// Read a `i64` number until an invalid digit is found.
    ///
    /// Returns an error if no valid digits were found or if the number is out
    /// of range.
    pub fn scan_i64(&mut self) -> Result<i64> {
        self.scan_number_str(false)
            .parse()
            .map_err(|_| self.scan_error(ScannerErrorKind::InvalidNumber))
    }

    /// Read a `f32` number until an invalid digit is found.
    ///
    /// Returns an error if no valid digits were found or if the number is out
    /// of range.
    pub fn scan_f32(&mut self) -> Result<f32> {
        self.scan_number_str(true)
            .parse()
            .map_err(|_| self.scan_error(ScannerErrorKind::InvalidNumber))
    }

    /// Call the `predicate` closure for each element in the buffer and read
    /// the scanner while the closure returns `true`.
    #[must_use]
    pub fn scan_while(&mut self, predicate: impl Fn(u8) -> bool) -> &'buf [u8] {
        let start = self.index;
        while let Some(&c) = self.peek() {
            if predicate(c) {
                self.bump(c);
            } else {
                break;
            }
        }
        let end = self.index;

        unsafe { self.buffer.get_unchecked(start..end) }
    }

    /// peek bytes in the buffer while the `predicate` returns true.
    ///
    /// Does not read the scanner position.
    pub fn peek_while(&'buf self, predicate: impl Fn(u8) -> bool) -> &'buf [u8] {
        let buffer = self.remaining();

        let n = buffer
            .iter()
            .position(|&b| !predicate(b))
            .unwrap_or(buffer.len());

        // SAFETY: `n` is guaranteed to be ≤ `buffer.len()`, so the range is always
        // in-bounds.
        unsafe { buffer.get_unchecked(..n) }
    }

    /// Peek next byte if `condition` returns `true`.
    pub fn peek_if(&self, condition: impl Fn(u8) -> bool) -> Option<u8> {
        self.peek().filter(|&&byte| condition(byte)).copied()
    }

    /// Read next bytes if equals to `expected`
    pub fn must_scan_slice(&mut self, expected: &[u8]) -> Result<()> {
        let remaining = &self.buffer[self.index..];
        for (&found, &expected) in remaining.iter().zip(expected) {
            if found != expected {
                return Err(self.scan_error(ScannerErrorKind::UnexpectedByte { expected, found }));
            }
            self.bump(found);
        }
        Ok(())
    }

    /// Read next byte if equals to `expected`.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the next byte is not equal to `expected` or
    /// the end of the slice has been reached.
    pub fn must_read(&mut self, expected: u8) -> Result<()> {
        match self.peek().copied() {
            Some(found) if expected == found => {
                self.bump(found);
                Ok(())
            }
            Some(found) => {
                Err(self.scan_error(ScannerErrorKind::UnexpectedByte { expected, found }))
            }
            None => Err(self.scan_error(ScannerErrorKind::Eof)),
        }
    }

    /// Reads bytes until the next byte equals `byte`.
    ///
    /// The matching byte is not consumed.
    #[inline]
    pub fn scan_until(&mut self, byte: u8) -> &'buf [u8] {
        self.scan_while(|b| b != byte)
    }

    #[inline]
    pub fn scan_until_as_str(&mut self, byte: u8) -> Result<&'buf str> {
        self.scan_while_as_str(|b| b != byte)
    }

    pub fn scan_until_any_as_str(&mut self, slice: &[u8]) -> Result<&'buf str> {
        self.scan_while_as_str(|b| !slice.contains(&b))
    }

    pub fn scan_newline(&mut self) -> Result<()> {
        let carriage_return = self.scan_if_eq(b'\r');
        let line_feed = self.scan_if_eq(b'\n');
        if carriage_return.is_none() && line_feed.is_none() {
            return Err(self.scan_error(ScannerErrorKind::NonNewLine));
        }
        Ok(())
    }

    /// Scan until a new line (`\r` or `\n`) is found.
    pub fn scan_line(&mut self) -> Result<&'buf str> {
        self.scan_while_as_str(|b| !is_newline(b))
    }

    pub(crate) fn scan_error(&self, kind: ScannerErrorKind) -> ScannerError {
        ScannerError {
            kind,
            position: self.position,
        }
    }

    /// Reads bytes while `predicate` returns true and converts them to a string
    /// slice.
    ///
    /// # Errors
    ///
    /// Returns `ScannerError::InvalidUtf8` if the resulting bytes are not
    /// valid UTF-8.
    pub fn scan_while_as_str(&mut self, predicate: impl Fn(u8) -> bool) -> Result<&'buf str> {
        let bytes = self.scan_while(predicate);

        std::str::from_utf8(bytes)
            .map_err(|err| self.scan_error(ScannerErrorKind::InvalidUtf8(err)))
    }

    /// Same as [`Scanner::scan_while`] but returns the bytes as a string slice
    /// without checking UTF-8.
    ///
    /// # Safety
    ///
    /// The caller must ensure that `predicate` only returns `true` for bytes that form valid
    /// UTF-8.
    #[inline]
    pub unsafe fn scan_while_as_str_unchecked(
        &mut self,
        predicate: impl Fn(u8) -> bool,
    ) -> &'buf str {
        let bytes = self.scan_while(predicate);

        // SAFETY: The caller guarantees that `predicate` only matches bytes forming valid
        // UTF-8.
        unsafe { std::str::from_utf8_unchecked(bytes) }
    }

    /// Call the `predicate` closure for next byte and read it if
    /// the closure returns `true`.
    ///
    /// # Returns
    ///
    /// The byte readed.
    #[must_use]
    pub fn scan_if(&mut self, predicate: impl FnOnce(u8) -> bool) -> Option<u8> {
        match self.peek().copied() {
            Some(byte) if predicate(byte) => {
                self.bump(byte);

                Some(byte)
            }
            _ => None,
        }
    }

    /// Consume and return the next byte if it is equal to `expected`.
    pub fn scan_if_eq(&mut self, expected: u8) -> Option<u8> {
        self.scan_if(|b| b == expected)
    }

    fn scan_number_str(&mut self, decimal_point: bool) -> &'buf str {
        let bytes = self.scan_while(|b| b.is_ascii_digit() || (decimal_point && b == b'.'));
        unsafe { std::str::from_utf8_unchecked(bytes) }
    }

    #[inline]
    fn bump(&mut self, byte: u8) {
        self.index += 1;

        if byte == b'\n' {
            self.position.column = 1;
            self.position.line += 1;
        } else {
            self.position.column += 1;
        }
    }
}

impl AsRef<[u8]> for Scanner<'_> {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        unsafe { self.buffer.get_unchecked(self.index..self.len) }
    }
}

impl fmt::Display for Scanner<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let to_print = String::from_utf8_lossy(self.remaining());
        write!(f, "{to_print}")?;
        Ok(())
    }
}

impl Position {
    /// Create a new `Position` starting at line 1, column 0.
    pub const fn new() -> Self {
        Self { line: 1, column: 0 }
    }
}

impl Default for Position {
    fn default() -> Self {
        Self { line: 1, column: 0 }
    }
}

#[cfg(test)]
mod tests {
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
        assert_matches::assert_matches!(err.kind, ScannerErrorKind::InvalidUtf8(_));
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
}
