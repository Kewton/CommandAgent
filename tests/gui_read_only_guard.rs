use std::path::{Path, PathBuf};

const DELEGATE_MODULE: &str = "src/bin/gui_server/sessions.rs";

#[test]
fn gui_server_can_execute_only_through_the_confirmed_cli_delegate() {
    let mut sources = vec![PathBuf::from("src/bin/gui_server.rs")];
    collect_rust_files(Path::new("src/bin/gui_server"), &mut sources);
    assert!(
        sources.len() >= 3,
        "GUI server modules are missing: {sources:?}"
    );

    let globally_forbidden = [
        "provider_call",
        "providers::",
        "planner::runner",
        "minimal_loop::loop_run",
        "run_resolved_config",
        "reqwest",
        "tokio::process",
        "fs::write",
        "fs::create_dir",
        "OpenOptions",
        "File::create",
        ".write_all(",
        ".write(",
        "routing::put",
        "routing::patch",
        "routing::delete",
        "X-Forwarded",
        "x-forwarded",
    ];
    for path in sources {
        let source = std::fs::read_to_string(&path).unwrap();
        for token in globally_forbidden {
            assert!(
                !source.contains(token),
                "{} contains forbidden GUI capability {token:?}",
                path.display()
            );
        }
        if path != Path::new(DELEGATE_MODULE) {
            for token in ["std::process", "Command::new", ".spawn("] {
                assert!(
                    !source.contains(token),
                    "{} can spawn outside the sole CLI delegate: {token:?}",
                    path.display()
                );
            }
        }
    }

    let delegate = std::fs::read_to_string(DELEGATE_MODULE).unwrap();
    for required in [
        "shell.confirm(confirmation_hash)",
        ".trial_workspace.acquire(&id)",
        ".trial_workspace.require_current()",
        "Gate 1 workspace changed before CLI delegation",
        ".dispatch(|confirmed|",
        "Command::new(&state.commandagent_bin)",
        ".arg(\"--ultra-plan-run\")",
        ".restore_directive_proposal(&hash)",
        "shell.confirm_directive(&hash)",
        ".prepare_confirmed_continuation(",
        "shell.dispatch_directive(&continuation, ||",
        ".arg(\"--run-ultra-plan\")",
        "COMMANDAGENT_EVAL_EVENTS",
    ] {
        assert!(
            delegate.contains(required),
            "CLI delegation guard is missing {required:?}"
        );
    }
    for shell_bypass in ["sh\")", "bash\")", ".arg(\"-c\")", ".output("] {
        assert!(
            !delegate.contains(shell_bypass),
            "delegate contains an unbounded process surface {shell_bypass:?}"
        );
    }
}

#[test]
fn delegation_guard_negative_examples_are_rejected() {
    let bad_examples = [
        (
            "src/bin/gui_server/api.rs",
            "Command::new(\"commandagent\").spawn()",
        ),
        (
            DELEGATE_MODULE,
            "commandagent::providers::client_from_config(&config, false)",
        ),
        (
            DELEGATE_MODULE,
            "commandagent::planner::runner::run_plan_file()",
        ),
        (DELEGATE_MODULE, "Command::new(\"sh\").arg(\"-c\")"),
        (DELEGATE_MODULE, "Command::new(\"node\").spawn()"),
    ];
    for (path, source) in bad_examples {
        assert!(
            violates_delegation_guard(Path::new(path), source),
            "negative fixture unexpectedly passed: {path}: {source}"
        );
    }
}

#[test]
fn gui_dependencies_stay_out_of_the_default_rust_build() {
    let manifest = std::fs::read_to_string("Cargo.toml").unwrap();
    for dependency in ["axum", "tokio"] {
        let declaration = manifest
            .lines()
            .find(|line| line.starts_with(&format!("{dependency} =")))
            .unwrap_or_else(|| panic!("missing {dependency} declaration"));
        assert!(
            declaration.contains("optional = true"),
            "{dependency} must remain optional: {declaration}"
        );
    }
    assert!(manifest.contains("required-features = [\"gui\"]"));
    assert!(manifest.contains("default = []"));
}

