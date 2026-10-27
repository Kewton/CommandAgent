//! Pins `apply_final_testimony_check`'s handling of a `partial` static check:
//! it can lower a passing gate to `partial`, never raises a gate, and never
//! softens `failed`. The check runs through the production Next.js runtime
//! with workspaces that make the reachability check return each status.
use std::path::Path;

use clap::Parser;

use super::{
    Config, ProfileId, ProfileRuntimeRegistry, ReleaseGateSummary, RuntimeAcceptanceReport,
    UltraPlan, VerificationReport, final_acceptance_release_gate_with_runtime,
    ultra_final_acceptance_report_inner,
};
use crate::cli::Cli;
use crate::minimal_loop::repair_target::{RepairTarget, classify_repair_target};
use crate::planner::profile::{early_static_repair_lines, release_gate_static_repair_lines};
use crate::planner::runner::final_acceptance::final_acceptance_model_fixable_profile_failures;

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

// Early static repair guidance (Issue #890): `repair guidance:` lines only; the
// gate, the repair classification and the no-line cases stay as before.
const RUNTIME_FAILURE: &str = "runtime acceptance failed: persistence_evidence";
const GUIDANCE_LINE: &str = "repair guidance: route_unreachable_from_entry:/notes/[id] (src/app/notes/[id]/page.tsx): add a link from the entry page to this route, or delete the page if it is not used";

/// `/` has no navigation toward the hook-carrying `/notes/[id]`.
fn unreachable_workspace(root: &Path) {
    write(root, "src/app/page.tsx", HOOKED_PAGE);
    write(root, "src/app/notes/[id]/page.tsx", HOOKED_PAGE);
}

/// `/` reaches `/notes/[id]` only through a router call with a computed target.
fn unverified_workspace(root: &Path) {
    write(
        root,
        "src/app/page.tsx",
        "import { useRouter } from \"next/navigation\";\nexport default function Home() {\n  const router = useRouter();\n  return <button data-anvil-action=\"primary\" onClick={() => router.push(target)}>Go</button>;\n}\n",
    );
    write(root, "src/app/notes/[id]/page.tsx", HOOKED_PAGE);
}

fn nextjs_lines(root: &Path, runtime_passed: bool, build_failed: bool) -> Vec<String> {
    early_static_repair_lines(
        ProfileRuntimeRegistry::resolve(&ProfileId::Nextjs),
        root,
        runtime_passed,
        build_failed,
    )
}

fn failed_runtime_gate(root: &Path) -> ReleaseGateSummary {
    let mut config = Config::from_cli(Cli::parse_from(["commandagent", "--ux-demo"])).unwrap();
    config.workspace_root = root.to_path_buf();
    final_acceptance_release_gate_with_runtime(
        &config,
        ProfileRuntimeRegistry::resolve(&ProfileId::Nextjs),
        ABOUT_GOAL,
        &[],
        Some(&RuntimeAcceptanceReport {
            passed: false,
            primary_reason: RUNTIME_FAILURE.to_string(),
            ..RuntimeAcceptanceReport::default()
        }),
        true,
    )
}

#[test]
fn runtime_failure_round_gets_one_guidance_line_for_the_unreachable_page() {
    let dir = tempfile::tempdir().unwrap();
    unreachable_workspace(dir.path());
    assert_eq!(
        nextjs_lines(dir.path(), false, false),
        vec![GUIDANCE_LINE.to_string()]
    );
}

#[test]
fn guidance_is_empty_for_reached_unverified_and_non_nextjs_profiles() {
    let reached = tempfile::tempdir().unwrap();
    write(
        reached.path(),
        "src/app/page.tsx",
        "import Link from \"next/link\";\nexport default function Home() {\n  return <Link data-anvil-action=\"primary\" href={`/notes/${id}`}>Go</Link>;\n}\n",
    );
    write(reached.path(), "src/app/notes/[id]/page.tsx", HOOKED_PAGE);
    assert!(nextjs_lines(reached.path(), false, false).is_empty());

    let unverified = tempfile::tempdir().unwrap();
    unverified_workspace(unverified.path());
    assert!(nextjs_lines(unverified.path(), false, false).is_empty());

    let other = tempfile::tempdir().unwrap();
    unreachable_workspace(other.path());
    assert!(
        early_static_repair_lines(
            ProfileRuntimeRegistry::resolve(&ProfileId::Generic),
            other.path(),
            false,
            false,
        )
        .is_empty()
    );
}

