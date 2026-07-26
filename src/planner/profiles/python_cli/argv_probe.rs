use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use anyhow::{Context, anyhow, bail};
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
    pub source: Option<String>,
    pub observation: Option<Observation>,
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
    let output_examples = extract_output_examples(root, &config.usage_paths, &binding.entry)?;
    let mut output_claims = Vec::new();
    for (index, example) in output_examples.into_iter().enumerate() {
        let observation = observe(
            root,
            &config.entry,
            &example.args,
            config.timeout,
            &format!("output-claim-{}", index + 1),
        )?;
        let matched = example
            .expected_stdout
            .iter()
            .all(|claim| observation.stdout.text.lines().any(|line| line == claim));
        output_claims.push(OutputClaimBinding {
            claim: example.expected_stdout.join("\n"),
            matched,
            nearest_miss: (!matched).then(|| observation.stdout.text.clone()),
            source: Some(example.source),
            observation: Some(observation),
        });
    }
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
    let mut normal = None;
    for path in &config.usage_paths {
        if let Some(candidate) = extract_usage_case(root, path, &entry)? {
            normal = Some(candidate);
            break;
        }
    }
    let normal =
        normal.ok_or_else(|| anyhow!("case_extraction_failed: no CLI usage example found"))?;
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

fn extract_usage_case(
    root: &Path,
    relative: &Path,
    entry: &str,
) -> anyhow::Result<Option<BoundCase>> {
    let Some(path) =
        crate::tools::path_guard::resolve_existing(root, &relative.to_string_lossy()).ok()
    else {
        return Ok(None);
    };
    let text = std::fs::read_to_string(path)?;
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
        let args = normalize_usage_args(root, &words[2..]).with_context(|| {
            format!(
                "case_extraction_failed: {}:{}",
                relative.to_string_lossy(),
                index + 1
            )
        })?;
        return Ok(Some(BoundCase {
            id: "normal".to_string(),
            args,
            expected_stdout,
            source: format!("{}:{}", relative.to_string_lossy(), index + 1),
        }));
    }
    Ok(None)
}

fn normalize_usage_args(root: &Path, words: &[&str]) -> anyhow::Result<Vec<String>> {
    let mut normalized = Vec::new();
    let mut optional_group = false;
    for word in words {
        let opens = word.matches('[').count();
        let closes = word.matches(']').count();
        if optional_group || opens > 0 {
            if opens > 1 || closes > 1 || (optional_group && opens > 0) {
                bail!("nested or ambiguous optional CLI notation");
            }
            optional_group = closes == 0;
            continue;
        }
        if closes > 0 {
            bail!("unmatched optional CLI notation");
        }
        let argument = normalize_usage_arg(root, word)?;
        if argument.contains(['[', ']', '<', '>']) {
            bail!("unresolved CLI usage notation remains in argv");
        }
        normalized.push(argument);
    }
    if optional_group {
        bail!("unterminated optional CLI notation");
    }
    Ok(normalized)
}

fn normalize_usage_arg(root: &Path, word: &str) -> anyhow::Result<String> {
    if let Some((flag, value)) = word.split_once('=')
        && placeholder_name(value).is_some()
    {
        return Ok(format!(
            "{flag}={}",
            resolve_placeholder(root, placeholder_name(value).unwrap())?
        ));
    }
    if let Some(name) = placeholder_name(word) {
        return resolve_placeholder(root, name);
    }
    if word.contains(['<', '>']) {
        bail!("unsupported embedded CLI placeholder: {word}");
    }
    Ok(word.to_string())
}

fn placeholder_name(word: &str) -> Option<&str> {
    word.strip_prefix('<')?.strip_suffix('>')
}

