use crate::config::Config;
use crate::planner::profile::{ProfileBehaviorProbeReport, ProfileId};

pub(crate) fn registered_probe(
    config: &Config,
    profile_id: &ProfileId,
) -> Option<ProfileBehaviorProbeReport> {
    match crate::minimal_loop::completion_observations::run_if_registered(config, profile_id) {
        Ok(report) => report,
        Err(error) => Some(ProfileBehaviorProbeReport {
            status: "failed",
            reasons: vec![format!(
                "completion_contract_command_observation_error: {error}"
            )],
            evidence_path: None,
        }),
    }
}

pub(crate) fn non_recovery_failure_kind(primary_reason: &str) -> Option<String> {
    [
        (
            "unsupported completion obligation role",
            "profile_contract:unsupported_completion_obligation",
        ),
        (
            "missing_required_obligation_target:",
            "profile_contract:missing_required_obligation_target",
        ),
        (
            "completion contract binding required but missing",
            "profile_contract:completion_contract_binding_missing",
        ),
    ]
    .into_iter()
    .find_map(|(needle, kind)| primary_reason.contains(needle).then(|| kind.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_profile_contract_failures_as_non_recovery() {
        assert_eq!(
            non_recovery_failure_kind(
                "missing_required_obligation_target:verification:tests/test_main.py"
            )
            .as_deref(),
            Some("profile_contract:missing_required_obligation_target")
        );
        assert!(non_recovery_failure_kind("direct_cli_command_failed").is_none());
    }
}
