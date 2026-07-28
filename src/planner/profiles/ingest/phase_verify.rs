use std::path::Path;

use serde_json::json;

use crate::eval_events;
use crate::planner::profiles::ingest::accounting::CandidateSelector;
use crate::planner::step_plan::{PlanStep, StepKind, StepPlan};
use crate::planner::verify::VerificationReport;

pub(crate) const CHECK_COMMAND: &str = "anvil-ingest-check:phase_structure";
const VERIFY_INSTRUCTION: &str = "Verify the ingest phase structure: pipeline/main.py exists, \
output/records.json is JSON, output/inspection.json declares candidate_selector as kind/value, \
and output/report.md exists. Allowed candidate_selector kinds are css, html_tag, and line_prefix; \
the literal shape is {\"candidate_selector\": {\"kind\": \"css\", \"value\": \
\"ul.events > li\"}}. Values are examples only and must be replaced with actual snapshot \
observations.";

pub(crate) fn structure_check_step() -> PlanStep {
    PlanStep {
        id: "verify-ingest-phase-structure".to_string(),
        kind: "verify".to_string(),
        expected_result: "pass".to_string(),
        instruction: VERIFY_INSTRUCTION.to_string(),
        expected_paths: Vec::new(),
        verify: vec![CHECK_COMMAND.to_string()],
    }
}

pub(crate) fn canonicalize_step_plan(
    plan: &mut StepPlan,
    profile: &str,
    create_intent: bool,
    terminal_plan: bool,
    eval_events_path: Option<&Path>,
) -> usize {
    if !create_intent || profile.trim() != "ingest" {
        return 0;
    }
    if !terminal_plan {
        return remove_intermediate_model_verifiers(plan, eval_events_path);
    }
    let mut changes = plan
        .steps
        .iter_mut()
        .map(|step| canonicalize_step(step, eval_events_path))
        .sum();
    if !plan.steps.iter().any(has_structure_check) {
        let step = structure_check_step();
        emit_canonicalized(
            eval_events_path,
            &step.id,
            "missing phase verify",
            CHECK_COMMAND,
        );
        plan.steps.push(step);
        changes += 1;
    }
    changes
}

fn remove_intermediate_model_verifiers(
    plan: &mut StepPlan,
    eval_events_path: Option<&Path>,
) -> usize {
    let mut changes = 0;
    plan.steps.retain_mut(|step| {
        let verifier_artifact = step
            .expected_paths
            .iter()
            .any(|path| is_model_verifier_path(path));
        let invokes_verifier = step
            .verify
            .iter()
            .any(|command| command_invokes_model_verifier(command));
        let phase_verify = step.step_kind() == StepKind::Verify;
        if phase_verify
            || (verifier_artifact
                && step
                    .expected_paths
                    .iter()
                    .all(|path| is_model_verifier_path(path)))
        {
            emit_canonicalized(
                eval_events_path,
                &step.id,
                &format!(
                    "kind={}; expected_paths={}; verify={}",
                    step.kind,
                    step.expected_paths.join(","),
                    step.verify.join("\n")
                ),
                "deferred to terminal ingest phase structure gate",
            );
            changes += 1;
            return false;
        }
        if verifier_artifact || invokes_verifier || !step.verify.is_empty() {
            let original = format!(
                "expected_paths={}; verify={}",
                step.expected_paths.join(","),
                step.verify.join("\n")
            );
            step.expected_paths
                .retain(|path| !is_model_verifier_path(path));
            step.verify.clear();
            emit_canonicalized(
                eval_events_path,
                &step.id,
                &original,
                "deferred to terminal ingest phase structure gate",
            );
            changes += 1;
        }
        true
    });
    changes
}

pub(crate) fn is_check_command(command: &str) -> bool {
    command.trim() == CHECK_COMMAND
}

