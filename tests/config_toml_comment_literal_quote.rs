//! Issue #941: config comment removal treats a TOML literal string (`'...'`)
//! as a string, so a `#` inside it is not a comment and a `"` inside it does
//! not open a basic string. A line the line-based reader cannot interpret (a
//! multi-line or unclosed string) fails with a fixed diagnostic that names the
//! file and line only.
//!
//! Every `Config` case runs in a child copy of this test binary with a cleared
//! environment, a temporary `HOME`, and a temporary workspace (`child_case`);
//! every doctor case runs the `commandagent` binary the same way. No real
//! config file, dotenv, or credential is read, nothing is written outside the
//! temporary directories, and no provider is contacted: every host is the
//! closed loopback port 9.

use std::path::Path;
use std::process::Command;

use clap::Parser;
use commandagent::cli::Cli;
use commandagent::config::{Config, NarrationMode};
use commandagent::minimal_loop::request_pattern::{RequestPatternMode, settings};

const CASE_ENV: &str = "COMMANDAGENT_ISSUE941_CHILD_CASE";
const CHILD_TEST: &str = "child_case";
const OK: &str = "ISSUE941_OK";
const PRESET: &str = "routed";
/// A parent preset name that must never reach a refused-form diagnostic.
const PARENT: &str = "PRIVATE_PARENT_941";
const MARKER: &str = "PRIVATE_MARKER_941";
const VALUE: &str = "PRIVATE_VALUE_941";
const KEY_ENV: &str = "COMMANDAGENT_ISSUE941_FAKE_KEY";
const FAKE_SECRET: &str = "fake-secret-941-not-a-real-key";
const BASE_URL: &str = "http://127.0.0.1:9/v1";
/// The fixed text of `UNINTERPRETABLE_LINE`, without the location.
const UNINTERPRETABLE: &str = "has a multi-line or unclosed string that the config reader does not support (the key and value are not shown)";
const COMPLETE_KEYS: &str = "context_budget = 4096\nchat_timeout_secs = 10\nplan_preset = \"none\"\nprofile = \"generic\"\nnarration = \"quiet\"\nfooter = \"off\"\nstream = \"off\"\n";

// ---------------------------------------------------------------- parent side

struct Output {
    success: bool,
    text: String,
}

/// Runs the child case `case` with `files` (path relative to the temporary
/// directory, content) written first. `home/` is the child's `HOME` and
/// `work/` its workspace.
fn run_child(case: &str, files: &[(&str, String)]) -> String {
    let dir = tempfile::tempdir().expect("tempdir");
    write_files(dir.path(), files);
    let output = Command::new(std::env::current_exe().expect("test binary"))
        .args(["--exact", CHILD_TEST, "--nocapture", "--test-threads=1"])
        .env_clear()
        .env("HOME", dir.path().join("home"))
        .env(CASE_ENV, case)
        .env(KEY_ENV, FAKE_SECRET)
        .current_dir(dir.path().join("work"))
        .output()
        .expect("child process");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success() && stdout.contains("1 passed") && stdout.contains(OK),
        "child case {case} failed:\n{stdout}\n{stderr}"
    );
    stdout
}

/// Runs `commandagent --doctor --json --preset routed` the same way.
fn run_doctor(files: &[(&str, String)]) -> Output {
    let dir = tempfile::tempdir().expect("tempdir");
    write_files(dir.path(), files);
    let work = dir.path().join("work");
    let state = dir.path().join("state");
    std::fs::create_dir_all(&state).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_commandagent"))
        .args(["--doctor", "--json", "--preset", PRESET])
        .args(["--ollama-host", "http://127.0.0.1:9"])
        .args(["--lm-studio-host", "http://127.0.0.1:9"])
        .args(["--cwd", work.to_str().unwrap()])
        .args(["--state-dir", state.to_str().unwrap()])
        .env_clear()
        .env("HOME", dir.path().join("home"))
        .env(KEY_ENV, FAKE_SECRET)
        .current_dir(&work)
        .output()
        .expect("doctor process");
    Output {
        success: output.status.success(),
        text: format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ),
    }
}

