//! Issue #881: the hook, includes and source checks are host tokens, judged in Rust.
use std::path::Path;

use super::{
    Check, evaluate, hook_action_command, hook_state_command, is_generated_hook_check,
    is_source_check, legacy_for_token, legacy_inline_command, parse, same_hook_check,
    script_includes_command, source_keydown_command, source_token_command,
};
use crate::minimal_loop::dependency_setup::NodeDependencySetupAuthority;
use crate::minimal_loop::evidence::command_diagnosis;
use crate::planner::fix_contract_predicate::FixContractPredicateContext;
use crate::planner::lint::{PlanQualityContext, step_plan_quality_report};
use crate::planner::step_plan::{PlanStep, StepPlan};
use crate::planner::verify::verify_step_with_profile_setup_observed_with_offline_and_events;

const PAGE: &str = "src/app/page.tsx";

fn workspace(files: &[(&str, &[u8])]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (path, bytes) in files {
        let path = dir.path().join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }
    dir
}

fn text_workspace(files: &[(&str, &str)]) -> tempfile::TempDir {
    let files = files
        .iter()
        .map(|(path, text)| (*path, text.as_bytes()))
        .collect::<Vec<_>>();
    workspace(&files)
}

fn step(verify: Vec<String>) -> PlanStep {
    PlanStep {
        id: "verify-contract".to_string(),
        kind: "verify".to_string(),
        expected_result: "pass".to_string(),
        instruction: "Check the contract".to_string(),
        expected_paths: Vec::new(),
        verify,
    }
}

// ---------------------------------------------------------------- grammar

#[test]
fn issue881_builders_emit_tokens_that_parse_back_to_the_same_check() {
    let cases = [
        (
            hook_action_command("primary", PAGE),
            Check::HookAction {
                value: "primary".to_string(),
                path: PAGE.to_string(),
            },
        ),
        (
            hook_state_command("src/components/Board.tsx"),
            Check::HookState {
                path: "src/components/Board.tsx".to_string(),
            },
        ),
        (
            source_keydown_command(PAGE),
            Check::SourceKeydown {
                path: PAGE.to_string(),
            },
        ),
        (
            source_token_command("keys[", PAGE),
            Check::SourceToken {
                token: "keys[".to_string(),
                path: PAGE.to_string(),
            },
        ),
        (
            source_token_command("$set", "src/app/[id]/page.tsx"),
            Check::SourceToken {
                token: "$set".to_string(),
                path: "src/app/[id]/page.tsx".to_string(),
            },
        ),
    ];
    for (token, check) in cases {
        assert!(token.starts_with("anvil-nextjs-check:"), "{token}");
        // No shell metacharacter and no space survives in a token.
        assert!(
            token
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || "_./-:%".contains(ch)),
            "{token}"
        );
        assert_eq!(parse(&token), Some(check), "{token}");
        assert!(is_source_check(&token), "{token}");
    }
    let script = script_includes_command("next dev -p 3011").unwrap();
    assert_eq!(
        script,
        "anvil-nextjs-check:script_includes:dev:next%20dev%20-p%203011"
    );
    assert!(matches!(
        parse(&script),
        Some(Check::ScriptIncludes { pattern, .. }) if pattern == "next dev -p 3011"
    ));
}

#[test]
fn issue881_a_script_pattern_with_a_quote_or_backslash_is_left_as_the_grep() {
    assert!(script_includes_command("next build").is_some());
    for pattern in [
        "\"dev\": \"next dev -p 3011\"",
        "'dev': 'next dev'",
        "next dev\\n",
        "no script is named here",
    ] {
        assert_eq!(script_includes_command(pattern), None, "{pattern}");
    }
}

