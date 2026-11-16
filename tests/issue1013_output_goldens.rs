//! Issue #1013, acceptance 4: byte goldens of outputs that `--operator-check` must leave as they
//! are. They were captured from the binary before #1013 changed any code, and are never
//! regenerated: a mismatch means the change is wrong.
//!
//! Each command runs with an empty environment, a fake `HOME`, an empty or minimal `PATH`, and
//! `LC_ALL=C LANG=C`, so the host's locale, keys, and configuration files change nothing. Only
//! these host-dependent parts are normalised, and nothing else:
//!
//! - the per-test temporary directory, written as `<ROOT>`, and the repository root, written as
//!   `<REPO>`;
//! - `duration_secs` in the run summary, the wall clock of the run, written as `<DURATION>`;
//! - the doctor's `interaction.playwright` check, which reports whether Playwright is installed on
//!   the host, replaced as a whole by one `<HOST:interaction.playwright>` line.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const ROOT: &str = "<ROOT>";
const REPO: &str = "<REPO>";

fn golden(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden/issue1013")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("{}: {err}", path.display()))
}

/// A temporary root by its canonical path, so the binary sees the same spelling the test replaces.
fn root() -> (tempfile::TempDir, PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let path = std::fs::canonicalize(temp.path()).unwrap();
    (temp, path)
}

fn command(root: &Path, path: &str) -> Command {
    let home = root.join("home");
    std::fs::create_dir_all(&home).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_commandagent"));
    command
        .env_clear()
        .env("HOME", home)
        .env("PATH", path)
        .env("LC_ALL", "C")
        .env("LANG", "C");
    command
}

fn text(bytes: &[u8], root: &Path) -> String {
    String::from_utf8(bytes.to_vec())
        .unwrap()
        .replace(&root.display().to_string(), ROOT)
        .replace(env!("CARGO_MANIFEST_DIR"), REPO)
}

/// `"duration_secs":<number>` becomes `"duration_secs":<DURATION>`.
fn without_duration(text: &str) -> String {
    const KEY: &str = "\"duration_secs\":";
    let Some(start) = text.find(KEY) else {
        return text.to_string();
    };
    let value = start + KEY.len();
    let end = text[value..]
        .find([',', '}'])
        .map_or(text.len(), |offset| value + offset);
    format!("{}<DURATION>{}", &text[..value], &text[end..])
}

/// The pretty-printed check whose `id` is `id`, from its `{` line to its `}` line, becomes one
/// placeholder line.
fn without_check(text: &str, id: &str) -> String {
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let marker = format!("      \"id\": \"{id}\",\n");
    let Some(at) = lines.iter().position(|line| *line == marker) else {
        return text.to_string();
    };
    let start = (0..at)
        .rev()
        .find(|&index| lines[index] == "    {\n")
        .expect("the check's opening line");
    let end = (at..lines.len())
        .find(|&index| lines[index].starts_with("    }"))
        .expect("the check's closing line");
    let comma = if lines[end].starts_with("    },") {
        ","
    } else {
        ""
    };
    let mut out = String::new();
    out.extend(lines[..start].iter().copied());
    out.push_str(&format!("    <HOST:{id}>{comma}\n"));
    out.extend(lines[end + 1..].iter().copied());
    out
}

fn run_summary_stdout() -> (Output, String) {
    let (_temp, root) = root();
    let workspace = root.join("ws");
    let state = workspace.join("state");
    std::fs::create_dir_all(&state).unwrap();
    let output = command(&root, "/usr/bin:/bin")
        .env(
            "COMMANDAGENT_EVAL_EVENTS",
            workspace.join("run-evidence/events.jsonl"),
        )
        .args([
            "--offline",
            "--provider",
            "openai",
            "--model",
            "m-1013",
            "--planner-provider",
            "openai",
            "--planner-model",
            "m-1013",
            "--prompt",
            "hello",
            "--no-footer",
            "--summary-json",
            "--cwd",
        ])
        .arg(&workspace)
        .arg("--state-dir")
        .arg(&state)
        .output()
        .unwrap();
    let stdout = without_duration(&text(&output.stdout, &root));
    (output, stdout)
}

fn doctor_json_stdout() -> (Output, String) {
    let (_temp, root) = root();
    let workspace = root.join("ws");
    let state = workspace.join("state");
    std::fs::create_dir_all(&state).unwrap();
    let output = command(&root, "")
        .args([
            "--doctor",
            "--json",
            "--offline",
            "--provider",
            "openai",
            "--planner-provider",
            "openai",
            "--model",
            "m-1013",
            "--planner-model",
            "m-1013",
            "--cwd",
        ])
        .arg(&workspace)
        .arg("--state-dir")
        .arg(&state)
        .output()
        .unwrap();
    let stdout = without_check(&text(&output.stdout, &root), "interaction.playwright");
    (output, stdout)
}

fn valid_manifest() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(
        "tests/corpus/apps/issue248-manifest-v2/extension-root/profiles/static-site/manifest.toml",
    )
}

fn validate_manifest(path: &Path, root: &Path) -> (Output, String, String) {
    let output = command(root, "/usr/bin:/bin")
        .arg("--validate-manifest")
        .arg(path)
        .output()
        .unwrap();
    let stdout = text(&output.stdout, root);
    let stderr = text(&output.stderr, root);
    (output, stdout, stderr)
}

/// A run that stops before any provider call, as the headless contract projects it.
#[test]
fn run_summary_json_stdout_matches_the_pre_1013_golden() {
    let (output, stdout) = run_summary_stdout();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stdout, golden("run-summary.stdout"));
}

#[test]
fn doctor_json_stdout_matches_the_pre_1013_golden() {
    let (output, stdout) = doctor_json_stdout();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stdout, golden("doctor.json.stdout"));
}

#[test]
fn validate_manifest_outputs_match_the_pre_1013_golden() {
    let (_temp, root) = root();
    let (output, stdout, stderr) = validate_manifest(&valid_manifest(), &root);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stdout, golden("validate-manifest.valid.stdout"));
    assert_eq!(stderr, "");

    let profile = root.join("extension/profiles/broken");
    std::fs::create_dir_all(&profile).unwrap();
    let invalid = profile.join("manifest.toml");
    std::fs::write(&invalid, "[metadata]\nid = \"broken\"\n[metadata]\n").unwrap();
    let (output, stdout, stderr) = validate_manifest(&invalid, &root);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stdout, "");
    assert_eq!(stderr, golden("validate-manifest.invalid.stderr"));
}
