//! Issue #519 — align slash-command, provider, exclusivity, and environment
//! documentation with the shipped CLI, configuration, and boundary shell.
//!
//! The test pins only the four documented drifts (FIN-032..FIN-035). It does
//! not call a real provider: the CLI is parsed in-process, the binary cases use
//! an empty inherited environment and a temporary workspace, and the boundary
//! shell uses a temporary state directory.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use clap::Parser;
use clap::error::ErrorKind;

use commandagent::cli::Cli;
use commandagent::planner::adjudication::contract::IntentId;
use commandagent::planner::profile::ProfileId;
use commandagent::tui::boundary_shell::ambiguity::{
    ClassifierProvenance, ProposalStatus, RouteProposal,
};
use commandagent::tui::boundary_shell::confirmation::{ExecutionPins, PackSelection};
use commandagent::tui::boundary_shell::family_catalog::TaskFamilyId;
use commandagent::tui::boundary_shell::route::{RouteBasis, RouteCandidate};
use commandagent::tui::boundary_shell::{BoundaryShell, BoundaryState};
use commandagent::tui::slash::{SLASH_COMMANDS, render_help};

const EN_CLI: &str = "docs/guide/en/cli-reference.md";
const JA_CLI: &str = "docs/guide/ja/cli-reference.md";
const EN_SLASH: &str = "docs/guide/en/slash-commands.md";
const JA_SLASH: &str = "docs/guide/ja/slash-commands.md";
const EN_PROVIDERS: &str = "docs/guide/en/providers.md";
const JA_PROVIDERS: &str = "docs/guide/ja/providers.md";
const EN_CONFIG: &str = "docs/guide/en/configuration.md";
const JA_CONFIG: &str = "docs/guide/ja/configuration.md";
const EN_TROUBLE: &str = "docs/guide/en/troubleshooting.md";
const JA_TROUBLE: &str = "docs/guide/ja/troubleshooting.md";
const GUIDE_INDEX: &str = "docs/guide/README.md";

const PROVIDER_CANDIDATES: [&str; 5] = [
    "ollama",
    "lm-studio",
    "openai",
    "gemini",
    "openai-compatible",
];

const ENV_VARIABLES: [&str; 6] = [
    "COMMANDAGENT_STEP_WALL_CLOCK_CAP_MS",
    "COMMANDAGENT_PACK_DIRECTORY",
    "COMMANDAGENT_PACK_ID",
    "COMMANDAGENT_PACK_VERSION",
    "COMMANDAGENT_PACK_HASH",
    "COMMANDAGENT_UX_DEMO_FAST",
];

fn repo_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
}

fn read_repo_file(relative: &str) -> String {
    fs::read_to_string(repo_path(relative))
        .unwrap_or_else(|err| panic!("failed to read {relative}: {err}"))
}

fn markdown_section<'a>(markdown: &'a str, heading: &str, path: &str) -> &'a str {
    let marker = format!("{heading}\n");
    let (_, after_heading) = markdown
        .split_once(&marker)
        .unwrap_or_else(|| panic!("missing heading '{heading}' in {path}"));
    let end = after_heading.find("\n## ").unwrap_or(after_heading.len());
    &after_heading[..end]
}

/// Create a temporary directory under the Cargo target tree so tests never
/// write under the OS temp locations other workers share.
fn test_tempdir() -> tempfile::TempDir {
    tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).expect("temporary test directory")
}

