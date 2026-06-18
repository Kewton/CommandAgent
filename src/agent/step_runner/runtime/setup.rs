use crate::agent::events::bounded_event_text;
use crate::agent::minimal_loop::config::DependencySetupPolicy;
use crate::agent::step_runner::verify::VerificationFailure;
use crate::tools::bash::{BashPolicy, CommandClass, enforce_bash_policy};
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const SETUP_LOG_EXCERPT_BYTES: u64 = 4_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SetupCommand {
    NpmInstall,
    NpmCi,
    PnpmInstall,
}

impl SetupCommand {
    pub(super) fn as_shell_command(self) -> &'static str {
        match self {
            Self::NpmInstall => "npm install",
            Self::NpmCi => "npm ci",
            Self::PnpmInstall => "pnpm install",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum SetupSelectionError {
    UnsupportedVerifier(String),
    UnsupportedYarnLock,
    AmbiguousLockfiles,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum DependencySetupDisposition {
    NotApplicable,
    Blocked(String),
    Attempt(SetupCommand),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum SetupRunStatus {
    Success,
    CommandFailed {
        status: i32,
        stderr_excerpt: String,
    },
    TimedOut {
        timeout_secs: u64,
        stderr_excerpt: String,
    },
    Blocked {
        message: String,
    },
}

impl SetupRunStatus {
    pub(super) fn ok(&self) -> bool {
        matches!(self, Self::Success)
    }

    pub(super) fn label(&self) -> String {
        match self {
            Self::Success => "success".to_string(),
            Self::CommandFailed { status, .. } => format!("command_failed:{status}"),
            Self::TimedOut { timeout_secs, .. } => format!("timeout:{timeout_secs}s"),
            Self::Blocked { .. } => "blocked".to_string(),
        }
    }
}

pub(super) trait DependencySetupRunner {
    fn run_setup(
        &self,
        cwd: &Path,
        command: SetupCommand,
        policy: DependencySetupPolicy,
    ) -> SetupRunStatus;
}

#[derive(Debug, Default)]
pub(super) struct ShellDependencySetupRunner;

impl DependencySetupRunner for ShellDependencySetupRunner {
    fn run_setup(
        &self,
        cwd: &Path,
        command: SetupCommand,
        policy: DependencySetupPolicy,
    ) -> SetupRunStatus {
        let shell_command = command.as_shell_command();
        let decision = enforce_bash_policy(
            shell_command,
            cwd,
            BashPolicy::dependency_recovery(policy.offline, policy.auto_approve),
        );
        if !decision.allowed {
            return SetupRunStatus::Blocked {
                message: decision
                    .message
                    .unwrap_or_else(|| format!("dependency setup blocked as {:?}", decision.class)),
            };
        }
        if decision.class != CommandClass::EnvSetup {
            return SetupRunStatus::Blocked {
                message: "setup runner only accepts EnvSetup commands".to_string(),
            };
        }

        let log_dir = cwd.join(".commandagent/setup");
        if let Err(err) = fs::create_dir_all(&log_dir) {
            return SetupRunStatus::CommandFailed {
                status: 1,
                stderr_excerpt: format!("failed to create setup log dir: {err}"),
            };
        }

        let log_prefix = setup_log_prefix(command);
        let stdout_path = log_dir.join(format!("{log_prefix}.stdout.log"));
        let stderr_path = log_dir.join(format!("{log_prefix}.stderr.log"));
        let stdout = match File::create(&stdout_path) {
            Ok(file) => file,
            Err(err) => {
                return SetupRunStatus::CommandFailed {
                    status: 1,
                    stderr_excerpt: format!("failed to create setup stdout log: {err}"),
                };
            }
        };
        let stderr = match File::create(&stderr_path) {
            Ok(file) => file,
            Err(err) => {
                return SetupRunStatus::CommandFailed {
                    status: 1,
                    stderr_excerpt: format!("failed to create setup stderr log: {err}"),
                };
            }
        };

        let mut child = match Command::new("sh")
            .arg("-lc")
            .arg(shell_command)
            .current_dir(cwd)
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .spawn()
        {
            Ok(child) => child,
            Err(err) => {
                return SetupRunStatus::CommandFailed {
                    status: 1,
                    stderr_excerpt: format!("failed to start setup command: {err}"),
                };
            }
        };

        let timeout = Duration::from_secs(policy.timeout_secs);
        let started = Instant::now();
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    let stderr_excerpt = read_tail_excerpt(&stderr_path);
                    if status.success() {
                        return SetupRunStatus::Success;
                    }
                    return SetupRunStatus::CommandFailed {
                        status: status.code().unwrap_or(1),
                        stderr_excerpt,
                    };
                }
                Ok(None) => {
                    if started.elapsed() >= timeout {
                        let _ = child.kill();
                        let _ = child.wait();
                        return SetupRunStatus::TimedOut {
                            timeout_secs: policy.timeout_secs,
                            stderr_excerpt: read_tail_excerpt(&stderr_path),
                        };
                    }
                    thread::sleep(Duration::from_millis(100));
                }
                Err(err) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return SetupRunStatus::CommandFailed {
                        status: 1,
                        stderr_excerpt: format!("failed to poll setup command: {err}"),
                    };
                }
            }
        }
    }
}

