//! A supported reader consumes inputs; it does not own their production.
//! This is a closed profile check grammar, not side-effect analysis of scripts.
use super::*;
use crate::planner::recovery_contract_authority::verifier_obligations as scope;
use crate::planner::recovery_inspection::verifier_obligations::{FailureClass, failure};

pub(super) fn supported(step: &PlanStep) -> bool {
    // Require the complete bounded inspection duty AND exact product-generated
    // commands. Kind, ID, paths or a model's read-only claim are insufficient.
    // Other instructions (including appended creation work), empty checks,
    // arbitrary inline code and external scripts retain the old ownership rule.
    step.step_kind() == StepKind::Verify
        && step.instruction
            == "Verify the profile-owned package_manifest contract by running every declared check and report any exact failure."
        && !step.expected_paths.is_empty()
        && step
            .expected_paths
            .iter()
            .all(|p| scope::normalized(p).is_ok_and(|p| p == "package.json"))
        && !step.verify.is_empty()
        && step
            .verify
            .iter()
            .all(|c| crate::planner::profiles::nextjs::recovery_authority::is_package_check(c))
}

pub(super) fn preserve(original: &StepPlan, proposed: &StepPlan) -> anyhow::Result<()> {
    let mut previous = None;
    for (original_index, reader) in original.steps.iter().enumerate() {
        if !supported(reader) {
            continue;
        }
        // An unclassified Verify sharing the input might also write it. Do not
        // silently remove that potential owner from the old ownership boundary.
        let unknown_owner = proposed.steps.iter().any(|step| {
            step.step_kind() == StepKind::Verify
                && scope::overlaps(reader, step)
                && !supported(step)
        });
        // One complete, distinct execution discharges one acquired obligation.
        // Greedy ordered matching leaves maximum room for subsequent readers.
        let matched = proposed
            .steps
            .iter()
            .enumerate()
            .find(|(index, candidate)| {
                !unknown_owner
                    && previous.is_none_or(|previous| previous < *index)
                    && supported(candidate)
                    && candidate.instruction == reader.instruction
                    && candidate.expected_result == reader.expected_result
                    && reader.expected_paths.iter().all(|p| {
                        candidate
                            .expected_paths
                            .iter()
                            .any(|c| scope::normalized(p).ok() == scope::normalized(c).ok())
                    })
                    && reader.verify.iter().all(|c| candidate.verify.contains(c))
                    && within_boundary(original, proposed, original_index, *index)
            });
        let Some((index, _)) = matched else {
            return Err(failure(
                FailureClass::ProposalRepairable,
                format!(
                    "formation lost original read-only Verify duty {}: keep instruction, expected result, required inputs and complete checks together on a distinct Verify within its original output-owner boundary",
                    reader.id
                ),
            ));
        };
        previous = Some(index);
    }
    Ok(())
}

fn within_boundary(
    original: &StepPlan,
    proposed: &StepPlan,
    reader_index: usize,
    candidate_index: usize,
) -> bool {
    original.steps.iter().enumerate().all(|(index, owner)| {
        if !matches!(owner.step_kind(), StepKind::Implement | StepKind::Setup) {
            return true;
        }
        // Retain both sides, including all outputs of a split producer. The
        // ordinary preservation rule separately validates each owner's duties.
        owner.expected_paths.iter().all(|path| {
            let Ok(path) = scope::normalized(path) else {
                return false;
            };
            let owners: Vec<_> = proposed
                .steps
                .iter()
                .enumerate()
                .filter(|(_, s)| {
                    s.step_kind() == owner.step_kind()
                        && s.expected_paths
                            .iter()
                            .any(|p| scope::normalized(p).is_ok_and(|p| p == path))
                })
                .collect();
            !owners.is_empty()
                && owners.iter().all(|(candidate_owner, _)| {
                    if index < reader_index {
                        *candidate_owner < candidate_index
                    } else {
                        candidate_index < *candidate_owner
                    }
                })
        })
    })
}
