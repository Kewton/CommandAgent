//! Refuse a process's output when it holds a secret this run holds (Issue #974, G3).
//!
//! A tool can read this user's other processes (see the known limits in
//! `docs/dev/profile-tools.md`), and its output reaches events, the workspace, and the model.
//! The existing scrub replaces exact text in what is recorded. This judges the raw bytes before
//! any of that, in the forms that are cheap to make by accident or on purpose: the bytes as they
//! are, JSON-escaped, base64 in either alphabet at any of its three alignments and with or
//! without padding, and hex in either case.
//!
//! This refuses a finite list of forms. Encryption, splitting, compression, or any other form is
//! not recognized, and this is not a boundary against an executable that means harm.

use std::collections::BTreeSet;
use std::fmt;

use serde_json::Value;

use super::protocol::{decode_base64, decode_document, encode_base64};

/// The forms of this run's secrets. It shows how many there are and nothing about them.
pub(super) struct SecretForms {
    forms: BTreeSet<Vec<u8>>,
}

impl fmt::Debug for SecretForms {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SecretForms")
            .field("count", &self.forms.len())
            .finish_non_exhaustive()
    }
}

/// The part of `value`'s base64 that does not depend on the bytes around it, for one alignment:
/// `prefix` bytes come before it in the encoded run. It holds whatever follows, so this finds a
/// padded encoding, an unpadded one, and one with other bytes on either side.
fn base64_core(value: &[u8], prefix: usize) -> Vec<u8> {
    let mut aligned = vec![0u8; prefix];
    aligned.extend_from_slice(value);
    let encoded = encode_base64(&aligned);
    let encoded = encoded.trim_end_matches('=').as_bytes();
    // The first characters that hold any bits of the prefix bytes.
    let start = [0, 2, 3][prefix];
    // The last character holds bits of whatever follows, unless the value ends on a boundary.
    let end = encoded.len() - usize::from(!aligned.len().is_multiple_of(3));
    encoded[start..end].to_vec()
}

fn url_safe(standard: &[u8]) -> Vec<u8> {
    standard
        .iter()
        .map(|byte| match byte {
            b'+' => b'-',
            b'/' => b'_',
            other => *other,
        })
        .collect()
}

impl SecretForms {
    /// A value too short to be a secret by the catalog's own rule is not one here: it would
    /// refuse ordinary output. The catalog refuses a selected credential that short.
    pub(super) fn new<'a>(values: impl IntoIterator<Item = &'a str>) -> Self {
        let mut forms = BTreeSet::new();
        for value in values {
            for value in [value, value.trim()] {
                if value.chars().count() < crate::sensitive_data::MIN_GENERIC_SECRET_LEN {
                    continue;
                }
                add_forms(&mut forms, value);
            }
        }
        Self { forms }
    }

    /// Byte for byte, so bytes that are not text are judged the same way.
    pub(super) fn found_in(&self, bytes: &[u8]) -> bool {
        self.forms
            .iter()
            .any(|form| bytes.windows(form.len()).any(|window| window == form))
    }

    /// The whole judgement of what one process wrote, whatever else is wrong with it: the raw
    /// streams, then stdout as a document if it is one within the limits of an accepted result.
    pub(super) fn found_in_output(&self, stdout: &[u8], stderr: &[u8]) -> bool {
        self.found_in(stdout)
            || self.found_in(stderr)
            || decode_document(stdout).is_some_and(|document| self.found_in_document(&document))
    }

    /// Any string of a decoded document, keys too: an escape the raw bytes hide is gone here.
    pub(super) fn found_in_document(&self, document: &Value) -> bool {
        match document {
            Value::String(text) => self.found_in_text(text),
            Value::Array(items) => items.iter().any(|item| self.found_in_document(item)),
            Value::Object(fields) => fields
                .iter()
                .any(|(key, item)| self.found_in_text(key) || self.found_in_document(item)),
            _ => false,
        }
    }

    /// A string as it is, and what it decodes to if it is base64. A candidate travels that way,
    /// and a document not yet accepted says nothing reliable about which string is the candidate,
    /// so each is tried. A string that does not decode has no such bytes. What decodes is shorter
    /// than the string, so the document's own limit bounds this.
    fn found_in_text(&self, text: &str) -> bool {
        self.found_in(text.as_bytes())
            || decode_base64(text).is_some_and(|bytes| self.found_in(&bytes))
    }
}

