//! Recovery/diagnostic classification, evidence-hint extraction and repair
//! guidance pins for `completion.rs` (Issue #511, audit FIN-029 / FIN-030).
//!
//! Each test names the `cargo mutants` mutations (file:line in
//! `completion.rs` at develop `c6a31e17`) it is meant to catch with a
//! `// kills:` comment. Predicate tests come as tables whose rows straddle the
//! boundary, so a flipped condition changes at least one row.

use super::*;
use crate::planner::profile_descriptor::NEXTJS_PROFILE_ID as NEXT;

fn s(value: &str) -> String {
    value.to_string()
}

fn compile_error(
    path: &str,
    line: usize,
    column: usize,
    message: &str,
    excerpt: &str,
) -> CompileError {
    CompileError {
        path: s(path),
        line,
        column,
        message: s(message),
        excerpt: s(excerpt),
        symbol: None,
        route_bound: None,
    }
}

fn contract() -> CompletionContract {
    CompletionContract {
        required_paths: Vec::new(),
        protected_paths: Vec::new(),
        verify_commands: Vec::new(),
        fix_reproducer_command: None,
        profile: None,
        goal: None,
        required_capabilities: Vec::new(),
        deterministic_oracles: Vec::new(),
        required_evidence: Vec::new(),
        evidence_hint_tokens: Vec::new(),
        required_obligations: Vec::new(),
        deferred_verify_requirements: Vec::new(),
        verify_repair_cap: 2,
    }
}

fn deferred(command: &str, profile: Option<&str>, status: &str) -> DeferredVerifyRequirement {
    DeferredVerifyRequirement {
        command: s(command),
        reason: s("needs setup"),
        authority: s("postcheck"),
        profile: profile.map(s),
        status: s(status),
    }
}

fn install_next_dependencies(root: &Path) {
    std::fs::create_dir_all(root.join("node_modules/.bin")).unwrap();
    std::fs::create_dir_all(root.join("node_modules/next")).unwrap();
    std::fs::write(root.join("node_modules/.bin/next"), "#!/bin/sh\n").unwrap();
}

// ---------------------------------------------------------------------------
// FIN-029: deferred status / blocking reason / default status
// ---------------------------------------------------------------------------

#[test]
fn deferred_status_summary_classifies_each_requirement() {
    // kills: completion.rs:562 deferred_status_summary -> vec![] / vec![""] / vec!["xyzzy"]
    // kills: completion.rs:580 delete ! (optional requirement keeps its status)
    // kills: completion.rs:583 && -> || (npm test needs setup but not the next binary)
    // kills: completion.rs:584 && -> ||, delete ! (ready dependencies are not "missing")
    let missing = tempfile::tempdir().unwrap();
    let ready = tempfile::tempdir().unwrap();
    install_next_dependencies(ready.path());

    let mut optional = contract();
    optional.deferred_verify_requirements = vec![deferred("npm run build", Some(NEXT), "optional")];
    assert_eq!(
        optional.deferred_status_summary(missing.path(), "goal"),
        vec![s("npm run build:optional")]
    );

    let mut build = contract();
    build.deferred_verify_requirements = vec![deferred("npm run build", Some(NEXT), "pending")];
    assert_eq!(
        build.deferred_status_summary(missing.path(), "goal"),
        vec![s("npm run build:dependency_setup_missing")]
    );
    assert_eq!(
        build.deferred_status_summary(ready.path(), "goal"),
        vec![s("npm run build:build_verifier_required")]
    );

    let mut node_test = contract();
    node_test.deferred_verify_requirements = vec![deferred("npm test", None, "pending")];
    assert_eq!(
        node_test.deferred_status_summary(missing.path(), "goal"),
        vec![s("npm test:build_verifier_required")]
    );
}

#[test]
fn deferred_status_summary_passes_non_build_status_through() {
    // kills: completion.rs:562 return-value replacements (non-build rows)
    let root = tempfile::tempdir().unwrap();
    let mut c = contract();
    c.deferred_verify_requirements = vec![
        deferred("python3 -m unittest", None, "pending"),
        deferred(
            "python3 -m unittest",
            None,
            "covered_by_static_profile_check",
        ),
        deferred("python3 -m unittest", None, "blocked_by_setup"),
    ];
    assert_eq!(
        c.deferred_status_summary(root.path(), "goal"),
        vec![
            s("python3 -m unittest:pending"),
            s("python3 -m unittest:covered_by_static_profile_check"),
            s("python3 -m unittest:blocked_by_setup"),
        ]
    );
}

#[test]
fn deferred_requirement_blocking_reason_names_command_status_and_reason() {
    // kills: completion.rs:631 deferred_requirement_blocking_reason -> "" / "xyzzy"
    // (the `delete !` mutant at 631:12 is equivalent: both branches build the
    // same message, so only the message itself is pinned.)
    let root = tempfile::tempdir().unwrap();
    let c = contract();
    let requirement = deferred("npm run build", Some(NEXT), "pending");
    let expected = "deferred verify requirement pending: command `npm run build` status `pending` reason `needs setup`";
    assert_eq!(
        c.deferred_requirement_blocking_reason(root.path(), &requirement, false),
        expected
    );
    assert_eq!(
        c.deferred_requirement_blocking_reason(root.path(), &requirement, true),
        expected
    );
}

#[test]
fn default_deferred_status_is_pending() {
    // kills: completion.rs:2039 default_deferred_status -> "" / "xyzzy"
    assert_eq!(default_deferred_status(), "pending");
}

// ---------------------------------------------------------------------------
// FIN-030: feedback assembly
// ---------------------------------------------------------------------------

