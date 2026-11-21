//! Pins the first refusal the Node recognizer reports and the repair line built
//! from it. The verdicts themselves stay pinned by `tests.rs`.

use super::super::super::weak_sources::collect;
use super::super::super::{
    SourceFile, WeakEvidenceSource, WorkspaceEvidence, verify_runtime_acceptance_with_hints,
};
use super::super::node_weak_detail;
use super::{assertion_rejection, has_failure_check, repair_guidance};
use std::path::Path;

const BASE: &str = "const assert = require('assert'); assert.ok(x);";

fn with_assert(body: &str) -> String {
    format!("const assert = require('assert'); {body}")
}

fn rejection_table() -> Vec<(String, Option<&'static str>)> {
    vec![
        (BASE.into(), None),
        (
            "import assert from 'node:assert/strict'; assert.equal(a, b);".into(),
            None,
        ),
        ("if (a) { process.exit(1); }".into(), None),
        (
            "#!/usr/bin/env node\nconst assert = require('assert'); assert(x);".into(),
            None,
        ),
        (with_assert("assert.ok(`x`);"), Some("template_literal")),
        (
            with_assert("assert.ok(/[\"']/.test(x));"),
            Some("unterminated_quote_or_comment"),
        ),
        (
            "const s = 'abc\n`t`;".into(),
            Some("unterminated_quote_or_comment,template_literal"),
        ),
        (
            with_assert("/* never closed assert.ok(x);"),
            Some("unterminated_quote_or_comment"),
        ),
        (
            with_assert("/abc/.test(x); assert.ok(y);"),
            Some("opaque_token:/"),
        ),
        (with_assert("assert.ok(a / b);"), Some("opaque_token:/")),
        (
            with_assert("try { a(); } catch (e) { } assert.ok(y);"),
            Some("opaque_token:catch"),
        ),
        (
            with_assert("assert.ok(eval(x));"),
            Some("opaque_token:eval"),
        ),
        (
            with_assert("assert.ok(x); const s = 'a'; \\ s;"),
            Some("opaque_token:\\"),
        ),
        (
            with_assert("const process = {}; assert.ok(x);"),
            Some("process_or_require_shadowed"),
        ),
        (with_assert("assert.ok(process.argv);"), Some("process_use")),
        (
            with_assert("process.exit = null; assert.ok(x);"),
            Some("process_use"),
        ),
        (
            with_assert("assert['ok'](x);"),
            Some("assert_computed_member"),
        ),
        (
            "let assert = require('assert'); assert.ok(x);".into(),
            Some("assert_binding_unsupported"),
        ),
        (
            with_assert("assert = null; assert.ok(x);"),
            Some("assert_binding_unsupported"),
        ),
        (
            with_assert("const check = assert; check.ok(x);"),
            Some("assert_binding_unsupported"),
        ),
        (
            "{ const assert = require('assert'); } assert.ok(x);".into(),
            Some("assert_binding_unsupported"),
        ),
        ("console.log('ok');".into(), Some("no_assert_binding")),
        ("assert.ok(x);".into(), Some("no_assert_binding")),
        (
            with_assert("function f() { assert.ok(x); }"),
            Some("no_reachable_top_level_check"),
        ),
        (
            with_assert("throw new Error('x'); assert.ok(x);"),
            Some("no_reachable_top_level_check"),
        ),
    ]
}