/// Run the shipped binary with no inherited environment beyond `PATH`, a
/// temporary `HOME`/state directory, and a temporary workspace.
fn run_commandagent(arguments: &[&str], workspace: &Path) -> Output {
    let home = test_tempdir();
    Command::new(env!("CARGO_BIN_EXE_commandagent"))
        .args(arguments)
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("HOME", home.path())
        .env("XDG_STATE_HOME", home.path().join("state"))
        .current_dir(workspace)
        .output()
        .expect("run commandagent")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn action_selector_flags() -> BTreeSet<String> {
    commandagent::provider_cli::command()
        .get_arguments()
        .filter(|argument| !argument.is_hide_set())
        .filter(|argument| {
            argument
                .get_help_heading()
                .map(|heading| heading.to_string())
                .as_deref()
                == Some("Actions (use one)")
        })
        .filter_map(|argument| argument.get_long().map(|long| format!("--{long}")))
        .collect()
}

/// Collect the pure `--flag` names that appear inside Markdown code spans.
fn documented_flags(section: &str) -> BTreeSet<String> {
    section
        .split('`')
        .skip(1)
        .step_by(2)
        .filter_map(|token| {
            let token = token.trim();
            (token.starts_with("--")
                && token
                    .chars()
                    .all(|c| c == '-' || c.is_ascii_lowercase() || c.is_ascii_digit()))
            .then(|| token.to_string())
        })
        .collect()
}

#[test]
fn invalid_provider_value_lists_all_five_candidates_and_exits_two() {
    for flag in ["--provider", "--planner-provider"] {
        let error =
            commandagent::provider_cli::parse_from(["commandagent", flag, "not-a-provider"])
                .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::InvalidValue, "{flag}");
        assert_eq!(error.exit_code(), 2, "{flag}");
        let rendered = error.to_string();
        assert!(
            rendered.contains("invalid value 'not-a-provider'"),
            "{flag}: {rendered}"
        );
        for candidate in PROVIDER_CANDIDATES {
            assert!(
                rendered.contains(candidate),
                "{flag} candidate list is missing {candidate}: {rendered}"
            );
        }
    }
}

#[test]
fn all_five_provider_values_are_accepted_and_generic_extra_info_is_kept() {
    for value in PROVIDER_CANDIDATES {
        let parsed = commandagent::provider_cli::parse_from([
            "commandagent",
            "--provider",
            value,
            "--planner-provider",
            value,
        ])
        .unwrap_or_else(|err| panic!("{value} must be accepted: {err}"));
        assert!(parsed.cli.provider.is_some(), "{value}");
        assert!(parsed.cli.planner_provider.is_some(), "{value}");
        let generic = value == "openai-compatible";
        assert_eq!(
            parsed.provider_options.executor_openai_compatible, generic,
            "{value}"
        );
        assert_eq!(
            parsed.provider_options.planner_openai_compatible, generic,
            "{value}"
        );
    }
}

#[test]
fn action_selector_flags_match_the_bilingual_invocation_lists() {
    let actions = action_selector_flags();
    assert!(
        actions.contains("--workflow"),
        "the Clap `Actions (use one)` group must include --workflow"
    );
    for (path, heading) in [(EN_CLI, "## Invocation"), (JA_CLI, "## 呼び出し方")] {
        let markdown = read_repo_file(path);
        let section = markdown_section(&markdown, heading, path);
        let mut documented = documented_flags(section);
        documented.remove("--help");
        documented.remove("--version");
        assert_eq!(
            documented, actions,
            "{path} {heading} must list exactly the `Actions (use one)` flags"
        );
    }
}

#[test]
fn clap_and_configuration_rejections_keep_their_routes_and_exit_codes() {
    let workspace = test_tempdir();

    for arguments in [
        ["--doctor", "--runs"].as_slice(),
        ["--workflow", "workflow.yaml", "--intent", "create"].as_slice(),
        ["--origin", "origin-run"].as_slice(),
        ["--json"].as_slice(),
    ] {
        let output = run_commandagent(arguments, workspace.path());
        assert_eq!(
            output.status.code(),
            Some(2),
            "{arguments:?} must be a Clap rejection: {}",
            stderr(&output)
        );
        assert!(
            stdout(&output).is_empty(),
            "{arguments:?} must not emit a run summary"
        );
    }

    // A different planner provider without a planner model is a post-parse
    // configuration rejection, not a Clap conflict.
    let output = run_commandagent(
        &["--planner-provider", "openai", "--prompt", "x"],
        workspace.path(),
    );
    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
    assert!(stdout(&output).is_empty());

    let output = run_commandagent(&["--prompt", "x", "--ux-demo"], workspace.path());
    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
    assert!(
        stderr(&output).contains("only one action selector"),
        "{}",
        stderr(&output)
    );
    assert!(stdout(&output).is_empty());

    let output = run_commandagent(&["--workflow", "workflow.yaml"], workspace.path());
    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
    assert!(
        stderr(&output).contains("--workflow requires --origin"),
        "{}",
        stderr(&output)
    );
    assert!(stdout(&output).is_empty());
}

