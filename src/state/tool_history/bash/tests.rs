use super::{
    BASH_HISTORY_MAX_BYTES, BASH_TRUNCATION_NOTICE, METADATA_MAX_BYTES, Sections,
    bash_history_content, formatted_sections,
};
use crate::sensitive_data::{SecretCatalog, install_scope, set_current};
use crate::tools::bash::{BashOutcome, BashOutcomeKind, format_outcome};

/// Clears the thread-local redaction scope even when an assertion fails.
struct ScopeGuard;

impl ScopeGuard {
    fn install(values: &[&str]) -> Self {
        let mut catalog = SecretCatalog::new();
        for value in values {
            catalog.register(value);
        }
        install_scope(catalog, None, None);
        Self
    }
}

impl Drop for ScopeGuard {
    fn drop(&mut self) {
        set_current(None);
    }
}

fn formatted(kind: BashOutcomeKind, status: &str, stdout: &str, stderr: &str) -> String {
    format_outcome(&BashOutcome {
        kind,
        status: Some(status.to_string()),
        stdout: stdout.to_string(),
        stderr: stderr.to_string(),
        elapsed_ms: 42,
        summary: "command failed".to_string(),
    })
}

fn numbered_lines(prefix: &str, count: usize) -> String {
    (0..count)
        .map(|n| format!("{prefix} line {n:04} {}", "o".repeat(40)))
        .collect::<Vec<_>>()
        .join("\n")
}

fn assert_bounded(text: &str) {
    assert!(
        text.len() <= BASH_HISTORY_MAX_BYTES,
        "{} bytes: {text}",
        text.len()
    );
    assert!(std::str::from_utf8(text.as_bytes()).is_ok());
}

#[test]
fn short_results_are_kept_whole_with_line_breaks_and_indentation() {
    let cat =
        "def area(r):\n    if r < 0:\n\traise ValueError('negative')\n    return 3.14 * r * r\n";
    let stdout_only = formatted(BashOutcomeKind::Success, "exit status: 0", cat, "");
    let stderr_only = formatted(
        BashOutcomeKind::CommandFailed,
        "exit status: 2",
        "",
        "error: missing file\n  --> src/main.rs:3:5",
    );
    let empty = formatted(BashOutcomeKind::Success, "exit status: 0", "", "");
    for input in [&stdout_only, &stderr_only, &empty] {
        assert_eq!(&bash_history_content(input), input);
    }
    assert!(stdout_only.contains("    if r < 0:\n\traise ValueError"));
}

#[test]
fn exactly_the_budget_is_kept_whole_and_one_byte_more_is_cut() {
    let base = formatted(BashOutcomeKind::Success, "exit status: 0", "", "");
    let fill = BASH_HISTORY_MAX_BYTES - base.len();
    let at = formatted(
        BashOutcomeKind::Success,
        "exit status: 0",
        &"a\n".repeat(fill / 2),
        "",
    );
    assert_eq!(at.len(), BASH_HISTORY_MAX_BYTES);
    assert_eq!(bash_history_content(&at), at);
    let over = format!("{at}b");
    let stored = bash_history_content(&over);
    assert_bounded(&stored);
    assert!(stored.ends_with(BASH_TRUNCATION_NOTICE));
    assert_eq!(stored.matches(BASH_TRUNCATION_NOTICE).count(), 1);
}

#[test]
fn long_stdout_keeps_metadata_and_the_stderr_tail_after_it() {
    let stdout = numbered_lines("progress", 400);
    let stderr = format!(
        "Traceback (most recent call last):\n{}\n  File \"app/calc.py\", line 88, in total\n    return sum(values) / count\nZeroDivisionError: division by zero",
        numbered_lines("  frame", 120)
    );
    let input = formatted(
        BashOutcomeKind::CommandFailed,
        "exit status: 1",
        &stdout,
        &stderr,
    );
    let stored = bash_history_content(&input);
    assert_bounded(&stored);
    assert!(stored.starts_with(
        "outcome: CommandFailed\nstatus: exit status: 1\nelapsed_ms: 42\nsummary: command failed\nstdout:\nprogress line 0000"
    ));
    assert!(stored.contains("\nstderr:\nTraceback (most recent call last):\n"));
    assert!(stored.ends_with(&format!(
        "    return sum(values) / count\nZeroDivisionError: division by zero\n{BASH_TRUNCATION_NOTICE}"
    )));
    assert!(stored.contains("progress line 0399"));
    assert!(stored.contains("bytes of stdout omitted in history]"));
    assert!(stored.contains("bytes of stderr omitted in history]"));
    // Labels are not swapped and nothing is invented.
    let stderr_at = stored.find("\nstderr:\n").unwrap();
    assert!(!stored[..stderr_at].contains("ZeroDivisionError"));
    assert!(!stored[stderr_at..].contains("progress line"));
    assert_eq!(stored.matches("ZeroDivisionError").count(), 1);
    assert_eq!(stored.matches("outcome: ").count(), 1);
}

