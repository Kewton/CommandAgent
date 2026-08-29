use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use anyhow::{Context, bail};
use serde::Deserialize;
use serde_json::json;

use crate::bounded_process::{self, BoundedProcessOutcomeKind};
use crate::config::Config;
use crate::minimal_loop::verifier_env;
use crate::planner::profile::{ProfileBehaviorProbeReport, ProfileId};
use crate::tools::path_guard::validate_workspace_relative;

const FIXED_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_OBSERVATIONS: usize = 16;
const MAX_ARG_COUNT: usize = 32;
const MAX_ARG_BYTES: usize = 8_192;
const MAX_EXPECTED_BYTES: usize = 24_000;

#[derive(Debug, Deserialize)]
struct ContractProjection {
    #[serde(default)]
    command_observations: Vec<CommandObservation>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CommandObservation {
    argv: Vec<String>,
    expected_exit_code: i32,
    expected_stdout: String,
}

#[derive(Debug)]
struct ObservationExecution {
    exit_code: Option<i32>,
    timed_out: bool,
    stdout: String,
    stderr: String,
    reasons: Vec<String>,
}

impl ObservationExecution {
    fn passed(&self) -> bool {
        self.reasons.is_empty()
    }
}

pub(crate) fn run_if_registered(
    config: &Config,
    profile_id: &ProfileId,
) -> anyhow::Result<Option<ProfileBehaviorProbeReport>> {
    let Some(path) = contract_path(config)? else {
        return Ok(None);
    };
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read completion contract {}", path.display()))?;
    let projection: ContractProjection = serde_json::from_str(&text)
        .with_context(|| format!("invalid completion contract JSON {}", path.display()))?;
    if projection.command_observations.is_empty() {
        return Ok(None);
    }
    if !matches!(profile_id, ProfileId::Cli | ProfileId::PythonCli) {
        bail!("command_observations are registered only for CLI profiles");
    }
    if projection.command_observations.len() > MAX_OBSERVATIONS {
        bail!("command_observations exceed the fixed maximum of {MAX_OBSERVATIONS}");
    }

    let mut reasons = Vec::new();
    for (index, observation) in projection.command_observations.iter().enumerate() {
        validate_observation(&config.workspace_root, observation)?;
        let execution = execute_observation(&config.workspace_root, observation)
            .with_context(|| format!("command_observations[{}] failed to spawn", index + 1))?;
        crate::eval_events::emit(
            config.eval_events_path.as_deref(),
            json!({
                "event": "completion_contract_command_observation",
                "ordinal": index + 1,
                "argv": observation.argv,
                "expected_exit_code": observation.expected_exit_code,
                "observed_exit_code": execution.exit_code,
                "expected_stdout": observation.expected_stdout,
                "observed_stdout": execution.stdout,
                "observed_stderr": execution.stderr,
                "timed_out": execution.timed_out,
                "status": if execution.passed() { "passed" } else { "failed" },
                "reasons": execution.reasons,
            }),
        );
        if !execution.passed() {
            reasons.push(format!(
                "completion_contract_command_observation_{}_failed:{}",
                index + 1,
                execution.reasons.join(",")
            ));
        }
    }
    Ok(Some(ProfileBehaviorProbeReport {
        status: if reasons.is_empty() { "pass" } else { "failed" },
        reasons,
        evidence_path: None,
    }))
}

fn execute_observation(
    root: &Path,
    observation: &CommandObservation,
) -> anyhow::Result<ObservationExecution> {
    let mut command = verifier_env::normalized_command_at_root(
        observation.argv.first().expect("validated argv"),
        root,
    );
    command
        .args(&observation.argv[1..])
        .current_dir(root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = bounded_process::run_with_timeout(&mut command, FIXED_TIMEOUT)?;
    let exit_code = output.status.and_then(|status| status.code());
    let timed_out = output.kind == BoundedProcessOutcomeKind::TimedOut;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let mut reasons = Vec::new();
    if timed_out {
        reasons.push("timed_out".to_string());
    }
    if exit_code != Some(observation.expected_exit_code) {
        reasons.push(format!(
            "expected_exit_code_{}_observed_{}",
            observation.expected_exit_code,
            exit_code.map_or_else(|| "none".to_string(), |value| value.to_string())
        ));
    }
    if stdout != observation.expected_stdout {
        reasons.push("stdout_mismatch".to_string());
    }
    Ok(ObservationExecution {
        exit_code,
        timed_out,
        stdout,
        stderr,
        reasons,
    })
}

fn contract_path(config: &Config) -> anyhow::Result<Option<PathBuf>> {
    let path = config.completion_contract_path.clone().or_else(|| {
        crate::env_compat::var_os("COMMANDAGENT_COMPLETION_CONTRACT").map(PathBuf::from)
    });
    let Some(path) = path else {
        return Ok(None);
    };
    let path = if path.is_absolute() {
        path
    } else {
        config.workspace_root.join(path)
    };
    let root = config.workspace_root.canonicalize().with_context(|| {
        format!(
            "failed to resolve workspace {}",
            config.workspace_root.display()
        )
    })?;
    let canonical = path
        .canonicalize()
        .with_context(|| format!("failed to resolve completion contract {}", path.display()))?;
    if !canonical.starts_with(&root) {
        bail!("completion contract must stay inside the workspace");
    }
    Ok(Some(canonical))
}

fn validate_observation(root: &Path, observation: &CommandObservation) -> anyhow::Result<()> {
    if observation.argv.len() < 2 || observation.argv.len() > MAX_ARG_COUNT {
        bail!("command_observation argv must contain 2..{MAX_ARG_COUNT} entries");
    }
    if observation.argv.iter().map(String::len).sum::<usize>() > MAX_ARG_BYTES {
        bail!("command_observation argv exceeds {MAX_ARG_BYTES} bytes");
    }
    if observation.expected_stdout.len() > MAX_EXPECTED_BYTES {
        bail!("command_observation expected_stdout exceeds {MAX_EXPECTED_BYTES} bytes");
    }
    if !(0..=255).contains(&observation.expected_exit_code) {
        bail!("command_observation expected_exit_code must be in 0..=255");
    }
    if observation.argv.iter().any(|value| value.contains('\0')) {
        bail!("command_observation argv contains NUL");
    }
    let program = Path::new(&observation.argv[0])
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    if !matches!(program, "python" | "python3") {
        bail!("CLI command_observations require python or python3");
    }
    if observation.argv[1] != "cli/main.py" {
        bail!("CLI command_observations must execute cli/main.py directly");
    }
    validate_workspace_relative(&observation.argv[1])?;
    let entrypoint = root.join(&observation.argv[1]);
    if !entrypoint.is_file() {
        bail!("CLI command_observation entrypoint is missing");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_syntax_only_or_arbitrary_python_observations() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("cli")).unwrap();
        std::fs::write(root.path().join("cli/main.py"), "print('5')\n").unwrap();
        let syntax_only = CommandObservation {
            argv: vec![
                "python3".to_string(),
                "-m".to_string(),
                "py_compile".to_string(),
                "cli/main.py".to_string(),
            ],
            expected_exit_code: 0,
            expected_stdout: String::new(),
        };
        assert!(validate_observation(root.path(), &syntax_only).is_err());
    }

    #[test]
    fn accepts_exact_cli_entrypoint_observation() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("cli")).unwrap();
        std::fs::write(root.path().join("cli/main.py"), "print('5')\n").unwrap();
        let observation = CommandObservation {
            argv: vec![
                "python3".to_string(),
                "cli/main.py".to_string(),
                "2".to_string(),
                "3".to_string(),
            ],
            expected_exit_code: 0,
            expected_stdout: "5\n".to_string(),
        };
        assert!(validate_observation(root.path(), &observation).is_ok());
        let execution = execute_observation(root.path(), &observation).unwrap();
        assert!(execution.passed(), "{execution:?}");
        assert_eq!(execution.stdout, "5\n");
    }

    #[test]
    fn exact_cli_observation_rejects_stdout_mismatch() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("cli")).unwrap();
        std::fs::write(root.path().join("cli/main.py"), "print('5')\n").unwrap();
        let observation = CommandObservation {
            argv: vec!["python3".to_string(), "cli/main.py".to_string()],
            expected_exit_code: 0,
            expected_stdout: "4\n".to_string(),
        };

        let execution = execute_observation(root.path(), &observation).unwrap();

        assert!(!execution.passed());
        assert_eq!(execution.reasons, ["stdout_mismatch"]);
    }
}
