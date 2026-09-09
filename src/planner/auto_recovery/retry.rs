//! Continuation eligibility is separate from rejecting an unverified treatment.
use super::*;
use crate::minimal_loop::completion::CompletionContract;
use crate::planner::recovery_snapshot::{self, RecoveryBoundarySnapshot};
use crate::planner::repair::{self, RecoveryHandoff};
use crate::tools::path_guard::{resolve_optional_existing, validate_workspace_relative};
use crate::tools::workspace_policy::{WorkspacePolicy, ensure_tool_path_allowed};

pub(super) fn announce_continuation(config: &Config, used: u8, candidate: &RecoveryCandidate) {
    let path = repair::workspace_relative_handoff_path(&candidate.path);
    // Published only after successful control retention and continuation
    // validation, so terminal projections select a safe control-owned plan.
    crate::eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "recovery_continuation_prepared",
            "recovery_plan_auto_run_current": used,
            "recovery_plan_auto_runs_used": used,
            "recovery_plan_auto_runs": config.recovery_plan_auto_runs,
            "recovery_ultra_plan_path": path,
            "suggested_recovery_yaml_command": format!("/run-ultra-plan {path}"),
            "recovery_handoff_kind": candidate.handoff.failure_kind,
            "reason": "validated_continuation_from_retained_control",
        }),
    );
}

pub(super) fn observation_identity(
    report: &crate::planner::verify::VerificationReport,
    root: &Path,
) -> Vec<u8> {
    let stable = |values: Vec<String>| values.into_iter().collect::<BTreeSet<_>>();
    let semantic = |value: &str| semantic_diagnostic(root, value);
    json!({
        "missing_paths": stable(report.missing_paths.iter().map(|p| semantic(p)).collect()),
        "failed_commands": stable(report.command_failures.iter().map(|f| format!("{}\n{}", semantic(&f.command), semantic(&f.reason))).collect()),
        "compile_diagnostics": stable(report.compile_errors.iter().map(|e| format!("{}:{}:{}:{}", semantic(&e.path), e.line, e.column, semantic(&e.message))).collect()),
        "profile_failures": stable(report.profile_failures.iter().map(|reason| semantic(reason)).collect()),
    }).to_string().into_bytes()
}

