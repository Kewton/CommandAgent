//! Pins the bounded lexer's tokens and refusals through its existing entries
//! (`lex` and `Source::parse`) so a structural move cannot change them.
use super::lex;
use crate::planner::recovery_observation_policy::nextjs_outputs::syntax::Source;

fn render_lex(text: &str) -> String {
    match lex(text) {
        None => "None".to_string(),
        Some((tokens, unsafe_names, receivers)) => {
            format!("{tokens:?} unsafe={unsafe_names:?} receivers={receivers:?}")
        }
    }
}

fn render_source(text: &str) -> String {
    match Source::parse(text) {
        None => "None".to_string(),
        Some(source) => format!(
            "{:?} unsafe={:?} opaque={:?}",
            source.tokens, source.unsafe_names, source.opaque_names
        ),
    }
}

fn check(cases: &[(&str, &str, &str)], render: fn(&str) -> String) {
    let mismatches = cases
        .iter()
        .filter_map(|(name, input, expected)| {
            let actual = render(input);
            (actual != *expected).then(|| format!("{name}: {actual:?}"))
        })
        .collect::<Vec<_>>();
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
}

#[test]
fn lex_pins_tokens_for_strings_comments_regexps_and_templates() {
    check(CASES, render_lex);
}

#[test]
fn lex_refuses_dynamic_evaluation_escapes_and_jsx() {
    check(REFUSALS, render_lex);
}

#[test]
fn source_parse_pins_tokens_and_unsafe_names() {
    check(SOURCES, render_source);
}