pub(super) fn dependency_setup_disposition(
    cwd: &Path,
    step_id: &str,
    failures: &[VerificationFailure],
    missing_expected_paths: &[String],
    policy: DependencySetupPolicy,
    already_attempted: bool,
) -> DependencySetupDisposition {
    if failures.is_empty()
        || !missing_expected_paths.is_empty()
        || !failures
            .iter()
            .all(|failure| failure.reason == "dependency_missing")
    {
        return DependencySetupDisposition::NotApplicable;
    }

    if already_attempted {
        return DependencySetupDisposition::Blocked(dependency_missing_blocker_message(
            step_id,
            failures,
            "Dependency setup was already attempted once, but the verifier still reports missing dependencies.",
        ));
    }
    if policy.offline {
        return DependencySetupDisposition::Blocked(dependency_missing_blocker_message(
            step_id,
            failures,
            "Dependency setup was not run because --offline is enabled.",
        ));
    }
    if !policy.auto_approve {
        return DependencySetupDisposition::Blocked(dependency_missing_blocker_message(
            step_id,
            failures,
            "Dependency setup requires explicit approval. Rerun with --yes to allow one bounded setup attempt.",
        ));
    }

    match select_setup_command(cwd, failures) {
        Ok(Some(command)) => DependencySetupDisposition::Attempt(command),
        Ok(None) => DependencySetupDisposition::NotApplicable,
        Err(err) => DependencySetupDisposition::Blocked(dependency_missing_blocker_message(
            step_id,
            failures,
            &selection_error_message(&err),
        )),
    }
}

pub(super) fn select_setup_command(
    cwd: &Path,
    failures: &[VerificationFailure],
) -> Result<Option<SetupCommand>, SetupSelectionError> {
    if failures.is_empty()
        || !failures
            .iter()
            .all(|failure| failure.reason == "dependency_missing")
    {
        return Ok(None);
    }

    for failure in failures {
        if failure.command.trim() != "npm run build" {
            return Err(SetupSelectionError::UnsupportedVerifier(
                failure.command.clone(),
            ));
        }
    }

    let package_lock = cwd.join("package-lock.json").exists();
    let pnpm_lock = cwd.join("pnpm-lock.yaml").exists();
    if cwd.join("yarn.lock").exists() {
        return Err(SetupSelectionError::UnsupportedYarnLock);
    }
    if package_lock && pnpm_lock {
        return Err(SetupSelectionError::AmbiguousLockfiles);
    }
    if package_lock {
        return Ok(Some(SetupCommand::NpmCi));
    }
    if pnpm_lock {
        return Ok(Some(SetupCommand::PnpmInstall));
    }
    Ok(Some(SetupCommand::NpmInstall))
}

