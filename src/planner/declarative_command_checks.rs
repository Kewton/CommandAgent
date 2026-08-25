use std::path::{Component, Path};
use std::process::Stdio;
use std::time::{Duration, Instant};

use regex::Regex;
use serde::Serialize;
use serde_json::json;
use toml::Value;
use toml::value::Table;

use crate::bounded_process::{self, BoundedProcessOutcomeKind};
use crate::minimal_loop::verifier_env;
use crate::planner::capability_catalog::{
    CapabilityKind, CapabilitySpec, CatalogError, ParamSpec, ParamType, ResolvedCapability,
};

pub(crate) const ID: &str = "command_check";
const MAX_ARG_COUNT: usize = 64;
const MAX_ARG_BYTES: usize = 16_384;
const MAX_REGEX_BYTES: usize = 1_024;
const MAX_OUTPUT_BYTES: usize = 24_000;
const FIXED_TIMEOUT: Duration = Duration::from_secs(30);

static CWD_VALUES: [&str; 1] = ["workspace"];
static PARAMS: [ParamSpec; 3] = [
    ParamSpec {
        name: "argv",
        param_type: ParamType::StringList,
        required: true,
        default: None,
    },
    ParamSpec {
        name: "cwd",
        param_type: ParamType::Enum(&CWD_VALUES),
        required: true,
        default: None,
    },
    ParamSpec {
        name: "expect",
        param_type: ParamType::CommandExpectation,
        required: true,
        default: None,
    },
];
static SPEC: CapabilitySpec = CapabilitySpec {
    id: ID,
    kind: CapabilityKind::CommandCheck,
    params: &PARAMS,
    description: "Run one bounded argv-only check from the workspace root.",
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclarativeCommandCheck {
    argv: Vec<String>,
    expected_exit_code: i32,
    stdout_regex: Option<String>,
    max_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CommandCheckBinding {
    pub(crate) id: String,
    pub(crate) check: DeclarativeCommandCheck,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct CommandCheckResult {
    pub(crate) check_id: String,
    pub(crate) ordinal: usize,
    pub(crate) status: &'static str,
    pub(crate) observed_exit_code: Option<i32>,
    pub(crate) timed_out: bool,
    pub(crate) elapsed_ms: u128,
    pub(crate) reasons: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct CommandCheckSummary {
    pub(crate) passed: bool,
    pub(crate) check_count: usize,
    pub(crate) passed_count: usize,
    pub(crate) failed_count: usize,
    pub(crate) results: Vec<CommandCheckResult>,
}

#[derive(Debug)]
struct Observation {
    exit_code: Option<i32>,
    timed_out: bool,
    elapsed_ms: u128,
    stdout: String,
    stderr: String,
    output_truncated: bool,
    reasons: Vec<String>,
}

pub(super) fn combined_registry(base: &'static [CapabilitySpec]) -> &'static [CapabilitySpec] {
    use std::sync::OnceLock;
    static COMBINED: OnceLock<Vec<CapabilitySpec>> = OnceLock::new();
    COMBINED.get_or_init(|| base.iter().copied().chain([SPEC]).collect())
}

pub(super) fn is_id(id: &str) -> bool {
    id == ID
}

pub(super) fn resolve(params: &Table) -> Result<ResolvedCapability, CatalogError> {
    let argv = string_array(params, "argv")?;
    match params.get("cwd") {
        Some(Value::String(value)) if value == "workspace" => {}
        Some(_) => return Err(type_mismatch("cwd", "enum[workspace]")),
        None => return Err(missing("cwd")),
    }
    let expectation = match params.get("expect") {
        Some(Value::Table(value)) => value,
        Some(_) => return Err(type_mismatch("expect", "command_expectation")),
        None => return Err(missing("expect")),
    };
    reject_unknown(expectation, &["exit_code", "stdout_regex", "max_bytes"])?;
    let expected_exit_code = bounded_integer(expectation, "exit_code", 0, 255)? as i32;
    let max_bytes = bounded_integer(expectation, "max_bytes", 1, MAX_OUTPUT_BYTES as i64)? as usize;
    let stdout_regex = match expectation.get("stdout_regex") {
        Some(Value::String(value)) => Some(value.clone()),
        Some(_) => return Err(invalid("expect", "stdout_regex must be a string")),
        None => None,
    };
    let check = DeclarativeCommandCheck::new(argv, expected_exit_code, stdout_regex, max_bytes)
        .map_err(|reason| invalid("argv", reason))?;
    Ok(ResolvedCapability::CommandCheck(check))
}

impl DeclarativeCommandCheck {
    fn new(
        argv: Vec<String>,
        expected_exit_code: i32,
        stdout_regex: Option<String>,
        max_bytes: usize,
    ) -> Result<Self, String> {
        validate_argv(&argv)?;
        if let Some(pattern) = stdout_regex.as_deref() {
            if pattern.is_empty() || pattern.len() > MAX_REGEX_BYTES {
                return Err(format!(
                    "stdout_regex must contain 1..{MAX_REGEX_BYTES} bytes"
                ));
            }
            Regex::new(pattern).map_err(|error| format!("stdout_regex is invalid: {error}"))?;
        }
        Ok(Self {
            argv,
            expected_exit_code,
            stdout_regex,
            max_bytes,
        })
    }
}

pub(crate) fn run_and_record(
    root: &Path,
    bindings: &[CommandCheckBinding],
    events_path: Option<&Path>,
    source: &str,
    owner_id: &str,
) -> CommandCheckSummary {
    if bindings.is_empty() {
        return CommandCheckSummary {
            passed: true,
            ..CommandCheckSummary::default()
        };
    }
    let mut results = Vec::with_capacity(bindings.len());
    for (index, binding) in bindings.iter().enumerate() {
        let observation = execute(root, &binding.check);
        let passed = observation.reasons.is_empty();
        let result = CommandCheckResult {
            check_id: binding.id.clone(),
            ordinal: index + 1,
            status: if passed { "passed" } else { "failed" },
            observed_exit_code: observation.exit_code,
            timed_out: observation.timed_out,
            elapsed_ms: observation.elapsed_ms,
            reasons: observation.reasons.clone(),
        };
        crate::eval_events::emit(
            events_path,
            json!({
                "event": "declarative_command_check_result",
                "source": source,
                "owner_id": owner_id,
                "check_id": binding.id,
                "ordinal": index + 1,
                "argv": binding.check.argv,
                "cwd": "workspace",
                "expected_exit_code": binding.check.expected_exit_code,
                "observed_exit_code": observation.exit_code,
                "stdout_regex": binding.check.stdout_regex,
                "max_bytes": binding.check.max_bytes,
                "fixed_timeout_ms": fixed_timeout().as_millis(),
                "timed_out": observation.timed_out,
                "elapsed_ms": observation.elapsed_ms,
                "output_truncated": observation.output_truncated,
                "stdout": observation.stdout,
                "stderr": observation.stderr,
                "status": result.status,
                "reasons": observation.reasons,
            }),
        );
        results.push(result);
    }
    let passed_count = results
        .iter()
        .filter(|result| result.status == "passed")
        .count();
    let summary = CommandCheckSummary {
        passed: passed_count == results.len(),
        check_count: results.len(),
        passed_count,
        failed_count: results.len() - passed_count,
        results,
    };
    append_summary(events_path, source, owner_id, &summary);
    summary
}

impl CommandCheckSummary {
    pub(crate) fn failure_reasons(&self) -> Vec<String> {
        self.results
            .iter()
            .filter(|result| result.status == "failed")
            .map(|result| {
                format!(
                    "declarative command check `{}` #{} failed: {}",
                    result.check_id,
                    result.ordinal,
                    result
                        .reasons
                        .first()
                        .map(String::as_str)
                        .unwrap_or("unspecified failure")
                )
            })
            .collect()
    }

    pub(crate) fn primary_reason(&self) -> Option<String> {
        self.failure_reasons().into_iter().next()
    }
}

fn execute(root: &Path, check: &DeclarativeCommandCheck) -> Observation {
    let started = Instant::now();
    let mut command = verifier_env::normalized_command_at_root(&check.argv[0], root);
    command
        .args(&check.argv[1..])
        .current_dir(root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = match bounded_process::run_with_timeout(&mut command, fixed_timeout()) {
        Ok(output) => output,
        Err(error) => {
            return Observation {
                exit_code: None,
                timed_out: false,
                elapsed_ms: started.elapsed().as_millis(),
                stdout: String::new(),
                stderr: String::new(),
                output_truncated: false,
                reasons: vec![format!("command spawn failed: {error}")],
            };
        }
    };
    let raw_stdout = String::from_utf8_lossy(&output.stdout);
    let raw_stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = project_stream(&raw_stdout, check.max_bytes, "stdout");
    let stderr = project_stream(&raw_stderr, check.max_bytes, "stderr");
    let timed_out = output.kind == BoundedProcessOutcomeKind::TimedOut;
    let exit_code = output.status.and_then(|status| status.code());
    let mut reasons = Vec::new();
    if timed_out {
        reasons.push(format!(
            "command timed out after {} ms",
            fixed_timeout().as_millis()
        ));
    } else if exit_code != Some(check.expected_exit_code) {
        reasons.push(format!(
            "expected exit code {}, observed {}",
            check.expected_exit_code,
            exit_code.map_or_else(|| "none".to_string(), |code| code.to_string())
        ));
    }
    if raw_stdout.len() > check.max_bytes || raw_stderr.len() > check.max_bytes {
        reasons.push(format!(
            "command output exceeded max_bytes ({})",
            check.max_bytes
        ));
    }
    if let Some(pattern) = check.stdout_regex.as_deref()
        && !Regex::new(pattern)
            .expect("validated command-check regex")
            .is_match(&raw_stdout)
    {
        reasons.push("stdout did not match stdout_regex".to_string());
    }
    Observation {
        exit_code,
        timed_out,
        elapsed_ms: output.elapsed.as_millis(),
        stdout,
        stderr,
        output_truncated: raw_stdout.len() > check.max_bytes || raw_stderr.len() > check.max_bytes,
        reasons,
    }
}

fn validate_argv(argv: &[String]) -> Result<(), String> {
    if argv.is_empty() || argv.len() > MAX_ARG_COUNT {
        return Err(format!("argv must contain 1..{MAX_ARG_COUNT} entries"));
    }
    if argv.iter().map(String::len).sum::<usize>() > MAX_ARG_BYTES {
        return Err(format!("argv exceeds {MAX_ARG_BYTES} bytes"));
    }
    if argv[0].trim().is_empty() {
        return Err("argv[0] must not be empty".to_string());
    }
    for value in argv {
        validate_argument_path(value)?;
    }
    let program = Path::new(&argv[0])
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(&argv[0])
        .to_ascii_lowercase();
    if matches!(
        program.as_str(),
        "sh" | "bash" | "dash" | "zsh" | "fish" | "ksh" | "cmd" | "powershell" | "pwsh"
    ) {
        return Err("shell interpreters are not registered command-check programs".to_string());
    }
    if matches!(
        program.as_str(),
        "nice"
            | "nohup"
            | "timeout"
            | "stdbuf"
            | "setsid"
            | "busybox"
            | "watch"
            | "sudo"
            | "doas"
            | "su"
            | "script"
            | "unshare"
    ) {
        return Err(
            "process-launch wrappers are not registered command-check programs".to_string(),
        );
    }
    if matches!(
        program.as_str(),
        "rm" | "mv"
            | "cp"
            | "install"
            | "chmod"
            | "chown"
            | "dd"
            | "truncate"
            | "tee"
            | "env"
            | "xargs"
    ) {
        return Err("mutating filesystem programs are not command checks".to_string());
    }
    if program == "find"
        && argv
            .iter()
            .any(|arg| matches!(arg.as_str(), "-exec" | "-execdir" | "-delete"))
    {
        return Err("find mutation/execution flags are not command checks".to_string());
    }
    if program == "git"
        && argv.iter().skip(1).any(|arg| {
            matches!(
                arg.as_str(),
                "-c" | "--config-env"
                    | "clean"
                    | "reset"
                    | "checkout"
                    | "switch"
                    | "restore"
                    | "commit"
                    | "merge"
                    | "rebase"
                    | "push"
                    | "pull"
                    | "fetch"
            ) || arg.starts_with("--config-env=")
        })
    {
        return Err("mutating git subcommands are not command checks".to_string());
    }
    let inline_eval = match program.as_str() {
        "python" | "python3" => argv
            .iter()
            .skip(1)
            .any(|arg| arg == "-c" || arg.starts_with("-c")),
        "node" => argv.iter().skip(1).any(|arg| {
            matches!(arg.as_str(), "-e" | "--eval" | "-p" | "--print")
                || arg.starts_with("-e")
                || arg.starts_with("-p")
                || arg.starts_with("--eval=")
                || arg.starts_with("--print=")
        }),
        "ruby" | "perl" => argv.iter().skip(1).any(|arg| {
            matches!(arg.as_str(), "-e" | "-E" | "--eval")
                || arg.starts_with("-e")
                || arg.starts_with("-E")
                || arg.starts_with("--eval=")
        }),
        "php" => argv
            .iter()
            .skip(1)
            .any(|arg| arg == "-r" || arg.starts_with("-r") || arg == "--run"),
        _ => false,
    };
    if inline_eval {
        return Err("inline interpreter code is not a declarative command check".to_string());
    }
    let rendered = render_argv(argv);
    if let Some(reason) = crate::tools::bash::blocked_reason(&rendered, true) {
        return Err(format!("command is outside verify policy: {reason}"));
    }
    crate::planner::verify::normalize_verify_command(&rendered)
        .map(|_| ())
        .map_err(|error| format!("command is outside verify policy: {error}"))
}

pub(crate) fn validate_shadow_argv(argv: &[String]) -> Result<(), String> {
    validate_argv(argv)
}

fn validate_argument_path(value: &str) -> Result<(), String> {
    let candidate = value.split_once('=').map_or(value, |(_, value)| value);
    let path = Path::new(candidate);
    if path.is_absolute()
        || path
            .components()
            .any(|component| component == Component::ParentDir)
    {
        return Err(format!(
            "argv path `{candidate}` must stay workspace-relative"
        ));
    }
    Ok(())
}

fn render_argv(argv: &[String]) -> String {
    argv.iter()
        .map(|arg| {
            if !arg.is_empty()
                && arg
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"_-./:=,@%+".contains(&byte))
            {
                arg.clone()
            } else {
                format!("'{}'", arg.replace('\'', "'\\''"))
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn project_stream(value: &str, max_bytes: usize, label: &str) -> String {
    crate::util::excerpt_with_newline_marker(
        value,
        max_bytes,
        &format!("[commandagent: command-check {label} truncated at {max_bytes} bytes]"),
    )
}

fn append_summary(
    events_path: Option<&Path>,
    source: &str,
    owner_id: &str,
    summary: &CommandCheckSummary,
) {
    let mut lines = vec![format!(
        "Declarative command checks ({source} `{owner_id}`): {}/{} passed",
        summary.passed_count, summary.check_count
    )];
    lines.extend(summary.results.iter().map(|result| {
        let reason = result
            .reasons
            .first()
            .map(|reason| format!(" — {reason}"))
            .unwrap_or_default();
        format!(
            "- {} #{}: {}{}",
            result.check_id, result.ordinal, result.status, reason
        )
    }));
    crate::eval_events::append_run_summary(events_path, &lines.join("\n"));
}

fn fixed_timeout() -> Duration {
    if cfg!(test) {
        Duration::from_millis(100)
    } else {
        FIXED_TIMEOUT
    }
}

fn string_array(params: &Table, name: &str) -> Result<Vec<String>, CatalogError> {
    match params.get(name) {
        Some(Value::Array(values)) => values
            .iter()
            .map(|value| match value {
                Value::String(value) => Ok(value.clone()),
                _ => Err(type_mismatch(name, "[string]")),
            })
            .collect(),
        Some(_) => Err(type_mismatch(name, "[string]")),
        None => Err(missing(name)),
    }
}

fn bounded_integer(params: &Table, name: &str, min: i64, max: i64) -> Result<i64, CatalogError> {
    match params.get(name) {
        Some(Value::Integer(value)) if (min..=max).contains(value) => Ok(*value),
        Some(Value::Integer(_)) => Err(invalid(
            "expect",
            format!("{name} must be between {min} and {max}"),
        )),
        Some(_) => Err(invalid("expect", format!("{name} must be an integer"))),
        None => Err(invalid("expect", format!("{name} is required"))),
    }
}

fn reject_unknown(params: &Table, allowed: &[&str]) -> Result<(), CatalogError> {
    if let Some(name) = params.keys().find(|name| !allowed.contains(&name.as_str())) {
        return Err(invalid("expect", format!("unknown field `{name}`")));
    }
    Ok(())
}

fn missing(parameter: &str) -> CatalogError {
    CatalogError::MissingParameter {
        id: ID.to_string(),
        parameter: parameter.to_string(),
    }
}

fn type_mismatch(parameter: &str, expected: &str) -> CatalogError {
    CatalogError::TypeMismatch {
        id: ID.to_string(),
        parameter: parameter.to_string(),
        expected: expected.to_string(),
    }
}

fn invalid(parameter: &str, reason: impl Into<String>) -> CatalogError {
    CatalogError::InvalidParameter {
        id: ID.to_string(),
        parameter: parameter.to_string(),
        reason: reason.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::profile_manifest::ManifestStatus;

    fn check(argv: &[&str], regex: Option<&str>, max_bytes: usize) -> DeclarativeCommandCheck {
        DeclarativeCommandCheck::new(
            argv.iter().map(|value| (*value).to_string()).collect(),
            0,
            regex.map(str::to_string),
            max_bytes,
        )
        .unwrap()
    }

    fn params(argv: &[&str]) -> Table {
        let mut params = Table::new();
        params.insert(
            "argv".to_string(),
            Value::Array(
                argv.iter()
                    .map(|value| Value::String((*value).to_string()))
                    .collect(),
            ),
        );
        params.insert("cwd".to_string(), Value::String("workspace".to_string()));
        params.insert(
            "expect".to_string(),
            Value::Table(Table::from_iter([
                ("exit_code".to_string(), Value::Integer(0)),
                ("max_bytes".to_string(), Value::Integer(4096)),
            ])),
        );
        params
    }

    #[test]
    fn catalog_schema_resolves_only_the_closed_command_shape() {
        assert!(matches!(
            crate::planner::capability_catalog::resolve(ID, &params(&["test", "-f", "index.html"])),
            Ok(ResolvedCapability::CommandCheck(_))
        ));

        let mut free_shell = params(&["test", "-f", "index.html"]);
        free_shell.insert(
            "command".to_string(),
            Value::String("sh -c 'true'".to_string()),
        );
        assert!(matches!(
            crate::planner::capability_catalog::resolve(ID, &free_shell),
            Err(CatalogError::UnknownParameter { parameter, .. }) if parameter == "command"
        ));

        let mut bad_expect = params(&["test", "-f", "index.html"]);
        bad_expect
            .get_mut("expect")
            .and_then(Value::as_table_mut)
            .unwrap()
            .insert(
                "promote_assurance".to_string(),
                Value::String("full".to_string()),
            );
        assert!(matches!(
            crate::planner::capability_catalog::resolve(ID, &bad_expect),
            Err(CatalogError::InvalidParameter { parameter, .. }) if parameter == "expect"
        ));
    }

    #[test]
    fn direct_argv_check_records_pass_event_summary_and_keeps_draft_static() {
        let root = tempfile::tempdir().unwrap();
        let events = root.path().join("run/events.jsonl");
        let summary = run_and_record(
            root.path(),
            &[CommandCheckBinding {
                id: ID.to_string(),
                check: check(&["printf", "green-tea\\n"], Some("green-tea"), 128),
            }],
            Some(&events),
            "draft_profile",
            "static-site",
        );
        assert!(summary.passed);
        let emitted = std::fs::read_to_string(&events).unwrap();
        assert!(emitted.contains(r#""event":"declarative_command_check_result""#));
        assert!(emitted.contains(r#""status":"passed""#));
        assert!(emitted.contains(r#""source":"draft_profile""#));
        let rendered =
            std::fs::read_to_string(events.parent().unwrap().join("summary.md")).unwrap();
        assert!(rendered.contains("Declarative command checks"));
        assert!(rendered.contains("1/1 passed"));

        let mut level = "full".to_string();
        let mut reason = String::new();
        crate::planner::profile_admission::cap_assurance_for_status(
            ManifestStatus::Draft,
            &mut level,
            &mut reason,
        );
        assert_eq!(level, "static");
        assert_eq!(reason, "profile_not_admitted");
    }

    #[test]
    fn failures_and_timeout_are_honest_and_output_is_bounded() {
        let root = tempfile::tempdir().unwrap();
        let failed = run_and_record(
            root.path(),
            &[
                CommandCheckBinding {
                    id: ID.to_string(),
                    check: check(&["printf", "0123456789"], Some("missing"), 4),
                },
                CommandCheckBinding {
                    id: ID.to_string(),
                    check: check(&["sleep", "1"], None, 32),
                },
            ],
            None,
            "draft_profile",
            "static-site",
        );
        assert!(!failed.passed);
        assert_eq!(failed.failed_count, 2);
        assert!(
            failed.results[0]
                .reasons
                .iter()
                .any(|reason| reason.contains("max_bytes"))
        );
        assert!(
            failed.results[0]
                .reasons
                .iter()
                .any(|reason| reason.contains("stdout"))
        );
        assert!(failed.results[1].timed_out);
    }

    #[test]
    fn declaration_rejects_shell_eval_mutation_and_workspace_escape() {
        for argv in [
            vec!["sh", "-c", "test -f index.html"],
            vec!["python3", "-c", "print('pass')"],
            vec!["python3", "-cprint('pass')"],
            vec!["node", "-p", "process.version"],
            vec!["nice", "sh", "-c", "true"],
            vec!["busybox", "sh", "-c", "true"],
            vec!["rm", "index.html"],
            vec!["env", "sh", "-c", "true"],
            vec!["git", "reset", "--hard"],
            vec!["git", "--no-pager", "reset", "--hard"],
            vec!["find", ".", "-delete"],
            vec!["test", "-f", "../index.html"],
            vec!["test", "-f", "/tmp/index.html"],
        ] {
            let values = argv.into_iter().map(str::to_string).collect();
            assert!(
                DeclarativeCommandCheck::new(values, 0, None, 128).is_err(),
                "argv should be rejected"
            );
        }
    }

    #[test]
    fn manifest_command_check_is_final_acceptance_only() {
        let source = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/corpus/apps/issue250-declarative-command-checks/extension-root/profiles/",
            "static-site/manifest.toml"
        ));
        let phase_bound = source.replace(
            "id = \"command_check\"",
            "id = \"command_check\"\nphases = [\"implementation\"]",
        );
        let error = crate::planner::profile_manifest::external_manifest_from_toml(&phase_bound)
            .unwrap_err()
            .to_string();
        assert!(error.contains("final acceptance"), "{error}");
    }
}