fn feedback_lines(report: &VerificationReport) -> Vec<String> {
    format_verify_feedback_with_contract(report, None)
        .lines()
        .map(str::to_string)
        .collect()
}

#[test]
fn verify_feedback_for_empty_report_has_header_and_guidance_only() {
    // kills: completion.rs:683 format_verify_feedback_with_contract -> "" / "xyzzy"
    // kills: completion.rs:702 delete ! (no missing paths -> no line)
    // kills: completion.rs:708 delete ! (no compile errors -> no section)
    // kills: completion.rs:738 delete ! (no targets -> no "Likely target files")
    let lines = feedback_lines(&VerificationReport::pass());
    assert_eq!(
        lines.first().map(String::as_str),
        Some("Deterministic completion verification failed. Fix the implementation and retry.")
    );
    assert!(lines.iter().any(|l| l.starts_with("Repair target: ")));
    assert!(
        lines
            .last()
            .unwrap()
            .starts_with("Repair guidance: inspect the smallest relevant file range")
    );
    for absent in [
        "Missing required paths",
        "Compile repair details",
        "Likely target files",
        "Command failed",
        "Failure excerpt",
    ] {
        assert!(
            !lines.iter().any(|l| l.contains(absent)),
            "unexpected `{absent}` in {lines:?}"
        );
    }
}

#[test]
fn verify_feedback_lists_missing_paths_and_command_failure_details() {
    // kills: completion.rs:702 delete ! (missing paths present -> line emitted)
    // kills: completion.rs:738 delete ! (targets present -> line emitted)
    let mut report = VerificationReport::pass();
    report.push_missing_path("src/lib.rs");
    report.push_missing_path("src/main.rs");
    report.push_command_failure(
        "python3 -m unittest tests/test_calc.py",
        "AssertionError: expected 3 actual 4\n  File \"src/calc.py\", line 3",
    );
    let lines = feedback_lines(&report);
    assert!(lines.contains(&s("Missing required paths: src/lib.rs, src/main.rs")));
    assert!(lines.contains(&s("Likely target files: src/calc.py, tests/test_calc.py")));
    assert!(lines.contains(&s("Failure excerpt: AssertionError: expected 3 actual 4")));
    assert!(lines.contains(&s(
        "Command failed: `python3 -m unittest tests/test_calc.py`"
    )));
}

#[test]
fn verify_feedback_flags_pytest_style_tests_under_unittest() {
    let mut report = VerificationReport::pass();
    report.push_command_failure(
        "python3 -m unittest tests/test_calc.py",
        "test_framework_mismatch:pytest_style_under_unittest",
    );
    let lines = feedback_lines(&report);
    assert!(lines.iter().any(|l| l.starts_with(
        "Test framework mismatch: `python3 -m unittest` does not discover pytest-style free functions."
    )));
}

#[test]
fn verify_feedback_includes_compile_repair_section() {
    // kills: completion.rs:708 delete ! (compile errors present -> section emitted)
    let mut report = VerificationReport::pass();
    report.push_compile_errors(
        "npm run build",
        vec![compile_error(
            "src/app/page.tsx",
            3,
            5,
            "Cannot find name 'foo'.",
            "",
        )],
    );
    let lines = feedback_lines(&report);
    assert!(lines.contains(&s("Compile repair details:")));
    assert!(lines.contains(&s("- Compile error location: src/app/page.tsx:3:5")));
    assert!(lines.contains(&s("Target implementation files: src/app/page.tsx")));
}

#[test]
fn verify_feedback_profile_failure_guidance_requires_both_css_markers() {
    // kills: completion.rs:772 && -> || (CSS guidance needs both markers)
    let css_line = "Next.js CSS repair: replace or create `src/app/global.d.ts` with exactly `declare module \"*.css\";` on its own line.";
    let mut both = VerificationReport::pass();
    both.push_profile_failure("CSS side-effect imports require declare module \"*.css\"");
    assert!(feedback_lines(&both).contains(&s(css_line)));

    for only_one in [
        "CSS side-effect imports require a declaration",
        "add declare module \"*.css\" somewhere",
    ] {
        let mut report = VerificationReport::pass();
        report.push_profile_failure(only_one);
        let lines = feedback_lines(&report);
        assert!(lines.contains(&format!("Profile contract failed: {only_one}")));
        assert!(
            !lines.iter().any(|l| l.starts_with("Next.js CSS repair")),
            "{only_one}: {lines:?}"
        );
    }
}

#[test]
fn verify_feedback_use_client_guidance() {
    let mut report = VerificationReport::pass();
    report.push_profile_failure("src/app/page.tsx must start with \"use client\"");
    assert!(feedback_lines(&report).iter().any(|l| l.starts_with(
        "Next.js client component repair: put `\"use client\";` as the first non-empty statement"
    )));
}

#[test]
fn verify_feedback_capability_evidence_guidance_fires_for_each_marker() {
    // kills: completion.rs:786 || -> && ; 787 || -> && ; 788 || -> &&
    let guidance = "Capability evidence guidance: add the smallest concrete test/check/UI evidence requested by the task, then keep the existing verification strong enough to exercise that evidence.";
    for marker in [
        "missing_required_capabilities:score",
        "missing_required_evidence:test_artifact",
        "weak_verification_evidence:node_smoke_without_assertion",
        "unverified runtime evidence",
    ] {
        let mut report = VerificationReport::pass();
        report.push_profile_failure(marker);
        let lines = feedback_lines(&report);
        assert!(
            lines.contains(&s(guidance)),
            "marker `{marker}` lost its guidance: {lines:?}"
        );
    }
    let mut unrelated = VerificationReport::pass();
    unrelated.push_profile_failure("something else entirely");
    assert!(!feedback_lines(&unrelated).contains(&s(guidance)));
}