pub(crate) fn run_step_check(
    root: &Path,
    profile: Option<&str>,
    step: &PlanStep,
    report: &mut VerificationReport,
) {
    if !step.verify.iter().any(|command| is_check_command(command)) {
        return;
    }
    if profile != Some("ingest") {
        report.push_command_failure(
            CHECK_COMMAND,
            "ingest phase structure check is invalid outside the active ingest profile",
        );
        return;
    }
    for reason in verify_structure(root) {
        report.push_profile_failure(reason);
    }
}

fn canonicalize_step(step: &mut PlanStep, eval_events_path: Option<&Path>) -> usize {
    let verifier_artifact = step
        .expected_paths
        .iter()
        .any(|path| is_model_verifier_path(path));
    let invokes_verifier = step
        .verify
        .iter()
        .any(|command| command_invokes_model_verifier(command));
    let phase_verify = step.step_kind() == StepKind::Verify;
    if !verifier_artifact && !invokes_verifier && !phase_verify {
        if step.verify.is_empty() {
            return 0;
        }
        let original = step.verify.join("\n");
        step.verify.clear();
        emit_canonicalized(
            eval_events_path,
            &step.id,
            &original,
            "deferred to final ingest phase structure gate",
        );
        return 1;
    }

    let original = format!(
        "kind={}; expected_paths={}; verify={}",
        step.kind,
        step.expected_paths.join(","),
        step.verify.join("\n")
    );
    step.kind = "verify".to_string();
    step.expected_result = "pass".to_string();
    step.instruction = VERIFY_INSTRUCTION.to_string();
    step.expected_paths
        .retain(|path| !is_model_verifier_path(path));
    step.verify = vec![CHECK_COMMAND.to_string()];
    emit_canonicalized(eval_events_path, &step.id, &original, CHECK_COMMAND);
    1
}

// This phase gate is intentionally structural and language-neutral. N1-N5
// acceptance owns execution, source binding, accounting, declared schema, and
// rerun equality; none of those semantic checks may be duplicated here.
fn verify_structure(root: &Path) -> Vec<String> {
    let mut failures = Vec::new();
    if !root.join("pipeline/main.py").is_file() {
        failures.push("ingest_phase_structure:pipeline_missing".to_string());
    }

    match std::fs::read_to_string(root.join("output/records.json")) {
        Ok(raw) if serde_json::from_str::<serde_json::Value>(&raw).is_ok() => {}
        Ok(_) => failures.push("ingest_phase_structure:records_invalid_json".to_string()),
        Err(_) => failures.push("ingest_phase_structure:records_missing".to_string()),
    }

    match std::fs::read_to_string(root.join("output/inspection.json"))
        .ok()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
        .and_then(|document| document.get("candidate_selector").cloned())
        .and_then(|selector| serde_json::from_value::<CandidateSelector>(selector).ok())
    {
        Some(selector) if !selector.value.trim().is_empty() => {}
        _ => failures.push("ingest_phase_structure:selector_not_kind_value".to_string()),
    }

    if !root.join("output/report.md").is_file() {
        failures.push("ingest_phase_structure:report_missing".to_string());
    }
    failures
}

fn is_model_verifier_path(path: &str) -> bool {
    let normalized = path.replace('\\', "/").to_ascii_lowercase();
    let name = normalized.rsplit('/').next().unwrap_or_default();
    name.ends_with(".py")
        && (name.starts_with("verify_")
            || name.starts_with("verify-")
            || name.starts_with("smoke_")
            || name.starts_with("smoke-"))
}

fn command_invokes_model_verifier(command: &str) -> bool {
    command
        .split_whitespace()
        .map(|word| word.trim_matches(['\'', '"']))
        .any(is_model_verifier_path)
}

fn has_structure_check(step: &PlanStep) -> bool {
    step.verify.iter().any(|command| is_check_command(command))
}

