//! Error type with source position.

use std::fmt;

/// What went wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorKind {
    /// Input ended in the middle of a value.
    UnexpectedEof,
    /// A character that cannot appear where it did.
    UnexpectedChar(char),
    /// Invalid number literal, e.g. `01`, `1.`, `.5`, `1e`.
    InvalidNumber,
    /// A backslash escape that is not one of `"\/bfnrtu`.
    InvalidEscape(char),
    /// `\u` followed by something that is not 4 hex digits.
    InvalidUnicodeEscape,
    /// A high or low surrogate not part of a valid pair.
    LoneSurrogate(u16),
    /// Non-whitespace garbage after the top-level value.
    TrailingCharacters(char),
    /// Nesting deeper than the parser is willing to follow.
    DepthLimitExceeded,
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorKind::UnexpectedEof => write!(f, "unexpected end of input"),
            ErrorKind::UnexpectedChar(c) => write!(f, "unexpected character {c:?}"),
            ErrorKind::InvalidNumber => write!(f, "invalid number literal"),
            ErrorKind::InvalidEscape(c) => write!(f, "invalid escape sequence \\{c}"),
            ErrorKind::InvalidUnicodeEscape => {
                write!(f, "invalid \\u escape (expected 4 hex digits)")
            }
            ErrorKind::LoneSurrogate(cp) => {
                write!(f, "lone surrogate \\u{cp:04X} without a pair")
            }
            ErrorKind::TrailingCharacters(c) => write!(f, "trailing characters after value: {c:?}"),
            ErrorKind::DepthLimitExceeded => write!(
                f,
                "nesting depth limit exceeded ({} levels)",
                crate::parser::MAX_DEPTH
            ),
        }
    }
}

/// A parse error, carrying the byte offset plus 1-based line/column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub kind: ErrorKind,
    /// Byte offset into the input where the error was detected.
    pub offset: usize,
    /// 1-based line number.
    pub line: usize,
    /// 1-based column, in characters.
    pub column: usize,
}

impl Error {
    /// Compute line/column from the original input and `offset`.
    pub fn at(input: &str, offset: usize, kind: ErrorKind) -> Self {
        let clamped = offset.min(input.len());
        let (mut line, mut column) = (1, 1);
        for ch in input[..clamped].chars() {
            if ch == '\n' {
                line += 1;
                column = 1;
            } else {
                column += 1;
            }
        }
        Error {
            kind,
            offset: clamped,
            line,
            column,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} at line {}, column {}",
            self.kind, self.line, self.column
        )
    }
}

impl std::error::Error for Error {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;

    #[test]
    fn position_is_one_based_and_accurate() {
        let input = "{\n  \"a\": 1,\n  x\n}";
        let err = parse(input).unwrap_err();
        assert_eq!(err.line, 3);
        assert_eq!(err.column, 3);
    }

    #[test]
    fn error_at_eof_has_sensible_position() {
        let input = "[1, 2";
        let err = parse(input).unwrap_err();
        assert_eq!(err.kind, ErrorKind::UnexpectedEof);
        assert_eq!(err.line, 1);
    }
}
