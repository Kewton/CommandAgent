use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};

use crate::bounded_process::{self, BoundedProcessOutcomeKind};
use crate::minimal_loop::pipeline_probe::{StreamCapture, capture_stream, join_capture};
use crate::minimal_loop::verifier_env;

pub const CASE_BINDING_PATH: &str = "evidence/cli-case-binding.json";
pub const EVIDENCE_PATH: &str = "evidence/cli-probe.json";
pub(super) const INVALID_OPTION: &str = "--anvil-invalid-probe";

#[derive(Debug, Clone)]
pub struct Config {
    entry: PathBuf,
    usage_paths: Vec<PathBuf>,
    timeout: Duration,
}

impl Config {
    pub fn new(entry: impl Into<PathBuf>, usage_paths: &[&str]) -> Self {
        Self {
            entry: entry.into(),
            usage_paths: usage_paths.iter().map(PathBuf::from).collect(),
            timeout: Duration::from_secs(5),
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundCase {
    pub id: String,
    pub args: Vec<String>,
    pub expected_stdout: Vec<String>,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaseBinding {
    pub entry: String,
    pub cases: Vec<BoundCase>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Observation {
    pub case_id: String,
    pub args: Vec<String>,
    pub outcome: String,
    pub exit_code: Option<i32>,
    pub duration_ms: u64,
    pub stdout: StreamCapture,
    pub stderr: StreamCapture,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputClaimBinding {
    pub claim: String,
    pub matched: bool,
    pub nearest_miss: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    pub capability_id: String,
    pub status: String,
    pub ok: bool,
    pub binding_intact: bool,
    pub c1_ok: bool,
    pub c4_ok: bool,
    pub binding: CaseBinding,
    pub observations: Vec<Observation>,
    pub output_claims: Vec<OutputClaimBinding>,
    pub failure_kinds: Vec<String>,
}

pub fn run(root: &Path, config: Config) -> anyhow::Result<Report> {
    validate_config(root, &config)?;
    let candidate = bind_cases(root, &config)?;
    let (binding, binding_intact) = freeze_binding(root, &candidate)?;
    let mut observations = Vec::new();
    if binding_intact {
        observations.push(observe(
            root,
            &config.entry,
            &binding.cases[0].args,
            config.timeout,
            "normal",
        )?);
        observations.push(observe(
            root,
            &config.entry,
            &binding.cases[1].args,
            config.timeout,
            "invalid",
        )?);
        observations.push(observe(
            root,
            &config.entry,
            &binding.cases[0].args,
            config.timeout,
            "normal-rerun",
        )?);
    }
    let c1_ok = observations.len() == 3
        && observations[0].exit_code == Some(0)
        && observations[1].exit_code.is_some_and(|code| code != 0);
    let c4_ok = observations.len() == 3 && equivalent(&observations[0], &observations[2]);
    let output_claims = binding.cases[0]
        .expected_stdout
        .iter()
        .map(|claim| {
            let matched = observations.first().is_some_and(|observation| {
                observation.stdout.text.lines().any(|line| line == claim)
            });
            OutputClaimBinding {
                claim: claim.clone(),
                matched,
                nearest_miss: (!matched)
                    .then(|| observations.first().map(|item| item.stdout.text.clone()))
                    .flatten(),
            }
        })
        .collect();
    let mut failure_kinds = Vec::new();
    if !binding_intact {
        failure_kinds.push("cli_case_binding_changed".to_string());
    }
    if !c1_ok {
        failure_kinds.push("cli_probe_polarity_violation".to_string());
    }
    if !c4_ok {
        failure_kinds.push("cli_rerun_mismatch".to_string());
    }
    let ok = failure_kinds.is_empty();
    let report = Report {
        capability_id: "cli_probe".to_string(),
        status: if ok { "pass" } else { "failed" }.to_string(),
        ok,
        binding_intact,
        c1_ok,
        c4_ok,
        binding,
        observations,
        output_claims,
        failure_kinds,
    };
    write_json(root, EVIDENCE_PATH, &report)?;
    Ok(report)
}

fn validate_config(root: &Path, config: &Config) -> anyhow::Result<()> {
    if config.timeout.is_zero() || config.usage_paths.is_empty() {
        bail!("CLI probe requires a positive timeout and at least one usage path");
    }
    let entry = config.entry.to_string_lossy();
    crate::tools::path_guard::validate_workspace_relative(&entry)?;
    crate::tools::path_guard::resolve_existing(root, &entry)
        .context("CLI entry is not an accessible workspace file")?;
    Ok(())
}

fn bind_cases(root: &Path, config: &Config) -> anyhow::Result<CaseBinding> {
    let entry = config.entry.to_string_lossy().replace('\\', "/");
    let normal = config
        .usage_paths
        .iter()
        .find_map(|path| extract_usage_case(root, path, &entry))
        .context("no executable CLI usage example found")?;
    Ok(CaseBinding {
        entry,
        cases: vec![
            normal,
            BoundCase {
                id: "invalid".to_string(),
                args: vec![INVALID_OPTION.to_string()],
                expected_stdout: Vec::new(),
                source: "contract:deterministic-invalid-option".to_string(),
            },
        ],
    })
}

fn extract_usage_case(root: &Path, relative: &Path, entry: &str) -> Option<BoundCase> {
    let text = std::fs::read_to_string(
        crate::tools::path_guard::resolve_existing(root, &relative.to_string_lossy()).ok()?,
    )
    .ok()?;
    let mut fenced = false;
    let mut lines = text.lines().enumerate().peekable();
    while let Some((index, line)) = lines.next() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if !fenced {
            continue;
        }
        let command = line.trim().strip_prefix("$ ").unwrap_or(line.trim());
        let words = command.split_whitespace().collect::<Vec<_>>();
        if words.len() < 2
            || !matches!(words[0], "python" | "python3")
            || words[1].trim_start_matches("./") != entry
        {
            continue;
        }
        let expected_stdout = lines
            .take_while(|(_, next)| !next.trim_start().starts_with("```"))
            .map(|(_, output)| output.trim_end().to_string())
            .filter(|output| !output.is_empty() && !output.starts_with("$ "))
            .collect();
        return Some(BoundCase {
            id: "normal".to_string(),
            args: words[2..].iter().map(|word| (*word).to_string()).collect(),
            expected_stdout,
            source: format!("{}:{}", relative.to_string_lossy(), index + 1),
        });
    }
    None
}

fn freeze_binding(root: &Path, candidate: &CaseBinding) -> anyhow::Result<(CaseBinding, bool)> {
    let path = root.join(CASE_BINDING_PATH);
    if path.exists() {
        let frozen = serde_json::from_slice::<CaseBinding>(&std::fs::read(path)?)?;
        let intact = frozen == *candidate;
        return Ok((frozen, intact));
    }
    write_json(root, CASE_BINDING_PATH, candidate)?;
    Ok((candidate.clone(), true))
}

pub(super) fn observe(
    root: &Path,
    entry: &Path,
    args: &[String],
    timeout: Duration,
    case_id: &str,
) -> anyhow::Result<Observation> {
    let mut command = verifier_env::normalized_command_at_root("python3", root);
    command
        .arg("-B")
        .arg(entry)
        .args(args)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = bounded_process::spawn_child(&mut command)?;
    let stdout = capture_stream(
        child.stdout.take().context("CLI stdout pipe missing")?,
        24_000,
    );
    let stderr = capture_stream(
        child.stderr.take().context("CLI stderr pipe missing")?,
        24_000,
    );
    let outcome = bounded_process::wait_with_timeout(child, timeout)?;
    Ok(Observation {
        case_id: case_id.to_string(),
        args: args.to_vec(),
        outcome: match outcome.kind {
            BoundedProcessOutcomeKind::Exited => "exited",
            _ => "timed_out",
        }
        .to_string(),
        exit_code: outcome.status.and_then(|status| status.code()),
        duration_ms: outcome.elapsed.as_millis().try_into().unwrap_or(u64::MAX),
        stdout: join_capture(stdout, "stdout")?,
        stderr: join_capture(stderr, "stderr")?,
    })
}

fn equivalent(first: &Observation, second: &Observation) -> bool {
    crate::minimal_loop::rerun_consistency::reproduced(
        &(
            &first.outcome,
            first.exit_code,
            &first.stdout,
            &first.stderr,
        ),
        &(
            &second.outcome,
            second.exit_code,
            &second.stdout,
            &second.stderr,
        ),
    )
}

pub(super) fn write_json<T: Serialize>(
    root: &Path,
    relative: &str,
    value: &T,
) -> anyhow::Result<()> {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().context("CLI evidence parent missing")?)?;
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    std::fs::write(path, bytes)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(script: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("cli")).unwrap();
        std::fs::write(dir.path().join("cli/main.py"), script).unwrap();
        std::fs::write(
            dir.path().join("README.md"),
            "## Usage\n\n```console\n$ python3 cli/main.py sample.csv\nvalue=7\n```\n",
        )
        .unwrap();
        dir
    }

    #[test]
    fn probe_observes_both_exit_polarities_and_rerun_equality() {
        let dir = fixture(
            "import sys\nif '--anvil-invalid-probe' in sys.argv: raise SystemExit(2)\nprint('value=7')\n",
        );
        let report = run(
            dir.path(),
            Config::new("cli/main.py", &["README.md", "USAGE.md"]),
        )
        .unwrap();
        assert!(report.ok, "{report:?}");
        assert_eq!(report.observations[0].exit_code, Some(0));
        assert_eq!(report.observations[1].exit_code, Some(2));
        assert!(report.c4_ok);
        assert!(dir.path().join(CASE_BINDING_PATH).is_file());
    }

    #[test]
    fn swallowing_cli_is_rejected_when_invalid_input_exits_zero() {
        let dir = fixture("print('value=7')\n");
        let report = run(dir.path(), Config::new("cli/main.py", &["README.md"])).unwrap();
        assert!(!report.ok);
        assert!(!report.c1_ok);
        assert!(
            report
                .failure_kinds
                .contains(&"cli_probe_polarity_violation".to_string())
        );
    }

    #[test]
    fn first_bound_usage_case_is_frozen_before_execution() {
        let dir = fixture(
            "import sys\nif '--anvil-invalid-probe' in sys.argv: raise SystemExit(2)\nprint('value=7')\n",
        );
        let config = || Config::new("cli/main.py", &["README.md"]);
        assert!(run(dir.path(), config()).unwrap().ok);
        let frozen = std::fs::read(dir.path().join(CASE_BINDING_PATH)).unwrap();
        std::fs::write(
            dir.path().join("README.md"),
            "```console\n$ python3 cli/main.py replacement.csv\nvalue=8\n```\n",
        )
        .unwrap();

        let report = run(dir.path(), config()).unwrap();

        assert!(!report.binding_intact);
        assert!(report.observations.is_empty());
        assert_eq!(
            std::fs::read(dir.path().join(CASE_BINDING_PATH)).unwrap(),
            frozen
        );
        assert_eq!(report.binding.cases[0].args, ["sample.csv"]);
    }
}
