use super::RunSessionStepKind;

impl RunSessionStepKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            RunSessionStepKind::Inspect => "inspect",
            RunSessionStepKind::Setup => "setup",
            RunSessionStepKind::Implement => "implement",
            RunSessionStepKind::Verify => "verify",
            RunSessionStepKind::Report => "report",
            RunSessionStepKind::Unknown => "unknown",
        }
    }

    pub(in crate::minimal_loop::loop_run) fn bash_policy_purpose(self) -> &'static str {
        match self {
            RunSessionStepKind::Inspect => "runtime_inspection",
            RunSessionStepKind::Setup => "runtime_setup",
            RunSessionStepKind::Implement => "runtime_implementation",
            RunSessionStepKind::Verify | RunSessionStepKind::Report => {
                "deterministic_verifier_evidence"
            }
            RunSessionStepKind::Unknown => "runtime_unknown",
        }
    }

    pub(in crate::minimal_loop::loop_run) fn requires_verifier_bash_policy(self) -> bool {
        matches!(
            self,
            RunSessionStepKind::Verify | RunSessionStepKind::Report
        )
    }
}
