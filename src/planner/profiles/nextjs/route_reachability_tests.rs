use std::path::Path;

use serde_json::Value;

use super::{
    EVIDENCE_RELATIVE_PATH, ReachabilityReport, ReachabilityStatus, evaluate, merge_with_testimony,
    write_evidence,
};
use crate::planner::profile::{ProfileId, ProfileRuntimeRegistry};

const HOOKED_PAGE: &str = r#"export default function Page() {
  return <button data-anvil-action="primary">Go</button>;
}
"#;
const PLAIN_PAGE: &str = "export default function Page() { return <main>hello</main>; }\n";

fn app(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (path, content) in files {
        let full = dir.path().join(path);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(full, content).unwrap();
    }
    dir
}

fn home_with(body: &str) -> String {
    format!(
        "import Link from \"next/link\";\nimport {{ useRouter }} from \"next/navigation\";\nexport default function Home() {{\n  const router = useRouter();\n  return (\n    <main>\n      <button data-anvil-action=\"primary\">New</button>\n      {body}\n    </main>\n  );\n}}\n"
    )
}

fn detail_route_outcome(entry_body: &str) -> ReachabilityReport {
    let home = home_with(entry_body);
    let dir = app(&[
        ("src/app/page.tsx", &home),
        ("src/app/notes/[id]/page.tsx", HOOKED_PAGE),
    ]);
    evaluate(dir.path())
}

fn assert_reached(report: &ReachabilityReport) {
    assert_eq!(report.status, ReachabilityStatus::Pass, "{report:?}");
    assert!(report.violations.is_empty() && report.unverified.is_empty());
    assert_eq!(report.targets.len(), 1, "{report:?}");
    assert_eq!(report.targets[0].outcome, "reached");
    assert!(report.targets[0].reached_via.is_some());
}

#[test]
fn l113_shape_without_a_link_to_the_hook_route_fails() {
    let dir = app(&[
        ("src/app/page.tsx", &home_with("<p>notes</p>")),
        (
            "src/app/notes/[id]/page.tsx",
            "import { useRouter } from \"next/navigation\";\nexport default function Edit() {\n  const router = useRouter();\n  return <button data-anvil-action=\"primary\" onClick={() => router.push(\"/\")}>Save</button>;\n}\n",
        ),
    ]);
    let report = evaluate(dir.path());
    assert_eq!(report.status, ReachabilityStatus::Failed);
    assert_eq!(
        report.violations,
        vec!["route_unreachable_from_entry:/notes/[id]".to_string()]
    );
    assert!(report.unverified.is_empty());
    assert_eq!(report.targets[0].outcome, "unreachable");
    assert_eq!(report.targets[0].page, "src/app/notes/[id]/page.tsx");
    assert_eq!(report.targets[0].hook_source, "src/app/notes/[id]/page.tsx");
}

#[test]
fn l111_shape_with_template_and_literal_links_reaches_the_route() {
    let report = detail_route_outcome(
        "<Link href={`/notes/${note.id}`}>open</Link>\n<Link href=\"/notes/new\">new</Link>",
    );
    assert_reached(&report);
    assert_eq!(report.reached_routes, vec!["/notes/[id]".to_string()]);
    assert_eq!(
        report.targets[0].reached_via.as_ref().unwrap().syntax,
        "href"
    );
}

#[test]
fn entry_only_app_has_no_targets() {
    let dir = app(&[("src/app/page.tsx", HOOKED_PAGE)]);
    let report = evaluate(dir.path());
    assert_eq!(report.status, ReachabilityStatus::Pass);
    assert!(report.targets.is_empty());
}

#[test]
fn hookless_detail_route_is_not_a_target() {
    let dir = app(&[
        ("src/app/page.tsx", HOOKED_PAGE),
        ("src/app/notes/[id]/page.tsx", PLAIN_PAGE),
    ]);
    let report = evaluate(dir.path());
    assert_eq!(report.status, ReachabilityStatus::Pass);
    assert!(report.targets.is_empty(), "{report:?}");
}

#[test]
fn hook_reached_only_through_the_import_closure_makes_the_route_a_target() {
    let dir = app(&[
        ("src/app/page.tsx", HOOKED_PAGE),
        (
            "src/app/notes/[id]/page.tsx",
            "import Editor from \"@/components/Editor\";\nexport default function P() { return <Editor />; }\n",
        ),
        ("src/components/Editor.tsx", HOOKED_PAGE),
    ]);
    let report = evaluate(dir.path());
    assert_eq!(report.status, ReachabilityStatus::Failed, "{report:?}");
    assert_eq!(report.targets[0].hook_source, "src/components/Editor.tsx");
}

#[test]
fn two_step_navigation_through_a_hookless_list_reaches_the_detail_route() {
    let list = "import Link from \"next/link\";\nexport default function List() {\n  return <Link href={`/notes/${id}`}>open</Link>;\n}\n";
    let dir = app(&[
        (
            "src/app/page.tsx",
            "import Link from \"next/link\";\nexport default function Home() {\n  return <Link href=\"/notes\" data-anvil-action=\"primary\">notes</Link>;\n}\n",
        ),
        ("src/app/notes/page.tsx", list),
        ("src/app/notes/[id]/page.tsx", HOOKED_PAGE),
    ]);
    let report = evaluate(dir.path());
    assert_eq!(report.status, ReachabilityStatus::Pass, "{report:?}");
    assert_eq!(
        report.reached_routes,
        vec!["/notes".to_string(), "/notes/[id]".to_string()]
    );
    assert_eq!(
        report.targets[0].reached_via.as_ref().unwrap().file,
        "src/app/notes/page.tsx"
    );
}

