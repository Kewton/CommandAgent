use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use commandagent::eval_events::{
    CompletionSnapshot, append_completion_summary, project_completion,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CodeLineCounts {
    production: usize,
    test: usize,
}

impl CodeLineCounts {
    fn total(self) -> usize {
        self.production + self.test
    }
}

#[derive(Debug, Clone, Copy)]
struct ChokepointBudget {
    path: &'static str,
    total_baseline: usize,
    production_baseline: usize,
    test_baseline: usize,
}

#[test]
fn generic_profile_reduced_assurance_markers_still_render() {
    let dir = tempfile::tempdir().unwrap();
    let events_path = dir.path().join(".anvil/runs/generic/events.jsonl");
    let mut snapshot = CompletionSnapshot::empty();
    snapshot.assurance_level = "reduced".to_string();
    snapshot.assurance_reason =
        commandagent::eval_events::GENERIC_REDUCED_ASSURANCE_REASON.to_string();
    snapshot.runtime_acceptance_status = "pass".to_string();
    snapshot.final_acceptance_status = "partial".to_string();
    snapshot.release_gate_status = "partial".to_string();
    snapshot.release_gate_reasons = vec![
        "interaction_unverified:probe_unavailable".to_string(),
        "generic_profile_reduced_assurance".to_string(),
    ];
    snapshot.unverified_evidence = vec![
        "stateful_update_evidence:unverified:probe_unavailable".to_string(),
        "browser_interaction:unverified:generic_profile_reduced_assurance".to_string(),
    ];
    snapshot.browser_readiness_status =
        "unavailable:browser_readiness_evidence_missing".to_string();
    snapshot.interaction_evidence_status = "unavailable:interaction_evidence_missing".to_string();
    snapshot.evidence_arbitration_summary = "partial (probe unavailable)".to_string();

    let projection = project_completion(true, &snapshot);
    assert_eq!(projection.status, "complete_with_partial_release_gate");
    assert_eq!(projection.task_status, "partial (interaction unverified)");
    append_completion_summary(
        Some(&events_path),
        "process",
        Some("ultra-plan-run"),
        Some("/ultra-plan-run --profile generic build an interactive app"),
        "completed",
        "",
        &projection,
    );

    let summary =
        std::fs::read_to_string(events_path.parent().unwrap().join("summary.md")).expect("summary");
    for marker in [
        "Task status: partial (interaction unverified)",
        "Final acceptance: partial",
        "Release gate: partial",
        "Release gate reasons:",
        "- interaction_unverified:probe_unavailable",
        "- generic_profile_reduced_assurance",
        "Unverified (probe required):",
        "- stateful_update_evidence:unverified:probe_unavailable",
        "- browser_interaction:unverified:generic_profile_reduced_assurance",
        "Interaction verification: interaction_unverified:probe_unavailable",
        "Next action: run_setup_interaction_probe_to_enable_interaction_release_checks",
        "Assurance: reduced (generic profile — no capability contract, no behavioral verification)",
    ] {
        assert!(
            summary.contains(marker),
            "missing marker {marker:?}\n{summary}"
        );
    }
    assert!(
        !summary.contains("Final acceptance: full_success"),
        "{summary}"
    );
}

#[test]
fn generic_profile_static_assurance_markers_render() {
    let dir = tempfile::tempdir().unwrap();
    let events_path = dir.path().join(".anvil/runs/generic-static/events.jsonl");
    let mut snapshot = CompletionSnapshot::empty();
    snapshot.assurance_level = "static".to_string();
    snapshot.assurance_reason =
        commandagent::eval_events::GENERIC_STATIC_ASSURANCE_REASON.to_string();
    snapshot.runtime_acceptance_status = "pass".to_string();
    snapshot.final_acceptance_status = "full_success".to_string();
    snapshot.release_gate_status = "pass".to_string();

    let projection = project_completion(true, &snapshot);
    assert_eq!(projection.status, "complete");
    assert_eq!(projection.task_status, "completed (static assurance)");
    append_completion_summary(
        Some(&events_path),
        "process",
        Some("ultra-plan-run"),
        Some("/ultra-plan-run --profile generic ちょっとしたメモアプリを作って"),
        "completed",
        "",
        &projection,
    );

    let summary =
        std::fs::read_to_string(events_path.parent().unwrap().join("summary.md")).expect("summary");
    for marker in [
        "Task status: completed (static assurance)",
        "Runtime acceptance: pass",
        "Final acceptance: full_success",
        "Release gate: pass",
        "Assurance: static (generic profile — minimal interactive contract verified statically; no behavioral verification)",
    ] {
        assert!(
            summary.contains(marker),
            "missing marker {marker:?}\n{summary}"
        );
    }
}

#[test]
fn nextjs_boundary_erosion_tripwire_keeps_dispatch_sites_audited() {
    let actual = nextjs_literal_counts_outside_profiles();
    let expected = BTreeMap::from([
        ("src/minimal_loop/browser_probe.rs".to_string(), 2),
        ("src/minimal_loop/evidence.rs".to_string(), 4),
        ("src/minimal_loop/import_scan.rs".to_string(), 1),
        ("src/minimal_loop/loop_run.rs".to_string(), 2),
        ("src/planner/adjudication/create.rs".to_string(), 1),
        ("src/planner/lint.rs".to_string(), 2),
        ("src/planner/runner.rs".to_string(), 13),
        ("src/planner/verify.rs".to_string(), 3),
    ]);
    assert_eq!(
        actual, expected,
        "new production \"nextjs\" literal outside src/planner/profiles must be audited here or moved behind the profile boundary"
    );
}

#[test]
fn adjudication_dependency_direction_stays_create_to_skeleton() {
    for path in [
        "src/planner/adjudication/mod.rs",
        "src/planner/adjudication/contract.rs",
        "src/planner/adjudication/core.rs",
        "src/planner/adjudication/requirements.rs",
        "src/planner/adjudication/terminal.rs",
    ] {
        let source = std::fs::read_to_string(path).expect("read adjudication skeleton module");
        assert!(
            !source.contains("adjudication_create") && !source.contains("create::"),
            "{path} must not depend on the private create adapter"
        );
        let production = production_lines(&source).join("\n");
        for create_token in [
            "browser_readiness",
            "interaction_evidence",
            "nextjs",
            "ReleaseGateSummary",
            "AcceptanceGateTelemetry",
        ] {
            assert!(
                !production.contains(create_token),
                "{path} must not bake create token {create_token:?} into the skeleton"
            );
        }
    }

    let runner = std::fs::read_to_string("src/planner/runner.rs").expect("read runner");
    assert!(
        runner.contains("mod adjudication_create;"),
        "the create adapter must remain private so skeleton modules cannot import it"
    );
    assert!(!runner.contains("pub(crate) mod adjudication_create;"));

    let create =
        std::fs::read_to_string("src/planner/adjudication/create.rs").expect("read create adapter");
    assert!(
        create.contains("use crate::planner::adjudication::{"),
        "the create adapter must consume the intent-independent skeleton"
    );
    let fix = std::fs::read_to_string("src/planner/adjudication/fix.rs").expect("read fix adapter");
    assert!(
        fix.contains("super::contract::{"),
        "the fix adapter must consume the intent-independent contract types"
    );
    let profile = std::fs::read_to_string("src/planner/profile.rs").expect("read profile boundary");
    assert!(
        !profile.contains("adjudication::fix"),
        "profile hooks must return skeleton outcomes without depending on the fix evaluator"
    );
}

#[test]
fn manifest_execution_sections_exclude_measured_fixture_vocabulary() {
    // Heuristic tripwire only: it catches known scenario leakage, not semantic overfitting.
    for (profile, source) in [
        (
            "data",
            include_str!("../src/planner/profiles/data/manifest.toml"),
        ),
        (
            "nextjs",
            include_str!("../src/planner/profiles/nextjs/manifest.toml"),
        ),
    ] {
        let manifest = source.parse::<toml::Value>().unwrap();
        for section in ["plan", "step_templates", "checks"] {
            let value = manifest
                .get(section)
                .unwrap_or_else(|| panic!("{profile} manifest missing {section}"));
            for token in ["sales", "売上", "東京", "大阪", "名古屋"] {
                assert!(
                    !toml_value_contains(value, token),
                    "heuristic overfitting tripwire: {profile}.{section} contains {token:?}; vocabulary is intentionally outside this scan"
                );
            }
        }
    }
}

#[test]
fn runner_chokepoints_do_not_grow_past_interim_budget() {
    for budget in [
        ChokepointBudget {
            path: "src/planner/runner.rs",
            total_baseline: 18_242,
            production_baseline: 9_904,
            test_baseline: 8_339,
        },
        ChokepointBudget {
            path: "src/minimal_loop/loop_run.rs",
            total_baseline: 7_444,
            production_baseline: 4_960,
            test_baseline: 2_485,
        },
        ChokepointBudget {
            path: "src/minimal_loop/loop_run/runtime_bash_policy_telemetry.rs",
            total_baseline: 79,
            production_baseline: 79,
            test_baseline: 0,
        },
        ChokepointBudget {
            path: "src/minimal_loop/repair_pressure.rs",
            total_baseline: 746,
            production_baseline: 278,
            test_baseline: 468,
        },
        ChokepointBudget {
            path: "src/planner/repair_targeting.rs",
            total_baseline: 597,
            production_baseline: 459,
            test_baseline: 138,
        },
        ChokepointBudget {
            path: "src/planner/final_acceptance.rs",
            total_baseline: 2_209,
            production_baseline: 2_204,
            test_baseline: 5,
        },
        ChokepointBudget {
            path: "src/planner/adjudication/mod.rs",
            total_baseline: 7,
            production_baseline: 7,
            test_baseline: 0,
        },
        ChokepointBudget {
            path: "src/planner/adjudication/core.rs",
            total_baseline: 255,
            production_baseline: 255,
            test_baseline: 0,
        },
        ChokepointBudget {
            path: "src/planner/adjudication/contract.rs",
            total_baseline: 279,
            production_baseline: 246,
            test_baseline: 33,
        },
        ChokepointBudget {
            path: "src/planner/adjudication/fix.rs",
            total_baseline: 521,
            production_baseline: 351,
            test_baseline: 170,
        },
        ChokepointBudget {
            path: "src/planner/adjudication/create.rs",
            total_baseline: 2_179,
            production_baseline: 2_165,
            test_baseline: 14,
        },
        ChokepointBudget {
            path: "src/planner/adjudication/requirements.rs",
            total_baseline: 184,
            production_baseline: 110,
            test_baseline: 74,
        },
        ChokepointBudget {
            path: "src/planner/adjudication/terminal.rs",
            total_baseline: 124,
            production_baseline: 124,
            test_baseline: 0,
        },
        ChokepointBudget {
            path: "src/planner/final_acceptance_contract.rs",
            total_baseline: 187,
            production_baseline: 115,
            test_baseline: 72,
        },
        ChokepointBudget {
            path: "src/planner/ultra_plan_flow.rs",
            total_baseline: 1_570,
            production_baseline: 1_570,
            test_baseline: 0,
        },
        ChokepointBudget {
            path: "src/planner/fix_runtime.rs",
            total_baseline: 919,
            production_baseline: 650,
            test_baseline: 269,
        },
        ChokepointBudget {
            path: "src/planner/fix_contract_predicate.rs",
            total_baseline: 401,
            production_baseline: 156,
            test_baseline: 245,
        },
        ChokepointBudget {
            path: "src/planner/fix_reproducer.rs",
            total_baseline: 223,
            production_baseline: 145,
            test_baseline: 78,
        },
        ChokepointBudget {
            path: "src/planner/fix_diagnostics.rs",
            total_baseline: 377,
            production_baseline: 241,
            test_baseline: 136,
        },
        ChokepointBudget {
            path: "src/planner/assurance.rs",
            total_baseline: 63,
            production_baseline: 63,
            test_baseline: 0,
        },
        ChokepointBudget {
            path: "src/planner/profiles/nextjs.rs",
            total_baseline: 3_684,
            production_baseline: 2_361,
            test_baseline: 1_323,
        },
        ChokepointBudget {
            path: "src/planner/profiles/nextjs/domain.rs",
            total_baseline: 309,
            production_baseline: 294,
            test_baseline: 15,
        },
        ChokepointBudget {
            path: "src/planner/profiles/nextjs/repair_excerpts.rs",
            total_baseline: 60,
            production_baseline: 40,
            test_baseline: 20,
        },
        ChokepointBudget {
            path: "src/minimal_loop/evidence.rs",
            total_baseline: 6_702,
            production_baseline: 4_088,
            test_baseline: 2_694,
        },
        ChokepointBudget {
            path: "src/planner/capability_catalog.rs",
            total_baseline: 614,
            production_baseline: 407,
            test_baseline: 207,
        },
        ChokepointBudget {
            path: "src/planner/profile_manifest.rs",
            total_baseline: 713,
            production_baseline: 443,
            test_baseline: 270,
        },
        ChokepointBudget {
            path: "src/planner/profile_admission.rs",
            total_baseline: 98,
            production_baseline: 33,
            test_baseline: 65,
        },
        ChokepointBudget {
            path: "src/planner/profile_manifest/check_phase_scope.rs",
            total_baseline: 94,
            production_baseline: 62,
            test_baseline: 32,
        },
        ChokepointBudget {
            path: "src/planner/profile_manifest/schema_v1.rs",
            total_baseline: 268,
            production_baseline: 268,
            test_baseline: 0,
        },
        ChokepointBudget {
            path: "src/planner/profile_manifest/v1_validation.rs",
            total_baseline: 115,
            production_baseline: 115,
            test_baseline: 0,
        },
        ChokepointBudget {
            path: "src/planner/profiles/data/results_schema.rs",
            total_baseline: 187,
            production_baseline: 106,
            test_baseline: 81,
        },
        ChokepointBudget {
            path: "src/minimal_loop/pipeline_probe.rs",
            total_baseline: 455,
            production_baseline: 362,
            test_baseline: 93,
        },
        ChokepointBudget {
            path: "src/planner/capability_catalog/data.rs",
            total_baseline: 204,
            production_baseline: 141,
            test_baseline: 63,
        },
        ChokepointBudget {
            path: "src/planner/profiles/data/checks.rs",
            total_baseline: 452,
            production_baseline: 304,
            test_baseline: 148,
        },
        ChokepointBudget {
            path: "src/planner/profiles/data/phase_scope.rs",
            total_baseline: 110,
            production_baseline: 52,
            test_baseline: 58,
        },
        ChokepointBudget {
            path: "src/planner/profiles/data/pre_satisfied.rs",
            total_baseline: 107,
            production_baseline: 59,
            test_baseline: 48,
        },
        ChokepointBudget {
            path: "src/planner/profiles/data/inspection_schema.rs",
            total_baseline: 403,
            production_baseline: 328,
            test_baseline: 75,
        },
        ChokepointBudget {
            path: "src/planner/profiles/data/inspection_schema/input_table.rs",
            total_baseline: 190,
            production_baseline: 142,
            test_baseline: 48,
        },
        ChokepointBudget {
            path: "src/planner/profiles/data/inspection_schema/input_selection.rs",
            total_baseline: 237,
            production_baseline: 139,
            test_baseline: 98,
        },
        ChokepointBudget {
            path: "src/planner/profiles/data/internal_checks.rs",
            total_baseline: 28,
            production_baseline: 28,
            test_baseline: 0,
        },
        ChokepointBudget {
            path: "src/planner/profiles/data/runtime_checks.rs",
            total_baseline: 106,
            production_baseline: 94,
            test_baseline: 12,
        },
        ChokepointBudget {
            path: "src/planner/profiles/data/claims_binding.rs",
            total_baseline: 228,
            production_baseline: 174,
            test_baseline: 54,
        },
        ChokepointBudget {
            path: "src/planner/profiles/data/claims_binding/candidates.rs",
            total_baseline: 107,
            production_baseline: 60,
            test_baseline: 47,
        },
        ChokepointBudget {
            path: "src/planner/profiles/data/claims_binding/comparison.rs",
            total_baseline: 106,
            production_baseline: 39,
            test_baseline: 67,
        },
        ChokepointBudget {
            path: "src/planner/profiles/data/claims_binding/date_labels.rs",
            total_baseline: 107,
            production_baseline: 55,
            test_baseline: 52,
        },
        ChokepointBudget {
            path: "src/planner/profiles/data/claims_binding/matching.rs",
            total_baseline: 48,
            production_baseline: 48,
            test_baseline: 0,
        },
        ChokepointBudget {
            path: "src/planner/profiles/data/claims_binding/tests.rs",
            total_baseline: 64,
            production_baseline: 0,
            test_baseline: 64,
        },
        ChokepointBudget {
            path: "src/planner/profile_manifest/validation.rs",
            total_baseline: 148,
            production_baseline: 148,
            test_baseline: 0,
        },
        ChokepointBudget {
            path: "src/planner/profiles/data/manifest.rs",
            total_baseline: 308,
            production_baseline: 192,
            test_baseline: 116,
        },
        ChokepointBudget {
            path: "src/planner/profiles/data/runtime.rs",
            total_baseline: 344,
            production_baseline: 248,
            test_baseline: 96,
        },
        ChokepointBudget {
            path: "src/planner/setup_step_policy.rs",
            total_baseline: 910,
            production_baseline: 417,
            test_baseline: 493,
        },
        ChokepointBudget {
            path: "src/planner/lint.rs",
            total_baseline: 2_144,
            production_baseline: 1_097,
            test_baseline: 1_047,
        },
        ChokepointBudget {
            path: "src/planner/verify.rs",
            total_baseline: 4_757,
            production_baseline: 3_029,
            test_baseline: 1_728,
        },
        ChokepointBudget {
            path: "src/planner/verify/shell_control.rs",
            total_baseline: 161,
            production_baseline: 100,
            test_baseline: 61,
        },
        ChokepointBudget {
            path: "src/planner/verify/shell_rewrite.rs",
            total_baseline: 221,
            production_baseline: 113,
            test_baseline: 108,
        },
        ChokepointBudget {
            path: "src/planner/profiles/data.rs",
            total_baseline: 193,
            production_baseline: 104,
            test_baseline: 89,
        },
        ChokepointBudget {
            path: "src/planner/profiles/data/step_policy.rs",
            total_baseline: 634,
            production_baseline: 470,
            test_baseline: 164,
        },
        ChokepointBudget {
            path: "src/planner/profiles/data/step_policy/contract_assertion.rs",
            total_baseline: 190,
            production_baseline: 69,
            test_baseline: 121,
        },
        ChokepointBudget {
            path: "src/planner/profiles/data/step_policy/phase_filter.rs",
            total_baseline: 116,
            production_baseline: 74,
            test_baseline: 42,
        },
        ChokepointBudget {
            path: "src/planner/profiles/data/step_policy/verify_default.rs",
            total_baseline: 261,
            production_baseline: 101,
            test_baseline: 160,
        },
        ChokepointBudget {
            path: "src/planner/repair.rs",
            total_baseline: 1_063,
            production_baseline: 775,
            test_baseline: 288,
        },
        ChokepointBudget {
            path: "src/planner/profiles/data/repair_policy.rs",
            total_baseline: 255,
            production_baseline: 152,
            test_baseline: 103,
        },
        ChokepointBudget {
            path: "src/planner/profiles/data/repair_policy/claims_binding_guidance.rs",
            total_baseline: 141,
            production_baseline: 87,
            test_baseline: 54,
        },
        ChokepointBudget {
            path: "src/planner/profiles/data/repair_policy/inspection_guidance.rs",
            total_baseline: 12,
            production_baseline: 12,
            test_baseline: 0,
        },
        ChokepointBudget {
            path: "src/completion_metadata.rs",
            total_baseline: 52,
            production_baseline: 52,
            test_baseline: 0,
        },
        ChokepointBudget {
            path: "src/completion_metadata/data.rs",
            total_baseline: 153,
            production_baseline: 37,
            test_baseline: 116,
        },
        ChokepointBudget {
            path: "src/minimal_loop/tool_feedback.rs",
            total_baseline: 66,
            production_baseline: 48,
            test_baseline: 18,
        },
    ] {
        let text = std::fs::read_to_string(budget.path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", budget.path));
        let counts = code_line_counts(&text);
        assert_line_budget(
            budget.path,
            "production code",
            counts.production,
            budget.production_baseline,
        );
        assert_line_budget(budget.path, "test code", counts.test, budget.test_baseline);
        let allowed = allowed_line_count(budget.total_baseline);
        assert!(
            counts.total() <= allowed,
            "{} grew to {} total lines (production {}, test {}); total baseline is {}, allowed max is {}. Move new subsystems to new modules or land a shrinking refactor first.",
            budget.path,
            counts.total(),
            counts.production,
            counts.test,
            budget.total_baseline,
            allowed,
        );
    }
}

#[test]
fn code_line_counts_separate_cfg_test_blocks_and_items() {
    let source = "\
fn production_before() {}
#[cfg(test)]
use super::*;
fn production_between() {}
#[cfg(test)]
fn test_helper() {
    assert!(true);
}
fn production_after_helper() {}
#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {}
}
";

    let counts = code_line_counts(source);

    assert_eq!(
        counts,
        CodeLineCounts {
            production: 3,
            test: 11,
        }
    );
    assert_eq!(counts.total(), source.lines().count());
    assert_eq!(
        production_lines(source),
        vec![
            "fn production_before() {}".to_string(),
            "fn production_between() {}".to_string(),
            "fn production_after_helper() {}".to_string(),
        ]
    );
}

fn assert_line_budget(path: &str, kind: &str, current: usize, baseline: usize) {
    let allowed = allowed_line_count(baseline);
    assert!(
        current <= allowed,
        "{path} {kind} grew to {current} lines; {kind} baseline is {baseline}, allowed max is {allowed}. Move new subsystems to new modules or land a shrinking refactor first.",
    );
}

fn allowed_line_count(baseline: usize) -> usize {
    baseline + ((baseline * 2).div_ceil(100))
}

fn nextjs_literal_counts_outside_profiles() -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for path in rust_source_files(Path::new("src")) {
        let rel = path.to_string_lossy().replace('\\', "/");
        if rel.starts_with("src/planner/profiles/") {
            continue;
        }
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
        let count = production_lines(&text)
            .into_iter()
            .filter(|line| line.contains("\"nextjs\""))
            .count();
        if count > 0 {
            counts.insert(rel, count);
        }
    }
    counts
}

