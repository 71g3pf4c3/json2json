//! The JSON data model.
//!
//! Design goals, in priority order:
//!
//! 1. **Order-preserving.** Object keys are stored in a `Vec`, exactly as
//!    they appear in the source. No `HashMap`, no sorting, no dedup —
//!    duplicate keys are legal JSON and are kept as-is.
//! 2. **Lossless numbers.** A number is its original lexical form
//!    (`1e2` stays `1e2`, never `100.0`), so no `f64` round-tripping damage.
//! 3. **Plain data.** `Value` is a dumb tree you can build, walk and
//!    serialize; it carries no borrow or lifetime baggage.

use std::fmt;

/// A JSON number, stored as its original lexical representation.
///
/// Validity invariant: the contained string matches the RFC 8259 `number`
/// production. It is only produced by [`crate::parse`], which enforces this.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Number(String);

impl Number {
    /// The original lexical form, e.g. `"-0.25e+3"`.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Build a number from its lexical form.
    ///
    /// **Caller promise:** `src` must be a valid RFC 8259 number. The
    /// serializer writes it verbatim; [`crate::parse`] is the only place
    /// that validates. For arbitrary numeric data, parse a literal instead.
    pub fn new(src: impl Into<String>) -> Self {
        Number(src.into())
    }

    /// Best-effort conversion to `f64`.
    ///
    /// This is a convenience for consumers that want to *do math*; the
    /// serializer never uses it, so precision loss is impossible during a
    /// json2json round-trip.
    pub fn as_f64(&self) -> f64 {
        self.0.parse().unwrap_or(f64::NAN)
    }
}

impl fmt::Display for Number {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A JSON object: an ordered list of key-value pairs.
///
/// Like [`Value`], this deliberately preserves insertion order and
/// duplicates. `get` returns the *first* match, mirroring typical
/// first-wins object semantics.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Object {
    entries: Vec<(String, Value)>,
}

impl Object {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Append a pair. Does not check for duplicate keys — by design.
    pub fn push(&mut self, key: String, value: Value) {
        self.entries.push((key, value));
    }

    /// Value of the first entry with this key, if any.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn iter(&self) -> std::slice::Iter<'_, (String, Value)> {
        self.entries.iter()
    }
}

impl IntoIterator for Object {
    type Item = (String, Value);
    type IntoIter = std::vec::IntoIter<(String, Value)>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.into_iter()
    }
}

impl<'a> IntoIterator for &'a Object {
    type Item = &'a (String, Value);
    type IntoIter = std::slice::Iter<'a, (String, Value)>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter()
    }
}

impl FromIterator<(String, Value)> for Object {
    fn from_iter<I: IntoIterator<Item = (String, Value)>>(iter: I) -> Self {
        Object {
            entries: iter.into_iter().collect(),
        }
    }
}

/// A JSON value.
///
/// Semantically equivalent to the input document after [`crate::parse`];
/// serializing it with [`crate::to_string`] produces the same data with
/// normalized whitespace only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Null,
    Bool(bool),
    Number(Number),
    String(String),
    Array(Vec<Value>),
    Object(Object),
}

impl Value {
    /// Type name as it appears in error messages: `null`, `object`, ...
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Null => "null",
            Value::Bool(_) => "boolean",
            Value::Number(_) => "number",
            Value::String(_) => "string",
            Value::Array(_) => "array",
            Value::Object(_) => "object",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_preserves_order_and_duplicates() {
        let mut obj = Object::new();
        obj.push("b".into(), Value::Null);
        obj.push("a".into(), Value::Bool(true));
        obj.push("b".into(), Value::Bool(false)); // duplicate, kept

        let keys: Vec<&str> = obj.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(keys, ["b", "a", "b"]);
        assert_eq!(obj.get("b"), Some(&Value::Null)); // first wins
    }

    #[test]
    fn number_display_is_lexical() {
        let n = Number::new("1e2");
        assert_eq!(n.to_string(), "1e2");
        assert_eq!(n.as_f64(), 100.0);
    }
}