const CASES: &[(&str, &str, &str)] = &[
    (
        "plain_statements",
        "const a = \"x\"; foo('y', 1);",
        "[Word(\"const\"), Word(\"a\"), Symbol('='), String(\"x\"), Symbol(';'), Word(\"foo\"), Symbol('('), String(\"y\"), Symbol(','), Symbol('1'), Symbol(')'), Symbol(';')] unsafe={} receivers={}",
    ),
    (
        "line_comment_then_code",
        "a // note }\nb",
        "[Word(\"a\"), Word(\"b\")] unsafe={} receivers={}",
    ),
    (
        "line_comment_at_eof",
        "a // tail",
        "[Word(\"a\")] unsafe={} receivers={}",
    ),
    (
        "block_comment_newline",
        "a /* x\n y */ b",
        "[Word(\"a\"), Word(\"b\")] unsafe={} receivers={}",
    ),
    (
        "block_comment_star",
        "a /** x **/ b",
        "[Word(\"a\"), Word(\"b\")] unsafe={} receivers={}",
    ),
    (
        "division_after_word",
        "a / b",
        "[Word(\"a\"), Symbol('/'), Word(\"b\")] unsafe={} receivers={}",
    ),
    (
        "division_after_number",
        "1 / 2",
        "[Symbol('1'), Symbol('/'), Symbol('2')] unsafe={} receivers={}",
    ),
    (
        "division_after_paren",
        "(a) / 2",
        "[Symbol('('), Word(\"a\"), Symbol(')'), Symbol('/'), Symbol('2')] unsafe={} receivers={}",
    ),
    (
        "division_after_bracket",
        "a[0] / 2",
        "[Word(\"a\"), Symbol('['), Symbol('0'), Symbol(']'), Symbol('/'), Symbol('2')] unsafe={} receivers={}",
    ),
    (
        "regexp_assignment",
        "const r = /ab+c/gi;",
        "[Word(\"const\"), Word(\"r\"), Symbol('='), Opaque, Symbol(';')] unsafe={\"ab\", \"cgi\"} receivers={}",
    ),
    (
        "regexp_class_with_slash",
        "const r = /[/]x/;",
        "[Word(\"const\"), Word(\"r\"), Symbol('='), Opaque, Symbol(';')] unsafe={\"x\"} receivers={}",
    ),
    (
        "regexp_escape",
        "const r = /a\\/b/;",
        "[Word(\"const\"), Word(\"r\"), Symbol('='), Opaque, Symbol(';')] unsafe={\"a\", \"b\"} receivers={}",
    ),
    (
        "regexp_after_return",
        "return /ab/.test(x);",
        "[Word(\"return\"), Opaque, Symbol('.'), Word(\"test\"), Symbol('('), Word(\"x\"), Symbol(')'), Symbol(';')] unsafe={\"ab\"} receivers={}",
    ),
    (
        "regexp_after_control_paren",
        "if (a) /x/.test(b);",
        "[Word(\"if\"), Symbol('('), Word(\"a\"), Symbol(')'), Opaque, Symbol('.'), Word(\"test\"), Symbol('('), Word(\"b\"), Symbol(')'), Symbol(';')] unsafe={\"x\"} receivers={}",
    ),
    (
        "division_after_call_paren",
        "f(a) / 2",
        "[Word(\"f\"), Symbol('('), Word(\"a\"), Symbol(')'), Symbol('/'), Symbol('2')] unsafe={} receivers={}",
    ),
    (
        "regexp_start",
        "/x/",
        "[Opaque] unsafe={\"x\"} receivers={}",
    ),
    (
        "regexp_flags_hide_names",
        "x = /a(b)/dgu",
        "[Word(\"x\"), Symbol('='), Opaque] unsafe={\"a\", \"b\", \"dgu\"} receivers={}",
    ),
    (
        "escaped_string_opaque",
        "a = 'x\\ny'",
        "[Word(\"a\"), Symbol('='), Opaque] unsafe={} receivers={}",
    ),
    (
        "string_with_slash_comment",
        "a = '// no'",
        "[Word(\"a\"), Symbol('='), String(\"// no\")] unsafe={} receivers={}",
    ),
    (
        "string_double",
        "a = \"q\"",
        "[Word(\"a\"), Symbol('='), String(\"q\")] unsafe={} receivers={}",
    ),
    (
        "template_plain",
        "a = `hello`",
        "[Word(\"a\"), Symbol('='), Opaque] unsafe={} receivers={}",
    ),
    (
        "template_escape",
        "a = `a\\`b`",
        "[Word(\"a\"), Symbol('='), Opaque] unsafe={} receivers={}",
    ),
    (
        "template_process_pid",
        "a = `x${process.pid}y`",
        "[Word(\"a\"), Symbol('='), Opaque] unsafe={} receivers={}",
    ),
    (
        "template_basename",
        "a = `${path.basename(p)}`",
        "[Word(\"a\"), Symbol('='), Opaque] unsafe={} receivers={\"path\"}",
    ),
    (
        "template_other_binding",
        "a = `${foo}`",
        "[Word(\"a\"), Symbol('='), Opaque] unsafe={\"foo\"} receivers={}",
    ),
    (
        "template_property_not_reference",
        "a = `${ROLES.join}`",
        "[Word(\"a\"), Symbol('='), Opaque] unsafe={\"ROLES\"} receivers={}",
    ),
    (
        "template_spread_property",
        "a = `${x...y}`",
        "[Word(\"a\"), Symbol('='), Opaque] unsafe={\"x\", \"y\"} receivers={}",
    ),
    (
        "template_nested",
        "a = `${`${b}`}`",
        "[Word(\"a\"), Symbol('='), Opaque] unsafe={\"b\"} receivers={}",
    ),
    (
        "template_object",
        "a = `${{k: v}}`",
        "[Word(\"a\"), Symbol('='), Opaque] unsafe={\"k\", \"v\"} receivers={}",
    ),
    (
        "template_call_args",
        "a = `${f(g, h)}`",
        "[Word(\"a\"), Symbol('='), Opaque] unsafe={\"f\", \"g\", \"h\"} receivers={}",
    ),
    (
        "angle_less_than",
        "a < b",
        "[Word(\"a\"), Symbol('<'), Word(\"b\")] unsafe={} receivers={}",
    ),
    ("angle_after_return_word", "return <b", "None"),
    (
        "brace_nesting",
        "{ a: { b } }",
        "[Symbol('{'), Word(\"a\"), Symbol(':'), Symbol('{'), Word(\"b\"), Symbol('}'), Symbol('}')] unsafe={} receivers={}",
    ),
    (
        "word_chars",
        "$x_1 y",
        "[Word(\"$x_1\"), Word(\"y\")] unsafe={} receivers={}",
    ),
    (
        "non_ascii_symbol",
        "a ¬ b",
        "[Word(\"a\"), Symbol('¬'), Word(\"b\")] unsafe={} receivers={}",
    ),
    ("empty", "", "[] unsafe={} receivers={}"),
    ("whitespace_only", " \n\t ", "[] unsafe={} receivers={}"),
    (
        "slash_slash_in_regexp_context",
        "x = //c\n1",
        "[Word(\"x\"), Symbol('='), Symbol('1')] unsafe={} receivers={}",
    ),
    (
        "division_after_string",
        "'a' / 2",
        "[String(\"a\"), Symbol('/'), Symbol('2')] unsafe={} receivers={}",
    ),
    (
        "division_after_template",
        "`a` / 2",
        "[Opaque, Symbol('/'), Symbol('2')] unsafe={} receivers={}",
    ),
    (
        "division_after_arrow",
        "f = a => /x/",
        "[Word(\"f\"), Symbol('='), Word(\"a\"), Symbol('='), Symbol('>'), Opaque] unsafe={\"x\"} receivers={}",
    ),
    (
        "division_after_ge",
        "a >= /x/",
        "[Word(\"a\"), Symbol('>'), Symbol('='), Opaque] unsafe={\"x\"} receivers={}",
    ),
    (
        "regexp_after_plus",
        "a + /x/",
        "[Word(\"a\"), Symbol('+'), Opaque] unsafe={\"x\"} receivers={}",
    ),
    (
        "regexp_after_comma",
        "f(a, /x/)",
        "[Word(\"f\"), Symbol('('), Word(\"a\"), Symbol(','), Opaque, Symbol(')')] unsafe={\"x\"} receivers={}",
    ),
    (
        "minus_then_division",
        "a - /x/",
        "[Word(\"a\"), Symbol('-'), Opaque] unsafe={\"x\"} receivers={}",
    ),
    ("newline_before_division_none", "a\n/ 2", "None"),
    (
        "comment_then_division",
        "a /* c */ / 2",
        "[Word(\"a\"), Symbol('/'), Symbol('2')] unsafe={} receivers={}",
    ),
    ("line_comment_then_regexp_newline", "a // c\n/x/", "None"),
    (
        "jsx_less_than_digit",
        "a <1",
        "[Word(\"a\"), Symbol('<'), Symbol('1')] unsafe={} receivers={}",
    ),
    ("jsx_after_yield_word", "yield <b", "None"),
    ("jsx_after_await_word", "await <b", "None"),
    (
        "jsx_after_return_slash_ok",
        "return /x/",
        "[Word(\"return\"), Opaque] unsafe={\"x\"} receivers={}",
    ),
    (
        "block_comment_without_newline_then_regexp",
        "a = /* c */ /x/",
        "[Word(\"a\"), Symbol('='), Opaque] unsafe={\"x\"} receivers={}",
    ),
    (
        "regexp_class_closes",
        "a = /[a]/",
        "[Word(\"a\"), Symbol('='), Opaque] unsafe={\"a\"} receivers={}",
    ),
    (
        "regexp_class_closed_then_slash",
        "a = /[a]b/",
        "[Word(\"a\"), Symbol('='), Opaque] unsafe={\"a\", \"b\"} receivers={}",
    ),
    (
        "regexp_flags_only_ascii",
        "a = /x/g1",
        "[Word(\"a\"), Symbol('='), Opaque, Symbol('1')] unsafe={\"xg\"} receivers={}",
    ),
    (
        "regexp_after_throw",
        "throw /x/",
        "[Word(\"throw\"), Opaque] unsafe={\"x\"} receivers={}",
    ),
    (
        "regexp_after_case",
        "case /x/",
        "[Word(\"case\"), Opaque] unsafe={\"x\"} receivers={}",
    ),
    (
        "regexp_after_delete",
        "delete /x/",
        "[Word(\"delete\"), Opaque] unsafe={\"x\"} receivers={}",
    ),
    (
        "regexp_after_void",
        "void /x/",
        "[Word(\"void\"), Opaque] unsafe={\"x\"} receivers={}",
    ),
    (
        "regexp_after_typeof",
        "typeof /x/",
        "[Word(\"typeof\"), Opaque] unsafe={\"x\"} receivers={}",
    ),
    (
        "regexp_after_new",
        "new /x/",
        "[Word(\"new\"), Opaque] unsafe={\"x\"} receivers={}",
    ),
    (
        "regexp_after_in",
        "a in /x/",
        "[Word(\"a\"), Word(\"in\"), Opaque] unsafe={\"x\"} receivers={}",
    ),
    (
        "regexp_after_instanceof",
        "a instanceof /x/",
        "[Word(\"a\"), Word(\"instanceof\"), Opaque] unsafe={\"x\"} receivers={}",
    ),
    (
        "regexp_after_else",
        "else /x/",
        "[Word(\"else\"), Opaque] unsafe={\"x\"} receivers={}",
    ),
    (
        "regexp_after_do",
        "do /x/",
        "[Word(\"do\"), Opaque] unsafe={\"x\"} receivers={}",
    ),
    (
        "division_after_dotted_keyword",
        "a.typeof / 2",
        "[Word(\"a\"), Symbol('.'), Word(\"typeof\"), Symbol('/'), Symbol('2')] unsafe={} receivers={}",
    ),
    (
        "division_after_in_property",
        "a.in / 2",
        "[Word(\"a\"), Symbol('.'), Word(\"in\"), Symbol('/'), Symbol('2')] unsafe={} receivers={}",
    ),
    (
        "regexp_after_while_paren",
        "while (a) /x/",
        "[Word(\"while\"), Symbol('('), Word(\"a\"), Symbol(')'), Opaque] unsafe={\"x\"} receivers={}",
    ),
    (
        "regexp_after_for_paren",
        "for (a;;) /x/",
        "[Word(\"for\"), Symbol('('), Word(\"a\"), Symbol(';'), Symbol(';'), Symbol(')'), Opaque] unsafe={\"x\"} receivers={}",
    ),
    (
        "regexp_after_with_paren",
        "with (a) /x/",
        "[Word(\"with\"), Symbol('('), Word(\"a\"), Symbol(')'), Opaque] unsafe={\"x\"} receivers={}",
    ),
    (
        "regexp_after_switch_paren",
        "switch (a) /x/",
        "[Word(\"switch\"), Symbol('('), Word(\"a\"), Symbol(')'), Opaque] unsafe={\"x\"} receivers={}",
    ),
    (
        "regexp_after_catch_paren",
        "catch (e) /x/",
        "[Word(\"catch\"), Symbol('('), Word(\"e\"), Symbol(')'), Opaque] unsafe={\"x\"} receivers={}",
    ),
    (
        "regexp_after_for_await_paren",
        "for await (a of b) /x/",
        "[Word(\"for\"), Word(\"await\"), Symbol('('), Word(\"a\"), Word(\"of\"), Word(\"b\"), Symbol(')'), Opaque] unsafe={\"x\"} receivers={}",
    ),
    (
        "division_after_dotted_if_paren",
        "a.if (b) /x/",
        "[Word(\"a\"), Symbol('.'), Word(\"if\"), Symbol('('), Word(\"b\"), Symbol(')'), Symbol('/'), Word(\"x\"), Symbol('/')] unsafe={} receivers={}",
    ),
    (
        "regexp_after_nested_control_paren",
        "if ((a)) /x/",
        "[Word(\"if\"), Symbol('('), Symbol('('), Word(\"a\"), Symbol(')'), Symbol(')'), Opaque] unsafe={\"x\"} receivers={}",
    ),
    (
        "division_after_nested_call_paren",
        "f((a)) / 2",
        "[Word(\"f\"), Symbol('('), Symbol('('), Word(\"a\"), Symbol(')'), Symbol(')'), Symbol('/'), Symbol('2')] unsafe={} receivers={}",
    ),
    (
        "division_after_dotted_for_await",
        "a.for await (b) / 2",
        "None",
    ),
    (
        "division_after_opaque_regexp",
        "/x/ / 2",
        "[Opaque, Symbol('/'), Symbol('2')] unsafe={\"x\"} receivers={}",
    ),
    (
        "division_after_escaped_string",
        "'a\\n' / 2",
        "[Opaque, Symbol('/'), Symbol('2')] unsafe={} receivers={}",
    ),
    (
        "division_after_digit_symbol",
        "a = 1 / 2",
        "[Word(\"a\"), Symbol('='), Symbol('1'), Symbol('/'), Symbol('2')] unsafe={} receivers={}",
    ),
    (
        "minus_then_regexp",
        "a - /x/",
        "[Word(\"a\"), Symbol('-'), Opaque] unsafe={\"x\"} receivers={}",
    ),
    (
        "template_dollar_without_brace",
        "a = `$x`",
        "[Word(\"a\"), Symbol('='), Opaque] unsafe={} receivers={}",
    ),
    (
        "template_dotted_chain",
        "a = `${a.b.c}`",
        "[Word(\"a\"), Symbol('='), Opaque] unsafe={\"a\"} receivers={}",
    ),
    (
        "template_leading_spread",
        "a = `${...a}`",
        "[Word(\"a\"), Symbol('='), Opaque] unsafe={\"a\"} receivers={}",
    ),
    (
        "template_unterminated_text_ok",
        "a = `${b}c${d}`",
        "[Word(\"a\"), Symbol('='), Opaque] unsafe={\"b\", \"d\"} receivers={}",
    ),
    (
        "depth_exactly_64_with_text",
        "a = `x${`y${b}`}`",
        "[Word(\"a\"), Symbol('='), Opaque] unsafe={\"b\"} receivers={}",
    ),
];

