//! State-aware completion observation for intent-owned runtimes.

use super::state::{PhaseObservation, PhaseState};

pub(super) const fn successful_observation(state: PhaseState) -> PhaseObservation {
    match state {
        PhaseState::FinalAcceptance { .. } => PhaseObservation::AcceptancePassed,
        _ => PhaseObservation::IntentFinalized,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::runner::phase::state::PhaseIntentKind;

    #[test]
    fn recovery_fix_finishes_as_acceptance() {
        assert_eq!(
            successful_observation(PhaseState::FinalAcceptance { cycle: 0 }),
            PhaseObservation::AcceptancePassed
        );
    }

    #[test]
    fn ordinary_fix_finishes_as_intent() {
        assert_eq!(
            successful_observation(PhaseState::IntentFinalizing {
                kind: PhaseIntentKind::Fix,
            }),
            PhaseObservation::IntentFinalized
        );
    }
}
