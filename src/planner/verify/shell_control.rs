#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Quote {
    Unquoted,
    Single,
    Double,
}

pub(super) const UNQUOTED_ESCAPE_REASON: &str =
    "verify command uses an unquoted shell escape outside the deterministic verify grammar";

pub(super) fn contains(command: &str) -> bool {
    let bytes = command.as_bytes();
    let mut quote = Quote::Unquoted;
    let mut index = 0usize;
    while index < bytes.len() {
        match quote {
            Quote::Unquoted => match bytes[index] {
                b'\\' => index += escaped_width(bytes, index),
                b'\'' => {
                    quote = Quote::Single;
                    index += 1;
                }
                b'"' => {
                    quote = Quote::Double;
                    index += 1;
                }
                b'$' if bytes.get(index + 1) == Some(&b'(') => return true,
                b';' | b'&' | b'|' | b'<' | b'>' | b'`' | b'\n' | b'\r' => return true,
                _ => index += 1,
            },
            Quote::Single => {
                if bytes[index] == b'\'' {
                    quote = Quote::Unquoted;
                }
                index += 1;
            }
            Quote::Double => match bytes[index] {
                b'"' => {
                    quote = Quote::Unquoted;
                    index += 1;
                }
                b'\\' if double_quote_escape(bytes.get(index + 1).copied()) => {
                    index += escaped_width(bytes, index);
                }
                b'$' if bytes.get(index + 1) == Some(&b'(') => return true,
                b'`' => return true,
                _ => index += 1,
            },
        }
    }
    false
}

pub(super) fn has_unquoted_escape(command: &str) -> bool {
    let bytes = command.as_bytes();
    let mut quote = Quote::Unquoted;
    let mut index = 0usize;
    while index < bytes.len() {
        match quote {
            Quote::Unquoted => match bytes[index] {
                b'\\' => return true,
                b'\'' => {
                    quote = Quote::Single;
                    index += 1;
                }
                b'"' => {
                    quote = Quote::Double;
                    index += 1;
                }
                _ => index += 1,
            },
            Quote::Single => {
                if bytes[index] == b'\'' {
                    quote = Quote::Unquoted;
                }
                index += 1;
            }
            Quote::Double => match bytes[index] {
                b'"' => {
                    quote = Quote::Unquoted;
                    index += 1;
                }
                b'\\' if double_quote_escape(bytes.get(index + 1).copied()) => {
                    index += escaped_width(bytes, index);
                }
                _ => index += 1,
            },
        }
    }
    false
}

fn escaped_width(bytes: &[u8], index: usize) -> usize {
    if index + 1 < bytes.len() { 2 } else { 1 }
}

fn double_quote_escape(next: Option<u8>) -> bool {
    next.is_some_and(|byte| matches!(byte, b'$' | b'`' | b'"' | b'\\' | b'\n'))
}

#[cfg(test)]
mod tests {
    use super::*;

    const RUN6_REJECTION: &str = include_str!(
        "../../../tests/corpus/apps/test0716_data7b_quoted_lint/fixtures/run6-runtime-rejection.jsonl"
    );

    #[test]
    fn measured_run6_python_payload_is_one_control_free_command() {
        let event: serde_json::Value = serde_json::from_str(RUN6_REJECTION.trim()).unwrap();
        let command = event["original_command"].as_str().unwrap();

        assert!(!contains(command));
    }

    #[test]
    fn controls_inside_quotes_are_literal_but_substitutions_remain_active() {
        assert!(!contains(r#"python3 -c "a=1; b='x|y&&z||q&r'""#));
        assert!(!contains(r#"node -e 'const x="a;b|c&&d||e"'"#));
        assert!(!contains(r#"python3 -c "print(\"safe;value\")""#));
        assert!(!contains(r#"python3 -c "print(\$(literal))""#));
        assert!(contains(r#"python3 -c "print($(whoami))""#));
        assert!(contains(r#"python3 -c "print(`whoami`)""#));
    }

    #[test]
    fn every_unquoted_control_family_remains_detected() {
        for command in [
            "python -c \"print('ok')\"; echo bad",
            "npm test && npm run build",
            "npm test || echo bad",
            "cargo test | cat",
            "cargo test & echo bad",
            "cargo test > out.log",
            "cat < input.txt",
        ] {
            assert!(contains(command), "{command}");
        }
    }

    #[test]
    fn sh_escapes_prevent_operator_interpretation() {
        assert!(!contains(r"printf \;"));
        assert!(!contains(r"printf \|"));
        assert!(!contains(r"printf \&"));
        assert!(has_unquoted_escape(r"printf \;"));
        assert!(!has_unquoted_escape(r#"python -c "print(\"x\")""#));
    }
}
