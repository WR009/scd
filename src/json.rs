//! A tiny, dependency-free JSON reader/writer.
//!
//! `scd` stores everything it knows in one `config.json`. The project deliberately has no
//! external crates (it must build offline), so this module implements exactly the JSON
//! subset that file needs - plus enough strictness to tell the user where a hand-edited
//! file went wrong. Object key order is preserved, which keeps re-saves diff-friendly.

use std::fmt::{self, Write as _};

/// Any JSON value.
#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Array(Vec<Json>),
    /// Key/value pairs in document order (duplicates collapse, last value wins).
    Object(Vec<(String, Json)>),
}

/// A parse failure with a human-usable position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub message: String,
    pub line: usize,
    pub column: usize,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (line {}, column {})", self.message, self.line, self.column)
    }
}

impl std::error::Error for Error {}

impl Json {
    pub fn object() -> Self {
        Json::Object(Vec::new())
    }

    pub fn array() -> Self {
        Json::Array(Vec::new())
    }

    /// Look up an object key.
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Object(entries) => entries.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// Insert or replace an object key, keeping the original position on replace.
    pub fn insert(&mut self, key: &str, value: Json) {
        if let Json::Object(entries) = self {
            match entries.iter_mut().find(|(k, _)| k == key) {
                Some(slot) => slot.1 = value,
                None => entries.push((key.to_string(), value)),
            }
        } else {
            *self = Json::Object(vec![(key.to_string(), value)]);
        }
    }

    /// Append to an array.
    pub fn push(&mut self, value: Json) {
        if let Json::Array(items) = self {
            items.push(value);
        } else {
            *self = Json::Array(vec![value]);
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(text) => Some(text),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Json::Bool(flag) => Some(*flag),
            _ => None,
        }
    }

    /// Integral numbers only; `2.5` and `null` yield `None`.
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Json::Num(n) if n.fract() == 0.0 => {
                if *n >= i64::MIN as f64 && *n <= i64::MAX as f64 {
                    Some(*n as i64)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    pub fn as_u64(&self) -> Option<u64> {
        self.as_i64().and_then(|n| u64::try_from(n).ok())
    }

    pub fn as_array(&self) -> Option<&[Json]> {
        match self {
            Json::Array(items) => Some(items),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<&[(String, Json)]> {
        match self {
            Json::Object(entries) => Some(entries),
            _ => None,
        }
    }

    /// Parse a whole document; trailing garbage is an error.
    pub fn parse(text: &str) -> Result<Json, Error> {
        let mut parser = Parser::new(text);
        parser.skip_ws();
        let value = parser.value()?;
        parser.skip_ws();
        if !parser.eof() {
            return Err(parser.error("unexpected text after the JSON value"));
        }
        Ok(value)
    }

    /// Serialise with two-space indentation and a trailing newline on every nesting level.
    pub fn to_string_pretty(&self) -> String {
        let mut out = String::new();
        self.write(&mut out, 0);
        out
    }

    fn write(&self, out: &mut String, depth: usize) {
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(true) => out.push_str("true"),
            Json::Bool(false) => out.push_str("false"),
            Json::Num(n) => write_number(out, *n),
            Json::Str(text) => write_string(out, text),
            Json::Array(items) => {
                if items.is_empty() {
                    out.push_str("[]");
                    return;
                }
                out.push('[');
                for (index, item) in items.iter().enumerate() {
                    if index > 0 {
                        out.push(',');
                    }
                    out.push('\n');
                    push_indent(out, depth + 1);
                    item.write(out, depth + 1);
                }
                out.push('\n');
                push_indent(out, depth);
                out.push(']');
            }
            Json::Object(entries) => {
                if entries.is_empty() {
                    out.push_str("{}");
                    return;
                }
                out.push('{');
                for (index, (key, value)) in entries.iter().enumerate() {
                    if index > 0 {
                        out.push(',');
                    }
                    out.push('\n');
                    push_indent(out, depth + 1);
                    write_string(out, key);
                    out.push_str(": ");
                    value.write(out, depth + 1);
                }
                out.push('\n');
                push_indent(out, depth);
                out.push('}');
            }
        }
    }
}

impl From<&str> for Json {
    fn from(value: &str) -> Self {
        Json::Str(value.to_string())
    }
}

impl From<String> for Json {
    fn from(value: String) -> Self {
        Json::Str(value)
    }
}

impl From<i64> for Json {
    fn from(value: i64) -> Self {
        Json::Num(value as f64)
    }
}

impl From<u64> for Json {
    fn from(value: u64) -> Self {
        Json::Num(value as f64)
    }
}

impl From<usize> for Json {
    fn from(value: usize) -> Self {
        Json::Num(value as f64)
    }
}

impl From<bool> for Json {
    fn from(value: bool) -> Self {
        Json::Bool(value)
    }
}

fn push_indent(out: &mut String, depth: usize) {
    for _ in 0..depth {
        out.push_str("  ");
    }
}

fn write_number(out: &mut String, n: f64) {
    if !n.is_finite() {
        // JSON has no NaN/Infinity; null keeps the document parseable.
        out.push_str("null");
        return;
    }
    if n.fract() == 0.0 && n.abs() < 9_007_199_254_740_992.0 {
        let _ = write!(out, "{}", n as i64);
    } else {
        let _ = write!(out, "{n}");
    }
}

/// Escape `text` as a JSON string literal (quotes included).
pub fn write_string(out: &mut String, text: &str) {
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            other if (other as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", other as u32);
            }
            other => out.push(other),
        }
    }
    out.push('"');
}

/// Recursive-descent parser over `Vec<char>` so that positions map to characters, not bytes.
struct Parser {
    chars: Vec<char>,
    pos: usize,
}

impl Parser {
    fn new(text: &str) -> Self {
        Parser { chars: text.chars().collect(), pos: 0 }
    }