#[test]
fn doctor_configuration_failure_exits_one_with_a_json_report() {
    let workspace = test_tempdir();
    for arguments in [
        ["--doctor", "--json", "--planner-provider", "openai"].as_slice(),
        [
            "--ux-demo",
            "--doctor",
            "--json",
            "--planner-provider",
            "openai",
        ]
        .as_slice(),
    ] {
        let output = run_commandagent(arguments, workspace.path());
        assert_eq!(
            output.status.code(),
            Some(1),
            "doctor must exit 1 on a Configuration failure: {arguments:?}: {}",
            stderr(&output)
        );
        let report = stdout(&output);
        assert!(
            report.trim_start().starts_with('{'),
            "{arguments:?} must print a JSON report: {report}"
        );
        assert!(
            report.contains("config.resolution"),
            "{arguments:?} JSON report must record the Configuration failure: {report}"
        );
    }
}

#[test]
fn json_targets_and_workflow_dependencies_are_fixed() {
    for arguments in [
        ["commandagent", "--doctor", "--json"].as_slice(),
        ["commandagent", "--extensions", "--json"].as_slice(),
        ["commandagent", "--runs", "--json"].as_slice(),
    ] {
        Cli::try_parse_from(arguments)
            .unwrap_or_else(|err| panic!("{arguments:?} must accept --json: {err}"));
    }

    let error = Cli::try_parse_from(["commandagent", "--json"]).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::MissingRequiredArgument, "{error}");

    let error = Cli::try_parse_from(["commandagent", "--origin", "run"]).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::MissingRequiredArgument, "{error}");

    let error = Cli::try_parse_from(["commandagent", "--workflow", "w.yaml", "--intent", "create"])
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::ArgumentConflict, "{error}");

    // `--workflow` alone parses at the Clap layer; its `--origin` requirement
    // is a resolved-configuration check.
    Cli::try_parse_from(["commandagent", "--workflow", "w.yaml"])
        .expect("--workflow must parse without --origin");
}

#[test]
fn generic_provider_requires_a_base_url_for_any_role() {
    let workspace = test_tempdir();

    // Executor uses the generic provider without a base URL.
    let output = run_commandagent(
        &[
            "--provider",
            "openai-compatible",
            "--model",
            "served-model",
            "--prompt",
            "x",
        ],
        workspace.path(),
    );
    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
    assert!(
        stderr(&output).contains("base_url") || stderr(&output).contains("--base-url"),
        "{}",
        stderr(&output)
    );

    // `--base-url`/`--api-key-env` without any generic role is rejected.
    let output = run_commandagent(
        &[
            "--base-url",
            "http://127.0.0.1:9/v1",
            "--api-key-env",
            "SOME_KEY",
            "--prompt",
            "x",
        ],
        workspace.path(),
    );
    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));

    // Only the classifier is generic; the base URL is still required.
    let config_dir = workspace.path().join(".commandagent");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(
        config_dir.join("config.toml"),
        concat!(
            "[preset.classifier_gateway]\n",
            "provider = \"ollama\"\n",
            "model = \"executor-model\"\n",
            "planner_provider = \"ollama\"\n",
            "planner_model = \"planner-model\"\n",
            "classifier_provider = \"openai-compatible\"\n",
            "classifier_model = \"classifier-model\"\n",
        ),
    )
    .unwrap();
    let cwd = workspace.path().to_string_lossy().to_string();
    let output = run_commandagent(
        &[
            "--cwd",
            &cwd,
            "--preset",
            "classifier_gateway",
            "--prompt",
            "x",
        ],
        workspace.path(),
    );
    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
    assert!(
        stderr(&output).contains("base_url") || stderr(&output).contains("--base-url"),
        "{}",
        stderr(&output)
    );
}

fn issue519_proposal() -> RouteProposal {
    RouteProposal {
        selected: Some(RouteCandidate {
            profile: ProfileId::Ingest,
            intent: IntentId::Create,
            family: TaskFamilyId::List,
            bases: vec![RouteBasis {
                rule: "issue519",
                observation: "list".to_string(),
            }],
            contract_ref: "docs/ingest-profile-contract.md",
        }),
        alternatives: Vec::new(),
        classifier: ClassifierProvenance {
            used: false,
            provider: "ollama".to_string(),
            model: "planner".to_string(),
            prompt_version: "issue519-test",
            candidate_keys: Vec::new(),
            raw_response_hash: None,
            parse_reason: "deterministic_unique".to_string(),
        },
        status: ProposalStatus::AwaitingConfirmation,
        confirmation_required: true,
    }
}

