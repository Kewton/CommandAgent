use super::*;
use crate::minimal_loop::evidence::RuntimeAcceptanceReport;

pub(super) struct PlanFinalProbe {
    report: Option<ProfileBehaviorProbeReport>,
}

impl PlanFinalProbe {
    pub(super) fn dispatch(
        config: &Config,
        runtime: &dyn ProfileRuntime,
        profile_id: &ProfileId,
        goal: &str,
        required_capabilities: &[String],
    ) -> Self {
        let report = (config.resolved_intent(goal) == "create"
            && runtime.plan_final_behavior_probe_required(profile_id))
        .then(|| {
            run_profile_behavior_probe(
                config,
                profile_id.as_str(),
                goal,
                required_capabilities,
                &VerificationReport::pass(),
            )
        });
        Self { report }
    }

    pub(super) fn bind_release_gate(&self, release_gate: &mut ReleaseGateSummary) {
        let Some(report) = self.report.as_ref() else {
            return;
        };
        if report.status == "failed" {
            release_gate.status = "failed".to_string();
            release_gate.reasons = crate::planner::adjudication::profile_behavior_failure_reasons(
                &release_gate.reasons,
                &report.reasons,
                report.evidence_path.as_deref(),
            );
        } else if matches!(report.status, "partial" | "static") {
            release_gate.status = "partial".to_string();
            release_gate.reasons = dedup_strings(
                release_gate
                    .reasons
                    .iter()
                    .cloned()
                    .chain(std::iter::once(format!(
                        "profile_behavior_probe_{}",
                        report.status
                    )))
                    .collect(),
            );
        }
    }

    pub(super) fn runtime_acceptance_passed(&self, runtime_ok: bool) -> bool {
        runtime_ok
            && self
                .report
                .as_ref()
                .is_none_or(|report| report.status != "failed")
    }

    pub(super) fn runtime_acceptance_status(
        &self,
        runtime_ok: bool,
        runtime_acceptance: Option<&RuntimeAcceptanceReport>,
    ) -> &'static str {
        match self.report.as_ref().map(|report| report.status) {
            Some("failed") => "failed",
            Some("partial") => "partial",
            Some("static") => "static",
            _ => super::runtime_acceptance_status(runtime_ok, runtime_acceptance),
        }
    }

    pub(super) fn assurance(
        &self,
        root: &Path,
        base: (&'static str, &'static str),
    ) -> (String, String) {
        match self.report.as_ref() {
            Some(report) if report.status != "pass" => {
                let (assurance, reason) =
                    crate::completion_metadata::cli::completion_assurance(root);
                (assurance.as_str().to_string(), reason.to_string())
            }
            _ => (base.0.to_string(), base.1.to_string()),
        }
    }

    pub(super) fn event_status(&self) -> &'static str {
        self.report
            .as_ref()
            .map(|report| report.status)
            .unwrap_or("not_applicable")
    }

    pub(super) fn reasons(&self) -> &[String] {
        self.report
            .as_ref()
            .map(|report| report.reasons.as_slice())
            .unwrap_or_default()
    }

    pub(super) fn evidence_path(&self) -> &str {
        self.report
            .as_ref()
            .and_then(|report| report.evidence_path.as_deref())
            .unwrap_or_default()
    }
}

#[cfg(test)]
#[path = "plan_final_probe/tests.rs"]
mod tests;
