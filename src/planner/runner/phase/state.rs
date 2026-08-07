#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PhaseIntentKind {
    Standard,
    Fix,
    Investigation,
}

impl PhaseIntentKind {
    pub(super) fn from_intent(intent: &str) -> Self {
        match intent {
            "fix" => Self::Fix,
            "investigate" | "investigation" => Self::Investigation,
            _ => Self::Standard,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FinalRepairMode {
    Appended,
    Compact,
    EvidenceRegeneration,
    CompileRegeneration,
    Rollback,
    Deterministic,
}

impl FinalRepairMode {
    pub(super) fn from_policy(policy: &str) -> Self {
        match policy {
            "compact" => Self::Compact,
            "evidence" | "evidence_regeneration" => Self::EvidenceRegeneration,
            "compile" | "compile_regeneration" => Self::CompileRegeneration,
            "rollback" => Self::Rollback,
            "deterministic" => Self::Deterministic,
            _ => Self::Appended,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PhaseFailureStage {
    Initialization,
    PhaseStart,
    PhaseScaffold,
    PhasePlanPersist,
    BeforePhase,
    PhaseExecute,
    ProfileInvariantRepair,
    ProfileInvariant,
    PhaseTransition,
    IntentFinalization,
    FinalAcceptance,
    FinalRepair,
}

impl PhaseFailureStage {
    pub(super) fn from_label(label: &str) -> Self {
        match label {
            "initialization" => Self::Initialization,
            "phase_start" => Self::PhaseStart,
            "phase_scaffold" => Self::PhaseScaffold,
            "phase_plan_persist" => Self::PhasePlanPersist,
            "before_phase" => Self::BeforePhase,
            "phase_execute" => Self::PhaseExecute,
            "profile_invariant_repair" => Self::ProfileInvariantRepair,
            "profile_invariant" => Self::ProfileInvariant,
            "phase_transition" => Self::PhaseTransition,
            "intent_finalization" => Self::IntentFinalization,
            "final_acceptance" => Self::FinalAcceptance,
            _ => Self::FinalRepair,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PhaseState {
    Initializing,
    PhaseStart {
        index: usize,
    },
    PhasePlanning {
        index: usize,
    },
    PhasePlanReady {
        index: usize,
    },
    IntentBeforePhase {
        index: usize,
        kind: PhaseIntentKind,
    },
    IntentPhaseConsumed {
        index: usize,
        kind: PhaseIntentKind,
    },
    PhaseExecuting {
        index: usize,
    },
    InvariantChecking {
        index: usize,
        final_phase: bool,
    },
    InvariantRepairing {
        index: usize,
    },
    PhaseCommitting {
        index: usize,
        final_phase: bool,
    },
    IntentFinalizing {
        kind: PhaseIntentKind,
    },
    FinalAcceptance {
        cycle: usize,
    },
    FinalRepair {
        attempt: usize,
        mode: FinalRepairMode,
    },
    Completed,
    Failed {
        stage: PhaseFailureStage,
    },
    Interrupted {
        stage: PhaseFailureStage,
    },
}

impl PhaseState {
    pub(super) const fn failed(stage: PhaseFailureStage) -> Self {
        Self::Failed { stage }
    }

    pub(super) fn terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Failed { .. } | Self::Interrupted { .. }
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PhaseObservation {
    Initialized,
    PhaseStarted,
    PlanResolved,
    PlanPersisted {
        kind: PhaseIntentKind,
    },
    StandardIntentSelected,
    IntentConsumed {
        has_next_phase: bool,
    },
    StepPlanSucceeded {
        final_phase: bool,
    },
    InvariantPassed,
    FinalInvariantObserved,
    InvariantNeedsRepair,
    InvariantRepairAttempted,
    InvariantRepairExhausted,
    PhaseCommitted {
        has_next_phase: bool,
        intent: PhaseIntentKind,
    },
    IntentFinalized,
    AcceptancePassed,
    AcceptanceNeedsRepair {
        attempt: usize,
        mode: FinalRepairMode,
    },
    FinalRepairRequiresRecheck {
        cycle: usize,
    },
    Failed {
        stage: PhaseFailureStage,
    },
    Interrupted {
        stage: PhaseFailureStage,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PhaseEffect {
    StartPhase,
    ResolvePlan,
    PersistPlan,
    RunIntentHook,
    RecordIntentConsumed,
    ExecutePlan,
    VerifyInvariant,
    RepairInvariant,
    CommitPhase,
    FinalizeIntent,
    RunFinalAcceptance,
    RunFinalRepair,
    Complete,
    Fail,
    Interrupt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PhaseTransition {
    pub(super) next: PhaseState,
    pub(super) effect: PhaseEffect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct InvalidTransition {
    pub(super) state: PhaseState,
    pub(super) observation: PhaseObservation,
}

impl std::fmt::Display for InvalidTransition {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid phase transition: state={:?}, observation={:?}",
            self.state, self.observation
        )
    }
}

impl std::error::Error for InvalidTransition {}