#[test]
fn runtime_passed_and_build_failed_rounds_get_no_guidance_line() {
    let dir = tempfile::tempdir().unwrap();
    unreachable_workspace(dir.path());
    assert!(nextjs_lines(dir.path(), true, false).is_empty());
    assert!(nextjs_lines(dir.path(), false, true).is_empty());
    assert!(nextjs_lines(dir.path(), true, true).is_empty());
}

#[test]
fn release_gate_for_a_failed_runtime_is_the_same_with_an_unreachable_page() {
    let reachable_free = tempfile::tempdir().unwrap();
    write(reachable_free.path(), "src/app/page.tsx", HOOKED_PAGE);
    let unreachable = tempfile::tempdir().unwrap();
    unreachable_workspace(unreachable.path());

    for dir in [&reachable_free, &unreachable] {
        let gate = failed_runtime_gate(dir.path());
        assert_eq!(gate.status, "failed", "{gate:?}");
        assert_eq!(gate.reasons, vec![RUNTIME_FAILURE.to_string()], "{gate:?}");
        assert_eq!(gate.browser_readiness_status, "not_checked", "{gate:?}");
        assert_eq!(gate.interaction_evidence_status, "not_checked", "{gate:?}");
    }
}

#[test]
fn guidance_line_does_not_change_the_repair_classification() {
    for (route, page) in [
        ("/notes/[id]", "src/app/notes/[id]/page.tsx"),
        ("/layout-demo", "src/app/layout-demo/page.tsx"),
        ("/css", "src/app/css/page.tsx"),
    ] {
        let mut base = VerificationReport::pass();
        base.push_profile_failure(RUNTIME_FAILURE.to_string());
        let mut with_guidance = base.clone();
        with_guidance.push_profile_failure(format!(
            "repair guidance: route_unreachable_from_entry:{route} ({page}): add a link from the entry page to this route, or delete the page if it is not used"
        ));
        assert_eq!(
            classify_repair_target(&with_guidance),
            classify_repair_target(&base),
            "{route}"
        );
        let prompt_lines = final_acceptance_model_fixable_profile_failures(&with_guidance);
        assert_eq!(prompt_lines, with_guidance.profile_failures, "{route}");
    }
}

#[test]
fn final_acceptance_report_with_a_missing_build_setup_carries_no_guidance() {
    let dir = tempfile::tempdir().unwrap();
    unreachable_workspace(dir.path());
    let mut config = Config::from_cli(Cli::parse_from(["commandagent", "--ux-demo"])).unwrap();
    config.workspace_root = dir.path().to_path_buf();
    config.eval_events_path = Some(dir.path().join("events.jsonl"));
    let plan = UltraPlan {
        goal: ABOUT_GOAL.to_string(),
        profile: ProfileId::Nextjs.to_string(),
        style: "default".to_string(),
        intent: "create".to_string(),
        phases: Vec::new(),
    };
    let report = ultra_final_acceptance_report_inner(&plan, &config, 0).unwrap();
    assert!(!report.is_pass(), "{report:?}");
    assert!(
        report
            .dependency_missing
            .iter()
            .any(|reason| reason.contains("dependency_setup_missing")),
        "the workspace must count as a failed production build"
    );
    assert!(
        !report
            .profile_failures
            .iter()
            .any(|failure| failure.starts_with("repair guidance: route_unreachable_from_entry")),
        "{:?}",
        report.profile_failures
    );
}

// Release gate repair guidance after a passing runtime (Issue #893, IC-40):
// the gate is unchanged; `repair guidance:` lines follow `release gate failed:`
// only for the routes the gate names, and the repair targets the app source.
fn passed_runtime_lines(root: &Path, gate: &ReleaseGateSummary) -> Vec<String> {
    release_gate_static_repair_lines(
        ProfileRuntimeRegistry::resolve(&ProfileId::Nextjs),
        root,
        true,
        false,
        &gate.reasons,
    )
}

/// The `release gate failed:` branch of the final acceptance report.
fn failed_gate_report(root: &Path, gate: &ReleaseGateSummary) -> VerificationReport {
    let mut report = VerificationReport::pass();
    report.push_profile_failure(format!("release gate failed: {}", gate.reasons.join("; ")));
    for line in passed_runtime_lines(root, gate) {
        report.push_profile_failure(line);
    }
    report
}

#[test]
fn passed_runtime_release_gate_with_an_unreachable_page_is_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    unreachable_workspace(dir.path());
    assert_eq!(
        gate(dir.path(), ABOUT_GOAL, &[]),
        ReleaseGateSummary {
            status: "failed".to_string(),
            reasons: vec![FAILED_REASON.to_string()],
            browser_readiness_status: "not_applicable".to_string(),
            browser_readiness_evidence_path: String::new(),
            interaction_evidence_status: "not_applicable".to_string(),
            interaction_evidence_path: String::new(),
        }
    );
}