fn issue519_pins() -> ExecutionPins {
    ExecutionPins {
        planner_provider: "ollama".to_string(),
        planner_model: "planner".to_string(),
        executor_provider: "ollama".to_string(),
        executor_model: "executor".to_string(),
        preset: "profile".to_string(),
        think: None,
    }
}

#[test]
fn gate_only_directives_stay_outside_the_registry_and_refuse_before_confirmation() {
    let registry = SLASH_COMMANDS
        .iter()
        .flat_map(|spec| std::iter::once(spec.name).chain(spec.aliases.iter().copied()))
        .collect::<BTreeSet<_>>();
    assert_eq!(SLASH_COMMANDS.len(), 23, "primary registry size");
    assert_eq!(registry.len(), 24, "accepted registry names");
    let help = render_help();
    for gate_only in ["/directive", "/confirm-directive"] {
        assert!(
            !registry.contains(gate_only),
            "{gate_only} must stay outside the general registry"
        );
        assert!(
            !help.contains(gate_only),
            "/help must not advertise the Gate-only command {gate_only}"
        );
    }

    let dir = test_tempdir();
    let events = dir.path().join("events.jsonl");
    let mut shell = BoundaryShell::new(dir.path().join("confirmations"), Some(events));

    // With no confirmed terminal run, both directive commands are refused.
    let refused = shell
        .begin_directive("repair README", "run-001", 1)
        .unwrap_err();
    assert!(
        refused.to_string().contains("Gate 3 or Gate 4"),
        "{refused}"
    );
    assert!(shell.confirm_directive("sha256:abc").is_err());

    shell
        .begin_gate_one(
            issue519_proposal(),
            "request",
            dir.path(),
            issue519_pins(),
            PackSelection::None,
        )
        .unwrap();
    let card_hash = match shell.state() {
        BoundaryState::AwaitingConfirmation { card_hash, .. } => card_hash.clone(),
        state => panic!("unexpected state: {state:?}"),
    };
    shell.confirm(&card_hash).unwrap();
    shell.dispatch(|_| Ok("failed".to_string())).unwrap();
    shell
        .present_terminal(
            "# sheet\n\n## 5. Stop reason\nfailed".to_string(),
            false,
            Some("failed".to_string()),
        )
        .unwrap();

    let directive = shell
        .begin_directive("repair README", "run-001", 1)
        .unwrap()
        .clone();
    assert_eq!(directive.artifact().issued_gate, "gate_4");

    // Gate 1 prefix matching does not apply: only the exact hash is accepted.
    assert!(shell.confirm_directive("sha256:wrong").is_err());
    assert!(shell.confirm_directive(directive.hash()).is_ok());
}

fn preset_config(provider_key: &str, model_key: &str, model_value: &str) -> String {
    let mut lines = vec![
        "[preset.alias_role]".to_string(),
        "provider = \"ollama\"".to_string(),
        "model = \"executor-model\"".to_string(),
        "planner_provider = \"ollama\"".to_string(),
        "planner_model = \"planner-model\"".to_string(),
        "classifier_provider = \"ollama\"".to_string(),
        "classifier_model = \"classifier-model\"".to_string(),
    ];
    lines.retain(|line| {
        !line.starts_with(&format!("{provider_key} = "))
            && !line.starts_with(&format!("{model_key} = "))
    });
    lines.push(format!("{provider_key} = \"openai\""));
    lines.push(format!("{model_key} = \"{model_value}\""));
    lines.join("\n") + "\n"
}