#[test]
fn second_step_link_does_not_count_when_the_first_step_is_missing() {
    let list = "import Link from \"next/link\";\nexport default function List() {\n  return <Link href={`/notes/${id}`}>open</Link>;\n}\n";
    let dir = app(&[
        ("src/app/page.tsx", HOOKED_PAGE),
        ("src/app/notes/page.tsx", list),
        ("src/app/notes/[id]/page.tsx", HOOKED_PAGE),
    ]);
    let report = evaluate(dir.path());
    assert_eq!(report.status, ReachabilityStatus::Failed, "{report:?}");
    assert_eq!(
        report.violations,
        vec!["route_unreachable_from_entry:/notes/[id]".to_string()]
    );
}

#[test]
fn unresolved_router_argument_without_a_matching_literal_is_partial() {
    let report = detail_route_outcome("<button onClick={() => router.push(target)}>go</button>");
    assert_eq!(report.status, ReachabilityStatus::Partial, "{report:?}");
    assert_eq!(
        report.unverified,
        vec!["route_reachability_unverified:/notes/[id]".to_string()]
    );
    assert!(report.violations.is_empty());
    assert_eq!(report.targets[0].outcome, "unverified");
    assert_eq!(report.unresolved_navigations.len(), 1);
    assert_eq!(report.unresolved_navigations[0].syntax, "router_push");
}

#[test]
fn route_shaped_constant_behind_an_unresolved_navigation_is_partial_not_reached() {
    let dir = app(&[
        (
            "src/app/page.tsx",
            "import { EDIT } from \"@/lib/routes\";\nimport Link from \"next/link\";\nexport default function Home() {\n  return <Link data-anvil-action=\"primary\" href={EDIT + id}>edit</Link>;\n}\n",
        ),
        ("src/lib/routes.ts", "export const EDIT = \"/notes/\";\n"),
        ("src/app/notes/[id]/page.tsx", HOOKED_PAGE),
    ]);
    let report = evaluate(dir.path());
    assert_eq!(report.status, ReachabilityStatus::Partial, "{report:?}");
    assert_eq!(report.targets[0].outcome, "unverified");
    assert!(report.targets[0].reached_via.is_none());
    assert!(report.reached_routes.is_empty());
    assert!(
        report
            .unverifiable
            .contains(&"route_shaped_literal:/notes/[id]".to_string())
    );
}

#[test]
fn constants_do_not_count_without_an_unresolved_navigation() {
    let dir = app(&[
        ("src/app/page.tsx", HOOKED_PAGE),
        ("src/lib/routes.ts", "export const EDIT = \"/notes/1\";\n"),
        ("src/app/notes/[id]/page.tsx", HOOKED_PAGE),
    ]);
    assert_eq!(evaluate(dir.path()).status, ReachabilityStatus::Failed);
}

#[test]
fn route_groups_are_dropped_and_api_pages_are_not_targets() {
    let dir = app(&[
        (
            "src/app/(main)/page.tsx",
            &home_with("<Link href=\"/settings\">s</Link>"),
        ),
        ("src/app/(main)/(admin)/settings/page.tsx", HOOKED_PAGE),
        ("src/app/(main)/(admin)/billing/page.tsx", HOOKED_PAGE),
        (
            "src/app/api/notes/route.ts",
            "export const MARK = \"data-anvil-action\";\n",
        ),
        ("src/app/api/ping/page.tsx", HOOKED_PAGE),
        ("src/app/_private/page.tsx", HOOKED_PAGE),
    ]);
    let report = evaluate(dir.path());
    assert_eq!(report.status, ReachabilityStatus::Failed, "{report:?}");
    assert_eq!(
        report.violations,
        vec!["route_unreachable_from_entry:/billing".to_string()]
    );
    let routes: Vec<&str> = report.targets.iter().map(|t| t.route.as_str()).collect();
    assert_eq!(routes, vec!["/billing", "/settings"]);
}

#[test]
fn every_navigation_form_reaches_the_route() {
    let forms = [
        "<a href=\"/notes/new\">x</a>",
        "<Link href='/notes/new'>x</Link>",
        "<button onClick={() => router.push(\"/notes/new\")}>x</button>",
        "<button onClick={() => router.replace(`/notes/${id}`)}>x</button>",
        "<button onClick={() => useRouter().push(\"/notes/1\")}>x</button>",
        "<button onClick={() => redirect(\"/notes/1\")}>x</button>",
        "<button onClick={() => permanentRedirect(\"/notes/1\")}>x</button>",
        "<button onClick={() => { window.location.href = \"/notes/1\"; }}>x</button>",
        "<button onClick={() => location.assign(\"/notes/1\")}>x</button>",
        "<button onClick={() => location.replace(\"/notes/1\")}>x</button>",
        "<button onClick={() => window.open(\"/notes/1\")}>x</button>",
        "<form action=\"/notes/1\"><button>x</button></form>",
        "<form onSubmit={(e) => e.preventDefault()} action={`/notes/${id}`}><button>x</button></form>",
    ];
    for form in forms {
        let report = detail_route_outcome(form);
        assert_eq!(
            report.status,
            ReachabilityStatus::Pass,
            "{form}: {report:?}"
        );
    }
}