fn rust_source_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    collect_rust_source_files(root, &mut out);
    out.sort();
    out
}

fn collect_rust_source_files(root: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(root)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", root.display()))
        .flatten()
    {
        let path = entry.path();
        if path.is_dir() {
            collect_rust_source_files(&path, out);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

fn toml_value_contains(value: &toml::Value, token: &str) -> bool {
    match value {
        toml::Value::String(text) => text.to_lowercase().contains(token),
        toml::Value::Array(values) => values.iter().any(|value| toml_value_contains(value, token)),
        toml::Value::Table(table) => table.iter().any(|(key, value)| {
            key.to_lowercase().contains(token) || toml_value_contains(value, token)
        }),
        _ => false,
    }
}

fn code_line_counts(text: &str) -> CodeLineCounts {
    classified_rust_lines(text).into_iter().fold(
        CodeLineCounts {
            production: 0,
            test: 0,
        },
        |mut counts, (_, class)| {
            match class {
                RustLineClass::Production => counts.production += 1,
                RustLineClass::Test => counts.test += 1,
            }
            counts
        },
    )
}

fn production_lines(text: &str) -> Vec<String> {
    classified_rust_lines(text)
        .into_iter()
        .filter(|(_, class)| *class == RustLineClass::Production)
        .map(|(line, _)| line.to_string())
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RustLineClass {
    Production,
    Test,
}

fn classified_rust_lines(text: &str) -> Vec<(&str, RustLineClass)> {
    let mut out = Vec::new();
    let mut skip_next_cfg_test_item = false;
    let mut skipping_cfg_test_block = false;
    let mut test_mod_to_eof = false;
    let mut block_depth = 0i32;

    for line in text.lines() {
        let trimmed = line.trim();
        if test_mod_to_eof {
            out.push((line, RustLineClass::Test));
            continue;
        }
        if skipping_cfg_test_block {
            out.push((line, RustLineClass::Test));
            block_depth += brace_delta(line);
            if block_depth <= 0 {
                skipping_cfg_test_block = false;
                block_depth = 0;
            }
            continue;
        }
        if skip_next_cfg_test_item {
            if trimmed.starts_with("#[") {
                out.push((line, RustLineClass::Test));
                continue;
            }
            if trimmed.starts_with("mod tests") {
                out.push((line, RustLineClass::Test));
                test_mod_to_eof = true;
                skip_next_cfg_test_item = false;
                continue;
            }
            out.push((line, RustLineClass::Test));
            let delta = brace_delta(line);
            if delta > 0 {
                skipping_cfg_test_block = true;
                block_depth = delta;
                if block_depth <= 0 {
                    skipping_cfg_test_block = false;
                }
            }
            skip_next_cfg_test_item = false;
            continue;
        }
        if trimmed == "#[cfg(test)]" {
            out.push((line, RustLineClass::Test));
            skip_next_cfg_test_item = true;
            continue;
        }
        out.push((line, RustLineClass::Production));
    }
    out
}

fn brace_delta(line: &str) -> i32 {
    line.chars().fold(0, |depth, ch| match ch {
        '{' => depth + 1,
        '}' => depth - 1,
        _ => depth,
    })
}
