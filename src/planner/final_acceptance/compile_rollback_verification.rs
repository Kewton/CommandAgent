use crate::config::Config;
use crate::minimal_loop::build_verifier::{
    self, BuildVerifierStatus, emit_dependency_build_lifecycle,
};
use crate::minimal_loop::dependency_setup::NodeDependencySetupAuthority;
use crate::planner::profile::is_production_build_command;
use crate::planner::verify::VerificationReport;
use crate::{eval_events, json};

pub(super) struct RollbackBuildVerification {
    pub(super) passed: bool,
    command: String,
    status: String,
    reason: String,
    duration_ms: Option<u64>,
}

pub(super) fn run(
    config: &Config,
    profile: &str,
    phase_id: &str,
    failed_report: &VerificationReport,
) -> RollbackBuildVerification {
    let command = failed_report
        .command_failures
        .iter()
        .map(|failure| failure.command.as_str())
        .find(|command| is_production_build_command(Some(profile), command));
    let Some(command) = command else {
        return unavailable("rollback has no registered production build command");
    };
    let Some(requirement) = build_verifier::requirement_from_deferred(
        command,
        Some(profile),
        "compile rollback requires production build re-verification",
        "compile_rollback_reverification",
        "required",
    ) else {
        return unavailable("rollback production build command has no build verifier");
    };
    let lifecycle = build_verifier::observe_requirement_lifecycle_with_offline(
        &config.workspace_root,
        &requirement,
        NodeDependencySetupAuthority::None,
        config.offline,
    );
    emit_dependency_build_lifecycle(
        config.eval_events_path.as_deref(),
        "compile_rollback",
        Some(phase_id),
        &lifecycle,
    );
    let duration_ms = lifecycle.build_duration_ms();
    RollbackBuildVerification {
        command: requirement.command,
        status: lifecycle.final_status.as_str().to_string(),
        reason: lifecycle.final_reason,
        duration_ms,
        passed: lifecycle.final_status == BuildVerifierStatus::Passed,
    }
}

pub(super) fn emit_failed(
    config: &Config,
    phase_id: &str,
    paths: &[String],
    snapshot_origins: &[String],
    exhausted_reason: &str,
    static_report: &VerificationReport,
    build: &RollbackBuildVerification,
) {
    let rebuild_reason = if !static_report.is_pass() {
        static_report.primary_reason()
    } else {
        build.reason.clone()
    };
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "compile_rollback_failed",
            "phase_id": phase_id,
            "paths": paths,
            "snapshot_origins": snapshot_origins,
            "exhausted_reason": exhausted_reason,
            "rebuild_reason": eval_events::body_snippet(&rebuild_reason),
            "build_reverified": false,
            "build_command": build.command,
            "build_reverification_status": build.status,
            "build_duration_ms": build.duration_ms,
        }),
    );
}

pub(super) fn emit_applied(
    config: &Config,
    phase_id: &str,
    paths: &[String],
    snapshot_origins: &[String],
    exhausted_reason: &str,
    carry_forward_guidance: &[String],
    build: &RollbackBuildVerification,
) {
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "compile_rollback_applied",
            "phase_id": phase_id,
            "paths": paths,
            "snapshot_origins": snapshot_origins,
            "exhausted_reason": exhausted_reason,
            "carry_forward_guidance": carry_forward_guidance,
            "build_reverified": true,
            "build_command": build.command,
            "build_reverification_status": build.status,
            "build_duration_ms": build.duration_ms,
        }),
    );
}

fn unavailable(reason: &str) -> RollbackBuildVerification {
    RollbackBuildVerification {
        command: String::new(),
        status: "unavailable".to_string(),
        reason: reason.to_string(),
        duration_ms: None,
        passed: false,
    }
}
