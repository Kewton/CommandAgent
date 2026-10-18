//! The status-to-report mapping the completion contract and the planner share.
//!
//! Only the five-status core lives here. Each adapter keeps what differs for
//! it: where the reason comes from, the invalid-semver remedy, the setup output,
//! and whether the result is required for completion. The mode is explicit
//! because the reasons deliberately differ per mode and must not be unified.

use super::diagnostics::{BuildVerifierStatus, CompileError};
use super::report::VerificationReport;

/// Which caller is mapping a status. `Required` and `Deferred` share every
/// prefix today; they stay separate so a caller never relies on an implicit
/// default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildReportMode {
    Required,
    Deferred,
    Planner,
}

impl BuildReportMode {
    fn failed_prefix(self) -> &'static str {
        match self {
            Self::Required | Self::Deferred => "build_verify_failed",
            Self::Planner => "dependency_setup_lifecycle_failed",
        }
    }
}

pub struct BuildStatusInput<'a> {
    pub mode: BuildReportMode,
    pub status: BuildVerifierStatus,
    pub command: &'a str,
    /// Reason used for `PolicyRejected` and `Blocked` in every mode.
    pub reason: &'a str,
    pub compile_errors: &'a [CompileError],
}

/// Records a non-passing status on `report` and returns whether it passed.
///
/// `dependency_reason` and `failed_reason` are lazy because only the planner
/// adapter derives them from setup output and the manifest.
pub fn push_build_status_with(
    report: &mut VerificationReport,
    input: BuildStatusInput<'_>,
    dependency_reason: impl FnOnce() -> String,
    failed_reason: impl FnOnce() -> String,
) -> bool {
    let BuildStatusInput {
        mode,
        status,
        command,
        reason,
        compile_errors,
    } = input;
    match status {
        BuildVerifierStatus::Passed => return true,
        BuildVerifierStatus::DependencyMissing => {
            report.push_dependency_missing(format!(
                "dependency_setup_missing: {}",
                dependency_reason()
            ));
        }
        BuildVerifierStatus::PolicyRejected => {
            report.push_command_failure(
                command.to_string(),
                format!("build_verify_policy_rejected: {reason}"),
            );
        }
        BuildVerifierStatus::Blocked => {
            report.push_profile_failure(format!(
                "build_verify_blocked: command `{command}` reason `{reason}`"
            ));
        }
        BuildVerifierStatus::Failed if compile_errors.is_empty() => {
            report.push_command_failure(
                command.to_string(),
                format!("{}: {}", mode.failed_prefix(), failed_reason()),
            );
        }
        BuildVerifierStatus::Failed => {
            report.push_compile_errors(command.to_string(), compile_errors.to_vec());
        }
    }
    false
}

/// The mapping for callers whose dependency and failure reasons are `reason`.
pub fn push_build_status(report: &mut VerificationReport, input: BuildStatusInput<'_>) -> bool {
    let reason = input.reason;
    push_build_status_with(report, input, || reason.to_string(), || reason.to_string())
}