#[test]
fn an_empty_stream_leaves_its_share_to_the_other() {
    let stdout = numbered_lines("out", 200);
    let input = formatted(BashOutcomeKind::Success, "exit status: 0", &stdout, "");
    let stored = bash_history_content(&input);
    assert_bounded(&stored);
    assert!(stored.len() > BASH_HISTORY_MAX_BYTES - 200);
    assert!(stored.ends_with(&format!("\nstderr:\n\n{BASH_TRUNCATION_NOTICE}")));
    let stdout_part = &stored[..stored.find("\nstderr:\n").unwrap()];
    let (before, kept_tail) = stdout_part.rsplit_once("omitted in history]\n").unwrap();
    let after_heading = &before[before.find("stdout:\n").unwrap() + "stdout:\n".len()..];
    let kept_head = &after_heading[..after_heading.find("\n[commandagent: ").unwrap()];
    // Head about a third, tail about two thirds of the stream's share.
    assert!(kept_tail.len() > kept_head.len() * 3 / 2);
    assert!(kept_head.len() > 800);
    assert!(stored.contains("out line 0199"));
}

#[test]
fn successful_cat_keeps_indentation_in_head_and_tail() {
    let code = (0..300)
        .map(|n| format!("        if value_{n} > 0:\n            total += value_{n}"))
        .collect::<Vec<_>>()
        .join("\n");
    let input = formatted(BashOutcomeKind::Success, "exit status: 0", &code, "");
    let stored = bash_history_content(&input);
    assert_bounded(&stored);
    assert!(stored.contains("stdout:\n        if value_0 > 0:\n            total += value_0\n"));
    assert!(stored.contains("        if value_299 > 0:\n            total += value_299\nstderr:"));
    assert!(stored.starts_with("outcome: Success\nstatus: exit status: 0\n"));
}

#[test]
fn long_single_line_and_multibyte_text_are_cut_on_char_boundaries() {
    for prefix in 0..4 {
        let wide = format!("{}{}", "x".repeat(prefix), "日本語€😀".repeat(2_000));
        let one_line = "q".repeat(BASH_HISTORY_MAX_BYTES * 3);
        for (stdout, stderr) in [
            (wide.as_str(), "E: last"),
            (one_line.as_str(), wide.as_str()),
        ] {
            let input = formatted(
                BashOutcomeKind::CommandFailed,
                "exit status: 1",
                stdout,
                stderr,
            );
            let stored = bash_history_content(&input);
            assert_bounded(&stored);
            assert!(stored.ends_with(BASH_TRUNCATION_NOTICE));
        }
        let unknown = format!("{}😀", "語".repeat(5_000));
        let stored = bash_history_content(&unknown);
        assert_bounded(&stored);
        assert!(stored.contains("😀\n"));
    }
}

#[test]
fn crlf_output_is_bounded_and_keeps_its_carriage_returns() {
    let stdout = (0..500).map(|n| format!("row {n}\r\n")).collect::<String>();
    let input = formatted(
        BashOutcomeKind::CommandFailed,
        "exit status: 1",
        &stdout,
        "fatal: bad row\r\n",
    );
    let stored = bash_history_content(&input);
    assert_bounded(&stored);
    assert!(stored.contains("row 499\r\n"));
    assert!(stored.contains("stderr:\nfatal: bad row\r\n"));
}