#[test]
fn next_export_and_base_path_audit_are_pinned() {
    let config = std::fs::read_to_string("gui/next.config.ts").unwrap();
    assert!(config.contains("output: \"export\""));
    assert!(config.contains("process.env.GUI_BASE_PATH"));
    assert!(config.contains("NEXT_PUBLIC_GUI_BASE_PATH: basePath"));

    let package = std::fs::read_to_string("gui/package.json").unwrap();
    assert!(package.contains("scripts/lint-internal-paths.mjs"));
    assert!(Path::new("gui/package-lock.json").is_file());
}

#[test]
fn trial_ui_keeps_gate_one_confirmation_and_has_no_intervention_surface() {
    let source = std::fs::read_to_string("gui/app/try/page.tsx").unwrap();
    for required in [
        "if (!confirmed || proposal === null)",
        "\"x-commandagent-trial-authorization\": `Bearer ${token.trim()}`",
        "confirmation_hash: proposal.card_hash",
        "data-testid=\"trial-workspace\"",
        "proposal.identity.workspace",
        "このディレクトリ内の内容を作成・変更・削除できます",
        "apiPath(\"trial-options\")",
        "trialOptions.profiles.map",
        "trialOptions.providers.map",
        "data-testid=\"trial-profile-description\"",
        "data-testid=\"trial-provider-model-hint\"",
        "プロバイダーを変更しても実行モデルは自動更新されません。",
        "契約を確認する前に、目標を入力してください。",
        "契約を確認する前に、実行モデルの正確な ID を入力してください。",
        "契約を確認する前に、計画モデルの正確な ID を入力してください。",
        "window.matchMedia(\"(max-width: 720px)\")",
        "target.scrollIntoView({ behavior: \"smooth\", block: \"start\" })",
        "契約を確認する前に、実行時の Trial アクセストークンを入力してください。",
        "stage === \"gate_2\" || stage === \"terminal\" || stage === \"closed\"",
        "disabled={busy || launchIdentityLocked}",
        "disabled={!confirmed || busy || stage === \"gate_2\"}",
        "確認して CLI に委譲",
        "D-3d 継続を確認",
        "追加実行せず終了",
        "data-testid=\"start-new-run\"",
        "新しい実行を開始",
    ] {
        assert!(
            source.contains(required),
            "trial UI is missing {required:?}"
        );
    }
    for empty_default in ["goal: \"\",", "model: \"\",", "planner_model: \"\","] {
        assert!(
            source.contains(empty_default),
            "trial UI retains a demo default for {empty_default:?}"
        );
    }
    assert!(
        !source.contains("qwen3:8b")
            && !source.contains("<option value=\"python-cli\">")
            && !source.contains("<option value=\"lm-studio\">")
            && !source.contains("<option value=\"openai\">")
            && !source.contains("<option value=\"gemini\">"),
        "Trial defaults and options must not be copied into the client"
    );
    assert!(
        source.find("契約を確認する前に、目標を入力してください。")
            < source
                .find("契約を確認する前に、実行時の Trial アクセストークンを入力してください。"),
        "empty Goal guidance must run before token validation"
    );
    assert!(
        !source.contains("disabled={trialToken === \"\""),
        "Trial token guidance must not be hidden behind a disabled button"
    );
    assert_eq!(
        source.matches("disabled={launchIdentityLocked}").count(),
        4,
        "goal, token, and both model controls must share the run-stage lock"
    );
    assert_eq!(
        source
            .matches("disabled={launchIdentityLocked || trialOptions === null}")
            .count(),
        2,
        "profile and provider controls must combine option loading with the run-stage lock"
    );
    for reset in [
        "setProposal(null)",
        "setConfirmed(false)",
        "setCreated(null)",
        "setSession(null)",
        "setDirectiveText(\"\")",
        "setDirective(null)",
        "setError(null)",
        "setStage(\"compose\")",
    ] {
        assert!(
            source.contains(reset),
            "new-run transition is missing reset {reset:?}"
        );
    }
    for forbidden in [
        "Cancel session",
        "Interrupt session",
        "Stop session",
        "Override gate",
        "Skip gate",
    ] {
        assert!(
            !source.contains(forbidden),
            "trial UI exposes forbidden intervention control {forbidden:?}"
        );
    }

    let smoke = std::fs::read_to_string("gui/scripts/smoke.mjs").unwrap();
    for explicit_fill in [
        "[data-testid='trial-goal']\").fill(\"Create a CLI --pattern filter command\")",
        "[data-testid='trial-executor-model']\").fill(model)",
        "[data-testid='trial-planner-model']\").fill(model)",
    ] {
        assert!(
            smoke.matches(explicit_fill).count() >= 3,
            "GUI smoke must fill {explicit_fill:?} for initial, conflict, and lifecycle runs"
        );
    }
    for browser_check in [
        "initialTrialFieldsEmpty",
        "emptyGoalGuidance.includes(\"目標を入力してください\")",
        "selectOption(\"lm-studio\")",
        "providerModelGuidance.includes(\"実行モデルは自動更新されません\")",
    ] {
        assert!(
            smoke.contains(browser_check),
            "GUI smoke is missing browser acceptance check {browser_check:?}"
        );
    }
}

