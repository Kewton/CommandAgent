use super::super::{Arg, Heredoc, heredoc_bodies, strip_comments_and_heredocs};
use super::unquoted_cat_body_is_literal;

fn arg(word: &str, quoted: bool) -> Arg {
    Arg {
        bytes: word.as_bytes().to_vec(),
        quoted,
    }
}

fn heredoc(delimiter: &str, quoted: bool, dash: bool) -> Heredoc {
    Heredoc {
        delimiter: delimiter.as_bytes().to_vec(),
        quoted,
        dash,
    }
}

fn cat() -> Vec<Arg> {
    vec![arg("cat", false)]
}

/// Bodies a JSON or prose fixture carries that the kept-body reading misreads
/// as paths or quotes: none holds `$`, a backtick, or a backslash.
const INERT_BODIES: &[&str] = &[
    "{\"op\": \"add\", \"path\": \"/b\", \"value\": 2}\n",
    "see https://example.invalid/a/b?c=d#e\n",
    "[{\"path\": \"/a/0\"}, {\"path\": \"/~1x\"}]\n(paren) {brace}\n",
    "it's a \"quoted\" word\nand 'one' more\n",
    "  leading and trailing blanks  \n\n\tTab\n",
    "日本語のテキスト / スラッシュ\n",
    "",
];

#[test]
fn literal_data_accepts_inert_bodies_behind_any_ascii_identifier_delimiter() {
    for delimiter in ["EOF", "END", "data_1", "_x", "Z"] {
        for body in INERT_BODIES {
            assert!(
                unquoted_cat_body_is_literal(
                    &cat(),
                    &[heredoc(delimiter, false, false)],
                    body.as_bytes(),
                    true
                ),
                "delimiter {delimiter:?} body {body:?}"
            );
        }
    }
}

#[test]
fn literal_data_rejects_every_condition_it_cannot_prove() {
    let body = b"{\"path\": \"/b\"}\n";
    let plain = [heredoc("EOF", false, false)];
    // The heredoc shape.
    assert!(!unquoted_cat_body_is_literal(
        &cat(),
        &[heredoc("EOF", true, false)],
        body,
        true
    ));
    assert!(!unquoted_cat_body_is_literal(
        &cat(),
        &[heredoc("EOF", false, true)],
        body,
        true
    ));
    assert!(!unquoted_cat_body_is_literal(&cat(), &[], body, true));
    assert!(!unquoted_cat_body_is_literal(
        &cat(),
        &[heredoc("A", false, false), heredoc("B", false, false)],
        body,
        true
    ));
    // An unterminated body.
    assert!(!unquoted_cat_body_is_literal(&cat(), &plain, body, false));
    // A delimiter that is not an ASCII identifier.
    for delimiter in ["1EOF", "E-OF", "E.OF", "EÖF", "E:F", ""] {
        assert!(
            !unquoted_cat_body_is_literal(&cat(), &[heredoc(delimiter, false, false)], body, true),
            "delimiter {delimiter:?}"
        );
    }
    // The command: only a literal `cat` with no argument.
    for argv in [
        vec![],
        vec![arg("cat", true)],
        vec![arg("cat", false), arg("-", false)],
        vec![arg("cat", false), arg("x", false)],
        vec![arg("tee", false)],
        vec![arg("git", false), arg("commit", false)],
    ] {
        assert!(
            !unquoted_cat_body_is_literal(&argv, &plain, body, true),
            "argv {argv:?}"
        );
    }
    // A body the shell may rewrite, or that is not UTF-8.
    for body in [
        &b"$HOME\n"[..],
        b"cost $5\n",
        b"${x}\n",
        b"$(true)\n",
        b"$((1+1))\n",
        b"`true`\n",
        b"a\\b\n",
        b"trailing \\\n",
        b"\xff\xfe\n",
    ] {
        assert!(
            !unquoted_cat_body_is_literal(&cat(), &plain, body, true),
            "body {body:?}"
        );
    }
}