#[test]
fn first_refusal_is_named_for_each_screen() {
    let wrong = rejection_table()
        .into_iter()
        .filter(|(source, expected)| assertion_rejection(source).as_deref() != *expected)
        .map(|(source, expected)| {
            format!(
                "expected {expected:?}, got {:?}: {source}",
                assertion_rejection(&source)
            )
        })
        .collect::<Vec<_>>();
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn verdict_is_the_absence_of_a_refusal() {
    let wrong = rejection_table()
        .into_iter()
        .map(|(source, _)| source)
        .filter(|source| has_failure_check(source) != assertion_rejection(source).is_none())
        .collect::<Vec<_>>();
    assert!(wrong.is_empty(), "{wrong:?}");
}

fn workspace(script: &str) -> WorkspaceEvidence {
    WorkspaceEvidence {
        source_files: vec![SourceFile::new("check.js".into(), script.into())],
        ..Default::default()
    }
}

#[test]
fn command_stage_refusals_are_named() {
    let workspace = workspace("console.log('ok');");
    for (command, expected) in [
        ("node check.js", Some("no_assert_binding")),
        ("node check.js $HOME", Some("command_not_single_literal")),
        ("node", Some("no_script_argument")),
        (
            "node --no-warnings check.js",
            Some("node_option_before_script"),
        ),
        ("node ../check.js", Some("script_path_not_plain_relative")),
        ("node /check.js", Some("script_path_not_plain_relative")),
        ("node absent.js", Some("script_not_in_workspace_evidence")),
        ("node --print=\"x\"", Some("inline_not_executable")),
        ("node -e \"console.log(1)\"", Some("no_assert_binding")),
        ("npm run build", None),
        ("npm test", None),
        ("cargo test", None),
    ] {
        assert_eq!(
            node_weak_detail(command, &workspace).as_deref(),
            expected,
            "{command}"
        );
    }
    assert_eq!(
        node_weak_detail("node check.js", &self::workspace(BASE)),
        None
    );
}

#[test]
fn weak_sources_carry_the_detail_and_omit_it_otherwise() {
    let reason = "node_smoke_without_assertion".to_string();
    let sources = collect(
        &[reason.clone(), "route_unbound:/".into()],
        &["node check.js".into()],
        &workspace("console.log('ok');"),
    );
    assert_eq!(sources.len(), 2, "{sources:?}");
    assert_eq!(sources[0].detail.as_deref(), Some("no_assert_binding"));
    assert_eq!(sources[1].detail, None);
    let json = serde_json::to_value(&sources).unwrap();
    assert_eq!(json[0]["detail"], "no_assert_binding");
    assert!(json[1].get("detail").is_none(), "{json}");
}

fn source(command: Option<&str>, detail: Option<&str>) -> WeakEvidenceSource {
    WeakEvidenceSource {
        source: "contract_verify_command".into(),
        reason: "node_smoke_without_assertion".into(),
        command: command.map(str::to_string),
        detail: detail.map(str::to_string),
    }
}

#[test]
fn repair_line_names_command_first_refusal_and_accepted_shape() {
    let line = source(Some("node smoke-check.js"), Some("no_assert_binding"))
        .repair_guidance()
        .expect("a source with a detail has guidance");
    assert_eq!(
        line,
        repair_guidance("node smoke-check.js", "no_assert_binding")
    );
    for expected in [
        "`node smoke-check.js`",
        "first rejection: no_assert_binding",
        "Keep the verify command",
        "const assert = require(\"node:assert/strict\");",
        "assert.<ok|equal|",
        "top-level",
        "backticks",
        "regex literals and the `/` operator",
        "new RegExp",
        "try/catch/finally",
        "`process.exit(<code>)`",
        "`assert[...]`",
    ] {
        assert!(line.contains(expected), "missing {expected:?}: {line}");
    }
    // Those words would route the line into other prompt handling.
    for forbidden in [
        "weak_verification_evidence",
        "unverified:",
        "node --test",
        "\n",
    ] {
        assert!(!line.contains(forbidden), "found {forbidden:?}: {line}");
    }
}

#[test]
fn only_a_source_with_command_and_detail_has_a_repair_line() {
    assert_eq!(source(Some("node a.js"), None).repair_guidance(), None);
    assert_eq!(
        source(None, Some("no_assert_binding")).repair_guidance(),
        None
    );
}

fn corpus_acceptance(case: &str) -> crate::minimal_loop::evidence::RuntimeAcceptanceReport {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus/apps");
    verify_runtime_acceptance_with_hints(
        &root.join(case),
        &["package.json".to_string(), "smoke-check.js".to_string()],
        &["node smoke-check.js".to_string()],
        &[],
        &["implementation_artifact".to_string()],
        &[],
        &[],
        &[],
    )
}

#[test]
fn corpus_script_is_refused_with_a_reason_and_a_repair_line() {
    let report = corpus_acceptance("issue849-node-smoke-rejection-detail");
    assert!(!report.passed, "{report:?}");
    assert_eq!(
        report.primary_reason,
        "weak_verification_evidence:node_smoke_without_assertion"
    );
    let [weak] = report.weak_evidence_sources.as_slice() else {
        panic!("{:?}", report.weak_evidence_sources);
    };
    assert_eq!(
        weak.detail.as_deref(),
        Some("unterminated_quote_or_comment,template_literal")
    );
    let line = weak.repair_guidance().expect("repair line");
    assert!(line.contains("`node smoke-check.js`"), "{line}");
    assert!(
        line.contains("first rejection: unterminated_quote_or_comment,template_literal"),
        "{line}"
    );
}

#[test]
fn corpus_script_rewritten_as_the_guidance_says_is_accepted() {
    let report = corpus_acceptance("issue849-node-smoke-rejection-detail-fixed");
    assert!(report.passed, "{report:?}");
    assert!(report.weak_evidence_sources.is_empty(), "{report:?}");
    assert!(report.weak_evidence.is_empty(), "{report:?}");
}