#[test]
fn trial_monitor_retries_and_reconnects_without_persisting_access() {
    let page = std::fs::read_to_string("gui/app/try/page.tsx").unwrap();
    for required in [
        "redirect: \"manual\"",
        "response.type === \"opaqueredirect\"",
        "data-testid=\"monitor-state\"",
        "最終更新成功:",
        "監視を再接続",
        "GET のみを使用し、別の CLI プロセスは起動しません。",
        "new URLSearchParams(window.location.search).get(\"session\")",
        "url.searchParams.set(\"session\", id)",
        "sessionIdFromConflict(detail)",
    ] {
        assert!(
            page.contains(required),
            "trial monitoring UI is missing {required:?}"
        );
    }
    for forbidden in ["localStorage", "sessionStorage", "trialToken=", "token="] {
        assert!(
            !page.contains(forbidden),
            "trial UI persists or exposes access through {forbidden:?}"
        );
    }

    let policy = std::fs::read_to_string("gui/lib/trial-monitor.ts").unwrap();
    for required in [
        "const MAX_BACKOFF_MS = 12_000",
        "export const TERMINAL_FAILURE_LIMIT = 4",
        "Math.min(POLL_INTERVAL_MS * 2 ** exponent, MAX_BACKOFF_MS)",
        "response.status === 401 || response.status === 403",
        "response.status === 413 || invalidJsonl",
        "プロキシまたはネットワーク接続",
    ] {
        assert!(
            policy.contains(required),
            "trial monitoring policy is missing {required:?}"
        );
    }

    let smoke = std::fs::read_to_string("gui/scripts/smoke.mjs").unwrap();
    for required in [
        "Synthetic browser fetch rejection",
        "return \"opaqueredirect\"",
        "reconnectMethods.every((method) => method === \"GET\")",
        "tokenStayedInMemory",
        "probeMobile(browser",
    ] {
        assert!(
            smoke.contains(required),
            "browser smoke is missing {required:?}"
        );
    }
}