#[test]
fn literal_data_matches_the_quoted_spelling_view_and_bodies() {
    // The elided view and the raw body list of the plain spelling equal those
    // of the quoted spelling, apart from the delimiter word itself.
    for delimiter in ["EOF", "JSON_1"] {
        for body in INERT_BODIES {
            for redirect in ["", " > out.json", " >> notes.md"] {
                let plain =
                    format!("cat <<{delimiter}{redirect}\n{body}{delimiter}\nwc -c out.json");
                let quoted =
                    format!("cat <<'{delimiter}'{redirect}\n{body}{delimiter}\nwc -c out.json");
                let plain_view = strip_comments_and_heredocs(&plain)
                    .unwrap_or_else(|| panic!("plain must be readable: {plain:?}"));
                let quoted_view = strip_comments_and_heredocs(&quoted)
                    .unwrap_or_else(|| panic!("quoted must be readable: {quoted:?}"));
                assert_eq!(
                    plain_view,
                    format!("cat <<{delimiter}{redirect}\nwc -c out.json"),
                    "plain: {plain:?}"
                );
                assert_eq!(
                    quoted_view.replacen(&format!("'{delimiter}'"), delimiter, 1),
                    plain_view,
                    "quoted: {quoted:?}"
                );
                assert_eq!(
                    heredoc_bodies(&plain),
                    Some(vec![body.to_string()]),
                    "plain bodies: {plain:?}"
                );
                assert_eq!(heredoc_bodies(&plain), heredoc_bodies(&quoted));
            }
        }
    }
}

#[test]
fn literal_data_keeps_the_body_when_it_cannot_prove_it_inert() {
    // Each body holds a write the kept-body reading names; a guard that drops
    // these bodies hides it.
    for command in [
        "cat <<EOF\n$X\ntee sub/link/f\nEOF",
        "cat <<EOF\n${X}\ntee sub/link/f\nEOF",
        "cat <<EOF\ncost $5\ntee sub/link/f\nEOF",
        "cat <<EOF\na\\b\ntee sub/link/f\nEOF",
        "cat <<-EOF\n\ttee sub/link/f\n\tEOF",
        "cat <<EOF\ndata\ntee sub/link/f",
        "cat <<E-F\ntee sub/link/f\nE-F",
        "cat <<A <<B\ntee sub/link/f\nA\nx\nB",
        "cat - <<EOF\ntee sub/link/f\nEOF",
        "tee x <<EOF\ntee sub/link/f\nEOF",
        "cat <<EOF | sh\ntee sub/link/f\nEOF",
        "sh <<EOF\ntee sub/link/f\nEOF",
        "python3 <<EOF\nopen(\"sub/link/f\", \"w\")\nEOF",
        "c\"at\" <<EOF\ntee sub/link/f\nEOF",
        "FOO=1 cat <<EOF\ntee sub/link/f\nEOF",
        "# lead\ncat <<EOF\ntee sub/link/f\nEOF",
        "sh <<'OUTER'\ncat <<INNER\ntee sub/link/f\nINNER\nOUTER",
    ] {
        let elided = strip_comments_and_heredocs(command)
            .unwrap_or_else(|| panic!("command must be readable: {command:?}"));
        assert!(
            elided.contains("sub/link/f"),
            "body must be kept: {command:?} -> {elided:?}"
        );
    }
}

#[test]
fn literal_data_keeps_unreadable_and_execution_refusals() {
    for command in [
        "cat <<EOF\n$(tee sub/link/f)\nEOF",
        "cat <<EOF\n`tee sub/link/f`\nEOF",
        "cat <<EOF\nit's\n$(true)\nEOF",
        "cat <<EOF\nit's",
        "cat <<EOF\r\nit's\nEOF\r\n",
        "cat <<EOF\nit's\nEOF\r\n",
        "cat <<-EOF\n\tit's\n\tEOF",
        "cat <<EOF\nit's\nEOF\n(( x ))",
        "cat <<EOF | cat\nit's\nEOF",
        "echo $(cat <<EOF\nit's\nEOF\n)",
    ] {
        assert_eq!(
            strip_comments_and_heredocs(command),
            None,
            "expected unreadable: {command:?}"
        );
    }
}

#[test]
fn literal_data_keeps_the_text_after_the_terminator() {
    for (command, expected) in [
        (
            "cat <<EOF > a.json\n{\"p\": \"/b\"}\nEOF\ntee sub/link/f",
            "cat <<EOF > a.json\ntee sub/link/f",
        ),
        (
            "cat <<EOF\nEOF \n EOF\nEOF\ncat .env",
            "cat <<EOF\ncat .env",
        ),
        ("cat <<EOF\nit's\nEOF", "cat <<EOF\n"),
    ] {
        assert_eq!(
            strip_comments_and_heredocs(command).as_deref(),
            Some(expected),
            "command: {command:?}"
        );
    }
}