#[test]
fn issue881_an_altered_token_does_not_parse() {
    for altered in [
        // Unknown check, or the family alone.
        "anvil-nextjs-check:hook_action",
        // Missing or extra argument.
        "anvil-nextjs-check:hook_action:primary",
        "anvil-nextjs-check:hook_action:primary:src/app/page.tsx:extra",
        "anvil-nextjs-check:hook_state",
        // A value or path the builders refuse.
        "anvil-nextjs-check:hook_action:pri%20mary:src/app/page.tsx",
        "anvil-nextjs-check:hook_action:primary:../page.tsx",
        "anvil-nextjs-check:hook_action:primary:/etc/page.tsx",
        "anvil-nextjs-check:hook_action:primary:src/app/page.css",
        "anvil-nextjs-check:hook_action:primary:node_modules/x/page.tsx",
        "anvil-nextjs-check:hook_action:primary:./src/app/page.tsx",
        "anvil-nextjs-check:source_keydown:app/page.tsx",
        "anvil-nextjs-check:source_token:1bad:src/app/page.tsx",
        "anvil-nextjs-check:source_token:useState:components/page.tsx",
        // The encoding is not the canonical one.
        "anvil-nextjs-check:hook_action:primary:src%2Fapp/page.tsx",
        "anvil-nextjs-check:script_includes:dev:next%20dev%2d",
        "anvil-nextjs-check:script_includes:dev:next%2",
        "anvil-nextjs-check:script_includes:dev:next+dev",
        "anvil-nextjs-check:script_includes:dev:next dev",
        "anvil-nextjs-check:script_includes:dev:next%e3%81",
        // The script must be the one the pattern is about.
        "anvil-nextjs-check:script_includes:build:next%20dev",
        "anvil-nextjs-check:script_includes:test:next%20dev",
        // A pattern with a quote is never tokenized.
        "anvil-nextjs-check:script_includes:dev:%22dev%22%20next%20dev",
        // Surrounding text.
        "anvil-nextjs-check:hook_state:src/app/page.tsx || true",
        "anvil-nextjs-check:hook_state:src/app/page.tsx;touch%20x",
    ] {
        assert_eq!(parse(altered), None, "{altered}");
        // Still the host's, so an altered token is judged and failed, never run.
        assert!(is_source_check(altered), "{altered}");
        let root = text_workspace(&[(PAGE, "data-anvil-state data-anvil-action=\"primary\"")]);
        let reason = evaluate(root.path(), altered).unwrap_err();
        assert!(reason.contains("malformed"), "{altered}: {reason}");
    }
}

// ------------------------------------------------- the former JS, as a table

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Meaning {
    /// The Rust judgement and the former `node -p` agree.
    Same,
    /// The Rust judgement fails where `node -p` passed (stricter, by design).
    Stricter,
}

