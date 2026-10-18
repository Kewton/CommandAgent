use super::Source;

fn toks(text: &str) -> Vec<(String, bool)> {
    Source::parse(text)
        .expect("source should parse")
        .tokens
        .into_iter()
        .map(|t| (t.text, t.quoted))
        .collect()
}

fn plain(items: &[&str]) -> Vec<(String, bool)> {
    items.iter().map(|s| (s.to_string(), false)).collect()
}

#[test]
fn tokenize_splits_identifiers_and_punctuation() {
    assert_eq!(
        toks("const a_1 = $x + 12;"),
        plain(&["const", "a_1", "=", "$x", "+", "12", ";"])
    );
}

#[test]
fn tokenize_marks_string_literals_quoted_and_keeps_escapes() {
    assert_eq!(
        toks(r#"f('a' "b" `c${d}` 'e\'f')"#),
        vec![
            ("f".to_string(), false),
            ("(".to_string(), false),
            ("a".to_string(), true),
            ("b".to_string(), true),
            ("c${d}".to_string(), true),
            (r"e\'f".to_string(), true),
            (")".to_string(), false),
        ]
    );
}

#[test]
fn tokenize_skips_line_and_block_comments() {
    assert_eq!(toks("a // x y\nb /* c ( */ d"), plain(&["a", "b", "d"]));
}

#[test]
fn tokenize_unterminated_block_comment_or_string_is_none() {
    assert!(Source::parse("a /* open").is_none());
    assert!(Source::parse("a 'open").is_none());
    assert!(Source::parse("a 'open\\").is_none());
}

#[test]
fn tokenize_regex_literal_is_opaque_quoted_token_after_operator() {
    assert_eq!(
        toks("x = /a[/]b\\//gi;"),
        vec![
            ("x".to_string(), false),
            ("=".to_string(), false),
            (String::new(), true),
            (";".to_string(), false),
        ]
    );
    assert_eq!(
        toks("return /a/"),
        vec![("return".to_string(), false), (String::new(), true)]
    );
}

#[test]
fn tokenize_slash_after_value_is_division_not_regex() {
    assert_eq!(toks("a / b / c"), plain(&["a", "/", "b", "/", "c"]));
}

#[test]
fn tokenize_slash_after_quoted_token_is_division() {
    assert_eq!(
        toks("'a' / 2"),
        vec![
            ("a".to_string(), true),
            ("/".to_string(), false),
            ("2".to_string(), false)
        ]
    );
}

#[test]
fn tokenize_unterminated_or_multiline_regex_is_none() {
    assert!(Source::parse("x = /abc").is_none());
    assert!(Source::parse("x = /ab\ncd/").is_none());
}

#[test]
fn tokenize_leading_slash_is_regex() {
    assert_eq!(toks("/a/"), vec![(String::new(), true)]);
}

#[test]
fn pairs_link_matching_brackets_both_ways() {
    let s = Source::parse("f(a[0]){}").expect("parse");
    // tokens: f ( a [ 0 ] ) { }
    assert_eq!(s.pairs[1], Some(6));
    assert_eq!(s.pairs[6], Some(1));
    assert_eq!(s.pairs[3], Some(5));
    assert_eq!(s.pairs[5], Some(3));
    assert_eq!(s.pairs[7], Some(8));
    assert_eq!(s.pairs[8], Some(7));
    assert_eq!(s.pairs[0], None);
    assert_eq!(s.pairs[2], None);
}

#[test]
fn pairs_unbalanced_or_mismatched_is_none() {
    assert!(Source::parse("f(a").is_none());
    assert!(Source::parse("a)").is_none());
    assert!(Source::parse("(a]").is_none());
    assert!(Source::parse("{ ( } )").is_none());
}

#[test]
fn quoted_brackets_do_not_form_pairs() {
    let s = Source::parse("a '(' b").expect("parse");
    assert!(s.pairs.iter().all(Option::is_none));
    assert!(Source::parse("'}' '{'").is_some());
}

#[test]
fn scopes_track_innermost_enclosing_brace_only() {
    let s = Source::parse("a { b ( c ) { d } e } f").expect("parse");
    // 0 a, 1 {, 2 b, 3 (, 4 c, 5 ), 6 {, 7 d, 8 }, 9 e, 10 }, 11 f
    assert_eq!(s.scopes[0], None);
    assert_eq!(s.scopes[1], None);
    assert_eq!(s.scopes[2], Some(1));
    assert_eq!(s.scopes[4], Some(1));
    assert_eq!(s.scopes[6], Some(1));
    assert_eq!(s.scopes[7], Some(6));
    assert_eq!(s.scopes[8], Some(6));
    assert_eq!(s.scopes[9], Some(1));
    assert_eq!(s.scopes[10], Some(1));
    assert_eq!(s.scopes[11], None);
}

#[test]
fn scope_end_and_lookups_follow_scopes_and_quoting() {
    let s = Source::parse("a { b 'x' } c").expect("parse");
    assert_eq!(s.scope_end(2), 4);
    assert_eq!(s.scope_end(0), s.tokens.len());
    assert!(s.is(2, "b"));
    assert!(!s.is(3, "x"));
    assert!(s.any(1, &["(", "{"]));
    assert!(s.seq(0, &["a", "{"]));
    assert_eq!(s.name(2), Some("b"));
    assert_eq!(s.name(3), None);
    assert_eq!(s.name(1), None);
    assert_eq!(s.name(99), None);
}

#[test]
fn expression_end_and_items_respect_nesting() {
    let s = Source::parse("a ? b : c , d ( e , f ) , g").expect("parse");
    assert_eq!(s.expression_end(0, s.tokens.len()), 5);
    let items = s.items(0, s.tokens.len());
    assert_eq!(items, vec![(0, 5), (6, 12), (13, 14)]);
}

#[test]
fn empty_source_parses_to_empty_view() {
    let s = Source::parse("  // only comment\n").expect("parse");
    assert!(s.tokens.is_empty() && s.pairs.is_empty() && s.scopes.is_empty());
}
