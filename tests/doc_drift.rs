use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use commandagent::config::{SUPPORTED_PRESET_KEYS, SUPPORTED_TOP_LEVEL_KEYS};
use commandagent::tui::slash::{SLASH_COMMANDS, render_help};

const CLI_DOC: &str = "docs/guide/en/cli-reference.md";
const JA_CLI_DOC: &str = "docs/guide/ja/cli-reference.md";
const PLAN_YAML_DOC: &str = "docs/guide/en/plan-yaml.md";
const JA_PLAN_YAML_DOC: &str = "docs/guide/ja/plan-yaml.md";
const SLASH_DOC: &str = "docs/guide/en/slash-commands.md";
const JA_SLASH_DOC: &str = "docs/guide/ja/slash-commands.md";
const GUIDE_INDEX: &str = "docs/guide/README.md";
const CONFIG_DOC: &str = "docs/guide/en/configuration.md";
const CANONICAL_SAMPLE_GOAL: &str = "Create a CLI --pattern filter command";
const GUI_SAMPLE_GOAL: &str = "--pattern で行を絞り込む CLI コマンドを作ってください";
const ROOT_MARKDOWN_DOCS: &[&str] = &[
    ".devcontainer/README.md",
    "benchmarks/README.md",
    "CHANGELOG.md",
    "CONTRIBUTING.md",
    "PROFILES.md",
    "README.ja.md",
    "README.md",
    "SECURITY.md",
];
const MARKDOWN_DOC_DIRECTORIES: &[&str] = &["docs", "packs"];
const READER_DOCS: &[&str] = &[
    "docs/user/getting-started-cli.md",
    "docs/user/getting-started-gui.md",
    "docs/user/gui-trial.md",
    "docs/user/gui-history.md",
    "docs/user/gui-extensions.md",
    "docs/user/gui-setup.md",
    "docs/user/gui-operations.md",
    "docs/user/gui-help-map.md",
    "docs/dev/extension-catalog.md",
];

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

