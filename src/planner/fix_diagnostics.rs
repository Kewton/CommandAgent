use std::path::Path;

use crate::config::Config;
use crate::planner::adjudication::contract::{EvidenceStage, ExpectedOutcome};
use crate::planner::adjudication::fix::{FixEvidenceObservation, ProbeOutcome};
use crate::planner::repair_targeting::{RepairTargetSelection, RepairTargetSelectionReason};
use crate::planner::step_plan::{StepKind, StepPlan};
use crate::planner::ultra_plan::UltraPhase;
use crate::tools::bash::BashOutcome;

mod prompt_guidance;
mod reproducer_execution;
use prompt_guidance::{diagnostic_phase, render_guidance};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FixFailureDiagnostic {
    pub(crate) target_path: String,
    pub(crate) line: usize,
    pub(crate) column: usize,
    pub(crate) error_kind: String,
    pub(crate) message: String,
    pub(crate) excerpt: String,
    pub(crate) selection_reason: RepairTargetSelectionReason,
}

pub(crate) struct ReproducerRun {
    pub(crate) evidence: FixEvidenceObservation,
    pub(crate) diagnostic: Option<FixFailureDiagnostic>,
    pub(crate) reproducer_defect: Option<String>,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn run_reproducer(
    config: &Config,
    run_id: &str,
    requirement_id: &str,
    stage: EvidenceStage,
    expected: ExpectedOutcome,
    epoch: u64,
    command: &str,
    lineage: &str,
    profile: &str,
    goal: &str,
) -> ReproducerRun {
    let mut diagnostic = None;
    let execution = reproducer_execution::run(config, command, profile, goal);
    let assessment = crate::planner::fix_reproducer_defect::classify(
        command,
        execution.outcome,
        execution.shell_observation.as_ref(),
    );
    if stage == EvidenceStage::Before
        && execution.outcome == ProbeOutcome::Failure
        && assessment.classification.is_subject()
        && let Some(observation) = execution.shell_observation.as_ref()
    {
        diagnostic = extract_failure_diagnostic(
            &config.workspace_root,
            command,
            observation,
            config.eval_events_path.as_deref(),
        );
    }
    let mut evidence = FixEvidenceObservation::new(
        requirement_id,
        command,
        stage,
        expected,
        lineage,
        epoch,
        run_id,
        execution.outcome,
        &execution.reason,
    );
    evidence.failure_classification = assessment.classification;
    ReproducerRun {
        evidence,
        diagnostic,
        reproducer_defect: assessment.error_kind,
    }
}

pub(crate) fn extract_failure_diagnostic(
    root: &Path,
    command: &str,
    outcome: &BashOutcome,
    eval_events_path: Option<&Path>,
) -> Option<FixFailureDiagnostic> {
    let combined = format!("{}\n{}", outcome.stderr, outcome.stdout);
    let output = crate::minimal_loop::build_verifier::FullCommandOutput::from_bounded_executor(
        root, command, &combined,
    );
    if let Some(error) = crate::minimal_loop::build_verifier::parse_compile_errors(&output)
        .into_iter()
        .next()
    {
        let target_path = error.path.trim_start_matches("./").to_string();
        crate::tools::path_guard::validate_workspace_relative(&target_path).ok()?;
        let error_kind = error
            .message
            .split_once(':')
            .map(|(kind, _)| kind.trim())
            .filter(|kind| kind.to_ascii_lowercase().contains("error"))
            .unwrap_or("compile_error")
            .to_string();
        return Some(FixFailureDiagnostic {
            target_path,
            line: error.line,
            column: error.column,
            error_kind,
            message: error.message,
            excerpt: crate::eval_events::body_snippet(&error.excerpt),
            selection_reason: RepairTargetSelectionReason::DiagnosisMapped,
        });
    }
    let traceback = crate::minimal_loop::python_traceback::extract_failed_command(
        command,
        &outcome.stderr,
        root,
        eval_events_path,
    )?;
    Some(FixFailureDiagnostic {
        target_path: traceback.target_path?,
        line: traceback.final_frame.line,
        column: 0,
        error_kind: traceback.exception_type,
        message: traceback.message,
        excerpt: format!(
            "{}:{} in {}",
            traceback.final_frame.file, traceback.final_frame.line, traceback.final_frame.function
        ),
        selection_reason: RepairTargetSelectionReason::TracebackMapped,
    })
}

pub(crate) fn attach_to_phase_prompt(
    phase: &UltraPhase,
    diagnostic: Option<&FixFailureDiagnostic>,
    mut prompt: String,
) -> String {
    if diagnostic_phase(phase)
        && let Some(diagnostic) = diagnostic
    {
        prompt.push_str("\n\n");
        prompt.push_str(&render_guidance(diagnostic));
    }
    prompt
}

pub(crate) fn bind_step_plan(
    phase: &UltraPhase,
    diagnostic: Option<&FixFailureDiagnostic>,
    plan: &mut StepPlan,
) {
    let Some(diagnostic) = diagnostic.filter(|_| diagnostic_phase(phase)) else {
        return;
    };
    let guidance = render_guidance(diagnostic);
    for step in &mut plan.steps {
        if step.step_kind() != StepKind::Implement {
            continue;
        }
        if !step.instruction.contains("Fix F1 failure diagnostic") {
            step.instruction.push_str("\n\n");
            step.instruction.push_str(&guidance);
        }
        if phase.id == "repair"
            && step.step_kind() == StepKind::Implement
            && !step.expected_paths.contains(&diagnostic.target_path)
        {
            step.expected_paths.push(diagnostic.target_path.clone());
        }
    }
}

pub(crate) fn repair_target_from_prompt(prompt: &str) -> Option<RepairTargetSelection> {
    let value = prompt
        .lines()
        .find_map(|line| line.trim().strip_prefix("- write-pressure target: "))?;
    let (path, reason) = value.split_once(" (selection_reason=")?;
    let reason = reason.strip_suffix(')')?;
    crate::tools::path_guard::validate_workspace_relative(path).ok()?;
    let selection_reason = match reason {
        "contract_attribute" => RepairTargetSelectionReason::ContractAttribute,
        "diagnosis_mapped" => RepairTargetSelectionReason::DiagnosisMapped,
        "traceback_mapped" => RepairTargetSelectionReason::TracebackMapped,
        _ => return None,
    };
    Some(RepairTargetSelection {
        selected_targets: vec![path.to_string()],
        selection_reason,
    })
}

pub(crate) fn prompt_has_diagnostic(prompt: &str) -> bool {
    repair_target_from_prompt(prompt).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::step_plan::PlanStep;
    use crate::tools::bash::BashOutcomeKind;

    fn failed(stderr: &str) -> BashOutcome {
        BashOutcome {
            kind: BashOutcomeKind::CommandFailed,
            status: Some("exit status: 1".to_string()),
            stdout: String::new(),
            stderr: stderr.to_string(),
            elapsed_ms: 1,
            summary: "command failed".to_string(),
        }
    }

    #[test]
    fn run1_compile_failure_maps_init_game_to_page() {
        let root = tempfile::tempdir().unwrap();
        let outcome = failed(
            "Failed to compile.\n\n./src/app/page.tsx:250:5\nType error: Cannot find name 'initGame'.\n\n  248 |\n  249 | const startGame = () => {\n> 250 |     initGame();\n      |     ^\n",
        );

        let diagnostic = extract_failure_diagnostic(root.path(), "npm run build", &outcome, None)
            .expect("compile diagnostic");

        assert_eq!(diagnostic.target_path, "src/app/page.tsx");
        assert_eq!((diagnostic.line, diagnostic.column), (250, 5));
        assert_eq!(diagnostic.error_kind, "Type error");
        assert!(diagnostic.message.contains("Cannot find name 'initGame'"));
        assert_eq!(
            diagnostic.selection_reason,
            RepairTargetSelectionReason::DiagnosisMapped
        );
        let phase = UltraPhase {
            id: "isolate-cause".to_string(),
            prompt: "Narrow the cause.".to_string(),
        };
        let prompt = attach_to_phase_prompt(&phase, Some(&diagnostic), "phase".to_string());
        assert!(prompt.contains("src/app/page.tsx:250:5"));
        let selection = repair_target_from_prompt(&prompt).unwrap();
        assert_eq!(selection.selected_targets, ["src/app/page.tsx"]);
        assert_eq!(
            selection.selection_reason,
            RepairTargetSelectionReason::DiagnosisMapped
        );
    }

    #[test]
    fn python_failure_reuses_traceback_target_mapping() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("pipeline")).unwrap();
        std::fs::write(
            root.path().join("pipeline/main.py"),
            "raise ValueError('bad')\n",
        )
        .unwrap();
        let outcome = failed(
            "Traceback (most recent call last):\n  File \"pipeline/main.py\", line 7, in <module>\n    run()\nValueError: bad input\n",
        );