#[test]
fn gui_style_and_run_ledger_accessibility_contracts_are_pinned() {
    let styles = std::fs::read_to_string("gui/app/globals.css").unwrap();
    assert_eq!(
        styles.matches(".trial-compose > input,").count(),
        2,
        "desktop and mobile Trial insets must include the token input"
    );
    assert!(styles.contains(
        ".trial-compose > textarea,\n.trial-compose > input {\n  width: calc(100% - 2.5rem);"
    ));
    assert!(styles.contains(
        ".trial-compose > textarea,\n  .trial-compose > input {\n    width: calc(100% - 2rem);"
    ));
    assert!(styles.contains(
        ".gate-one-grid,\n  .execution-panel,\n  .terminal-grid {\n    scroll-margin-top: 4.5rem;"
    ));

    let dashboard = std::fs::read_to_string("gui/app/page.tsx").unwrap();
    assert!(dashboard.contains("<div className=\"run-table\">"));
    assert!(dashboard.contains("<div className=\"run-table-head\" aria-hidden=\"true\">"));
    assert!(!dashboard.contains("role=\"table\""));
    assert!(!dashboard.contains("role=\"row\""));
}

#[test]
fn gui_language_navigation_titles_and_runtime_status_are_pinned() {
    let gui_files = [
        "gui/app/page.tsx",
        "gui/app/try/page.tsx",
        "gui/app/runs/page.tsx",
        "gui/app/assets/page.tsx",
        "gui/app/measurements/page.tsx",
        "gui/components/shell.tsx",
        "gui/scripts/smoke.mjs",
    ];
    let gui = gui_files
        .iter()
        .map(|path| std::fs::read_to_string(path).unwrap())
        .collect::<String>();
    let removed = [
        ["Launch once.", " Trust the gates."].concat(),
        ["Claims need", " coordinates."].concat(),
        ["Pinned means", " visible."].concat(),
        ["Evidence,", " at a glance."].concat(),
        ["One run.", " Every receipt."].concat(),
        ["CLI", " delegated"].concat(),
        ["Existing gates", " remain authoritative."].concat(),
        ["CommandAgent", " Observatory"].concat(),
    ];
    for removed in removed {
        assert!(
            !gui.contains(&removed),
            "obsolete GUI copy remains: {removed}"
        );
    }

    let shell = std::fs::read_to_string("gui/components/shell.tsx").unwrap();
    assert_eq!(
        shell
            .lines()
            .filter(|line| line.trim_start().starts_with("{ route: \""))
            .count(),
        4
    );
    assert!(!shell.contains("{ route: \"assets\""));
    for required in [
        "data-testid=\"runtime-status\"",
        "data-trial-available",
        "data-session-state",
        "Trial 利用可",
        "実行中なし",
        "要復旧",
    ] {
        assert!(
            shell.contains(required),
            "runtime shell is missing {required:?}"
        );
    }

    let dashboard = std::fs::read_to_string("gui/app/page.tsx").unwrap();
    assert!(dashboard.contains("data-testid=\"assets-link\""));
    let styles = std::fs::read_to_string("gui/app/globals.css").unwrap();
    assert!(styles.contains("grid-template-columns: repeat(4, minmax(0, 1fr));"));
    assert!(styles.contains(".page-intro > p {\n    display: none;"));

    let titles = [
        ("gui/app/layout.tsx", "default: \"概要 | CommandAgent\""),
        ("gui/app/try/layout.tsx", "title: \"トライアル\""),
        ("gui/app/runs/layout.tsx", "title: \"実行詳細\""),
        ("gui/app/assets/layout.tsx", "title: \"アセット\""),
        ("gui/app/measurements/layout.tsx", "title: \"計測\""),
    ];
    for (path, title) in titles {
        assert!(
            std::fs::read_to_string(path).unwrap().contains(title),
            "{path} is missing {title:?}"
        );
    }

    let server = std::fs::read_to_string("src/bin/gui_server.rs").unwrap();
    assert!(server.contains("/api/runtime-status"));
    let runtime = std::fs::read_to_string("src/bin/gui_server/runtime_status.rs").unwrap();
    assert!(runtime.contains("state.trial_workspace.runtime_status()"));
}

