//! Issue #850: a simple `const res = cond ? await fetch(...) : await fetch(...)`
//! followed by a sibling `if (!res.ok ...)` guard counts as a checked mutation
//! response. Every case expands the corpus fixtures into a tempdir project and
//! runs the public final and invariant profile checks.

use std::path::Path;

use commandagent::planner::profile::{
    ProfileSnapshot, verify_profile_final, verify_profile_invariant,
};
use commandagent::planner::verify::VerificationReport;

const CORPUS: &str = "tests/corpus/apps/issue850-conditional-fetch";
const GOAL: &str = "Create a notes app on port 3011";
const PAGE_PATH: &str = "src/app/notes/[id]/page.tsx";
const COMMENT: &str = "      // Check Response.ok before accepting the mutation result.";

fn read(path: &str) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|err| panic!("failed to read {path}: {err}"))
}

fn write(root: &Path, relative: &str, body: &str) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body).unwrap();
}

/// Expand `page` and the fixture routes into a complete App Router project and
/// return the final and invariant reports.
fn reports(page: &str) -> [VerificationReport; 2] {
    let root = tempfile::tempdir().unwrap();
    let base = "tests/corpus/apps/issue430-nextjs-utf8-response-window";
    for shared in ["package.json", "tsconfig.json", "src/app/layout.tsx"] {
        write(root.path(), shared, &read(&format!("{base}/{shared}")));
    }
    write(
        root.path(),
        "src/app/page.tsx",
        "export default function Page() {\n  return <main>ready</main>;\n}\n",
    );
    write(root.path(), PAGE_PATH, page);
    let routes = read(&format!("{CORPUS}/fixtures/routes.ts"));
    write(root.path(), "src/app/api/notes/route.ts", &routes);
    write(root.path(), "src/app/api/notes/[id]/route.ts", &routes);
    [
        verify_profile_final(root.path(), "nextjs", GOAL),
        verify_profile_invariant(root.path(), "nextjs", GOAL, &ProfileSnapshot::None),
    ]
}

fn assert_accepted(page: &str, name: &str) {
    for report in reports(page) {
        assert!(report.is_pass(), "{name}: {report:?}");
    }
}

fn assert_rejected(page: &str, name: &str, reason: &str) {
    for report in reports(page) {
        assert!(!report.is_pass(), "{name}: {report:?}");
        assert_eq!(report.profile_failures.len(), 1, "{name}: {report:?}");
        assert!(
            report.profile_failures[0].starts_with("api_contract_failure: ")
                && report.profile_failures[0].contains(reason),
            "{name}: {report:?}"
        );
    }
}

#[test]
fn fixture_cases_expand_through_both_public_profile_checks() {
    let base = read(&format!("{CORPUS}/fixtures/page.tsx"));
    let cases: serde_json::Value =
        serde_json::from_str(&read(&format!("{CORPUS}/fixtures/cases.json"))).unwrap();
    let cases = cases["cases"].as_array().unwrap();
    let mut accepted = 0;
    let mut rejected = 0;
    for case in cases {
        let name = case["name"].as_str().unwrap();
        let mut page = base.clone();
        for edit in case["replace"].as_array().unwrap() {
            let from = edit["from"].as_str().unwrap();
            assert_eq!(page.matches(from).count(), 1, "{name}: anchor {from:?}");
            page = page.replacen(from, edit["to"].as_str().unwrap(), 1);
        }
        match case["expect"].as_str().unwrap() {
            "accept" => {
                accepted += 1;
                assert_accepted(&page, name);
            }
            "reject" => {
                rejected += 1;
                assert_rejected(&page, name, case["reason_contains"].as_str().unwrap());
            }
            other => panic!("{name}: unknown expectation {other}"),
        }
    }
    assert!(accepted >= 4 && rejected >= 20, "{accepted}/{rejected}");
}

#[test]
fn live_shape_posts_and_puts_share_one_guard() {
    // Both branches are mutations of the same route family; each must resolve
    // to the one `res` guard rather than only the first.
    let base = read(&format!("{CORPUS}/fixtures/page.tsx"));
    assert!(base.contains("method: \"POST\"") && base.contains("method: \"PUT\""));
    assert_eq!(base.matches("res.ok").count(), 1);
    assert_accepted(&base, "live shape");
}

#[test]
fn guard_must_end_inside_the_4096_byte_window_around_utf8() {
    let base = read(&format!("{CORPUS}/fixtures/page.tsx"));
    assert_eq!(base.matches(COMMENT).count(), 1);
    let with_pad = |pad: &str| base.replacen(COMMENT, &format!("      /*{pad}*/"), 1);
    let distance = |page: &str| {
        let post = page.find("method: \"POST\"").unwrap() + "method: \"POST\"".len();
        page.find("res.ok").unwrap() + "res.ok".len() - post
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
                assert_accepted(&page, &name);
            } else {
                assert_rejected(&page, &name, "POST /api/notes used by");
            }
        }
    }
}
