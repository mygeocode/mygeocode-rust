//! A small, strict JSON reader and writer (RFC 8259), so the crate needs no JSON dependency.

use std::fmt;

/// A decoded JSON value. Objects keep the order the API sent their fields in.
#[derive(Clone, Debug, PartialEq, Default)]
pub enum Value {
    /// `null` (also what a missing field reads as).
    #[default]
    Null,
    /// `true` or `false`.
    Bool(bool),
    /// A number. Whole numbers that fit are kept exactly, see [`Value::as_i64`].
    Number(Number),
    /// A string.
    String(String),
    /// An array.
    Array(Vec<Value>),
    /// An object, as name and value pairs in the order they were sent.
    Object(Vec<(String, Value)>),
}

/// A JSON number: a whole number that fits in an `i64`, or any other number as `f64`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Number {
    /// A whole number.
    Int(i64),
    /// Any other number.
    Float(f64),
}

static NULL: Value = Value::Null;

impl Value {
    /// The field `name` of an object; [`Value::Null`] when it is missing or this is not an object.
    pub fn get(&self, name: &str) -> &Value {
        match self {
            Value::Object(fields) => fields
                .iter()
                .find(|(k, _)| k == name)
                .map(|(_, v)| v)
                .unwrap_or(&NULL),
            _ => &NULL,
        }
    }

    /// True for `null`.
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    /// The text of a string.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    /// A number as `f64`.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Number(Number::Int(i)) => Some(*i as f64),
            Value::Number(Number::Float(f)) => Some(*f),
            _ => None,
        }
    }

    /// A whole number as `i64` (a decimal with no fraction counts too).
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Number(Number::Int(i)) => Some(*i),
            Value::Number(Number::Float(f)) if f.fract() == 0.0 && f.abs() < 9.0e15 => {
                Some(*f as i64)
            }
            _ => None,
        }
    }

    /// A boolean.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// The items of an array.
    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Value::Array(items) => Some(items),
            _ => None,
        }
    }

    /// The fields of an object.
    pub fn as_object(&self) -> Option<&[(String, Value)]> {
        match self {
            Value::Object(fields) => Some(fields),
            _ => None,
        }
    }

    /// Reads JSON text.
    pub fn parse(text: &str) -> Result<Value, ParseError> {
        let mut p = Parser {
            s: text.as_bytes(),
            pos: 0,
        };
        p.ws();
        let v = p.value(0)?;
        p.ws();
        if p.pos != p.s.len() {
            return Err(p.err("unexpected trailing characters"));
        }
        Ok(v)
    }

    /// Writes compact JSON text.
    pub fn to_json(&self) -> String {
        let mut out = String::new();
        write_value(&mut out, self);
        out
    }

    /// Text for a text field: a string as it is, a number or boolean written as text, anything else `None`.
    pub(crate) fn text(&self) -> Option<String> {
        match self {
            Value::String(s) => Some(s.clone()),
            Value::Bool(b) => Some(b.to_string()),
            Value::Number(Number::Int(i)) => Some(i.to_string()),
            Value::Number(Number::Float(f)) => Some(format_number(*f)),
            _ => None,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_json())
    }
}

/// JSON text that could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    /// What was wrong.
    pub message: String,
    /// The byte position it was found at.
    pub position: usize,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at byte {}", self.message, self.position)
    }
}

impl std::error::Error for ParseError {}