fn write_files(root: &Path, files: &[(&str, String)]) {
    std::fs::create_dir_all(root.join("home")).unwrap();
    std::fs::create_dir_all(root.join("work")).unwrap();
    for (path, text) in files {
        let path = root.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
}

/// A complete `routed` preset that extends `PARENT`, so the parent is reached
/// only through the recursive `extends` resolution.
fn complete_child() -> String {
    format!(
        "[preset.{PRESET}]\nextends = \"{PARENT}\"\nmodel = \"m941\"\nprovider = \"openai-compatible\"\nplanner_model = \"p941\"\nplanner_provider = \"openai-compatible\"\nbase_url = \"{BASE_URL}\"\napi_key_env = \"{KEY_ENV}\"\n{COMPLETE_KEYS}"
    )
}

/// The parent layer path for `layer`.
fn parent_path(layer: &str) -> &'static str {
    if layer == "home" {
        "home/.commandagent/config.toml"
    } else {
        "work/.anvil/config.toml"
    }
}

/// Parent preset forms whose key or header holds a `#` inside a literal
/// string; each must be refused with the routing form's fixed text.
fn refused_parent_forms() -> Vec<String> {
    vec![
        format!("[preset.{PARENT}]\nopenrouter_provider.'{MARKER}#=0' = [\"a\"]\n"),
        format!("[preset.{PARENT}]\nopenrouter_provider.'{MARKER}#' = [\"a\"] # 'c'\n"),
        format!(
            "[preset.{PARENT}]\nmodel = \"x\"\n[preset.{PARENT}.openrouter_provider.'{MARKER}#=0']\nonly = [\"a\"]\n"
        ),
        format!("[preset.{PARENT}]\nopenrouter_provider.'{MARKER}#=0' = \"\"\"\n{VALUE}\n\"\"\"\n"),
    ]
}

fn assert_hidden(text: &str, hidden: &[&str]) {
    for hidden in hidden {
        assert!(!text.contains(hidden), "{hidden} leaked: {text}");
    }
}

/// A preset with literal and basic strings around real comments, whose
/// routing table is `routing`.
fn routing_preset(routing: &str) -> String {
    format!(
        "[preset.{PRESET}] # it's \"the\" preset\nmodel = \"m\\\"#941\" # 'c'\nprovider = \"openai-compatible\"\nplanner_model = \"p941\"\nplanner_provider = \"openai-compatible\"\nbase_url = \"{BASE_URL}\"\n{COMPLETE_KEYS}openrouter_provider = {routing} # trailing 'comment' \"here\"\n[tool] # an unread section\n'a#b' = 'c\"d' # e\n"
    )
}

/// Verification 2: a preset with literal strings, a basic string's `\"`
/// escape before a `#`, and a literal quoted key with `#` in another section
/// is read through the public `Config` path.
#[test]
fn literal_strings_in_a_preset_file_are_read_whole() {
    let config = routing_preset(
        "{ only = ['deep-seek', 'open.ai/x'], order = [\"x-y\"], 'allow_fallbacks' = false }",
    );
    run_child(
        "routing-read",
        &[("work/.commandagent/config.toml", config)],
    );
}

/// Verification 2: a literal routing value with `#` reaches the slug rule
/// whole, which refuses it without the value, instead of a cut table.
#[test]
fn a_literal_routing_value_with_a_hash_reaches_the_slug_rule_whole() {
    let config = routing_preset(&format!("{{ only = ['ok', '{MARKER}#x'] }}"));
    run_child(
        "routing-slug",
        &[("work/.commandagent/config.toml", config)],
    );
}

/// Verification 2: a literal quoted key with `#` in another section no
/// longer breaks the file, so the top-level setting of the same file is read.
#[test]
fn a_literal_string_with_a_hash_does_not_hide_top_level_settings() {
    let config = "narration = \"quiet\" # it's\n[tool] # don't\n'a#b' = 'c\"d' # e\n".to_string();
    run_child(
        "top-level-read",
        &[("work/.commandagent/config.toml", config)],
    );
}

/// Verification 2: config validation reads a quoted top-level key with `#`
/// whole and refuses it as an unknown key instead of skipping a cut line.
#[test]
fn config_validation_reads_a_literal_quoted_key_whole() {
    run_child(
        "validation-key",
        &[(
            "work/.commandagent/config.toml",
            "'unknown#key_941' = 1\n".to_string(),
        )],
    );
}