#[test]
fn static_argument_shapes_are_read() {
    let shapes = [
        "<Link href={\"/notes/\" + note.id}>x</Link>",
        "<Link href={\"/notes/\" + String(note.id) + \"/\"}>x</Link>",
        "<Link href={base + \"/notes/\" + id}>x</Link>",
        "<Link href={`${base}/notes/${id}`}>x</Link>",
        "<Link href={cond ? \"/about\" : `/notes/${id}`}>x</Link>",
        "<Link href={maybe || \"/notes/1\"}>x</Link>",
        "<Link href={ready && \"/notes/1\"}>x</Link>",
        "<Link href={{ pathname: \"/notes/[id]\", query: { id } }}>x</Link>",
        "<Link href={{ pathname: `/notes/${id}` }}>x</Link>",
        "<Link href=\"/notes/7?tab=edit#top\">x</Link>",
        "<Link href={(`/notes/${id}` as const)}>x</Link>",
    ];
    for shape in shapes {
        let report = detail_route_outcome(shape);
        assert_eq!(
            report.status,
            ReachabilityStatus::Pass,
            "{shape}: {report:?}"
        );
    }
}

#[test]
fn navigations_to_other_places_do_not_reach_the_route() {
    let shapes = [
        "<a href=\"https://example.com/notes/1\">x</a>",
        "<a href=\"mailto:a@b.c\">x</a>",
        "<a href=\"#notes\">x</a>",
        "<a href=\"//example.com/notes/1\">x</a>",
        "<Link href=\"/about\">x</Link>",
        "<Link href=\"/notes\">x</Link>",
        "<Link href=\"/notes/1/edit\">x</Link>",
        "{/* <Link href=\"/notes/1\">x</Link> */}",
    ];
    for shape in shapes {
        let report = detail_route_outcome(shape);
        assert_eq!(
            report.status,
            ReachabilityStatus::Failed,
            "{shape}: {report:?}"
        );
    }
}

#[test]
fn line_comment_navigation_does_not_reach_the_route() {
    let home = format!(
        "{}// router.push(\"/notes/1\");\n",
        home_with("<p>x</p>").replace("</main>", "</main>\n")
    );
    let dir = app(&[
        ("src/app/page.tsx", &home),
        ("src/app/notes/[id]/page.tsx", HOOKED_PAGE),
    ]);
    assert_eq!(evaluate(dir.path()).status, ReachabilityStatus::Failed);
}

#[test]
fn relative_and_computed_hrefs_are_unverified_not_failed() {
    for shape in [
        "<Link href=\"notes/1\">x</Link>",
        "<Link href={noteHref(note)}>x</Link>",
        "<Link href={`${id}`}>x</Link>",
        "<Link href={{ query: { id } }}>x</Link>",
        "<Link href={item.href}>x</Link>",
    ] {
        let report = detail_route_outcome(shape);
        assert_eq!(
            report.status,
            ReachabilityStatus::Partial,
            "{shape}: {report:?}"
        );
    }
}

#[test]
fn form_action_function_is_not_a_navigation() {
    let report = detail_route_outcome("<form action={createNote}><button>x</button></form>");
    assert_eq!(report.status, ReachabilityStatus::Failed, "{report:?}");
}

#[test]
fn navigation_in_a_layout_and_in_an_imported_component_counts() {
    let dir = app(&[
        (
            "src/app/layout.tsx",
            "import Nav from \"@/components/Nav\";\nexport default function Layout({ children }) { return <body><Nav />{children}</body>; }\n",
        ),
        (
            "src/components/Nav.tsx",
            "import Link from \"next/link\";\nexport default function Nav() { return <Link href=\"/notes/new\">new</Link>; }\n",
        ),
        ("src/app/page.tsx", HOOKED_PAGE),
        ("src/app/notes/[id]/page.tsx", HOOKED_PAGE),
    ]);
    let report = evaluate(dir.path());
    assert_eq!(report.status, ReachabilityStatus::Pass, "{report:?}");
    assert!(
        report
            .entry_sources
            .contains(&"src/app/layout.tsx".to_string())
    );
}

#[test]
fn catch_all_and_optional_catch_all_routes_match_by_depth() {
    let dir = app(&[
        (
            "app/page.tsx",
            &home_with("<Link href=\"/docs\">d</Link><Link href=\"/blog/2024/a\">b</Link>"),
        ),
        ("app/docs/[[...slug]]/page.tsx", HOOKED_PAGE),
        ("app/blog/[...slug]/page.tsx", HOOKED_PAGE),
        ("app/shop/[...slug]/page.tsx", HOOKED_PAGE),
    ]);
    let report = evaluate(dir.path());
    assert_eq!(
        report.violations,
        vec!["route_unreachable_from_entry:/shop/[...slug]".to_string()],
        "{report:?}"
    );
    let reached: Vec<&str> = report
        .targets
        .iter()
        .filter(|target| target.outcome == "reached")
        .map(|target| target.route.as_str())
        .collect();
    assert_eq!(reached, vec!["/blog/[...slug]", "/docs/[[...slug]]"]);
}

#[test]
fn nested_project_directory_is_resolved_like_the_route_closure() {
    let dir = app(&[
        ("package.json", "{}"),
        ("web/package.json", "{}"),
        ("web/src/app/page.tsx", HOOKED_PAGE),
        ("web/src/app/notes/[id]/page.tsx", HOOKED_PAGE),
    ]);
    // The root has a package.json, so the root itself is the project and the
    // nested app is not scanned.
    assert_eq!(evaluate(dir.path()).status, ReachabilityStatus::Pass);
    let nested = app(&[
        ("web/package.json", "{}"),
        ("web/src/app/page.tsx", HOOKED_PAGE),
        ("web/src/app/notes/[id]/page.tsx", HOOKED_PAGE),
    ]);
    let report = evaluate(nested.path());
    assert_eq!(report.status, ReachabilityStatus::Failed, "{report:?}");
    assert_eq!(report.targets[0].page, "web/src/app/notes/[id]/page.tsx");
}