#[test]
fn trial_phase_badges_distinguish_pending_running_completed_failed_and_interrupted() {
    let css = std::fs::read_to_string("gui/app/globals.css").unwrap();
    for required in [
        ".phase-row em {\n  padding: 0.25rem 0.45rem;\n  border-radius: 999px;\n  background: var(--surface-soft);",
        ".phase-row.running em {\n  background: var(--accent-soft);\n  color: var(--accent-hover);",
        ".phase-row.completed em,\n.phase-row.passed em {\n  background: var(--success-soft);\n  color: var(--success);",
        ".phase-row.failed em {\n  background: var(--danger-soft);\n  color: var(--danger);",
        ".phase-row.interrupted em {\n  background: color-mix(in srgb, var(--warning) 12%, transparent);\n  color: var(--warning);",
    ] {
        assert!(
            css.contains(required),
            "phase badge CSS is missing distinct styling {required:?}"
        );
    }
}

#[test]
fn trial_workspace_and_authentication_guards_are_not_optional() {
    let entry = std::fs::read_to_string("src/bin/gui_server.rs").unwrap();
    assert!(!entry.contains("unwrap_or_else(|| arguments.repository_root.clone())"));
    assert!(entry.contains("TrialWorkspace::configure"));
    assert!(entry.contains("TrialAccess::from_environment"));

    let delegate = std::fs::read_to_string(DELEGATE_MODULE).unwrap();
    assert_eq!(
        delegate.matches("require_trial(&state, &headers").count(),
        6,
        "every Trial API handler must enforce workspace and access guards"
    );
    for required in [
        "StatusCode::SERVICE_UNAVAILABLE",
        "StatusCode::UNAUTHORIZED",
        "StatusCode::FORBIDDEN",
        "complete_from_events",
        "lease_snapshot",
        "rollback_unstarted",
        "failed to spawn delegated CLI binary",
    ] {
        assert!(
            delegate.contains(required),
            "missing Trial guard {required:?}"
        );
    }

    assert!(entry.contains(".route(\"/api/trial-workspace\", get(sessions::workspace_status))"));
}

#[test]
fn trial_workspace_recovery_is_visible_but_read_only() {
    let page = std::fs::read_to_string("gui/app/try/page.tsx").unwrap();
    for required in [
        "fetch(apiPath(\"trial-workspace\")",
        "data-testid=\"workspace-lease-status\"",
        "data-testid=\"workspace-lease-session\"",
        "復旧が必要",
        "読み取り専用の確認です。リースの解除や CLI プロセスの起動は行いません。",
        "ワークスペースのリースを確認",
    ] {
        assert!(
            page.contains(required),
            "Trial lease UI is missing {required:?}"
        );
    }
    for forbidden in [
        "Clear workspace lease",
        "Reset workspace lease",
        "Force idle",
        "Recover automatically",
        "ワークスペースのリースを解除",
        "ワークスペースのリースをリセット",
        "強制的に待機状態へ変更",
        "自動復旧",
    ] {
        assert!(
            !page.contains(forbidden),
            "Trial lease UI exposes a mutating recovery action {forbidden:?}"
        );
    }

    let guide = std::fs::read_to_string("docs/user/gui.md").unwrap();
    for required in [
        "## Workspace lease inspection and recovery",
        "GET api/trial-workspace",
        "`commandagent` remains for the execution root",
        "archive outside",
        "do not append a synthetic terminal event",
        "It must report `Idle`",
    ] {
        assert!(
            guide.contains(required),
            "GUI recovery guide is missing {required:?}"
        );
    }
}

fn collect_rust_files(root: &Path, output: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(root).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rust_files(&path, output);
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
            output.push(path);
        }
    }
}

fn violates_delegation_guard(path: &Path, source: &str) -> bool {
    [
        "provider_call",
        "providers::",
        "planner::runner",
        "minimal_loop::loop_run",
        "run_resolved_config",
        "tokio::process",
        "Command::new(\"sh\")",
        "Command::new(\"bash\")",
        ".arg(\"-c\")",
    ]
    .iter()
    .any(|token| source.contains(token))
        || source.contains("Command::new(\"")
        || (path != Path::new(DELEGATE_MODULE)
            && ["std::process", "Command::new", ".spawn("]
                .iter()
                .any(|token| source.contains(token)))
}
