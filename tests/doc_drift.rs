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

fn markdown_anchor(heading: &str) -> String {
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

fn assert_local_markdown_target(source: &str, destination: &str) {
    let (relative, fragment) = destination
        .split_once('#')
        .map_or((destination, None), |(path, anchor)| (path, Some(anchor)));
    let source_path = repo_path(source);
    let target_path = source_path
        .parent()
        .expect("Markdown source must have a parent")
        .join(relative);
    assert!(
        target_path.is_file(),
        "{source} links to missing local file {destination}"
    );

    let Some(fragment) = fragment else {
        return;
    };
    let target = fs::read_to_string(&target_path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", target_path.display()));
    let anchors = target
        .lines()
        .filter_map(|line| {
            line.strip_prefix("## ")
                .or_else(|| line.strip_prefix("### "))
        })
        .map(markdown_anchor)
        .collect::<BTreeSet<_>>();
    assert!(
        anchors.contains(fragment),
        "{source} links to missing fragment #{fragment} in {}",
        target_path.display()
    );
}

fn assert_local_markdown_links(source: &str) -> usize {
    let markdown = read_repo_file(source);
    let mut remainder = markdown.as_str();
    let mut local_links = 0;
    while let Some((_, after_open)) = remainder.split_once("](") {
        let Some((destination, after_close)) = after_open.split_once(')') else {
            break;
        };
        remainder = after_close;
        if destination.starts_with("http://") || destination.starts_with("https://") {
            continue;
        }
        assert_local_markdown_target(source, destination);
        local_links += 1;
    }
    local_links
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
            "gui/components/getting-started.tsx",
            "前提を確認し、サンプル目標から Gate 1 の実行前確認を試せます。",
            "getting-started-gui.md#はじめに",
        ),
        (
            "gui/components/getting-started.tsx",
            "CLI を動かす前に、目標・変更範囲・検証条件を確認する段階です。",
            "getting-started-gui.md#terms-shown-in-the-app",
        ),
        (
            "gui/components/getting-started.tsx",
            "Trial がファイルを変更できる、専用の作業ディレクトリです。",
            "getting-started-gui.md#terms-shown-in-the-app",
        ),
        (
            "gui/components/getting-started.tsx",
            "目標に追加する検証知識。選択した版とハッシュが確認内容に固定されます。",
            "gui-trial.md#pack-selection-and-frozen-identity",
        ),
        (
            "gui/components/trial-run.tsx",
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
            "Trial で使う",
            "gui-extensions.md#extensions-catalog",
        ),
        (
            "gui/components/pack-wizard.tsx",
            "pack 作成ウィザードを開く",
            "gui-extensions.md#pack-creation-wizard",
        ),
        (
            "gui/components/trial-session-index.tsx",
            "確認済み GUI Trial セッションはありません。",
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