#[test]
fn merged_status_is_never_better_than_either_input() {
    let rank = |status: &str| match status {
        "pass" => 0,
        "partial" => 1,
        "failed" => 2,
        other => panic!("unexpected status {other}"),
    };
    for testimony_failed in [false, true] {
        for reach in [
            ReachabilityStatus::Pass,
            ReachabilityStatus::Partial,
            ReachabilityStatus::Failed,
        ] {
            let report = ReachabilityReport {
                status: reach,
                ..ReachabilityReport::default()
            };
            let (status, _) = merge_with_testimony(testimony_failed, Vec::new(), &report);
            let worst = rank(reach.as_str()).max(if testimony_failed { 2 } else { 0 });
            assert_eq!(
                rank(status),
                worst,
                "testimony_failed={testimony_failed} {reach:?}"
            );
        }
    }
}

#[test]
fn merged_reasons_keep_testimony_first_then_violations_then_unverified() {
    let report = ReachabilityReport {
        status: ReachabilityStatus::Failed,
        violations: vec!["route_unreachable_from_entry:/a".to_string()],
        unverified: vec!["route_reachability_unverified:/b".to_string()],
        ..ReachabilityReport::default()
    };
    let (status, reasons) = merge_with_testimony(
        true,
        vec!["testimony_binding_violation:x".to_string()],
        &report,
    );
    assert_eq!(status, "failed");
    assert_eq!(
        reasons,
        vec![
            "testimony_binding_violation:x",
            "route_unreachable_from_entry:/a",
            "route_reachability_unverified:/b",
        ]
    );
}

#[test]
fn evidence_uses_the_testimony_envelope_shape() {
    let dir = app(&[
        ("src/app/page.tsx", HOOKED_PAGE),
        ("src/app/notes/[id]/page.tsx", HOOKED_PAGE),
    ]);
    let report = evaluate(dir.path());
    write_evidence(dir.path(), &report).unwrap();
    let document: Value = serde_json::from_str(
        &std::fs::read_to_string(dir.path().join(EVIDENCE_RELATIVE_PATH)).unwrap(),
    )
    .unwrap();
    assert_eq!(document["check_id"], "nextjs_route_reachability");
    assert_eq!(document["status"], "failed");
    assert_eq!(document["evidence_envelope"]["family"], "T");
    assert_eq!(document["evidence_envelope"]["kind"], "route_reachability");
    assert_eq!(document["targets"][0]["route"], "/notes/[id]");
    assert_eq!(document["targets"][0]["outcome"], "unreachable");
    assert_eq!(
        document["violations"][0],
        "route_unreachable_from_entry:/notes/[id]"
    );
}

#[test]
fn corpus_l113_shape_is_failed_with_the_pinned_reason() {
    let case = Path::new("tests/corpus/apps/nextjs-route-unreachable-from-entry");
    let expected: Value =
        serde_json::from_str(&std::fs::read_to_string(case.join("expected-gate.json")).unwrap())
            .unwrap();
    let report = evaluate(case);
    assert_eq!(report.status.as_str(), expected["release_gate_status"]);
    assert_eq!(serde_json::json!(report.reasons()), expected["reasons"]);
}

fn run_production_check(dir: &tempfile::TempDir) -> (&'static str, Vec<String>) {
    let report = ProfileRuntimeRegistry::resolve(&ProfileId::Nextjs)
        .run_final_testimony_check(dir.path(), None, None)
        .unwrap()
        .expect("nextjs runs the final static check");
    assert!(dir.path().join(EVIDENCE_RELATIVE_PATH).is_file());
    (report.status, report.reasons)
}

#[test]
fn production_check_projects_failed_partial_and_unchanged_pass() {
    let l113 = app(&[
        ("src/app/page.tsx", &home_with("<p>x</p>")),
        ("src/app/notes/[id]/page.tsx", HOOKED_PAGE),
    ]);
    let (status, reasons) = run_production_check(&l113);
    assert_eq!(status, "failed");
    assert_eq!(reasons, vec!["route_unreachable_from_entry:/notes/[id]"]);

    let unresolved = app(&[
        (
            "src/app/page.tsx",
            &home_with("<button onClick={() => router.push(target)}>x</button>"),
        ),
        ("src/app/notes/[id]/page.tsx", HOOKED_PAGE),
    ]);
    let (status, reasons) = run_production_check(&unresolved);
    assert_eq!(status, "partial");
    assert_eq!(reasons, vec!["route_reachability_unverified:/notes/[id]"]);

    let l111 = app(&[
        (
            "src/app/page.tsx",
            &home_with("<Link href={`/notes/${n.id}`}>x</Link>"),
        ),
        ("src/app/notes/[id]/page.tsx", HOOKED_PAGE),
    ]);
    assert_eq!(run_production_check(&l111), ("pass", Vec::new()));

    let l110 = app(&[("src/app/page.tsx", HOOKED_PAGE)]);
    assert_eq!(run_production_check(&l110), ("pass", Vec::new()));
}

#[test]
fn non_ascii_and_unbalanced_sources_are_scanned_without_panicking() {
    let home = home_with(
        "<p>メモ—don't \"quote</p>\n<Link href={`/メモ/${id}` + '😀'}>開く</Link>\n<a href=\"/notes/1\">日本語</a>\n/* 未終了",
    );
    let dir = app(&[
        ("src/app/page.tsx", &home),
        ("src/app/notes/[id]/page.tsx", HOOKED_PAGE),
    ]);
    assert_eq!(evaluate(dir.path()).status, ReachabilityStatus::Pass);
}

#[test]
fn route_shaped_literal_without_a_recognized_navigation_is_partial() {
    for shape in [
        "<p>{list.push(\"/notes/1\")}</p>",
        "<p>{revalidatePath(\"/notes/1\")}</p>",
        "<div data-href=\"/notes/1\" />",
    ] {
        let report = detail_route_outcome(shape);
        assert_eq!(
            report.status,
            ReachabilityStatus::Partial,
            "{shape}: {report:?}"
        );
        assert!(report.reached_routes.is_empty(), "{shape}");
    }
}

