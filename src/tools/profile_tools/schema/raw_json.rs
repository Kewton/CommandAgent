//! The bounded JSON decode of the G2 protocol. A `serde_json::Value` has already
//! lost a repeated key and the excess digits of a number, so this reads the raw
//! text. It accepts RFC 8259 JSON and nothing else: no repair, no second parse.

use serde_json::{Map, Value};

use super::{SizeDetail, UNIT_BYTES, UNIT_ELEMENTS, UNIT_LEVELS};

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
    decode_measured(raw, limits).map_err(|(error, _)| error)
}

/// [`decode`], with the limit and the measured value a size refusal reports
/// (Issue #1087). The decoder stops at the first value past a limit, so its count
/// and depth are floors and are reported as such; the byte count is the whole raw
/// text and is exact.
pub(crate) fn decode_measured(
    raw: &str,
    limits: DecodeLimits,
) -> Result<Value, (DecodeError, Option<SizeDetail>)> {
    if raw.len() > limits.bytes {
        return Err((
            DecodeError::TooLarge,
            Some(SizeDetail::exact(raw.len(), limits.bytes, UNIT_BYTES)),
        ));
    }
    let mut decoder = Decoder {
        raw,
        bytes: raw.as_bytes(),
        position: 0,
        elements: 0,
        detail: None,
        limits,
    };
    decoder.skip_whitespace();
    let value = match decoder.value(0) {
        Ok(value) => value,
        Err(error) => return Err((error, decoder.detail)),
    };
    decoder.skip_whitespace();
    if decoder.position != decoder.bytes.len() {
        return Err((DecodeError::TrailingData, None));
    }
    Ok(value)
}

struct Decoder<'a> {
    raw: &'a str,
    bytes: &'a [u8],
    position: usize,
    elements: usize,
    /// Set at the first size refusal, so `decode_measured` can report it.
    detail: Option<SizeDetail>,
    limits: DecodeLimits,
}

/// Whether a byte can begin a JSON value. `]`, `}`, `,` and end of input cannot,
/// so a trailing comma or a run-off stays malformed rather than counting a value.
fn starts_value(byte: u8) -> bool {
    matches!(
        byte,
        b'{' | b'[' | b'"' | b't' | b'f' | b'n' | b'-' | b'0'..=b'9'
    )
}

