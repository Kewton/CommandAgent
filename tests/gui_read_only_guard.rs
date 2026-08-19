use std::path::{Path, PathBuf};

const DELEGATE_MODULE: &str = "src/bin/gui_server/delegate.rs";
const DIRECTIVES_MODULE: &str = "src/bin/gui_server/directives.rs";
const SESSION_FILES_MODULE: &str = "src/bin/gui_server/session_files.rs";

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
        ".trial_workspace\n        .acquire(&id)\n        .map_err(workspace_conflict)",
        ".trial_workspace\n        .require_current()",
        "Gate 1 workspace changed before CLI delegation",
        ".dispatch(|confirmed|",
        "Command::new(&state.commandagent_bin)",
        "command.env_clear()",
        "DELEGATE_PARENT_ENV_ALLOWLIST",
        "fn check_binary(path: &Path)",
        ".arg(\"--version\")",
        ".output()",
        ".arg(\"--ultra-plan-run\")",
        ".arg(\"--run-ultra-plan\")",
        "COMMANDAGENT_EVAL_EVENTS",
        ".arg(\"--extension-root\")",
    ] {
        assert!(
            delegate.contains(required),
            "CLI delegation guard is missing {required:?}"
        );
    }
    for shell_bypass in ["sh\")", "bash\")", ".arg(\"-c\")"] {
        assert!(
            !delegate.contains(shell_bypass),
            "delegate contains an unbounded process surface {shell_bypass:?}"
        );
    }
    assert!(
        !delegate.contains("\"COMMANDAGENT_PACK_"),
        "delegate allowlist admits ambient pack selectors"
    );

    let directives = std::fs::read_to_string(DIRECTIVES_MODULE).unwrap();
    for required in [
        ".restore_directive_proposal(&hash)",
        "shell.confirm_directive(&hash)",
        ".prepare_confirmed_continuation(",
        "shell.dispatch_directive(&continuation, ||",
        "run_cli_continuation(&state, &paths, &identity, &continuation)",
    ] {
        assert!(
            directives.contains(required),
            "directive confirmation guard is missing {required:?}"
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
        (
            DELEGATE_MODULE,
            "Command::new(&state.commandagent_bin).spawn()",
        ),
        (
            DELEGATE_MODULE,
            "command.env(\"COMMANDAGENT_PACK_DIRECTORY\", value)",
        ),
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
fn gui_fetch_failures_use_one_actionable_error_descriptor() {
    let descriptor = std::fs::read_to_string("gui/lib/errors.ts").unwrap();
    for required in [
        "export function describeError",
        "export async function responseError",
        "trial_token_invalid",
        "GUI_TRIAL_ALLOWED_ORIGINS",
        "--commandagent-bin",
        "trial_workspace_running",
        "trial_workspace_recovery_required",
        "trial_request_invalid",
        "resource_too_large",
        "上流プロキシまたはアクセス認証",
        "isTrialTokenRejected",
        "reconnectSessionId",
    ] {
        assert!(
            descriptor.contains(required),
            "shared GUI error descriptor is missing {required:?}"
        );
    }

    for path in [
        "gui/lib/use-resource.ts",
        "gui/app/runs/page.tsx",
        "gui/app/measurements/page.tsx",
    ] {
        let source = std::fs::read_to_string(path).unwrap();
        for required in ["describeError", "responseError"] {
            assert!(
                source.contains(required),
                "{path} bypasses the common failure path: missing {required}"
            );
        }
    }
    for (path, required) in [
        ("gui/hooks/use-trial-run.ts", "describeError"),
        ("gui/components/trial-session-index.tsx", "describeError"),
        ("gui/lib/trial-api.ts", "responseError"),
    ] {
        assert!(
            std::fs::read_to_string(path).unwrap().contains(required),
            "{path} bypasses the common failure path: missing {required}"
        );
    }

    let raw_network_message = ["Failed", "to", "fetch"].join(" ");
    for path in gui_source_files(Path::new("gui")) {
        let source = std::fs::read_to_string(&path).unwrap();
        assert!(
            !source.contains(&raw_network_message),
            "{} exposes the browser's raw network message",
            path.display()
        );
    }
}

#[test]
fn trial_route_is_wiring_only_and_shared_helpers_have_single_owners() {
    let page = std::fs::read_to_string("gui/app/try/page.tsx").unwrap();
    assert!(
        page.lines().count() <= 20,
        "Trial route entrypoint grew beyond wiring"
    );
    for required in ["<Shell", "<TrialRun />"] {
        assert!(
            page.contains(required),
            "Trial route wiring is missing {required:?}"
        );
    }
    for forbidden in ["useState", "useEffect", "fetch(", "data-testid"] {
        assert!(
            !page.contains(forbidden),
            "Trial route entrypoint owns non-wiring behavior {forbidden:?}"
        );
    }

    let component = std::fs::read_to_string("gui/components/trial-run.tsx").unwrap();
    assert!(component.contains("useTrialRun(terminalHeading)"));
    assert!(component.contains("data-testid=\"trial-active-stage\""));

    let hook = std::fs::read_to_string("gui/hooks/use-trial-run.ts").unwrap();
    for required in [
        "export function useTrialRun",
        "useState",
        "useEffect",
        "isTrialTokenRejected(reason)",
    ] {
        assert!(
            hook.contains(required),
            "Trial workflow hook is missing {required:?}"
        );
    }

    let api = std::fs::read_to_string("gui/lib/trial-api.ts").unwrap();
    assert!(api.contains("export function trialAuthorizationHeaders"));
    assert_eq!(
        api.matches("x-commandagent-trial-authorization").count(),
        1,
        "Trial authorization header must have one owner"
    );

    let monitor = std::fs::read_to_string("gui/lib/trial-monitor.ts").unwrap();
    for required in [
        "status: number",
        "code: string | null",
        "status: response.status",
        "code: detail.code",
    ] {
        assert!(
            monitor.contains(required),
            "MonitorFailure is missing {required:?}"
        );
    }

    let gui = gui_source_files(Path::new("gui"))
        .iter()
        .map(|path| std::fs::read_to_string(path).unwrap())
        .collect::<String>();
    assert_eq!(gui.matches("function byteLabel(").count(), 1);
    assert_eq!(gui.matches("function dateLabel(").count(), 1);
}

#[test]
fn trial_ui_keeps_gate_one_confirmation_and_has_no_intervention_surface() {
    let source = trial_ui_sources();
    for required in [
        "if (!confirmed || proposal === null)",
        "\"x-commandagent-trial-authorization\": `Bearer ${token.trim()}`",
        "launchSession(trialToken, spec, proposal.card_hash)",
        "<GateCardMarkdown markdown={proposal.card_markdown} />",
        "data-testid=\"trial-workspace\"",
        "proposal.identity.workspace",
        "このディレクトリ内の内容だけを作成・変更・削除できます",
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
        "disabled={!confirmed || busy || launchBlockReason !== null}",
        "確認して CLI を実行",
        "確認して追加の依頼を実行",
        "data-testid=\"terminal-result-heading\"",
        "data-testid=\"terminal-result-summary\"",
        "data-testid=\"terminal-verdict-summary\"",
        "data-testid=\"terminal-assurance-summary\"",
        "data-testid=\"terminal-status-summary\"",
        "terminalHeading(session)",
        "verdictSummary(session)",
        "assuranceSummary(session.assurance)",
        "最終受け入れは記録されていません。",
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

    for internal_copy in [
        "Frozen launch identity",
        "MEASURED PRICE TAG",
        "NEXT ACTION / D-3d",
        "Boundary instruction",
        "Scrub and persist instruction",
        "session.verdict ?? session.status",
    ] {
        assert!(
            !source.contains(internal_copy),
            "trial UI still exposes internal copy {internal_copy:?}"
        );
    }

    let markdown = std::fs::read_to_string("gui/components/gate-card-markdown.tsx").unwrap();
    for required in [
        "data-testid=\"gate-one-card-markdown\"",
        "parseGateCard(markdown)",
        "<h2 key={index}>{block.text}</h2>",
        "<h3 key={index}>{block.text}</h3>",
    ] {
        assert!(
            markdown.contains(required),
            "Gate 1 markdown renderer is missing {required:?}"
        );
    }
    assert!(
        !markdown.contains("dangerouslySetInnerHTML"),
        "Gate 1 markdown must stay escaped by React"
    );
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
        "terminalTitle === \"✔ すべての必須チェックに合格しました — CommandAgent\"",
        "code: \"trial_workspace_running\"",
        "conflictGuidance.includes(`セッション ${sessionId} に再接続`)",
    ] {
        assert!(
            smoke.contains(browser_check),
            "GUI smoke is missing browser acceptance check {browser_check:?}"
        );
    }
}

#[test]
fn trial_monitor_retries_and_reconnects_with_tab_scoped_access() {
    let page = trial_ui_sources();
    for required in [
        "redirect: \"manual\"",
        "response.type === \"opaqueredirect\"",
        "data-testid=\"monitor-state\"",
        "最終更新成功:",
        "監視を再接続",
        "GET のみを使用し、別の CLI プロセスは起動しません。",
        "new URLSearchParams(window.location.search).get(\"session\")",
        "url.searchParams.set(\"session\", id)",
        "reconnectIdFromError(reason)",
        "data-testid=\"reconnect-session-link\"",
    ] {
        assert!(
            page.contains(required),
            "trial monitoring UI is missing {required:?}"
        );
    }
    for forbidden in ["localStorage", "trialToken=", "token="] {
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
        "code: typeof parsed.code === \"string\" ? parsed.code : null",
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
        "reloadRestoredToken",
        "rejectedTokenRemoved",
        "tokenStayedTabScoped",
        "probeMobile(browser",
    ] {
        assert!(
            smoke.contains(required),
            "browser smoke is missing {required:?}"
        );
    }
}

#[test]
fn trial_token_storage_is_base_path_scoped_and_non_durable() {
    let storage = std::fs::read_to_string("gui/lib/trial-token-storage.ts").unwrap();
    for required in [
        "commandagent.gui.trial-token",
        "guiBasePath() || \"/\"",
        "window.sessionStorage.getItem",
        "window.sessionStorage.setItem",
        "window.sessionStorage.removeItem",
        "storedValue?.trim() === rejectedValue.trim()",
    ] {
        assert!(
            storage.contains(required),
            "Trial token storage is missing {required:?}"
        );
    }
    for forbidden in [
        "localStorage",
        "BroadcastChannel",
        "addEventListener(\"storage\"",
        "window.open",
    ] {
        assert!(
            !storage.contains(forbidden),
            "Trial token storage adds forbidden persistence or synchronization {forbidden:?}"
        );
    }

    let page = trial_ui_sources();
    for required in [
        "setTrialToken(restoreTrialToken())",
        "persistTrialToken(value)",
        "removeRejectedTrialToken(rejectedValue)",
        "isTrialTokenRejected(reason)",
        "onAccessTokenRejected={rejectTrialToken}",
        "type=\"password\"",
        "autoComplete=\"off\"",
        "\"x-commandagent-trial-authorization\": `Bearer ${token.trim()}`",
    ] {
        assert!(
            page.contains(required),
            "Trial page is missing storage/authentication boundary {required:?}"
        );
    }

    let errors = std::fs::read_to_string("gui/lib/errors.ts").unwrap();
    assert!(errors.contains("reason as { code?: unknown }).code === \"trial_token_invalid\""));

    let smoke = std::fs::read_to_string("gui/scripts/storage-smoke.mjs").unwrap();
    for required in [
        "buildBasePath: \"/\"",
        "buildBasePath: \"/proxy/commandagent/\"",
        "reload_restored_token",
        "independent_tab_empty",
        "edited_value_persisted",
        "cleared_value_removed",
        "rejected_value_removed",
        "local_storage_excludes_tokens",
        "urls_exclude_tokens",
        "static_export_excludes_tokens",
        "console_and_errors_exclude_tokens",
        "server_diagnostics_exclude_tokens",
    ] {
        assert!(
            smoke.contains(required),
            "focused Trial storage smoke is missing {required:?}"
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
    for required in [
        "<div className=\"run-table\">",
        "<div className=\"run-table-head\" aria-hidden=\"true\">",
        "useResource<RunIndex>(\"runs\")",
        "data-testid=\"run-count\"",
        "${recentRuns.length} / ${runs.data.total}",
        "statusTone(run.state)",
        "{run.status_text}",
    ] {
        assert!(
            dashboard.contains(required),
            "Overview run contract is missing {required:?}"
        );
    }
    assert!(!dashboard.contains("role=\"table\""));
    assert!(!dashboard.contains("role=\"row\""));

    let types = std::fs::read_to_string("gui/lib/types.ts").unwrap();
    for required in [
        "export type RunIndex = {",
        "status: string;",
        "status_text: string;",
        "state: RunState;",
    ] {
        assert!(
            types.contains(required),
            "RunIndex types are missing {required:?}"
        );
    }

    let smoke = std::fs::read_to_string("gui/scripts/smoke.mjs").unwrap();
    for required in [
        "--overview-only",
        "statusBadgesArePlainText",
        "runCountText === expectedRunCountText",
    ] {
        assert!(
            smoke.contains(required),
            "Overview smoke is missing {required:?}"
        );
    }
}

#[test]
fn run_detail_and_measurement_read_only_browsing_contracts_are_pinned() {
    let run_page = std::fs::read_to_string("gui/app/runs/page.tsx").unwrap();
    for required in [
        "id=\"run-filter\"",
        "run.status_text",
        "filteredRuns.map",
        "label=\"実行未選択\"",
        "label=\"該当なし\"",
        "sourceHref={documentSourceHref}",
    ] {
        assert!(
            run_page.contains(required),
            "Run detail browsing contract is missing {required:?}"
        );
    }

    let viewer = std::fs::read_to_string("gui/components/document-viewer.tsx").unwrap();
    for required in [
        "sourceHref?: string | null",
        "data-testid=\"document-source-link\"",
        "target=\"_blank\"",
        "data-testid=\"document-wrap-toggle\"",
        "document-content--unwrapped",
    ] {
        assert!(
            viewer.contains(required),
            "document viewer contract is missing {required:?}"
        );
    }

    let states = std::fs::read_to_string("gui/components/states.tsx").unwrap();
    assert!(states.contains("label = \"記録なし\""));
    assert!(states.contains("<span className=\"state-code\">{label}</span>"));

    let measurements = std::fs::read_to_string("gui/app/measurements/page.tsx").unwrap();
    for required in [
        "data-testid=\"measurement-map-frame\"",
        "className=\"map-source-link\"",
        "原寸 SVG を開く",
        "apiPath(\"reports/view\"",
    ] {
        assert!(
            measurements.contains(required),
            "Measurements browsing contract is missing {required:?}"
        );
    }

    let smoke = std::fs::read_to_string("gui/scripts/smoke.mjs").unwrap();
    for required in [
        "--read-only",
        "options_include_dates_and_status",
        "filter_matches_id",
        "no_match_label_visible",
        "sourceLinkPresent",
        "mobileMap.horizontally_scrollable",
    ] {
        assert!(
            smoke.contains(required),
            "Issue 75 browser smoke is missing {required:?}"
        );
    }
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
        ("gui/app/runs/layout.tsx", "title: \"検証・運用レポート\""),
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
    assert!(runtime.contains("runtime_status(state.trial_access.authentication_enabled())"));
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
fn trial_status_polling_revalidates_and_backs_off_without_changing_the_schema() {
    let page = trial_ui_sources();
    for required in [
        "headers[\"if-none-match\"] = etag",
        "response.status === 304",
        "unchangedPollDelay(unchangedResponses)",
        "CHANGED_POLL_INTERVAL_MS",
    ] {
        assert!(
            page.contains(required),
            "trial status polling is missing {required:?}"
        );
    }
    assert!(!page.contains("setTimeout(() => void poll(), 750)"));

    let policy = std::fs::read_to_string("gui/lib/trial-monitor.ts").unwrap();
    for required in [
        "CHANGED_POLL_INTERVAL_MS = 1_000",
        "MAX_UNCHANGED_POLL_INTERVAL_MS = 10_000",
        "Math.min(CHANGED_POLL_INTERVAL_MS * 2 ** exponent, MAX_UNCHANGED_POLL_INTERVAL_MS)",
    ] {
        assert!(
            policy.contains(required),
            "adaptive polling policy is missing {required:?}"
        );
    }
    assert!(policy.contains("retryDelay(attempt: number)"));
    assert!(!std::path::Path::new("gui/lib/trial-polling.ts").exists());

    let types = std::fs::read_to_string("gui/lib/types.ts").unwrap();
    let schema = types
        .split("export type PolledSession = {")
        .nth(1)
        .and_then(|tail| tail.split("};").next())
        .unwrap();
    for field in [
        "id:",
        "gate:",
        "status:",
        "verdict:",
        "assurance:",
        "phases:",
        "event_count:",
        "acceptance_sheet:",
        "section5:",
        "events_path:",
    ] {
        assert!(schema.contains(field), "PolledSession lost {field}");
    }
    assert_eq!(schema.lines().filter(|line| line.contains(':')).count(), 10);

    let smoke = std::fs::read_to_string("gui/scripts/smoke.mjs").unwrap();
    for required in [
        "probeTenMinutePolling",
        "--polling-only",
        "durationMs = 600_000",
        "fixed_750ms_calls",
        "observed_call_count",
        "observed_calls: observedCalls",
        "observedCalls.length >= 50",
        "observedCalls.length <= 65",
        "reductionPercent >= 90",
    ] {
        assert!(
            smoke.contains(required),
            "polling smoke evidence is missing {required:?}"
        );
    }
}

#[test]
fn trial_feedback_uses_elapsed_time_phase_total_and_terminal_title() {
    let source = trial_ui_sources();
    for required in [
        "data-testid=\"elapsed-time\"",
        "window.setInterval(tick, 1_000)",
        "data-testid=\"mean-duration-comparison\"",
        "data-testid=\"phase-progress\"",
        "currentPhase.total > 0",
        "フェーズ {currentPhase.index} / {currentPhase.total}",
        "平均所要時間（予測ではありません）",
        "document.title = `✔ ${terminalHeading(session)} — CommandAgent`",
    ] {
        assert!(
            source.contains(required),
            "trial feedback is missing {required:?}"
        );
    }

    let smoke = std::fs::read_to_string("gui/scripts/smoke.mjs").unwrap();
    for required in [
        "--feedback-only",
        "probeTrialFeedback",
        "フェーズ 2 / 5",
        "平均 10.2 分",
        "elapsed_changed",
        "zero_total_hidden",
        "monitor_and_progress_separate",
        "title_changed",
    ] {
        assert!(
            smoke.contains(required),
            "Trial feedback smoke is missing {required:?}"
        );
    }
}

#[test]
fn trial_ui_renders_one_japanese_labeled_state_with_mobile_primary_actions() {
    let source = trial_ui_sources();
    for required in [
        "aria-label=\"Trial の進行状況\"",
        "[\"依頼\", \"Gate 1\"]",
        "[\"確認\", \"Gate 1\"]",
        "[\"実行\", \"Gate 2\"]",
        "[\"結果\", \"Gate 3 / 4\"]",
        "data-testid=\"trial-active-stage\"",
        "data-stage={stage}",
        "stage === \"compose\"",
        "proposal !== null && stage === \"gate_1\"",
        "stage === \"gate_2\" && created !== null",
        "stage === \"terminal\" && session !== null",
        "className=\"trial-action-bar trial-request-actions\"",
        "className=\"gate-one-actions trial-action-bar\"",
    ] {
        assert!(
            source.contains(required),
            "trial state layout is missing {required:?}"
        );
    }
    for forbidden in [
        "proposal !== null && (stage === \"gate_1\" || stage === \"gate_2\")",
        "(stage === \"gate_2\" || stage === \"terminal\") && created !== null",
    ] {
        assert!(
            !source.contains(forbidden),
            "trial state layout retains accumulated UI {forbidden:?}"
        );
    }

    let css = std::fs::read_to_string("gui/app/globals.css").unwrap();
    for required in [
        ".trial-stage-compose,\n  .trial-stage-gate_1",
        ".trial-request-actions,\n  .gate-one-actions",
        "bottom: calc(4.65rem + env(safe-area-inset-bottom));",
        "grid-template-areas:\n      \"action\"\n      \"verdict\";",
    ] {
        assert!(
            css.contains(required),
            "mobile state CSS is missing {required:?}"
        );
    }

    let smoke = std::fs::read_to_string("gui/scripts/smoke.mjs").unwrap();
    for required in [
        "probeTrialLayout(",
        "{ width: 390, height: 844 }",
        "const expectedLabels = [\"依頼\", \"確認\", \"実行\", \"結果\"]",
        "primary_in_initial_viewport",
        "one_state_visible",
    ] {
        assert!(
            smoke.contains(required),
            "layout smoke is missing {required:?}"
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
    let directives = std::fs::read_to_string(DIRECTIVES_MODULE).unwrap();
    let gate_one = std::fs::read_to_string("src/bin/gui_server/gate_one.rs").unwrap();
    let sessions = std::fs::read_to_string("src/bin/gui_server/sessions.rs").unwrap();
    let trial_handlers = format!("{delegate}{directives}{gate_one}{sessions}");
    assert_eq!(
        trial_handlers
            .matches("require_trial(&state, &headers")
            .count(),
        6,
        "every Trial API handler must enforce workspace and access guards"
    );
    let session_index = std::fs::read_to_string("src/bin/gui_server/session_index.rs").unwrap();
    assert_eq!(
        session_index
            .matches("require_trial(&state, &headers, false)")
            .count(),
        1,
        "the Trial session index must enforce workspace and access guards"
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
            trial_handlers.contains(required),
            "missing Trial guard {required:?}"
        );
    }

    assert!(entry.contains(".route(\"/api/trial-workspace\", get(sessions::workspace_status))"));
}

#[test]
fn trial_workspace_recovery_is_visible_but_read_only() {
    let page = trial_ui_sources();
    for required in [
        "apiPath(\"trial-workspace\")",
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

#[test]
fn trial_session_files_are_get_only_authenticated_views() {
    let entry = std::fs::read_to_string("src/bin/gui_server.rs").unwrap();
    for required in [
        "\"/api/sessions/{id}/artifacts\"",
        "get(session_files::artifacts)",
        "\"/api/sessions/{id}/events\"",
        "get(session_files::events)",
    ] {
        assert!(
            entry.contains(required),
            "missing GET-only route {required:?}"
        );
    }
    for forbidden in [
        "post(session_files::artifacts)",
        "post(session_files::events)",
    ] {
        assert!(
            !entry.contains(forbidden),
            "session file route can mutate: {forbidden:?}"
        );
    }

    let files = std::fs::read_to_string(SESSION_FILES_MODULE).unwrap();
    assert_eq!(
        files
            .matches("session_run_root(&state, &id, &headers)")
            .count(),
        2
    );
    assert_eq!(
        files
            .matches("require_trial(state, headers, false)")
            .count(),
        1
    );
    for required in [
        "require_session_id(id)",
        "checked_existing_path_without_symlinks",
        "MAX_LIST_ENTRIES",
        "MAX_TEXT_BYTES",
        "MAX_EVENT_TAIL_LINES",
        "spawn_blocking",
    ] {
        assert!(
            files.contains(required),
            "missing session read guard {required:?}"
        );
    }

    let page = trial_ui_sources();
    for required in [
        "data-testid=\"trial-events-footer\"",
        "data-testid=\"trial-events-open\"",
        "data-testid={artifact.path === \"summary.md\" ? \"trial-summary-open\" : undefined}",
        "data-testid=\"trial-file-viewer\"",
        "headers: trialAuthorizationHeaders(token)",
        "直近 200 行",
    ] {
        assert!(
            page.contains(required),
            "missing Trial file viewer {required:?}"
        );
    }

    let delegate = std::fs::read_to_string(DELEGATE_MODULE).unwrap();
    assert_eq!(delegate.matches(".stdout(Stdio::null())").count(), 1);
    assert_eq!(delegate.matches(".stderr(Stdio::null())").count(), 1);
}

#[test]
fn trial_session_index_is_bounded_read_only_and_reconnects_by_link() {
    let entry = std::fs::read_to_string("src/bin/gui_server.rs").unwrap();
    assert!(entry.contains("get(session_index::list).post(delegate::create)"));

    let index = std::fs::read_to_string("src/bin/gui_server/session_index.rs").unwrap();
    for required in [
        "const MAX_SESSIONS: usize = 100",
        "require_trial(&state, &headers, false)",
        "workspace.join(\".anvil/runs\")",
        "has_confirmation_record",
        ".lease_snapshot()",
        "started_epoch_seconds",
        "id.get_version_num() == 7",
        "gate: Option<&'static str>",
        "full_terminal_without_sheet",
        "let right_is_active = active_session == Some(right.id.as_str())",
        ".cmp(&left_is_active)",
        "sessions.truncate(MAX_SESSIONS)",
        "human_directive_continuation_started",
    ] {
        assert!(
            index.contains(required),
            "Trial session index is missing {required:?}"
        );
    }

    let page = trial_ui_sources();
    let panel = [
        "gui/components/trial-session-index.tsx",
        "gui/lib/trial-api.ts",
    ]
    .iter()
    .map(|path| std::fs::read_to_string(path).unwrap())
    .collect::<String>();
    let smoke = std::fs::read_to_string("gui/scripts/smoke.mjs").unwrap();
    let lifecycle_smoke = std::fs::read_to_string("gui/scripts/session-index-smoke.mjs").unwrap();
    for required in [
        "data-testid=\"trial-session-index\"",
        "fetchSessionIndex(token)",
        "fetchJson<TrialSessionIndex>(apiPath(\"sessions\"), {",
        "cache: \"no-store\"",
        "x-commandagent-trial-authorization",
        "session.started_epoch_seconds",
        "session.modified_epoch_seconds",
        "session.gate ?? \"unknown\"",
        "href={sessionLink(session.id)}",
        "return `?session=${encodeURIComponent(id)}`",
        "data-testid=\"session-reconnect-link\"",
        "data-testid=\"trial-session-auth-required\"",
        "data-testid=\"trial-session-freshness\"",
        "最後に取得できた一覧を表示しています。",
        "window.addEventListener(\"focus\", refresh)",
        "document.addEventListener(\"visibilitychange\", refreshWhenVisible)",
        "previous === \"running\"",
        "runtimeLease === \"idle\" || runtimeLease === \"recovery_required\"",
        "mergeObservedSession",
    ] {
        assert!(
            panel.contains(required),
            "Trial session list panel is missing {required:?}"
        );
    }
    assert!(
        !panel.contains("setInterval"),
        "Trial session index must not add an independent polling interval"
    );
    assert!(
        !panel.contains("useRuntimeStatus("),
        "Trial session index must share the Shell runtime projection"
    );
    for required in [
        "<TrialSessionIndexPanel",
        "trialTokenAuthEnabled",
        "trialAccessReady",
        "data-testid=\"trial-token-auth-disabled\"",
        "observedSession={observedSession}",
        "onLeaseChange={setWorkspaceLease}",
        "revalidationKey={sessionIndexRevision}",
        "setSessionIndexRevision((current) => current + 1)",
        "data-testid=\"terminal-session-history-link\"",
        "launchBlockReason !== null",
        "実行中のセッション ${lease.session_id} がワークスペースを使用しているため",
    ] {
        assert!(
            page.contains(required),
            "Trial session list UI is missing {required:?}"
        );
    }
    assert!(panel.contains("!tokenAuthEnabled || trimmedToken.length"));
    for forbidden in [
        "Delete session",
        "Remove session",
        "Clear workspace lease",
        "Force idle",
    ] {
        assert!(
            !page.contains(forbidden) && !panel.contains(forbidden),
            "Trial session list exposes forbidden mutation {forbidden:?}"
        );
    }
    for required in [
        "probeSessionIndexLease",
        "lease: { status: \"running\", session_id: sessionId }",
        "launch_disabled: launchDisabled",
        "dispatchCount === 0",
    ] {
        assert!(
            smoke.contains(required),
            "Trial session index smoke is missing {required:?}"
        );
    }
    for required in [
        "buildBasePath: \"/\"",
        "buildBasePath: \"/proxy/commandagent/\"",
        "no_periodic_index_polling",
        "optimistic launch row state",
        "terminal transition refresh",
        "refresh failure removed the last successful row",
        "focus refresh",
        "visible-tab refresh",
        "reconnect_get_only",
        "repository-only",
        "trial-only",
        "both",
        "trial-unauthenticated",
    ] {
        assert!(
            lifecycle_smoke.contains(required),
            "Trial lifecycle smoke is missing {required:?}"
        );
    }

    let shell = std::fs::read_to_string("gui/components/shell.tsx").unwrap();
    assert!(shell.contains("RuntimeStatusContext.Provider value={runtime}"));
    assert!(shell.contains("label: \"検証・運用レポート\""));
    let runs = std::fs::read_to_string("gui/app/runs/page.tsx").unwrap();
    let dashboard = std::fs::read_to_string("gui/app/page.tsx").unwrap();
    for required in [
        "REPOSITORY / workspace/management/runs",
        "GUI Trial の execution root ではなく",
    ] {
        assert!(
            runs.contains(required),
            "repository report source UI is missing {required:?}"
        );
    }
    assert!(dashboard.contains("参照元: workspace/management/runs"));
    assert!(panel.contains("EXECUTION ROOT / .anvil/runs"));
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

fn trial_ui_sources() -> String {
    [
        "gui/app/try/page.tsx",
        "gui/components/trial-run.tsx",
        "gui/hooks/use-trial-run.ts",
        "gui/lib/trial-api.ts",
    ]
    .iter()
    .map(|path| std::fs::read_to_string(path).unwrap())
    .collect()
}

fn gui_source_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(root).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            if !matches!(
                path.file_name().and_then(|name| name.to_str()),
                Some(".next" | "node_modules" | "out")
            ) {
                files.extend(gui_source_files(&path));
            }
        } else if matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("ts" | "tsx" | "js" | "mjs")
        ) {
            files.push(path);
        }
    }
    files
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
        || source.contains(".env(\"COMMANDAGENT_PACK_")
        || (path != Path::new(DELEGATE_MODULE)
            && ["std::process", "Command::new", ".spawn("]
                .iter()
                .any(|token| source.contains(token)))
        || (path == Path::new(DELEGATE_MODULE)
            && ["Command::new", ".spawn(", ".status()"]
                .iter()
                .any(|token| source.contains(token))
            && !source.contains(".env_clear()"))
}