#[test]
fn aliased_router_calls_are_unverified_not_failed() {
    let aliases = [
        "const nav = useRouter();\nconst go = () => nav.push(\"/notes/1\");",
        "const r = useRouter();\nconst go = () => r.push(target);",
        "const { push } = useRouter();\nconst go = () => push(target);",
        "const go = () => navigate(target);\nimport { redirect as navigate } from \"next/navigation\";",
        "import { useRouter as useNav } from \"next/navigation\";",
    ];
    for alias in aliases {
        let report = detail_route_outcome(alias);
        assert_eq!(
            report.status,
            ReachabilityStatus::Partial,
            "{alias}: {report:?}"
        );
        assert!(
            report
                .unverifiable
                .iter()
                .any(|reason| reason.starts_with("router_alias:")),
            "{alias}: {report:?}"
        );
        assert!(report.reached_routes.is_empty(), "{alias}");
    }
}

#[test]
fn plain_router_name_is_not_an_alias() {
    let report = detail_route_outcome("const appRouter = useRouter();");
    assert_eq!(report.status, ReachabilityStatus::Failed, "{report:?}");
}

#[test]
fn imports_the_scan_cannot_follow_make_the_route_unverified() {
    let nav = "import Nav from \"{spec}\";\nexport default function Home() { return <Nav data-anvil-action=\"primary\" />; }\n";
    for spec in [
        "~/components/Nav",
        "#components/Nav",
        "@components/Nav",
        "./Nav.mdx",
        "../lib/nav.mjs",
        "./nav.cjs",
    ] {
        let dir = app(&[
            ("src/app/page.tsx", &nav.replace("{spec}", spec)),
            ("src/app/notes/[id]/page.tsx", HOOKED_PAGE),
            (
                "tsconfig.json",
                "{ // comment\n \"compilerOptions\": { \"paths\": { \"@components/*\": [\"./src/components/*\"], \"@/*\": [\"./src/*\"] } } }",
            ),
        ]);
        let report = evaluate(dir.path());
        assert_eq!(
            report.status,
            ReachabilityStatus::Partial,
            "{spec}: {report:?}"
        );
        assert!(
            report
                .unverifiable
                .iter()
                .any(|reason| reason.contains(spec)),
            "{spec}: {report:?}"
        );
    }
}

#[test]
fn followable_imports_and_packages_do_not_make_the_route_unverified() {
    let dir = app(&[
        (
            "src/app/page.tsx",
            "import Link from \"next/link\";\nimport Nav from \"@/components/Nav\";\nimport { clsx } from \"clsx\";\nimport \"./globals.css\";\nexport default function Home() { return <Nav data-anvil-action=\"primary\" />; }\n",
        ),
        (
            "src/components/Nav.tsx",
            "export default function Nav() { return null; }\n",
        ),
        ("src/app/globals.css", "body {}\n"),
        ("src/app/notes/[id]/page.tsx", HOOKED_PAGE),
        (
            "tsconfig.json",
            "{ \"compilerOptions\": { \"paths\": { \"@/*\": [\"./src/*\"] } } }",
        ),
        (
            "next.config.js",
            "module.exports = { reactStrictMode: true };\n",
        ),
    ]);
    let report = evaluate(dir.path());
    assert_eq!(report.status, ReachabilityStatus::Failed, "{report:?}");
    assert!(report.unverifiable.is_empty(), "{report:?}");
}

#[test]
fn middleware_and_config_rewrites_or_redirects_make_the_route_unverified() {
    let cases = [
        (
            "middleware.ts",
            "export function middleware() {}\n",
            "middleware",
        ),
        (
            "src/middleware.js",
            "export function middleware() {}\n",
            "middleware",
        ),
        (
            "next.config.mjs",
            "export default { async rewrites() { return []; } };\n",
            "next_config_rewrites",
        ),
        (
            "next.config.js",
            "module.exports = { async redirects() { return []; } };\n",
            "next_config_rewrites",
        ),
    ];
    for (path, content, reason) in cases {
        let dir = app(&[
            ("src/app/page.tsx", HOOKED_PAGE),
            ("src/app/notes/[id]/page.tsx", HOOKED_PAGE),
            (path, content),
        ]);
        let report = evaluate(dir.path());
        assert_eq!(
            report.status,
            ReachabilityStatus::Partial,
            "{path}: {report:?}"
        );
        assert!(
            report.unverifiable.contains(&reason.to_string()),
            "{path}: {report:?}"
        );
    }
    let commented = app(&[
        ("src/app/page.tsx", HOOKED_PAGE),
        ("src/app/notes/[id]/page.tsx", HOOKED_PAGE),
        (
            "next.config.js",
            "// rewrites later\nmodule.exports = {};\n",
        ),
    ]);
    assert_eq!(
        evaluate(commented.path()).status,
        ReachabilityStatus::Failed
    );
}

#[test]
fn leading_base_expression_reaches_only_without_skipped_segments() {
    let dir = app(&[
        (
            "src/app/page.tsx",
            &home_with("<Link href={`${prefix}/notes/${id}`}>x</Link>"),
        ),
        ("src/app/[locale]/notes/[id]/page.tsx", HOOKED_PAGE),
        ("src/app/notes/[id]/page.tsx", HOOKED_PAGE),
        ("src/app/[locale]/about/team/page.tsx", HOOKED_PAGE),
    ]);
    let report = evaluate(dir.path());
    let outcome = |route: &str| {
        report
            .targets
            .iter()
            .find(|target| target.route == route)
            .map(|target| target.outcome)
    };
    assert_eq!(outcome("/[locale]/notes/[id]"), Some("unverified"));
    assert_eq!(outcome("/notes/[id]"), Some("reached"));
    assert_eq!(outcome("/[locale]/about/team"), Some("unreachable"));
    assert_eq!(report.status, ReachabilityStatus::Failed, "{report:?}");
}