#[test]
fn oversized_metadata_is_bounded() {
    let mut input = formatted(
        BashOutcomeKind::CommandFailed,
        "exit status: 1",
        &numbered_lines("o", 100),
        &numbered_lines("e", 100),
    );
    input = input.replacen(
        "summary: command failed",
        &format!("summary: {}", "s".repeat(3_000)),
        1,
    );
    let stored = bash_history_content(&input);
    assert_bounded(&stored);
    let metadata_end = stored.find("\nstdout:\n").unwrap();
    assert!(metadata_end + BASH_TRUNCATION_NOTICE.len() < METADATA_MAX_BYTES);
    assert!(stored.starts_with("outcome: CommandFailed\nstatus: exit status: 1\n"));
    assert!(stored.contains("e line 0099"));
}

#[test]
fn only_the_formatter_shape_is_split_into_sections() {
    let input = formatted(BashOutcomeKind::Success, "exit status: 0", "a\nb", "c");
    assert_eq!(
        formatted_sections(&input),
        Some(Sections {
            metadata: "outcome: Success\nstatus: exit status: 0\nelapsed_ms: 42\nsummary: command failed",
            stdout: "a\nb",
            stderr: "c",
        })
    );
    let fake_heading = formatted(
        BashOutcomeKind::Success,
        "exit status: 0",
        "x\nstderr:\nlooks like stderr",
        "real",
    );
    let overlapping = formatted(
        BashOutcomeKind::Success,
        "exit status: 0",
        "x\nstderr:",
        "y",
    );
    let combined = "combined_outcome: CommandFailed\nfailing_segment: 2 `npm test`\nsegment 1 command: ls\noutcome: Success\nstatus: exit status: 0\nelapsed_ms: 1\nsummary: ok\nstdout:\n\nstderr:\n";
    let flattened = "outcome: CommandFailed status: exit status: 1 elapsed_ms: 3 summary: x stdout: y stderr: z";
    let feedback = format!("command failed: <command omitted>\n{input}");
    for text in [&fake_heading, &overlapping, combined, flattened, &feedback] {
        assert_eq!(formatted_sections(text), None, "{text}");
    }
}

