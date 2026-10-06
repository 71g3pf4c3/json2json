//! Serializer: turn a [`Value`] back into JSON text.
//!
//! Two guarantees that make a json2json round-trip faithful:
//!
//! * Numbers are written from their lexical form, untouched.
//! * Object key order (and duplicates) are written back exactly as parsed.
//!
//! Whitespace is normalized: pretty output is 2 spaces per level (or a
//! custom indent), compact output has none. String escapes are
//! re-normalized to the minimal set — the decoded text is identical.

use crate::value::Value;

/// Serialize with 2-space indentation (the default look).
pub fn to_string(value: &Value) -> String {
    to_string_with_indent(value, 2)
}

/// Serialize with a custom number of spaces per nesting level.
pub fn to_string_with_indent(value: &Value, indent: usize) -> String {
    let mut out = String::new();
    write_value(&mut out, value, Style::Pretty { indent }, 0);
    out
}

/// Serialize with no whitespace between tokens.
pub fn to_string_compact(value: &Value) -> String {
    let mut out = String::new();
    write_value(&mut out, value, Style::Compact, 0);
    out
}

/// One-line summary of both output modes.
#[derive(Clone, Copy)]
enum Style {
    Pretty { indent: usize },
    Compact,
}

/// Recursion depth for [`write_value`]. Bounded by [`crate::parser::MAX_DEPTH`]
/// in practice, but serialization of hand-built values needs its own guard.
const MAX_WRITE_DEPTH: usize = crate::parser::MAX_DEPTH;

fn write_value(out: &mut String, value: &Value, style: Style, level: usize) {
    if level > MAX_WRITE_DEPTH {
        // A Value this deep can only come from a hand-built structure
        // (the parser rejects anything deeper). Give up rather than
        // overflow the stack.
        out.push_str("null");
        return;
    }

    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Number(n) => out.push_str(n.as_str()),
        Value::String(s) => write_string(out, s),
        Value::Array(items) => {
            if items.is_empty() {
                out.push_str("[]");
                return;
            }
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                newline_indent(out, style, level + 1);
                write_value(out, item, style, level + 1);
            }
            newline_indent(out, style, level);
            out.push(']');
        }
        Value::Object(object) => {
            if object.is_empty() {
                out.push_str("{}");
                return;
            }
            out.push('{');
            for (i, (key, item)) in object.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                newline_indent(out, style, level + 1);
                write_string(out, key);
                match style {
                    Style::Compact => out.push(':'),
                    Style::Pretty { .. } => out.push_str(": "),
                }
                write_value(out, item, style, level + 1);
            }
            newline_indent(out, style, level);
            out.push('}');
        }
    }
}

/// In pretty mode emit `\n` + `level * indent` spaces; in compact, nothing.
fn newline_indent(out: &mut String, style: Style, level: usize) {
    if let Style::Pretty { indent } = style {
        out.push('\n');
        for _ in 0..level * indent {
            out.push(' ');
        }
    }
}

/// Write a JSON string literal with minimal, correct escaping.
///
/// Everything that must be escaped per RFC 8259 (`"`, `\`, U+0000..U+001F)
/// is escaped; everything else passes through as raw UTF-8.
fn write_string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{0008}' => out.push_str("\\b"),
            '\u{000C}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                use std::fmt::Write;
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;
    use crate::value::Object;

    #[test]
    fn pretty_output() {
        let v = parse(r#"{"a":[1,2],"b":{"c":null}}"#).unwrap();
        assert_eq!(
            to_string(&v),
            "{\n  \"a\": [\n    1,\n    2\n  ],\n  \"b\": {\n    \"c\": null\n  }\n}"
        );
    }

    #[test]
    fn compact_output() {
        let v = parse(r#"{"a": [1, 2], "b": {"c": null}}"#).unwrap();
        assert_eq!(to_string_compact(&v), r#"{"a":[1,2],"b":{"c":null}}"#);
    }

    #[test]
    fn empty_containers_stay_on_one_line() {
        let v = parse(r#"{"a":[], "b":{}}"#).unwrap();
        assert_eq!(to_string(&v), "{\n  \"a\": [],\n  \"b\": {}\n}");
    }

    #[test]
    fn numbers_round_trip_verbatim() {
        for src in ["0", "-0", "1e2", "1E+2", "-0.25e-3", "9007199254740993"] {
            let v = parse(src).unwrap();
            assert_eq!(to_string(&v), src);
            assert_eq!(to_string_compact(&v), src);
        }
    }

    #[test]
    fn string_escaping() {
        let v = Value::String("a\"b\\c\u{8}\u{c}\n\r\t\u{1}".into());
        assert_eq!(
            to_string_compact(&v),
            "\"a\\\"b\\\\c\\b\\f\\n\\r\\t\\u0001\""
        );
    }

    #[test]
    fn non_ascii_is_not_escaped() {
        let v = Value::String("héllo → 🌍".into());
        assert_eq!(to_string_compact(&v), "\"héllo → 🌍\"");
    }

    #[test]
    fn key_order_and_duplicates_survive() {
        let v = parse(r#"{"b":1,"a":2,"b":3}"#).unwrap();
        assert_eq!(to_string_compact(&v), r#"{"b":1,"a":2,"b":3}"#);
    }

    #[test]
    fn deep_hand_built_value_does_not_recurse_forever() {
        let mut v = Value::Null;
        for _ in 0..(crate::parser::MAX_DEPTH + 8) {
            v = Value::Array(vec![v]);
        }
        // Must terminate; contents beyond the limit degrade to null.
        assert!(to_string_compact(&v).starts_with("[[[[["));
    }

    #[test]
    fn custom_indent() {
        let mut obj = Object::new();
        obj.push("k".into(), Value::Bool(true));
        assert_eq!(
            to_string_with_indent(&Value::Object(obj), 4),
            "{\n    \"k\": true\n}"
        );
    }
}