#[test]
fn nothing_found_stays_failed_for_the_l113_shape_even_with_unrelated_literals() {
    let dir = app(&[
        (
            "src/app/page.tsx",
            &home_with("<p>{fetch(\"/api/notes\")}</p><Link href=\"/about\">a</Link>"),
        ),
        ("src/app/notes/[id]/page.tsx", HOOKED_PAGE),
    ]);
    let report = evaluate(dir.path());
    assert_eq!(report.status, ReachabilityStatus::Failed, "{report:?}");
    assert_eq!(
        report.violations,
        vec!["route_unreachable_from_entry:/notes/[id]".to_string()]
    );
}

#[test]
fn leading_base_expression_never_reaches_a_route_that_needs_skipped_segments() {
    let dir = app(&[
        (
            "src/app/page.tsx",
            &home_with("<Link href={`${prefix}/notes/${id}`}>x</Link>"),
        ),
        ("src/app/admin/notes/[id]/page.tsx", HOOKED_PAGE),
    ]);
    let report = evaluate(dir.path());
    assert_eq!(report.status, ReachabilityStatus::Partial, "{report:?}");
    assert!(report.reached_routes.is_empty(), "{report:?}");
    assert_eq!(report.targets[0].outcome, "unverified");
    assert!(
        report
            .unverifiable
            .contains(&"leading_base_expression:/admin/notes/[id]".to_string())
    );
    assert_eq!(
        report.unverified,
        vec!["route_reachability_unverified:/admin/notes/[id]".to_string()]
    );
}

#[test]
fn route_literal_only_makes_its_own_route_unverified() {
    let dir = app(&[
        (
            "src/app/page.tsx",
            &home_with(
                "<p>{metadata.canonical}</p>\nexport const metadata = { alternates: { canonical: \"/about\" } };",
            ),
        ),
        ("src/app/about/page.tsx", PLAIN_PAGE),
        ("src/app/notes/[id]/page.tsx", HOOKED_PAGE),
        ("src/app/team/page.tsx", HOOKED_PAGE),
    ]);
    let report = evaluate(dir.path());
    assert_eq!(report.status, ReachabilityStatus::Failed, "{report:?}");
    assert!(report.unverified.is_empty(), "{report:?}");
    assert_eq!(
        report.violations,
        vec![
            "route_unreachable_from_entry:/notes/[id]".to_string(),
            "route_unreachable_from_entry:/team".to_string(),
        ]
    );

    let both = app(&[
        (
            "src/app/page.tsx",
            &home_with("<p>{list.push(\"/team\")}</p>"),
        ),
        ("src/app/notes/[id]/page.tsx", HOOKED_PAGE),
        ("src/app/team/page.tsx", HOOKED_PAGE),
    ]);
    let report = evaluate(both.path());
    assert_eq!(
        report.violations,
        vec!["route_unreachable_from_entry:/notes/[id]".to_string()]
    );
    assert_eq!(
        report.unverified,
        vec!["route_reachability_unverified:/team".to_string()]
    );
}

fn outcomes(report: &ReachabilityReport) -> Vec<(&str, &str)> {
    report
        .targets
        .iter()
        .map(|target| (target.route.as_str(), target.outcome))
        .collect()
}

#[test]
fn second_step_behind_a_custom_hook_is_unverified_not_failed() {
    let dir = app(&[
        (
            "src/lib/nav.ts",
            "import { useRouter } from \"next/navigation\";\nexport const useNav = () => useRouter();\n",
        ),
        (
            "src/app/page.tsx",
            "import { useNav } from \"@/lib/nav\";\nexport default function Home() {\n  const nav = useNav();\n  return <button data-anvil-action=\"primary\" onClick={() => nav.push(\"/notes/1\")}>Go</button>;\n}\n",
        ),
        (
            "src/app/notes/[id]/page.tsx",
            "import { useNav } from \"@/lib/nav\";\nexport default function Note() {\n  const nav = useNav();\n  return <button data-anvil-action=\"primary\" onClick={() => nav.push(`/notes/${id}/edit`)}>Edit</button>;\n}\n",
        ),
        ("src/app/notes/[id]/edit/page.tsx", HOOKED_PAGE),
    ]);
    let report = evaluate(Path::new(dir.path()));
    assert_eq!(report.status, ReachabilityStatus::Partial, "{report:?}");
    assert_eq!(
        outcomes(&report),
        vec![
            ("/notes/[id]", "unverified"),
            ("/notes/[id]/edit", "unverified")
        ]
    );
    assert!(report.reached_routes.is_empty());
    assert!(report.violations.is_empty());
}

#[test]
fn second_step_behind_a_leading_base_link_is_unverified_not_failed() {
    let dir = app(&[
        (
            "src/app/page.tsx",
            &home_with("<Link href={`${prefix}/notes/${id}`}>x</Link>"),
        ),
        (
            "src/app/[locale]/notes/[id]/page.tsx",
            "import Link from \"next/link\";\nexport default function P() {\n  return <Link data-anvil-action=\"primary\" href={`${prefix}/notes/${id}/edit`}>edit</Link>;\n}\n",
        ),
        ("src/app/[locale]/notes/[id]/edit/page.tsx", HOOKED_PAGE),
    ]);
    let report = evaluate(dir.path());
    assert_eq!(report.status, ReachabilityStatus::Partial, "{report:?}");
    assert_eq!(
        outcomes(&report),
        vec![
            ("/[locale]/notes/[id]", "unverified"),
            ("/[locale]/notes/[id]/edit", "unverified")
        ]
    );
    assert!(report.reached_routes.is_empty());
}