const REFUSALS: &[(&str, &str, &str)] = &[
    ("eval_word", "eval(x)", "None"),
    ("function_word", "new Function('a')", "None"),
    ("constructor_word", "a.constructor(b)", "None"),
    ("constructor_string", "a['constructor']", "None"),
    ("eval_string", "a['eval']", "None"),
    ("eval_in_template", "a = `${eval(x)}`", "None"),
    ("eval_in_nested_template", "a = `${`${Function}`}`", "None"),
    (
        "lowercase_function_is_not_refused",
        "function f() {}",
        "[Word(\"function\"), Word(\"f\"), Symbol('('), Symbol(')'), Symbol('{'), Symbol('}')] unsafe={} receivers={}",
    ),
    ("escaped_identifier", "\\u0061 = 1", "None"),
    ("escaped_in_template", "`${\\u0061}`", "None"),
    ("jsx_element", "a = <div>x</div>", "None"),
    ("jsx_close", "</div>", "None"),
    ("jsx_fragment", "a = <>x</>", "None"),
    ("type_assertion", "a = <T>b", "None"),
    ("jsx_after_return", "return <div/>", "None"),
    ("jsx_after_paren", "f(<a/>)", "None"),
    (
        "less_than_after_word",
        "a <b",
        "[Word(\"a\"), Symbol('<'), Word(\"b\")] unsafe={} receivers={}",
    ),
    ("regexp_after_newline_asi", "a\n/x/g.test(b)", "None"),
    ("unterminated_block_comment", "a /* never", "None"),
    ("unterminated_string", "a = 'x", "None"),
    ("string_newline", "a = 'x\ny'", "None"),
    ("string_cr", "a = \"x\ry\"", "None"),
    ("string_escape_at_eof", "a = 'x\\", "None"),
    ("unterminated_template", "a = `x", "None"),
    ("unterminated_interpolation", "a = `${x", "None"),
    ("unterminated_regexp", "a = /x", "None"),
    ("regexp_newline", "a = /x\n/", "None"),
    ("regexp_class_unterminated", "a = /[/", "None"),
    ("unbalanced_open_brace", "{ a", "None"),
    ("unbalanced_close_brace", "a }", "None"),
    ("regexp_after_close_brace", "} /x/", "None"),
    ("regexp_after_bang", "!/x/", "None"),
    ("regexp_after_dot", "a./x/", "None"),
    ("regexp_after_double_plus", "a++ /x/", "None"),
    ("regexp_after_double_minus", "a-- /x/", "None"),
    ("regexp_after_yield", "yield /x/", "None"),
    ("regexp_after_await", "await /x/", "None"),
    ("regexp_after_gt", "a > /x/", "None"),
    (
        "division_after_dot_return",
        "a.return /x/",
        "[Word(\"a\"), Symbol('.'), Word(\"return\"), Symbol('/'), Word(\"x\"), Symbol('/')] unsafe={} receivers={}",
    ),
    (
        "slash_after_close_brace_template",
        "`${{}}` /x/",
        "[Opaque, Symbol('/'), Word(\"x\"), Symbol('/')] unsafe={} receivers={}",
    ),
    (
        "depth_64_nested_templates",
        "a = `${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${b}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`",
        "[Word(\"a\"), Symbol('='), Opaque] unsafe={\"b\"} receivers={}",
    ),
    (
        "depth_65_nested_templates",
        "a = `${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${b}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`",
        "None",
    ),
    (
        "depth_66_nested_templates",
        "a = `${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${`${b}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`}`",
        "None",
    ),
    ("jsx_fragment_open_only", "a = <>", "None"),
    ("jsx_fragment_after_paren", "f(<>", "None"),
    ("jsx_close_only", "a = </", "None"),
    ("single_quote_newline_then_word", "a = 'x\ny", "None"),
    ("double_quote_newline_then_word", "a = \"x\ny", "None"),
    ("single_quote_cr_then_word", "a = 'x\ry", "None"),
    ("double_quote_cr_then_word", "a = \"x\ry", "None"),
    ("regexp_cr", "a = /x\r/", "None"),
    ("regexp_escape_at_eof", "a = /\\", "None"),
    ("regexp_class_newline", "a = /[\n]/", "None"),
    (
        "regexp_after_block_comment_with_newline",
        "a /* c\n */ /x/",
        "None",
    ),
    (
        "regexp_after_block_comment_with_cr",
        "a /* c\r */ /x/",
        "None",
    ),
    ("block_comment_star_at_eof", "a /* c *", "None"),
    (
        "regexp_after_unmatched_paren",
        "a) /x/",
        "[Word(\"a\"), Symbol(')'), Symbol('/'), Word(\"x\"), Symbol('/')] unsafe={} receivers={}",
    ),
    ("regexp_after_yield_word_property", "yield /x/", "None"),
    (
        "division_after_string_escape_newline",
        "'a\\n'\n/ 2",
        "None",
    ),
    ("template_unterminated_escape", "a = `\\", "None"),
    ("template_unterminated_nested", "a = `${`x}`", "None"),
];

