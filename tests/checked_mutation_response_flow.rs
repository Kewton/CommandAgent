//! Issue #899: a `fetch(...).then((res) => ...)` chain whose first callback
//! gates the success path on `res.ok` counts as a checked mutation response.
//! Every case expands the corpus fixtures into a tempdir project and runs the
//! public final and invariant profile checks.

use std::collections::BTreeSet;
use std::path::Path;

use commandagent::planner::profile::{
    ProfileSnapshot, verify_profile_final, verify_profile_invariant,
};
use commandagent::planner::verify::VerificationReport;

const CORPUS: &str = "tests/corpus/apps/checked-mutation-response-flow";
const GOAL: &str = "Create an app on port 3011";
const REASON_PREFIX: &str = "api_contract_failure: ";
const MARKER: &str = "    /* CASE */";
/// Every Issue #899 D2 item that the synthetic cases must reject.
const D2_ITEMS: &[&str] = &[
    "json_only",
    "payload_ok_only",
    "ok_ignored",
    "both_branches_succeed",
    "unconditional_success_after_sentinel",
    "catch_returns_success",
    "success_before_guard",
    "other_fetch_ok",
    "shadowing_or_reassignment",
    "inner_function_return",
    "string_comment_template",
    "truncated_syntax",
];

fn read(path: &str) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|err| panic!("failed to read {path}: {err}"))
}

fn json(path: &str) -> serde_json::Value {
    serde_json::from_str(&read(path)).unwrap()
}

fn write(root: &Path, relative: &str, body: &str) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body).unwrap();
}

/// Expand `page` as `src/app/page.tsx` with `routes` at `route_path` into a
/// complete App Router project and return the final and invariant reports.
fn reports(page: &str, route_path: &str, routes: &str) -> [VerificationReport; 2] {
    let root = tempfile::tempdir().unwrap();
    let base = "tests/corpus/apps/issue430-nextjs-utf8-response-window";
    for shared in ["package.json", "tsconfig.json", "src/app/layout.tsx"] {
        write(root.path(), shared, &read(&format!("{base}/{shared}")));
    }
    write(root.path(), "src/app/page.tsx", page);
    write(root.path(), route_path, routes);
    [
        verify_profile_final(root.path(), "nextjs", GOAL),
        verify_profile_invariant(root.path(), "nextjs", GOAL, &ProfileSnapshot::None),
    ]
}

fn recorded_reports(page: &str) -> [VerificationReport; 2] {
    let routes = read(&format!("{CORPUS}/fixtures/recorded/route.ts"));
    reports(page, "src/app/api/highscore/route.ts", &routes)
}

fn synthetic_reports(page: &str) -> [VerificationReport; 2] {
    let routes = read(&format!("{CORPUS}/fixtures/synthetic/route.ts"));
    reports(page, "src/app/api/bookmarks/route.ts", &routes)
}

fn assert_accepted(reports: [VerificationReport; 2], name: &str) {
    for report in reports {
        assert!(report.is_pass(), "{name}: {report:?}");
    }
}

fn assert_rejected(reports: [VerificationReport; 2], name: &str, reason: &str) {
    for report in reports {
        assert!(!report.is_pass(), "{name}: {report:?}");
        assert_eq!(report.profile_failures.len(), 1, "{name}: {report:?}");
        let failure = &report.profile_failures[0];
        assert!(
            failure.starts_with(REASON_PREFIX) && failure.contains(reason),
            "{name}: {report:?}"
        );
    }
}

#[test]
fn recorded_highscore_callback_cases_expand_through_both_public_checks() {
    let base = read(&format!("{CORPUS}/fixtures/recorded/page.tsx"));
    let cases = json(&format!("{CORPUS}/fixtures/recorded/cases.json"));
    let reason = cases["unchecked_reason"].as_str().unwrap();
    let mut seen = [0, 0];
    for case in cases["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let mut page = base.clone();
        for edit in case["replace"].as_array().unwrap() {
            let from = edit["from"].as_str().unwrap();
            assert_eq!(page.matches(from).count(), 1, "{name}: anchor {from:?}");
            page = page.replacen(from, edit["to"].as_str().unwrap(), 1);
        }
        match case["expect"].as_str().unwrap() {
            "accept" => {
                seen[0] += 1;
                assert_accepted(recorded_reports(&page), name);
            }
            "reject" => {
                seen[1] += 1;
                assert_rejected(recorded_reports(&page), name, reason);
            }
            other => panic!("{name}: unknown expectation {other}"),
        }
    }
    assert_eq!(seen, [1, 4]);
}

