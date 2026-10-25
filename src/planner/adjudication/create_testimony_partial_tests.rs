//! Pins `apply_final_testimony_check`'s handling of a `partial` static check:
//! it can lower a passing gate to `partial`, never raises a gate, and never
//! softens `failed`. The check runs through the production Next.js runtime
//! with workspaces that make the reachability check return each status.
use std::path::Path;

use clap::Parser;

use super::{
    Config, ProfileId, ProfileRuntimeRegistry, ReleaseGateSummary, RuntimeAcceptanceReport,
    final_acceptance_release_gate_with_runtime,
};
use crate::cli::Cli;

const HOOKED_PAGE: &str =
    "export default function P() { return <button data-anvil-action=\"primary\">Go</button>; }\n";
const ABOUT_GOAL: &str = "Create a static about page";
const PARTIAL_REASON: &str = "route_reachability_unverified:/notes/[id]";
const FAILED_REASON: &str = "route_unreachable_from_entry:/notes/[id]";

fn write(root: &Path, relative: &str, content: &str) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

/// `/` reaches `/notes/[id]` only through a router call with a computed target.
fn partial_workspace(root: &Path) {
    write(
        root,
        "src/app/page.tsx",
        "import { useRouter } from \"next/navigation\";\nexport default function Home() {\n  const router = useRouter();\n  return <button data-anvil-action=\"primary\" onClick={() => router.push(target)}>Go</button>;\n}\n",
    );
    write(root, "src/app/notes/[id]/page.tsx", HOOKED_PAGE);
}

/// `/` has no navigation toward the hook-carrying `/notes/[id]`.
fn failed_workspace(root: &Path) {
    write(root, "src/app/page.tsx", HOOKED_PAGE);
    write(root, "src/app/notes/[id]/page.tsx", HOOKED_PAGE);
}

fn reached_workspace(root: &Path) {
    write(
        root,
        "src/app/page.tsx",
        "import Link from \"next/link\";\nexport default function Home() {\n  return <Link data-anvil-action=\"primary\" href={`/notes/${id}`}>Go</Link>;\n}\n",
    );
    write(root, "src/app/notes/[id]/page.tsx", HOOKED_PAGE);
}

fn gate(root: &Path, goal: &str, capabilities: &[String]) -> ReleaseGateSummary {
    let mut config = Config::from_cli(Cli::parse_from(["commandagent", "--ux-demo"])).unwrap();
    config.workspace_root = root.to_path_buf();
    final_acceptance_release_gate_with_runtime(
        &config,
        ProfileRuntimeRegistry::resolve(&ProfileId::Nextjs),
        goal,
        capabilities,
        Some(&RuntimeAcceptanceReport {
            passed: true,
            primary_reason: "pass".to_string(),
            ..RuntimeAcceptanceReport::default()
        }),
        true,
    )
}

fn has_reason(gate: &ReleaseGateSummary, reason: &str) -> bool {
    gate.reasons.iter().any(|candidate| candidate == reason)
}

#[test]
fn partial_static_check_lowers_a_passing_gate_to_partial_and_adds_the_reason() {
    let dir = tempfile::tempdir().unwrap();
    partial_workspace(dir.path());
    let result = gate(dir.path(), ABOUT_GOAL, &[]);
    assert_eq!(result.status, "partial", "{result:?}");
    assert!(has_reason(&result, PARTIAL_REASON), "{result:?}");
}

#[test]
fn partial_static_check_keeps_an_already_partial_gate_partial() {
    let dir = tempfile::tempdir().unwrap();
    partial_workspace(dir.path());
    write(
        dir.path(),
        "browser-readiness.json",
        r#"{"ok":true,"http_status":200,"route_rendered":true,"has_canvas":false,"interactive_control_count":1,"title_text_excerpt":"Start"}"#,
    );
    let result = gate(
        dir.path(),
        "Create a canvas-based interactive browser game",
        &["player_control".to_string()],
    );
    assert_eq!(result.status, "partial", "{result:?}");
    assert!(has_reason(&result, PARTIAL_REASON), "{result:?}");
    assert!(
        result
            .reasons
            .iter()
            .any(|reason| reason.contains("rendered_without_expected_surface")),
        "{result:?}"
    );
}

#[test]
fn failed_static_check_makes_the_gate_failed() {
    let dir = tempfile::tempdir().unwrap();
    failed_workspace(dir.path());
    let result = gate(dir.path(), ABOUT_GOAL, &[]);
    assert_eq!(result.status, "failed", "{result:?}");
    assert!(has_reason(&result, FAILED_REASON), "{result:?}");
}

#[test]
fn reached_route_leaves_the_gate_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    reached_workspace(dir.path());
    let result = gate(dir.path(), ABOUT_GOAL, &[]);
    assert_eq!(result.status, "pass", "{result:?}");
    assert!(result.reasons.is_empty(), "{result:?}");
}

#[test]
fn static_check_never_raises_a_partial_or_failed_gate() {
    for (workspace, expected) in [
        (reached_workspace as fn(&Path), "partial"),
        (partial_workspace as fn(&Path), "partial"),
    ] {
        let dir = tempfile::tempdir().unwrap();
        workspace(dir.path());
        write(
            dir.path(),
            "browser-readiness.json",
            r#"{"ok":true,"http_status":200,"route_rendered":true,"has_canvas":false,"interactive_control_count":1,"title_text_excerpt":"Start"}"#,
        );
        let result = gate(
            dir.path(),
            "Create a canvas-based interactive browser game",
            &["player_control".to_string()],
        );
        assert_eq!(result.status, expected, "{result:?}");
    }
}
