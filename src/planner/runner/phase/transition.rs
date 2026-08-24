use super::state::{
    InvalidTransition, PhaseEffect, PhaseFailureStage, PhaseIntentKind, PhaseObservation,
    PhaseState, PhaseTransition,
};

pub(super) fn transition(
    state: PhaseState,
    observation: PhaseObservation,
) -> Result<PhaseTransition, InvalidTransition> {
    if state.terminal() {
        return Err(InvalidTransition { state, observation });
    }

    let (next, effect) = match (state, observation) {
        (PhaseState::Initializing, PhaseObservation::Initialized) => {
            (PhaseState::PhaseStart { index: 0 }, PhaseEffect::StartPhase)
        }
        (PhaseState::PhaseStart { index }, PhaseObservation::PhaseStarted) => (
            PhaseState::PhasePlanning { index },
            PhaseEffect::ResolvePlan,
        ),
        (PhaseState::PhasePlanning { index }, PhaseObservation::PlanResolved) => (
            PhaseState::PhasePlanReady { index },
            PhaseEffect::PersistPlan,
        ),
        (PhaseState::PhasePlanReady { index }, PhaseObservation::PlanPersisted { kind }) => (
            PhaseState::IntentBeforePhase { index, kind },
            PhaseEffect::RunIntentHook,
        ),
        (
            PhaseState::IntentBeforePhase {
                index,
                kind: PhaseIntentKind::Standard,
            },
            PhaseObservation::StandardIntentSelected,
        ) => (
            PhaseState::PhaseExecuting { index },
            PhaseEffect::ExecutePlan,
        ),
        (
            PhaseState::IntentBeforePhase { index, kind },
            PhaseObservation::IntentConsumed { .. },
        ) if kind != PhaseIntentKind::Standard => (
            PhaseState::IntentPhaseConsumed { index, kind },
            PhaseEffect::RecordIntentConsumed,
        ),
        (
            PhaseState::IntentPhaseConsumed { index, .. },
            PhaseObservation::IntentConsumed {
                has_next_phase: true,
            },
        ) => (
            PhaseState::PhaseStart { index: index + 1 },
            PhaseEffect::StartPhase,
        ),
        (
            PhaseState::IntentPhaseConsumed { kind, .. },
            PhaseObservation::IntentConsumed {
                has_next_phase: false,
            },
        ) => (
            PhaseState::IntentFinalizing { kind },
            PhaseEffect::FinalizeIntent,
        ),
        (
            PhaseState::PhaseExecuting { index },
            PhaseObservation::StepPlanSucceeded { final_phase },
        ) => (
            PhaseState::InvariantChecking { index, final_phase },
            PhaseEffect::VerifyInvariant,
        ),
        (
            PhaseState::InvariantChecking { index, final_phase },
            PhaseObservation::InvariantPassed,
        ) => (
            PhaseState::PhaseCommitting { index, final_phase },
            PhaseEffect::CommitPhase,
        ),
        (
            PhaseState::InvariantChecking {
                index,
                final_phase: true,
            },
            PhaseObservation::FinalInvariantObserved,
        ) => (
            PhaseState::PhaseCommitting {
                index,
                final_phase: true,
            },
            PhaseEffect::CommitPhase,
        ),
        (
            PhaseState::InvariantChecking {
                index,
                final_phase: false,
            },
            PhaseObservation::InvariantNeedsRepair,
        ) => (
            PhaseState::InvariantRepairing { index },
            PhaseEffect::RepairInvariant,
        ),
        (PhaseState::InvariantRepairing { index }, PhaseObservation::InvariantRepairAttempted) => (
            PhaseState::InvariantChecking {
                index,
                final_phase: false,
            },
            PhaseEffect::VerifyInvariant,
        ),
        (
            PhaseState::InvariantRepairing { .. }
            | PhaseState::InvariantChecking {
                final_phase: false, ..
            },
            PhaseObservation::InvariantRepairExhausted,
        ) => (
            PhaseState::failed(PhaseFailureStage::ProfileInvariant),
            PhaseEffect::Fail,
        ),
        (
            PhaseState::PhaseCommitting { index, .. },
            PhaseObservation::PhaseCommitted {
                has_next_phase: true,
                ..
            },
        ) => (
            PhaseState::PhaseStart { index: index + 1 },
            PhaseEffect::StartPhase,
        ),
        (
            PhaseState::PhaseCommitting { .. },
            PhaseObservation::PhaseCommitted {
                has_next_phase: false,
                intent: PhaseIntentKind::Standard,
            },
        ) => (
            PhaseState::FinalAcceptance { cycle: 0 },
            PhaseEffect::RunFinalAcceptance,
        ),
        (
            PhaseState::PhaseCommitting { .. },
            PhaseObservation::PhaseCommitted {
                has_next_phase: false,
                intent,
            },
        ) => (
            PhaseState::IntentFinalizing { kind: intent },
            PhaseEffect::FinalizeIntent,
        ),
        (PhaseState::IntentFinalizing { .. }, PhaseObservation::IntentFinalized)
        | (PhaseState::FinalAcceptance { .. }, PhaseObservation::AcceptancePassed) => {
            (PhaseState::Completed, PhaseEffect::Complete)
        }
        (
            PhaseState::FinalAcceptance { .. },
            PhaseObservation::AcceptanceNeedsRepair { attempt, mode },
        ) => (
            PhaseState::FinalRepair { attempt, mode },
            PhaseEffect::RunFinalRepair,
        ),
        (
            PhaseState::FinalRepair { .. },
            PhaseObservation::FinalRepairRequiresRecheck { cycle },
        ) => (
            PhaseState::FinalAcceptance { cycle },
            PhaseEffect::RunFinalAcceptance,
        ),
        (_, PhaseObservation::Failed { stage }) => (PhaseState::failed(stage), PhaseEffect::Fail),
        (_, PhaseObservation::Interrupted { stage }) => {
            (PhaseState::Interrupted { stage }, PhaseEffect::Interrupt)
        }
        (state, observation) => return Err(InvalidTransition { state, observation }),
    };

    Ok(PhaseTransition { next, effect })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::runner::phase::state::{FinalRepairMode, PhaseIntentKind};

    #[test]
    fn standard_phase_reaches_final_acceptance_without_inventing_intent_states() {
        let mut state = PhaseState::Initializing;
        for observation in [
            PhaseObservation::Initialized,
            PhaseObservation::PhaseStarted,
            PhaseObservation::PlanResolved,
            PhaseObservation::PlanPersisted {
                kind: PhaseIntentKind::Standard,
            },
            PhaseObservation::StandardIntentSelected,
            PhaseObservation::StepPlanSucceeded { final_phase: true },
            PhaseObservation::InvariantPassed,
            PhaseObservation::PhaseCommitted {
                has_next_phase: false,
                intent: PhaseIntentKind::Standard,
            },
        ] {
            state = transition(state, observation).unwrap().next;
        }
        assert_eq!(state, PhaseState::FinalAcceptance { cycle: 0 });
    }

    #[test]
    fn final_invariant_failure_is_deferred_while_non_final_enters_repair() {
        let final_transition = transition(
            PhaseState::InvariantChecking {
                index: 1,
                final_phase: true,
            },
            PhaseObservation::FinalInvariantObserved,
        )
        .unwrap();
        assert_eq!(
            final_transition.next,
            PhaseState::PhaseCommitting {
                index: 1,
                final_phase: true
            }
        );

        let intermediate_transition = transition(
            PhaseState::InvariantChecking {
                index: 0,
                final_phase: false,
            },
            PhaseObservation::InvariantNeedsRepair,
        )
        .unwrap();
        assert_eq!(
            intermediate_transition.next,
            PhaseState::InvariantRepairing { index: 0 }
        );
    }

    #[test]
    fn final_repair_returns_to_acceptance_before_completion() {
        let state = PhaseState::FinalRepair {
            attempt: 1,
            mode: FinalRepairMode::Appended,
        };
        assert_eq!(
            transition(
                state,
                PhaseObservation::FinalRepairRequiresRecheck { cycle: 1 }
            )
            .unwrap()
            .next,
            PhaseState::FinalAcceptance { cycle: 1 }
        );
    }

    #[test]
    fn terminal_states_reject_every_outgoing_transition() {
        for state in [
            PhaseState::Completed,
            PhaseState::Failed {
                stage: PhaseFailureStage::FinalAcceptance,
            },
            PhaseState::Interrupted {
                stage: PhaseFailureStage::PhaseExecute,
            },
        ] {
            assert!(transition(state, PhaseObservation::AcceptancePassed).is_err());
        }
    }
}