#[test]
fn synthetic_cases_expand_through_both_public_checks() {
    let base = read(&format!("{CORPUS}/fixtures/synthetic/page.tsx"));
    assert_eq!(base.matches(MARKER).count(), 1);
    let cases = json(&format!("{CORPUS}/fixtures/synthetic/cases.json"));
    let reason = cases["unchecked_reason"].as_str().unwrap();
    let mut accepted = 0;
    let mut traps = 0;
    let mut covered = BTreeSet::new();
    for case in cases["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let mut code = case["code"].as_str().unwrap().to_string();
        for (placeholder, call) in cases["fetch"].as_object().unwrap() {
            code = code.replace(placeholder.as_str(), call.as_str().unwrap());
        }
        assert!(code.contains("fetch(\"/api/bookmarks\""), "{name}");
        let page = base.replacen(MARKER, &format!("    {code}"), 1);
        match case["expect"].as_str().unwrap() {
            "accept" => {
                accepted += 1;
                assert_accepted(synthetic_reports(&page), name);
            }
            "reject" => {
                covered.insert(case["d2"].as_str().unwrap().to_string());
                traps += usize::from(case["fallback_trap"].as_bool().unwrap_or(false));
                assert_rejected(synthetic_reports(&page), name, reason);
            }
            other => panic!("{name}: unknown expectation {other}"),
        }
    }
    let missing = D2_ITEMS
        .iter()
        .filter(|item| !covered.contains(**item))
        .collect::<Vec<_>>();
    assert!(missing.is_empty(), "D2 items without a case: {missing:?}");
    assert!(accepted >= 6 && traps >= 5, "{accepted}/{traps}");
}

#[test]
fn recorded_guard_must_end_inside_the_4096_byte_window_around_utf8() {
    let base = read(&format!("{CORPUS}/fixtures/recorded/page.tsx"));
    let anchor = "    })\n      .then((res) =>";
    assert_eq!(base.matches(anchor).count(), 1);
    let with_pad = |pad: &str| {
        base.replacen(
            anchor,
            &format!("    }}) /*{pad}*/\n      .then((res) =>"),
            1,
        )
    };
    let distance = |page: &str| {
        let post = page.find("method: \"POST\"").unwrap() + "method: \"POST\"".len();
        let guard = "data.ok !== true) return";
        page.find(guard).unwrap() + guard.len() - post
    };
    let floor = distance(&with_pad(""));
    for symbol in ["日", "─", "😀", "a"] {
        for target in 4_094..=4_098 {
            let extra = target - floor;
            let pad = format!(
                "{}{}",
                symbol.repeat(extra / symbol.len()),
                " ".repeat(extra % symbol.len())
            );
            let page = with_pad(&pad);
            assert_eq!(distance(&page), target, "symbol={symbol:?}");
            let name = format!("symbol={symbol:?}, target={target}");
            if target <= 4_096 {
                assert_accepted(recorded_reports(&page), &name);
            } else {
                assert_rejected(
                    recorded_reports(&page),
                    &name,
                    "POST /api/highscore used by",
                );
            }
        }
    }
}

#[test]
fn unchecked_reason_keeps_its_prefix_and_names_the_promise_forms() {
    let base = read(&format!("{CORPUS}/fixtures/recorded/page.tsx"));
    let page = base.replacen("        if (!data || data.ok !== true) return;\n", "", 1);
    for report in recorded_reports(&page) {
        let [failure] = report.profile_failures.as_slice() else {
            panic!("{report:?}");
        };
        let (prefix, forms) = failure.split_once("; accepted forms: ").unwrap();
        assert_eq!(
            prefix,
            "api_contract_failure: POST /api/highscore used by src/app/page.tsx does not check Response.ok before accepting the mutation"
        );
        for form in [
            "`const res = await fetch(...)` then return/throw when `!res.ok`",
            "a simple ternary whose two branches are both `await fetch(...)`",
            "`fetch(...).then((res) => ...)` whose callback returns/throws first when `!res.ok`",
            "is only `if (res.ok) { ... }`",
            "`res.ok ? res.json() : null` with the next `.then((data) => ...)` returning/throwing first when `!data`",
        ] {
            assert!(forms.contains(form), "{form}: {failure}");
        }
    }
}
