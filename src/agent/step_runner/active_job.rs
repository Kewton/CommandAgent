//! Active recovery-job ownership.
//!
//! Active job facts decide which contract owns a failure. The decision is
//! rendered for repair; it does not execute the repair.
#![allow(dead_code)]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RecoveryOwner {
    Setup,
    Manifest,
    Scaffold,
    RouteIntegration,
    Source,
    Test,
    Docs,
    EvidenceBinding,
    VerifierContract,
    ToolProtocol,
    ContractConflict,
    ExplicitStop,
}

impl RecoveryOwner {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Setup => "setup",
            Self::Manifest => "manifest",
            Self::Scaffold => "scaffold",
            Self::RouteIntegration => "route_integration",
            Self::Source => "source",
            Self::Test => "test",
            Self::Docs => "docs",
            Self::EvidenceBinding => "evidence_binding",
            Self::VerifierContract => "verifier_contract",
            Self::ToolProtocol => "tool_protocol",
            Self::ContractConflict => "contract_conflict",
            Self::ExplicitStop => "explicit_stop",
        }
    }

    pub(crate) fn for_active_job(job: &str) -> Self {
        match job {
            "setup_bootstrap" => Self::Setup,
            "manifest_repair" => Self::Manifest,
            "scaffold_materialization" => Self::Scaffold,
            "route_integration_repair" => Self::RouteIntegration,
            "source_implementation_repair" => Self::Source,
            "test_artifact_completion" | "test_alignment_repair" => Self::Test,
            "documentation_repair" => Self::Docs,
            "evidence_binding_repair" => Self::EvidenceBinding,
            "verifier_contract_correction" => Self::VerifierContract,
            "tool_protocol_correction" => Self::ToolProtocol,
            "contract_conflict" => Self::ContractConflict,
            _ => Self::ExplicitStop,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ActiveJobDecision {
    pub(crate) owner: RecoveryOwner,
    pub(crate) job: String,
    pub(crate) priority: u8,
    pub(crate) reason: String,
}

impl ActiveJobDecision {
    pub(crate) fn new(job: impl Into<String>, priority: u8, reason: impl Into<String>) -> Self {
        let job = job.into();
        Self {
            owner: RecoveryOwner::for_active_job(&job),
            job,
            priority,
            reason: reason.into(),
        }
    }

    pub(crate) fn render_line(&self) -> String {
        format!(
            "owner={} job={} priority={} reason={}",
            self.owner.as_str(),
            self.job,
            self.priority,
            self.reason.split_whitespace().collect::<Vec<_>>().join(" ")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_job_maps_to_recovery_owner() {
        let decision = ActiveJobDecision::new("route_integration_repair", 40, "profile failure");

        assert_eq!(decision.owner, RecoveryOwner::RouteIntegration);
        assert!(decision.render_line().contains("owner=route_integration"));
    }
}
