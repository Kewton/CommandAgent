use super::state::{
    FinalRepairMode, InvalidTransition, PhaseFailureStage, PhaseIntentKind, PhaseObservation,
    PhaseState, PhaseTransition,
};
use super::transition::transition;

/// I/O-free state holder used by the phase effect driver.
///
/// Event, filesystem, provider, and process effects remain in `flow`; this
/// object only rejects control-flow drift from the reviewed E-5f table.
#[derive(Debug)]
pub(super) struct PhaseMachine {
    state: PhaseState,
}

impl PhaseMachine {
    pub(super) fn start() -> Result<Self, InvalidTransition> {
        let mut machine = Self {
            state: PhaseState::Initializing,
        };
        machine.observe(PhaseObservation::Initialized)?;
        Ok(machine)
    }

    pub(super) fn interrupt(&mut self, stage: &str) -> Result<(), InvalidTransition> {
        self.observe(PhaseObservation::Interrupted {
            stage: PhaseFailureStage::from_label(stage),
        })?;
        Ok(())
    }

    pub(super) fn phase_started(&mut self) -> Result<(), InvalidTransition> {
        self.observe(PhaseObservation::PhaseStarted)?;
        Ok(())
    }

    pub(super) fn plan_resolved(&mut self) -> Result<(), InvalidTransition> {
        self.observe(PhaseObservation::PlanResolved)?;
        Ok(())
    }

    pub(super) fn plan_persisted(
        &mut self,
        fix_before: bool,
        investigation_before: bool,
    ) -> Result<(), InvalidTransition> {
        let kind = if fix_before {
            PhaseIntentKind::Fix
        } else if investigation_before {
            PhaseIntentKind::Investigation
        } else {
            PhaseIntentKind::Standard
        };
        self.observe(PhaseObservation::PlanPersisted { kind })?;
        Ok(())
    }

    pub(super) fn before_phase_completed(
        &mut self,
        consumed: bool,
        final_phase: bool,
    ) -> Result<(), InvalidTransition> {
        if consumed {
            let observation = PhaseObservation::IntentConsumed {
                has_next_phase: !final_phase,
            };
            self.observe(observation)?;
            self.observe(observation)?;
        } else {
            self.observe(PhaseObservation::StandardIntentSelected)?;
        }
        Ok(())
    }

    pub(super) fn step_succeeded(&mut self, final_phase: bool) -> Result<(), InvalidTransition> {
        self.observe(PhaseObservation::StepPlanSucceeded { final_phase })?;
        Ok(())
    }

    pub(super) fn invariant_needs_repair(&mut self) -> Result<(), InvalidTransition> {
        self.observe(PhaseObservation::InvariantNeedsRepair)?;
        Ok(())
    }

    pub(super) fn invariant_repair_attempted(&mut self) -> Result<(), InvalidTransition> {
        self.observe(PhaseObservation::InvariantRepairAttempted)?;
        Ok(())
    }

    pub(super) fn invariant_exhausted(&mut self) -> Result<(), InvalidTransition> {
        self.observe(PhaseObservation::InvariantRepairExhausted)?;
        Ok(())
    }

    pub(super) fn invariant_observed(&mut self, passed: bool) -> Result<(), InvalidTransition> {
        let observation = if passed {
            PhaseObservation::InvariantPassed
        } else {
            PhaseObservation::FinalInvariantObserved
        };
        self.observe(observation)?;
        Ok(())
    }

    pub(super) fn phase_committed(
        &mut self,
        final_phase: bool,
        intent: &str,
    ) -> Result<(), InvalidTransition> {
        self.observe(PhaseObservation::PhaseCommitted {
            has_next_phase: !final_phase,
            intent: PhaseIntentKind::from_intent(intent),
        })?;
        Ok(())
    }

    pub(super) fn intent_finished(&mut self, success: bool) -> Result<(), InvalidTransition> {
        let observation = if success {
            super::intent_completion::successful_observation(self.state)
        } else {
            PhaseObservation::Failed {
                stage: PhaseFailureStage::from_label("intent_finalization"),
            }
        };
        self.observe(observation)?;
        Ok(())
    }

    pub(super) fn acceptance_needs_repair(
        &mut self,
        attempt: usize,
        policy: &str,
    ) -> Result<(), InvalidTransition> {
        self.observe(PhaseObservation::AcceptanceNeedsRepair {
            attempt,
            mode: FinalRepairMode::from_policy(policy),
        })?;
        Ok(())
    }

    pub(super) fn fail(&mut self, stage: &str) -> Result<(), InvalidTransition> {
        self.observe(PhaseObservation::Failed {
            stage: PhaseFailureStage::from_label(stage),
        })?;
        Ok(())
    }

    pub(super) fn acceptance_passed(&mut self, cycle: usize) -> Result<(), InvalidTransition> {
        if matches!(self.state, PhaseState::FinalRepair { .. }) {
            self.observe(PhaseObservation::FinalRepairRequiresRecheck { cycle })?;
        }
        self.observe(PhaseObservation::AcceptancePassed)?;
        Ok(())
    }

    fn observe(
        &mut self,
        observation: PhaseObservation,
    ) -> Result<PhaseTransition, InvalidTransition> {
        let next = transition(self.state, observation)?;
        self.state = next.next;
        Ok(next)
    }
}