#[test]
fn passed_runtime_unreachable_round_gets_one_guidance_line_after_the_gate_line() {
    let dir = tempfile::tempdir().unwrap();
    unreachable_workspace(dir.path());
    let result = gate(dir.path(), ABOUT_GOAL, &[]);
    let report = failed_gate_report(dir.path(), &result);
    assert_eq!(
        report.profile_failures,
        vec![
            format!("release gate failed: {FAILED_REASON}"),
            GUIDANCE_LINE.to_string(),
        ]
    );
    assert_eq!(
        classify_repair_target(&report),
        RepairTarget::Implementation
    );
    assert_eq!(
        final_acceptance_model_fixable_profile_failures(&report),
        report.profile_failures
    );
}

#[test]
fn passed_runtime_guidance_names_only_the_unreachable_page() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "src/app/page.tsx",
        "import Link from \"next/link\";\nexport default function Home() {\n  return <Link data-anvil-action=\"primary\" href=\"/notes\">Go</Link>;\n}\n",
    );
    write(dir.path(), "src/app/notes/page.tsx", HOOKED_PAGE);
    write(dir.path(), "src/app/orphan/page.tsx", HOOKED_PAGE);
    let result = gate(dir.path(), ABOUT_GOAL, &[]);
    assert_eq!(result.status, "failed", "{result:?}");
    assert_eq!(
        result.reasons,
        vec!["route_unreachable_from_entry:/orphan".to_string()]
    );
    assert_eq!(
        passed_runtime_lines(dir.path(), &result),
        vec![
            "repair guidance: route_unreachable_from_entry:/orphan (src/app/orphan/page.tsx): add a link from the entry page to this route, or delete the page if it is not used"
                .to_string()
        ]
    );
}

#[test]
fn passed_runtime_guidance_is_empty_for_reached_unverified_and_unnamed_routes() {
    let reached = tempfile::tempdir().unwrap();
    reached_workspace(reached.path());
    let reached_gate = gate(reached.path(), ABOUT_GOAL, &[]);
    assert!(passed_runtime_lines(reached.path(), &reached_gate).is_empty());

    let unverified = tempfile::tempdir().unwrap();
    unverified_workspace(unverified.path());
    let unverified_gate = gate(unverified.path(), ABOUT_GOAL, &[]);
    assert_eq!(unverified_gate.reasons, vec![PARTIAL_REASON.to_string()]);
    assert!(passed_runtime_lines(unverified.path(), &unverified_gate).is_empty());

    let unnamed = tempfile::tempdir().unwrap();
    unreachable_workspace(unnamed.path());
    let other_reason = ReleaseGateSummary {
        reasons: vec!["browser_readiness_failed:route_render".to_string()],
        ..gate(unnamed.path(), ABOUT_GOAL, &[])
    };
    assert!(passed_runtime_lines(unnamed.path(), &other_reason).is_empty());
}

#[test]
fn passed_runtime_guidance_is_empty_for_failed_runtime_build_failure_and_other_profiles() {
    let dir = tempfile::tempdir().unwrap();
    unreachable_workspace(dir.path());
    let reasons = vec![FAILED_REASON.to_string()];
    let nextjs = ProfileRuntimeRegistry::resolve(&ProfileId::Nextjs);
    assert!(
        release_gate_static_repair_lines(nextjs, dir.path(), false, false, &reasons).is_empty()
    );
    assert!(release_gate_static_repair_lines(nextjs, dir.path(), true, true, &reasons).is_empty());
    assert!(
        release_gate_static_repair_lines(
            ProfileRuntimeRegistry::resolve(&ProfileId::Generic),
            dir.path(),
            true,
            false,
            &reasons,
        )
        .is_empty()
    );
}

#[test]
fn failed_runtime_round_keeps_only_the_early_guidance_line() {
    let dir = tempfile::tempdir().unwrap();
    unreachable_workspace(dir.path());
    let result = failed_runtime_gate(dir.path());
    let mut lines = nextjs_lines(dir.path(), false, false);
    lines.extend(release_gate_static_repair_lines(
        ProfileRuntimeRegistry::resolve(&ProfileId::Nextjs),
        dir.path(),
        false,
        false,
        &result.reasons,
    ));
    assert_eq!(lines, vec![GUIDANCE_LINE.to_string()]);
}
