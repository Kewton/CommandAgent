//! The bounded JSON decode of the G2 protocol. A `serde_json::Value` has already
//! lost a repeated key and the excess digits of a number, so this reads the raw
//! text. It accepts RFC 8259 JSON and nothing else: no repair, no second parse.

use serde_json::{Map, Value};

/// What a document may cost. A value is one element, whatever its type; a key is
/// part of its member. An empty container is depth 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DecodeLimits {
    pub(crate) bytes: usize,
    pub(crate) depth: usize,
    pub(crate) elements: usize,
    /// A number whose token an `f64` cannot hold is refused, not rounded.
    pub(crate) lossless_numbers: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DecodeError {
    TooLarge,
    TooDeep,
    TooManyElements,
    DuplicateKey,
    Malformed,
    TrailingData,
    LossyNumber,
}

impl DecodeError {
    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::TooLarge => "arguments_too_large",
            Self::TooDeep => "arguments_too_deep",
            Self::TooManyElements => "too_many_elements",
            Self::DuplicateKey => "duplicate_key",
            Self::Malformed => "malformed_json",
            Self::TrailingData => "trailing_data",
            Self::LossyNumber => "lossy_number",
        }
    }
}

pub(crate) fn decode(raw: &str, limits: DecodeLimits) -> Result<Value, DecodeError> {
    if raw.len() > limits.bytes {
        return Err(DecodeError::TooLarge);
    }
    let mut decoder = Decoder {
        raw,
        bytes: raw.as_bytes(),
        position: 0,
        elements: 0,
        limits,
    };
    decoder.skip_whitespace();
    let value = decoder.value(0)?;
    decoder.skip_whitespace();
    if decoder.position != decoder.bytes.len() {
        return Err(DecodeError::TrailingData);
    }
    Ok(value)
}

struct Decoder<'a> {
    raw: &'a str,
    bytes: &'a [u8],
    position: usize,
    elements: usize,
    limits: DecodeLimits,
}

impl Decoder<'_> {
    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.position += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.position).copied()
    }

    fn value(&mut self, depth: usize) -> Result<Value, DecodeError> {
        self.elements += 1;
        if self.elements > self.limits.elements {
            return Err(DecodeError::TooManyElements);
        }
        match self.peek().ok_or(DecodeError::Malformed)? {
            b'{' => self.object(depth + 1),
            b'[' => self.array(depth + 1),
            b'"' => self.string().map(Value::String),
            b't' => self.literal("true", Value::Bool(true)),
            b'f' => self.literal("false", Value::Bool(false)),
            b'n' => self.literal("null", Value::Null),
            _ => self.number(),
        }
    }

    fn object(&mut self, depth: usize) -> Result<Value, DecodeError> {
        if depth > self.limits.depth {
            return Err(DecodeError::TooDeep);
        }
        self.position += 1;
        let mut members = Map::new();
        self.skip_whitespace();
        if self.peek() == Some(b'}') {
            self.position += 1;
            return Ok(Value::Object(members));
        }
        loop {
            self.skip_whitespace();
            if self.peek() != Some(b'"') {
                return Err(DecodeError::Malformed);
            }
            let key = self.string()?;
            self.skip_whitespace();
            if self.peek() != Some(b':') {
                return Err(DecodeError::Malformed);
            }
            self.position += 1;
            self.skip_whitespace();
            let value = self.value(depth)?;
            if members.insert(key, value).is_some() {
                return Err(DecodeError::DuplicateKey);
            }
            self.skip_whitespace();
            match self.peek() {
                Some(b',') => self.position += 1,
                Some(b'}') => {
                    self.position += 1;
                    return Ok(Value::Object(members));
                }
                _ => return Err(DecodeError::Malformed),
            }
        }
    }

    fn array(&mut self, depth: usize) -> Result<Value, DecodeError> {
        if depth > self.limits.depth {
            return Err(DecodeError::TooDeep);
        }
        self.position += 1;
        let mut items = Vec::new();
        self.skip_whitespace();
        if self.peek() == Some(b']') {
            self.position += 1;
            return Ok(Value::Array(items));
        }
        loop {
            self.skip_whitespace();
            items.push(self.value(depth)?);
            self.skip_whitespace();
            match self.peek() {
                Some(b',') => self.position += 1,
                Some(b']') => {
                    self.position += 1;
                    return Ok(Value::Array(items));
                }
                _ => return Err(DecodeError::Malformed),
            }
        }
    }

    /// The token's escapes are decoded by `serde_json`, which refuses a control
    /// character, an unknown escape, and a lone surrogate.
    fn string(&mut self) -> Result<String, DecodeError> {
        let start = self.position;
        self.position += 1;
        loop {
            match self.peek().ok_or(DecodeError::Malformed)? {
                b'"' => break,
                b'\\' => self.position += 2,
                _ => self.position += 1,
            }
        }
        self.position += 1;
        let token = self.raw.get(start..self.position);
        token
            .and_then(|token| serde_json::from_str(token).ok())
            .ok_or(DecodeError::Malformed)
    }

    fn literal(&mut self, text: &str, value: Value) -> Result<Value, DecodeError> {
        let end = self.position + text.len();
        if self.bytes.get(self.position..end) != Some(text.as_bytes()) {
            return Err(DecodeError::Malformed);
        }
        self.position = end;
        Ok(value)
    }

    fn number(&mut self) -> Result<Value, DecodeError> {
        let start = self.position;
        while matches!(
            self.peek(),
            Some(b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9')
        ) {
            self.position += 1;
        }
        let token = self
            .raw
            .get(start..self.position)
            .ok_or(DecodeError::Malformed)?;
        let number = serde_json::from_str::<serde_json::Number>(token)
            .map_err(|_| DecodeError::Malformed)?;
        if self.limits.lossless_numbers && !holds_exactly(token, &number) {
            return Err(DecodeError::LossyNumber);
        }
        Ok(Value::Number(number))
    }
}