#[test]
fn second_step_without_any_link_stays_failed() {
    let dir = app(&[
        (
            "src/app/page.tsx",
            &home_with("<p>{list.push(\"/notes/1\")}</p>"),
        ),
        ("src/app/notes/[id]/page.tsx", HOOKED_PAGE),
        ("src/app/notes/[id]/edit/page.tsx", HOOKED_PAGE),
    ]);
    let report = evaluate(dir.path());
    assert_eq!(report.status, ReachabilityStatus::Failed, "{report:?}");
    assert_eq!(
        outcomes(&report),
        vec![
            ("/notes/[id]", "unverified"),
            ("/notes/[id]/edit", "unreachable")
        ]
    );
    assert_eq!(
        report.violations,
        vec!["route_unreachable_from_entry:/notes/[id]/edit".to_string()]
    );
}

#[test]
fn recognized_navigation_from_a_maybe_reached_page_is_not_a_reach() {
    let dir = app(&[
        (
            "src/app/page.tsx",
            &home_with("<p>{list.push(\"/notes/1\")}</p>"),
        ),
        (
            "src/app/notes/[id]/page.tsx",
            "import Link from \"next/link\";\nexport default function P() {\n  return <Link data-anvil-action=\"primary\" href=\"/notes/1/edit\">edit</Link>;\n}\n",
        ),
        ("src/app/notes/[id]/edit/page.tsx", HOOKED_PAGE),
    ]);
    let report = evaluate(dir.path());
    assert_eq!(report.status, ReachabilityStatus::Partial, "{report:?}");
    assert!(report.reached_routes.is_empty(), "{report:?}");
    assert!(
        report
            .targets
            .iter()
            .all(|target| target.reached_via.is_none())
    );
    assert_eq!(
        outcomes(&report),
        vec![
            ("/notes/[id]", "unverified"),
            ("/notes/[id]/edit", "unverified")
        ]
    );
}

#[test]
fn reason_found_only_beyond_the_certain_closure_makes_other_routes_unverified() {
    let dir = app(&[
        (
            "src/app/page.tsx",
            &home_with("<p>{list.push(\"/notes/1\")}</p>"),
        ),
        (
            "src/app/notes/[id]/page.tsx",
            "export default function P() {\n  return <button data-anvil-action=\"primary\" onClick={() => router.push(target)}>x</button>;\n}\n",
        ),
        ("src/app/team/page.tsx", HOOKED_PAGE),
    ]);
    let report = evaluate(dir.path());
    assert_eq!(report.status, ReachabilityStatus::Partial, "{report:?}");
    assert_eq!(
        outcomes(&report),
        vec![("/notes/[id]", "unverified"), ("/team", "unverified")]
    );
    assert!(report.reached_routes.is_empty());
}

fn markdown_home(string_expression: &str) -> String {
    format!(
        "export default function Home() {{\n  const html = text.replace(/\\[(.+)\\]\\((.+)\\)/g, {string_expression});\n  return <main><button data-anvil-action=\"primary\">New</button><div dangerouslySetInnerHTML={{{{ __html: html }}}} /></main>;\n}}\n"
    )
}

fn detail_outcome_with_home(home: &str) -> ReachabilityReport {
    let dir = app(&[
        ("src/app/page.tsx", home),
        ("src/app/notes/[id]/page.tsx", HOOKED_PAGE),
    ]);
    evaluate(dir.path())
}

#[test]
fn substitution_href_inside_a_js_string_is_not_an_unresolved_navigation() {
    let report =
        detail_outcome_with_home(&markdown_home("'<a href=\"$2\" target=\"_blank\">$1</a>'"));
    assert_eq!(report.status, ReachabilityStatus::Failed, "{report:?}");
    assert_eq!(
        report.violations,
        vec!["route_unreachable_from_entry:/notes/[id]".to_string()]
    );
    assert!(report.unverified.is_empty() && report.unresolved_navigations.is_empty());
    assert_eq!(report.targets[0].outcome, "unreachable");
    assert!(report.targets[0].repair.is_some());
}

#[test]
fn only_replacement_references_inside_a_js_string_href_count_as_nothing() {
    for value in ["$1", "$&", "$1$2"] {
        let report =
            detail_outcome_with_home(&markdown_home(&format!("'<a href=\"{value}\">x</a>'")));
        assert_eq!(
            report.status,
            ReachabilityStatus::Failed,
            "{value}: {report:?}"
        );
        assert!(report.unresolved_navigations.is_empty(), "{value}");
    }
    for value in ["$1/x", "$name"] {
        let report =
            detail_outcome_with_home(&markdown_home(&format!("'<a href=\"{value}\">x</a>'")));
        assert_eq!(
            report.status,
            ReachabilityStatus::Partial,
            "{value}: {report:?}"
        );
    }
}

#[test]
fn internal_href_inside_a_js_string_still_reaches_the_route() {
    let report = detail_outcome_with_home(&markdown_home("'<a href=\"/notes/abc\">open</a>'"));
    assert_reached(&report);
    assert_eq!(
        report.targets[0].reached_via.as_ref().unwrap().syntax,
        "href"
    );
    assert!(report.targets[0].repair.is_none());
}

#[test]
fn internal_href_inside_a_template_string_still_reaches_the_route() {
    let template = detail_outcome_with_home(&markdown_home("`<a href=\"/notes/${id}\">open</a>`"));
    assert_reached(&template);
}

