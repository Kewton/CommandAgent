use super::{
    RepairTargetPathBuckets, RepairTargetSelection, RepairTargetSelectionReason,
    ordered_non_empty_paths,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum RepairTargetPriority {
    #[default]
    Legacy,
    FixIntent,
}

impl RepairTargetPriority {
    pub(crate) fn for_intent(intent: &str) -> Self {
        if crate::planner::adjudication::contract::is_fix_intent(intent) {
            Self::FixIntent
        } else {
            Self::Legacy
        }
    }
}

pub(super) fn select(buckets: RepairTargetPathBuckets<'_>) -> Option<RepairTargetSelection> {
    let mapped = buckets.mapped_selection.and_then(|selection| {
        let selected_targets = ordered_non_empty_paths(&selection.selected_targets);
        (!selected_targets.is_empty()).then_some(RepairTargetSelection {
            selected_targets,
            selection_reason: selection.selection_reason,
        })
    });
    if buckets.priority == RepairTargetPriority::Legacy
        && let Some(mapped) = mapped
    {
        return Some(mapped);
    }
    let mut candidates = mapped.into_iter().collect::<Vec<_>>();
    for (paths, reason) in [
        (
            buckets.evidence_mapped_paths,
            RepairTargetSelectionReason::EvidenceMapped,
        ),
        (
            buckets.contract_attribute_paths,
            RepairTargetSelectionReason::ContractAttribute,
        ),
        (
            buckets.repair_changed_paths,
            RepairTargetSelectionReason::RepairChanged,
        ),
        (
            buckets.required_paths,
            RepairTargetSelectionReason::RequiredPath,
        ),
        (
            buckets.fallback_paths,
            RepairTargetSelectionReason::Fallback,
        ),
    ] {
        let selected_targets = ordered_non_empty_paths(paths);
        if !selected_targets.is_empty() {
            candidates.push(RepairTargetSelection {
                selected_targets,
                selection_reason: reason,
            });
        }
    }
    if buckets.priority == RepairTargetPriority::FixIntent {
        candidates.sort_by_key(|selection| fix_priority_rank(selection.selection_reason));
    }
    candidates.into_iter().next()
}

fn fix_priority_rank(reason: RepairTargetSelectionReason) -> u8 {
    match reason {
        RepairTargetSelectionReason::DiagnosisMapped
        | RepairTargetSelectionReason::TracebackMapped => 0,
        RepairTargetSelectionReason::ContractAttribute => 1,
        RepairTargetSelectionReason::EvidenceMapped
        | RepairTargetSelectionReason::RepairChanged => 2,
        RepairTargetSelectionReason::RequiredPath | RepairTargetSelectionReason::Fallback => 3,
    }
}