/// An integer token must stay an integer. A fractional token must read back, digit
/// for digit, from the shortest decimal of the `f64` it became: no count of digits
/// can promise that, because a decimal is only certain to survive up to 15 of them.
fn holds_exactly(token: &str, number: &serde_json::Number) -> bool {
    if !token.contains(['.', 'e', 'E']) {
        return number.is_i64() || number.is_u64();
    }
    let Some(value) = number.as_f64() else {
        return false;
    };
    let kept = decimal_value(&format!("{value:e}"));
    kept.is_some() && kept == decimal_value(token)
}

/// A decimal's significant digits and the power of ten of the last one, so that two
/// spellings of one number compare equal.
fn decimal_value(text: &str) -> Option<(String, i64)> {
    let text = text.strip_prefix('-').unwrap_or(text);
    let (mantissa, exponent) = match text.split_once(['e', 'E']) {
        Some((mantissa, exponent)) => (mantissa, exponent.parse::<i64>().ok()?),
        None => (text, 0),
    };
    let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let digits = format!("{whole}{fraction}");
    let significant = digits.trim_start_matches('0').trim_end_matches('0');
    if significant.is_empty() {
        return Some((String::new(), 0));
    }
    let trailing = digits.len() - digits.trim_end_matches('0').len();
    let fraction_len = i64::try_from(fraction.len()).ok()?;
    let trailing = i64::try_from(trailing).ok()?;
    let power = exponent.checked_sub(fraction_len)?.checked_add(trailing)?;
    Some((significant.to_string(), power))
}

#[cfg(test)]
mod tests {
    use super::{DecodeError, DecodeLimits, decode};
    use serde_json::json;

    const LIMITS: DecodeLimits = DecodeLimits {
        bytes: 1024,
        depth: 3,
        elements: 8,
        lossless_numbers: true,
    };

    #[test]
    fn a_document_within_the_limits_keeps_its_values() {
        let value = decode(r#" {"a":[1,-2.5,"A"],"b":null} "#, LIMITS).unwrap();
        assert_eq!(value, json!({"a":[1,-2.5,"A"],"b":null}));
    }

    #[test]
    fn a_repeated_key_is_refused_also_when_escaped_or_nested() {
        for raw in [
            r#"{"a":1,"a":1}"#,
            "{\"a\":1,\"\\u0061\":2}",
            r#"{"b":{"a":1,"a":2}}"#,
        ] {
            assert_eq!(decode(raw, LIMITS), Err(DecodeError::DuplicateKey), "{raw}");
        }
    }

    #[test]
    fn what_a_repair_would_accept_is_refused() {
        for raw in [
            "{'a':1}",
            r#"{"a":1,}"#,
            "{a:1}",
            r#"{"a":1"#,
            "[1,]",
            "",
            "tru",
            "01",
            r#""\ud800""#,
        ] {
            assert_eq!(decode(raw, LIMITS), Err(DecodeError::Malformed), "{raw}");
        }
        assert_eq!(
            decode(r#"{"a":1}{"b":2}"#, LIMITS),
            Err(DecodeError::TrailingData)
        );
        assert_eq!(
            decode(r#"{"a":1} garbage"#, LIMITS),
            Err(DecodeError::TrailingData)
        );
    }

    #[test]
    fn each_limit_holds_at_its_edge() {
        assert!(decode(r#"{"a":{"b":[]}}"#, LIMITS).is_ok());
        assert_eq!(
            decode(r#"{"a":{"b":[[]]}}"#, LIMITS),
            Err(DecodeError::TooDeep)
        );
        assert!(decode("[1,2,3,4,5,6,7]", LIMITS).is_ok());
        assert_eq!(
            decode("[1,2,3,4,5,6,7,8]", LIMITS),
            Err(DecodeError::TooManyElements)
        );
        let exact = DecodeLimits { bytes: 2, ..LIMITS };
        assert!(decode("{}", exact).is_ok());
        assert_eq!(decode("{ }", exact), Err(DecodeError::TooLarge));
    }

    #[test]
    fn a_number_an_f64_would_change_is_refused() {
        for raw in [
            "18446744073709551616",
            "-0",
            "1e-400",
            "1.000000000000000001",
            "1.0000000000000001",
            "0.30000000000000000001",
        ] {
            assert_eq!(decode(raw, LIMITS), Err(DecodeError::LossyNumber), "{raw}");
        }
        for raw in [
            "18446744073709551615",
            "-9223372036854775808",
            "0.0",
            "-0.0",
            "1e3",
            "1.50",
            "1.5e0",
            "0.1",
            "-2.5e-3",
            "100.0",
            "0.30000000000000004",
            "1.0000000000000002",
        ] {
            assert!(decode(raw, LIMITS).is_ok(), "{raw}");
        }
    }
}