struct Parser<'a> {
    s: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn err(&self, message: &str) -> ParseError {
        ParseError {
            message: message.to_string(),
            position: self.pos,
        }
    }

    fn ws(&mut self) {
        while self.pos < self.s.len() && matches!(self.s[self.pos], b' ' | b'\t' | b'\n' | b'\r') {
            self.pos += 1;
        }
    }

    fn value(&mut self, depth: usize) -> Result<Value, ParseError> {
        if depth > 512 {
            return Err(self.err("nesting too deep"));
        }
        match self.s.get(self.pos) {
            None => Err(self.err("unexpected end of input")),
            Some(b'{') => self.object(depth),
            Some(b'[') => self.array(depth),
            Some(b'"') => self.string().map(Value::String),
            Some(b't') => self.literal("true", Value::Bool(true)),
            Some(b'f') => self.literal("false", Value::Bool(false)),
            Some(b'n') => self.literal("null", Value::Null),
            Some(c) if *c == b'-' || c.is_ascii_digit() => self.number(),
            Some(_) => Err(self.err("unexpected character")),
        }
    }

    fn literal(&mut self, word: &str, v: Value) -> Result<Value, ParseError> {
        if self.s[self.pos..].starts_with(word.as_bytes()) {
            self.pos += word.len();
            Ok(v)
        } else {
            Err(self.err("invalid literal"))
        }
    }

    fn object(&mut self, depth: usize) -> Result<Value, ParseError> {
        self.pos += 1;
        let mut fields = Vec::new();
        self.ws();
        if self.s.get(self.pos) == Some(&b'}') {
            self.pos += 1;
            return Ok(Value::Object(fields));
        }
        loop {
            self.ws();
            if self.s.get(self.pos) != Some(&b'"') {
                return Err(self.err("expected a string key"));
            }
            let key = self.string()?;
            self.ws();
            if self.s.get(self.pos) != Some(&b':') {
                return Err(self.err("expected ':'"));
            }
            self.pos += 1;
            self.ws();
            let v = self.value(depth + 1)?;
            match fields
                .iter_mut()
                .find(|(k, _): &&mut (String, Value)| *k == key)
            {
                Some(slot) => slot.1 = v,
                None => fields.push((key, v)),
            }
            self.ws();
            match self.s.get(self.pos) {
                Some(b',') => self.pos += 1,
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(Value::Object(fields));
                }
                _ => return Err(self.err("expected ',' or '}'")),
            }
        }
    }

    fn array(&mut self, depth: usize) -> Result<Value, ParseError> {
        self.pos += 1;
        let mut items = Vec::new();
        self.ws();
        if self.s.get(self.pos) == Some(&b']') {
            self.pos += 1;
            return Ok(Value::Array(items));
        }
        loop {
            self.ws();
            items.push(self.value(depth + 1)?);
            self.ws();
            match self.s.get(self.pos) {
                Some(b',') => self.pos += 1,
                Some(b']') => {
                    self.pos += 1;
                    return Ok(Value::Array(items));
                }
                _ => return Err(self.err("expected ',' or ']'")),
            }
        }
    }

    fn hex4(&mut self) -> Result<u32, ParseError> {
        let digits = self
            .s
            .get(self.pos..self.pos + 4)
            .ok_or_else(|| self.err("bad unicode escape"))?;
        let text = std::str::from_utf8(digits).map_err(|_| self.err("bad unicode escape"))?;
        let n = u32::from_str_radix(text, 16).map_err(|_| self.err("bad unicode escape"))?;
        self.pos += 4;
        Ok(n)
    }

    fn string(&mut self) -> Result<String, ParseError> {
        self.pos += 1;
        let mut out: Vec<u8> = Vec::new();
        loop {
            let c = *self
                .s
                .get(self.pos)
                .ok_or_else(|| self.err("unterminated string"))?;
            self.pos += 1;
            match c {
                b'"' => {
                    return String::from_utf8(out).map_err(|_| self.err("invalid UTF-8 in string"))
                }
                b'\\' => {
                    let e = *self
                        .s
                        .get(self.pos)
                        .ok_or_else(|| self.err("unterminated escape"))?;
                    self.pos += 1;
                    let ch = match e {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => {
                            let first = self.hex4()?;
                            let code = if (0xD800..0xDC00).contains(&first) {
                                if self.s.get(self.pos..self.pos + 2) != Some(b"\\u") {
                                    return Err(self.err("lone surrogate"));
                                }
                                self.pos += 2;
                                let second = self.hex4()?;
                                if !(0xDC00..0xE000).contains(&second) {
                                    return Err(self.err("lone surrogate"));
                                }
                                0x10000 + ((first - 0xD800) << 10) + (second - 0xDC00)
                            } else {
                                first
                            };
                            char::from_u32(code).ok_or_else(|| self.err("bad unicode escape"))?
                        }
                        _ => return Err(self.err("bad escape")),
                    };
                    let mut buf = [0u8; 4];
                    out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                }
                c if c < 0x20 => return Err(self.err("control character in string")),
                c => out.push(c),
            }
        }
    }

    fn digits(&mut self) -> usize {
        let start = self.pos;
        while self.pos < self.s.len() && self.s[self.pos].is_ascii_digit() {
            self.pos += 1;
        }
        self.pos - start
    }

    fn number(&mut self) -> Result<Value, ParseError> {
        let start = self.pos;
        if self.s[self.pos] == b'-' {
            self.pos += 1;
        }
        match self.s.get(self.pos) {
            Some(b'0') => self.pos += 1,
            Some(c) if c.is_ascii_digit() => {
                self.digits();
            }
            _ => return Err(self.err("invalid number")),
        }
        let mut whole = true;
        if self.s.get(self.pos) == Some(&b'.') {
            whole = false;
            self.pos += 1;
            if self.digits() == 0 {
                return Err(self.err("invalid number"));
            }
        }
        if matches!(self.s.get(self.pos), Some(b'e') | Some(b'E')) {
            whole = false;
            self.pos += 1;
            if matches!(self.s.get(self.pos), Some(b'+') | Some(b'-')) {
                self.pos += 1;
            }
            if self.digits() == 0 {
                return Err(self.err("invalid number"));
            }
        }
        let text = std::str::from_utf8(&self.s[start..self.pos])
            .map_err(|_| self.err("invalid number"))?;
        if whole {
            if let Ok(i) = text.parse::<i64>() {
                return Ok(Value::Number(Number::Int(i)));
            }
        }
        text.parse::<f64>()
            .map(|f| Value::Number(Number::Float(f)))
            .map_err(|_| self.err("invalid number"))
    }
}

