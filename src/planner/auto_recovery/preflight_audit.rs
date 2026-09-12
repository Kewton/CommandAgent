//! Audit the original control on every post-checkpoint exit, including errors
//! in the isolated observer. Legacy reason names are not restoration evidence.
use super::{Config, RecoveryPreflight, json};
use crate::planner::recovery_snapshot::{self as snapshot, RecoveryBoundarySnapshot};

mod restore;

pub(super) fn run(
    config: &Config,
    checkpoint: &RecoveryBoundarySnapshot,
    observe: impl FnOnce() -> RecoveryPreflight,
) -> RecoveryPreflight {
    let outcome = observe();
    let observed = snapshot::current_source_sha256(&config.workspace_root);
    let mut event = json!({
        "event": "recovery_preflight_control_audit",
        "observation_id": checkpoint.workspace_relative_path,
        "observation_isolated": true,
        "control_audited": true,
        "control_status": "observation_failed",
        "control_before_sha256": checkpoint.snapshot_sha256,
        "control_after_observation_sha256": observed.as_ref().ok(),
        "restore_invoked": false,
        "restore_succeeded": false,
        "control_retained": false,
        "isolated_outcome": outcome_kind(&outcome),
        "isolated_reason": outcome_reason(&outcome),
    });
    let failure = match observed {
        Ok(hash) if hash == checkpoint.snapshot_sha256 => {
            event["control_status"] = json!("unchanged");
            event["control_retained"] = json!(true);
            None
        }
        Ok(_) => {
            event["control_status"] = json!("changed");
            // Check before copying and after restoration. A corrupt source
            // must neither overwrite control nor produce a success claim.
            let restored = restore::validate_source(&config.workspace_root, checkpoint)
                .and_then(|()| {
                    event["restore_invoked"] = json!(true);
                    snapshot::restore_transaction(&config.workspace_root, checkpoint)
                })
                .and_then(|report| {
                    anyhow::ensure!(
                        report.snapshot_sha256 == checkpoint.snapshot_sha256,
                        "restored control differs from original checkpoint"
                    );
                    Ok(report)
                });
            match restored {
                Ok(report) => {
                    event["restore_succeeded"] = json!(true);
                    event["control_retained"] = json!(true);
                    event["control_after_restore_sha256"] = json!(report.snapshot_sha256);
                    event["restored_file_count"] = json!(report.restored_file_count);
                    event["removed_file_count"] = json!(report.removed_file_count);
                    Some("preflight_control_source_mutation_rejected_and_restored".to_string())
                }
                Err(error) => {
                    event["restore_error"] = json!(error.to_string());
                    Some(format!("preflight_control_source_restore_failed:{error}"))
                }
            }
        }
        Err(error) => {
            event["observation_error"] = json!(error.to_string());
            Some(format!(
                "preflight_control_source_observation_failed:{error}"
            ))
        }
    };
    crate::eval_events::emit(config.eval_events_path.as_deref(), event);
    failure.map_or(outcome, |reason| RecoveryPreflight::Unavailable { reason })
}

fn outcome_kind(outcome: &RecoveryPreflight) -> &'static str {
    match outcome {
        RecoveryPreflight::CurrentSuccess { .. } => "pass",
        RecoveryPreflight::Failed { .. } => "fail",
        RecoveryPreflight::Unavailable { .. } => "unavailable",
        RecoveryPreflight::VerificationInconsistency { .. } => "verification_inconsistency",
        RecoveryPreflight::NotConfigured => "not_configured",
    }
}

fn outcome_reason(outcome: &RecoveryPreflight) -> Option<&str> {
    match outcome {
        RecoveryPreflight::CurrentSuccess { reason }
        | RecoveryPreflight::Failed { reason, .. }
        | RecoveryPreflight::Unavailable { reason }
        | RecoveryPreflight::VerificationInconsistency { reason } => Some(reason),
        RecoveryPreflight::NotConfigured => None,
    }
}