pub(super) fn setup_failed_blocker_message(
    step_id: &str,
    command: SetupCommand,
    status: &SetupRunStatus,
) -> String {
    match status {
        SetupRunStatus::Success => {
            format!(
                "dependency setup {} completed, but step {} still failed for an unknown reason.",
                command.as_shell_command(),
                step_id
            )
        }
        SetupRunStatus::CommandFailed {
            status,
            stderr_excerpt,
        } => format!(
            "dependency_setup_failed: step {step_id} setup command `{}` exited with status {status}.\n\nstderr excerpt:\n{}",
            command.as_shell_command(),
            empty_excerpt(stderr_excerpt)
        ),
        SetupRunStatus::TimedOut {
            timeout_secs,
            stderr_excerpt,
        } => format!(
            "dependency_setup_timeout: step {step_id} setup command `{}` exceeded {timeout_secs}s.\n\nstderr excerpt:\n{}",
            command.as_shell_command(),
            empty_excerpt(stderr_excerpt)
        ),
        SetupRunStatus::Blocked { message } => format!(
            "dependency_setup_blocked: step {step_id} setup command `{}` was blocked.\n\n{message}",
            command.as_shell_command()
        ),
    }
}

pub(super) fn dependency_missing_blocker_message(
    step_id: &str,
    failures: &[VerificationFailure],
    reason: &str,
) -> String {
    let mut commands = Vec::new();
    let mut diagnostics = Vec::new();
    for failure in failures {
        if !commands.contains(&failure.command) {
            commands.push(failure.command.clone());
        }
        if !failure.diagnostic_excerpt.trim().is_empty() {
            diagnostics.push(failure.diagnostic_excerpt.trim().to_string());
        }
    }

    let mut message = format!(
        "dependency_missing: step {step_id} cannot be repaired by editing files.\n\n\
This is an environment/setup blocker, not a code repair failure. {reason}"
    );
    if !diagnostics.is_empty() {
        message.push_str("\n\nVerifier evidence:\n");
        message.push_str(&diagnostics.join("\n"));
    }
    message.push_str("\n\nRun dependency setup manually, for example:\n  npm install");
    if commands.is_empty() {
        message.push_str("\n\nThen rerun the original verifier.");
    } else {
        message.push_str("\n\nThen rerun:\n");
        for command in commands {
            message.push_str("  ");
            message.push_str(&command);
            message.push('\n');
        }
        if message.ends_with('\n') {
            message.pop();
        }
    }
    message
        .push_str("\n\nCommandAgent did not create a repair prompt because this blocker requires explicit setup.");
    message
}

fn selection_error_message(err: &SetupSelectionError) -> String {
    match err {
        SetupSelectionError::UnsupportedVerifier(command) => format!(
            "Dependency setup recovery only supports npm build verifiers in this slice; unsupported verifier: `{}`.",
            bounded_event_text(command)
        ),
        SetupSelectionError::UnsupportedYarnLock => {
            "Dependency setup recovery does not support yarn.lock in this slice.".to_string()
        }
        SetupSelectionError::AmbiguousLockfiles => {
            "Dependency setup recovery found both package-lock.json and pnpm-lock.yaml, so the setup command is ambiguous.".to_string()
        }
    }
}

fn setup_log_prefix(command: SetupCommand) -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    let name = command.as_shell_command().replace(' ', "-");
    format!("{millis}-{name}")
}

fn read_tail_excerpt(path: &Path) -> String {
    let Ok(mut file) = File::open(path) else {
        return String::new();
    };
    let len = file.metadata().map(|meta| meta.len()).unwrap_or_default();
    let start = len.saturating_sub(SETUP_LOG_EXCERPT_BYTES);
    if file.seek(SeekFrom::Start(start)).is_err() {
        return String::new();
    }
    let mut bytes = Vec::new();
    if file.read_to_end(&mut bytes).is_err() {
        return String::new();
    }
    String::from_utf8_lossy(&bytes).trim().to_string()
}