fn resolve_placeholder(root: &Path, name: &str) -> anyhow::Result<String> {
    let lower = name.trim().to_ascii_lowercase();
    let samples = sample_files(root);
    let sample = sample_for_placeholder(&samples, &lower)
        .ok_or_else(|| anyhow!("no sample artifact binds <{name}>"))?;
    if lower.contains("column") {
        let sample_text = std::fs::read_to_string(root.join(sample))?;
        let header = sample_text
            .lines()
            .next()
            .and_then(|line| {
                line.split(',')
                    .map(str::trim)
                    .find(|value| !value.is_empty())
            })
            .ok_or_else(|| anyhow!("sample CSV has no column binding for <{name}>"))?;
        return Ok(header.to_string());
    }
    if ["pattern", "search", "query", "string"]
        .iter()
        .any(|token| lower.contains(token))
    {
        let sample_text = std::fs::read_to_string(root.join(sample))?;
        let value = sample_text
            .split(|ch: char| !(ch.is_alphanumeric() || matches!(ch, '_' | '-')))
            .find(|value| !value.is_empty())
            .ok_or_else(|| anyhow!("sample text has no value binding for <{name}>"))?;
        return Ok(value.to_string());
    }
    if ["file", "path", "input", "csv", "text", "txt"]
        .iter()
        .any(|token| lower.contains(token))
    {
        return Ok(sample.clone());
    }
    bail!("unsupported CLI placeholder <{name}>")
}

fn sample_files(root: &Path) -> Vec<String> {
    let data = root.join("data");
    let Ok(entries) = std::fs::read_dir(&data) else {
        return Vec::new();
    };
    let mut samples = entries
        .flatten()
        .filter_map(|entry| {
            entry
                .file_type()
                .ok()
                .filter(|kind| kind.is_file())
                .and_then(|_| entry.path().strip_prefix(root).ok().map(Path::to_path_buf))
                .map(|path| path.to_string_lossy().replace('\\', "/"))
        })
        .collect::<Vec<_>>();
    samples.sort();
    samples
}

fn sample_for_placeholder<'a>(samples: &'a [String], name: &str) -> Option<&'a String> {
    let expected_extension = if name.contains("csv") || name.contains("column") {
        Some("csv")
    } else if ["text", "txt", "pattern", "search", "query", "string"]
        .iter()
        .any(|token| name.contains(token))
    {
        Some("txt")
    } else {
        None
    };
    expected_extension
        .and_then(|extension| {
            samples.iter().find(|sample| {
                Path::new(sample).extension().and_then(|ext| ext.to_str()) == Some(extension)
            })
        })
        .or_else(|| samples.first())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OutputExample {
    args: Vec<String>,
    expected_stdout: Vec<String>,
    source: String,
}

fn extract_output_examples(
    root: &Path,
    usage_paths: &[PathBuf],
    entry: &str,
) -> anyhow::Result<Vec<OutputExample>> {
    let mut examples = Vec::new();
    for relative in usage_paths {
        let Some(path) =
            crate::tools::path_guard::resolve_existing(root, &relative.to_string_lossy()).ok()
        else {
            continue;
        };
        let text = std::fs::read_to_string(path)?;
        let lines = text.lines().collect::<Vec<_>>();
        let mut index = 0;
        while index < lines.len() {
            let Some(marker) = fence_marker(lines[index]) else {
                index += 1;
                continue;
            };
            let Some(end) = closing_fence(&lines, index + 1, marker) else {
                break;
            };
            for command_index in index + 1..end {
                let Some(words) = cli_invocation_words(lines[command_index], entry) else {
                    continue;
                };
                let args = normalize_usage_args(root, &words[2..]).with_context(|| {
                    format!(
                        "cli_output_claim_extraction_failed: {}:{}",
                        relative.to_string_lossy(),
                        command_index + 1
                    )
                })?;
                let inline = lines[command_index + 1..end]
                    .iter()
                    .take_while(|line| cli_invocation_words(line, entry).is_none())
                    .map(|line| line.trim_end().to_string())
                    .filter(|line| !line.is_empty() && !line.starts_with("$ "))
                    .collect::<Vec<_>>();
                let (expected_stdout, output_line) = if inline.is_empty() {
                    labeled_output_block(&lines, end + 1)
                        .unwrap_or_else(|| (Vec::new(), command_index + 1))
                } else {
                    (inline, command_index + 2)
                };
                if !expected_stdout.is_empty() {
                    examples.push(OutputExample {
                        args,
                        expected_stdout,
                        source: format!(
                            "{}:{}->{}",
                            relative.to_string_lossy(),
                            command_index + 1,
                            output_line
                        ),
                    });
                }
            }
            index = end + 1;
        }
    }
    Ok(examples)
}

fn cli_invocation_words<'a>(line: &'a str, entry: &str) -> Option<Vec<&'a str>> {
    let command = line.trim().strip_prefix("$ ").unwrap_or(line.trim());
    let words = command.split_whitespace().collect::<Vec<_>>();
    (words.len() >= 2
        && matches!(words[0], "python" | "python3")
        && words[1].trim_start_matches("./") == entry)
        .then_some(words)
}

