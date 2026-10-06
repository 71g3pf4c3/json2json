//! Recursive-descent parser for RFC 8259 JSON.
//!
//! Strictness notes:
//!
//! * Numbers follow the RFC grammar exactly (`01`, `+1`, `.5`, `1.` are
//!   errors) and are captured verbatim into [`crate::value::Number`].
//! * Duplicate keys are preserved, in source order.
//! * Lone surrogates (`\uD800` without a pair) are rejected.
//! * Nesting depth is bounded by [`MAX_DEPTH`] so hostile inputs fail
//!   gracefully instead of overflowing the stack.

use crate::error::{Error, ErrorKind};
use crate::value::{Number, Object, Value};

/// Maximum nesting depth of arrays/objects.
pub const MAX_DEPTH: usize = 512;

pub struct Parser<'a> {
    input: &'a str,
    pos: usize,
    depth: usize,
}

/// Parse a complete JSON document. Trailing non-whitespace is an error.
pub fn parse(input: &str) -> Result<Value, Error> {
    let mut parser = Parser::new(input);
    parser.skip_ws();
    let value = parser.parse_value()?;
    parser.skip_ws();
    if let Some(c) = parser.peek_char() {
        return Err(parser.err(ErrorKind::TrailingCharacters(c)));
    }
    Ok(value)
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Parser {
            input,
            pos: 0,
            depth: 0,
        }
    }

    // ---- cursor primitives -------------------------------------------------

    fn err(&self, kind: ErrorKind) -> Error {
        Error::at(self.input, self.pos, kind)
    }

    fn peek_byte(&self) -> Option<u8> {
        self.input.as_bytes().get(self.pos).copied()
    }

    fn peek_char(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    fn bump(&mut self) -> Option<u8> {
        let b = self.peek_byte();
        if b.is_some() {
            self.pos += 1;
        }
        b
    }

    fn skip_ws(&mut self) {
        while let Some(b) = self.peek_byte() {
            match b {
                b' ' | b'\t' | b'\n' | b'\r' => self.pos += 1,
                _ => break,
            }
        }
    }

    fn expect(&mut self, b: u8, kind: ErrorKind) -> Result<(), Error> {
        match self.bump() {
            Some(x) if x == b => Ok(()),
            Some(x) => Err(self.err(ErrorKind::UnexpectedChar(x as char))),
            None => Err(self.err(kind)),
        }
    }

    // ---- grammar ------------------------------------------------------------

    fn parse_value(&mut self) -> Result<Value, Error> {
        match self.peek_byte() {
            Some(b'{') => self.parse_object(),
            Some(b'[') => self.parse_array(),
            Some(b'"') => Ok(Value::String(self.parse_string()?)),
            Some(b't') => self.parse_literal("true", Value::Bool(true)),
            Some(b'f') => self.parse_literal("false", Value::Bool(false)),
            Some(b'n') => self.parse_literal("null", Value::Null),
            Some(b'-') | Some(b'0'..=b'9') | Some(b'.') | Some(b'+') => {
                Ok(Value::Number(self.parse_number()?))
            }
            Some(b) => Err(self.err(ErrorKind::UnexpectedChar(b as char))),
            None => Err(self.err(ErrorKind::UnexpectedEof)),
        }
    }

    fn parse_literal(&mut self, word: &str, value: Value) -> Result<Value, Error> {
        if self.input[self.pos..].starts_with(word) {
            self.pos += word.len();
            Ok(value)
        } else {
            // Consume the run of letters to fail on something concrete.
            while matches!(self.peek_byte(), Some(b'a'..=b'z') | Some(b'A'..=b'Z')) {
                self.pos += 1;
            }
            match self.peek_char() {
                Some(c) => Err(self.err(ErrorKind::UnexpectedChar(c))),
                None => Err(self.err(ErrorKind::UnexpectedEof)),
            }
        }
    }

    fn enter(&mut self) -> Result<(), Error> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            Err(self.err(ErrorKind::DepthLimitExceeded))
        } else {
            Ok(())
        }
    }

    fn parse_object(&mut self) -> Result<Value, Error> {
        self.enter()?;
        self.pos += 1; // '{'
        let mut object = Object::new();
        self.skip_ws();
        if self.peek_byte() == Some(b'}') {
            self.pos += 1;
            return Ok(Value::Object(object));
        }
        loop {
            self.skip_ws();
            if self.peek_byte() != Some(b'"') {
                return Err(self.err_or_eof());
            }
            let key = self.parse_string()?;
            self.skip_ws();
            self.expect(b':', ErrorKind::UnexpectedEof)?;
            self.skip_ws();
            let value = self.parse_value()?;
            object.push(key, value);
            self.skip_ws();
            match self.bump() {
                Some(b',') => continue,
                Some(b'}') => break,
                Some(b) => {
                    return Err(Error::at(
                        self.input,
                        self.pos - 1,
                        ErrorKind::UnexpectedChar(b as char),
                    ))
                }
                None => return Err(self.err(ErrorKind::UnexpectedEof)),
            }
        }
        self.depth -= 1;
        Ok(Value::Object(object))
    }

    fn parse_array(&mut self) -> Result<Value, Error> {
        self.enter()?;
        self.pos += 1; // '['
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek_byte() == Some(b']') {
            self.pos += 1;
            return Ok(Value::Array(items));
        }
        loop {
            self.skip_ws();
            items.push(self.parse_value()?);
            self.skip_ws();
            match self.bump() {
                Some(b',') => continue,
                Some(b']') => break,
                Some(b) => {
                    return Err(Error::at(
                        self.input,
                        self.pos - 1,
                        ErrorKind::UnexpectedChar(b as char),
                    ))
                }
                None => return Err(self.err(ErrorKind::UnexpectedEof)),
            }
        }
        self.depth -= 1;
        Ok(Value::Array(items))
    }

    /// Fail on whatever the cursor is looking at — an unexpected character,
    /// or EOF if the input ran out.
    fn err_or_eof(&self) -> Error {
        match self.peek_char() {
            Some(c) => self.err(ErrorKind::UnexpectedChar(c)),
            None => self.err(ErrorKind::UnexpectedEof),
        }
    }

    // ---- strings ---------------------------------------------------------

    /// Parse a `"..."` token, decoding escapes. Assumes the cursor sits on
    /// the opening quote.
    fn parse_string(&mut self) -> Result<String, Error> {
        self.pos += 1; // opening quote
        let mut out = String::new();
        loop {
            let start = self.pos;
            // Fast path: copy the run of plain bytes up to a special one.
            while let Some(b) = self.peek_byte() {
                if b == b'"' || b == b'\\' || b < 0x20 {
                    break;
                }
                self.pos += 1;
            }
            if self.pos > start {
                // Unwraps are safe: the scanned bytes contain no escapes and
                // we only advanced through complete UTF-8 code points
                // (control bytes < 0x20 are excluded above, and multi-byte
                // sequences never contain bytes < 0x20... they can contain
                // bytes in 0x80..=0xFF which are allowed through, but a
                // continuation byte is always >= 0x80, so slicing at self.pos
                // after this loop can land mid-codepoint only if we stopped on
                // a byte < 0x20 or quote/backslash, which are ASCII).
                out.push_str(&self.input[start..self.pos]);
            }
            match self.bump() {
                Some(b'"') => return Ok(out),
                Some(b'\\') => self.parse_escape(&mut out)?,
                Some(b) if b < 0x20 => {
                    return Err(Error::at(
                        self.input,
                        self.pos - 1,
                        ErrorKind::UnexpectedChar(b as char),
                    ))
                }
                Some(_) => unreachable!("fast path consumes plain bytes"),
                None => return Err(self.err(ErrorKind::UnexpectedEof)),
            }
        }
    }

    fn parse_escape(&mut self, out: &mut String) -> Result<(), Error> {
        let escape_start = self.pos - 1; // at the backslash
        match self.bump() {
            Some(b'"') => out.push('"'),
            Some(b'\\') => out.push('\\'),
            Some(b'/') => out.push('/'),
            Some(b'b') => out.push('\u{0008}'),
            Some(b'f') => out.push('\u{000C}'),
            Some(b'n') => out.push('\n'),
            Some(b'r') => out.push('\r'),
            Some(b't') => out.push('\t'),
            Some(b'u') => {
                let hi = self.parse_hex4()?;
                let cp = match hi {
                    0xD800..=0xDBFF => {
                        if self.peek_byte() != Some(b'\\') {
                            return Err(self.err(ErrorKind::LoneSurrogate(hi)));
                        }
                        self.pos += 1; // backslash
                        if self.bump() != Some(b'u') {
                            return Err(Error::at(
                                self.input,
                                escape_start,
                                ErrorKind::InvalidEscape('u'),
                            ));
                        }
                        let lo = self.parse_hex4()?;
                        if !(0xDC00..=0xDFFF).contains(&lo) {
                            return Err(self.err(ErrorKind::LoneSurrogate(lo)));
                        }
                        0x10000 + ((hi as u32 - 0xD800) << 10) + (lo as u32 - 0xDC00)
                    }
                    0xDC00..=0xDFFF => return Err(self.err(ErrorKind::LoneSurrogate(hi))),
                    _ => hi as u32,
                };
                match char::from_u32(cp) {
                    Some(c) => out.push(c),
                    // A valid surrogate pair always maps to a char, so this
                    // is defensive only.
                    None => return Err(self.err(ErrorKind::LoneSurrogate(cp as u16))),
                }
            }
            Some(b) => {
                return Err(Error::at(
                    self.input,
                    self.pos - 1,
                    ErrorKind::InvalidEscape(b as char),
                ))
            }
            None => return Err(self.err(ErrorKind::UnexpectedEof)),
        }
        Ok(())
    }

    fn parse_hex4(&mut self) -> Result<u16, Error> {
        let mut value: u16 = 0;
        for _ in 0..4 {
            let b = self
                .bump()
                .ok_or_else(|| self.err(ErrorKind::InvalidUnicodeEscape))?;
            let digit = match b {
                b'0'..=b'9' => b - b'0',
                b'a'..=b'f' => b - b'a' + 10,
                b'A'..=b'F' => b - b'A' + 10,
                _ => return Err(self.err(ErrorKind::InvalidUnicodeEscape)),
            };
            value = value * 16 + digit as u16;
        }
        Ok(value)
    }

    // ---- numbers ----------------------------------------------------------

    /// Parse a number literal and return its exact source text.
    fn parse_number(&mut self) -> Result<Number, Error> {
        let start = self.pos;

        if self.peek_byte() == Some(b'-') {
            self.pos += 1;
        }
        // Integer part: `0` alone, or [1-9] followed by digits.
        match self.peek_byte() {
            Some(b'0') => {
                self.pos += 1;
                // A leading zero may not be followed by more digits
                // ("01" is not a number per RFC 8259).
                if matches!(self.peek_byte(), Some(b'0'..=b'9')) {
                    return Err(self.number_error(start));
                }
            }
            Some(b'1'..=b'9') => {
                while matches!(self.peek_byte(), Some(b'0'..=b'9')) {
                    self.pos += 1;
                }
            }
            _ => return Err(self.number_error(start)),
        }

        // Fraction: `.` followed by one or more digits.
        if self.peek_byte() == Some(b'.') {
            self.pos += 1;
            if !matches!(self.peek_byte(), Some(b'0'..=b'9')) {
                return Err(self.number_error(start));
            }
            while matches!(self.peek_byte(), Some(b'0'..=b'9')) {
                self.pos += 1;
            }
        }

        // Exponent: [eE] [+-]? digits.
        if matches!(self.peek_byte(), Some(b'e') | Some(b'E')) {
            self.pos += 1;
            if matches!(self.peek_byte(), Some(b'+') | Some(b'-')) {
                self.pos += 1;
            }
            if !matches!(self.peek_byte(), Some(b'0'..=b'9')) {
                return Err(self.number_error(start));
            }
            while matches!(self.peek_byte(), Some(b'0'..=b'9')) {
                self.pos += 1;
            }
        }

        // If the literal is followed by something that looks like more
        // number ("1.2.3", "1e2e3", "0x10"), report it as a bad number
        // rather than confusing trailing garbage. A letter can never
        // legally follow a number in JSON.
        if matches!(
            self.peek_byte(),
            Some(b'0'..=b'9') | Some(b'.') | Some(b'a'..=b'z') | Some(b'A'..=b'Z')
        ) {
            return Err(self.number_error(start));
        }

        // The literal was fully validated above; Number::new trusts it.
        Ok(Number::new(&self.input[start..self.pos]))
    }

    fn number_error(&self, start: usize) -> Error {
        // Back up to the start of the malformed literal for the caret.
        let kind = ErrorKind::InvalidNumber;
        Error::at(self.input, start, kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorKind;

    fn parse_err(input: &str) -> crate::error::Error {
        parse(input).unwrap_err()
    }

    #[test]
    fn scalars() {
        assert_eq!(parse("null").unwrap(), Value::Null);
        assert_eq!(parse("true").unwrap(), Value::Bool(true));
        assert_eq!(parse("false").unwrap(), Value::Bool(false));
    }

    #[test]
    fn numbers_are_verbatim() {
        for src in [
            "0",
            "-0",
            "1e2",
            "1E+2",
            "-0.25e-3",
            "123456789012345678901234567890",
        ] {
            assert_eq!(parse(src).unwrap(), Value::Number(Number::new(src)));
        }
    }

    #[test]
    fn invalid_numbers() {
        for src in ["01", "+1", ".5", "1.", "1e", "-", "1e+", "0x10", "1.2.3"] {
            assert_eq!(
                parse_err(src).kind,
                ErrorKind::InvalidNumber,
                "input: {src}"
            );
        }
    }

    #[test]
    fn string_escapes() {
        let v = parse(r#""a\"b\\c\/d\b\f\n\r\t😀""#).unwrap();
        assert_eq!(v, Value::String("a\"b\\c/d\u{8}\u{c}\n\r\t😀".into()));
    }

    #[test]
    fn lone_surrogates_are_rejected() {
        assert_eq!(
            parse(r#""\uD800""#).unwrap_err().kind,
            ErrorKind::LoneSurrogate(0xD800)
        );
        assert_eq!(
            parse(r#""\uDC00""#).unwrap_err().kind,
            ErrorKind::LoneSurrogate(0xDC00)
        );
        assert_eq!(
            parse(r#""\uD800x""#).unwrap_err().kind,
            ErrorKind::LoneSurrogate(0xD800)
        );
    }

    #[test]
    fn invalid_escapes() {
        assert_eq!(
            parse(r#""\x""#).unwrap_err().kind,
            ErrorKind::InvalidEscape('x')
        );
        assert_eq!(
            parse(r#""\u00ZZ""#).unwrap_err().kind,
            ErrorKind::InvalidUnicodeEscape
        );
        assert_eq!(
            parse(r#""\u12""#).unwrap_err().kind,
            ErrorKind::InvalidUnicodeEscape
        );
    }

    #[test]
    fn raw_control_chars_in_strings_are_rejected() {
        assert_eq!(
            parse("\"a\nb\"").unwrap_err().kind,
            ErrorKind::UnexpectedChar('\n')
        );
    }

    #[test]
    fn objects_preserve_order_and_duplicates() {
        let v = parse(r#"{"b":1,"a":2,"b":3}"#).unwrap();
        let Value::Object(o) = v else {
            panic!("not an object")
        };
        let keys: Vec<&str> = o.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(keys, ["b", "a", "b"]);
        assert_eq!(o.get("b"), Some(&Value::Number(Number::new("1"))));
    }

    #[test]
    fn trailing_garbage_is_rejected() {
        assert_eq!(parse_err("1 2").kind, ErrorKind::TrailingCharacters('2'));
        assert_eq!(parse_err("{} {}").kind, ErrorKind::TrailingCharacters('{'));
    }

    #[test]
    fn unclosed_containers() {
        assert_eq!(parse_err("[1, 2").kind, ErrorKind::UnexpectedEof);
        assert_eq!(parse_err("{\"a\": 1").kind, ErrorKind::UnexpectedEof);
        assert_eq!(parse_err("\"abc").kind, ErrorKind::UnexpectedEof);
    }

    #[test]
    fn whitespace_between_tokens_is_flexible() {
        let v = parse("  [ 1 ,\t2 ,\r\n 3 ]\n").unwrap();
        assert_eq!(
            v,
            Value::Array(vec![
                Value::Number(Number::new("1")),
                Value::Number(Number::new("2")),
                Value::Number(Number::new("3")),
            ])
        );
    }

    #[test]
    fn depth_limit() {
        let deep = "[".repeat(MAX_DEPTH + 1);
        assert_eq!(parse_err(&deep).kind, ErrorKind::DepthLimitExceeded);
        let ok = "[".repeat(MAX_DEPTH) + &"]".repeat(MAX_DEPTH);
        assert!(parse(&ok).is_ok());
    }

    #[test]
    fn empty_containers() {
        assert_eq!(parse("{}").unwrap(), Value::Object(Object::new()));
        assert_eq!(parse("[]").unwrap(), Value::Array(vec![]));
        // Whitespace around empty containers is fine.
        assert_eq!(parse("  {  }  ").unwrap(), Value::Object(Object::new()));
    }

    #[test]
    fn unicode_passthrough() {
        let v = parse(r#""héllo → 🌍""#).unwrap();
        assert_eq!(v, Value::String("héllo → 🌍".into()));
    }
}
