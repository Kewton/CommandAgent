//! Host-produced equivalence records exist only before closure. Projecting a
//! checked strengthening retains #478's exact scope/owner comparisons.
use super::*;
use crate::minimal_loop::evidence::{command_diagnosis, import_check};
use crate::planner::recovery_inspection::verifier_obligations::{FailureClass, failure};

#[derive(Clone, PartialEq, Eq, serde::Serialize)]
pub(super) struct Replacement {
    original_command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    import_target: Option<String>,
    expected_result: String,
    replacement_command: String,
    reason: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    package_script: Option<super::package_script_formation::Obligation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    literal_markers: Option<super::literal_marker_formation::Obligation>,
}

pub(super) fn project(
    original: &StepPlan,
    proposed: &StepPlan,
    package_scripts: Option<&super::package_script_formation::PackageScripts>,
    marker_checks: &[super::literal_marker_formation::Obligation],
) -> (StepPlan, Vec<Replacement>) {
    let mut projected = proposed.clone();
    let mut records = Vec::new();
    for step in &original.steps {
        if step.expected_result != "pass" {
            continue;
        }
        for command in &step.verify {
            let Some(target) = import_check::pure_target(command) else {
                continue;
            };
            for candidate in &mut projected.steps {
                if candidate.expected_result != step.expected_result {
                    continue;
                }
                for replacement in &mut candidate.verify {
                    let structural =
                        import_check::export_set_target(replacement).as_ref() == Some(&target);
                    if !structural
                        && import_check::strengthened_target(replacement).as_ref() != Some(&target)
                    {
                        continue;
                    }
                    records.push(Replacement {
                        original_command: command.clone(),
                        import_target: Some(target.clone()),
                        expected_result: step.expected_result.clone(),
                        replacement_command: replacement.clone(),
                        reason: if structural {
                            "identical_import_then_explicit_runtime_export_set"
                        } else {
                            "identical_import_then_direct_target_assertion_without_catch"
                        },
                        package_script: None,
                        literal_markers: None,
                    });
                    *replacement = command.clone();
                }
            }
        }
    }
    if let Some(package_scripts) = package_scripts {
        for obligation in &package_scripts.obligations {
            for candidate in &mut projected.steps {
                if candidate.expected_result != obligation.expected_result {
                    continue;
                }
                for replacement in &mut candidate.verify {
                    // Canonical whole-command equality preserves output, failure
                    // propagation, target and the literal acquired before retry.
                    if *replacement != obligation.replacement_command {
                        continue;
                    }
                    records.push(Replacement {
                        original_command: obligation.original_command.clone(), import_target: None,
                        expected_result: obligation.expected_result.clone(), replacement_command: replacement.clone(),
                        reason: "saved_package_script_literal_strict_comparison_then_original_output",
                        package_script: Some(obligation.clone()),
                        literal_markers: None,
                    });
                    *replacement = obligation.original_command.clone();
                }
            }
        }
    }
    for obligation in marker_checks {
        for candidate in &mut projected.steps {
            if candidate.expected_result != obligation.expected_result {
                continue;
            }
            for replacement in &mut candidate.verify {
                if super::literal_marker_formation::matches(obligation, replacement) {
                    records.push(Replacement {
                        original_command: obligation.original_command.clone(),
                        import_target: None,
                        expected_result: obligation.expected_result.clone(),
                        replacement_command: replacement.clone(),
                        reason: "identical_file_ordered_literal_predicates_and_output_without_catch",
                        package_script: None,
                        literal_markers: Some(obligation.clone()),
                    });
                    *replacement = obligation.original_command.clone();
                }
            }
        }
    }
    (projected, records)
}

pub(super) fn preserve_registered(
    records: &[Replacement],
    proposed: &StepPlan,
) -> anyhow::Result<()> {
    for record in records {
        if !proposed.steps.iter().any(|s| {
            s.expected_result == record.expected_result
                && s.verify.contains(&record.replacement_command)
        }) {
            return Err(failure(
                FailureClass::ProposalRepairable,
                format!(
                    "formation lost validated replacement or changed its explicit expectation: {}",
                    record.replacement_command
                ),
            ));
        }
    }
    Ok(())
}

pub(super) fn require_formed(config: &Config, commands: &[String]) -> anyhow::Result<()> {
    let diagnoses = command_diagnosis::collect(&config.workspace_root, commands);
    if let Some(reason) = command_diagnosis::immutable_failure(&diagnoses) {
        return Err(failure(
            FailureClass::ProposalRepairable,
            format!(
                "preclosure weak inline verification requires formation: {reason}\nPackage-script value displays require a fixed literal in saved original model/host obligations; a value absent from the command alone is not grounds for refusal. Use only the supplied saved-literal replacement, including strict comparison and original stdout. Dynamic properties, wrappers, ambiguous/conflicting or absent original literals are unsupported. Never infer expectations from artifacts or a later proposal.\nPreserve each original import target and pass result. Replace a pure import only with a target-specific check in this closed shape: node -e \"import('./original-path.js').then(actual=>{{require('node:assert/strict').deepStrictEqual(actual.exportName,42)}})\". Supply the actual required export/value, or actual.exportName(...[literal JSON arguments]) and a literal JSON expected result. Alternatively explicitly propose the sorted runtime export-name set using deepStrictEqual(Object.keys(actual).sort(),[literal JSON names]); an explicitly empty set can describe a type-only module. That alternative verifies loadability and runtime export boundary only, not interface fields or business behavior. Do not infer its expectation from a filename. Preserve all original instructions, outputs and owner order. No unrelated assertion, added check beside the weak original, catch or success override discharges it. Unsupported checks must be reproposed with preserved scope or stop within the current budget."
            ),
        ));
    }
    Ok(())
}

pub(super) fn require_repairable(
    config: &Config,
    contract: &CompletionContract,
) -> anyhow::Result<()> {
    let diagnoses = command_diagnosis::collect(&config.workspace_root, &contract.verify_commands);
    crate::eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event":"recovery_verifier_repairability", "stage":"closed_binding",
            "registered_verify_commands":contract.verify_commands, "command_diagnoses":diagnoses,
            "contract_unchanged":true,
        }),
    );
    if let Some(reason) = command_diagnosis::contract_failure(contract, &diagnoses) {
        return Err(failure(
            FailureClass::OriginalInconsistent,
            format!(
                "fixed verification evidence is not repairable by app edits; preserve command/hash and stop: {reason}"
            ),
        ));
    }
    Ok(())
}