#[test]
fn openai_alias_is_rejected_for_all_three_roles_and_exact_ids_are_accepted() {
    let workspace = test_tempdir();
    let config_dir = workspace.path().join(".commandagent");
    fs::create_dir_all(&config_dir).unwrap();
    let config_path = config_dir.join("config.toml");
    let cwd = workspace.path().to_string_lossy().to_string();

    for (role, provider_key, model_key) in [
        ("executor", "provider", "model"),
        ("planner", "planner_provider", "planner_model"),
        ("classifier", "classifier_provider", "classifier_model"),
    ] {
        for model in ["gpt-5.6", "gpt-5.6-terra"] {
            fs::write(&config_path, preset_config(provider_key, model_key, model)).unwrap();
            // `--runs` resolves the configuration without creating provider
            // clients, so a resolved configuration exits 0 and an alias
            // rejection still exits 2.
            let output = run_commandagent(
                &["--cwd", &cwd, "--preset", "alias_role", "--runs"],
                workspace.path(),
            );
            if model == "gpt-5.6" {
                assert_eq!(
                    output.status.code(),
                    Some(2),
                    "{role} alias must be rejected: {}",
                    stderr(&output)
                );
                assert!(
                    stderr(&output).contains("gpt-5.6"),
                    "{role}: {}",
                    stderr(&output)
                );
            } else {
                assert_eq!(
                    output.status.code(),
                    Some(0),
                    "{role} exact ID gpt-5.6-terra must resolve: {}",
                    stderr(&output)
                );
            }
        }
    }
}

#[test]
fn bilingual_docs_record_the_issue_519_contract() {
    let slash_en = read_repo_file(EN_SLASH);
    let slash_ja = read_repo_file(JA_SLASH);
    for marker in [
        "Gate 3/4 dedicated commands",
        "`/directive <instruction>`",
        "`/confirm-directive <hash>`",
        "switches only among the four built-in providers",
    ] {
        assert!(slash_en.contains(marker), "{EN_SLASH} missing {marker:?}");
    }
    for marker in [
        "Gate 3/4 専用コマンド",
        "`/directive <instruction>`",
        "`/confirm-directive <hash>`",
        "が切り替えられるのは",
    ] {
        assert!(slash_ja.contains(marker), "{JA_SLASH} missing {marker:?}");
    }

    let index = read_repo_file(GUIDE_INDEX);
    for marker in [
        "all 24 accepted command names",
        "plus the Gate 3/4 dedicated",
        "受け付ける全 24 コマンド名",
        "Gate 3/4 専用の",
    ] {
        assert!(index.contains(marker), "{GUIDE_INDEX} missing {marker:?}");
    }

    let cli_en = read_repo_file(EN_CLI);
    let cli_ja = read_repo_file(JA_CLI);
    for marker in [
        "resolved configuration accepts exactly one action selector",
        "`--doctor` is the exception",
        "`--json` applies to",
        "needs its base URL",
        "switches only among the four built-in providers",
    ] {
        assert!(cli_en.contains(marker), "{EN_CLI} missing {marker:?}");
    }
    for marker in [
        "解決済み設定はアクション選択フラグを 1 つだけ受け付け",
        "`--doctor` は例外",
        "`--json` は `--doctor`",
        "汎用プロバイダに解決される役割には base URL が必要",
        "4 名 `ollama`、`lm-studio`、`openai`、`gemini`",
    ] {
        assert!(cli_ja.contains(marker), "{JA_CLI} missing {marker:?}");
    }

    let providers_en = read_repo_file(EN_PROVIDERS);
    let providers_ja = read_repo_file(JA_PROVIDERS);
    assert!(providers_en.contains("executor, planner, and classifier"));
    assert!(providers_en.contains("not applied to the generic"));
    assert!(providers_ja.contains("executor、planner、classifier"));
    assert!(providers_ja.contains("transport には適用しません"));

    let trouble_en = read_repo_file(EN_TROUBLE);
    let trouble_ja = read_repo_file(JA_TROUBLE);
    assert!(trouble_en.contains("every OpenAI role"));
    assert!(trouble_ja.contains("すべての OpenAI 役割"));

    let config_en = read_repo_file(EN_CONFIG);
    let config_ja = read_repo_file(JA_CONFIG);
    for variable in ENV_VARIABLES {
        assert!(
            config_en.contains(variable),
            "{EN_CONFIG} missing {variable}"
        );
        assert!(
            config_ja.contains(variable),
            "{JA_CONFIG} missing {variable}"
        );
    }
    for marker in [
        "no `ANVIL_*` legacy name",
        "use `--pack` or a preset to select a pack",
    ] {
        assert!(config_en.contains(marker), "{EN_CONFIG} missing {marker:?}");
    }
    for marker in [
        "`ANVIL_*` の旧名を持ちません",
        "pack の選択には `--pack` または",
    ] {
        assert!(config_ja.contains(marker), "{JA_CONFIG} missing {marker:?}");
    }
}
