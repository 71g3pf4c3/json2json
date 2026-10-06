//! json2json — a lossless JSON-to-JSON transformer.
//!
//! Reads JSON, parses it into an order-preserving tree, and writes it back
//! out. The data is never touched: key order, duplicate keys, number
//! lexical forms and string contents all survive the round-trip. Only the
//! whitespace changes.
//!
//! ```no_run
//! let pretty = json2json::identity(r#"{"a":[1,2]}"#).unwrap();
//! ```
//!
//! Layout:
//!
//! * [`value`] — the data model (`Value`, `Number`, `Object`)
//! * [`parser`] — strict RFC 8259 recursive-descent parser
//! * [`serializer`] — pretty/compact writer
//! * [`error`] — positional parse errors

pub mod error;
pub mod parser;
pub mod serializer;
pub mod value;

pub use error::{Error, ErrorKind};
pub use parser::{parse, MAX_DEPTH};
pub use serializer::{to_string, to_string_compact, to_string_with_indent};
pub use value::{Number, Object, Value};

/// The whole point: parse `input` and re-serialize it pretty-printed.
///
/// Semantically `input == identity(input)` (after re-parsing); whitespace
/// and string escapes are normalized, nothing else moves.
pub fn identity(input: &str) -> Result<String, Error> {
    Ok(to_string(&parse(input)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The core property: re-parsing the output yields an identical tree.
    #[test]
    fn round_trip_is_semantically_identical() {
        let inputs = [
            "null",
            "true",
            "123",
            "-0.5e-10",
            "\"hello\"",
            "[]",
            "{}",
            "[1, [2, [3, [4]]]]",
            "{\"a\": 1, \"b\": [true, null, \"x\"], \"c\": {\"d\": {\"e\": []}}}",
            // duplicate keys and non-trivial numbers
            "{\"x\":1,\"x\":2}",
            "[1e2, 1E+2, 0.0, -0, 9007199254740993]",
            // unicode
            "\"héllo → 🌍\"",
            "[\"\\u0041\\u00e9\\ud83d\\ude00\"]",
        ];
        for input in inputs {
            let once = identity(input).unwrap();
            let twice = identity(&once).unwrap();
            assert_eq!(
                parse(&once).unwrap(),
                parse(input).unwrap(),
                "input: {input}"
            );
            // And the transformation is idempotent from the second pass on.
            assert_eq!(once, twice, "input: {input}");
        }
    }
}