// ---------------------------------------------------------------------------
// FIN-030: compile error repair guidance
// ---------------------------------------------------------------------------

#[test]
fn compile_error_repair_guidance_snapshot_without_excerpt() {
    // kills: completion.rs:851 compile_error_repair_guidance -> vec![] / vec![""] / vec!["xyzzy"]
    // kills: completion.rs:858 compile_error_repair_guidance_with_root -> same
    // kills: completion.rs:866 delete ! (empty excerpt adds no excerpt line)
    // kills: completion.rs:896 == -> != (route_bound None adds no route note)
    let mut error = compile_error("src/app/page.tsx", 3, 5, "Cannot find name 'foo'.", "");
    error.symbol = Some(s("foo"));
    let expected = vec![
        s("Compile error: src/app/page.tsx:3:5 Cannot find name 'foo'."),
        s("Compile error location: src/app/page.tsx:3:5"),
        s("Compile error message: Cannot find name 'foo'."),
        s(
            "You MUST modify src/app/page.tsx using the edit tool; a reply without file edits fails this repair.",
        ),
        s(
            "Cannot-find-name repair for `foo` in src/app/page.tsx:3:5: define foo, or replace the reference with an existing handler, or remove the dead code.",
        ),
    ];
    assert_eq!(compile_error_repair_guidance(&[error.clone()]), expected);
    assert_eq!(
        compile_error_repair_guidance_with_root(None, &[error]),
        expected
    );
}

#[test]
fn compile_error_repair_guidance_snapshot_with_excerpt_and_route_note() {
    // kills: completion.rs:866 delete ! (non-empty excerpt is emitted, trimmed)
    // kills: completion.rs:896 == -> != (route note only for route_bound == Some(false))
    let mut error = compile_error(
        "src/lib/util.ts",
        7,
        0,
        "Cannot find name 'bar'.",
        "\n  7 | bar();\n    | ^^^\n",
    );
    error.symbol = Some(s("bar"));

    error.route_bound = Some(false);
    let not_routed = compile_error_repair_guidance(&[error.clone()]);
    assert_eq!(
        not_routed,
        vec![
            s("Compile error: src/lib/util.ts:7 Cannot find name 'bar'."),
            s("Compile error location: src/lib/util.ts:7"),
            s("Compile error message: Cannot find name 'bar'."),
            s("Compile error excerpt for src/lib/util.ts:7:\n7 | bar();\n    | ^^^"),
            s(
                "You MUST modify src/lib/util.ts using the edit tool; a reply without file edits fails this repair.",
            ),
            s(
                "Cannot-find-name repair for `bar` in src/lib/util.ts:7: define bar, or replace the reference with an existing handler, or remove the dead code - the file is not imported by any route.",
            ),
        ]
    );

    for route_bound in [Some(true), None] {
        error.route_bound = route_bound;
        let lines = compile_error_repair_guidance(&[error.clone()]);
        assert!(
            lines.last().unwrap().ends_with("or remove the dead code."),
            "{route_bound:?}: {lines:?}"
        );
    }
}

#[test]
fn typescript_nullability_message_table() {
    // kills: completion.rs:926 -> true / false
    // kills: completion.rs:928,929,930,931,932 || -> &&
    for message in [
        "'user' is possibly 'null'.",
        "\"user\" is possibly \"null\".",
        "`user` is possibly `null`.",
        "'user' is possibly 'undefined'.",
        "\"user\" is possibly \"undefined\".",
        "`user` is possibly `undefined`.",
        "OBJECT IS POSSIBLY 'NULL'.",
    ] {
        assert!(typescript_nullability_message(message), "{message}");
    }
    for message in [
        "'user' is possibly nothing.",
        "Object is possibly.",
        "Type 'null' is not assignable to type 'string'.",
        "",
    ] {
        assert!(!typescript_nullability_message(message), "{message}");
    }
}

#[test]
fn typescript_const_reassignment_message_table() {
    // kills: completion.rs:986 -> true / false
    // kills: completion.rs:988,989,990 || -> &&
    for message in [
        "Cannot assign to 'x'",
        "Property is a read only property",
        "you may not reassign here",
        "x is a constant",
        "CANNOT ASSIGN TO 'X'",
    ] {
        assert!(typescript_const_reassignment_message(message), "{message}");
    }
    for message in ["Cannot find name 'x'.", "assign", "read only", ""] {
        assert!(!typescript_const_reassignment_message(message), "{message}");
    }
}

#[test]
fn const_reassignment_declares_symbol_table() {
    // kills: completion.rs:1004 -> true / false
    // kills: completion.rs:1005 delete !
    // kills: completion.rs:1009,1010,1011,1012 || -> && (one row per terminator)
    // kills: completion.rs:1016,1017 && -> || and the || between the two forms
    let rows: &[(&str, &str, bool)] = &[
        ("const x = 1;", "x", true),
        ("const x=1;", "x", true),
        ("const x, y = pair;", "x", true),
        ("const x) trailing", "x", true),
        ("const x] trailing", "x", true),
        ("const [a, setX] = useState(0);", "setX", true),
        ("const { a, b } = obj;", "b", true),
        ("let x = 1;", "x", false),
        ("var x = 1;", "x", false),
        ("const y = x + 1;", "x", false),
        ("const [a, b] = pair;", "zzz", false),
        ("const { a } = obj;", "zzz", false),
        ("const xy = 1;", "x", false),
        ("", "x", false),
    ];
    for (line, symbol, expected) in rows {
        assert_eq!(
            const_reassignment_declares_symbol(line, symbol),
            *expected,
            "{line} / {symbol}"
        );
    }
}