fn semantic_diagnostic(root: &Path, raw: &str) -> String {
    // Diagnostic cwd labels may end in a colon; display-only shell path
    // normalization deliberately does not rewrite that form.
    let cwd_label = format!("{}:", root.display());
    let text = raw
        .split_inclusive(char::is_whitespace)
        .map(|token| {
            token
                .strip_prefix(&cwd_label)
                .map(|suffix| format!(".:{suffix}"))
                .unwrap_or_else(|| token.to_string())
        })
        .collect::<String>();
    let text = repair::display_text(Some(root), &text);
    // Strip only run_checked's known timing header, before its stdout payload.
    // Never discard compiler codes, exit status, profile reasons or command
    // output, and never guess that an arbitrary number in output is a clock.
    let Some((header, streams)) = text.split_once("\nstdout:\n") else {
        return text;
    };
    if !header.starts_with("command failed: ") || !header.contains("\noutcome: ") {
        return text;
    }
    let header = header
        .lines()
        .filter(|line| {
            line.strip_prefix("elapsed_ms: ")
                .is_none_or(|value| value.parse::<u128>().is_err())
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!("{header}\nstdout:\n{streams}")
}

pub(super) fn readiness_failure(
    observation: &crate::minimal_loop::browser_probe::BrowserReadinessObservation,
    root: &Path,
    reason: String,
) -> RecoveryPreflight {
    // Startup/port/timeout/environment failures do not prove a product defect.
    // Only typed compile diagnostics or an actual failing HTTP response qualify.
    let product_failure = observation.status == "failed"
        && ((observation.failure_kind == "build_verifier_failed"
            && !observation.compile_errors.is_empty())
            || (observation.http_status.is_some_and(|status| status >= 400)
                && observation.failure_kind
                    == format!("http_{}", observation.http_status.unwrap())));
    if product_failure {
        let diagnostics = observation
            .compile_errors
            .iter()
            .map(|error| {
                semantic_diagnostic(
                    root,
                    &format!(
                        "{}:{}:{} {}",
                        error.path, error.line, error.column, error.message
                    ),
                )
            })
            .collect::<BTreeSet<_>>();
        let identity = json!({
            "kind": observation.failure_kind,
            "compile": diagnostics,
        })
        .to_string()
        .into_bytes();
        let reason = if diagnostics.is_empty() {
            reason
        } else {
            format!(
                "{reason}\n{}",
                diagnostics.into_iter().collect::<Vec<_>>().join("\n")
            )
        };
        RecoveryPreflight::Failed { identity, reason }
    } else {
        RecoveryPreflight::Unavailable { reason }
    }
}

pub(super) fn check_boundary(
    config: &Config,
    treatment: &Config,
    snapshot: &RecoveryBoundarySnapshot,
    identity: &RecoveryObserverIdentity,
) -> Result<(), String> {
    if recovery_observer_identity(config).as_ref() != Some(identity)
        || recovery_observer_identity(treatment).as_ref() != Some(identity)
    {
        return Err("recovery_observer_authority_changed".into());
    }
    let check = || -> anyhow::Result<()> {
        anyhow::ensure!(
            recovery_snapshot::current_source_sha256(&config.workspace_root)?
                == snapshot.snapshot_sha256,
            "recovery_control_drift"
        );
        let contract = CompletionContract::load_for_config(config)?;
        if let Some(contract) = contract {
            let delta = recovery_snapshot::treatment_delta(
                &config.workspace_root,
                snapshot,
                &treatment.workspace_root,
            )?;
            for bucket in [&delta.product, &delta.runtime_evidence] {
                for path in bucket
                    .changed_paths
                    .iter()
                    .chain(&bucket.added_paths)
                    .chain(&bucket.removed_paths)
                {
                    anyhow::ensure!(
                        !is_protected_repair_path(path, &contract.protected_paths),
                        "recovery_protected_path_changed:{path}"
                    );
                }
            }
        }
        Ok(())
    };
    check().map_err(|error| error.to_string())
}

fn implementation_failure(kind: &str) -> bool {
    // Positive list: a generic phase/provider error does not establish that a
    // model can repair it. Never infer eligibility from diagnostic prose.
    matches!(
        kind,
        "implementation_compile_error"
            | "compile_error"
            | "verification_failed"
            | "bounded_repair_exhausted"
            | "compile_repair_no_source_change"
            | "verify_repair_progress_unchanged"
            | "model_stagnation:read_only_loop"
            | "model_stagnation:no_progress_recorded"
    )
}

pub(super) fn reject_execution(
    config: &Config,
    treatment: &Config,
    used: u8,
    snapshot: RecoveryBoundarySnapshot,
    mut outcome: AttemptOutcome,
) -> AttemptOutcome {
    let child = match outcome.failure.take() {
        Some(AttemptFailure::Recoverable(child))
            if implementation_failure(&child.handoff.failure_kind)
                && child
                    .handoff
                    .failure_evidence
                    .iter()
                    .any(|line| !line.trim().is_empty()) =>
        {
            Some(child)
        }
        failure => {
            outcome.failure = failure;
            None
        }
    };
    retain_control_then(
        config,
        used,
        snapshot,
        outcome,
        "recovery_execution_failed",
        false,
        || {
            let Some(child) = child else { return Ok(None) };
            // Validate the actual child first. Re-rendering must not erase a review
            // requirement, corrupt YAML, confinement violation or resume drift.
            prepare_candidate(treatment, &child)?;
            rebuild(config, treatment, *child).map(Some)
        },
    )
}

pub(super) fn reject_verification(
    config: &Config,
    treatment: &Config,
    used: u8,
    snapshot: RecoveryBoundarySnapshot,
    candidate: &RecoveryCandidate,
    outcome: AttemptOutcome,
    observation: (&str, Vec<u8>),
) -> AttemptOutcome {
    let (reason, identity) = observation;
    retain_control_then(config, used, snapshot, outcome, reason, true, || {
        let mut next = candidate.clone();
        next.retry_identity = Some(identity);
        next.handoff.failure_kind = "verification_failed".into();
        next.handoff.failed_phase = Some("post-recovery-verification".into());
        next.handoff.failed_step = None;
        // Keep the latest readable diagnosis without growing historical prose.
        // The separate semantic identity handles observation timing/path noise.
        next.handoff.failure_evidence = vec![format!(
            "Registered post-Recovery observation failed in a rejected treatment: {reason}"
        )];
        next.inspection_context = crate::planner::recovery_inspection::load(treatment)
            .map_err(|_| CandidateStop::TreatmentContractBindFailed)?;
        rebuild(config, treatment, next).map(Some)
    })
}

fn rebuild(
    config: &Config,
    treatment: &Config,
    mut candidate: RecoveryCandidate,
) -> Result<RecoveryCandidate, CandidateStop> {
    let contract = CompletionContract::load_for_config(config)
        .map_err(|_| CandidateStop::ContractCommandBindFailed)?
        .ok_or(CandidateStop::ContractCommandBindFailed)?;
    let root = &treatment.workspace_root;
    let mut handoff = candidate.handoff.clone();
    handoff.repair_targets = handoff
        .repair_targets
        .iter()
        .map(|path| control_target(config, treatment, &contract, path))
        .collect::<Result<_, _>>()?;
    handoff.missing_paths = handoff
        .missing_paths
        .iter()
        .map(|path| control_target(config, treatment, &contract, path))
        .collect::<Result<_, _>>()?;
    handoff.failure_evidence = handoff
        .failure_evidence
        .iter()
        .map(|line| repair::display_text(Some(root), line))
        .collect();
    // A rejected treatment's completed artifacts and checks are not authority
    // for the next workspace. All phases start again from retained control.
    handoff.changed_paths.clear();
    handoff.verify_commands = contract.verify_commands.clone();
    handoff.original_goal = contract
        .goal
        .clone()
        .ok_or(CandidateStop::ContractHandoffBindFailed)?;
    handoff.profile = contract
        .profile
        .clone()
        .unwrap_or_else(|| config.profile.clone());
    candidate.handoff = handoff;
    candidate.verify_command_source = "failure_handoff".into();
    save(config, &mut candidate)?;
    let candidate =
        bind_candidate_verify_commands(config, candidate, "retained control requires repair")?;
    prepare_candidate(config, &candidate)?;
    Ok(candidate)
}

fn save(config: &Config, candidate: &mut RecoveryCandidate) -> Result<(), CandidateStop> {
    let handoff: &RecoveryHandoff = &candidate.handoff;
    candidate.plan =
        repair::build_recovery_ultra_plan_at_root(Some(&config.workspace_root), handoff);
    candidate.path =
        repair::save_recovery_ultra_plan(&config.workspace_root, "continuation", handoff)
            .map_err(|_| CandidateStop::ContractHandoffBindFailed)?;
    Ok(())
}

fn control_target(
    config: &Config,
    treatment: &Config,
    contract: &CompletionContract,
    raw: &str,
) -> Result<String, CandidateStop> {
    let path = Path::new(raw);
    let relative = if path.is_absolute() {
        path.strip_prefix(&treatment.workspace_root)
            .map_err(|_| CandidateStop::PathEscape)?
    } else {
        path
    };
    let raw = relative.to_string_lossy();
    validate_workspace_relative(&raw).map_err(|_| CandidateStop::PathEscape)?;
    let relative = relative.components().collect::<PathBuf>();
    let raw = relative.to_string_lossy().replace('\\', "/");
    for root in [&config.workspace_root, &treatment.workspace_root] {
        let resolved =
            resolve_optional_existing(root, &raw).map_err(|_| CandidateStop::PathEscape)?;
        ensure_tool_path_allowed(root, &resolved, WorkspacePolicy::NormalTask)
            .map_err(|_| CandidateStop::ResumeSafetyRejected)?;
        let canonical_root = root.canonicalize().map_err(|_| CandidateStop::PathEscape)?;
        let resolved_relative = resolved
            .strip_prefix(&canonical_root)
            .map_err(|_| CandidateStop::PathEscape)?;
        if is_protected_repair_path(&raw, &contract.protected_paths)
            || is_protected_repair_path(
                &resolved_relative.to_string_lossy(),
                &contract.protected_paths,
            )
            || relative
                .components()
                .chain(resolved_relative.components())
                .any(|part| {
                    matches!(
                        part.as_os_str().to_str(),
                        Some(".commandagent" | ".anvil" | ".env")
                    )
                })
        {
            return Err(CandidateStop::ResumeSafetyRejected);
        }
    }
    Ok(raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn issue458_identity_retains_changed_command_compiler_and_profile_semantics() {
        use crate::planner::verify::{CommandFailure, VerificationReport};
        let root1 = Path::new("/fixture/recovery-treatments/attempt-1/workspace/observation");
        let root2 = Path::new("/fixture/recovery-treatments/attempt-2/workspace/observation");
        let mut first = VerificationReport::pass();
        first.command_failures.push(CommandFailure {
            command:"python3 check.py".into(),
            reason:format!("command failed: python3 check.py\noutcome: CommandFailed\nstatus: exit status: 1\nelapsed_ms: 5\nsummary: assertion failed\nstdout:\n{}: wrong response shape\nstderr:\n", root1.display())
        });
        first.profile_failures = vec!["required_response_field:members".into()];
        first
            .compile_errors
            .push(crate::minimal_loop::build_verifier::CompileError {
                path: root1.join("src/app.ts").to_string_lossy().into(),
                line: 3,
                column: 2,
                message: "TS2322: Type mismatch".into(),
                excerpt: String::new(),
                symbol: None,
                route_bound: None,
            });
        let mut second = first.clone();
        second.command_failures[0].reason = second.command_failures[0]
            .reason
            .replace(
                &root1.to_string_lossy().to_string(),
                &root2.to_string_lossy(),
            )
            .replace("elapsed_ms: 5", "elapsed_ms: 231");
        second.compile_errors[0].path = root2.join("src/app.ts").to_string_lossy().into();
        let initial = observation_identity(&first, root1);
        assert_eq!(initial, observation_identity(&second, root2));
        for field in ["command", "compiler", "profile"] {
            let mut changed = second.clone();
            match field {
                "command" => {
                    changed.command_failures[0].reason = changed.command_failures[0]
                        .reason
                        .replace("wrong response shape", "wrong assignment value")
                }
                "compiler" => changed.compile_errors[0].message = "TS2345: Invalid argument".into(),
                "profile" => {
                    changed.profile_failures = vec!["required_response_field:assignee".into()]
                }
                _ => unreachable!(),
            }
            assert_ne!(initial, observation_identity(&changed, root2), "{field}");
        }
    }

    #[test]
    fn issue458_direct_and_aliased_env_are_denied_in_control_and_treatment() {
        use clap::Parser;
        use std::os::unix::fs::symlink;
        for alias_in_control in [false, true] {
            let control = tempfile::tempdir().unwrap();
            let treatment_root = tempfile::tempdir().unwrap();
            let mut config =
                Config::from_cli(crate::cli::Cli::parse_from(["commandagent", "--ux-demo"]))
                    .unwrap();
            config.workspace_root = control.path().to_path_buf();
            let mut treatment = config.clone();
            treatment.workspace_root = treatment_root.path().to_path_buf();
            let contract: CompletionContract = serde_json::from_value(json!({
                "goal":"Create ready.txt", "profile":"generic", "protected_paths":[],
                "required_paths":["settings.ts"], "verify_commands":["test -f settings.ts"]
            }))
            .unwrap();
            for root in [control.path(), treatment_root.path()] {
                std::fs::write(root.join(".env"), "test-only fixture\n").unwrap();
            }
            let (alias_root, regular_root) = if alias_in_control {
                (control.path(), treatment_root.path())
            } else {
                (treatment_root.path(), control.path())
            };
            symlink(alias_root.join(".env"), alias_root.join("settings.ts")).unwrap();
            std::fs::write(regular_root.join("settings.ts"), "ordinary source\n").unwrap();
            for target in [".env", "settings.ts"] {
                assert_eq!(
                    control_target(&config, &treatment, &contract, target),
                    Err(CandidateStop::ResumeSafetyRejected),
                    "control alias={alias_in_control}, target={target}"
                );
            }
            assert_eq!(
                std::fs::read_to_string(alias_root.join(".env")).unwrap(),
                "test-only fixture\n"
            );
            assert_eq!(
                control_target(&config, &treatment, &contract, "../outside"),
                Err(CandidateStop::PathEscape)
            );
        }
    }
}