const SOURCES: &[(&str, &str, &str)] = &[
    (
        "import_path_basename_ok",
        "import path from 'node:path';\nconst a = `${path.basename(p)}`;",
        "[Word(\"import\"), Word(\"path\"), Word(\"from\"), String(\"node:path\"), Symbol(';'), Word(\"const\"), Word(\"a\"), Symbol('='), Opaque, Symbol(';')] unsafe={} opaque={}",
    ),
    (
        "basename_receiver_shadowed",
        "import path from 'node:path';\nconst path = 1;\nconst a = `${path.basename(p)}`;",
        "None",
    ),
    (
        "basename_receiver_unimported",
        "const a = `${other.basename(p)}`;",
        "None",
    ),
    (
        "basename_receiver_fs",
        "import fs from 'node:fs';\nconst a = `${fs.basename(p)}`;",
        "None",
    ),
    (
        "regexp_names_unsafe",
        "const r = /writeFile/; fs.writeFile(a);",
        "[Word(\"const\"), Word(\"r\"), Symbol('='), Opaque, Symbol(';'), Word(\"fs\"), Symbol('.'), Word(\"writeFile\"), Symbol('('), Word(\"a\"), Symbol(')'), Symbol(';')] unsafe={\"writeFile\"} opaque={\"writeFile\"}",
    ),
    (
        "template_binding_unsafe",
        "import fs from 'node:fs';\nconst a = `${fs}`;",
        "[Word(\"import\"), Word(\"fs\"), Word(\"from\"), String(\"node:fs\"), Symbol(';'), Word(\"const\"), Word(\"a\"), Symbol('='), Opaque, Symbol(';')] unsafe={\"fs\"} opaque={\"fs\"}",
    ),
    (
        "assignment_unsafe",
        "let x = 1; x = 2;",
        "[Word(\"let\"), Word(\"x\"), Symbol('='), Symbol('1'), Symbol(';'), Word(\"x\"), Symbol('='), Symbol('2'), Symbol(';')] unsafe={\"x\"} opaque={}",
    ),
    (
        "arrow_param_unsafe",
        "const f = (a) => a;",
        "[Word(\"const\"), Word(\"f\"), Symbol('='), Symbol('('), Word(\"a\"), Symbol(')'), Symbol('='), Symbol('>'), Word(\"a\"), Symbol(';')] unsafe={\"a\"} opaque={}",
    ),
    ("eval_refused", "eval('x')", "None"),
    ("escaped_refused", "\\u0061 = 1", "None"),
    ("jsx_refused", "const a = <div/>;", "None"),
    ("unbalanced_refused", "function f() {", "None"),
    (
        "process_pid_template",
        "const t = `/tmp/x-${process.pid}`;",
        "[Word(\"const\"), Word(\"t\"), Symbol('='), Opaque, Symbol(';')] unsafe={} opaque={}",
    ),
];
