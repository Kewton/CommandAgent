//! Pins the observable verdict of the Node failure-check recognizer through its
//! existing entries. Every case flips one decision (tokenize, unsafe-token
//! rejection, assert binding, statement walk) so a change to any of them fails.

use super::super::super::{SourceFile, VerifyCommandKind, WorkspaceEvidence};
use super::super::node_command_kind;
use super::has_failure_check;

const BASE: &str = "const assert = require('assert'); assert.ok(x);";

fn with_assert(body: &str) -> String {
    format!("const assert = require('assert'); {body}")
}

fn check_all(cases: &[(&str, bool)]) {
    let wrong = cases
        .iter()
        .filter(|(source, expected)| has_failure_check(source) != *expected)
        .map(|(source, expected)| format!("expected {expected}: {source}"))
        .collect::<Vec<_>>();
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn representative_failure_checks_are_recognized() {
    check_all(&[
        (BASE, true),
        (
            "import assert from 'node:assert';\nassert.strictEqual(a, b);",
            true,
        ),
        (
            "const assert = require(\"node:assert/strict\"); assert(x);",
            true,
        ),
        ("const assert=require('assert');assert.ok(x)", true),
        (
            "const assert = require('assert');\nassert\n.\nok\n(x)",
            true,
        ),
        // No automatic semicolon insertion: the binding must end in `;` or EOF.
        ("const assert = require('assert')\nassert.ok(x)", false),
        (
            "#!/usr/bin/env node\nconst assert = require('assert'); assert.ok(x)",
            true,
        ),
        (
            "// catch\nconst assert = require('assert'); /* eval */ assert.ok(x);",
            true,
        ),
        ("if (!ok) { process.exit(1); }", true),
        ("if (ok) process.exit(2); else process.exit(0);", true),
        ("if (ok) { foo(); } else { process.exit(1); }", true),
        ("if (ok) foo(); else process.exit(3);", true),
        ("if (a) { if (b) { process.exit(1); } }", true),
        ("if (a) { } else if (b) { process.exit(1); }", true),
        ("if (x === null) process.exit(255);", true),
        ("if (x === \"true\") process.exit(1);", true),
        (&with_assert("if (x) { assert.ok(y); }"), true),
        (&with_assert("if (a) assert.ok(x);"), true),
        (&with_assert("if (a) { } assert.ok(x);"), true),
    ]);
}

#[test]
fn every_assert_method_is_a_check_and_unknown_methods_are_not() {
    for method in [
        "ok",
        "equal",
        "strictEqual",
        "deepEqual",
        "deepStrictEqual",
        "notEqual",
        "notStrictEqual",
        "notDeepEqual",
        "notDeepStrictEqual",
        "throws",
        "doesNotThrow",
        "ifError",
        "match",
        "doesNotMatch",
        "fail",
    ] {
        let source = with_assert(&format!("assert.{method}(x);"));
        assert!(has_failure_check(&source), "{method}");
    }
    assert!(!has_failure_check(&with_assert("assert.bogus(x);")));
    assert!(!has_failure_check(&with_assert("assert.rejects(x);")));
    assert!(!has_failure_check(&with_assert("assert.ok;")));
    assert!(!has_failure_check(&with_assert("assert[\"ok\"](x);")));
}

#[test]
fn unsafe_tokens_reject_a_script_that_otherwise_passes() {
    assert!(has_failure_check(BASE));
    for token in [
        "catch",
        "finally",
        "eval",
        "Function",
        "constructor",
        "global",
        "globalThis",
        "/",
        "\\",
    ] {
        let source = format!("{BASE} a {token} b;");
        assert!(!has_failure_check(&source), "{token}");
        let leading = format!("{token} {BASE}");
        assert!(!has_failure_check(&leading), "{token}");
    }
    for source in [
        "const assert = require('assert'); try { a(); } catch (e) { } assert.ok(x);",
        "const assert = require('assert'); try { a(); } finally { } assert.ok(x);",
        "const assert = require('assert'); assert.ok(eval('1'));",
        "const assert = require('assert'); assert.ok(new Function('return 1')());",
        "const assert = require('assert'); assert.ok(/ab+c/.test(x));",
        "const assert = require('assert'); assert.ok(a / b);",
    ] {
        assert!(!has_failure_check(source), "{source}");
    }
}

#[test]
fn unsafe_words_inside_literals_and_identifiers_are_opaque() {
    for word in [
        "catch",
        "finally",
        "eval",
        "Function",
        "constructor",
        "global",
        "globalThis",
        "process",
        "assert",
    ] {
        assert!(
            has_failure_check(&with_assert(&format!("assert.ok(\"{word}\");"))),
            "{word}"
        );
        assert!(
            has_failure_check(&with_assert(&format!("assert.ok('{word}');"))),
            "{word}"
        );
    }
    check_all(&[
        (
            &with_assert("assert.equal(\"catch eval process / \\\\\", y);"),
            true,
        ),
        (&with_assert("assert.ok('it\\'s');"), true),
        (&with_assert("assert.ok(evaluate(x));"), true),
        (&with_assert("assert.ok(catchy + _finally + $eval);"), true),
        (
            &with_assert("assert.ok(globalThisX + Functions + constructors);"),
            true,
        ),
        (&with_assert("const process$ = 1; assert.ok(x);"), true),
        (&with_assert("const requirex = 1; assert.ok(x);"), true),
    ]);
}

#[test]
fn shadowed_process_and_require_bindings_reject() {
    assert!(has_failure_check(&with_assert("assert.ok(x);")));
    for declaration in ["const", "let", "var", "function", "class", "import"] {
        for name in ["process", "require"] {
            let source = format!("{BASE} {declaration} {name};");
            assert!(!has_failure_check(&source), "{declaration} {name}");
        }
    }
    assert!(!has_failure_check(&with_assert(
        "function require() { return {}; } assert.ok(x);"
    )));
}

#[test]
fn process_mutation_and_indirect_use_reject() {
    assert!(has_failure_check("if (a) { process.exit(1); }"));
    for source in [
        "if (a) { process.exit(1); } process.exit = f;",
        "if (a) { process.exit(1); } process.on('exit', f);",
        "if (a) { process.exit(1); } process.once('exit', f);",
        "if (a) { process.exit(1); } process.argv.length;",
        "if (a) { process.exit(1); } process.exitCode = 1;",
        "if (a) { process.exit(1); } process.exit;",
        "if (a) { process.exit(1); } const p = process;",
        "if (a) { process.exit(1); } process[\"exit\"](1);",
        "if (a) { process.exit(1); } process",
        "process.on('exit', f); if (a) { process.exit(1); }",
        "const {exit} = process; if (a) { exit(1); }",
    ] {
        assert!(!has_failure_check(source), "{source}");
    }
}

#[test]
fn process_exit_needs_a_conditional_nonzero_u8_literal_call() {
    check_all(&[
        ("process.exit(1);", false),
        ("if (x) process.exit(0);", false),
        ("if (x) process.exit(256);", false),
        ("if (x) process.exit(abc);", false),
        ("if (x) process.exit();", false),
        ("if (x) process.exit(1, 2);", false),
        ("if (x) process.exit(-1);", false),
        ("if (x) process.exit(1.5);", false),
        ("if (x) process.exit(1", false),
        ("if (x) process.exit(1);", true),
        ("if (x) process.exit(10);", true),
        ("if (x) process.exit(255);", true),
        ("if (x) process.exit( 1 );", true),
        // The first process.exit decides the whole branch body.
        ("if (x) { process.exit(0); process.exit(1); }", false),
        (
            &with_assert("if (x) { process.exit(0); assert.ok(y); }"),
            false,
        ),
        (
            "if (x) { process.exit(0); } if (y) { process.exit(1); }",
            true,
        ),
    ]);
}

#[test]
fn return_and_throw_stop_the_walk() {
    check_all(&[
        ("return;", false),
        (&with_assert("throw 1; assert.ok(x);"), false),
        (&with_assert("return; assert.ok(x);"), false),
        (&with_assert("assert.ok(x); throw 1;"), true),
        (&with_assert("assert.ok(x); return;"), true),
        (&with_assert("if (x) { throw 1; } assert.ok(y);"), true),
        (
            &with_assert("if (x) throw new Error(\"x\"); assert.ok(y);"),
            true,
        ),
        (&with_assert("if (x) { return; } assert.ok(y);"), true),
        (&with_assert("if (x) { throw 1; }"), false),
    ]);
}

#[test]
fn if_conditions_must_be_meaningful_and_not_literal_booleans() {
    check_all(&[
        (&with_assert("if (true) { assert.ok(x); }"), false),
        (&with_assert("if (false) { assert.ok(x); }"), false),
        (
            &with_assert("if (false && actual.ok) { assert.ok(x); }"),
            false,
        ),
        (&with_assert("if (x && true) { assert.ok(y); }"), false),
        (&with_assert("if (x || false) { assert.ok(y); }"), false),
        (&with_assert("if (null) { assert.ok(x); }"), false),
        (&with_assert("if (undefined) { assert.ok(x); }"), false),
        (&with_assert("if (1) { assert.ok(x); }"), false),
        (&with_assert("if (\"abc\") { assert.ok(x); }"), false),
        (&with_assert("if () { assert.ok(x); }"), false),
        // A rejected condition ends the whole walk, even before a later assert.
        (&with_assert("if (1) { } assert.ok(x);"), false),
        (&with_assert("if (true) { } assert.ok(x);"), false),
        (&with_assert("if (ok) { } assert.ok(x);"), true),
        (&with_assert("if (x === undefined) { assert.ok(y); }"), true),
        ("if ok process.exit(1);", false),
    ]);
}

#[test]
fn unterminated_branches_conditions_and_else_reject_in_order() {
    check_all(&[
        ("if (y { } process.exit(1);", false),
        (&with_assert("if (y { } assert.ok(x);"), false),
        (&with_assert("if (a) { assert.ok(x);"), false),
        (&with_assert("if (a) { } else { assert.ok(x);"), false),
        // The then-body is judged before the else-branch is parsed.
        (&with_assert("if (a) { assert.ok(x); } else {"), true),
        (&with_assert("if (a) { assert.ok(x); } else"), true),
        (&with_assert("if (a) { } else"), false),
        (&with_assert("if (a) { assert.ok(x); }"), true),
    ]);
}

#[test]
fn assertions_in_uncalled_or_compound_code_do_not_count() {
    check_all(&[
        (&with_assert("function f() { assert.ok(x); }"), false),
        (&with_assert("foo(() => { assert.ok(x); });"), false),
        (&with_assert("[1].forEach((v) => assert.ok(v));"), false),
        (&with_assert("const f = () => { assert.ok(x); };"), false),
        (&with_assert("(function () { assert.ok(x); })();"), false),
        (&with_assert("x = { a: assert.ok(y) };"), false),
        // A block with no `;` after it swallows the next statement.
        (
            &with_assert("function f() { assert.ok(x); } assert.ok(y);"),
            false,
        ),
        (
            &with_assert("function f() { assert.ok(x); }; assert.ok(y);"),
            true,
        ),
        (&with_assert("; ; assert.ok(y);"), true),
        (&with_assert(""), false),
    ]);
}

#[test]
fn assert_requires_a_top_level_direct_binding() {
    check_all(&[
        ("assert.ok(x);", false),
        ("if (x) { assert.ok(y); }", false),
        ("import assert from 'assert'; assert.ok(x);", true),
        (
            "import assert from \"node:assert/strict\"; assert.ok(x);",
            true,
        ),
        ("import assert from 'fs'; assert.ok(x);", false),
        ("import assert from 'assert/strict'; assert.ok(x);", false),
        ("import assert, { x } from 'assert'; assert.ok(y);", false),
        ("import { assert } from 'assert'; assert.ok(y);", false),
        ("import assert 'assert'; assert.ok(y);", false),
        ("{ import assert from 'assert'; } assert.ok(y);", false),
        ("const assert = require('fs'); assert.ok(x);", false),
        ("const assert = require('assert', 1); assert.ok(x);", false),
        (
            "const assert = require('assert').strict; assert.ok(x);",
            false,
        ),
        ("const assert = require('assert') assert.ok(x);", false),
        ("const assert = require('assert');assert.ok(x)", true),
        ("const assert = require('assert')", false),
        ("const assert = require(assert); assert.ok(x);", false),
        ("const assert = req('assert'); assert.ok(x);", false),
        ("const assert = 1; assert.ok(x);", false),
        ("let assert = require('assert'); assert.ok(x);", false),
        ("var assert = require('assert'); assert.ok(x);", false),
        ("function assert() {} assert.ok(x);", false),
        ("class assert {} assert.ok(x);", false),
        ("{ const assert = require('assert'); } assert.ok(x);", false),
        (
            "(function () { const assert = require('assert'); })(); assert.ok(x);",
            false,
        ),
        ("[ const assert = require('assert'); ] assert.ok(x);", false),
        ("{ } const assert = require('assert'); assert.ok(x);", true),
        ("} const assert = require('assert'); assert.ok(x);", true),
    ]);
}

#[test]
fn assert_aliasing_and_overriding_reject() {
    for source in [
        "const a = assert; assert.ok(x);",
        "foo(assert); assert.ok(x);",
        "assert = null; assert.ok(x);",
        "assert.ok = f; assert.ok(x);",
        "assert.strictEqual = f; assert.ok(x);",
        "x.assert = f; assert.ok(y);",
        "assert; assert.ok(x);",
        "assert.ok(x); assert = null;",
        "assert.ok(x); assert.ok = f;",
        "assert.ok(x); const b = assert;",
        "assert.ok(x); const assert = require('fs');",
    ] {
        assert!(!has_failure_check(&with_assert(source)), "{source}");
    }
    // Rebinding to the same module is still a counterfeit-free binding.
    assert!(has_failure_check(&with_assert(
        "const assert = require('node:assert'); assert.ok(x);"
    )));
}

#[test]
fn nesting_depth_is_limited_to_sixty_four() {
    let open = |depth: usize| "(".repeat(depth) + &")".repeat(depth);
    assert!(has_failure_check(&format!("{} {BASE}", open(64))));
    assert!(!has_failure_check(&format!("{} {BASE}", open(65))));
    let open_only = |depth: usize| "{".repeat(depth);
    assert!(!has_failure_check(&format!("{} {BASE}", open_only(65))));
}

#[test]
fn comments_shebang_and_empty_inputs() {
    check_all(&[
        ("", false),
        ("   \n\t ", false),
        ("// only a comment", false),
        ("/* only a comment */", false),
        ("#!", false),
        ("#!/usr/bin/env node", false),
        ("#!/usr/bin/env node\n", false),
        ("#!/usr/bin/env node\nif (a) { process.exit(1); }", true),
        // Only a leading shebang is stripped.
        (
            "x;\n#!/usr/bin/env node\nif (a) { process.exit(1); }",
            false,
        ),
        (&with_assert("// assert.ok(x)"), false),
        (&with_assert("/* assert.ok(x) */"), false),
        (&with_assert("assert.ok(x); // catch"), true),
        (&with_assert("assert.ok(x); /* a / b */"), true),
        ("if (a) { process.exit(1); } // process.exit = f", true),
    ]);
}

#[test]
fn templates_regexps_and_unterminated_input_are_uncertain_so_false() {
    check_all(&[
        (&with_assert("assert.ok(`x`);"), false),
        (&with_assert("assert.ok(`${x}`);"), false),
        (&with_assert("assert.ok(\"`\");"), false),
        (&with_assert("assert.ok('`');"), false),
        ("if (a) { process.exit(1); } const t = `x`;", false),
        (&with_assert("assert.ok(/ab+c/.test(x));"), false),
        (&with_assert("assert.ok(\"abc);"), false),
        (&with_assert("assert.ok('abc);"), false),
        (&with_assert("assert.ok(x); /* unterminated"), false),
        (&with_assert("assert.ok(x); `"), false),
        (&with_assert("assert.ok(x);\\"), false),
        // The same inputs with the uncertainty removed pass.
        (&with_assert("assert.ok(\"abc\");"), true),
        (&with_assert("assert.ok(x); /* closed */"), true),
    ]);
}

#[test]
fn bracket_skipping_handles_unbalanced_and_nested_statements() {
    check_all(&[
        (&with_assert("foo((((; assert.ok(x);"), false),
        (&with_assert("foo(a, [b, {c: d}]); assert.ok(x);"), true),
        (&with_assert("foo(a, [b, {c: d}); assert.ok(x);"), false),
        (&with_assert("x = ( ; assert.ok(y)"), false),
        (&with_assert("x = ) ; assert.ok(y)"), true),
        (&with_assert("{ a; b } assert.ok(y);"), false),
        (&with_assert("{ a; b }; assert.ok(y);"), true),
        (&with_assert("{ a; { b; } }; assert.ok(y);"), true),
        (&with_assert("{ a; assert.ok(y);"), false),
    ]);
}

fn command_kind(command: &str, script: &str) -> VerifyCommandKind {
    let workspace = WorkspaceEvidence {
        source_files: vec![SourceFile::new("check.js".into(), script.into())],
        ..Default::default()
    };
    node_command_kind(command, &workspace).expect("node command")
}

#[test]
fn node_command_kind_uses_the_recognizer_verdict() {
    let weak = || VerifyCommandKind::Weak("node_smoke_without_assertion".into());
    assert_eq!(command_kind("node check.js", BASE), VerifyCommandKind::Test);
    assert_eq!(
        command_kind("node ./check.js", "if (a) { process.exit(1); }"),
        VerifyCommandKind::Test
    );
    assert_eq!(command_kind("node check.js", "console.log('ok');"), weak());
    assert_eq!(
        command_kind(
            "node check.js",
            "try { a(); } catch (e) { } if (a) { process.exit(1); }"
        ),
        weak()
    );
    assert_eq!(
        command_kind(
            "node check.js",
            "const assert = require('assert'); assert.ok(`x`);"
        ),
        weak()
    );
    assert_eq!(
        command_kind("node check.js", "if (a) { process.exit(0); }"),
        weak()
    );
    assert_eq!(command_kind("node absent.js", BASE), weak());
    assert_eq!(
        command_kind("node -e \"if (a) { process.exit(1); }\"", ""),
        VerifyCommandKind::Test
    );
    assert_eq!(
        command_kind("node --eval=\"if (a) { process.exit(1); }\"", ""),
        VerifyCommandKind::Test
    );
    assert_eq!(
        command_kind("node -p \"if (a) { process.exit(1); }\"", ""),
        VerifyCommandKind::Test
    );
    assert_eq!(
        command_kind("node --print=\"if (a) { process.exit(1); }\"", ""),
        weak()
    );
    assert_eq!(
        command_kind("node -e \"if (a) { eval(1); process.exit(1); }\"", ""),
        weak()
    );
    assert_eq!(command_kind("node -e \"console.log(1)\"", ""), weak());
}