struct Case {
    name: &'static str,
    files: Vec<(&'static str, Vec<u8>)>,
    token: String,
    /// What the Rust judgement says.
    passes: bool,
    meaning: Meaning,
}

fn case(
    name: &'static str,
    files: Vec<(&'static str, Vec<u8>)>,
    token: String,
    passes: bool,
    meaning: Meaning,
) -> Case {
    Case {
        name,
        files,
        token,
        passes,
        meaning,
    }
}

fn page(text: &str) -> Vec<(&'static str, Vec<u8>)> {
    vec![(PAGE, text.as_bytes().to_vec())]
}

fn package(text: &str) -> Vec<(&'static str, Vec<u8>)> {
    vec![("package.json", text.as_bytes().to_vec())]
}

fn action_cases() -> Vec<Case> {
    let primary = hook_action_command("primary", PAGE);
    let same =
        |name, text: &str, passes| case(name, page(text), primary.clone(), passes, Meaning::Same);
    let mut cases = vec![
        same("double quote", "<b data-anvil-action=\"primary\">", true),
        same("single quote", "<b data-anvil-action='primary'>", true),
        same("braced double", "<b data-anvil-action={\"primary\"}>", true),
        same("braced single", "<b data-anvil-action={'primary'}>", true),
        same("braced template", "<b data-anvil-action={`primary`}>", true),
        same("spaces around =", "data-anvil-action = \"primary\"", true),
        same(
            "newline and tab",
            "data-anvil-action\n\t=\n\t\"primary\"",
            true,
        ),
        same(
            "braced with spaces",
            "data-anvil-action={ \n 'primary' \t}",
            true,
        ),
        same(
            "no-break space",
            "data-anvil-action\u{A0}=\u{A0}\"primary\"",
            true,
        ),
        same(
            "zero width no-break space",
            "data-anvil-action\u{FEFF}=\"primary\"",
            true,
        ),
        same("em space", "data-anvil-action\u{2003}=\"primary\"", true),
        same(
            "ideographic space",
            "data-anvil-action=\u{3000}\"primary\"",
            true,
        ),
        same(
            "line separator",
            "data-anvil-action\u{2028}=\"primary\"",
            true,
        ),
        // U+0085 and U+180E are not JS whitespace, though Rust's `\s` reads U+0085.
        same(
            "next line is not a space",
            "data-anvil-action\u{85}=\"primary\"",
            false,
        ),
        same(
            "mongolian vowel separator",
            "data-anvil-action\u{180E}=\"primary\"",
            false,
        ),
        same("other value", "data-anvil-action=\"restart\"", false),
        same(
            "prefix of the value",
            "data-anvil-action=\"primary2\"",
            false,
        ),
        same(
            "suffix of the value",
            "data-anvil-action=\"xprimary\"",
            false,
        ),
        same("unquoted", "data-anvil-action=primary", false),
        same("mismatched quotes", "data-anvil-action=\"primary'", false),
        same(
            "brace without the close",
            "data-anvil-action={\"primary\"",
            false,
        ),
        same("attribute name only", "data-anvil-action", false),
        same("another attribute", "data-anvil-actions=\"primary\"", false),
    ];
    cases.push(case(
        "no file",
        Vec::new(),
        primary.clone(),
        false,
        Meaning::Same,
    ));
    cases.push(case(
        "invalid UTF-8 elsewhere",
        vec![(PAGE, b"<b data-anvil-action=\"primary\"/>\xFF\xFE".to_vec())],
        primary.clone(),
        true,
        Meaning::Same,
    ));
    cases.push(case(
        "invalid UTF-8 inside the value",
        vec![(PAGE, b"<b data-anvil-action=\"pri\xFFmary\"/>".to_vec())],
        primary,
        false,
        Meaning::Same,
    ));
    cases
}

fn state_cases() -> Vec<Case> {
    let state = hook_state_command(PAGE);
    let same =
        |name, text: &str, passes| case(name, page(text), state.clone(), passes, Meaning::Same);
    vec![
        same("with a value", "<main data-anvil-state=\"idle\">", true),
        same("bare", "<main data-anvil-state>", true),
        same("equals at the end", "data-anvil-state=", true),
        same("spaced equals", "data-anvil-state \n =x", true),
        same("hyphen continues", "data-anvil-state-x", true),
        same("word character continues", "data-anvil-statee", false),
        same("underscore continues", "data-anvil-state_x", false),
        same("digit continues", "data-anvil-state2", false),
        // `\b` is ASCII in JS: a non-ASCII letter after the name is still a boundary.
        same("accented letter continues", "data-anvil-stat\u{E9}", false),
        same("accent after the name", "data-anvil-state\u{E9}", true),
        same("other attribute", "data-anvil-action=\"primary\"", false),
        case("no file", Vec::new(), state.clone(), false, Meaning::Same),
        case(
            "invalid UTF-8",
            vec![(PAGE, b"\xFF data-anvil-state \xFF".to_vec())],
            state,
            true,
            Meaning::Same,
        ),
    ]
}

fn includes_cases() -> Vec<Case> {
    let dev = script_includes_command("next dev -p 3011").unwrap();
    let build = script_includes_command("next build").unwrap();
    let start = script_includes_command("next start").unwrap();
    let flagged = script_includes_command("next build --lint").unwrap();
    let same = |name, text: &str, token: &String, passes| {
        case(name, package(text), token.clone(), passes, Meaning::Same)
    };
    vec![
        same(
            "exact",
            r#"{"scripts":{"dev":"next dev -p 3011"}}"#,
            &dev,
            true,
        ),
        same(
            "longer value",
            r#"{"scripts":{"dev":"next dev -p 30112"}}"#,
            &dev,
            true,
        ),
        same(
            "part of a longer script",
            r#"{"scripts":{"dev":"PORT=1 next dev -p 3011 --turbo"}}"#,
            &dev,
            true,
        ),
        same(
            "another port",
            r#"{"scripts":{"dev":"next dev -p 3000"}}"#,
            &dev,
            false,
        ),
        same(
            "another script",
            r#"{"scripts":{"build":"next dev -p 3011"}}"#,
            &dev,
            false,
        ),
        same(
            "build",
            r#"{"scripts":{"build":"next build"}}"#,
            &build,
            true,
        ),
        same(
            "build with a flag",
            r#"{"scripts":{"build":"next build --lint"}}"#,
            &build,
            true,
        ),
        same(
            "pattern with a flag",
            r#"{"scripts":{"build":"next build --lint"}}"#,
            &flagged,
            true,
        ),
        same(
            "pattern with a flag, bare script",
            r#"{"scripts":{"build":"next build"}}"#,
            &flagged,
            false,
        ),
        same(
            "start",
            r#"{"scripts":{"start":"next start -p 3011"}}"#,
            &start,
            true,
        ),
        same(
            "script missing",
            r#"{"scripts":{"build":"next build"}}"#,
            &dev,
            false,
        ),
        same("scripts missing", "{}", &dev, false),
        same(
            "scripts is a string",
            r#"{"scripts":"next dev -p 3011"}"#,
            &dev,
            false,
        ),
        same(
            "value is a number",
            r#"{"scripts":{"dev":3011}}"#,
            &dev,
            false,
        ),
        same("value is null", r#"{"scripts":{"dev":null}}"#, &dev, false),
        same(
            "value is an object",
            r#"{"scripts":{"dev":{"cmd":"next dev -p 3011"}}}"#,
            &dev,
            false,
        ),
        same("broken json", r#"{"scripts":"#, &dev, false),
        case(
            "no package.json",
            Vec::new(),
            dev.clone(),
            false,
            Meaning::Same,
        ),
        // `String([...])` joins an array, so the former check read it as text.
        case(
            "value is an array",
            package(r#"{"scripts":{"dev":["next dev -p 3011"]}}"#),
            dev.clone(),
            false,
            Meaning::Stricter,
        ),
        // `require` drops a BOM; the host does not read a manifest with one.
        case(
            "byte order mark",
            vec![(
                "package.json",
                b"\xEF\xBB\xBF{\"scripts\":{\"dev\":\"next dev -p 3011\"}}".to_vec(),
            )],
            dev,
            false,
            Meaning::Stricter,
        ),
    ]
}

fn source_cases() -> Vec<Case> {
    let keydown = source_keydown_command(PAGE);
    let token = |name: &str| source_token_command(name, PAGE);
    let same = |name, text: &str, command: &String, passes| {
        case(name, page(text), command.clone(), passes, Meaning::Same)
    };
    let use_state = token("useState");
    let keys = token("keys[");
    let dollar = token("$set");
    vec![
        same("onKeyDown", "<div onKeyDown={go}>", &keydown, true),
        same("keys index", "if (keys[e.key]) {}", &keydown, true),
        same(
            "addEventListener and keydown",
            "window.addEventListener('keydown', go)",
            &keydown,
            true,
        ),
        same(
            "addEventListener alone",
            "window.addEventListener('click', go)",
            &keydown,
            false,
        ),
        same("keydown alone", "const name = 'keydown'", &keydown, false),
        same(
            "onKeydown spelling",
            "<div onKeydown={go}>",
            &keydown,
            false,
        ),
        same(
            "no handler",
            "export default function Page() {}",
            &keydown,
            false,
        ),
        same(
            "useState present",
            "const [a, b] = useState(0)",
            &use_state,
            true,
        ),
        same(
            "useState absent",
            "const [a, b] = useReducer(r, 0)",
            &use_state,
            false,
        ),
        same(
            "case differs",
            "const [a, b] = usestate(0)",
            &use_state,
            false,
        ),
        same("keys bracket", "keys[e.key] = true", &keys, true),
        same("keys without bracket", "keys.e", &keys, false),
        same("dollar identifier", "$set(1)", &dollar, true),
        case("no file", Vec::new(), keydown.clone(), false, Meaning::Same),
        case(
            "invalid UTF-8",
            vec![(PAGE, b"\xFF onKeyDown \xFF".to_vec())],
            keydown,
            true,
            Meaning::Same,
        ),
    ]
}

fn node_available() -> bool {
    std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join("node").is_file()))
}

/// What the inline program the host used to assemble answers for this workspace.
fn former_js_passes(root: &Path, token: &str) -> bool {
    let inline = legacy_for_token(token).expect("a valid token has an inline form");
    crate::planner::verify::verify_step(root, &step(vec![inline])).is_pass()
}

fn all_cases() -> Vec<Case> {
    let mut cases = action_cases();
    cases.extend(state_cases());
    cases.extend(includes_cases());
    cases.extend(source_cases());
    cases
}

#[test]
fn issue881_the_rust_judgement_matches_the_table() {
    for case in all_cases() {
        let files = case
            .files
            .iter()
            .map(|(path, bytes)| (*path, bytes.as_slice()))
            .collect::<Vec<_>>();
        let root = workspace(&files);
        let verdict = evaluate(root.path(), &case.token);
        assert_eq!(verdict.is_ok(), case.passes, "{}: {verdict:?}", case.name);
    }
}

#[test]
fn issue881_the_rust_judgement_is_the_former_node_check_or_stricter() {
    if !node_available() {
        eprintln!("SKIPPED: node is not available, so the former `node -p` table was not compared");
        return;
    }
    for case in all_cases() {
        let files = case
            .files
            .iter()
            .map(|(path, bytes)| (*path, bytes.as_slice()))
            .collect::<Vec<_>>();
        let root = workspace(&files);
        let rust = evaluate(root.path(), &case.token).is_ok();
        let node = former_js_passes(root.path(), &case.token);
        // Rust passes => node passes, in every case.
        assert!(!rust || node, "{}: rust passes where node fails", case.name);
        match case.meaning {
            Meaning::Same => assert_eq!(rust, node, "{}", case.name),
            Meaning::Stricter => assert!(!rust && node, "{}: rust={rust} node={node}", case.name),
        }
    }
}

// --------------------------------------------------------- compatibility

#[test]
fn issue881_the_generated_hook_identity_holds_for_tokens_and_the_former_text() {
    for value in ["primary", "restart", "input", "search", "submit"] {
        let token = hook_action_command(value, PAGE);
        let inline = legacy_for_token(&token).unwrap();
        assert!(is_generated_hook_check(&token), "{token}");
        assert!(is_generated_hook_check(&inline), "{inline}");
        // One character different is another command, in either spelling.
        assert!(!is_generated_hook_check(&format!("{token} || true")));
        assert!(!is_generated_hook_check(&format!("{inline} || true")));
        assert!(!is_generated_hook_check(
            &inline.replace("process.exit(1)", "process.exit(0)")
        ));
        assert!(!is_generated_hook_check(&inline.replacen(
            value,
            &format!("{value}x"),
            1
        )));
    }
    let state = hook_state_command(PAGE);
    assert!(is_generated_hook_check(&state));
    assert!(is_generated_hook_check(&legacy_for_token(&state).unwrap()));
    for other in [
        hook_action_command("custom", PAGE),
        legacy_inline_command(&Check::HookAction {
            value: "custom".to_string(),
            path: PAGE.to_string(),
        }),
        source_keydown_command(PAGE),
        script_includes_command("next build").unwrap(),
        "node -p 'console.log(\"data-anvil-state\")'".to_string(),
    ] {
        assert!(!is_generated_hook_check(&other), "{other}");
    }
}

#[test]
fn issue881_same_hook_check_compares_tokens_and_the_former_text() {
    let token = hook_action_command("restart", PAGE);
    let inline = legacy_for_token(&token).unwrap();
    let spaced = inline.replacen(")].some", ") ].some", 1);
    assert_ne!(spaced, inline);
    for pair in [
        (&token, &token),
        (&token, &inline),
        (&inline, &token),
        (&token, &spaced),
    ] {
        assert!(same_hook_check(pair.0, pair.1), "{pair:?}");
    }
    let other_path = hook_action_command("restart", "src/app/other.tsx");
    let other_value = hook_action_command("primary", PAGE);
    let state = hook_state_command(PAGE);
    for different in [&other_path, &other_value, &state] {
        assert!(!same_hook_check(&token, different), "{different}");
        assert!(!same_hook_check(&inline, different), "{different}");
    }
    assert!(!same_hook_check(&token, "npm run build"));
    assert!(!same_hook_check("npm run build", "npm run build"));
    assert!(!same_hook_check(
        &token,
        &inline.replacen("restart", "restar", 1)
    ));
}

#[test]
fn issue881_fix_contract_lineage_reads_tokens_and_the_former_text_alike() {
    let root = text_workspace(&[(PAGE, "<button data-anvil-action=\"primary\">Start</button>")]);
    let canonical =
        crate::planner::verify::hook_attribute_present_check_command("action", "restart", PAGE)
            .unwrap();
    let inline = legacy_for_token(&canonical).unwrap();
    let spaced = inline.replacen(")].some", ") ].some", 1);
    let context = |command: &str| {
        FixContractPredicateContext::from_failed_reproducer(root.path(), "nextjs", command, None)
    };
    for command in [&canonical, &inline, &spaced] {
        assert!(context(command).is_some(), "no lineage for {command}");
    }
    for command in [
        hook_action_command("restart", "src/app/other.tsx"),
        hook_action_command("custom", PAGE),
        inline.replacen(PAGE, "src/app /page.tsx", 1),
        inline.replacen("restart", "restar", 1),
        "npm run build".to_string(),
    ] {
        assert!(context(&command).is_none(), "{command}");
    }
}

// --------------------------------------------------------- evidence strength

#[test]
fn issue881_a_token_is_classified_like_the_inline_text_it_replaced() {
    let root = text_workspace(&[(PAGE, "x"), ("package.json", "{}")]);
    let tokens = [
        hook_action_command("primary", PAGE),
        hook_action_command("restart", PAGE),
        hook_action_command("input", PAGE),
        hook_action_command("search", PAGE),
        hook_action_command("submit", PAGE),
        hook_action_command("custom", PAGE),
        hook_state_command(PAGE),
        script_includes_command("next dev -p 3011").unwrap(),
        script_includes_command("next build").unwrap(),
        source_keydown_command(PAGE),
        source_token_command("useState", PAGE),
    ];
    let inline = tokens
        .iter()
        .map(|token| legacy_for_token(token).unwrap())
        .collect::<Vec<_>>();
    let of_tokens = command_diagnosis::collect(root.path(), &tokens);
    let of_inline = command_diagnosis::collect(root.path(), &inline);
    // (kind, reason) per command; the hook checks of the contract values are the
    // same static syntax evidence they always were, and nothing is stronger.
    let expected = [
        ("static_syntax", None),
        ("static_syntax", None),
        ("static_syntax", None),
        ("static_syntax", None),
        ("static_syntax", None),
        ("weak", Some("node_smoke_without_assertion")),
        ("static_syntax", None),
        ("weak", Some("node_smoke_without_assertion")),
        ("weak", Some("node_smoke_without_assertion")),
        ("weak", Some("node_smoke_without_assertion")),
        ("weak", Some("node_smoke_without_assertion")),
    ];
    for (index, token) in tokens.iter().enumerate() {
        assert_eq!(
            (of_tokens[index].kind, of_tokens[index].reason.as_deref()),
            (of_inline[index].kind, of_inline[index].reason.as_deref()),
            "{token}"
        );
        assert_eq!(
            (of_tokens[index].kind, of_tokens[index].reason.as_deref()),
            expected[index],
            "{token}"
        );
    }
}

// --------------------------------------------------- source assertion demotion

#[test]
fn issue881_a_failed_source_token_is_still_demoted_to_an_advisory() {
    let root = text_workspace(&[
        (
            PAGE,
            "<main data-anvil-state data-anvil-action=\"primary\">",
        ),
        ("package.json", "{}"),
    ]);
    let events = root.path().join(".anvil/runs/test/events.jsonl");
    let run = |verify: Vec<String>| {
        verify_step_with_profile_setup_observed_with_offline_and_events(
            root.path(),
            &step(verify),
            Some("nextjs"),
            NodeDependencySetupAuthority::None,
            false,
            Some(&events),
        )
        .0
    };
    let contract = hook_state_command(PAGE);
    let missing_api = source_token_command("useState", PAGE);
    let missing_keydown = source_keydown_command(PAGE);

    // The contract check passes, the implementation spelling is missing: advisory.
    let report = run(vec![contract.clone(), missing_api.clone()]);
    assert!(report.is_pass(), "{report:?}");
    let report = run(vec![missing_keydown.clone(), contract.clone()]);
    assert!(report.is_pass(), "{report:?}");
    let log = std::fs::read_to_string(&events).unwrap();
    assert_eq!(log.matches("verify_demoted_advisory").count(), 2, "{log}");

    // Without a contract or build check beside it, the same failure counts.
    let report = run(vec![missing_api.clone()]);
    assert!(!report.is_pass(), "{report:?}");
    // A failing contract check is never demoted.
    let report = run(vec![hook_action_command("restart", PAGE), missing_api]);
    assert!(!report.is_pass(), "{report:?}");
    // The advisory is only for the source assertions.
    let report = run(vec![hook_action_command("restart", PAGE), contract]);
    assert!(!report.is_pass(), "{report:?}");
}

// ------------------------------------------------------------------- lint

#[test]
fn issue881_correct_nextjs_tokens_are_a_strong_verify_only_in_the_nextjs_profile() {
    let plan = |verify: Vec<String>| StepPlan {
        goal: "Create an interactive Next.js app".to_string(),
        steps: vec![PlanStep {
            id: "contract-wiring".to_string(),
            kind: "implement".to_string(),
            expected_result: "pass".to_string(),
            instruction: "Wire the contract hooks".to_string(),
            expected_paths: vec![PAGE.to_string()],
            verify,
        }],
    };
    let weak = |profile: &str, verify: Vec<String>| {
        let context = PlanQualityContext {
            profile: profile.to_string(),
            task_intent: "create".to_string(),
            ..PlanQualityContext::default()
        };
        step_plan_quality_report(&plan(verify), &context)
            .issues
            .iter()
            .any(|issue| issue.category == "weak_code_verify")
    };
    let tokens = [
        hook_action_command("primary", PAGE),
        hook_state_command(PAGE),
        script_includes_command("next build").unwrap(),
        source_keydown_command(PAGE),
        source_token_command("useState", PAGE),
        crate::planner::profiles::nextjs::package_script_check::build_check_command(),
    ];
    for token in &tokens {
        assert!(!weak("nextjs", vec![token.clone()]), "{token}");
        assert!(weak("generic", vec![token.clone()]), "{token}");
    }
    // An altered token is no verify at all.
    for altered in [
        "anvil-nextjs-check:hook_state:src/app/page.tsx%2f",
        "anvil-nextjs-check:hook_state",
        "anvil-nextjs-check:unknown",
    ] {
        assert!(weak("nextjs", vec![altered.to_string()]), "{altered}");
    }
}

// ------------------------------------------------------------ prompt note

#[test]
fn issue881_a_host_check_line_gets_a_note_and_every_other_line_stays_as_it_was() {
    use crate::planner::profiles::step_checks::verify_prompt_bullets;
    let note = "  (host check: the host judges it after the step; do not run it in Bash)\n";
    let tokens = [
        hook_action_command("primary", PAGE),
        script_includes_command("next build").unwrap(),
        source_keydown_command(PAGE),
        crate::planner::profiles::nextjs::package_script_check::build_check_command(),
        crate::planner::profiles::ingest::phase_verify::CHECK_COMMAND.to_string(),
    ];
    for token in &tokens {
        assert_eq!(
            verify_prompt_bullets(std::slice::from_ref(token)),
            format!("- {token}\n{note}")
        );
    }
    // Plain commands, including the inline interpreters a model may write, are as before.
    let plain = vec![
        "npm run build".to_string(),
        "grep -q \"next dev\" package.json".to_string(),
        "node -p '1'".to_string(),
    ];
    assert_eq!(
        verify_prompt_bullets(&plain),
        "- npm run build\n- grep -q \"next dev\" package.json\n- node -p '1'\n"
    );
    assert_eq!(verify_prompt_bullets(&[]), "- none\n");
    // The prompt reader that takes each `- ` line as a command sees the commands alone.
    let mut verify = plain.clone();
    verify.splice(1..1, tokens.iter().cloned());
    let prompt = format!(
        "Expected paths after this step:\n- package.json\n\nVerification commands for this step:\n{}\nExpected verification result:\npass",
        verify_prompt_bullets(&verify)
    );
    assert_eq!(
        crate::planner::setup_step_policy::verification_commands_from_prompt(&prompt),
        verify
    );
}
