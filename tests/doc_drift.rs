use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use clap::CommandFactory;
use commandagent::cli::Cli;
use commandagent::config::{SUPPORTED_PRESET_KEYS, SUPPORTED_TOP_LEVEL_KEYS};
use commandagent::tui::slash::{SLASH_COMMANDS, render_help};

const CLI_DOC: &str = "docs/guide/en/cli-reference.md";
const SLASH_DOC: &str = "docs/guide/en/slash-commands.md";
const CONFIG_DOC: &str = "docs/guide/en/configuration.md";

fn repo_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
}

fn read_repo_file(relative: &str) -> String {
    fs::read_to_string(repo_path(relative))
        .unwrap_or_else(|err| panic!("failed to read {relative}: {err}"))
}

fn first_cell_entries(markdown: &str, prefix: &str) -> BTreeSet<String> {
    markdown
        .lines()
        .filter_map(|line| {
            let cell = line.strip_prefix("| `")?.split_once('`')?.0;
            cell.starts_with(prefix).then(|| cell.to_string())
        })
        .collect()
}

fn markdown_section<'a>(markdown: &'a str, heading: &str, path: &str) -> &'a str {
    let marker = format!("{heading}\n");
    let (_, after_heading) = markdown
        .split_once(&marker)
        .unwrap_or_else(|| panic!("missing heading '{heading}' in {path}"));
    let end = after_heading.find("\n## ").unwrap_or(after_heading.len());
    &after_heading[..end]
}

fn assert_same_entries(
    label: &str,
    expected: &BTreeSet<String>,
    actual: &BTreeSet<String>,
    expected_path: &str,
    actual_path: &str,
) {
    let missing_from_actual = expected.difference(actual).cloned().collect::<Vec<_>>();
    let missing_from_expected = actual.difference(expected).cloned().collect::<Vec<_>>();
    if missing_from_actual.is_empty() && missing_from_expected.is_empty() {
        return;
    }

    let mut failures = Vec::new();
    if !missing_from_actual.is_empty() {
        failures.push(format!(
            "missing from {actual_path}: {}",
            missing_from_actual.join(", ")
        ));
    }
    if !missing_from_expected.is_empty() {
        failures.push(format!(
            "missing from {expected_path}: {}",
            missing_from_expected.join(", ")
        ));
    }
    panic!(
        "{label} drift detected:\n- {}\nFix {expected_path} or {actual_path} so both sides match.",
        failures.join("\n- ")
    );
}

#[test]
fn public_cli_flags_match_english_reference() {
    let code_flags = Cli::command()
        .get_arguments()
        .filter(|argument| !argument.is_hide_set())
        .filter_map(|argument| argument.get_long())
        .map(|long| format!("--{long}"))
        .collect::<BTreeSet<_>>();
    let documented_flags = first_cell_entries(&read_repo_file(CLI_DOC), "--");

    assert_same_entries(
        "public CLI flag",
        &code_flags,
        &documented_flags,
        "src/cli.rs (Cli::command())",
        CLI_DOC,
    );
}

fn rendered_help_commands() -> BTreeSet<String> {
    let help = render_help();
    help.lines()
        .filter_map(|line| line.split_once(" - ").map(|(usage, _)| usage))
        .flat_map(str::split_whitespace)
        .filter(|token| token.starts_with('/'))
        .map(|token| {
            token
                .trim_end_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != '-')
                .to_string()
        })
        .collect()
}

#[test]
fn slash_commands_match_rendered_help_dispatch_and_english_reference() {
    let dispatch_commands = SLASH_COMMANDS
        .iter()
        .flat_map(|spec| std::iter::once(spec.name).chain(spec.aliases.iter().copied()))
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    let help_commands = rendered_help_commands();
    assert_same_entries(
        "slash-command help/dispatch",
        &dispatch_commands,
        &help_commands,
        "src/tui/slash.rs (SLASH_COMMANDS/handle_command)",
        "src/tui/slash.rs (render_help)",
    );

    let documented_commands = first_cell_entries(&read_repo_file(SLASH_DOC), "/");
    assert_same_entries(
        "slash command",
        &help_commands,
        &documented_commands,
        "src/tui/slash.rs (render_help)",
        SLASH_DOC,
    );
}

fn string_set(values: &[&str]) -> BTreeSet<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

#[test]
fn configuration_keys_match_english_reference() {
    let markdown = read_repo_file(CONFIG_DOC);
    let preset_keys = first_cell_entries(markdown_section(&markdown, "## Presets", CONFIG_DOC), "");
    assert_same_entries(
        "preset configuration key",
        &string_set(SUPPORTED_PRESET_KEYS),
        &preset_keys,
        "src/config.rs (SUPPORTED_PRESET_KEYS)",
        CONFIG_DOC,
    );

    let top_level_keys = first_cell_entries(
        markdown_section(&markdown, "## Top-level keys", CONFIG_DOC),
        "",
    );
    assert_same_entries(
        "top-level configuration key",
        &string_set(SUPPORTED_TOP_LEVEL_KEYS),
        &top_level_keys,
        "src/config.rs (SUPPORTED_TOP_LEVEL_KEYS)",
        CONFIG_DOC,
    );
}