    fn eof(&self) -> bool {
        self.pos >= self.chars.len()
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn peek_at(&self, offset: usize) -> Option<char> {
        self.chars.get(self.pos + offset).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let ch = self.peek();
        if ch.is_some() {
            self.pos += 1;
        }
        ch
    }

    fn skip_ws(&mut self) {
        while let Some(ch) = self.peek() {
            if ch == ' ' || ch == '\t' || ch == '\n' || ch == '\r' {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn error(&self, message: &str) -> Error {
        let mut line = 1usize;
        let mut column = 1usize;
        for &ch in self.chars.iter().take(self.pos.min(self.chars.len())) {
            if ch == '\n' {
                line += 1;
                column = 1;
            } else {
                column += 1;
            }
        }
        Error { message: message.to_string(), line, column }
    }

    fn value(&mut self) -> Result<Json, Error> {
        match self.peek() {
            None => Err(self.error("unexpected end of input")),
            Some('{') => self.object(),
            Some('[') => self.array(),
            Some('"') => Ok(Json::Str(self.string()?)),
            Some('t') => self.literal("true", Json::Bool(true)),
            Some('f') => self.literal("false", Json::Bool(false)),
            Some('n') => self.literal("null", Json::Null),
            Some(ch) if ch == '-' || ch == '+' || ch.is_ascii_digit() => self.number(),
            Some(ch) => Err(self.error(&format!("unexpected character {ch:?}"))),
        }
    }

    fn literal(&mut self, word: &str, value: Json) -> Result<Json, Error> {
        let start = self.pos;
        for expected in word.chars() {
            if self.bump() != Some(expected) {
                self.pos = start;
                return Err(self.error(&format!("expected '{word}'")));
            }
        }
        Ok(value)
    }

    fn string(&mut self) -> Result<String, Error> {
        if self.bump() != Some('"') {
            return Err(self.error("expected a quoted string"));
        }
        let mut out = String::new();
        loop {
            let Some(ch) = self.bump() else {
                return Err(self.error("unterminated string"));
            };
            match ch {
                '"' => return Ok(out),
                '\\' => {
                    let Some(escape) = self.bump() else {
                        return Err(self.error("unterminated escape sequence"));
                    };
                    match escape {
                        '"' => out.push('"'),
                        '\\' => out.push('\\'),
                        '/' => out.push('/'),
                        'b' => out.push('\u{8}'),
                        'f' => out.push('\u{c}'),
                        'n' => out.push('\n'),
                        'r' => out.push('\r'),
                        't' => out.push('\t'),
                        'u' => out.push(self.unicode_escape()?),
                        other => {
                            return Err(self.error(&format!("invalid escape sequence '\\{other}'")))
                        }
                    }
                }
                other if (other as u32) < 0x20 => {
                    return Err(self.error("unescaped control character inside a string"));
                }
                other => out.push(other),
            }
        }
    }

    fn unicode_escape(&mut self) -> Result<char, Error> {
        let high = self.hex4()?;
        let code = if (0xD800..0xDC00).contains(&high) {
            if self.peek() != Some('\\') || self.peek_at(1) != Some('u') {
                return Err(self.error("high surrogate is not followed by \\u low surrogate"));
            }
            self.pos += 2;
            let low = self.hex4()?;
            if !(0xDC00..0xE000).contains(&low) {
                return Err(self.error("invalid low surrogate in unicode escape"));
            }
            0x1_0000 + ((high - 0xD800) << 10) + (low - 0xDC00)
        } else if (0xDC00..0xE000).contains(&high) {
            return Err(self.error("unexpected low surrogate in unicode escape"));
        } else {
            high
        };
        char::from_u32(code).ok_or_else(|| self.error("unicode escape is not a character"))
    }

    fn hex4(&mut self) -> Result<u32, Error> {
        let mut value = 0u32;
        for _ in 0..4 {
            let Some(ch) = self.bump() else {
                return Err(self.error("truncated unicode escape"));
            };
            let digit = match ch.to_digit(16) {
                Some(digit) => digit,
                None => return Err(self.error(&format!("{ch:?} is not a hex digit"))),
            };
            value = value * 16 + digit;
        }
        Ok(value)
    }

    fn number(&mut self) -> Result<Json, Error> {
        let start = self.pos;
        if matches!(self.peek(), Some('-') | Some('+')) {
            self.pos += 1;
        }
        let mut digits = false;
        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit() {
                digits = true;
                self.pos += 1;
            } else {
                break;
            }
        }
        if self.peek() == Some('.') {
            self.pos += 1;
            while let Some(ch) = self.peek() {
                if ch.is_ascii_digit() {
                    digits = true;
                    self.pos += 1;
                } else {
                    break;
                }
            }
        }
        if matches!(self.peek(), Some('e') | Some('E')) {
            self.pos += 1;
            if matches!(self.peek(), Some('-') | Some('+')) {
                self.pos += 1;
            }
            let mut exponent_digits = false;
            while let Some(ch) = self.peek() {
                if ch.is_ascii_digit() {
                    exponent_digits = true;
                    self.pos += 1;
                } else {
                    break;
                }
            }
            if !exponent_digits {
                return Err(self.error("number exponent has no digits"));
            }
        }
        if !digits {
            self.pos = start;
            return Err(self.error("malformed number"));
        }
        let text: String = self.chars[start..self.pos].iter().collect();
        match text.parse::<f64>() {
            Ok(value) => Ok(Json::Num(value)),
            Err(_) => Err(self.error(&format!("malformed number '{text}'"))),
        }
    }

    fn array(&mut self) -> Result<Json, Error> {
        self.pos += 1; // consume '['
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(']') {
            self.pos += 1;
            return Ok(Json::Array(items));
        }
        loop {
            self.skip_ws();
            items.push(self.value()?);
            self.skip_ws();
            match self.bump() {
                Some(',') => continue,
                Some(']') => return Ok(Json::Array(items)),
                Some(ch) => {
                    return Err(self.error(&format!("expected ',' or ']' in array, found {ch:?}")))
                }
                None => return Err(self.error("unterminated array")),
            }
        }
    }

    fn object(&mut self) -> Result<Json, Error> {
        self.pos += 1; // consume '{'
        let mut entries: Vec<(String, Json)> = Vec::new();
        self.skip_ws();
        if self.peek() == Some('}') {
            self.pos += 1;
            return Ok(Json::Object(entries));
        }
        loop {
            self.skip_ws();
            if self.peek() != Some('"') {
                return Err(self.error("object keys must be quoted strings"));
            }
            let key = self.string()?;
            self.skip_ws();
            match self.bump() {
                Some(':') => {}
                Some(ch) => {
                    return Err(self.error(&format!("expected ':' after key {key:?}, found {ch:?}")))
                }
                None => return Err(self.error("unterminated object")),
            }
            self.skip_ws();
            let value = self.value()?;
            match entries.iter_mut().find(|(existing, _)| *existing == key) {
                Some(slot) => slot.1 = value,
                None => entries.push((key, value)),
            }
            self.skip_ws();
            match self.bump() {
                Some(',') => continue,
                Some('}') => return Ok(Json::Object(entries)),
                Some(ch) => {
                    return Err(self.error(&format!("expected ',' or '}}' in object, found {ch:?}")))
                }
                None => return Err(self.error("unterminated object")),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nested_document() {
        let text = r#"{
            "version": 1,
            "max_display": 9,
            "locations": [
                { "name": "scd", "path": "D:\\data\\Projects\\scd", "visits": 3 },
                "D:\\plain\\string"
            ],
            "flag": true,
            "nothing": null
        }"#;
        let json = Json::parse(text).expect("valid document");
        assert_eq!(json.get("version").and_then(Json::as_i64), Some(1));
        assert_eq!(json.get("max_display").and_then(Json::as_i64), Some(9));
        assert_eq!(json.get("flag").and_then(Json::as_bool), Some(true));
        assert_eq!(json.get("nothing"), Some(&Json::Null));

        let locations = json.get("locations").and_then(Json::as_array).expect("array");
        assert_eq!(locations.len(), 2);
        assert_eq!(
            locations[0].get("path").and_then(Json::as_str),
            Some("D:\\data\\Projects\\scd")
        );
        assert_eq!(locations[0].get("visits").and_then(Json::as_u64), Some(3));
        assert_eq!(locations[1].as_str(), Some("D:\\plain\\string"));
    }

    #[test]
    fn keeps_object_key_order() {
        let json = Json::parse(r#"{"b":1,"a":2,"c":3}"#).unwrap();
        let keys: Vec<&str> = json
            .as_object()
            .unwrap()
            .iter()
            .map(|(key, _)| key.as_str())
            .collect();
        assert_eq!(keys, vec!["b", "a", "c"]);
        assert_eq!(json.to_string_pretty(), "{\n  \"b\": 1,\n  \"a\": 2,\n  \"c\": 3\n}");
    }

    #[test]
    fn duplicate_keys_keep_first_position_and_last_value() {
        let json = Json::parse(r#"{"a":1,"b":2,"a":3}"#).unwrap();
        assert_eq!(json.get("a").and_then(Json::as_i64), Some(3));
        let keys: Vec<&str> = json.as_object().unwrap().iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn handles_string_escapes_and_unicode() {
        let json = Json::parse(r#""quote \" back \\ slash \/ tab \t nl \n uni \u00e6 \ud83d\ude80""#)
            .unwrap();
        assert_eq!(
            json.as_str().unwrap(),
            "quote \" back \\ slash / tab \t nl \n uni \u{e6} \u{1f680}"
        );
    }

    #[test]
    fn round_trips_windows_paths() {
        let mut root = Json::object();
        let mut location = Json::object();
        location.insert("path", Json::from("D:\\data\\我的项目\\scd"));
        location.insert("visits", Json::from(7i64));
        let mut locations = Json::array();
        locations.push(location);
        root.insert("locations", locations);

        let text = root.to_string_pretty();
        assert!(text.contains(r#""path": "D:\\data\\我的项目\\scd""#), "{text}");
        let reparsed = Json::parse(&text).expect("our own output must parse");
        assert_eq!(reparsed, root);
        assert_eq!(
            reparsed
                .get("locations")
                .and_then(Json::as_array)
                .unwrap()[0]
                .get("path")
                .and_then(Json::as_str),
            Some("D:\\data\\我的项目\\scd")
        );
    }

    #[test]
    fn numbers_are_formatted_as_integers_when_integral() {
        assert_eq!(Json::Num(3.0).to_string_pretty(), "3");
        assert_eq!(Json::Num(-12.0).to_string_pretty(), "-12");
        assert_eq!(Json::Num(2.5).to_string_pretty(), "2.5");
        assert_eq!(Json::parse("1e3").unwrap(), Json::Num(1000.0));
        assert_eq!(Json::Num(f64::NAN).to_string_pretty(), "null");
    }

    #[test]
    fn as_i64_rejects_fractions() {
        assert_eq!(Json::Num(2.5).as_i64(), None);
        assert_eq!(Json::Num(2.0).as_i64(), Some(2));
        assert_eq!(Json::Str("2".to_string()).as_i64(), None);
    }

    #[test]
    fn insert_replaces_in_place() {
        let mut json = Json::object();
        json.insert("a", Json::from(1i64));
        json.insert("b", Json::from(2i64));
        json.insert("a", Json::from(9i64));
        assert_eq!(json.to_string_pretty(), "{\n  \"a\": 9,\n  \"b\": 2\n}");
    }

    #[test]
    fn reports_errors_with_position() {
        let err = Json::parse("{\n  \"a\": ,\n}").unwrap_err();
        assert_eq!(err.line, 2);
        assert!(err.message.contains("unexpected character ','"), "{err}");
        assert!(err.to_string().contains("line 2"), "{err}");
    }

    #[test]
    fn rejects_malformed_documents() {
        for bad in [
            "",
            "   ",
            "{",
            "[]]",
            "{\"a\"}",
            "{\"a\":}",
            "{\"a\":1,}",
            "[1,]",
            "\"unterminated",
            "{\"a\":1} trailing",
            "{\"a\":tru}",
            "\\u12",
            "\"\\q\"",
            "\"raw\nnewline\"",
        ] {
            assert!(Json::parse(bad).is_err(), "should not parse: {bad:?}");
        }
    }

    #[test]
    fn accepts_empty_containers_and_plain_scalars() {
        assert_eq!(Json::parse("{}").unwrap(), Json::Object(vec![]));
        assert_eq!(Json::parse("[]").unwrap(), Json::Array(vec![]));
        assert_eq!(Json::parse("  \"x\"  ").unwrap(), Json::Str("x".to_string()));
        assert_eq!(Json::parse("-0.5").unwrap(), Json::Num(-0.5));
        assert_eq!(Json::parse("null").unwrap(), Json::Null);
    }

    #[test]
    fn escapes_control_characters() {
        let mut out = String::new();
        write_string(&mut out, "a\u{1}b\"c\\d");
        assert_eq!(out, "\"a\\u0001b\\\"c\\\\d\"");
        assert_eq!(Json::parse(&out).unwrap().as_str().unwrap(), "a\u{1}b\"c\\d");
    }
}