#[test]
fn duplicate_binding_repair_guidance_snapshots() {
    // kills: completion.rs:1032 -> None / Some("") / Some("xyzzy")
    // kills: completion.rs:1034 && -> ||, > -> ==, > -> <, > -> >=, delete !
    // kills: completion.rs:1041 == -> !=
    let message =
        "Cannot redeclare block-scoped variable 'count'. 'count' is defined multiple times.";
    let excerpt = "  3 | const count = 1;\n> 5 | const count = 2;\n    |       ^^^";

    // Earlier line from the excerpt, later line from the error position.
    let two_lines =
        duplicate_binding_repair_guidance(&compile_error("src/a.ts", 5, 7, message, excerpt));
    assert_eq!(
        two_lines.as_deref(),
        Some(
            "TypeScript duplicate-binding repair for `count`: lines 3 and 5 both declare `count`; remove or rename the later redeclaration (line 5); the earlier binding (line 3) is already in scope in this block."
        )
    );

    // The error line is appended when the excerpt does not contain it.
    let appended = duplicate_binding_repair_guidance(&compile_error(
        "src/a.ts",
        9,
        1,
        message,
        "  3 | const count = 1;",
    ));
    assert!(appended.unwrap().contains("lines 3 and 9 both declare"));

    // Line 0 is never appended; the excerpt alone decides.
    let no_line =
        duplicate_binding_repair_guidance(&compile_error("src/a.ts", 0, 0, message, excerpt));
    assert!(no_line.unwrap().contains("lines 3 and 5 both declare"));

    // A single known line means the redeclaration is in the same block.
    let single = duplicate_binding_repair_guidance(&compile_error("src/a.ts", 5, 7, message, ""));
    assert_eq!(
        single.as_deref(),
        Some(
            "TypeScript duplicate-binding repair for `count`: line 5 redeclares `count` in the same block; remove or rename the redeclaration so only one binding remains in scope."
        )
    );

    // No line information at all, or an unrelated message: nothing to say.
    assert_eq!(
        duplicate_binding_repair_guidance(&compile_error("src/a.ts", 0, 0, message, "")),
        None
    );
    assert_eq!(
        duplicate_binding_repair_guidance(&compile_error(
            "src/a.ts",
            5,
            1,
            "Cannot find name 'count'.",
            excerpt
        )),
        None
    );
}

#[test]
fn postcss_plugins_key_guidance_table() {
    // kills: completion.rs:1060 -> None / Some("") / Some("xyzzy")
    // kills: completion.rs:1061 delete !
    // kills: completion.rs:1062,1063 && -> || ; 1063 || -> &&
    let expected = "PostCSS config-format remedy: make postcss.config.js export a plugins key using CommonJS, e.g. module.exports = { plugins: { tailwindcss: {}, autoprefixer: {} } }; or make the package coherently ESM with .mjs/package.json type module.";
    for message in [
        "PostCSS config must export an object with plugins",
        "postcss: plugins - please export a config",
    ] {
        let error = compile_error("postcss.config.js", 1, 1, message, "");
        assert_eq!(
            postcss_plugins_key_repair_guidance(&error).as_deref(),
            Some(expected),
            "{message}"
        );
    }
    for message in [
        "postcss config must export an object",
        "plugins must export an object",
        "postcss plugins are listed",
        "Cannot find name 'x'.",
    ] {
        let error = compile_error("postcss.config.js", 1, 1, message, "");
        assert_eq!(
            postcss_plugins_key_repair_guidance(&error),
            None,
            "{message}"
        );
    }
}

#[test]
fn compile_excerpt_source_line_numbers_table() {
    // kills: completion.rs:1074 -> vec![] / vec![0] / vec![1]
    // kills: completion.rs:1081 || -> && (x2), == -> != (x2)
    // kills: completion.rs:1082 && -> ||
    // kills: completion.rs:1091 && -> ||, delete !
    let rows: &[(&str, &[usize])] = &[
        (
            "  3 | const a = 1;\n> 5 | const a = 2;\n    |       ^^^",
            &[3, 5],
        ),
        (">7 | x\n8: | y", &[7, 8]),
        ("3 | a\n3 | b\n4 | c", &[3, 4]),
        ("x1 | foo", &[]),
        ("abc | def", &[]),
        ("> | no digits", &[]),
        ("no pipe 4", &[]),
        ("    | ^^^", &[]),
        ("", &[]),
    ];
    for (excerpt, expected) in rows {
        assert_eq!(
            compile_excerpt_source_line_numbers(excerpt),
            expected.to_vec(),
            "{excerpt:?}"
        );
    }
}

#[test]
fn call_callee_for_error_column_table() {
    // kills: completion.rs:1100 -> None / Some("") / Some("xyzzy")
    // kills: completion.rs:1108 || -> && (x3), == -> != (x3)
    // kills: completion.rs:1114 delete !
    // kills: completion.rs:1117 <= -> >
    let rows: &[(&str, usize, Option<&str>)] = &[
        ("foo(1)", 1, Some("foo")),
        ("x = foo(1)", 5, Some("foo")),
        ("a_b(1)", 1, Some("a_b")),
        ("a$b(1)", 1, Some("a$b")),
        ("obj.method(1)", 1, Some("method")),
        ("foo (1)", 1, Some("foo")),
        ("a(1) + b(2)", 9, Some("b")),
        ("a(1) + b(2)", 1, Some("a")),
        ("a(1) + b(2)", 3, Some("a")),
        ("(1 + 2)", 1, None),
        ("no call here", 1, None),
    ];
    for (line, column, expected) in rows {
        assert_eq!(
            call_callee_for_error_column(line, *column).as_deref(),
            *expected,
            "{line} @ {column}"
        );
    }
}