/// Verification 2: the request pattern settings use the same comment rule.
#[test]
fn request_pattern_settings_use_the_same_comment_rule() {
    let config = "request_pattern = \"jev\" # it's 'jev' \"here\"\nrequest_pattern_threshold = 0.25 # 'a\"b'\n[tool] # don't\n'a#b' = 'c\"d' # e\n".to_string();
    run_child(
        "request-pattern",
        &[("work/.commandagent/config.toml", config)],
    );
}

/// Verification 2 and the #939 security recheck 3: a refused routing form
/// whose key or header has a `#` inside a literal string, in a parent preset
/// of another layer, never names the parent, the marker, or the value through
/// the public `Config` path.
#[test]
fn config_never_names_the_parent_of_a_refused_literal_form() {
    for layer in ["anvil", "home"] {
        for (index, form) in refused_parent_forms().into_iter().enumerate() {
            run_child(
                &format!("refused-parent:{index}"),
                &[
                    ("work/.commandagent/config.toml", complete_child()),
                    (parent_path(layer), form),
                ],
            );
        }
    }
}

/// The same forms through doctor, which also inspects every layer and the
/// recursive resolution.
#[test]
fn doctor_never_names_the_parent_of_a_refused_literal_form() {
    for layer in ["anvil", "home"] {
        for form in refused_parent_forms() {
            let output = run_doctor(&[
                ("work/.commandagent/config.toml", complete_child()),
                (parent_path(layer), form),
            ]);
            assert!(!output.success, "{}", output.text);
            assert_hidden(
                &output.text,
                &[PARENT, MARKER, VALUE, FAKE_SECRET, "extends '"],
            );
            assert!(output.text.contains("not shown"), "{}", output.text);
        }
    }
}

/// Lines the reader cannot interpret, each with its config file path.
fn uninterpretable_cases() -> Vec<(&'static str, String)> {
    vec![
        // A top-level multi-line value that would otherwise be read as `"`.
        (
            "work/.commandagent/config.toml",
            format!("extension_root = \"\"\"\n{VALUE}\n\"\"\"\n"),
        ),
        // Its body would otherwise open a section.
        (
            "work/.commandagent/config.toml",
            format!("extension_root = '''\n[preset.{MARKER}]\nmodel = \"{VALUE}\"\n'''\n"),
        ),
        // A one-line multi-line string with a `#` and an `${ENV}` reference.
        (
            "work/.commandagent/config.toml",
            format!("[preset.{PRESET}]\nmodel = \"\"\"{VALUE}#${{{KEY_ENV}}}\"\"\"\n"),
        ),
        // In a parent preset of another layer.
        (
            "work/.anvil/config.toml",
            format!("[preset.{PARENT}]\nmodel = '''\n{VALUE}\n'''\n"),
        ),
        (
            "home/.commandagent/config.toml",
            format!("[preset.{PARENT}]\nmodel = \"\"\"\n{VALUE} ${{{KEY_ENV}}}\n\"\"\"\n"),
        ),
    ]
}

/// Acceptance: an uninterpretable line fails startup with the fixed text,
/// naming the file and line only, never the key, the value, the parent preset,
/// or an expanded secret, and never adopting a cut value.
#[test]
fn an_uninterpretable_line_fails_startup_with_only_its_location() {
    for (index, (path, text)) in uninterpretable_cases().into_iter().enumerate() {
        let mut files = vec![(path, text)];
        if path != "work/.commandagent/config.toml" {
            files.push(("work/.commandagent/config.toml", complete_child()));
        }
        run_child(&format!("uninterpretable:{index}"), &files);
    }
}

/// Doctor reports an uninterpretable line without the value or a secret.
#[test]
fn doctor_reports_an_uninterpretable_line_without_the_value() {
    for (path, text) in uninterpretable_cases() {
        let mut files = vec![(path, text)];
        if path != "work/.commandagent/config.toml" {
            files.push(("work/.commandagent/config.toml", complete_child()));
        }
        let output = run_doctor(&files);
        assert!(!output.success, "{}", output.text);
        assert_hidden(&output.text, &[VALUE, MARKER, FAKE_SECRET]);
        assert!(output.text.contains(UNINTERPRETABLE), "{}", output.text);
    }
}

