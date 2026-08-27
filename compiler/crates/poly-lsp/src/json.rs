//! Minimal JSON value type, parser, and serializer.
//!
//! The language server speaks JSON-RPC 2.0 over stdio.  Keeping the JSON
//! handling in-tree (rather than pulling in serde_json) matches the compiler's
//! dependency-light ethos and is plenty for the LSP message shapes.

use std::fmt::Write as _;

/// A JSON value.  Object keys are kept insertion-ordered via `BTreeMap` is
/// not needed; `Vec` preserves order and the server only does lookups.
#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    /// JSON-RPC uses null for absent optional values and empty results.
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Json>),
    /// A vector preserves insertion order, which keeps server responses stable
    /// without introducing a map dependency or sorting protocol fields.
    Object(Vec<(String, Json)>),
}

impl Json {
    /// Convenience constructor for a string field.
    pub fn str(value: impl Into<String>) -> Self {
        Json::String(value.into())
    }

    /// Convenience constructor for a number field.
    pub fn num(value: impl Into<f64>) -> Self {
        Json::Number(value.into())
    }

    /// Build an object from `(key, value)` pairs.
    pub fn obj(pairs: Vec<(impl Into<String>, Json)>) -> Self {
        Json::Object(
            pairs
                .into_iter()
                .map(|(key, value)| (key.into(), value))
                .collect(),
        )
    }

    /// Build an empty object (avoids ambiguous `vec![]` type inference).
    pub fn empty_object() -> Self {
        Json::Object(Vec::new())
    }

    /// Look up a field on an object.
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Object(pairs) => pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// Look up a string field on an object.
    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(|value| match value {
            Json::String(text) => Some(text.as_str()),
            _ => None,
        })
    }

    /// Look up a number field on an object.
    pub fn get_f64(&self, key: &str) -> Option<f64> {
        self.get(key).and_then(|value| match value {
            Json::Number(number) => Some(*number),
            _ => None,
        })
    }

    /// Serialize to a compact JSON string.
    pub fn serialize(&self) -> String {
        let mut out = String::new();
        self.write(&mut out);
        out
    }

    fn write(&self, out: &mut String) {
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(true) => out.push_str("true"),
            Json::Bool(false) => out.push_str("false"),
            Json::Number(number) => {
                if number.fract() == 0.0 && number.is_finite() && number.abs() < 1e15 {
                    let _ = write!(out, "{}", *number as i64);
                } else {
                    let _ = write!(out, "{}", number);
                }
            }
            Json::String(text) => write_json_string(out, text),
            Json::Array(items) => {
                out.push('[');
                for (index, item) in items.iter().enumerate() {
                    if index > 0 {
                        out.push(',');
                    }
                    item.write(out);
                }
                out.push(']');
            }
            Json::Object(pairs) => {
                out.push('{');
                for (index, (key, value)) in pairs.iter().enumerate() {
                    if index > 0 {
                        out.push(',');
                    }
                    write_json_string(out, key);
                    out.push(':');
                    value.write(out);
                }
                out.push('}');
            }
        }
    }
}

/// Escape JSON control characters and quotes without depending on a general
/// serialization crate; this is sufficient for the server's fixed LSP shapes.
fn write_json_string(out: &mut String, text: &str) {
    out.push('"');
    for character in text.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0C}' => out.push_str("\\f"),
            character if (character as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", character as u32);
            }
            character => out.push(character),
        }
    }
    out.push('"');
}

/// A JSON parsing error.
#[derive(Debug, Clone, PartialEq)]
pub struct JsonError {
    /// Human-readable parse detail for protocol logs and test failures.
    pub message: String,
    /// Byte offset, rather than a character index, matches the parser's input
    /// representation and lets callers highlight the exact invalid frame.
    pub position: usize,
}

impl std::fmt::Display for JsonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} at byte {}", self.message, self.position)
    }
}

impl std::error::Error for JsonError {}

/// Parse a JSON document from a string.
pub fn parse(text: &str) -> Result<Json, JsonError> {
    // Parse exactly one value and reject trailing bytes so malformed LSP frames
    // cannot be accepted as a valid request with an ignored suffix.
    let mut parser = Parser {
        bytes: text.as_bytes(),
        position: 0,
    };
    let value = parser.parse_value()?;
    parser.skip_whitespace();
    if parser.position != parser.bytes.len() {
        return Err(JsonError {
            message: "trailing characters after JSON value".to_string(),
            position: parser.position,
        });
    }
    Ok(value)
}