#[test]
fn local_arrow_function_definition_starts_table() {
    // kills: completion.rs:1152 -> true / false
    for prefix in [
        "const",
        "let",
        "var",
        "export const",
        "export let",
        "export var",
    ] {
        let line = format!("{prefix} handler = (a, b) =>");
        assert!(
            local_arrow_function_definition_starts(&line, "handler"),
            "{line}"
        );
        assert!(
            !local_arrow_function_definition_starts(&line, "other"),
            "{line}"
        );
    }
    assert!(!local_arrow_function_definition_starts(
        "function handler() {",
        "handler"
    ));
    assert!(!local_arrow_function_definition_starts(
        "const handler: Fn = ",
        "handler"
    ));
}

#[test]
fn property_missing_type_name_table() {
    // kills: completion.rs:1276 -> None / Some("") / Some("xyzzy")
    assert_eq!(
        property_missing_type_name("Property 'foo' does not exist on type 'Bar'.").as_deref(),
        Some("Bar")
    );
    assert_eq!(
        property_missing_type_name("Property 'foo' does not exist on type 'Store<T>'."),
        None
    );
    assert_eq!(
        property_missing_type_name("Property 'foo' does not exist on type '{ a: number }'."),
        None
    );
    assert_eq!(
        property_missing_type_name("Property 'foo' is missing."),
        None
    );
}

#[test]
fn find_object_destructure_assignment_expression_table() {
    // kills: completion.rs:1349 -> None / Some("") / Some("xyzzy")
    // kills: completion.rs:1352 delete !
    // kills: completion.rs:1361 delete !
    // kills: completion.rs:1366 && -> ||
    // kills: completion.rs:1370 && -> || (x2), delete ! (x2)
    let find = find_object_destructure_assignment_expression;
    let single = "import x from 'y';\nconst { alpha, beta } = useStore();\n";
    assert_eq!(find(single, "beta").as_deref(), Some("useStore()"));
    assert_eq!(find(single, "alpha").as_deref(), Some("useStore()"));
    assert_eq!(find(single, "gamma"), None);
    assert_eq!(find("const value = 1;\n", "value"), None);
    assert_eq!(
        find("let { a: renamed } = make(1);\n", "a").as_deref(),
        Some("make(1)")
    );
    assert_eq!(find("var {a}=wrap();\n", "a").as_deref(), Some("wrap()"));

    // A multi-line pattern keeps its newlines, so a trailing comment cannot
    // swallow the next property.
    let multi = "const {\n  a, // first\n  b,\n} = useThing();\n";
    assert_eq!(find(multi, "b").as_deref(), Some("useThing()"));
    assert_eq!(find(multi, "a").as_deref(), Some("useThing()"));

    // A statement that ends before any closing `} =` stops the scan, so a
    // later unrelated assignment is not attributed to the pattern.
    let unterminated = "let {\np, q;\n} = after();\n";
    assert_eq!(find(unterminated, "p"), None);
}

#[test]
fn destructuring_contains_property_table() {
    // kills: completion.rs:1387 -> true / false
    // kills: completion.rs:1403 == -> !=
    let rows: &[(&str, &str, bool)] = &[
        ("const { a, b } = x", "b", true),
        ("const { a, b } = x", "a", true),
        ("const { a, b } = x", "c", false),
        ("const { a: alias } = x", "a", true),
        ("const { a: alias } = x", "alias", false),
        ("const { a = 1 } = x", "a", true),
        ("const {\n a,\n b\n} = x", "b", true),
        ("const a = x", "a", false),
        ("const { a", "a", false),
    ];
    for (block, property, expected) in rows {
        assert_eq!(
            destructuring_contains_property(block, property),
            *expected,
            "{block} / {property}"
        );
    }
}

#[test]
fn source_line_reads_one_based_lines() {
    // kills: completion.rs:1432 -> None / Some("") / Some("xyzzy")
    // kills: completion.rs:1432 == -> != (line 0 is rejected, not read)
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.ts"), "first\nsecond\nthird\n").unwrap();
    assert_eq!(source_line(dir.path(), "a.ts", 1).as_deref(), Some("first"));
    assert_eq!(
        source_line(dir.path(), "a.ts", 2).as_deref(),
        Some("second")
    );
    assert_eq!(source_line(dir.path(), "a.ts", 0), None);
    assert_eq!(source_line(dir.path(), "a.ts", 4), None);
    assert_eq!(source_line(dir.path(), "missing.ts", 1), None);
}

#[test]
fn compile_excerpt_source_line_table() {
    // kills: completion.rs:1443 -> None / Some("") / Some("xyzzy")
    // kills: completion.rs:1446 && -> ||, delete ! (x2)
    let rows: &[(&str, Option<&str>)] = &[
        ("  3 |   foo(bar)\n    | ^^^", Some("foo(bar)")),
        ("  2 |\n  3 | code", Some("code")),
        ("    |   ^^^\n  5 | code", Some("code")),
        ("    |   ^^^", None),
        ("  2 |", None),
        ("no pipe at all", None),
        ("", None),
    ];
    for (excerpt, expected) in rows {
        assert_eq!(
            compile_excerpt_source_line(excerpt).as_deref(),
            *expected,
            "{excerpt:?}"
        );
    }
}