fn fence_marker(line: &str) -> Option<&'static str> {
    let trimmed = line.trim_start();
    if trimmed.starts_with("```") {
        Some("```")
    } else if trimmed.starts_with("~~~") {
        Some("~~~")
    } else {
        None
    }
}

fn closing_fence(lines: &[&str], start: usize, marker: &str) -> Option<usize> {
    (start..lines.len()).find(|index| lines[*index].trim_start().starts_with(marker))
}

fn labeled_output_block(lines: &[&str], start: usize) -> Option<(Vec<String>, usize)> {
    let mut index = start;
    while index < lines.len() && lines[index].trim().is_empty() {
        index += 1;
    }
    if index >= lines.len()
        || lines[index].trim_start().starts_with('#')
        || !is_output_label(lines[index])
    {
        return None;
    }
    index += 1;
    while index < lines.len() && lines[index].trim().is_empty() {
        index += 1;
    }
    let marker = fence_marker(lines.get(index)?)?;
    let end = closing_fence(lines, index + 1, marker)?;
    let output = lines[index + 1..end]
        .iter()
        .map(|line| line.trim_end().to_string())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    Some((output, index + 2))
}

fn is_output_label(line: &str) -> bool {
    let normalized = line
        .trim()
        .trim_matches(|ch: char| matches!(ch, '*' | '_' | '`' | ':' | '：'))
        .trim()
        .to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        "output" | "expected output" | "output example" | "出力" | "出力例" | "実行結果"
    )
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

    const MEASURED_FIXTURE: &str = "tests/corpus/apps/test0725_cli_elev_003/fixtures";

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

    fn measured_fixture(include_sample: bool) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for relative in ["README.md", "cli/main.py"] {
            let target = dir.path().join(relative);
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            std::fs::copy(Path::new(MEASURED_FIXTURE).join(relative), target).unwrap();
        }
        if include_sample {
            let target = dir.path().join("data/sample.txt");
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            std::fs::copy(Path::new(MEASURED_FIXTURE).join("data/sample.txt"), target).unwrap();
        }
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

    #[test]
    fn measured_optional_and_placeholder_usage_normalizes_to_sample_values() {
        let dir = measured_fixture(true);

        let report = run(dir.path(), Config::new("cli/main.py", &["README.md"])).unwrap();

        assert_eq!(
            report.binding.cases[0].args,
            ["data/sample.txt", "--pattern", "Apple"]
        );
        assert_eq!(report.binding.cases[0].source, "README.md:8");
        assert!(
            report.binding.cases[0]
                .args
                .iter()
                .all(|arg| !arg.contains(['[', ']', '<', '>']))
        );
        assert_eq!(report.observations[0].exit_code, Some(0));
    }

    #[test]
    fn measured_usage_without_sample_binding_fails_honestly() {
        let dir = measured_fixture(false);

        let error = run(dir.path(), Config::new("cli/main.py", &["README.md"])).unwrap_err();

        assert!(
            error.to_string().contains("case_extraction_failed"),
            "{error:#}"
        );
        assert!(!dir.path().join(CASE_BINDING_PATH).exists());
    }

    #[test]
    fn measured_readme_extracts_two_output_examples_and_rejects_both() {
        let dir = measured_fixture(true);

        let report = run(dir.path(), Config::new("cli/main.py", &["README.md"])).unwrap();

        assert!(report.c1_ok);
        assert!(report.c4_ok);
        assert_eq!(report.output_claims.len(), 2);
        assert!(
            report.output_claims.iter().all(|claim| !claim.matched),
            "{:?}",
            report.output_claims
        );
        assert_eq!(
            report.output_claims[0].claim,
            "I like apple.\nApple is red.\nAn apple a day keeps the doctor away."
        );
        assert_eq!(
            report.output_claims[0].nearest_miss.as_deref(),
            Some("I like apple pie.\n")
        );
        assert_eq!(report.output_claims[1].claim, "2");
        assert_eq!(report.output_claims[1].nearest_miss.as_deref(), Some("0\n"));
        assert!(
            report
                .output_claims
                .iter()
                .all(|claim| claim.observation.is_some())
        );
    }
}