fn empty_excerpt(value: &str) -> &str {
    if value.trim().is_empty() {
        "(no stderr output captured; full logs are under .commandagent/setup/)"
    } else {
        value.trim()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn selects_npm_install_without_lockfile() {
        let root = temp_workspace("no-lock");
        let command = select_setup_command(&root, &[dependency_missing("npm run build")])
            .unwrap()
            .unwrap();

        assert_eq!(command, SetupCommand::NpmInstall);
    }

    #[test]
    fn selects_npm_ci_with_package_lock() {
        let root = temp_workspace("package-lock");
        fs::write(root.join("package-lock.json"), "{}").unwrap();

        let command = select_setup_command(&root, &[dependency_missing("npm run build")])
            .unwrap()
            .unwrap();

        assert_eq!(command, SetupCommand::NpmCi);
    }

    #[test]
    fn selects_pnpm_install_with_pnpm_lock() {
        let root = temp_workspace("pnpm-lock");
        fs::write(root.join("pnpm-lock.yaml"), "").unwrap();

        let command = select_setup_command(&root, &[dependency_missing("npm run build")])
            .unwrap()
            .unwrap();

        assert_eq!(command, SetupCommand::PnpmInstall);
    }

    #[test]
    fn rejects_yarn_lock() {
        let root = temp_workspace("yarn-lock");
        fs::write(root.join("yarn.lock"), "").unwrap();

        let err = select_setup_command(&root, &[dependency_missing("npm run build")]).unwrap_err();

        assert_eq!(err, SetupSelectionError::UnsupportedYarnLock);
    }

    #[test]
    fn rejects_ambiguous_npm_and_pnpm_lockfiles() {
        let root = temp_workspace("ambiguous");
        fs::write(root.join("package-lock.json"), "{}").unwrap();
        fs::write(root.join("pnpm-lock.yaml"), "").unwrap();

        let err = select_setup_command(&root, &[dependency_missing("npm run build")]).unwrap_err();

        assert_eq!(err, SetupSelectionError::AmbiguousLockfiles);
    }

    #[test]
    fn does_not_select_for_non_dependency_missing() {
        let root = temp_workspace("non-dependency");
        let command = select_setup_command(
            &root,
            &[VerificationFailure {
                command: "npm run build".to_string(),
                reason: "command_failed:1".to_string(),
                stdout_excerpt: String::new(),
                stderr_excerpt: String::new(),
                diagnostic_excerpt: String::new(),
                source_excerpt: None,
            }],
        )
        .unwrap();

        assert_eq!(command, None);
    }

    #[test]
    fn rejects_non_npm_build_dependency_missing() {
        let root = temp_workspace("non-npm-build");
        let err = select_setup_command(&root, &[dependency_missing("cargo test")]).unwrap_err();

        assert_eq!(
            err,
            SetupSelectionError::UnsupportedVerifier("cargo test".to_string())
        );
    }

    #[test]
    fn production_runner_blocks_unapproved_setup_without_running_npm() {
        let root = temp_workspace("runner-block");
        let status = ShellDependencySetupRunner.run_setup(
            &root,
            SetupCommand::NpmInstall,
            DependencySetupPolicy::default(),
        );

        assert!(matches!(status, SetupRunStatus::Blocked { .. }));
        assert!(!root.join(".commandagent/setup").exists());
    }

    fn dependency_missing(command: &str) -> VerificationFailure {
        VerificationFailure {
            command: command.to_string(),
            reason: "dependency_missing".to_string(),
            stdout_excerpt: String::new(),
            stderr_excerpt: String::new(),
            diagnostic_excerpt: "missing next".to_string(),
            source_excerpt: None,
        }
    }

    fn temp_workspace(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "commandagent-setup-{}-{}",
            name,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }
}