#[test]
fn receiver_before_property_table() {
    // kills: completion.rs:1451 -> None / Some("") / Some("xyzzy")
    // kills: completion.rs:1457 || -> && (x2), == -> != (x2)
    let rows: &[(&str, &str, Option<&str>)] = &[
        ("const v = store.getState()", "getState", Some("store")),
        ("this.items.length", "length", Some("items")),
        ("a_b.foo", "foo", Some("a_b")),
        ("a$b.foo", "foo", Some("a$b")),
        ("(compute()).bar", "bar", None),
        ("1abc.foo", "foo", None),
        ("no property here", "foo", None),
    ];
    for (line, property, expected) in rows {
        assert_eq!(
            receiver_before_property(line, property).as_deref(),
            *expected,
            "{line} / {property}"
        );
    }
}

#[test]
fn is_identifier_table() {
    // kills: completion.rs:1466 -> true / false
    // kills: completion.rs:1470 || -> && (x2), == -> != (x2)
    // kills: completion.rs:1471 && -> ||, || -> && (x2), == -> != (x2)
    let rows: &[(&str, bool)] = &[
        ("a", true),
        ("Abc", true),
        ("_x", true),
        ("$y", true),
        ("a1", true),
        ("a_b", true),
        ("a$b", true),
        ("", false),
        ("1a", false),
        ("a-b", false),
        ("a b", false),
        ("-a", false),
        ("a.b", false),
    ];
    for (value, expected) in rows {
        assert_eq!(is_identifier(value), *expected, "{value:?}");
    }
}

// ---------------------------------------------------------------------------
// FIN-029: evidence hint extraction
// ---------------------------------------------------------------------------

#[test]
fn push_normalized_evidence_hint_token_filters_and_dedups() {
    // kills: completion.rs:1596 push_normalized_evidence_hint_token -> ()
    // kills: completion.rs:1596,1597 delete !
    // kills: completion.rs:1597,1598 && -> ||
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    push_normalized_evidence_hint_token("ball", &mut seen, &mut out);
    assert_eq!(out, vec![s("ball")]);
    // duplicate
    push_normalized_evidence_hint_token("ball", &mut seen, &mut out);
    assert_eq!(out, vec![s("ball")]);
    // empty
    push_normalized_evidence_hint_token("", &mut seen, &mut out);
    assert_eq!(out, vec![s("ball")]);
    // stopwords (ascii and japanese)
    push_normalized_evidence_hint_token(NEXT, &mut seen, &mut out);
    push_normalized_evidence_hint_token("ゲーム", &mut seen, &mut out);
    assert_eq!(out, vec![s("ball")]);
    // a second, different token is still accepted
    push_normalized_evidence_hint_token("paddle", &mut seen, &mut out);
    assert_eq!(out, vec![s("ball"), s("paddle")]);
}

#[test]
fn ascii_goal_words_table() {
    // kills: completion.rs:1614 ascii_goal_words -> vec![] / vec![""] / vec!["xyzzy"]
    // kills: completion.rs:1628 push_ascii_goal_word -> ()
    // kills: completion.rs:1628 >= -> < (four letters qualify, three do not)
    let rows: &[(&str, &[&str])] = &[
        (
            "Draw a sprite; cat ball2cube game: NEXTJS",
            &["draw", "sprite", "ball", "cube"],
        ),
        ("cat dog", &[]),
        ("ball Ball BALL", &["ball"]),
        ("trailing ball", &["trailing", "ball"]),
        ("", &[]),
    ];
    for (goal, expected) in rows {
        assert_eq!(ascii_goal_words(goal), *expected, "{goal:?}");
    }
}

#[test]
fn evidence_hint_tokens_for_goal_combines_ascii_and_katakana() {
    // kills: completion.rs:1614 (ascii part), 1596/1597/1598 (filtering)
    assert_eq!(
        evidence_hint_tokens_for_goal("Draw a sprite game with ブロック"),
        vec![
            s("draw"),
            s("sprite"),
            s("ブロック"),
            s("block"),
            s("brick")
        ]
    );
    assert_eq!(
        evidence_hint_tokens_for_goal("スペースインベーダー"),
        vec![
            s("スペースインベーダー"),
            s("インベーダー"),
            s("invader"),
            s("invaders"),
        ]
    );
    assert_eq!(
        evidence_hint_tokens_for_goal("ゲーム nextjs"),
        Vec::<String>::new()
    );
}

#[test]
fn evidence_hint_stopword_table() {
    // kills: completion.rs:1673 -> true / false
    // kills: completion.rs:1678 == -> !=
    for token in [NEXT, "build", "ゲーム", "スペース"] {
        assert!(evidence_hint_stopword(token), "{token}");
    }
    for token in ["ball", "paddle", "ブロック", ""] {
        assert!(!evidence_hint_stopword(token), "{token}");
    }
}

// ---------------------------------------------------------------------------
// FIN-030: evidence repair guidance / dependency / target classification
// ---------------------------------------------------------------------------