        let diagnostic =
            extract_failure_diagnostic(root.path(), "python pipeline/main.py", &outcome, None)
                .expect("traceback diagnostic");

        assert_eq!(diagnostic.target_path, "pipeline/main.py");
        assert_eq!(diagnostic.line, 7);
        assert_eq!(diagnostic.error_kind, "ValueError");
        assert_eq!(
            diagnostic.selection_reason,
            RepairTargetSelectionReason::TracebackMapped
        );
    }

    #[test]
    fn phase_two_prompt_carries_compile_location_and_error_kind() {
        let diagnostic = FixFailureDiagnostic {
            target_path: "src/app/page.tsx".to_string(),
            line: 250,
            column: 5,
            error_kind: "Type error".to_string(),
            message: "Cannot find name 'initGame'.".to_string(),
            excerpt: "> 250 | initGame();".to_string(),
            selection_reason: RepairTargetSelectionReason::DiagnosisMapped,
        };
        let phase = UltraPhase {
            id: "isolate-cause".to_string(),
            prompt: "Narrow the cause.".to_string(),
        };

        let prompt = attach_to_phase_prompt(&phase, Some(&diagnostic), "base".to_string());

        assert!(prompt.contains("src/app/page.tsx:250:5"));
        assert!(prompt.contains("error kind: Type error"));
        assert!(prompt.contains("Cannot find name 'initGame'"));
    }

    #[test]
    fn repair_plan_and_write_pressure_use_diagnosis_target() {
        let diagnostic = FixFailureDiagnostic {
            target_path: "src/app/page.tsx".to_string(),
            line: 250,
            column: 5,
            error_kind: "Type error".to_string(),
            message: "Cannot find name 'initGame'.".to_string(),
            excerpt: String::new(),
            selection_reason: RepairTargetSelectionReason::DiagnosisMapped,
        };
        let phase = UltraPhase {
            id: "repair".to_string(),
            prompt: "Repair the defect.".to_string(),
        };
        let mut plan = StepPlan {
            goal: "repair".to_string(),
            steps: vec![PlanStep {
                id: "repair".to_string(),
                kind: "implement".to_string(),
                expected_result: "pass".to_string(),
                instruction: "Apply the repair.".to_string(),
                expected_paths: Vec::new(),
                verify: Vec::new(),
            }],
        };

        bind_step_plan(&phase, Some(&diagnostic), &mut plan);

        assert_eq!(plan.steps[0].expected_paths, ["src/app/page.tsx"]);
        let selection = repair_target_from_prompt(&plan.steps[0].instruction).unwrap();
        assert_eq!(selection.selected_targets, ["src/app/page.tsx"]);
        assert_eq!(
            selection.selection_reason,
            RepairTargetSelectionReason::DiagnosisMapped
        );
    }
}
