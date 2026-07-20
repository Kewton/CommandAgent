use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use clap::CommandFactory;
use commandagent::cli::Cli;
use commandagent::config::{SUPPORTED_PRESET_KEYS, SUPPORTED_TOP_LEVEL_KEYS};
use commandagent::tui::slash::{SLASH_COMMANDS, render_help};

const CLI_DOC: &str = "docs/guide/en/cli-reference.md";
const SLASH_DOC: &str = "docs/guide/en/slash-commands.md";
const JA_SLASH_DOC: &str = "docs/guide/ja/slash-commands.md";
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

#[test]
fn repl_multiline_continuation_is_documented_in_help_and_bilingual_guides() {
    const HELP_LINE: &str = "Multi-line input: end a line with \\ or leave a double quote open; Enter continues at the ... prompt. Submit with quotes closed and no trailing \\.";

    let help = render_help();
    assert!(
        help.lines().any(|line| line == HELP_LINE),
        "render_help must describe both continuation triggers and how to submit:\n{help}"
    );

    let english = read_repo_file(SLASH_DOC);
    let english = markdown_section(&english, "## Multi-line input", SLASH_DOC);
    for marker in [
        "end a line with `\\`",
        "leave a double quote",
        "`... `",
        "does not end in `\\`",
        "removes each trailing\ncontinuation `\\`",
        "joins the lines with single spaces",
    ] {
        assert!(
            english.contains(marker),
            "{SLASH_DOC} multi-line section is missing '{marker}'"
        );
    }

    let japanese = read_repo_file(JA_SLASH_DOC);
    let japanese = markdown_section(&japanese, "## 複数行入力", JA_SLASH_DOC);
    for marker in [
        "行末を `\\`",
        "ダブルクォートを閉じず",
        "`... `",
        "末尾が `\\` でない",
        "継続用の末尾 `\\` を削除",
        "各行を 1 個の空白で\n結合",
    ] {
        assert!(
            japanese.contains(marker),
            "{JA_SLASH_DOC} multi-line section is missing '{marker}'"
        );
    }
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