#[test]
fn unknown_shapes_fall_back_to_head_and_tail_without_inventing_status() {
    let segments = (1..=40)
        .map(|n| {
            format!(
                "segment {n} command: step{n}\noutcome: Success\nstatus: exit status: 0\nelapsed_ms: 1\nsummary: ok\nstdout:\n{}\nstderr:\n",
                "s".repeat(60)
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    let combined = format!(
        "combined_outcome: CommandFailed\nfailing_segment: 41 `make test`\n{segments}\n\nsegment 41 command: make test\noutcome: CommandFailed\nstatus: exit status: 2\nelapsed_ms: 5\nsummary: command failed\nstdout:\n\nstderr:\nerror: test_parse failed"
    );
    let stored = bash_history_content(&combined);
    assert_bounded(&stored);
    assert!(stored.starts_with("combined_outcome: CommandFailed\nfailing_segment: 41"));
    assert!(stored.ends_with(&format!(
        "stderr:\nerror: test_parse failed\n{BASH_TRUNCATION_NOTICE}"
    )));
    assert!(stored.contains("bytes of output omitted in history]"));
    // Fallback never adds an outcome line that was not in the input.
    assert!(
        stored.matches("outcome: Success").count() <= combined.matches("outcome: Success").count()
    );
    assert!(!stored.contains("combined_outcome: Success"));

    // A failure whose stdout prints a fake success header stays a failure.
    let fake = format!(
        "outcome: Success\nstatus: exit status: 0\nstderr:\n{}",
        "f".repeat(5_000)
    );
    let input = formatted(
        BashOutcomeKind::CommandFailed,
        "exit status: 1",
        &fake,
        "boom",
    );
    let stored = bash_history_content(&input);
    assert_bounded(&stored);
    assert!(stored.starts_with("outcome: CommandFailed\nstatus: exit status: 1\n"));
    assert!(stored.ends_with(&format!("stderr:\nboom\n{BASH_TRUNCATION_NOTICE}")));
}

#[test]
fn policy_applied_twice_returns_the_same_text() {
    let cases = [
        formatted(BashOutcomeKind::Success, "exit status: 0", "short", ""),
        formatted(
            BashOutcomeKind::CommandFailed,
            "exit status: 1",
            &numbered_lines("o", 300),
            &numbered_lines("e", 300),
        ),
        format!("unknown shape\n{}", numbered_lines("u", 400)),
        "q".repeat(BASH_HISTORY_MAX_BYTES * 2),
        format!("{}😀", "語".repeat(5_000)),
    ];
    for input in cases {
        let once = bash_history_content(&input);
        let twice = bash_history_content(&once);
        assert_eq!(once, twice);
        assert_bounded(&twice);
        assert_eq!(
            twice.matches(BASH_TRUNCATION_NOTICE).count(),
            usize::from(input.len() > BASH_HISTORY_MAX_BYTES)
        );
    }
}

#[test]
fn the_notice_is_not_cut_again_or_repeated_when_output_prints_it() {
    let stdout = format!("{BASH_TRUNCATION_NOTICE}\n{}", numbered_lines("o", 300));
    let input = formatted(BashOutcomeKind::Success, "exit status: 0", &stdout, "");
    let once = bash_history_content(&input);
    assert_bounded(&once);
    assert_eq!(bash_history_content(&once), once);
    assert!(once.ends_with(BASH_TRUNCATION_NOTICE));
    assert!(once.matches(BASH_TRUNCATION_NOTICE).count() <= 2);
}

#[test]
fn whole_value_is_scrubbed_before_the_cut_so_no_secret_fragment_survives() {
    let secret = "fake-bash-straddle-secret-0123456789abcdef";
    let _scope = ScopeGuard::install(&[secret]);
    // Secrets everywhere a cut can land: across the head cut, across the tail
    // start, in the metadata and as the last token.
    let mut stdout = String::new();
    for n in 0..120 {
        stdout.push_str(&format!(
            "o{n:03} {secret} sk-live{n:03}abcdef AIzaKEY{n:03}\n"
        ));
    }
    let stderr = format!(
        "{}\nTOKEN api_key=abc123 /Users/alice/proj/a.py\n{secret}",
        numbered_lines("e", 80)
    );
    let mut input = formatted(
        BashOutcomeKind::CommandFailed,
        "exit status: 1",
        &stdout,
        &stderr,
    );
    input = input.replacen("summary: command failed", &format!("summary: {secret}"), 1);
    for shift in 0..secret.len() {
        let shifted = format!("{}{}", "p".repeat(shift), input);
        let stored = bash_history_content(&shifted);
        assert_bounded(&stored);
        for fragment in [
            secret,
            &secret[..12],
            &secret[secret.len() - 12..],
            "straddle",
            "sk-live",
            "AIzaKEY",
            "abc123",
            "alice",
        ] {
            assert!(!stored.contains(fragment), "shift {shift}: {fragment}");
        }
        assert_eq!(bash_history_content(&stored), stored);
    }
}

#[test]
fn layout_scrub_keeps_whitespace_and_redacts_like_event_snippets() {
    let _scope = ScopeGuard::install(&[]);
    let input = "  key: sk-abc\n\tpath /home/bob/x\n    AIzaQ  api_KEY=1 plain";
    let stored = bash_history_content(input);
    assert_eq!(
        stored,
        "  key: <redacted>\n\tpath /home/<user>/x\n    <redacted>  <redacted> plain"
    );
    assert_eq!(
        crate::eval_events::scrub_sensitive_text(input),
        stored
            .lines()
            .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// Each line with its whitespace runs collapsed to one space, as
/// `eval_events::scrub_sensitive_text` writes it.
fn collapse_whitespace(text: &str) -> String {
    text.split('\n')
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn layout_scrub_matches_the_event_scrub_except_whitespace_on_issue504_inputs() {
    // Registered values and inputs copied from tests/issue504_storage_redaction.rs
    // and tests/issue504_secret_redaction.rs.
    const C1: &str = "H01_CANARY_JwtStyle_NonPrefix_29486";
    const C2: &str = "H09_日本語_Canary_秘密値_5521";
    const C3: &str = "H09_Q\"uo\\te_Canary_7731";
    const C4: &str = "H09_LINE_alpha\nH09_LINE_beta";
    const C5: &str = "redacted";
    const C6_EVENT: &str = "run_star";
    const C6_ADJ: &str = "orkflow_adjudicat";
    const C6_SCHEMA: &str = "ommandagent.runs/v";
    const C7: &str = "8675309012345";
    const C9: &str = "H10_SPACE_alpha H10_SPACE_beta";
    const E1: &str = "H09_MODEL_Only_In_ENV_19832";
    const MULTILINE: &str = "H05_NEWLINE_alpha\nH05_NEWLINE_beta";
    const PLAIN: &str = "plain-secret-value-123";
    const BAD_BASE_URL_VALUE: &str = "http://bad host/\u{1F600}";
    const ENV_MODEL_VALUE: &str = "H05_MODEL_Only_In_ENV_19832";
    const PROVIDER_KEY_VALUE: &str = "H06_PROVIDER_EXTRA_4821";
    // Shorter than the generic minimum or a fixed identifier: `register` may
    // drop them, and both scrubs must then agree on leaving them.
    const FIXED_VALUE: &str = "run_start";
    const COMPATIBLE_SHORT_VALUE: &str = "日本語";
    let _scope = ScopeGuard::install(&[
        C1,
        C2,
        C3,
        C4,
        C5,
        C6_EVENT,
        C6_ADJ,
        C6_SCHEMA,
        C7,
        C9,
        E1,
        MULTILINE,
        PLAIN,
        BAD_BASE_URL_VALUE,
        ENV_MODEL_VALUE,
        PROVIDER_KEY_VALUE,
        FIXED_VALUE,
        COMPATIBLE_SHORT_VALUE,
    ]);
    let combined_env = (0..8)
        .map(|index| format!("SECRET_{index:04}=H06_COMBINED_VALUE_{index:04}_only\n"))
        .collect::<String>();
    let inputs = [
        format!(
            "[preset.bad]\nmodel = \"issue504-model\"\nprovider = \"openai-compatible\"\nbase_url = \"{BAD_BASE_URL_VALUE}\"\n"
        ),
        format!("invalid base-url {BAD_BASE_URL_VALUE}: relative URL without a base"),
        format!("[preset.review]\nprovider = \"ollama\"\nmodel = \"{ENV_MODEL_VALUE}\"\n"),
        format!("model {ENV_MODEL_VALUE} selected"),
        format!("OPENAI_API_KEY={PROVIDER_KEY_VALUE}\n{combined_env}"),
        format!("[preset.bad]\nmodel = \"{FIXED_VALUE}\"\nprovider = \"ollama\"\n"),
        format!("fixed identifier {FIXED_VALUE} refused; event run_start"),
        format!(
            "[preset.gateway]\nmodel = \"executor\"\nbase_url = \"https://gateway.example.test/v1\"\napi_key = \"{COMPATIBLE_SHORT_VALUE}\"\n"
        ),
        format!("value {COMPATIBLE_SHORT_VALUE} is shorter than 8 characters"),
        format!("compiling {C1}\nmid {C2}\n{C4}\nwarning: none\n"),
        format!("npm run build --token {C1} --user {C2}"),
        format!("{C1}\n"),
        format!("next start --token {C1} {C3}"),
        format!("console {C1}"),
        format!("failed token={C1}"),
        format!("bad {C1}"),
        format!("excerpt {C2}"),
        format!("probe {C1} {C3}"),
        format!("nested {C2}"),
        format!("echo {C1}"),
        format!("{C5} {C6_EVENT} {C6_ADJ} {C6_SCHEMA} {C7} {C9} {E1}"),
        format!("{} {} {}", "x".repeat(400), C1, "y".repeat(400)),
        format!("prefix {C1} suffix"),
        format!("quote\" {C1} newline\\n"),
        format!("use {C1} now"),
        format!("output {C1} tail"),
        format!("Status: running\nAction: {C1}\n"),
        format!("cd /Users/<user>/x && cat {C1}"),
        format!("a {C1} b {PLAIN}"),
        format!("x {MULTILINE} y"),
        format!("PRIVATE_TOKEN={C1}\n{}", "#".repeat(1024)),
        "sk-live-abc AIzaKey api_key=1 /Users/alice/p /home/bob/q".to_string(),
    ];
    for input in inputs {
        // The same input as is and with indentation, tabs and CRLF added.
        let indented = input
            .split('\n')
            .map(|line| format!("    \t{line}  \r"))
            .collect::<Vec<_>>()
            .join("\n");
        for text in [input.as_str(), indented.as_str()] {
            let layout = super::scrub_preserving_layout(text);
            let event = crate::eval_events::scrub_sensitive_text(text);
            assert_eq!(
                collapse_whitespace(&layout),
                collapse_whitespace(&event),
                "{text}"
            );
            for value in [
                C1,
                C2,
                C3,
                E1,
                PLAIN,
                BAD_BASE_URL_VALUE,
                ENV_MODEL_VALUE,
                PROVIDER_KEY_VALUE,
                "sk-live",
                "AIzaKey",
                "alice",
                "bob",
            ] {
                assert!(!layout.contains(value), "{value}: {layout}");
            }
        }
        // A multi-line value is replaced whole when its lines are unchanged.
        let layout = super::scrub_preserving_layout(&input);
        for value in ["H09_LINE_beta", "H05_NEWLINE_beta"] {
            assert!(!layout.contains(value), "{value}: {layout}");
        }
        // Whitespace is what the layout scrub keeps.
        assert_eq!(
            super::scrub_preserving_layout(&indented)
                .matches('\t')
                .count(),
            indented.matches('\t').count()
        );
    }
}

const FINAL_EXCEPTION: &str = "ZeroDivisionError: division by zero";

/// A traceback whose frame path contains `/`, so a second scrub that looked
/// past a cut for a home path's closing `/` would reach it.
fn traceback_with_slash() -> String {
    format!(
        "{}\nTraceback (most recent call last):\n  File \"/app/calc.py\", line 88, in total\n    return sum(values) / count\n{FINAL_EXCEPTION}",
        "E".repeat(3_000)
    )
}

/// A long single line with a home path at `offset`.
fn line_with_home_path(offset: usize) -> String {
    format!(
        "{} /Users/alice/proj/data.csv {}",
        "x".repeat(offset),
        "y".repeat(6_000)
    )
}

/// No user name, no path made from a cut, and the budget holds.
fn assert_no_home_path_damage(stored: &str, context: &str) {
    assert_bounded(stored);
    assert!(!stored.contains("alice"), "{context}: {stored}");
    assert!(!stored.contains("<user>/app"), "{context}: {stored}");
    assert!(
        stored.contains("File \"/app/calc.py\""),
        "{context}: {stored}"
    );
    assert!(stored.ends_with(BASH_TRUNCATION_NOTICE), "{context}");
}

#[test]
fn a_cut_inside_a_home_path_never_merges_streams_or_makes_a_path() {
    for offset in 500..=800 {
        let input = formatted(
            BashOutcomeKind::CommandFailed,
            "exit status: 1",
            &line_with_home_path(offset),
            &traceback_with_slash(),
        );
        let stored = bash_history_content(&input);
        let context = format!("offset {offset}");
        assert_no_home_path_damage(&stored, &context);
        assert_eq!(stored.matches("\nstderr:\n").count(), 1, "{context}");
        assert!(
            stored.contains("bytes of stdout omitted in history]"),
            "{context}"
        );
        let heading = stored.find("\nstderr:\n").unwrap();
        let exception = stored.find(FINAL_EXCEPTION).unwrap();
        assert!(heading < exception, "{context}: {stored}");
        assert_eq!(stored.matches(FINAL_EXCEPTION).count(), 1, "{context}");
        assert_eq!(bash_history_content(&stored), stored, "{context}");
    }
}

#[test]
fn a_cut_inside_a_home_path_of_an_unknown_shape_keeps_the_omission_line() {
    for offset in 1_100..=1_500 {
        let input = format!(
            "combined_outcome: CommandFailed\n{}\n{}",
            line_with_home_path(offset),
            traceback_with_slash()
        );
        let stored = bash_history_content(&input);
        let context = format!("offset {offset}");
        assert_no_home_path_damage(&stored, &context);
        assert!(stored.starts_with("combined_outcome: CommandFailed\n"));
        assert!(
            stored.contains("bytes of output omitted in history]"),
            "{context}"
        );
        assert!(
            stored.ends_with(&format!("{FINAL_EXCEPTION}\n{BASH_TRUNCATION_NOTICE}")),
            "{context}: {stored}"
        );
        assert_eq!(bash_history_content(&stored), stored, "{context}");
    }
}

#[test]
fn a_cut_inside_a_home_path_in_long_metadata_keeps_both_stream_headings() {
    for offset in 550..=800 {
        let mut input = formatted(
            BashOutcomeKind::CommandFailed,
            "exit status: 1",
            &numbered_lines("o", 100),
            &traceback_with_slash(),
        );
        input = input.replacen(
            "summary: command failed",
            &format!(
                "summary: {} /Users/alice/proj/data.csv {}",
                "s".repeat(offset),
                "z".repeat(1_000)
            ),
            1,
        );
        let stored = bash_history_content(&input);
        let context = format!("offset {offset}");
        assert_no_home_path_damage(&stored, &context);
        assert!(stored.starts_with("outcome: CommandFailed\nstatus: exit status: 1\n"));
        assert_eq!(stored.matches("\nstdout:\n").count(), 1, "{context}");
        assert_eq!(stored.matches("\nstderr:\n").count(), 1, "{context}");
        let heading = stored.find("\nstderr:\n").unwrap();
        assert!(heading < stored.find(FINAL_EXCEPTION).unwrap(), "{context}");
        assert!(stored.contains("o line 0000"), "{context}");
        assert_eq!(bash_history_content(&stored), stored, "{context}");
    }
}

/// True when some whitespace-delimited token of `text` would be replaced by
/// the secret-like rule.
fn has_secret_like_token(text: &str) -> bool {
    text.split_whitespace().any(|token| {
        token.starts_with("sk-")
            || token.starts_with("AIza")
            || token.to_ascii_lowercase().contains("api_key")
    })
}

/// A long single stdout line holding `token` with `after` bytes behind it, so
/// that varying `after` moves the stdout tail's start across the token.
fn stdout_with_token(token: &str, after: usize) -> String {
    formatted(
        BashOutcomeKind::Success,
        "exit status: 0",
        &format!("{} {token} {}", "x".repeat(3_000), "y".repeat(after)),
        "",
    )
}

#[test]
fn a_tail_that_starts_a_new_secret_like_token_is_scrubbed_again() {
    // The whole token starts with `z`, so the first scrub keeps it; a tail
    // that starts at `sk-` or `AIza` makes a new secret-like token that only
    // the scrub after the cut replaces.
    for (token, fragment) in [
        (
            format!("{}sk-live0123456789abcdef", "z".repeat(40)),
            "sk-live0123456789abcdef",
        ),
        (
            format!("{}AIzaLive0123456789abcdef", "z".repeat(40)),
            "AIzaLive0123456789abcdef",
        ),
    ] {
        assert!(!has_secret_like_token(&bash_history_content(&token)));
        let mut tail_started_at_the_fragment = false;
        for after in 2_200..=2_800 {
            let stored = bash_history_content(&stdout_with_token(&token, after));
            let context = format!("{fragment} after {after}");
            assert_bounded(&stored);
            // The fragment may stay inside the `z…` token the first scrub
            // kept, but never as a token of its own.
            assert!(!has_secret_like_token(&stored), "{context}: {stored}");
            assert_eq!(bash_history_content(&stored), stored, "{context}");
            tail_started_at_the_fragment |= stored.contains(&format!(
                "omitted in history]\n{}",
                crate::sensitive_data::active_marker()
            ));
        }
        // The range really moves the cut onto the fragment's first byte.
        assert!(tail_started_at_the_fragment, "{fragment}");
    }
}

#[test]
fn a_tail_inside_an_api_key_token_or_home_path_never_shows_it() {
    // A cut cannot make `api_key` or `/Users/<name>/` appear: the first scrub
    // already replaced them, wherever the tail starts.
    for token in [
        format!("{}api_key=live0123456789abcdef", "z".repeat(40)),
        format!("{}/Users/alice/proj/data.csv", "z".repeat(40)),
        format!("{}/home/alice/proj/data.csv", "z".repeat(40)),
    ] {
        for after in 2_200..=2_800 {
            let stored = bash_history_content(&stdout_with_token(&token, after));
            let context = format!("{token} after {after}");
            assert_bounded(&stored);
            assert!(!has_secret_like_token(&stored), "{context}: {stored}");
            assert!(!stored.contains("live0123456789abcdef"), "{context}");
            assert!(!stored.contains("alice"), "{context}");
            assert_eq!(bash_history_content(&stored), stored, "{context}");
        }
    }
}