struct Parser<'a> {
    // JSON-RPC arrives as UTF-8 bytes. Tracking a byte cursor keeps framing
    // and error offsets exact; string parsing reconstructs Unicode explicitly.
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Parser<'a> {
    fn skip_whitespace(&mut self) {
        while self.position < self.bytes.len()
            && matches!(self.bytes[self.position], b' ' | b'\t' | b'\n' | b'\r')
        {
            self.position += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.position).copied()
    }

    fn error(&self, message: impl Into<String>) -> JsonError {
        JsonError {
            message: message.into(),
            position: self.position,
        }
    }

    fn parse_value(&mut self) -> Result<Json, JsonError> {
        self.skip_whitespace();
        match self.peek() {
            Some(b'n') => self.parse_literal("null", Json::Null),
            Some(b't') => self.parse_literal("true", Json::Bool(true)),
            Some(b'f') => self.parse_literal("false", Json::Bool(false)),
            Some(b'"') => Ok(Json::String(self.parse_string()?)),
            Some(b'[') => self.parse_array(),
            Some(b'{') => self.parse_object(),
            Some(b'-' | b'0'..=b'9') => self.parse_number(),
            Some(other) => Err(self.error(format!("unexpected character '{}'", other as char))),
            None => Err(self.error("unexpected end of input")),
        }
    }

    fn parse_literal(&mut self, literal: &str, value: Json) -> Result<Json, JsonError> {
        if self.bytes[self.position..].starts_with(literal.as_bytes()) {
            self.position += literal.len();
            Ok(value)
        } else {
            Err(self.error(format!("expected '{literal}'")))
        }
    }

    fn parse_string(&mut self) -> Result<String, JsonError> {
        // Opening quote is at the current position.
        self.position += 1;
        let mut result = String::new();
        loop {
            let byte = match self.peek() {
                Some(byte) => byte,
                None => return Err(self.error("unterminated string")),
            };
            self.position += 1;
            match byte {
                b'"' => return Ok(result),
                b'\\' => {
                    let escaped = match self.peek() {
                        Some(byte) => byte,
                        None => return Err(self.error("unterminated escape sequence")),
                    };
                    self.position += 1;
                    match escaped {
                        b'"' => result.push('"'),
                        b'\\' => result.push('\\'),
                        b'/' => result.push('/'),
                        b'b' => result.push('\u{08}'),
                        b'f' => result.push('\u{0C}'),
                        b'n' => result.push('\n'),
                        b'r' => result.push('\r'),
                        b't' => result.push('\t'),
                        b'u' => result.push(self.parse_unicode_escape()?),
                        other => {
                            return Err(self.error(format!("invalid escape '\\{}'", other as char)))
                        }
                    }
                }
                byte if byte < 0x20 => {
                    return Err(self.error("control character in string"));
                }
                byte => {
                    // Consume the full UTF-8 sequence of this character.
                    let start = self.position - 1;
                    let mut length = 1;
                    if byte >= 0xF0 {
                        length = 4;
                    } else if byte >= 0xE0 {
                        length = 3;
                    } else if byte >= 0xC0 {
                        length = 2;
                    }
                    let end = (start + length).min(self.bytes.len());
                    let text = std::str::from_utf8(&self.bytes[start..end])
                        .map_err(|_| self.error("invalid UTF-8 in string"))?;
                    result.push_str(text);
                    self.position = end;
                }
            }
        }
    }

    fn parse_unicode_escape(&mut self) -> Result<char, JsonError> {
        let value = self.parse_hex_quad()?;
        // Surrogate pairs: a high surrogate (\uD800..\uDBFF) must be followed
        // by a low surrogate (\uDC00..\uDFFF); together they encode one
        // astral code point.  Lone surrogates are invalid on their own.
        if (0xD800..=0xDBFF).contains(&value) {
            if self.peek() != Some(b'\\') {
                return Err(self.error("expected low surrogate after high surrogate"));
            }
            self.position += 1;
            if self.peek() != Some(b'u') {
                return Err(self.error("expected low surrogate after high surrogate"));
            }
            self.position += 1;
            let low = self.parse_hex_quad()?;
            if !(0xDC00..=0xDFFF).contains(&low) {
                return Err(self.error("invalid low surrogate"));
            }
            let combined = 0x10000 + ((value - 0xD800) << 10) + (low - 0xDC00);
            return char::from_u32(combined)
                .ok_or_else(|| self.error("invalid unicode code point"));
        }
        if (0xDC00..=0xDFFF).contains(&value) {
            return Err(self.error("unexpected low surrogate"));
        }
        char::from_u32(value).ok_or_else(|| self.error("invalid unicode code point"))
    }

    fn parse_hex_quad(&mut self) -> Result<u32, JsonError> {
        if self.position + 4 > self.bytes.len() {
            return Err(self.error("incomplete unicode escape"));
        }
        let digits = &self.bytes[self.position..self.position + 4];
        let mut value = 0u32;
        for &digit in digits {
            let digit_value = hex_value(digit)
                .ok_or_else(|| self.error(format!("invalid hex digit '{}'", digit as char)))?;
            value = value
                .checked_mul(16)
                .and_then(|v| v.checked_add(digit_value))
                .ok_or_else(|| self.error("unicode escape overflow"))?;
        }
        self.position += 4;
        Ok(value)
    }

    fn parse_number(&mut self) -> Result<Json, JsonError> {
        let start = self.position;
        if self.peek() == Some(b'-') {
            self.position += 1;
        }
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.position += 1;
        }
        if self.peek() == Some(b'.') {
            self.position += 1;
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.position += 1;
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.position += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.position += 1;
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.position += 1;
            }
        }
        let text = std::str::from_utf8(&self.bytes[start..self.position])
            .map_err(|_| self.error("invalid number"))?;
        text.parse::<f64>()
            .map(Json::Number)
            .map_err(|_| self.error(format!("invalid number '{text}'")))
    }

    fn parse_array(&mut self) -> Result<Json, JsonError> {
        self.position += 1; // consume '['
        let mut items = Vec::new();
        self.skip_whitespace();
        if self.peek() == Some(b']') {
            self.position += 1;
            return Ok(Json::Array(items));
        }
        loop {
            items.push(self.parse_value()?);
            self.skip_whitespace();
            match self.peek() {
                Some(b',') => {
                    self.position += 1;
                }
                Some(b']') => {
                    self.position += 1;
                    return Ok(Json::Array(items));
                }
                _ => return Err(self.error("expected ',' or ']' in array")),
            }
        }
    }

    fn parse_object(&mut self) -> Result<Json, JsonError> {
        self.position += 1; // consume '{'
        let mut pairs = Vec::new();
        self.skip_whitespace();
        if self.peek() == Some(b'}') {
            self.position += 1;
            return Ok(Json::Object(pairs));
        }
        loop {
            self.skip_whitespace();
            if self.peek() != Some(b'"') {
                return Err(self.error("expected string key in object"));
            }
            let key = self.parse_string()?;
            self.skip_whitespace();
            if self.peek() != Some(b':') {
                return Err(self.error("expected ':' after object key"));
            }
            self.position += 1;
            let value = self.parse_value()?;
            pairs.push((key, value));
            self.skip_whitespace();
            match self.peek() {
                Some(b',') => {
                    self.position += 1;
                }
                Some(b'}') => {
                    self.position += 1;
                    return Ok(Json::Object(pairs));
                }
                _ => return Err(self.error("expected ',' or '}' in object")),
            }
        }
    }
}