#[test]
fn evidence_repair_guidance_table() {
    // kills: completion.rs:1718 evidence_repair_guidance -> vec![] / vec![""] / vec!["xyzzy"]
    // kills: completion.rs:1720,1721,1729,1737 || -> &&
    // kills: completion.rs:1757 && -> ||
    let rows: &[(&str, Option<&str>)] = &[
        (
            "test_artifact",
            Some("For JavaScript/TypeScript, add an assertion-backed"),
        ),
        (
            "bound_verify_command",
            Some("For JavaScript/TypeScript, add an assertion-backed"),
        ),
        (
            "node_smoke_without_assertion",
            Some("For JavaScript/TypeScript, add an assertion-backed"),
        ),
        (
            "non_zero_test_or_assertion_evidence",
            Some("For Rust, add at least one real `#[test]`"),
        ),
        (
            "cargo_test_without_test_evidence",
            Some("For Rust, add at least one real `#[test]`"),
        ),
        (
            "interactive_ui_source_evidence",
            Some("For interactive UI work, implement state changes"),
        ),
        (
            "non_static_screen_evidence",
            Some("For interactive UI work, implement state changes"),
        ),
        (
            "challenge_or_adversary_evidence",
            Some("For challenge_or_adversary_evidence, edit the task implementation artifact"),
        ),
        (
            "failure_or_collision_evidence",
            Some("For failure_or_collision_evidence, edit the task implementation artifact"),
        ),
        (
            "restart_or_recoverable_state_evidence",
            Some(
                "For restart_or_recoverable_state_evidence, edit the task implementation artifact",
            ),
        ),
        (
            "restart_or_recoverable_state_evidence terminal_state_not_reached",
            Some("For restart_or_recoverable_state_evidence partial verification,"),
        ),
        ("terminal_state_not_reached", None),
        (
            "score_or_progression_evidence",
            Some("For score_or_progression_evidence, edit the task implementation artifact"),
        ),
        (
            "stateful_update_evidence",
            Some("For stateful_update_evidence, edit the task implementation artifact"),
        ),
        (
            "user_input_handler_evidence",
            Some("For user_input_handler_evidence, edit the task implementation artifact"),
        ),
        ("nothing relevant", None),
    ];
    for (failure, expected_prefix) in rows {
        let lines = evidence_repair_guidance(failure);
        match expected_prefix {
            Some(prefix) => {
                assert_eq!(lines.len(), 1, "{failure}: {lines:?}");
                assert!(lines[0].starts_with(prefix), "{failure}: {lines:?}");
            }
            None => assert!(lines.is_empty(), "{failure}: {lines:?}"),
        }
    }
    // Several evidence kinds in one failure produce one line each, in order.
    let combined = evidence_repair_guidance("test_artifact cargo_test_without_test_evidence");
    assert_eq!(combined.len(), 2);
    assert!(combined[0].starts_with("For JavaScript/TypeScript"));
    assert!(combined[1].starts_with("For Rust"));
}

#[test]
fn is_dependency_missing_error_table() {
    // kills: completion.rs:1791 -> true / false
    // kills: completion.rs:1799 delete !
    // kills: completion.rs:1805 || -> &&
    // kills: completion.rs:1808 || -> &&
    let rows: &[(&str, &str, bool)] = &[
        ("node app.js", "bash: node: command not found", true),
        ("npm test", "sh: 1: npm: not found: npm", true),
        (
            "python3 x.py",
            "python3: can't open file: No such file or directory",
            true,
        ),
        ("", "failed to spawn command", true),
        ("node app.js", "everything is fine", false),
        (
            "node app.js",
            "AssertionError: command not found in output",
            false,
        ),
        ("node app.js", "node: command not found in registry", false),
        ("python3 x.py", "sh: foo: command not found", false),
    ];
    for (command, reason, expected) in rows {
        assert_eq!(
            is_dependency_missing_error(command, reason),
            *expected,
            "{command} / {reason}"
        );
    }
}

#[test]
fn target_candidates_from_failure_table() {
    // kills: completion.rs:1812 -> BTreeSet::new() / {""} / {"xyzzy"}
    // kills: completion.rs:1815 || -> && (x3), == -> != (x3)
    // kills: completion.rs:1821 && -> ||
    let set = |items: &[&str]| items.iter().map(|i| s(i)).collect::<BTreeSet<_>>();
    let rows: &[(&str, &str, BTreeSet<String>)] = &[
        (
            "python3 -m unittest tests/test_a.py",
            "File \"src/b.py\", line 3",
            set(&["src/b.py", "tests/test_a.py"]),
        ),
        ("run", "c.js:12:5 failed", set(&["c.js"])),
        ("run", "error in 'd.ts' file", set(&["d.ts"])),
        ("run", "x:e.py", set(&["e.py"])),
        ("run", "see \"f.rs\"", set(&["f.rs"])),
        ("run", "see\tg.go\nand (h.java),", set(&["g.go", "h.java"])),
        ("run", "see ../escape.py and /abs/path.py", set(&[])),
        ("run", "README.md notes.txt", set(&[])),
        ("", "", set(&[])),
    ];
    for (command, reason, expected) in rows {
        assert_eq!(
            &target_candidates_from_failure(command, reason),
            expected,
            "{command} / {reason}"
        );
    }
}

#[test]
fn looks_like_implementation_target_path_table() {
    // kills: completion.rs:1841 -> true / false
    // kills: completion.rs:1843..1853 && -> || (non-source file must stay false)
    // kills: completion.rs:1843..1853 delete ! (each exclusion keeps a real source file true)
    let rows: &[(&str, bool)] = &[
        ("src/app/page.tsx", true),
        ("main.py", true),
        ("src/lib.rs", true),
        ("SRC/APP/PAGE.TSX", false), // extension check is case-sensitive
        ("notes.txt", false),
        ("README.md", false),
        ("styles/app.css", false),
        ("src/types/api.d.ts", false),
        ("src/app/layout.tsx", false),
        ("src/app/layout.jsx", false),
        ("tsconfig.json", false),
        ("next.config.js", false),
        ("tailwind.config.ts", false),
        ("src/app/page.test.tsx", false),
        ("src/evidence.ts", false),
        ("", false),
    ];
    for (path, expected) in rows {
        assert_eq!(
            looks_like_implementation_target_path(path),
            *expected,
            "{path}"
        );
    }
}