fn guide_files(relative_directory: &str) -> BTreeSet<String> {
    fs::read_dir(repo_path(relative_directory))
        .unwrap_or_else(|err| panic!("failed to read {relative_directory}: {err}"))
        .map(|entry| {
            entry.unwrap_or_else(|err| panic!("failed to read {relative_directory}: {err}"))
        })
        .filter(|entry| {
            entry
                .file_type()
                .unwrap_or_else(|err| {
                    panic!(
                        "failed to read file type for {}: {err}",
                        entry.path().display()
                    )
                })
                .is_file()
        })
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect()
}

fn heading_counts(markdown: &str) -> (usize, usize) {
    markdown.lines().fold((0, 0), |(h2, h3), line| {
        (
            h2 + usize::from(line.starts_with("## ")),
            h3 + usize::from(line.starts_with("### ")),
        )
    })
}

#[test]
fn english_and_japanese_guides_have_matching_files_and_heading_counts() {
    let en_directory = "docs/guide/en";
    let ja_directory = "docs/guide/ja";
    let en_files = guide_files(en_directory);
    let ja_files = guide_files(ja_directory);
    assert_same_entries(
        "EN/JA guide file",
        &en_files,
        &ja_files,
        en_directory,
        ja_directory,
    );

    let mut failures = Vec::new();
    for file in &en_files {
        let en_path = format!("{en_directory}/{file}");
        let ja_path = format!("{ja_directory}/{file}");
        let en_counts = heading_counts(&read_repo_file(&en_path));
        let ja_counts = heading_counts(&read_repo_file(&ja_path));
        if en_counts != ja_counts {
            failures.push(format!(
                "{en_path} has H2/H3 counts {en_counts:?}; {ja_path} has {ja_counts:?}"
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "EN/JA guide heading drift detected:\n- {}\nFix the named translation pair(s) under {en_directory} and {ja_directory}.",
        failures.join("\n- ")
    );
}

#[test]
fn demo_docs_distinguish_scripted_assets_from_provider_backed_recording() {
    let english = read_repo_file("README.md");
    let japanese = read_repo_file("README.ja.md");
    let notes = read_repo_file("docs/assets/ux-demo.md");
    let recording = repo_path("docs/assets/repl-ultra-plan-run.rec");

    for (path, readme, offline_marker) in [
        ("README.md", english, "offline"),
        ("README.ja.md", japanese, "オフライン"),
    ] {
        assert!(
            readme.contains("--ux-demo")
                && readme.contains(offline_marker)
                && readme.contains("provider-backed")
                && readme.contains("SVG"),
            "{path} must identify --ux-demo and the SVG as scripted rather than provider-backed"
        );
    }
    assert!(
        notes.contains("repl-ultra-plan-run.rec")
            && notes.contains("provider-backed")
            && notes.contains("script -p"),
        "recording notes must identify and explain replay of the provider-backed capture"
    );
    let capture = fs::read(&recording)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", recording.display()));
    assert!(capture.len() > 1_024, "recording should be non-trivial");
    for marker in [
        b"Accepted command".as_slice(),
        b"- Requested port: 3011 (goal)",
        b"UltraPlan accepted:",
        b"planning the overall plan",
        b"planning steps",
        b"interrupt requested; aborting current operation",
        b"Recovery UltraPlan",
        b"Active command: /ultra-plan-run",
        b"Current scope: interrupt requested",
        b"\x1b[1;22r",
        b"\x1b[s\x1b[r",
    ] {
        assert!(
            capture.windows(marker.len()).any(|window| window == marker),
            "provider-backed recording is missing marker {:?}",
            String::from_utf8_lossy(marker)
        );
    }
}

#[test]
fn pty_test_recipe_matches_contributing_command_and_includes_ignored_tests() {
    let justfile = read_repo_file("justfile");
    let contributing = read_repo_file("CONTRIBUTING.md");
    let (_, recipe_body) = justfile
        .split_once("test-pty:\n")
        .expect("justfile must define the test-pty recipe");
    let recipe_command = recipe_body
        .lines()
        .find_map(|line| line.strip_prefix("    "))
        .map(str::trim)
        .expect("test-pty recipe must contain a command");

    assert!(
        recipe_command.contains(" -- --include-ignored"),
        "test-pty must pass --include-ignored to libtest; command={recipe_command:?}"
    );
    assert!(
        contributing
            .lines()
            .any(|line| line.trim() == recipe_command),
        "CONTRIBUTING.md must document the test-pty command exactly; command={recipe_command:?}"
    );
}
