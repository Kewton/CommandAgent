use std::path::Path;

use serde_json::json;

use crate::eval_events;

use super::edit_anchor_recovery::EditAnchorFailureSummary;
use super::loop_run::{RunSessionErrorContext, RunSessionOptions, RunSessionStepKind};
use super::stagnation_escalation::{
    ReadOnlyStagnationStage, WRITE_REQUIRED_NO_WRITE_LIMIT, WriteRequiredState,
};

#[allow(clippy::too_many_arguments)]
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
            } else if let Some(selection) =
                crate::planner::fix_diagnostics::repair_target_from_prompt(user_prompt)
            {
                (selection.into(), String::new())
            } else {
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
                    },
                )?
                .into();
                let state_binding_feedback =
                    crate::planner::state_binding_scan::write_required_feedback(
                        root,
                        profile,
                        &pending_evidence,
                        &pending_error_context.missing_capabilities,
                        eval_events_path,
                    );
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
        let options = RunSessionOptions::plan_step(RunSessionStepKind::Implement);
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
}
