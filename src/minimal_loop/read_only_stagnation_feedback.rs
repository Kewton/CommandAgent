use std::path::Path;

use serde_json::json;

use crate::eval_events;

use super::edit_anchor_recovery::EditAnchorFailureSummary;
use super::loop_run::{RunSessionErrorContext, RunSessionOptions, RunSessionStepKind};
use super::stagnation_escalation::{
    ReadOnlyStagnationStage, WRITE_REQUIRED_NO_WRITE_LIMIT, WriteRequiredState,
};

#[allow(clippy::too_many_arguments, reason = "borrows disjoint loop state")]
pub(crate) fn maybe_read_only_stagnation_feedback(
    eval_events_path: Option<&Path>,
    root: &Path,
    profile: &str,
    user_prompt: &str,
    read_only_streak: usize,
    no_progress_streak: usize,
    options: &RunSessionOptions,
    write_required_state: &mut WriteRequiredState,
    pending_error_context: &RunSessionErrorContext,
    repair_changed_paths: &[String],
    required_paths: &[String],
    changed_paths: &[String],
    anchor_failure: Option<EditAnchorFailureSummary>,
) -> Option<String> {
    let mut pending_evidence = pending_error_context.missing_evidence.clone();
    if let Some(carryover) = options.escalation_carryover.as_ref() {
        for evidence in carryover.pending_evidence() {
            if !pending_evidence
                .iter()
                .any(|existing| existing == &evidence)
            {
                pending_evidence.push(evidence);
            }
        }
    }
    let decision = super::anchor_stagnation_interlock::stagnation_decision(
        read_only_streak,
        no_progress_streak,
        anchor_failure,
    )?;
    let stage = decision.stage;
    let selection = if stage == ReadOnlyStagnationStage::WriteRequired {
        let (selection, diagnostic_feedback) =
            if let Some(selection) = decision.write_required_selection() {
                (selection, decision.diagnostic_feedback())
            } else {
                let mapped_selection =
                    crate::planner::fix_diagnostics::repair_target_from_prompt(user_prompt);
                let mut fallback_candidates = changed_paths.to_vec();
                for path in &options.path_fallback_candidates {
                    if !fallback_candidates.iter().any(|existing| existing == path) {
                        fallback_candidates.push(path.clone());
                    }
                }
                let selection = crate::planner::repair_targeting::resolve_repair_targets(
                    crate::planner::repair_targeting::RepairTargetResolutionInput {
                        root,
                        profile,
                        pending_evidence: &pending_evidence,
                        missing_capabilities: &pending_error_context.missing_capabilities,
                        contract_attribute_paths: &[],
                        repair_changed_paths,
                        required_paths,
                        fallback_paths: &fallback_candidates,
                        mapped_selection: mapped_selection.as_ref(),
                        priority: options.repair_target_priority,
                    },
                )?
                .into();
                let state_binding_feedback = if mapped_selection.is_some() {
                    String::new()
                } else {
                    crate::planner::state_binding_scan::write_required_feedback(
                        root,
                        profile,
                        &pending_evidence,
                        &pending_error_context.missing_capabilities,
                        eval_events_path,
                    )
                };
                (selection, state_binding_feedback)
            };
        write_required_state.activate_with_feedback(selection.clone(), diagnostic_feedback);
        Some(selection)
    } else {
        None
    };
    let target_path = selection
        .as_ref()
        .and_then(|selection| selection.primary_target())
        .or_else(|| {
            decision
                .anchor_failure
                .as_ref()
                .map(|failure| failure.path.as_str())
        })
        .unwrap_or("");
    let selected_targets = selection
        .as_ref()
        .map(|selection| selection.selected_targets.clone())
        .or_else(|| {
            decision
                .anchor_failure
                .as_ref()
                .map(|failure| vec![failure.path.clone()])
        })
        .unwrap_or_default();
    let selection_reason = selection
        .as_ref()
        .map(|selection| selection.selection_reason.as_str())
        .or_else(|| decision.anchor_interlocked().then_some("anchor_failure"))
        .unwrap_or("");
    let objective = eval_events::body_snippet(user_prompt);
    super::anchor_stagnation_interlock::emit_interlock_event(
        eval_events_path,
        &decision,
        options.scope.as_str(),
        options
            .step_kind
            .map(RunSessionStepKind::as_str)
            .unwrap_or(""),
        options.phase_scope.as_deref(),
    );
    eval_events::emit(
        eval_events_path,
        json!({
            "event": "read_only_stagnation_feedback",
            "stage": stage.as_str(),
            "read_only_streak": read_only_streak,
            "objective": objective,
            "target_path": target_path,
            "selected_targets": selected_targets,
            "selection_reason": selection_reason,
            "pending_capability_evidence": pending_evidence,
            "session_scope": options.scope.as_str(),
            "step_kind": options.step_kind.map(RunSessionStepKind::as_str).unwrap_or(""),
            "phase_scope": options.phase_scope.as_deref().unwrap_or(""),
        }),
    );
    Some(match stage {
        ReadOnlyStagnationStage::Intervention if decision.anchor_interlocked() => decision
            .full_file_write_feedback(&objective)
            .unwrap_or_else(|| super::feedback::read_only_stagnation(&objective, read_only_streak)),
        ReadOnlyStagnationStage::Intervention => {
            super::feedback::read_only_stagnation(&objective, read_only_streak)
        }
        ReadOnlyStagnationStage::CompactRestatement if decision.anchor_interlocked() => decision
            .full_file_write_feedback(&objective)
            .unwrap_or_else(|| {
                super::feedback::read_only_stagnation_compact(&objective, read_only_streak)
            }),
        ReadOnlyStagnationStage::CompactRestatement => {
            super::feedback::read_only_stagnation_compact(&objective, read_only_streak)
        }
        ReadOnlyStagnationStage::WriteRequired if decision.anchor_interlocked() => decision
            .write_required_feedback(WRITE_REQUIRED_NO_WRITE_LIMIT)
            .unwrap_or_else(|| {
                super::stagnation_escalation::append_write_required_diagnostic(
                    super::stagnation_escalation::read_only_write_required_feedback(
                        write_required_state.selected_targets(),
                        read_only_streak,
                    ),
                    write_required_state.diagnostic_feedback(),
                )
            }),
        ReadOnlyStagnationStage::WriteRequired => {
            super::stagnation_escalation::append_write_required_diagnostic(
                super::stagnation_escalation::read_only_write_required_feedback(
                    write_required_state.selected_targets(),
                    read_only_streak,
                ),
                write_required_state.diagnostic_feedback(),
            )
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::minimal_loop::loop_run::RunSessionStepKind;
    use crate::minimal_loop::stagnation_escalation::WriteRequiredSelectionReason;

    #[test]
    fn fix_diagnostic_drives_write_pressure_with_diagnosis_reason() {
        let root = tempfile::tempdir().unwrap();
        let events = root.path().join("events.jsonl");
        let prompt = "Repair the F1 failure.\n\
- write-pressure target: src/app/page.tsx (selection_reason=diagnosis_mapped)";
        let options = RunSessionOptions::plan_step(RunSessionStepKind::Implement)
            .with_repair_target_priority(
                crate::planner::repair_targeting::RepairTargetPriority::FixIntent,
            );
        let mut state = WriteRequiredState::default();

        let feedback = maybe_read_only_stagnation_feedback(
            Some(&events),
            root.path(),
            "nextjs",
            prompt,
            7,
            7,
            &options,
            &mut state,
            &RunSessionErrorContext::default(),
            &[],
            &[],
            &[],
            None,
        )
        .expect("write pressure feedback");

        assert_eq!(state.selected_targets(), ["src/app/page.tsx"]);
        assert_eq!(
            state.selection_reason(),
            Some(WriteRequiredSelectionReason::DiagnosisMapped)
        );
        assert!(feedback.contains("src/app/page.tsx"));
        let event = std::fs::read_to_string(events).unwrap();
        assert!(event.contains(r#""selection_reason":"diagnosis_mapped""#));
    }

    #[test]
    fn fix_contract_predicate_preempts_package_required_path() {
        let root = tempfile::tempdir().unwrap();
        let events = root.path().join("events.jsonl");
        let prompt = "Inspect the F1 predicate failure.\n\
- write-pressure target: src/app/page.tsx (selection_reason=contract_attribute)";
        let options = RunSessionOptions::plan_step(RunSessionStepKind::Inspect)
            .with_repair_target_priority(
                crate::planner::repair_targeting::RepairTargetPriority::FixIntent,
            );
        let mut state = WriteRequiredState::default();

        let feedback = maybe_read_only_stagnation_feedback(
            Some(&events),
            root.path(),
            "nextjs",
            prompt,
            7,
            7,
            &options,
            &mut state,
            &RunSessionErrorContext::default(),
            &[],
            &["package.json".to_string(), "src/app/page.tsx".to_string()],
            &[],
            None,
        )
        .expect("write pressure feedback");

        assert_eq!(state.selected_targets(), ["src/app/page.tsx"]);
        assert_eq!(
            state.selection_reason(),
            Some(WriteRequiredSelectionReason::ContractAttribute)
        );
        assert!(feedback.contains("src/app/page.tsx"));
        assert!(!feedback.contains("package.json"));
        let event = std::fs::read_to_string(events).unwrap();
        assert!(event.contains(r#""selection_reason":"contract_attribute""#));
        assert!(!event.contains(r#""target_path":"package.json""#));
    }

    const CONTENT_G_SMOKE_CHECK: &str = "const { readOne } = require('./src/lib/persistence');\n\
const value = readOne('missing');\n\
if (value !== null) throw new Error('expected null, got ' + value);\n";

    fn content_g_workspace(persistence_js: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src/lib")).unwrap();
        std::fs::write(dir.path().join("smoke-check.js"), CONTENT_G_SMOKE_CHECK).unwrap();
        std::fs::write(dir.path().join("src/lib/persistence.js"), persistence_js).unwrap();
        std::fs::write(dir.path().join("src/lib/persistence.ts"), "export {};\n").unwrap();
        std::fs::write(
            dir.path().join("src/lib/types.ts"),
            "export type Todo = {};\n",
        )
        .unwrap();
        dir
    }

    fn verify_storage(root: &Path) -> crate::planner::verify::VerificationReport {
        let step = crate::planner::step_plan::PlanStep {
            id: "verify-storage".to_string(),
            kind: "verify".to_string(),
            expected_result: "pass".to_string(),
            instruction: "Run the storage smoke check".to_string(),
            expected_paths: Vec::new(),
            verify: vec!["node smoke-check.js".to_string()],
        };
        crate::planner::verify::verify_step(root, &step)
    }

    fn write_required_state(root: &Path, prompt: &str) -> (WriteRequiredState, String) {
        let events = root.join("events.jsonl");
        let options = RunSessionOptions::plan_step(RunSessionStepKind::Verify)
            .with_path_fallback_candidates(vec!["src/lib/types.ts".to_string()]);
        let mut state = WriteRequiredState::default();
        let feedback = maybe_read_only_stagnation_feedback(
            Some(&events),
            root,
            "nextjs",
            prompt,
            7,
            7,
            &options,
            &mut state,
            &RunSessionErrorContext::default(),
            &[],
            &[],
            &["src/lib/types.ts".to_string()],
            None,
        )
        .expect("write pressure feedback");
        (state, feedback)
    }

    #[test]
    fn failed_node_verify_names_the_loaded_file_for_write_pressure() {
        let dir = content_g_workspace("exports.readOne = () => undefined;\n");
        let report = verify_storage(dir.path());
        assert_eq!(report.command_failures.len(), 1, "{report:?}");
        assert!(report.dependency_missing.is_empty(), "{report:?}");
        let prompt = crate::planner::repair::build_repair_prompt_with_context(
            "verify-storage",
            &report,
            &crate::planner::repair::RepairContext {
                workspace_root: Some(dir.path().to_path_buf()),
                ..Default::default()
            },
        );

        let (state, feedback) = write_required_state(dir.path(), &prompt);

        assert_eq!(state.selected_targets(), ["src/lib/persistence.js"]);
        assert_eq!(
            state.selection_reason(),
            Some(WriteRequiredSelectionReason::DiagnosisMapped)
        );
        assert!(feedback.contains("src/lib/persistence.js"), "{feedback}");
        assert!(!feedback.contains("src/lib/types.ts"), "{feedback}");
        let event = std::fs::read_to_string(dir.path().join("events.jsonl")).unwrap();
        assert!(
            event.contains(r#""selection_reason":"diagnosis_mapped""#),
            "{event}"
        );

        std::fs::write(
            dir.path().join("src/lib/persistence.js"),
            "exports.readOne = () => null;\n",
        )
        .unwrap();
        let passed = verify_storage(dir.path());
        assert!(passed.command_failures.is_empty(), "{passed:?}");
    }

    #[test]
    fn unnameable_failure_keeps_the_fallback_selection() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src/hooks")).unwrap();
        std::fs::write(
            dir.path().join("src/hooks/useTodos.ts"),
            "import { Todo } from '../lib/types';\n",
        )
        .unwrap();
        let mut report = crate::planner::verify::VerificationReport::pass();
        report.push_command_failure(
            "grep -q \"src/lib/types\" src/hooks/useTodos.ts",
            "exit status 1",
        );
        let prompt = crate::planner::repair::build_repair_prompt_with_context(
            "verify-hook",
            &report,
            &crate::planner::repair::RepairContext {
                workspace_root: Some(dir.path().to_path_buf()),
                ..Default::default()
            },
        );
        assert!(!prompt.contains("write-pressure target"), "{prompt}");

        let (state, _) = write_required_state(dir.path(), &prompt);

        assert_eq!(
            state.selected_targets().first().map(String::as_str),
            Some("src/lib/types.ts")
        );
        assert_eq!(
            state.selection_reason(),
            Some(WriteRequiredSelectionReason::Fallback)
        );
    }
}