#[test]
fn escaped_quote_internal_href_stays_partial_not_reached() {
    let escaped =
        detail_outcome_with_home(&markdown_home("\"<a href=\\\"/notes/abc\\\">open</a>\""));
    assert_eq!(escaped.status, ReachabilityStatus::Partial, "{escaped:?}");
    assert!(escaped.reached_routes.is_empty());
}

#[test]
fn external_hrefs_inside_a_js_string_count_as_nothing() {
    for value in [
        "https://example.com",
        "mailto:a@b.c",
        "",
        "#top",
        "//cdn.example.com",
    ] {
        let report =
            detail_outcome_with_home(&markdown_home(&format!("'<a href=\"{value}\">x</a>'")));
        assert_eq!(
            report.status,
            ReachabilityStatus::Failed,
            "{value}: {report:?}"
        );
        assert!(
            report.unresolved_navigations.is_empty(),
            "{value}: {report:?}"
        );
    }
}

#[test]
fn escaped_quote_substitution_href_keeps_the_ordinary_reading() {
    let report = detail_outcome_with_home(&markdown_home("\"<a href=\\\"$2\\\">$1</a>\""));
    assert_eq!(report.status, ReachabilityStatus::Partial, "{report:?}");
}

#[test]
fn jsx_href_with_an_unresolved_expression_stays_partial() {
    let report = detail_route_outcome("<Link href={target}>go</Link>");
    assert_eq!(report.status, ReachabilityStatus::Partial, "{report:?}");
    assert_eq!(report.unresolved_navigations.len(), 1);
    assert_eq!(report.unresolved_navigations[0].syntax, "href");
}

#[test]
fn jsx_attribute_href_substitution_text_stays_unresolved() {
    let report = detail_route_outcome("<a href=\"$2\">go</a>");
    assert_eq!(report.status, ReachabilityStatus::Partial, "{report:?}");
}

#[test]
fn string_href_reading_never_raises_the_status() {
    // A string href either reaches a route (as before) or counts as nothing;
    // it never turns a failed final into partial or pass.
    let report = detail_outcome_with_home(&markdown_home(
        "'<a href=\"$2\">$1</a><a href=\"https://x.test\">y</a>'",
    ));
    assert_eq!(report.status, ReachabilityStatus::Failed, "{report:?}");
    assert!(report.reached_routes.is_empty());
}

#[test]
fn concatenated_or_variable_href_inside_a_js_string_stays_partial() {
    for expression in [
        "'<a href=\"' + note.url + '\">x</a>'",
        "`<a href=\"${note.url}\">x</a>`",
        "'<a href=\"notes/1\">x</a>'",
    ] {
        let report = detail_outcome_with_home(&markdown_home(expression));
        assert_eq!(
            report.status,
            ReachabilityStatus::Partial,
            "{expression}: {report:?}"
        );
        assert!(report.violations.is_empty(), "{expression}: {report:?}");
    }
}

#[test]
fn relative_href_between_two_apostrophes_in_jsx_text_stays_partial() {
    let home = home_with("<p>Don't <a href=\"notes/1\">go</a> isn't</p>");
    let report = detail_outcome_with_home(&home);
    assert_eq!(report.status, ReachabilityStatus::Partial, "{report:?}");
    assert!(report.violations.is_empty());
}

#[test]
fn template_href_with_a_leading_base_substitution_is_not_reached() {
    let report =
        detail_outcome_with_home(&markdown_home("`<a href=\"${base}/notes/${id}\">open</a>`"));
    assert_eq!(report.status, ReachabilityStatus::Partial, "{report:?}");
    assert!(report.reached_routes.is_empty());
}

#[test]
fn template_href_substitution_does_not_match_a_static_segment() {
    let dir = app(&[
        (
            "src/app/page.tsx",
            &markdown_home("`<a href=\"/notes/${id}\">open</a>`"),
        ),
        ("src/app/notes/[id]/page.tsx", HOOKED_PAGE),
        ("src/app/notes/archive/page.tsx", HOOKED_PAGE),
    ]);
    let report = evaluate(dir.path());
    assert_eq!(report.status, ReachabilityStatus::Failed, "{report:?}");
    assert_eq!(report.reached_routes, vec!["/notes/[id]".to_string()]);
    assert_eq!(
        report.violations,
        vec!["route_unreachable_from_entry:/notes/archive".to_string()]
    );
}

#[test]
fn route_shaped_concatenation_inside_a_js_string_still_reaches_the_route() {
    let report = detail_outcome_with_home(&markdown_home(
        "'<a href=\"/notes/' + note.id + '\">' + note.title + '</a>'",
    ));
    assert_reached(&report);
}

#[test]
fn multi_line_route_shaped_concatenation_stays_partial_not_reached() {
    let report = detail_outcome_with_home(&markdown_home(
        "'<a href=\"/notes/' +\n    note.id +\n    '\">x</a>'",
    ));
    assert_eq!(report.status, ReachabilityStatus::Partial, "{report:?}");
    assert!(report.reached_routes.is_empty());
}

#[test]
fn route_shaped_concatenation_does_not_match_a_static_segment() {
    let dir = app(&[
        (
            "src/app/page.tsx",
            &markdown_home("'<a href=\"/notes/' + note.id + '\">x</a>'"),
        ),
        ("src/app/notes/[id]/page.tsx", HOOKED_PAGE),
        ("src/app/notes/archive/page.tsx", HOOKED_PAGE),
    ]);
    let report = evaluate(dir.path());
    assert_eq!(report.status, ReachabilityStatus::Failed, "{report:?}");
    assert_eq!(
        report.violations,
        vec!["route_unreachable_from_entry:/notes/archive".to_string()]
    );
}
