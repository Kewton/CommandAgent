use crate::planner::adjudication::investigate::InvestigationRunEvidence;
use crate::planner::fix_diagnostics::ReproducerRun;

pub(crate) fn attach(run: &mut InvestigationRunEvidence, execution: &ReproducerRun) {
    run.stdout.clone_from(&execution.stdout_tail);
    if execution.stderr_tail.trim().is_empty() {
        run.stderr.clone_from(&execution.evidence.reason);
    } else {
        run.stderr.clone_from(&execution.stderr_tail);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::adjudication::contract::{EvidenceStage, ExpectedOutcome, ProbeOutcome};
    use crate::planner::adjudication::fix::FixEvidenceObservation;

    #[test]
    fn empty_stderr_uses_the_deterministic_failure_summary() {
        let mut run = InvestigationRunEvidence::new("test -f missing", 1, ProbeOutcome::Failure);
        let execution = ReproducerRun {
            evidence: FixEvidenceObservation::new(
                "reproducer_fails",
                "test -f missing",
                EvidenceStage::Diagnosis,
                ExpectedOutcome::Failure,
                "reproducer:fixture",
                1,
                "fixture",
                ProbeOutcome::Failure,
                "outcome: CommandFailed status: exit status: 1",
            ),
            diagnostic: None,
            reproducer_defect: None,
            stdout_tail: String::new(),
            stderr_tail: String::new(),
        };

        attach(&mut run, &execution);

        assert_eq!(run.stderr, execution.evidence.reason);
    }
}