fn emit_canonicalized(
    eval_events_path: Option<&Path>,
    step_id: &str,
    original: &str,
    replacement: &str,
) {
    eval_events::emit(
        eval_events_path,
        json!({
            "event": "verify_canonicalized",
            "step_id": step_id,
            "field": "ingest_phase_verify",
            "original": original,
            "replacement": replacement,
            "disposition": "canonical",
        }),
    );
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    const MEASURED_PLAN: &str = include_str!(
        "../../../../tests/fixtures/ingest-phase-structure/table_qwen35_002-plan.yaml"
    );
    const MEASURED_ELEV_002_INSPECTION: &str = include_str!(
        "../../../../tests/fixtures/ingest-phase-structure/elev-002-list-cloud-001-inspection.json"
    );
    const GUIDED_CANONICAL_INSPECTION: &str = include_str!(
        "../../../../tests/fixtures/ingest-phase-structure/guided-canonical-inspection.json"
    );
    const GUIDED_PLAN: &str = include_str!(
        "../../../../tests/fixtures/ingest-phase-structure/canonical-guidance-plan.yaml"
    );

    fn write_file(root: &Path, path: &str, content: &str) {
        let target = root.join(path);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(target, content).unwrap();
    }

    fn check_step() -> PlanStep {
        structure_check_step()
    }

    #[test]
    fn measured_self_verifier_plan_is_rebound_to_machine_structure_checks() {
        let mut plan: StepPlan = serde_yaml::from_str(MEASURED_PLAN).unwrap();
        assert_eq!(
            canonicalize_step_plan(&mut plan, "ingest", true, true, None),
            2
        );
        assert!(plan.steps.iter().all(|step| {
            !step
                .expected_paths
                .iter()
                .any(|path| is_model_verifier_path(path))
        }));
        assert!(plan.steps.iter().all(|step| {
            !step
                .verify
                .iter()
                .any(|command| command_invokes_model_verifier(command))
        }));
        assert_eq!(
            plan.steps
                .iter()
                .filter(|step| step.verify == [CHECK_COMMAND])
                .count(),
            2
        );
    }

    #[test]
    fn measured_inline_model_verify_is_removed_from_implement_steps() {
        let mut plan = StepPlan {
            goal: "Ingest snapshots".to_string(),
            steps: vec![PlanStep {
                id: "create-inspection-manifest".to_string(),
                kind: "implement".to_string(),
                expected_result: "pass".to_string(),
                instruction: "Create output/inspection.json.".to_string(),
                expected_paths: vec!["output/inspection.json".to_string()],
                verify: vec![
                    "python3 -c \"import json\\nd=json.load(open('output/inspection.json'))\\nassert 'candidate_selector' in d\""
                        .to_string(),
                ],
            }],
        };

        assert_eq!(
            canonicalize_step_plan(&mut plan, "ingest", true, true, None),
            2
        );
        assert!(plan.steps[0].verify.is_empty());
        assert_eq!(plan.steps[1].verify, [CHECK_COMMAND]);
        assert_eq!(plan.steps[1].step_kind(), StepKind::Verify);
    }

    #[test]
    fn canonical_structure_reaches_acceptance_without_content_assertions() {
        let dir = tempdir().unwrap();
        write_file(dir.path(), "pipeline/main.py", "raise SystemExit(99)\n");
        write_file(dir.path(), "output/records.json", "[]\n");
        write_file(
            dir.path(),
            "output/inspection.json",
            r#"{"candidate_selector":{"kind":"html_tag","value":"tr"}}"#,
        );
        write_file(dir.path(), "output/report.md", "");

        let mut report = VerificationReport::pass();
        run_step_check(dir.path(), Some("ingest"), &check_step(), &mut report);
        assert!(report.is_pass());
    }

    #[test]
    fn measured_string_selector_becomes_gate_positive_with_literal_guidance_shape() {
        let dir = tempdir().unwrap();
        write_file(dir.path(), "pipeline/main.py", "raise SystemExit(99)\n");
        write_file(dir.path(), "output/records.json", "[]\n");
        write_file(
            dir.path(),
            "output/inspection.json",
            MEASURED_ELEV_002_INSPECTION,
        );
        write_file(dir.path(), "output/report.md", "");

        assert_eq!(
            verify_structure(dir.path()),
            ["ingest_phase_structure:selector_not_kind_value"]
        );

        write_file(
            dir.path(),
            "output/inspection.json",
            GUIDED_CANONICAL_INSPECTION,
        );

        assert!(verify_structure(dir.path()).is_empty());
    }

    #[test]
    fn structure_gate_requirements_all_have_prior_literal_guidance() {
        let guidance = crate::planner::profiles::ingest::guidance::GENERATION_RULES;
        for (failure, required_guidance) in [
            ("pipeline_missing", "pipeline/main.py"),
            ("records_missing", "output/records.json"),
            ("records_invalid_json", "valid JSON"),
            (
                "selector_not_kind_value",
                crate::planner::profiles::ingest::guidance::SELECTOR_LITERAL,
            ),
            ("report_missing", "output/report.md"),
        ] {
            assert!(
                guidance.contains(required_guidance),
                "{failure} lacks prior guidance: {required_guidance}"
            );
        }
        let plan: StepPlan = serde_yaml::from_str(GUIDED_PLAN).unwrap();
        for marker in [
            "css, html_tag, and line_prefix",
            crate::planner::profiles::ingest::guidance::SELECTOR_LITERAL,
            "output/inspection.json",
            "output/records.json",
            "examples only",
            "actual snapshots",
        ] {
            assert!(plan.goal.contains(marker), "snapshot lacks {marker}");
            assert!(guidance.contains(marker), "runtime guidance lacks {marker}");
        }
        assert!(
            VERIFY_INSTRUCTION
                .contains(crate::planner::profiles::ingest::guidance::SELECTOR_LITERAL)
        );
    }

    #[test]
    fn missing_artifacts_invalid_json_and_noncanonical_selector_fail() {
        let dir = tempdir().unwrap();
        write_file(dir.path(), "output/records.json", "{");
        write_file(
            dir.path(),
            "output/inspection.json",
            r#"{"candidate_selector":"table tbody tr"}"#,
        );
        assert_eq!(
            verify_structure(dir.path()),
            [
                "ingest_phase_structure:pipeline_missing",
                "ingest_phase_structure:records_invalid_json",
                "ingest_phase_structure:selector_not_kind_value",
                "ingest_phase_structure:report_missing",
            ]
        );
    }

    #[test]
    fn other_profiles_and_intents_remain_unchanged() {
        let original: StepPlan = serde_yaml::from_str(MEASURED_PLAN).unwrap();
        for (profile, create) in [("data", true), ("ingest", false), ("cli", true)] {
            let mut candidate = original.clone();
            assert_eq!(
                canonicalize_step_plan(&mut candidate, profile, create, true, None),
                0
            );
            assert_eq!(candidate, original);
        }
    }

    #[test]
    fn intermediate_ultra_phase_defers_full_structure_gate() {
        let mut plan = StepPlan {
            goal: "Analyze snapshots before later implementation phases".to_string(),
            steps: vec![
                PlanStep {
                    id: "implement-analysis-script".to_string(),
                    kind: "implement".to_string(),
                    expected_result: "pass".to_string(),
                    instruction: "Create inspection metadata.".to_string(),
                    expected_paths: vec![
                        "scripts/analyze_snapshots.js".to_string(),
                        "output/inspection.json".to_string(),
                    ],
                    verify: Vec::new(),
                },
                PlanStep {
                    id: "verify-output-json".to_string(),
                    kind: "verify".to_string(),
                    expected_result: "pass".to_string(),
                    instruction: "Verify the analysis output.".to_string(),
                    expected_paths: Vec::new(),
                    verify: vec![
                        "node -e \"JSON.parse(require('fs').readFileSync('output/inspection.json'))\""
                            .to_string(),
                    ],
                },
            ],
        };

        assert_eq!(
            canonicalize_step_plan(&mut plan, "ingest", true, false, None),
            1
        );
        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.steps[0].id, "implement-analysis-script");
        assert!(!plan.steps.iter().any(has_structure_check));
    }
}