#[test]
fn looks_like_setup_target_path_table() {
    // kills: completion.rs:1857 -> true / false
    // kills: completion.rs:1868..1875 || -> && (one row per alternative)
    let names = [
        "package.json",
        "package-lock.json",
        "pnpm-lock.yaml",
        "yarn.lock",
        "cargo.toml",
        "cargo.lock",
        "pyproject.toml",
        "requirements.txt",
    ];
    for name in names {
        assert!(looks_like_setup_target_path(name), "{name}");
        assert!(
            looks_like_setup_target_path(&name.to_ascii_uppercase()),
            "{name}"
        );
        let nested = format!("web/{name}");
        assert!(looks_like_setup_target_path(&nested), "{nested}");
        let lookalike = format!("web/x{name}");
        assert!(!looks_like_setup_target_path(&lookalike), "{lookalike}");
    }
    assert!(!looks_like_setup_target_path("src/app/page.tsx"));
    assert!(!looks_like_setup_target_path(""));
}

#[test]
fn looks_like_framework_target_path_table() {
    // kills: completion.rs:1879 -> true / false
    // kills: completion.rs:1881..1887 || -> && (one row per alternative)
    for path in [
        "tsconfig.json",
        "next.config.js",
        "tailwind.config.ts",
        "postcss.config.js",
        "src/global.d.ts",
        "src/app/globals.css",
        "app/layout.tsx",
        "app/layout.jsx",
        "APP/LAYOUT.TSX",
    ] {
        assert!(looks_like_framework_target_path(path), "{path}");
    }
    for path in [
        "src/app/page.tsx",
        "src/global.ts",
        "src/app/styles.css",
        "",
    ] {
        assert!(!looks_like_framework_target_path(path), "{path}");
    }
}

#[test]
fn looks_like_evidence_target_path_table() {
    // kills: completion.rs:1891 -> true / false
    // kills: completion.rs:1893..1897 || -> && (one row per alternative)
    for path in [
        "src/calc.test.ts",
        "src/calc.spec.ts",
        "src/__tests__/calc.ts",
        "src/calc.snap",
        "docs/evidence/run.md",
        "README.md",
        "SRC/CALC.TEST.TS",
    ] {
        assert!(looks_like_evidence_target_path(path), "{path}");
    }
    for path in ["src/calc.ts", "src/app/page.tsx", "notes.txt", ""] {
        assert!(!looks_like_evidence_target_path(path), "{path}");
    }
}

#[test]
fn assertion_excerpt_table() {
    // kills: completion.rs:1901 -> None / Some("") / Some("xyzzy")
    // kills: completion.rs:1906..1909 || -> && (one row per keyword)
    // kills: completion.rs:1912 delete !
    for (reason, expected) in [
        ("AssertionError: boom", "AssertionError: boom"),
        ("  Expected 3 \n", "Expected 3"),
        ("the actual value", "the actual value"),
        ("  left: 1", "left: 1"),
        ("right: 2  ", "right: 2"),
        ("ASSERT FAILED", "ASSERT FAILED"),
    ] {
        assert_eq!(
            assertion_excerpt(reason).as_deref(),
            Some(expected),
            "{reason}"
        );
    }
    // Blank and unrelated lines are dropped; only the first four matches stay.
    let reason = "noise\n\nassert a\nexpected b\nactual c\nleft: d\nright: e\nmore noise";
    assert_eq!(
        assertion_excerpt(reason).as_deref(),
        Some("assert a\nexpected b\nactual c\nleft: d")
    );
    assert_eq!(assertion_excerpt("nothing to see"), None);
    assert_eq!(assertion_excerpt(""), None);
}

#[test]
fn pytest_style_under_unittest_detects_free_functions() {
    // kills: completion.rs:2012 -> true / false
    // kills: completion.rs:2019 && -> ||, delete !
    let dir = tempfile::tempdir().unwrap();
    let write = |name: &str, body: &str| std::fs::write(dir.path().join(name), body).unwrap();
    write("free.py", "def test_add():\n    assert 1 + 1 == 2\n");
    write(
        "klass.py",
        "import unittest\nclass T(unittest.TestCase):\n    def test_add(self):\n        pass\n",
    );
    write("plain.py", "x = 1\n");
    let cmd = |file: &str| format!("python3 -m unittest {file}");
    assert!(pytest_style_under_unittest(dir.path(), &cmd("free.py")));
    assert!(!pytest_style_under_unittest(dir.path(), &cmd("klass.py")));
    assert!(!pytest_style_under_unittest(dir.path(), &cmd("plain.py")));
    assert!(!pytest_style_under_unittest(dir.path(), &cmd("missing.py")));
    assert!(!pytest_style_under_unittest(
        dir.path(),
        "python3 -m unittest"
    ));
    assert!(!pytest_style_under_unittest(dir.path(), "python3 free.py"));
}

#[test]
fn unittest_target_path_table() {
    // kills: completion.rs:2023 -> None / Some("") / Some("xyzzy")
    // kills: completion.rs:2026 && -> ||, == -> != (x2)
    // kills: completion.rs:2029 + -> -, + -> *
    // kills: completion.rs:2031 && -> ||, delete !
    let rows: &[(&str, Option<&str>)] = &[
        (
            "python3 -m unittest tests/test_a.py",
            Some("tests/test_a.py"),
        ),
        (
            "python3 -m unittest -v tests/test_a.py",
            Some("tests/test_a.py"),
        ),
        (
            "env FOO=1 PYTHONPATH=. python3 -m unittest tests/x.py",
            Some("tests/x.py"),
        ),
        ("python3 -m unittest discover tests", None),
        ("python3 -m unittest", None),
        ("python3 -m pytest tests/a.py", None),
        ("python3 unittest -m foo tests/x.py", None),
        ("python3 tests/a.py", None),
    ];
    for (command, expected) in rows {
        assert_eq!(unittest_target_path(command), *expected, "{command}");
    }
}