// ----------------------------------------------------------------- child side

/// Runs one case selected by `CASE_ENV`; a no-op in the parent run.
#[test]
fn child_case() {
    let Some(case) = std::env::var_os(CASE_ENV) else {
        return;
    };
    let case = case.into_string().expect("case name");
    let root = std::env::current_dir()
        .expect("child workspace")
        .canonicalize()
        .unwrap();
    let (name, index) = case.split_once(':').unwrap_or((case.as_str(), ""));
    match name {
        "routing-read" => routing_read(&root),
        "routing-slug" => {
            let error = format!(
                "{:#}",
                load(&root, Some(PRESET)).map(drop).expect_err("slug")
            );
            assert!(
                error.contains(".commandagent/config.toml:14 preset.routed.openrouter_provider.only entry 2 is not a provider slug"),
                "{error}"
            );
            assert_hidden(&error, &[MARKER, FAKE_SECRET]);
        }
        "top-level-read" => {
            let config = load(&root, None).expect("config");
            assert_eq!(config.narration, NarrationMode::Quiet);
        }
        "validation-key" => {
            let error = format!(
                "{:#}",
                load(&root, None).map(drop).expect_err("unknown key")
            );
            assert!(
                error
                    .contains(".commandagent/config.toml:1 unknown config key ''unknown#key_941''"),
                "{error}"
            );
        }
        "request-pattern" => {
            load(&root, None).expect("config");
            let resolved = settings(None, &root).expect("request pattern settings");
            assert_eq!(resolved.mode, RequestPatternMode::Jev);
            assert_eq!(resolved.threshold, 0.25);
        }
        "refused-parent" => {
            let error = format!(
                "{:#}",
                load(&root, Some(PRESET)).map(drop).expect_err(index)
            );
            assert_hidden(&error, &[PARENT, MARKER, VALUE, FAKE_SECRET, "extends '"]);
            assert!(error.contains("not shown"), "{error}");
        }
        "uninterpretable" => {
            let index: usize = index.parse().unwrap();
            let (path, text) = &uninterpretable_cases()[index];
            let line = 1 + text
                .lines()
                .position(|line| line.contains("\"\"\"") || line.contains("'''"))
                .unwrap();
            let error = format!(
                "{:#}",
                load(&root, Some(PRESET))
                    .map(drop)
                    .expect_err("uninterpretable")
            );
            let suffix = path.split_once('/').unwrap().1;
            assert!(
                error.ends_with(&format!("{suffix}:{line} {UNINTERPRETABLE}")),
                "{error}"
            );
            assert_hidden(
                &error,
                &[
                    PARENT,
                    MARKER,
                    VALUE,
                    FAKE_SECRET,
                    "extends '",
                    "model",
                    "extension_root",
                ],
            );
        }
        other => panic!("unknown child case {other}"),
    }
    println!("{OK}");
}

fn load(root: &Path, preset: Option<&str>) -> anyhow::Result<Config> {
    let mut arguments = vec![
        "commandagent".to_string(),
        "--cwd".to_string(),
        root.to_string_lossy().into_owned(),
        "--state-dir".to_string(),
        root.join("state").to_string_lossy().into_owned(),
        "--ollama-host".to_string(),
        "http://127.0.0.1:9".to_string(),
        "--lm-studio-host".to_string(),
        "http://127.0.0.1:9".to_string(),
    ];
    if let Some(preset) = preset {
        arguments.push("--preset".to_string());
        arguments.push(preset.to_string());
    }
    Config::from_cli(Cli::parse_from(arguments))
}

fn routing_read(root: &Path) {
    let config = load(root, Some(PRESET)).expect("config");
    let routing = &config
        .openai_compatible
        .as_ref()
        .expect("openai-compatible")
        .openrouter_provider()
        .expect("routing")
        .routing;
    assert_eq!(
        routing.only(),
        Some(&["deep-seek".to_string(), "open.ai/x".to_string()][..])
    );
    assert_eq!(routing.order(), Some(&["x-y".to_string()][..]));
    assert_eq!(config.model, "m\"#941");
}