/// The RFC 8259 form of a number token — `[ minus ] int [ frac ] [ exp ]` — decided on
/// its bytes alone. It checks the grammar, never the value: neither the exponent's
/// magnitude nor the digits' count is bounded here, because converting a value and
/// checking its range are the decode's business (Issue #1100).
fn is_number_form(token: &str) -> bool {
    let bytes = token.as_bytes();
    let mut index = 0;
    if bytes.get(index) == Some(&b'-') {
        index += 1;
    }
    match bytes.get(index).copied() {
        Some(b'0') => index += 1,
        Some(b'1'..=b'9') => {
            index += 1;
            while matches!(bytes.get(index).copied(), Some(b'0'..=b'9')) {
                index += 1;
            }
        }
        _ => return false,
    }
    if bytes.get(index) == Some(&b'.') {
        index += 1;
        if !matches!(bytes.get(index).copied(), Some(b'0'..=b'9')) {
            return false;
        }
        while matches!(bytes.get(index).copied(), Some(b'0'..=b'9')) {
            index += 1;
        }
    }
    if matches!(bytes.get(index).copied(), Some(b'e' | b'E')) {
        index += 1;
        if matches!(bytes.get(index).copied(), Some(b'+' | b'-')) {
            index += 1;
        }
        if !matches!(bytes.get(index).copied(), Some(b'0'..=b'9')) {
            return false;
        }
        while matches!(bytes.get(index).copied(), Some(b'0'..=b'9')) {
            index += 1;
        }
    }
    index == bytes.len()
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

    /// Whether the candidate the element limit stopped the walk at is confirmed as a
    /// value (Issue #1087 review fix 3). A container must be whole, grammatically
    /// complete, and within the depth limit (Issue #1101 review fix 1); a scalar must be
    /// one whole token, so `n]`, `-]`, `1e]` and an unterminated string are not. Either
    /// kind also needs a delimiter after it, so `nullx`, `0x` and `truex` are not values
    /// (Issue #1101). `depth` is the depth of the container the candidate sits in. This
    /// only looks: it moves no position and adds no count.
    fn confirms_value(&self, byte: u8, depth: usize) -> bool {
        let end = match byte {
            b'{' | b'[' => self.confirm_value(self.position, depth),
            b'"' => self.confirm_string(self.position),
            b't' => self.confirm_literal(self.position, b"true"),
            b'f' => self.confirm_literal(self.position, b"false"),
            b'n' => self.confirm_literal(self.position, b"null"),
            _ => self.confirm_number(self.position),
        };
        end.is_some_and(|end| self.delimits_value(self.bytes.get(end).copied()))
    }

    /// Whether what follows a value ends it: whitespace, `,`, a closing bracket, or the
    /// end of input. A scalar that runs on into another token is not the value its
    /// prefix spells.
    fn delimits_value(&self, next: Option<u8>) -> bool {
        matches!(
            next,
            None | Some(b' ' | b'\t' | b'\n' | b'\r' | b',' | b'}' | b']')
        )
    }

    /// The index after the JSON value that starts at `start`, if it is complete as JSON
    /// and stays within the depth limit; `None` otherwise (Issue #1101 review fix 1).
    /// `outer_depth` is the depth of the container the candidate sits in, so the value's
    /// own containers are deeper. No value is built: a number is judged by its lexical
    /// form (Issue #1100), and a string by the escapes `serde_json` accepts.
    fn confirm_value(&self, start: usize, outer_depth: usize) -> Option<usize> {
        match self.bytes.get(start).copied()? {
            b'"' => self.confirm_string(start),
            b't' => self.confirm_literal(start, b"true"),
            b'f' => self.confirm_literal(start, b"false"),
            b'n' => self.confirm_literal(start, b"null"),
            b'-' | b'0'..=b'9' => self.confirm_number(start),
            b'[' => self.confirm_array(start, outer_depth),
            b'{' => self.confirm_object(start, outer_depth),
            _ => None,
        }
    }

    /// The index of the first byte after `index` that is not whitespace.
    fn skip_spaces(&self, mut index: usize) -> usize {
        while matches!(
            self.bytes.get(index).copied(),
            Some(b' ' | b'\t' | b'\n' | b'\r')
        ) {
            index += 1;
        }
        index
    }

    /// The index after a complete array, its elements and nested containers included.
    /// The container is one level deeper than `outer_depth`; a candidate that reaches
    /// past the depth limit is not confirmed.
    fn confirm_array(&self, start: usize, outer_depth: usize) -> Option<usize> {
        let level = outer_depth + 1;
        if level > self.limits.depth {
            return None;
        }
        let mut end = self.skip_spaces(start + 1);
        if self.bytes.get(end) == Some(&b']') {
            return Some(end + 1);
        }
        loop {
            end = self.confirm_value(end, level)?;
            end = self.skip_spaces(end);
            match self.bytes.get(end).copied()? {
                b',' => end = self.skip_spaces(end + 1),
                b']' => return Some(end + 1),
                _ => return None,
            }
        }
    }

    /// The index after a complete object: a quoted key, its colon, its value, and its
    /// separators. Duplicate keys are not judged here, because the walk never enters the
    /// candidate; only its grammar and its depth are.
    fn confirm_object(&self, start: usize, outer_depth: usize) -> Option<usize> {
        let level = outer_depth + 1;
        if level > self.limits.depth {
            return None;
        }
        let mut end = self.skip_spaces(start + 1);
        if self.bytes.get(end) == Some(&b'}') {
            return Some(end + 1);
        }
        loop {
            if self.bytes.get(end) != Some(&b'"') {
                return None;
            }
            end = self.confirm_string(end)?;
            end = self.skip_spaces(end);
            if self.bytes.get(end) != Some(&b':') {
                return None;
            }
            end = self.skip_spaces(end + 1);
            end = self.confirm_value(end, level)?;
            end = self.skip_spaces(end);
            match self.bytes.get(end).copied()? {
                b',' => end = self.skip_spaces(end + 1),
                b'}' => return Some(end + 1),
                _ => return None,
            }
        }
    }

    /// The index after a complete string token, if `serde_json` accepts its escapes;
    /// `None` for a run-off or an invalid escape such as `"\q"`. The delimiter after it
    /// is the caller's business.
    fn confirm_string(&self, start: usize) -> Option<usize> {
        let mut end = start + 1;
        loop {
            match self.bytes.get(end).copied()? {
                b'"' => {
                    end += 1;
                    break;
                }
                b'\\' => end += 2,
                _ => end += 1,
            }
        }
        self.raw
            .get(start..end)
            .filter(|token| serde_json::from_str::<String>(token).is_ok())
            .map(|_| end)
    }

    /// The index after a literal token, if `start` spells its whole word.
    fn confirm_literal(&self, start: usize, text: &[u8]) -> Option<usize> {
        let end = start + text.len();
        (self.bytes.get(start..end) == Some(text)).then_some(end)
    }

    /// The index after a number token's run. The form is checked on the bytes rather
    /// than by conversion, so a number outside the `serde_json` range such as `1e400` is
    /// still a value (Issue #1100); its delimiter is the caller's business.
    fn confirm_number(&self, start: usize) -> Option<usize> {
        let mut end = start;
        while matches!(
            self.bytes.get(end).copied(),
            Some(b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9')
        ) {
            end += 1;
        }
        self.raw
            .get(start..end)
            .filter(|token| is_number_form(token))
            .map(|_| end)
    }

    fn value(&mut self, depth: usize) -> Result<Value, DecodeError> {
        // Confirm a value starts here before counting it. End of input and a
        // trailing comma's `]` or `}` are malformed, not a value past the limit,
        // so the reported count never exceeds the values this walk confirmed.
        let byte = self.peek().ok_or(DecodeError::Malformed)?;
        if !starts_value(byte) {
            return Err(DecodeError::Malformed);
        }
        let counted = self.elements + 1;
        if counted > self.limits.elements {
            // The walk stops at the first candidate past the element limit, so its
            // count is a floor. Count the candidate only when it is confirmed as a
            // value here: a container that is grammatically complete and within the
            // depth limit, or a scalar whose own token this decoder can check without
            // reading past it. A run-off such as `n]`, `-]`, `1e]`, an unterminated
            // string, a malformed container or one deeper than the limit is not a value,
            // so it is not counted and the floor stays at the count of values the walk
            // confirmed.
            let measured = if self.confirms_value(byte, depth) {
                counted
            } else {
                self.elements
            };
            self.detail = Some(SizeDetail::floor(
                measured,
                self.limits.elements,
                UNIT_ELEMENTS,
            ));
            return Err(DecodeError::TooManyElements);
        }
        self.elements = counted;
        match byte {
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
            self.detail = Some(SizeDetail::floor(depth, self.limits.depth, UNIT_LEVELS));
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
            self.detail = Some(SizeDetail::floor(depth, self.limits.depth, UNIT_LEVELS));
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
        if !self.limits.lossless_numbers {
            return Ok(Value::Number(number));
        }
        exact_number(token, number)
            .map(Value::Number)
            .ok_or(DecodeError::LossyNumber)
    }
}

/// `serde_json` has already accepted the grammar. An integer token must stay an
/// integer. A fractional token is rounded by the standard library, because
/// `serde_json` without `float_roundtrip` can land one step off, and must read back,
/// digit for digit, from the shortest decimal of that value: no count of digits can
/// promise that, because a decimal is only certain to survive up to 15 of them. The
/// value compared is the value kept.
fn exact_number(token: &str, number: serde_json::Number) -> Option<serde_json::Number> {
    if !token.contains(['.', 'e', 'E']) {
        return (number.is_i64() || number.is_u64()).then_some(number);
    }
    let value = token.parse::<f64>().ok()?;
    let kept = decimal_value(&format!("{value:e}"))?;
    if decimal_value(token)? != kept {
        return None;
    }
    serde_json::Number::from_f64(value)
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

    /// Issue #1087: the raw decode reports the limit and the measured value for each
    /// size refusal. A count and a depth the walk stopped at are floors, so they say
    /// `at least`; the byte count is the whole text and is exact.
    #[test]
    fn a_size_refusal_reports_the_limit_and_the_measured_value() {
        use super::super::{SizeDetail, UNIT_BYTES, UNIT_ELEMENTS, UNIT_LEVELS};
        use super::decode_measured;

        for (raw, error, detail) in [
            (
                r#"{"a":{"b":[[]]}}"#,
                DecodeError::TooDeep,
                SizeDetail::floor(4, 3, UNIT_LEVELS),
            ),
            (
                "[1,2,3,4,5,6,7,8]",
                DecodeError::TooManyElements,
                SizeDetail::floor(9, 8, UNIT_ELEMENTS),
            ),
        ] {
            let (code, measured) = decode_measured(raw, LIMITS).unwrap_err();
            assert_eq!(code, error, "{raw}");
            assert_eq!(measured, Some(detail), "{raw}");
        }
        let exact = DecodeLimits { bytes: 2, ..LIMITS };
        let (code, measured) = decode_measured("{ }", exact).unwrap_err();
        assert_eq!(code, DecodeError::TooLarge);
        assert_eq!(measured, Some(SizeDetail::exact(3, 2, UNIT_BYTES)));
        // A refusal that is not about size carries no numbers.
        let (code, measured) = decode_measured("{'a':1}", LIMITS).unwrap_err();
        assert_eq!(code, DecodeError::Malformed);
        assert_eq!(measured, None);
    }

    /// Issue #1087 review fix 1: at exactly the limit, a trailing comma or a run-off is
    /// malformed, not a value past the limit. Before the fix the count added the position
    /// it failed to read, so it reported `at least 9 elements, limit 8` with no 9th value.
    #[test]
    fn a_trailing_comma_or_ended_input_at_the_limit_is_malformed_not_a_count() {
        use super::super::{SizeDetail, UNIT_ELEMENTS};
        use super::decode_measured;

        // `LIMITS.elements` is 8: an array plus seven items, or an object plus seven members,
        // fills it. The position after the trailing comma, and the end of input, are not a
        // value, so neither may be counted.
        for raw in [
            "[1,2,3,4,5,6,7,]",
            "[1,2,3,4,5,6,7,   ]",
            "[1,2,3,4,5,6,7",
            r#"{"a":1,"b":2,"c":3,"d":4,"e":5,"f":6,"g":7,}"#,
            r#"{"a":1,"b":2,"c":3,"d":4,"e":5,"f":6,"g":7"#,
        ] {
            let (code, measured) = decode_measured(raw, LIMITS).unwrap_err();
            assert_eq!(code, DecodeError::Malformed, "{raw}");
            assert_eq!(measured, None, "{raw}");
        }
        // The same documents without the trailing comma or run-off are accepted, unchanged.
        assert!(decode("[1,2,3,4,5,6,7]", LIMITS).is_ok());
        assert!(decode(r#"{"a":1,"b":2,"c":3,"d":4,"e":5,"f":6,"g":7}"#, LIMITS).is_ok());
        // A genuine eighth item is still past the limit and is counted, as a floor.
        let (code, measured) = decode_measured("[1,2,3,4,5,6,7,8]", LIMITS).unwrap_err();
        assert_eq!(code, DecodeError::TooManyElements);
        assert_eq!(measured, Some(SizeDetail::floor(9, 8, UNIT_ELEMENTS)));
    }

    /// Issue #1087 review fix 3: the candidate the element limit stops the walk at is
    /// counted as a floor only when it is confirmed as a value. A run-off is not a value
    /// (`n]`, `-]`, `1e]`, an unterminated string), so the floor stays at the values the
    /// walk confirmed instead of inventing one more.
    #[test]
    fn only_a_confirmed_value_past_the_element_limit_is_counted() {
        use super::super::{SizeDetail, UNIT_ELEMENTS};
        use super::decode_measured;

        // `LIMITS.elements` is 8: the root, the array, and six items fill it. The next
        // item is the candidate the limit stops at.
        let at_limit = r#"{"a":[1,1,1,1,1,1"#;
        for ending in [r#",n]}"#, r#",-]}"#, r#",1e]}"#, r#","abc]}"#] {
            let raw = format!("{at_limit}{ending}");
            let (code, measured) = decode_measured(&raw, LIMITS).unwrap_err();
            assert_eq!(code, DecodeError::TooManyElements, "{raw}");
            assert_eq!(
                measured,
                Some(SizeDetail::floor(8, 8, UNIT_ELEMENTS)),
                "{raw}"
            );
        }
        // A candidate that is a value is still counted, as before.
        for ending in [r#",1]}"#, r#",{}]}"#] {
            let raw = format!("{at_limit}{ending}");
            let (code, measured) = decode_measured(&raw, LIMITS).unwrap_err();
            assert_eq!(code, DecodeError::TooManyElements, "{raw}");
            assert_eq!(
                measured,
                Some(SizeDetail::floor(9, 8, UNIT_ELEMENTS)),
                "{raw}"
            );
        }
        // The limit exactly, an object at the limit, empty containers, and valid nesting
        // are accepted, unchanged.
        for raw in [
            r#"{"a":[1,1,1,1,1,1]}"#,
            r#"{"a":1,"b":2,"c":3,"d":4,"e":5,"f":6,"g":7}"#,
            "{}",
            "[]",
            r#"{"a":[]}"#,
            r#"{"a":{"b":[]}}"#,
        ] {
            assert!(decode(raw, LIMITS).is_ok(), "{raw}");
        }
    }

    /// Issue #1100: at the element limit the candidate number is confirmed by its JSON
    /// lexical form on the raw bytes, not by a conversion, so `1e400` — a number no
    /// `f64` holds — is counted as the value past the limit, while an incomplete `1e` is
    /// not. A value within the limit is still converted and range-checked as before.
    #[test]
    fn a_number_outside_the_serde_json_range_is_counted_when_it_is_the_value_past_the_limit() {
        use super::super::{SizeDetail, UNIT_ELEMENTS};
        use super::decode_measured;

        // `LIMITS.elements` is 8: the root, the array, and six items fill it. The next
        // token is the candidate the limit stops at.
        let at_limit = r#"{"a":[1,1,1,1,1,1"#;
        for ending in [r#",1e400]}"#, r#",1e+400]}"#, r#",-1.5e400]}"#] {
            let raw = format!("{at_limit}{ending}");
            let (code, measured) = decode_measured(&raw, LIMITS).unwrap_err();
            assert_eq!(code, DecodeError::TooManyElements, "{raw}");
            assert_eq!(
                measured,
                Some(SizeDetail::floor(9, 8, UNIT_ELEMENTS)),
                "{raw}"
            );
        }
        for ending in [r#",1e]}"#, r#",1e+]}"#, r#",1.]}"#, r#",01]}"#] {
            let raw = format!("{at_limit}{ending}");
            let (code, measured) = decode_measured(&raw, LIMITS).unwrap_err();
            assert_eq!(code, DecodeError::TooManyElements, "{raw}");
            assert_eq!(
                measured,
                Some(SizeDetail::floor(8, 8, UNIT_ELEMENTS)),
                "{raw}"
            );
        }
        // Within the limit the value is still converted: `1e400` has no `serde_json`
        // number, so it is refused as before. Only the count at the limit reads the form.
        assert_eq!(decode("1e400", LIMITS), Err(DecodeError::Malformed));
    }

    /// Issue #1101: a container is confirmed by its completed bracket pair, not by its
    /// opener, and a scalar needs a delimiter after it. `…,[` at the end of input and
    /// `…,{]` are not values, and neither is `nullx`, `0x` or `truex`; a scalar or an
    /// empty container that really is the 513th value is still counted.
    #[test]
    fn only_a_completed_container_or_a_delimited_scalar_past_the_element_limit_is_counted() {
        use super::super::{SizeDetail, UNIT_ELEMENTS};
        use super::decode_measured;

        // `LIMITS.elements` is 8: the root, the array, and six items fill it. The next
        // candidate is the one the limit stops at.
        let at_limit = r#"{"a":[1,1,1,1,1,1"#;
        // Not a value: an opening bracket alone, a mismatched close, a container whose
        // grammar is broken, one deeper than the limit, and scalars that run on past
        // their own token.
        for ending in [
            r#",["#,
            r#",{]"#,
            r#",nullx]}"#,
            r#",truex]}"#,
            r#",falsex]}"#,
            r#",0x]}"#,
            r#",1.5x]}"#,
            r#","abc"x]}"#,
            r#",[1,,2]}"#,
            r#",{"a"}]}"#,
            r#",["\q"]]}"#,
            r#",[[]]}"#,
        ] {
            let raw = format!("{at_limit}{ending}");
            let (code, measured) = decode_measured(&raw, LIMITS).unwrap_err();
            assert_eq!(code, DecodeError::TooManyElements, "{raw}");
            assert_eq!(
                measured,
                Some(SizeDetail::floor(8, 8, UNIT_ELEMENTS)),
                "{raw}"
            );
        }
        // A value: a scalar with its delimiter, an empty object, and an empty array at
        // the depth limit.
        for ending in [
            r#",1]}"#,
            r#",null]}"#,
            r#",true]}"#,
            r#","abc"]}"#,
            r#",{}]}"#,
            r#",[]]}"#,
        ] {
            let raw = format!("{at_limit}{ending}");
            let (code, measured) = decode_measured(&raw, LIMITS).unwrap_err();
            assert_eq!(code, DecodeError::TooManyElements, "{raw}");
            assert_eq!(
                measured,
                Some(SizeDetail::floor(9, 8, UNIT_ELEMENTS)),
                "{raw}"
            );
        }
        // Whitespace delimits a scalar too.
        for ending in [",1 }", ",null ]", ",true\n"] {
            let raw = format!("{at_limit}{ending}");
            let (code, measured) = decode_measured(&raw, LIMITS).unwrap_err();
            assert_eq!(code, DecodeError::TooManyElements, "{raw}");
            assert_eq!(
                measured,
                Some(SizeDetail::floor(9, 8, UNIT_ELEMENTS)),
                "{raw}"
            );
        }
        // Exactly the limit and valid JSON are accepted, unchanged.
        for raw in [
            r#"{"a":[1,1,1,1,1,1]}"#,
            "{}",
            "[]",
            r#"{"a":[]}"#,
            r#"{"a":[1,2,3]}"#,
        ] {
            assert!(decode(raw, LIMITS).is_ok(), "{raw}");
        }
    }

    /// Issue #1101 review fix 1, blocker 1: the candidate container is confirmed by its
    /// whole grammar, not by its brackets alone. A doubled comma, an object without a
    /// colon, and an invalid escape inside a string are not values, so the floor stays
    /// at 512; a well-formed candidate is still 513.
    #[test]
    fn a_malformed_candidate_container_past_the_element_limit_is_not_counted() {
        use super::super::{SizeDetail, UNIT_ELEMENTS};
        use super::decode_measured;

        // `LIMITS.elements` is 8: the root, the array, and six items fill it.
        let at_limit = r#"{"a":[1,1,1,1,1,1"#;
        for ending in [
            r#",[1,,2]]}"#,
            r#",[1,2,]]}"#,
            r#",{"a"}]}"#,
            r#",{"a":}]}"#,
            r#",{"a":1,}]}"#,
            r#",["\q"]]}"#,
            r#",["a" "b"]]}"#,
        ] {
            let raw = format!("{at_limit}{ending}");
            let (code, measured) = decode_measured(&raw, LIMITS).unwrap_err();
            assert_eq!(code, DecodeError::TooManyElements, "{raw}");
            assert_eq!(
                measured,
                Some(SizeDetail::floor(8, 8, UNIT_ELEMENTS)),
                "{raw}"
            );
        }
        // A well-formed candidate is still the 513th value.
        for ending in [r#",[1,2]]}"#, r#",{"a":1}]}"#, r#",[]]}"#, r#",{}]}"#] {
            let raw = format!("{at_limit}{ending}");
            let (code, measured) = decode_measured(&raw, LIMITS).unwrap_err();
            assert_eq!(code, DecodeError::TooManyElements, "{raw}");
            assert_eq!(
                measured,
                Some(SizeDetail::floor(9, 8, UNIT_ELEMENTS)),
                "{raw}"
            );
        }
    }

    /// Issue #1101 review fix 1, blocker 2: the depth the walk has reached is counted in
    /// when the candidate is confirmed, so a candidate that nests past the depth limit is
    /// not a value while one whose deepest container sits exactly at the limit is.
    #[test]
    fn a_candidate_container_deeper_than_the_depth_limit_past_the_element_limit_is_not_counted() {
        use super::super::{SizeDetail, UNIT_ELEMENTS};
        use super::decode_measured;

        // A depth limit of 5: the root object is 1, its array is 2, so a candidate's own
        // containers start at 3.
        let limits = DecodeLimits { depth: 5, ..LIMITS };
        let at_limit = r#"{"a":[1,1,1,1,1,1"#;
        // Three nested arrays take the whole document to depth 5, exactly the limit.
        let at_depth = format!("{at_limit},[[[]]]}}");
        let (code, measured) = decode_measured(&at_depth, limits).unwrap_err();
        assert_eq!(code, DecodeError::TooManyElements, "{at_depth}");
        assert_eq!(measured, Some(SizeDetail::floor(9, 8, UNIT_ELEMENTS)));
        // One more level passes it, and the candidate is not a value.
        let over_depth = format!("{at_limit},[[[[]]]]}}");
        let (code, measured) = decode_measured(&over_depth, limits).unwrap_err();
        assert_eq!(code, DecodeError::TooManyElements, "{over_depth}");
        assert_eq!(measured, Some(SizeDetail::floor(8, 8, UNIT_ELEMENTS)));
        // An unterminated deep run-off is not a value either, and it is not read far
        // enough to be a stack cost: confirmation stops at the depth limit.
        let run_off = format!("{at_limit},{}", "[".repeat(500));
        let (code, measured) = decode_measured(&run_off, limits).unwrap_err();
        assert_eq!(code, DecodeError::TooManyElements, "deep run-off");
        assert_eq!(measured, Some(SizeDetail::floor(8, 8, UNIT_ELEMENTS)));
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
            "9007199254740993.0",
        ] {
            assert_eq!(decode(raw, LIMITS), Err(DecodeError::LossyNumber), "{raw}");
        }
        assert_eq!(decode("1e400", LIMITS), Err(DecodeError::Malformed));
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

    /// A correctly rounded double keeps its value, also where `serde_json`'s fast path
    /// would land one step off. The expectation is the standard library's parse.
    #[test]
    fn a_correctly_rounded_double_passes_as_that_value() {
        let failures = [
            "14.700000000000001",
            "0.9242829794947799",
            "398.91571404865124",
            "1.602176634e-19",
            "5e-324",
            "1.7976931348623157e308",
            "3.141592653589793",
            "1e23",
        ]
        .into_iter()
        .filter(|raw| {
            decode(raw, LIMITS)
                .ok()
                .and_then(|value| value.as_f64())
                .is_none_or(|value| Some(value) != raw.parse::<f64>().ok())
        })
        .collect::<Vec<_>>();
        assert_eq!(failures, Vec::<&str>::new());
    }
}
