use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn execute_split_runtime_bash<F, G>(
    segments: &[RuntimeNormalizedCommandSegment],
    root: &Path,
    profile: &str,
    setup_authority: NodeDependencySetupAuthority,
    offline: bool,
    eval_events_path: Option<&Path>,
    is_cancelled: F,
    is_force_cancelled: G,
) -> anyhow::Result<String>
where
    F: Fn() -> bool,
    G: Fn() -> bool,
{
    let mut out = Vec::new();
    let mut and_chain_failed = false;
    let mut first_failure: Option<(usize, String, BashOutcomeKind)> = None;
    for (index, segment) in segments.iter().enumerate() {
        if segment.connector == RuntimeCommandConnector::Always {
            and_chain_failed = false;
        }
        if segment.connector == RuntimeCommandConnector::AndThen && and_chain_failed {
            out.push(format!(
                "segment {} skipped by && short-circuit: {}",
                index + 1,
                segment.command.as_str()
            ));
            continue;
        }
        let command = segment.command.as_str();
        match &segment.command {
            RuntimeNormalizedCommand::DependencyInstall { family, .. } => {
                let setup = run_runtime_verify_install_substitution(
                    root,
                    profile,
                    command,
                    *family,
                    setup_authority,
                    offline,
                    eval_events_path,
                );
                let passed = runtime_dependency_setup_allows_verify_continuation(&setup);
                out.push(format!(
                    "segment {} install substituted: {}\nsetup_status: {}\nfeedback: dependency installs are owned by the runtime; verify with the build/test command alone.",
                    index + 1,
                    command,
                    setup.status.as_str()
                ));
                if passed {
                    and_chain_failed = false;
                } else {
                    and_chain_failed = true;
                    if first_failure.is_none() {
                        first_failure = Some((
                            index + 1,
                            command.to_string(),
                            BashOutcomeKind::CommandFailed,
                        ));
                    }
                }
            }
            RuntimeNormalizedCommand::Verify(verify_command) => {
                let command = verify_command.as_str();
                let outcome = crate::tools::bash::run_structured_cancel_and_force(
                    command,
                    root,
                    offline,
                    Duration::from_secs(180),
                    &is_cancelled,
                    &is_force_cancelled,
                )?;
                match outcome.kind {
                    BashOutcomeKind::Blocked => bail!("{}", outcome.summary),
                    BashOutcomeKind::Timeout => bail!(
                        "command_timeout: {command}\n{}",
                        crate::tools::bash::format_outcome(&outcome)
                    ),
                    BashOutcomeKind::Cancelled => bail!(
                        "command_aborted_by_user: interrupted by user: {command}\n{}",
                        crate::tools::bash::format_outcome(&outcome)
                    ),
                    BashOutcomeKind::Success | BashOutcomeKind::CommandFailed => {}
                }
                out.push(format!(
                    "segment {} command: {}\n{}",
                    index + 1,
                    command,
                    crate::tools::bash::format_outcome(&outcome)
                ));
                if outcome.kind == BashOutcomeKind::Success {
                    and_chain_failed = false;
                } else {
                    and_chain_failed = true;
                    if first_failure.is_none() {
                        first_failure = Some((index + 1, command.to_string(), outcome.kind));
                    }
                }
            }
        }
    }
    let combined = if let Some((index, command, kind)) = first_failure {
        format!("combined_outcome: {kind:?}\nfailing_segment: {index} `{command}`")
    } else {
        "combined_outcome: Success".to_string()
    };
    Ok(format!("{combined}\n{}", out.join("\n\n")))
}

fn run_runtime_verify_install_substitution(
    root: &Path,
    profile: &str,
    command: &str,
    family: VerifyInstallCommandFamily,
    setup_authority: NodeDependencySetupAuthority,
    offline: bool,
    eval_events_path: Option<&Path>,
) -> dependency_setup::NodeDependencySetupObservation {
    let requirement = match family {
        VerifyInstallCommandFamily::Python => {
            dependency_setup::requirement_for_python_cli_dependencies(
                root,
                Some("python-cli"),
                "verify_segment dependency reconciliation",
                setup_authority,
            )
        }
        VerifyInstallCommandFamily::Node => {
            let canonical = profile.trim().to_ascii_lowercase();
            if canonical == "nextjs"
                && dependency_setup::package_json_declares_dependencies(root)
                && !dependency_setup::next_build_dependencies_ready(root)
            {
                dependency_setup::requirement_for_next_build(
                    root,
                    Some("nextjs"),
                    "verify_segment dependency reconciliation",
                    setup_authority,
                )
            } else {
                dependency_setup::requirement_for_node_declared_dependencies(
                    root,
                    Some(profile),
                    "verify_segment dependency reconciliation",
                    setup_authority,
                )
            }
        }
    };
    let setup = dependency_setup::run_node_dependency_setup_with_program_and_offline(
        root,
        &requirement,
        Path::new("npm"),
        offline,
    );
    eval_events::emit(
        eval_events_path,
        json!({
            "event": "verify_install_substituted",
            "trigger": "verify_segment",
            "command": eval_events::body_snippet(command),
            "family": family.as_str(),
            "setup_kind": setup.setup_kind.as_str(),
            "setup_status": setup.status.as_str(),
            "setup_attempted": setup.attempted,
            "setup_authority": setup.authority.as_str(),
            "feedback": "dependency installs are owned by the runtime; verify with the build/test command alone.",
        }),
    );
    setup
}

fn runtime_dependency_setup_allows_verify_continuation(
    setup: &dependency_setup::NodeDependencySetupObservation,
) -> bool {
    matches!(
        setup.status,
        NodeDependencySetupStatus::Passed | NodeDependencySetupStatus::NotRequired
    ) || setup.primary_reason.contains("already present")
        || setup.primary_reason.contains("has no dependency table")
        || setup
            .primary_reason
            .contains("has no project.dependencies table")
}