fn add_forms(forms: &mut BTreeSet<Vec<u8>>, value: &str) {
    let bytes = value.as_bytes();
    forms.insert(bytes.to_vec());
    if let Ok(escaped) = serde_json::to_string(value) {
        forms.insert(escaped.as_bytes()[1..escaped.len() - 1].to_vec());
    }
    for prefix in 0..3 {
        let core = base64_core(bytes, prefix);
        forms.insert(url_safe(&core));
        forms.insert(core);
    }
    let hex = bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    forms.insert(hex.to_ascii_uppercase().into_bytes());
    forms.insert(hex.into_bytes());
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::super::protocol::encode_base64;
    use super::SecretForms;

    /// Its base64 holds both `+` and `/`, so the two alphabets really differ.
    const DUMMY: &str = "fake~secret?>>~~??value-974";

    fn forms() -> SecretForms {
        SecretForms::new([DUMMY])
    }

    fn url(text: &str) -> String {
        text.replace('+', "-").replace('/', "_")
    }

    #[test]
    fn the_dummy_really_differs_between_the_alphabets_at_every_alignment() {
        for prefix in ["", "a", "ab"] {
            let encoded = encode_base64(format!("{prefix}{DUMMY}").as_bytes());
            assert_ne!(encoded, url(&encoded), "{prefix:?}");
        }
    }

    #[test]
    fn every_listed_form_is_found_with_other_bytes_around_it() {
        let hex = DUMMY
            .bytes()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let mut outputs = vec![
            DUMMY.to_string(),
            format!("before {DUMMY} after"),
            hex.clone(),
            hex.to_ascii_uppercase(),
            format!("xx{hex}yy"),
        ];
        for prefix in ["", "a", "ab", "abc", "abcd"] {
            for suffix in ["", "z", "zy", "zyx"] {
                let encoded = encode_base64(format!("{prefix}{DUMMY}{suffix}").as_bytes());
                let unpadded = encoded.trim_end_matches('=').to_string();
                outputs.push(url(&encoded));
                outputs.push(url(&unpadded));
                outputs.push(unpadded);
                outputs.push(encoded);
            }
        }
        for output in outputs {
            assert!(forms().found_in(output.as_bytes()), "{output}");
        }
    }

    #[test]
    fn a_json_escaped_value_is_found_raw_and_a_unicode_escape_is_found_once_decoded() {
        let secret = "fake\"secret\\value\n974";
        let forms = SecretForms::new([secret]);
        let escaped = serde_json::to_string(secret).unwrap();
        assert!(forms.found_in(escaped.as_bytes()));
        let hidden = SecretForms::new([DUMMY]);
        let spelled = DUMMY
            .chars()
            .map(|character| format!("\\u{:04x}", character as u32))
            .collect::<String>();
        assert!(!hidden.found_in(spelled.as_bytes()));
        let document: serde_json::Value = serde_json::from_str(&format!(
            r#"{{"diagnostics":[{{"message":"said {spelled}"}}]}}"#
        ))
        .unwrap();
        assert!(hidden.found_in_document(&document));
        assert!(hidden.found_in_document(&json!({ DUMMY: 1 })));
        assert!(!hidden.found_in_document(&json!({"message": ["other", 3, null]})));
    }

    /// JSON spells these differently, so only the raw form can find the value as it is.
    #[test]
    fn a_value_json_would_spell_differently_is_found_as_its_own_bytes() {
        for secret in [
            "fake\"secret-value-974",
            "fake\\secret-value-974",
            "fake\nsecret-value-974",
            "fake\u{1}secret-value-974",
        ] {
            let escaped = serde_json::to_string(secret).unwrap();
            assert_ne!(&escaped[1..escaped.len() - 1], secret);
            let forms = SecretForms::new([secret]);
            assert!(forms.found_in(secret.as_bytes()), "{secret:?}");
            assert!(
                forms.found_in(format!("before {secret} after").as_bytes()),
                "{secret:?}"
            );
        }
    }

    /// What only decoding shows, wherever the string sits and whatever else is in the document.
    #[test]
    fn a_form_inside_a_decoded_string_is_found_and_other_strings_are_not() {
        let forms = forms();
        let hex = DUMMY
            .bytes()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        for inner in [hex, encode_base64(DUMMY.as_bytes())] {
            let wire = encode_base64(inner.as_bytes());
            assert!(!forms.found_in(wire.as_bytes()), "{inner}");
            let document = json!({"outputs": [{"bytes_base64": wire}]});
            assert!(forms.found_in_document(&document), "{inner}");
            let stdout = document.to_string();
            assert!(forms.found_in_output(stdout.as_bytes(), b""), "{inner}");
            assert!(forms.found_in_document(&json!({ wire: 1 })), "{inner}");
            // Not a document, so nothing decodes it: the refusal it gets is another one's.
            assert!(!forms.found_in_output(format!("{stdout}x").as_bytes(), b""));
        }
        for text in ["not base64!", "", "AAAA", "AAA="] {
            assert!(
                !forms.found_in_document(&json!({"outputs": [text]})),
                "{text}"
            );
        }
    }

    #[test]
    fn either_stream_alone_is_enough() {
        let forms = forms();
        assert!(forms.found_in_output(DUMMY.as_bytes(), b""));
        assert!(forms.found_in_output(b"", DUMMY.as_bytes()));
        assert!(!forms.found_in_output(b"{}", b"plain words"));
    }

    #[test]
    fn bytes_that_are_not_text_are_judged_as_bytes() {
        let mut binary = vec![0xff, 0x00, 0xfe];
        binary.extend_from_slice(DUMMY.as_bytes());
        binary.push(0xff);
        assert!(forms().found_in(&binary));
    }

    #[test]
    fn ordinary_output_and_a_value_too_short_to_be_a_secret_are_not_refused() {
        assert!(!forms().found_in(b"{\"mode\":\"fast\"}\n"));
        assert!(!forms().found_in(b""));
        assert!(!forms().found_in(&DUMMY.as_bytes()[..DUMMY.len() - 1]));
        let short = SecretForms::new(["short", ""]);
        assert!(!short.found_in(b"short"));
        assert!(!short.found_in(b""));
    }

    #[test]
    fn a_value_with_space_around_it_is_found_with_and_without_that_space() {
        let spaced = SecretForms::new(["  fake-spaced-value-974\n"]);
        assert!(spaced.found_in(b"fake-spaced-value-974"));
        assert!(spaced.found_in(b"  fake-spaced-value-974\n"));
    }

    #[test]
    fn debug_shows_no_form() {
        let shown = format!("{:?} {:#?}", forms(), forms());
        assert!(!shown.contains("secret"), "{shown}");
        assert!(!shown.contains("forms:"), "{shown}");
    }
}