/// A number as plain text: 48.8584, 13, -74.0445 (never an exponent, no trailing .0 on whole numbers).
pub(crate) fn format_number(f: f64) -> String {
    if f.is_finite() && f.fract() == 0.0 && f.abs() < 1e15 {
        format!("{}", f as i64)
    } else {
        format!("{}", f)
    }
}

fn write_value(out: &mut String, v: &Value) {
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(Number::Int(i)) => out.push_str(&i.to_string()),
        Value::Number(Number::Float(f)) => {
            if f.is_finite() {
                let text = format!("{}", f);
                out.push_str(&text);
                if !text.contains('.') && !text.contains('e') && !text.contains("inf") {
                    out.push_str(".0");
                }
            } else {
                out.push_str("null");
            }
        }
        Value::String(s) => quote(out, s),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_value(out, item);
            }
            out.push(']');
        }
        Value::Object(fields) => {
            out.push('{');
            for (i, (k, item)) in fields.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                quote(out, k);
                out.push(':');
                write_value(out, item);
            }
            out.push('}');
        }
    }
}

fn quote(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_every_kind_of_value() {
        let v = Value::parse(r#" {"s":"a\"b\u00fc\n\ud83c\udde8","i":-12,"d":1.5e2,"t":true,"f":false,"n":null,"l":[1,[2],{}]} "#).unwrap();
        assert_eq!(v.get("s").as_str(), Some("a\"b\u{fc}\n\u{1f1e8}"));
        assert_eq!(v.get("i").as_i64(), Some(-12));
        assert_eq!(v.get("d").as_f64(), Some(150.0));
        assert_eq!(v.get("t").as_bool(), Some(true));
        assert!(v.get("n").is_null());
        assert!(v.get("missing").is_null());
        assert_eq!(v.get("l").as_array().map(|a| a.len()), Some(3));
    }

    #[test]
    fn rejects_broken_json() {
        for bad in [
            "",
            "{",
            "{\"a\":}",
            "[1,]",
            "tru",
            "\"abc",
            "{\"a\":1} x",
            "01",
            "\"\\x\"",
            "\"\\ud800\"",
        ] {
            assert!(Value::parse(bad).is_err(), "{}", bad);
        }
    }

    #[test]
    fn writes_what_it_reads() {
        let text = r#"{"queries":["Zürich \"quoted\"\n"],"coords":[[48.8584,2.2945]],"n":null,"b":true,"i":3,"f":2.0}"#;
        assert_eq!(Value::parse(text).unwrap().to_json(), text);
        assert_eq!(Value::String("\u{1}".into()).to_json(), "\"\\u0001\"");
    }

    #[test]
    fn numbers_as_query_text() {
        assert_eq!(format_number(48.8584), "48.8584");
        assert_eq!(format_number(13.0), "13");
        assert_eq!(format_number(-74.0445), "-74.0445");
        assert_eq!(
            Value::parse("68467").unwrap().text().as_deref(),
            Some("68467")
        );
    }
}