fn hex_value(byte: u8) -> Option<u32> {
    match byte {
        b'0'..=b'9' => Some((byte - b'0') as u32),
        b'a'..=b'f' => Some((byte - b'a' + 10) as u32),
        b'A'..=b'F' => Some((byte - b'A' + 10) as u32),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_objects() {
        let value = Json::obj(vec![
            ("jsonrpc", Json::str("2.0")),
            ("id", Json::num(1)),
            ("method", Json::str("initialize")),
            (
                "params",
                Json::obj(vec![("capabilities", Json::Object(vec![]))]),
            ),
        ]);
        let text = value.serialize();
        assert!(text.contains("\"jsonrpc\":\"2.0\""));
        let parsed = parse(&text).unwrap();
        assert_eq!(parsed, value);
    }

    #[test]
    fn parse_scalars() {
        assert_eq!(parse("null").unwrap(), Json::Null);
        assert_eq!(parse("true").unwrap(), Json::Bool(true));
        assert_eq!(parse("42").unwrap(), Json::Number(42.0));
        assert_eq!(parse("-1.5e2").unwrap(), Json::Number(-150.0));
        assert_eq!(
            parse("\"hi\\n\\\"there\\\"\"").unwrap(),
            Json::str("hi\n\"there\"")
        );
        assert_eq!(
            parse("[1, 2, 3]").unwrap(),
            Json::Array(vec![
                Json::Number(1.0),
                Json::Number(2.0),
                Json::Number(3.0),
            ])
        );
    }

    #[test]
    fn parse_unicode_escape() {
        assert_eq!(parse("\"\\u0041\"").unwrap(), Json::str("A"));
        assert_eq!(parse("\"\\u00e9\"").unwrap(), Json::str("é"));
    }

    #[test]
    fn parse_surrogate_pair_escape() {
        // 😀 = U+1F600 = \uD83D\uDE00 as a surrogate pair.
        assert_eq!(parse("\"\\uD83D\\uDE00\"").unwrap(), Json::str("😀"));
        assert_eq!(parse("\"a\\uD83D\\uDE00b\"").unwrap(), Json::str("a😀b"));
    }

    #[test]
    fn reject_lone_surrogates() {
        // A lone high surrogate without a following low surrogate, and a lone
        // low surrogate, are invalid JSON escapes.
        assert!(parse("\"\\uD83D\"").is_err());
        assert!(parse("\"\\uDE00\"").is_err());
        assert!(parse("\"\\uD83Dx\"").is_err());
    }

    #[test]
    fn reject_malformed() {
        assert!(parse("{").is_err());
        assert!(parse("[1,]").is_err());
        assert!(parse("\"unterminated").is_err());
        assert!(parse("tru").is_err());
        assert!(parse("").is_err());
    }

    #[test]
    fn number_integer_rendering() {
        assert_eq!(Json::Number(7.0).serialize(), "7");
        assert_eq!(Json::Number(0.5).serialize(), "0.5");
    }
}