fn cli_flag_descriptions(markdown: &str) -> BTreeMap<String, String> {
    markdown
        .lines()
        .filter_map(|line| {
            let row = line.strip_prefix("| ")?.strip_suffix(" |")?;
            let cells = row.split(" | ").collect::<Vec<_>>();
            let flag = cells.first()?.strip_prefix('`')?.strip_suffix('`')?;
            flag.starts_with("--")
                .then(|| (flag.to_string(), cells.get(3).unwrap_or(&"").to_string()))
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
fn public_cli_flags_match_bilingual_references_and_advertised_counts() {
    let code_flags = commandagent::provider_cli::command()
        .get_arguments()
        .filter(|argument| !argument.is_hide_set())
        .filter_map(|argument| argument.get_long())
        .map(|long| format!("--{long}"))
        .collect::<BTreeSet<_>>();
    for path in [CLI_DOC, JA_CLI_DOC] {
        let documented_flags = first_cell_entries(&read_repo_file(path), "--");
        assert_same_entries(
            "public CLI flag",
            &code_flags,
            &documented_flags,
            "src/provider_cli.rs (provider_cli::command())",
            path,
        );
    }

    let flag_count = code_flags.len();
    for (path, marker) in [
        (CLI_DOC, format!("{flag_count} application flags below")),
        (JA_CLI_DOC, format!("{flag_count} フラグには含めません")),
        (GUIDE_INDEX, format!("all {flag_count} public flags")),
        (GUIDE_INDEX, format!("全 {flag_count} フラグ")),
    ] {
        assert!(
            read_repo_file(path).contains(&marker),
            "{path} is missing implementation-derived flag count {marker:?}"
        );
    }
}

#[test]
fn public_cli_help_matches_english_reference_descriptions() {
    let documented = cli_flag_descriptions(&read_repo_file(CLI_DOC));
    for argument in commandagent::provider_cli::command()
        .get_arguments()
        .filter(|argument| !argument.is_hide_set())
        .filter(|argument| argument.get_long().is_some())
    {
        let flag = format!("--{}", argument.get_long().unwrap());
        let help = argument
            .get_help()
            .unwrap_or_else(|| panic!("{flag} has no Clap help description"))
            .to_string();
        assert_eq!(
            documented.get(&flag),
            Some(&help),
            "{flag} help differs between the public CLI command and {CLI_DOC}"
        );
    }
}

#[test]
fn bilingual_plan_yaml_guides_pin_the_edit_validate_run_contract() {
    for path in [PLAN_YAML_DOC, JA_PLAN_YAML_DOC] {
        let markdown = read_repo_file(path);
        for marker in [
            "--plan-steps",
            "--ultra-plan",
            "--validate-plan",
            "--run-plan",
            "--run-ultra-plan",
            "path:line:column",
            "Recovery",
        ] {
            assert!(
                markdown.contains(marker),
                "{path} is missing plan YAML contract marker {marker:?}"
            );
        }
    }

    let template = commandagent::planner::plan::render_editable_step_plan(
        &commandagent::planner::step_plan::StepPlan::single("document plan editing"),
    );
    for marker in ["--validate-plan <path>", "--run-plan <path>"] {
        assert!(
            template.contains(marker),
            "saved step-plan template is missing documented marker {marker:?}"
        );
    }
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
fn slash_commands_match_rendered_help_dispatch_and_bilingual_references() {
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

    for path in [SLASH_DOC, JA_SLASH_DOC] {
        let documented_commands = first_cell_entries(&read_repo_file(path), "/");
        assert_same_entries(
            "slash command",
            &help_commands,
            &documented_commands,
            "src/tui/slash.rs (render_help)",
            path,
        );
    }
}

#[test]
fn slash_command_counts_match_registry_and_bilingual_guide_index() {
    let primary_count = SLASH_COMMANDS.len();
    let accepted_count = SLASH_COMMANDS
        .iter()
        .map(|command| 1 + command.aliases.len())
        .sum::<usize>();
    let english = read_repo_file(SLASH_DOC);
    let japanese = read_repo_file(JA_SLASH_DOC);
    let guide_index = read_repo_file(GUIDE_INDEX);

    for marker in [
        format!("contains {primary_count} primary entries"),
        format!("giving {accepted_count} accepted names in total"),
    ] {
        assert!(
            english.contains(&marker),
            "{SLASH_DOC} is missing registry count {marker:?}"
        );
    }
    for marker in [
        format!("主コマンドが {primary_count} 件"),
        format!("受け付ける名前は合計 {accepted_count} 件"),
    ] {
        assert!(
            japanese.contains(&marker),
            "{JA_SLASH_DOC} is missing registry count {marker:?}"
        );
    }
    for marker in [
        format!("all {accepted_count} accepted command names"),
        format!("受け付ける全 {accepted_count} コマンド名"),
    ] {
        assert!(
            guide_index.contains(&marker),
            "{GUIDE_INDEX} is missing accepted-name count {marker:?}"
        );
    }
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

fn is_table_row(line: &str) -> bool {
    let line = line.trim();
    line.starts_with('|') && line.ends_with('|') && line.matches('|').count() >= 2
}

fn is_table_separator(line: &str) -> bool {
    if !is_table_row(line) {
        return false;
    }
    line.trim()
        .trim_matches('|')
        .split('|')
        .map(str::trim)
        .all(|cell| {
            let dashes = cell.trim_matches(':');
            dashes.len() >= 3 && dashes.bytes().all(|byte| byte == b'-')
        })
}

fn table_row_counts(markdown: &str) -> Vec<usize> {
    let visible = markdown_without_fenced_code(markdown);
    let lines = visible.lines().collect::<Vec<_>>();
    let mut counts = Vec::new();
    let mut index = 1;
    while index < lines.len() {
        if is_table_row(lines[index - 1]) && is_table_separator(lines[index]) {
            let mut rows = 0;
            index += 1;
            while index < lines.len() && is_table_row(lines[index]) {
                rows += 1;
                index += 1;
            }
            counts.push(rows);
        } else {
            index += 1;
        }
    }
    counts
}

fn github_slug(heading: &str) -> String {
    heading
        .chars()
        .flat_map(char::to_lowercase)
        .filter_map(|character| {
            if character.is_alphanumeric() || matches!(character, '-' | '_') {
                Some(character)
            } else if character.is_whitespace() {
                Some('-')
            } else {
                None
            }
        })
        .collect()
}

fn markdown_without_fenced_code(markdown: &str) -> String {
    let mut in_fence = false;
    let mut visible = String::with_capacity(markdown.len());
    for line in markdown.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            visible.push('\n');
        } else if in_fence {
            visible.push('\n');
        } else {
            visible.push_str(line);
            visible.push('\n');
        }
    }
    visible
}

fn markdown_heading(line: &str) -> Option<&str> {
    let indentation = line.len() - line.trim_start_matches(' ').len();
    if indentation > 3 {
        return None;
    }
    let line = &line[indentation..];
    let hashes = line.bytes().take_while(|byte| *byte == b'#').count();
    if !(1..=6).contains(&hashes) || !line[hashes..].starts_with(char::is_whitespace) {
        return None;
    }
    Some(line[hashes..].trim().trim_end_matches('#').trim_end())
}

fn github_anchors(markdown: &str) -> BTreeSet<String> {
    let visible = markdown_without_fenced_code(markdown);
    let mut anchors = BTreeSet::new();
    for heading in visible.lines().filter_map(markdown_heading) {
        let base = github_slug(heading);
        let mut candidate = base.clone();
        let mut suffix = 0;
        while anchors.contains(&candidate) {
            suffix += 1;
            candidate = format!("{base}-{suffix}");
        }
        anchors.insert(candidate);
    }
    anchors
}

fn markdown_destinations(markdown: &str) -> Vec<(usize, String)> {
    let visible = markdown_without_fenced_code(markdown);
    let mut destinations = Vec::new();
    let mut offset = 0;
    while let Some(open) = visible[offset..].find("](") {
        let destination_start = offset + open + 2;
        let Some(close) = visible[destination_start..].find(')') else {
            break;
        };
        let raw = visible[destination_start..destination_start + close].trim();
        let destination = if let Some(angle) = raw.strip_prefix('<') {
            angle.split_once('>').map_or(angle, |(value, _)| value)
        } else {
            raw.split_ascii_whitespace().next().unwrap_or_default()
        };
        if !destination.is_empty() {
            let line = visible[..destination_start]
                .bytes()
                .filter(|byte| *byte == b'\n')
                .count()
                + 1;
            destinations.push((line, destination.to_string()));
        }
        offset = destination_start + close + 1;
    }
    destinations
}

fn is_external_destination(destination: &str) -> bool {
    ["http://", "https://", "mailto:", "data:"]
        .iter()
        .any(|prefix| destination.starts_with(prefix))
}

fn validate_local_markdown_target(source: &str, destination: &str) -> Result<(), String> {
    let (relative, fragment) = destination
        .split_once('#')
        .map_or((destination, None), |(path, anchor)| (path, Some(anchor)));
    let source_path = repo_path(source);
    let target_path = if relative.is_empty() {
        source_path
    } else {
        source_path
            .parent()
            .expect("Markdown source must have a parent")
            .join(relative)
    };
    if !target_path.exists() {
        return Err(format!(
            "{source} links to missing local target {destination}"
        ));
    }

    let Some(fragment) = fragment else {
        return Ok(());
    };
    if !target_path.is_file()
        || target_path.extension().and_then(|value| value.to_str()) != Some("md")
    {
        return Err(format!(
            "{source} links to fragment #{fragment} on non-Markdown target {}",
            target_path.display()
        ));
    }
    let target = fs::read_to_string(&target_path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", target_path.display()));
    let anchors = github_anchors(&target);
    if !anchors.contains(fragment) {
        return Err(format!(
            "{source} links to missing GitHub fragment #{fragment} in {}",
            target_path.display()
        ));
    }
    Ok(())
}

fn assert_local_markdown_links(source: &str) -> usize {
    let markdown = read_repo_file(source);
    let mut local_links = 0;
    for (_, destination) in markdown_destinations(&markdown) {
        if is_external_destination(&destination) {
            continue;
        }
        validate_local_markdown_target(source, &destination)
            .unwrap_or_else(|failure| panic!("{failure}"));
        local_links += 1;
    }
    local_links
}

fn collect_markdown_files(directory: &Path, files: &mut Vec<PathBuf>) {
    let mut entries = fs::read_dir(directory)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", directory.display()))
        .map(|entry| {
            entry.unwrap_or_else(|err| panic!("failed to read {}: {err}", directory.display()))
        })
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.path());
    for entry in entries {
        let path = entry.path();
        let file_type = entry
            .file_type()
            .unwrap_or_else(|err| panic!("failed to inspect {}: {err}", path.display()));
        if file_type.is_dir() {
            if path != repo_path("docs/migration") {
                collect_markdown_files(&path, files);
            }
        } else if file_type.is_file()
            && path.extension().and_then(|extension| extension.to_str()) == Some("md")
        {
            files.push(path);
        }
    }
}

fn maintained_markdown_files() -> Vec<PathBuf> {
    let mut files = ROOT_MARKDOWN_DOCS
        .iter()
        .map(|path| repo_path(path))
        .collect::<Vec<_>>();
    for directory in MARKDOWN_DOC_DIRECTORIES {
        collect_markdown_files(&repo_path(directory), &mut files);
    }
    files.sort();
    files
}

#[test]
fn maintained_markdown_links_and_github_anchors_are_valid() {
    let repository = repo_path("");
    let mut failures = Vec::new();
    let mut checked_links = 0;
    for source_path in maintained_markdown_files() {
        let source = source_path
            .strip_prefix(&repository)
            .expect("documentation path must be repository-relative")
            .to_string_lossy()
            .into_owned();
        for (line, destination) in markdown_destinations(&read_repo_file(&source)) {
            if is_external_destination(&destination) {
                continue;
            }
            checked_links += 1;
            if let Err(failure) = validate_local_markdown_target(&source, &destination) {
                failures.push(format!("{source}:{line}: {failure}"));
            }
        }
    }

    assert!(
        checked_links > 100,
        "documentation link scan was unexpectedly small"
    );
    assert!(
        failures.is_empty(),
        "maintained documentation link/anchor drift detected ({} errors):\n- {}",
        failures.len(),
        failures.join("\n- ")
    );
}

#[test]
fn github_anchor_slugging_handles_punctuation_unicode_and_duplicates() {
    let anchors =
        github_anchors("# A B\n## A B\n### A B-1\n## 2. full の意味（最重要・不変条件）\n");
    assert_eq!(
        anchors,
        string_set(&["2-full-の意味最重要不変条件", "a-b", "a-b-1", "a-b-1-1",])
    );
}

#[test]
fn reader_oriented_document_set_exists() {
    for path in READER_DOCS {
        assert!(
            repo_path(path).is_file(),
            "required reader document is missing: {path}"
        );
        assert_local_markdown_links(path);
    }
}

#[test]
fn legacy_gui_index_retains_anchors_and_points_to_live_sections() {
    let path = "docs/user/gui.md";
    let markdown = read_repo_file(path);
    for heading in [
        "## Prerequisites",
        "## はじめに",
        "## Guided setup and preflight",
        "## Serve at `/`",
        "## Serve below a reverse-proxy path",
        "## Extensions catalog",
        "### Extension supply API",
        "## Trial run: Gate 1 through Gate 3/4",
        "## Workspace lease inspection and recovery",
        "### Trial token lifetime and rotation when authentication is on",
        "## API",
        "### Error responses and recovery",
        "## Two-basePath browser smoke",
    ] {
        assert!(
            markdown.lines().any(|line| line == heading),
            "{path} no longer retains legacy anchor heading {heading:?}"
        );
    }

    let local_links = assert_local_markdown_links(path);
    assert!(
        local_links >= 14,
        "{path} should route every legacy section"
    );
}

#[test]
fn gui_help_map_copy_is_owned_once_and_checked_by_smoke() {
    let help_map = read_repo_file("docs/user/gui-help-map.md");
    let smoke = read_repo_file("gui/scripts/smoke.mjs");
    for (source, copy, owner) in [
        (
            "gui/app/page.tsx",
            "目標を、検証可能なコードに。",
            "getting-started-gui.md#overview-landing-page",
        ),
        (
            "gui/app/page.tsx",
            "失敗を成功に見せない",
            "getting-started-gui.md#safety-and-honest-results",
        ),
        (
            "gui/app/page.tsx",
            "Goal から検証済みの結果まで",
            "getting-started-gui.md#goal-to-verified-result",
        ),
        (
            "gui/app/page.tsx",
            "4 つのレイヤーで安全に拡張する",
            "gui-extensions.md#four-extension-layers",
        ),
        (
            "gui/app/page.tsx",
            "装飾ではなく、gui_server が返した実際の準備状態とセッションだけを表示します。",
            "getting-started-gui.md#live-readiness-and-session-state",
        ),
        (
            "gui/components/getting-started.tsx",
            "前提を確認し、サンプル目標から実行前確認、進行状況、履歴、結果へ順に進みます。",
            "getting-started-gui.md#はじめに",
        ),
        (
            "gui/components/getting-started.tsx",
            "FIRST USE / はじめに",
            "getting-started-gui.md#はじめに",
        ),
        (
            "gui/components/getting-started.tsx",
            "サンプル目標をトライアルに入力",
            "getting-started-gui.md#first-trial-walkthrough",
        ),
        (
            "gui/components/getting-started.tsx",
            "CLI を動かす前に、目標・変更範囲・検証条件を確認する段階です。",
            "getting-started-gui.md#terms-shown-in-the-app",
        ),
        (
            "gui/components/getting-started.tsx",
            "トライアルがファイルを変更できる、専用の作業ディレクトリです。",
            "getting-started-gui.md#terms-shown-in-the-app",
        ),
        (
            "gui/components/getting-started.tsx",
            "目標に追加する検証知識。選択した版とハッシュが確認内容に固定されます。",
            "gui-trial.md#pack-selection-and-frozen-identity",
        ),
        (
            "gui/components/trial-compose.tsx",
            "Gate 1 は CLI 実行前の確認です",
            "gui-trial.md#gate-1-confirm-before-execution",
        ),
        (
            "gui/app/assets/page.tsx",
            "固定済みパックが見つかりません。",
            "gui-extensions.md#extensions-catalog",
        ),
        (
            "gui/app/assets/page.tsx",
            "トライアルで使う",
            "gui-extensions.md#extensions-catalog",
        ),
        (
            "gui/components/pack-wizard.tsx",
            "パック作成ウィザードを開く",
            "gui-extensions.md#pack-creation-wizard",
        ),
        (
            "gui/components/trial-session-index.tsx",
            "確認済みのトライアルセッションはありません。",
            "gui-history.md#session-rows-and-refresh",
        ),
    ] {
        assert!(
            read_repo_file(source).contains(copy),
            "{source} is missing {copy:?}"
        );
        assert_eq!(
            help_map.matches(copy).count(),
            1,
            "GUI help copy must have one document-map owner: {copy:?}"
        );
        assert!(
            smoke.contains(copy) && smoke.contains(owner),
            "GUI smoke does not bind {copy:?} to {owner}"
        );
    }
    for marker in ["helpMapChecks", "map_count", "helpMapOk"] {
        assert!(
            smoke.contains(marker),
            "GUI smoke is missing help-map check {marker}"
        );
    }
}

#[test]
fn gui_help_map_freezes_foundation_terminology_and_status_labels() {
    let help_map = read_repo_file("docs/user/gui-help-map.md");
    for marker in [
        "| delegated GUI run | トライアル |",
        "| configured Trial workspace | 実行ルート |",
        "| Trial gate | `gate_2` | Gate 2（実行） |",
        "| Trial status | `completed` | 完了 |",
        "| phase stage | missing / future value | 段階不明 |",
    ] {
        assert!(
            help_map.contains(marker),
            "GUI help map is missing shared foundation contract {marker:?}"
        );
    }

    let format = read_repo_file("gui/lib/format.ts");
    for marker in [
        "gate_2: \"Gate 2（実行）\"",
        "completed: \"完了\"",
        "ultra_phase_context_attached: \"実行条件を準備中\"",
        "return TRIAL_GATE_LABELS[normalizedEnumValue(value)] ?? \"Gate 不明\"",
        "return enumLabel(value, PHASE_STAGE_LABELS, \"段階不明\")",
    ] {
        assert!(
            format.contains(marker),
            "shared GUI formatter is missing contract {marker:?}"
        );
    }
}

#[test]
fn bilingual_readme_quickstarts_reach_cli_gui_and_extensions() {
    for path in ["README.md", "README.ja.md"] {
        let markdown = read_repo_file(path);
        let quickstart = markdown_section(&markdown, "## Quickstart", path);
        for target in [
            "docs/user/getting-started-cli.md",
            "docs/user/getting-started-gui.md",
            "docs/user/gui-extensions.md",
        ] {
            assert!(
                quickstart.contains(target),
                "{path} Quickstart does not link to required layer {target}"
            );
        }
    }
}

#[test]
fn bilingual_readme_quickstarts_show_the_complete_repl_gate_one_flow() {
    for path in ["README.md", "README.ja.md"] {
        let markdown = read_repo_file(path);
        let quickstart = markdown_section(&markdown, "## Quickstart", path);
        let repl = quickstart
            .find("commandagent --provider ollama --model \"<your-model>\"")
            .unwrap_or_else(|| panic!("{path} Quickstart does not start the REPL"));
        let request = quickstart
            .find("commandagent> ")
            .unwrap_or_else(|| panic!("{path} Quickstart does not show a plain-text request"));
        let confirm = quickstart
            .find("commandagent> /confirm sha256:<card-hash>")
            .unwrap_or_else(|| panic!("{path} Quickstart does not show Gate 1 confirmation"));

        assert!(
            repl < request && request < confirm,
            "{path} Quickstart must order REPL start, request, and /confirm"
        );
        assert!(
            !quickstart.contains("--prompt"),
            "{path} Quickstart bypasses the REPL with --prompt"
        );
    }
}

#[test]
fn bilingual_learning_path_is_ordered_and_within_three_clicks() {
    let getting_started = read_repo_file("docs/user/getting-started-cli.md");
    for (readme, tutorial, reference) in [
        (
            "README.md",
            "docs/guide/en/tutorial.md",
            "docs/guide/en/cli-reference.md",
        ),
        (
            "README.ja.md",
            "docs/guide/ja/tutorial.md",
            "docs/guide/ja/cli-reference.md",
        ),
    ] {
        let readme_markdown = read_repo_file(readme);
        let quickstart = markdown_section(&readme_markdown, "## Quickstart", readme);
        let entry = quickstart
            .find("docs/user/getting-started-cli.md")
            .unwrap_or_else(|| panic!("{readme} does not link the entry layer"));
        let detail = quickstart
            .find(tutorial)
            .unwrap_or_else(|| panic!("{readme} does not link the detail layer"));
        let reference_position = quickstart
            .find(reference)
            .unwrap_or_else(|| panic!("{readme} does not link the reference layer"));
        assert!(
            entry < detail && detail < reference_position,
            "{readme} must order getting started, detailed tutorial, and reference"
        );

        let tutorial_from_entry = tutorial.strip_prefix("docs/").unwrap();
        let reference_from_entry = reference.strip_prefix("docs/").unwrap();
        assert!(
            getting_started.contains(tutorial_from_entry)
                && getting_started.contains(reference_from_entry),
            "CLI entry page must link the next two layers for {readme}"
        );
        let tutorial_markdown = read_repo_file(tutorial);
        assert!(
            tutorial_markdown.contains("cli-reference.md"),
            "{tutorial} must link its language-matched reference"
        );
    }
}

#[test]
fn introductory_surfaces_keep_cli_and_gui_sample_goals_explicit() {
    for path in [
        "README.md",
        "README.ja.md",
        "docs/user/getting-started-cli.md",
        "docs/user/first-loop.md",
        "docs/guide/en/tutorial.md",
        "docs/guide/ja/tutorial.md",
    ] {
        assert!(
            read_repo_file(path).contains(CANONICAL_SAMPLE_GOAL),
            "{path} does not use the canonical CLI sample goal"
        );
    }
    assert!(
        read_repo_file("gui/hooks/use-trial-compose.ts").contains(GUI_SAMPLE_GOAL),
        "GUI Trial does not use the Japanese sample goal"
    );
}

#[test]
fn english_and_japanese_guides_have_matching_files_headings_and_tables() {
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
        let en_tables = table_row_counts(&read_repo_file(&en_path));
        let ja_tables = table_row_counts(&read_repo_file(&ja_path));
        if en_tables != ja_tables {
            failures.push(format!(
                "{en_path} has table row counts {en_tables:?}; {ja_path} has {ja_tables:?}"
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "EN/JA guide structure drift detected:\n- {}\nFix the named translation pair(s) under {en_directory} and {ja_directory}.",
        failures.join("\n- ")
    );
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

#[test]
fn demo_docs_distinguish_scripted_assets_from_provider_backed_recording() {
    let english = read_repo_file("README.md");
    let japanese = read_repo_file("README.ja.md");
    let notes = read_repo_file("docs/assets/ux-demo.md");
    let recording = repo_path("docs/assets/repl-ultra-plan-run.rec");

    for (path, readme, offline_marker, real_marker) in [
        ("README.md", english, "offline", "real screens"),
        ("README.ja.md", japanese, "オフライン", "実際の画面"),
    ] {
        assert!(
            readme.contains("docs/assets/demo/cli-demo.gif")
                && readme.contains("docs/assets/demo/gui-demo.gif")
                && readme.contains(real_marker)
                && readme.contains("docs/assets/ux-demo.md"),
            "{path} must embed the real CLI and GUI recordings and link the recording notes"
        );
        assert!(
            readme.contains("--ux-demo")
                && readme.contains(offline_marker)
                && readme.contains("provider-backed"),
            "{path} must still identify --ux-demo as offline and scripted rather than provider-backed"
        );
        assert!(
            !readme.contains("ux-demo.svg"),
            "{path} must not embed the hand-authored SVG excerpt as if it were a recording"
        );
    }
    for asset in [
        "docs/assets/demo/cli-demo.gif",
        "docs/assets/demo/gui-demo.gif",
    ] {
        let size = fs::metadata(repo_path(asset))
            .unwrap_or_else(|err| panic!("missing demo asset {asset}: {err}"))
            .len();
        assert!(
            size > 16 * 1024,
            "{asset} should be a real multi-frame recording"
        );
    }
    assert!(
        notes.contains("record_cli_demo.py")
            && notes.contains("render_cli_demo.py")
            && notes.contains("record_gui_demo.mjs")
            && notes.contains("cli-demo.cast.json"),
        "recording notes must explain how the committed GIFs are captured and regenerated"
    );
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
fn pack_supply_contract_v0_1_is_fixed_across_institution_docs() {
    let contract_path = "docs/pack-institution-contract.md";
    let contract = read_repo_file(contract_path);
    for marker in [
        "Status: fixed v0.1 (2026-08-19)",
        "materials/<name>.md",
        "bytewise_sort(direct material paths)",
        "commandagent-pack-v0\\0",
        "65,536 bytes",
        "262,144 bytes",
        "pack_material_document",
        "default `16384`",
        "type name is exactly `PackSource`",
        "Admitted",
        "Repository",
        "Local",
        "`admitted`",
        "`repository`",
        "`local`",
        "`JournalEntry`",
        "stage|verify|pin|retire",
        "gui|cli",
        "ok|error",
        "Signed or remote supply",
    ] {
        assert!(
            contract.contains(marker),
            "{contract_path} is missing fixed v0.1 marker {marker:?}"
        );
    }

    for display in [
        "承認済み",
        "リポジトリ（未承認）",
        "ローカル（未承認・帯域未計測）",
    ] {
        assert!(
            contract.contains(display),
            "{contract_path} is missing supply display {display:?}"
        );
    }

    let shell_path = "docs/d3c-shell-design.md";
    let shell = read_repo_file(shell_path);
    let selection = markdown_section(&shell, "## 4. Pack-selection surface", shell_path);
    for marker in [
        "`PackSource`",
        "`admitted | repository | local`",
        "exact-byte pinned and not retired",
        "merely present\nYAML file is never a Gate 1 candidate",
        "ローカル（未承認・帯域未計測）",
        "signed/remote supply",
    ] {
        assert!(
            selection.contains(marker),
            "{shell_path} section 4 is missing fixed supply marker {marker:?}"
        );
    }

    let integration = read_repo_file("docs/dev/integration-notes.md");
    assert!(
        integration.contains("PACK SUPPLY CONTRACT v0.1 FIXED; IMPLEMENTATION QUEUED")
            && integration
                .contains("Signed/remote supply, publisher trust, and revocation remain Phase G"),
        "Phase E exit index must carry the v0.1/Phase G disposition"
    );

    let ledger = read_repo_file("docs/dev/mechanism-ledger.md");
    for marker in [
        "E-01 — 未署名ローカル pack 供給契約 v0.1",
        "`PackSource`",
        "`pack_material_document`",
        "`JournalEntry`",
        "Phase Gへ残すのは署名",
    ] {
        assert!(
            ledger.contains(marker),
            "mechanism ledger is missing Issue 104 decision marker {marker:?}"
        );
    }
}

#[test]
fn nextjs_convention_pack_vocabulary_stays_bound_to_the_profile_contract() {
    let institution = read_repo_file("docs/pack-institution-contract.md");
    let profile = read_repo_file("docs/nextjs-profile-contract.md");
    let readme = read_repo_file("packs/README.md");
    for marker in [
        "pack_material_document",
        "path_layout_conforms",
        "design_tokens_only",
        "lint_config_present",
        "pack_check_result",
        "nextjs-acme@1.0.0",
    ] {
        assert!(
            institution.contains(marker),
            "institution contract missing {marker}"
        );
        assert!(
            profile.contains(marker),
            "Next.js contract missing {marker}"
        );
    }
    for marker in [
        "packs/nextjs-acme/1.0.0",
        "unadmitted repository conformance fixture",
        "path_layout_conforms",
        "design_tokens_only",
        "lint_config_present",
    ] {
        assert!(readme.contains(marker), "pack README missing {marker}");
    }
}

#[test]
fn additive_profile_overlay_decision_stays_bound_to_the_ledger() {
    let contract = read_repo_file("docs/dev/profile-manifest.md");
    for marker in [
        "## Additive overlay contract",
        "[overlay]",
        "base_profile = \"nextjs\"",
        "mode = \"additive\"",
        "status = \"draft\"",
        "`artifacts`, `guidance`, `checks`, and\n`evidence_targets`",
        "`plan`, `step_templates`, and `vocabulary` are forbidden",
        "`ManifestSource` values for overlays are `repository` and `local`",
        "`base -> overlay -> pack`",
        "reason `profile_not_admitted`",
        "<display_name>（下書き上乗せ）",
    ] {
        assert!(
            contract.contains(marker),
            "profile overlay contract is missing fixed marker {marker:?}"
        );
    }

    let ledger = read_repo_file("docs/dev/mechanism-ledger.md");
    for marker in [
        "E-02 — 承認 profile への追加専用 overlay",
        "Issue #105",
        "E-17（Issue #116、`ef0703f6`）",
        "ManifestSource",
        "E-18（Issue #117）",
    ] {
        assert!(
            ledger.contains(marker),
            "mechanism ledger is missing overlay decision marker {marker:?}"
        );
    }
}
