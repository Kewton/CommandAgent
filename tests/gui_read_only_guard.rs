use std::path::{Path, PathBuf};

const DELEGATE_MODULE: &str = "src/bin/gui_server/delegate.rs";
const DIRECTIVES_MODULE: &str = "src/bin/gui_server/directives.rs";
const SESSION_FILES_MODULE: &str = "src/bin/gui_server/session_files.rs";
const SESSION_PATHS_MODULE: &str = "src/bin/gui_server/session_paths.rs";
const SERVER_ROOT_MODULE: &str = "src/bin/gui_server.rs";
const TRIAL_OPTIONS_MODULE: &str = "src/bin/gui_server/trial_options.rs";

#[test]
fn gui_server_mutates_only_init_roots_or_through_the_confirmed_cli_delegate() {
    let mut sources = vec![PathBuf::from(SERVER_ROOT_MODULE)];
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
        "tokio::process",
        "fs::write",
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
        if path != Path::new(SERVER_ROOT_MODULE) && path != Path::new(SESSION_PATHS_MODULE) {
            for token in ["fs::create_dir", "fs::set_permissions"] {
                assert!(
                    !source.contains(token),
                    "{} can mutate startup roots outside gui_server --init: {token:?}",
                    path.display()
                );
            }
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
        if path != Path::new(TRIAL_OPTIONS_MODULE) {
            assert!(
                !source.contains("reqwest"),
                "{} can make provider discovery requests outside trial_options",
                path.display()
            );
        }
    }

    let server_root = std::fs::read_to_string(SERVER_ROOT_MODULE).unwrap();
    for required in [
        "#[arg(long, conflicts_with = \"check\")]",
        "if arguments.init {\n        initialize_defaults(&mut arguments)?;\n    }",
        "if arguments.execution_root.is_none()",
        "if arguments.extension_root.is_none()",
        "refusing to initialize private GUI root through symlink",
        "std::fs::create_dir_all(root)",
        "std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o700))",
    ] {
        assert!(
            server_root.contains(required),
            "GUI startup mutation guard is missing {required:?}"
        );
    }
    assert_eq!(server_root.matches("std::fs::create_dir_all").count(), 1);
    assert_eq!(server_root.matches("std::fs::set_permissions").count(), 1);

    let delegate = std::fs::read_to_string(DELEGATE_MODULE).unwrap();
    for required in [
        "shell.confirm(confirmation_hash)",
        ".trial_workspace\n        .acquire(&id)\n        .map_err(workspace_conflict)",
        ".trial_workspace\n        .require_current()",
        "Gate 1 workspace changed before CLI delegation",
        ".dispatch(|confirmed|",
        "paths.create_execution_workspace()",
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
        "apply_confirmed_pack(&mut command, state, identity)",
        "PackSelection::Pinned",
        "locator.locate_pinned_from(*source, id, version, Some(hash))",
        ".args([\"--pack-hash\", hash])",
        ".env(PACK_DIRECTORY_ENV, directory)",
    ] {
        assert!(
            delegate.contains(required),
            "CLI delegation guard is missing {required:?}"
        );
    }
    assert!(
        delegate.find("shell.confirm(confirmation_hash)")
            < delegate.find("paths.create_execution_workspace()"),
        "session workspace creation must remain after Gate 1 confirmation"
    );
    for shell_bypass in ["sh\")", "bash\")", ".arg(\"-c\")"] {
        assert!(
            !delegate.contains(shell_bypass),
            "delegate contains an unbounded process surface {shell_bypass:?}"
        );
    }
    let allowlist_end = delegate
        .find("];")
        .expect("delegate environment allowlist must be closed");
    assert!(
        !delegate[..allowlist_end].contains("COMMANDAGENT_PACK_"),
        "delegate parent allowlist admits ambient pack selectors"
    );

    let trial_options = std::fs::read_to_string(TRIAL_OPTIONS_MODULE).unwrap();
    for required in [
        "client.get(",
        ".redirect(Policy::none())",
        "Provider::Ollama",
        "Provider::LmStudio",
    ] {
        assert!(
            trial_options.contains(required),
            "provider discovery guard is missing {required:?}"
        );
    }
    for forbidden in ["client.post(", "client.put(", "client.delete(", ".body("] {
        assert!(
            !trial_options.contains(forbidden),
            "provider discovery contains mutating request surface {forbidden:?}"
        );
    }

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
        "extensions_disabled",
        "extension_invalid_request",
        "extension_conflict",
        "extension_verification_failed",
        "profile_auth_failed",
        "profile_origin_not_allowed",
        "profile_body_too_large",
        "profile_validation_failed",
        "profile_confirmation_stale",
        "profile_conflict",
        "profile_io_failed",
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
        ("gui/hooks/use-trial-compose.ts", "describeError"),
        ("gui/hooks/use-trial-terminal.ts", "describeError"),
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
    for (path, surface) in [
        ("gui/app/try/page.tsx", "compose"),
        ("gui/app/try/status/page.tsx", "status"),
        ("gui/app/try/history/page.tsx", "history"),
        ("gui/app/try/history/detail/page.tsx", "detail"),
    ] {
        let page = std::fs::read_to_string(path).unwrap();
        assert!(page.lines().count() <= 25, "{path} grew beyond wiring");
        for required in ["<Shell", "<TrialPageNavigation"] {
            assert!(page.contains(required), "{path} is missing {required:?}");
        }
        let run = format!("<TrialRun surface=\"{surface}\" />");
        assert!(page.contains(&run), "{path} is missing {run:?}");
        for forbidden in ["useState", "useEffect", "fetch(", "data-testid"] {
            assert!(
                !page.contains(forbidden),
                "{path} owns non-wiring behavior {forbidden:?}"
            );
        }
    }

    let component = std::fs::read_to_string("gui/components/trial-run.tsx").unwrap();
    assert!(component.contains("useTrialRun(terminalHeading, { loadComposeOptions:"));
    assert!(component.contains("data-testid=\"trial-active-stage\""));
    assert!(component.contains("useTrialPageRouting(surface, stage, sessionId)"));

    let hook = std::fs::read_to_string("gui/hooks/use-trial-run.ts").unwrap();
    for required in ["export function useTrialRun", "useState", "useEffect"] {
        assert!(
            hook.contains(required),
            "Trial workflow hook is missing {required:?}"
        );
    }
    let compose_hook = std::fs::read_to_string("gui/hooks/use-trial-compose.ts").unwrap();
    assert!(
        compose_hook.contains("isTrialTokenRejected(reason)"),
        "Trial compose hook is missing shared token-rejection handling"
    );

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
    assert_eq!(gui.matches("function dateTimeLabel(").count(), 1);
}

#[test]
fn trial_ui_keeps_gate_one_confirmation_and_has_no_intervention_surface() {
    let source = trial_ui_sources();
    for required in [
        "if (!confirmed || proposal === null)",
        "\"x-commandagent-trial-authorization\": `Bearer ${token.trim()}`",
        "createSession(trialToken, spec, proposal.card_hash)",
        "<GateCardMarkdown markdown={proposal.card_markdown} />",
        "data-testid=\"trial-workspace\"",
        "proposal.identity.workspace",
        "このディレクトリ内の内容だけを作成・変更・削除できます",
        "apiPath(\"trial-options\")",
        "apiPath(\"pack-options\")",
        "trialOptions.profiles.map",
        "trialOptions.providers.map",
        "<fieldset className=\"trial-role-fields\" data-testid=\"trial-executor-role\">",
        "<legend>Executor / 実行</legend>",
        "data-testid=\"trial-provider\"",
        "data-testid=\"trial-planner-provider\"",
        "<fieldset className=\"trial-role-fields\" data-testid=\"trial-planner-role\">",
        "<legend>Planner / 計画</legend>",
        "data-testid=\"trial-intent\"",
        "<option value=\"\">自動判定</option>",
        "<option value=\"create\">作成</option>",
        "<option value=\"fix\">修正</option>",
        "<option value=\"investigate\">調査</option>",
        "update(\"intent\"",
        "update(\"provider\", event.target.value)",
        "update(\"planner_provider\", event.target.value)",
        "data-testid=\"trial-think\"",
        "disabled={launchIdentityLocked || !ollamaRoleSelected}",
        "update(\"think\"",
        "next.provider === \"ollama\" || next.planner_provider === \"ollama\"",
        "{ ...next, think: null }",
        "data-testid=\"trial-pack\"",
        "option.source_label",
        "data-testid=\"trial-profile-description\"",
        "data-testid=\"trial-provider-model-hint\"",
        "data-testid=\"trial-executor-model-warning\"",
        "data-testid=\"trial-planner-model-warning\"",
        "id=\"trial-executor-provider-model-options\"",
        "id=\"trial-planner-provider-model-options\"",
        "apiPath(\"provider-models\"",
        "data-testid=\"gate-one-primer\"",
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
        "resultSummary(session)",
        "reasonSummary(session)",
        "nextActionSummary(session)",
        "受入シートの詳細を表示",
        "<h2>セッションファイル</h2>",
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
        5,
        "goal, token, intent, and both model controls must share the run-stage lock"
    );
    assert_eq!(
        source
            .matches("disabled={launchIdentityLocked || trialOptions === null}")
            .count(),
        3,
        "profile and both provider controls must combine option loading with the run-stage lock"
    );
    assert_eq!(
        source
            .matches("disabled={launchIdentityLocked || trialOptions === null || selectedProfile?.status === \"draft\"}")
            .count(),
        1,
        "the pack control must also stay disabled for an external draft profile"
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
        "probeTrialRoleLayouts(page)",
        "providerChangesPreserveModels",
        "roleLayouts.ok",
        "terminalTitle === expectedTerminalTitle",
        "!terminalTitle.includes(\"✔\")",
        "code: \"trial_workspace_running\"",
        "gateOneText.includes(\"GATE 1 / 見積り\")",
        "const expectedFinalGateLabel = finalApi.body.gate === \"gate_3\"",
        "const text = row.innerText",
        "{ gateLabel: expectedFinalGateLabel, id: sessionId }",
        "conflictGuidance.includes(`セッション ${sessionId} に再接続`)",
        "accessible_name_matches:",
        "name: `セッション ${sessionId} に再接続`",
        "conflictReconnectButtonContract.tag_name === \"BUTTON\"",
        "conflictReconnectButtonContract.type === \"button\"",
        "conflictReconnectButtonContract.visible",
    ] {
        assert!(
            smoke.contains(browser_check),
            "GUI smoke is missing browser acceptance check {browser_check:?}"
        );
    }
    for obsolete in [
        "const text = row.textContent ?? \"\"",
        "{ gate: finalApi.body.gate, id: sessionId }",
        "const conflictReconnectHref",
    ] {
        assert!(
            !smoke.contains(obsolete),
            "GUI smoke restored obsolete visible/reconnect contract {obsolete:?}"
        );
    }
    assert!(
        !source.contains("planner_provider: value as string"),
        "execution-provider changes must not rewrite the planning provider"
    );
    let run_identity = std::fs::read_to_string("gui/components/trial-run-identity.tsx").unwrap();
    assert!(
        run_identity.contains("identity.pins.think !== undefined")
            && run_identity.contains("data-testid=\"trial-run-identity-think\""),
        "the selected thinking value must remain visible in the frozen run identity"
    );
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
        "const parameters = new URLSearchParams(window.location.search)",
        "parameters.get(\"session\")",
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
        "lifecycleReconnectCalls.every((call) => call.method === \"GET\")",
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
    assert!(styles.contains(".trial-role-fields {\n  min-width: 0;\n  grid-column: 1 / -1;"));
    assert!(styles.contains(
        ".trial-role-controls {\n  display: grid;\n  grid-template-columns: repeat(2, minmax(0, 1fr));"
    ));
    assert!(
        styles.contains(
            "  .trial-fields,\n  .trial-role-controls {\n    grid-template-columns: 1fr;"
        )
    );
    assert!(styles.contains(
        ".session-index,\n.session-list li,\n.gate-one-grid,\n.execution-panel,\n.terminal-grid {\n  scroll-margin-top: 4.5rem;"
    ));
    for required in [
        ".runtime-summary {\n  flex: 0 0 auto;\n  gap: 0.4rem;\n  white-space: nowrap;",
        ".runtime-badge {\n  flex: 0 0 auto;",
        ".getting-started-close {\n  flex: 0 0 auto;",
        "  .topbar {\n    gap: 0.5rem;",
    ] {
        assert!(
            styles.contains(required),
            "mobile single-line header contract is missing {required:?}"
        );
    }

    let dashboard = std::fs::read_to_string("gui/app/page.tsx").unwrap();
    for required in [
        "<div className=\"run-table\" role=\"table\" aria-label=\"最近の実行記録\">",
        "<div role=\"rowgroup\">",
        "<div className=\"run-table-head\" role=\"row\">",
        "<span role=\"columnheader\">実行ID</span>",
        "<div className=\"run-row\" role=\"row\" data-run-id={run.id}",
        "<strong role=\"cell\"><a href={href}>{run.id}</a></strong>",
        "<span role=\"cell\">",
        "useResource<RunIndex>(\"runs\")",
        "data-testid=\"run-count\"",
        "data-testid=\"run-total-count\"",
        "最近の実行記録（一覧に表示中）",
        "保存済みの実行記録（総数）",
        "${recentRuns.length} 件",
        "${runs.data.total} 件",
        "statusTone(run.state)",
        "repositoryRunStatusLabel(run.state, run.status_text)",
        "title={`記録上の状態: ${label}`}",
        "{label}",
    ] {
        assert!(
            dashboard.contains(required),
            "Overview run contract is missing {required:?}"
        );
    }
    assert!(!dashboard.contains("aria-hidden=\"true\""));

    let package = std::fs::read_to_string("gui/package.json").unwrap();
    assert!(package.contains("\"axe-core\": \"4.10.3\""));
    let smoke = std::fs::read_to_string("gui/scripts/smoke.mjs").unwrap();
    for required in [
        "values: [\"aria-required-children\"]",
        "axeAriaRequiredChildren.violationCount === 0",
        "axe_aria_required_children: axeAriaRequiredChildren",
    ] {
        assert!(
            smoke.contains(required),
            "Overview axe contract is missing {required:?}"
        );
    }

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
        "getting_started_close",
        "running_header_mobile_390",
        "runtimeHeaderLayout(page)",
        "singleLineTextLayout(gettingStartedClose)",
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
        "data-testid=\"run-direct-open\"",
        "data-testid=\"run-index-count\"",
        "`表示件数 ${runs.data.runs.length} / 総数 ${runs.data.total}`",
        "selectedRunIsOutsideFilter",
        "data-testid=\"run-selected-id\"",
        "overflowWrap: \"anywhere\"",
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

    for required in [
        "type RunOwned<T>",
        "loadedDetail?.runId === runId",
        "selectedEvidence?.runId === runId",
        "requestVersion.current === version",
        "evidenceController.current?.abort()",
        "setLoadedDetail(null)",
        "setSelectedEvidence(null)",
    ] {
        assert!(
            run_page.contains(required),
            "Run detail request ownership contract is missing {required:?}"
        );
    }

    let viewer = std::fs::read_to_string("gui/components/document-viewer.tsx").unwrap();
    for required in [
        "sourceHref?: string | null",
        "headingLevel?: 2 | 3",
        "const Heading = headingLevel === 2 ? \"h2\" : \"h3\"",
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
        "<h2 style={{ margin: 0 }}>",
        "<span>レポート一覧</span>",
        "headingLevel={3}",
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
        "count_matches_index_total",
        "direct_lookup",
        "heading_order_valid",
        "mobile_id_fits",
        "no_match_label_visible",
        "request_ownership",
        "empty_selection_cleared",
        "sourceLinkPresent",
        "mobileMap.fits_without_axis_scroll",
        "const repositoryRunStatusLabel = (state, statusText) => {",
        "normalizedEnumValue(statusText) === \"recorded\" ? \"記録あり\" : \"進行中\"",
        "normalizedEnumValue(statusText) === \"not_recorded\" ? \"未記録\" : \"判定不能\"",
        "const status = repositoryRunStatusLabel(run.state, run.status_text);",
        "`${date} — ${status} — ${run.id}`",
    ] {
        assert!(
            smoke.contains(required),
            "Issue 75 browser smoke is missing {required:?}"
        );
    }
    assert!(!smoke.contains("`${date} — ${run.status_text} — ${run.id}`"));
}

#[test]
fn measurement_filter_and_mobile_map_fit_are_pinned() {
    let measurements = std::fs::read_to_string("gui/app/measurements/page.tsx").unwrap();
    for required in [
        "data-testid=\"report-filter\"",
        "data-testid=\"report-filter-count\"",
        "toLocaleLowerCase(\"ja-JP\")",
        "`${report.id} ${report.path}`",
        "filteredReports.map",
        "label=\"該当なし\"",
        "画面幅に合わせた全体図",
        "aria-label=\"スコアと時間の計測マップ\"",
    ] {
        assert!(
            measurements.contains(required),
            "measurement filtering contract is missing {required:?}"
        );
    }
    assert!(!measurements.contains("横スクロールできるスコアと時間の計測マップ"));

    let styles = std::fs::read_to_string("gui/app/globals.css").unwrap();
    assert!(styles.contains(".measure-map .map-frame {\n    overflow: hidden;"));
    assert!(styles.contains(
        ".measure-map .map-frame img {\n    width: 100%;\n    max-width: 100%;\n    min-width: 0;\n    height: auto;"
    ));
    assert!(!styles.contains("min-width: 70rem"));

    let smoke = std::fs::read_to_string("gui/scripts/smoke.mjs").unwrap();
    for required in [
        "filter_matches_path",
        "no_match_label_visible",
        "count_restored",
        "selection_retained_after_filter",
        "fits_single_viewport",
        "fits_without_axis_scroll",
        "has_horizontal_overflow",
        "has_vertical_overflow",
        "image_fits_frame",
    ] {
        assert!(
            smoke.contains(required),
            "Issue 185 browser smoke is missing {required:?}"
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
    assert!(shell.contains("import Link from \"next/link\""));
    assert_eq!(shell.matches("<Link").count(), 3);
    assert!(
        !shell.contains("<a "),
        "shell navigation must stay client-side"
    );
    assert!(shell.contains("aria-current={item.route === active ? \"page\" : undefined}"));
    assert_eq!(
        shell
            .lines()
            .filter(|line| line.trim_start().starts_with("{ route: \""))
            .count(),
        5
    );
    assert!(shell.contains("{ route: \"assets\", label: \"拡張\", index: \"03\" }"));
    for required in [
        "data-testid=\"runtime-status\"",
        "data-trial-available",
        "data-session-state",
        "aria-atomic=\"true\"",
        "aria-live=\"polite\"",
        "トライアル利用可",
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
    let getting_started = std::fs::read_to_string("gui/components/getting-started.tsx").unwrap();
    for required in [
        "data-testid=\"getting-started\"",
        "data-testid=\"getting-started-sample\"",
        "data-testid=\"getting-started-close\"",
        "window.sessionStorage",
        "?sample=python-cli",
    ] {
        assert!(
            getting_started.contains(required),
            "getting-started guide is missing {required:?}"
        );
    }
    let styles = std::fs::read_to_string("gui/app/globals.css").unwrap();
    assert!(styles.contains("grid-template-columns: repeat(5, minmax(0, 1fr));"));
    assert!(styles.contains(".page-intro > p {\n    display: none;"));

    let titles = [
        ("gui/app/layout.tsx", "default: \"概要 | CommandAgent\""),
        ("gui/app/try/layout.tsx", "title: \"トライアル実行指示\""),
        (
            "gui/app/try/status/layout.tsx",
            "title: \"トライアル実行状況\"",
        ),
        (
            "gui/app/try/history/layout.tsx",
            "title: \"トライアル実行履歴\"",
        ),
        (
            "gui/app/try/history/detail/layout.tsx",
            "title: \"トライアル実行結果詳細\"",
        ),
        ("gui/app/runs/layout.tsx", "title: \"リポジトリ実行記録\""),
        ("gui/app/assets/layout.tsx", "title: \"拡張\""),
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
    for required in [
        "state.trial_workspace.runtime_status(authentication_enabled)",
        "execution_root: execution_root(&state)",
        "extension_root: extension_root(&state)",
        "commandagent_binary: commandagent_binary(&state.commandagent_bin)",
        "trial_authentication:",
        "status: \"unconfigured\"",
        "status: \"action_required\"",
    ] {
        assert!(
            runtime.contains(required),
            "runtime status is missing {required:?}"
        );
    }
}

#[test]
fn extension_catalog_keeps_supply_warnings_and_trial_handoff_explicit() {
    let server = std::fs::read_to_string("src/bin/gui_server/pack_catalog.rs").unwrap();
    for required in [
        "PackSource::Repository",
        "PackSource::Local",
        "PackSource::Admitted",
        "hash と pin が一致しません。",
        "pack が現在の profile / intent 契約と非互換です。",
        "ローカル優先: 同名のリポジトリ pack より拡張ルートを優先",
        "trial_eligible",
    ] {
        assert!(
            server.contains(required),
            "catalog server is missing {required:?}"
        );
    }

    let page = std::fs::read_to_string("gui/app/assets/page.tsx").unwrap();
    for required in [
        "title=\"拡張\"",
        "data-testid=\"extension-pack-row\"",
        "data-testid=\"pack-warning\"",
        "{pack.source_label}",
        "{pack.expected_hash ?? \"未固定\"}",
        "{pack.observed_hash ?? \"算出不可\"}",
        "pack.trial_eligible && pack.intent !== null",
        "トライアルで使う",
        "routePath(\"try\")",
    ] {
        assert!(
            page.contains(required),
            "extension page is missing {required:?}"
        );
    }

    let trial = std::fs::read_to_string("gui/hooks/use-trial-compose.ts").unwrap();
    for required in [
        "packPreselectionApplied",
        "new URLSearchParams(window.location.search).get(\"pack\")",
        "intent: option.intent",
        "field === \"profile\" || field === \"intent\"",
        "option.intent === (spec.intent ?? \"create\")",
    ] {
        assert!(
            trial.contains(required),
            "Trial preselection is missing {required:?}"
        );
    }

    let smoke = std::fs::read_to_string("gui/scripts/smoke.mjs").unwrap();
    for required in [
        "probeExtensionCatalog(page)",
        "probeTrialComposeRegression(",
        "proposalBody.pack === null",
        "explicitBody.intent === \"fix\"",
        "!(\"intent\" in proposalBody)",
        "compatibleInvestigateOptions.includes(investigateSelector)",
        "intentChangeClearedPack",
        "profileChangeClearedPack",
        "sourceLabels.includes(\"承認済み\")",
        "sourceLabels.includes(\"リポジトリ（未承認）\")",
        "selectedPack === selector",
    ] {
        assert!(
            smoke.contains(required),
            "extension smoke is missing {required:?}"
        );
    }
}

#[test]
fn extension_catalog_defines_layers_and_keeps_admission_controls_out_of_the_gui() {
    let page = std::fs::read_to_string("gui/app/assets/page.tsx").unwrap();
    for required in [
        "4 レイヤーと依存関係",
        "Layer 1",
        "Layer 2",
        "Layer 3",
        "Layer 4",
        "能力語彙",
        "下書きプロファイル",
        "パック供給",
        "Admission",
        "GUI 変更不可",
        "data-testid=\"extension-root-status\"",
        "runtime?.data?.prerequisites.extension_root",
        "data-testid=\"extension-profile-row\"",
        "profile.manifest_hash",
        "profile.assurance_ceiling",
        "profile_not_admitted",
        "data-testid=\"profile-registration-issue-link\"",
        "github.com/Kewton/CommandAgent/issues/new",
        "Contract / Suite は拡張種別ではありません",
        "<PackWizard onCatalogChange={packs.refresh} />",
        "<ProfileWizard enabled={extensionRootStatus === \"ready\"} />",
        "packUnavailableReason(pack)",
        "[\"layer\", \"Layer 3 / pack supply\"]",
        "[\"source\", pack.source_label]",
        "[\"status\", packStatusLabel(pack)]",
        "[\"hash\", pack.observed_hash ?? \"算出不可\"]",
        "[\"assurance\", packAssuranceLabel(pack)]",
        "[\"登録／昇格\", packRegistrationLabel(pack)]",
    ] {
        assert!(
            page.contains(required),
            "extension layer page is missing {required:?}"
        );
    }

    for forbidden in ["admitExtension", "promoteExtension", "addCapability"] {
        assert!(
            !page.contains(forbidden),
            "extension page exposes forbidden self-promotion control {forbidden:?}"
        );
    }

    let trial = std::fs::read_to_string("src/bin/gui_server/gate_one.rs").unwrap();
    let delegate = std::fs::read_to_string("src/bin/gui_server/delegate.rs").unwrap();
    assert!(trial.contains("pack_catalog::select_with_locator"));
    assert!(trial.contains("render_gate_one_for_gui(&identity, &locator)"));
    assert!(delegate.contains("--pack-hash"));
}

#[test]
fn extension_pack_wizard_delegates_lifecycle_and_keeps_failures_actionable() {
    let page = std::fs::read_to_string("gui/app/assets/page.tsx").unwrap();
    for required in [
        "<PackWizard onCatalogChange={packs.refresh} />",
        "role=\"tablist\"",
        "role=\"tab\"",
        "aria-selected={tab === item}",
        "role=\"tabpanel\"",
        "case \"ArrowRight\"",
        "case \"ArrowLeft\"",
        "aria-expanded={open}",
        "<i aria-hidden=\"true\">",
        "data-testid=\"pack-warning-status\" role=\"status\"",
        "data-testid=\"pack-warning\" role=\"note\"",
        "{name}: {present ? \"あり\" : \"なし\"}",
    ] {
        assert!(
            page.contains(required),
            "extension page is missing synchronized or accessible behavior {required:?}"
        );
    }

    let wizard = std::fs::read_to_string("gui/components/pack-wizard.tsx").unwrap();
    for required in [
        "対象セル",
        "const exampleAvailable = profile === \"nextjs\" && intent === \"create\";",
        "出発点",
        "編集",
        "検証",
        "pin",
        "data-testid=\"pack-wizard-nextjs-acme\"",
        "data-testid=\"pack-wizard-issues\"",
        "該当項目へ移動",
        "focusEditorField",
        "fetchExtensionPack",
        "stageExtensionPack",
        "verifyExtensionPack",
        "pinExtensionPack",
        "retireExtensionPack",
        "startNextVersion",
        "incrementPatchVersion",
        "immutableLifecycleFromConflict",
        "immutable = lifecycle === \"pinned\" || lifecycle === \"retired\"",
        "disabled={immutable}",
        "data-testid=\"pack-wizard-trial-link\"",
        "data-testid=\"pack-wizard-new-version\"",
        "useResource<TrialOptions>(\"trial-options\")",
        "profileOptions.data.profiles.map",
        "trial_token_auth_enabled !== false",
        "data-testid=\"pack-wizard-token-auth-disabled\"",
        "onCatalogChange?.()",
        "新しいバージョンを作る",
        "ローカル（未承認・帯域未計測）",
        "退役済み — 終端状態",
    ] {
        assert!(
            wizard.contains(required),
            "extension pack wizard is missing {required:?}"
        );
    }

    let api = std::fs::read_to_string("gui/lib/extension-api.ts").unwrap();
    for required in [
        "extensions/packs",
        "/verify",
        "/pin",
        "/retire",
        "encodeURIComponent(id)",
        "trialAuthorizationHeaders(token",
        "method: \"POST\"",
    ] {
        assert!(api.contains(required), "wizard API is missing {required:?}");
    }
    for forbidden in ["method: \"PUT\"", "method: \"PATCH\"", "method: \"DELETE\""] {
        assert!(
            !api.contains(forbidden),
            "wizard API exposes forbidden mutation {forbidden:?}"
        );
    }

    let resource = std::fs::read_to_string("gui/lib/use-resource.ts").unwrap();
    for required in [
        "refresh: () => void",
        "setRevision",
        "return { ...state, refresh }",
    ] {
        assert!(
            resource.contains(required),
            "resource hook is missing explicit refresh behavior {required:?}"
        );
    }

    let smoke = std::fs::read_to_string("gui/scripts/smoke.mjs").unwrap();
    for required in [
        "--wizard-only",
        "probePackWizard",
        "pack-wizard-issues",
        "pack-wizard-pinned",
        "pack-wizard-retired",
        "pinnedBytesMatchDisplay",
        "pinnedNextVersionStaged",
        "retiredNextDraftEditable",
        "selectedPack === selector",
        "profilesMatchTrial",
        "catalogRefreshedAfterPin",
        "probeAssetsAccessibility",
        "probePackWizardAuthOff",
        "warningAlerts === 0",
        "violation_count === 0",
    ] {
        assert!(
            smoke.contains(required),
            "wizard smoke is missing {required:?}"
        );
    }
}

#[test]
fn extension_profile_wizard_requires_root_preview_hash_and_restart_boundary() {
    let page = std::fs::read_to_string("gui/app/assets/page.tsx").unwrap();
    assert!(page.contains("<ProfileWizard enabled={extensionRootStatus === \"ready\"} />"));

    let wizard = std::fs::read_to_string("gui/components/profile-wizard.tsx").unwrap();
    for required in [
        "disabled={!enabled}",
        "compact manifest v2",
        "additive overlay v1",
        "previewExtensionProfile",
        "registerExtensionProfile",
        "expected_hash: preview.hash",
        "profile id",
        "normalized path",
        "exact hash",
        "draft / 未承認",
        "上限 {preview.assurance_ceiling}",
        "data-testid=\"profile-wizard-confirm\"",
        "data-restart-required={registration.restart_required}",
        "保存成功と runtime 反映は別です。",
        "restart_required:",
        "data-testid=\"profile-supply-row\"",
        "runtime 未反映",
    ] {
        assert!(
            wizard.contains(required),
            "profile wizard is missing {required:?}"
        );
    }
    for forbidden in [
        "admitted = true",
        "promote",
        "method: \"PUT\"",
        "method: \"DELETE\"",
    ] {
        assert!(
            !wizard.contains(forbidden),
            "profile wizard exposes forbidden capability {forbidden:?}"
        );
    }

    let api = std::fs::read_to_string("gui/lib/extension-api.ts").unwrap();
    for required in [
        "extensions/profiles",
        "extensions/profiles/preview",
        "extensions/profiles/register",
        "trialAuthorizationHeaders(token, true)",
        "restart_required: boolean",
    ] {
        assert!(
            api.contains(required),
            "profile API is missing {required:?}"
        );
    }

    let smoke = std::fs::read_to_string("gui/scripts/smoke.mjs").unwrap();
    for required in [
        "probeProfileWizard",
        "profile-wizard-preview",
        "profile-wizard-register",
        "profile-registration-result",
        "unavailable_before_restart",
        "同一内容",
    ] {
        assert!(
            smoke.contains(required),
            "profile wizard smoke is missing {required:?}"
        );
    }
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
fn trial_status_polling_revalidates_with_durable_timing_metadata() {
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
        "started_epoch_seconds:",
        "average_duration_seconds:",
        "gate:",
        "status:",
        "verdict:",
        "assurance:",
        "assurance_reason:",
        "stop_reason:",
        "failure_diagnostics?:",
        "next_action:",
        "phases:",
        "event_count:",
        "acceptance_sheet:",
        "section5:",
        "events_path:",
        "identity?:",
    ] {
        assert!(schema.contains(field), "PolledSession lost {field}");
    }
    assert_eq!(schema.lines().filter(|line| line.contains(':')).count(), 17);

    let identity = std::fs::read_to_string("gui/components/trial-run-identity.tsx").unwrap();
    for required in [
        "trial-run-identity-goal",
        "trial-run-identity-profile",
        "trial-run-identity-pack",
        "trial-run-identity-executor-model",
        "trial-run-identity-planner-model",
    ] {
        assert!(
            identity.contains(required),
            "Trial run identity summary lost {required:?}"
        );
    }

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
        "document.body.textContent?.includes(\"実行中\")",
    ] {
        assert!(
            smoke.contains(required),
            "polling smoke evidence is missing {required:?}"
        );
    }
    assert!(
        !smoke.contains("document.body.textContent?.includes(\"running\")"),
        "polling readiness must observe the localized visible status"
    );
}

#[test]
fn trial_feedback_restores_sessions_and_uses_an_honest_terminal_title() {
    let source = trial_ui_sources();
    for required in [
        "data-testid=\"elapsed-time\"",
        "window.setInterval(tick, 1_000)",
        "data-testid=\"mean-duration-comparison\"",
        "data-testid=\"phase-progress\"",
        "currentPhase.total > 0",
        "フェーズ {currentPhase.index} / {currentPhase.total}",
        "平均所要時間（予測ではありません）",
        "void reconnectExisting(id).then((restored)",
        "url.searchParams.delete(\"sample\")",
        "session.gate === \"gate_3\" ? \"✔\" : \"✗\"",
        "document.title = `${marker} ${heading} | CommandAgent`",
        "window.Notification.permission !== \"granted\"",
        "!document.hidden",
        "priorStage === \"gate_2\"",
        "clearSessionQuery()",
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
        "elapsed_preserved_after_reconnect",
        "mean_preserved_after_reconnect",
        "sample_consumed_before_reload",
        "reload_automatically_reconnected",
        "reload_only_gets",
        "zero_total_hidden",
        "monitor_and_progress_separate",
        "✗ 実行結果と次の一手を確認してください | CommandAgent",
        "!terminalTitle.includes(\"✔\")",
        "notification_matches",
        "acceptance_folded",
        "terminal_section_order",
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
        "data-stage={displayedStage}",
        "stage === \"compose\"",
        "if (proposal === null || stage !== \"gate_1\") return null",
        "if (stage !== \"gate_2\" || created === null) return null",
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

    let extensions = std::fs::read_to_string("src/bin/gui_server/extensions.rs").unwrap();
    assert_eq!(
        extensions
            .matches("require_trial(&state, &headers, false)")
            .count(),
        2,
        "both extension GET handlers must require Trial access"
    );
    assert_eq!(
        extensions
            .matches("require_trial(&state, &headers, true)")
            .count(),
        4,
        "every extension POST handler must require Trial access and Origin"
    );

    let profiles = std::fs::read_to_string("src/bin/gui_server/profile_extensions.rs").unwrap();
    assert_eq!(
        profiles
            .matches("require_access(&state, &headers, false)")
            .count(),
        1,
        "profile catalog must require extension access"
    );
    assert_eq!(
        profiles
            .matches("require_access(&state, &headers, true)")
            .count(),
        2,
        "profile preview and register must require Origin"
    );
    assert!(profiles.contains(".trial_access\n        .authorize(headers, require_origin)"));
}

#[test]
fn extension_supply_routes_are_post_only_for_mutation_and_delegate_writes_to_supply_root() {
    let entry = std::fs::read_to_string("src/bin/gui_server.rs").unwrap();
    for route in [
        "\"/api/extensions/packs\"",
        "\"/api/extensions/packs/{id}/{version}\"",
        "\"/api/extensions/packs/{id}/{version}/verify\"",
        "\"/api/extensions/packs/{id}/{version}/pin\"",
        "\"/api/extensions/packs/{id}/{version}/retire\"",
        "\"/api/extensions/profiles\"",
        "\"/api/extensions/profiles/preview\"",
        "\"/api/extensions/profiles/register\"",
    ] {
        assert!(entry.contains(route), "missing extension route {route}");
    }
    for forbidden in [
        "put(extensions::",
        "patch(extensions::",
        "delete(extensions::",
        "/delete",
        "/remove",
    ] {
        assert!(
            !entry.contains(forbidden),
            "extension router exposes forbidden mutation {forbidden}"
        );
    }

    let extensions = std::fs::read_to_string("src/bin/gui_server/extensions.rs").unwrap();
    for required in [
        "SupplyRoot::open(root)",
        "root.stage(",
        "root.verify_recorded(",
        "root.pin(",
        "root.retire(",
        "tokio::task::spawn_blocking",
        "MAX_BODY_BYTES: usize = 1024 * 1024",
    ] {
        assert!(
            extensions.contains(required),
            "extension handler bypasses required supply behavior {required:?}"
        );
    }

    let profiles = std::fs::read_to_string("src/bin/gui_server/profile_extensions.rs").unwrap();
    for required in [
        "ProfileSupplyRoot::open(root)",
        "root.preview(",
        "root.register(",
        "tokio::task::spawn_blocking",
        "MAX_PROFILE_BODY_BYTES",
        "profile_auth_failed",
        "profile_origin_not_allowed",
        "profile_body_too_large",
        "profile_validation_failed",
        "profile_conflict",
        "profile_io_failed",
    ] {
        assert!(
            profiles.contains(required),
            "profile handler bypasses required supply behavior {required:?}"
        );
    }
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

    let guide = std::fs::read_to_string("docs/user/gui-trial.md").unwrap();
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
        "runtime_paths::run_read_dirs(&workspace)",
        "let mut seen = HashSet::new()",
        "has_confirmation_record",
        ".lease_snapshot()",
        "started_epoch_seconds",
        "gate: Option<&'static str>",
        "profile: Option<String>",
        "intent: Option<String>",
        "record.identity().profile.clone()",
        "record.identity().intent.clone()",
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
    let sessions = std::fs::read_to_string("src/bin/gui_server/sessions.rs").unwrap();
    for required in [
        "pub(super) async fn started_epoch_seconds",
        "id.get_version_num() == 7",
        "metadata_created(events_path).await",
    ] {
        assert!(
            sessions.contains(required),
            "shared Trial session timing is missing {required:?}"
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
        "dateTimeLabel(session.started_epoch_seconds, \"反映待ち\")",
        "dateTimeLabel(session.modified_epoch_seconds, \"反映待ち\")",
        "trialGateLabel(session.gate)",
        "trialStatusLabel(session.status)",
        "href={sessionLink(session)}",
        "trialRoutePath(isTerminalSession(session) ? \"detail\" : \"status\", session.id)",
        "data-testid=\"session-route-link\"",
        "data-testid=\"session-profile\"",
        "data-testid=\"session-intent\"",
        "data-testid=\"session-pack\"",
        "session.pack.id}@${session.pack.version}",
        "data-testid=\"trial-session-auth-required\"",
        "data-testid=\"trial-session-freshness\"",
        "最後に取得できた一覧を表示しています。",
        "window.addEventListener(\"focus\", refresh)",
        "document.addEventListener(\"visibilitychange\", refreshWhenVisible)",
        "previous === \"running\"",
        "runtimeLease === \"idle\" || runtimeLease === \"recovery_required\"",
        "data-session-id={session.id}",
        "data-terminal={isTerminalSession(session)}",
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
    assert!(
        !panel.contains("<TrialFailureDiagnostics"),
        "compact Trial history must not expand diagnostics inline"
    );
    for required in [
        "<TrialSessionIndexPanel",
        "trialTokenAuthEnabled",
        "trialAccessReady",
        "data-testid=\"trial-token-auth-disabled\"",
        "onLeaseChange={setWorkspaceLease}",
        "data-testid=\"terminal-session-history-link\"",
        "trialRoutePath(\"history\")",
        "surface === \"compose\" && stage === \"compose\"",
        "surface === \"status\" && <TrialGateTwo",
        "surface === \"history\"",
        "surface === \"detail\"",
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
        "proposalCount += 1",
        "proposal_count: proposalCount",
        "check_contract_disabled: checkContractDisabled",
        "lease_inline_notice: leaseInlineNotice",
        "leaseInlineNotice.includes(sessionId)",
        "leaseInlineNotice.includes(\"新しい起動はできません\")",
        "proposalCount === 0",
        "dispatchCount === 0",
    ] {
        assert!(
            smoke.contains(required),
            "Trial session index smoke is missing {required:?}"
        );
    }
    assert!(
        !smoke.contains("launch_disabled: launchDisabled"),
        "running leases must block contract checking before proposal creation"
    );
    for required in [
        "buildBasePath: \"/\"",
        "buildBasePath: \"/proxy/commandagent/\"",
        "no_periodic_index_polling",
        "running history row state",
        "status_navigated_to_detail",
        "refresh failure removed the last successful row",
        "focus refresh",
        "visible-tab refresh",
        "reconnect_get_only",
        "automatic_reconnect_restored_result",
        "runtime_max_concurrent_requests",
        "runtime_paused_while_hidden",
        "runtime_resumed_when_visible",
        "runtime_live_region",
        "terminal_row_targeted",
        "terminal_row_compact",
        "GATE 2（実行） / 実行中",
        "time_labels_use_shared_ja_jp_format",
        "runtime_badge_navigated",
        "runtime_badge_reconnected",
        "route_ownership",
        "legacy_running_to_status",
        "legacy_terminal_to_detail",
        "mobile_fits",
        "resource_revalidation",
        "failure_retained_previous_data",
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
    assert!(shell.contains("label: \"リポジトリ実行記録\""));
    assert!(shell.contains("data-testid=\"runtime-session-link\""));
    assert!(shell.contains("trialRoutePath(\"status\", runtimeSession.id)"));
    let runs = std::fs::read_to_string("gui/app/runs/page.tsx").unwrap();
    let dashboard = std::fs::read_to_string("gui/app/page.tsx").unwrap();
    for required in [
        "リポジトリ / workspace/management/runs",
        "GUI トライアルの実行ルートではなく",
    ] {
        assert!(
            runs.contains(required),
            "repository report source UI is missing {required:?}"
        );
    }
    assert!(dashboard.contains("参照元: workspace/management/runs"));
    assert!(panel.contains("実行ルート / .commandagent/runs"));
}

#[test]
fn gui_visibility_revalidation_and_shared_time_format_are_pinned() {
    let runtime = std::fs::read_to_string("gui/lib/use-runtime-status.ts").unwrap();
    for required in [
        "const REFRESH_INTERVAL_MS = 750",
        "let requestInFlight = false",
        "document.visibilityState === \"hidden\"",
        "document.addEventListener(\"visibilitychange\", refreshWhenVisible)",
        "document.removeEventListener(\"visibilitychange\", refreshWhenVisible)",
        "controller?.abort()",
        "signal: controller.signal",
    ] {
        assert!(
            runtime.contains(required),
            "runtime status visibility contract is missing {required:?}"
        );
    }

    let gui = gui_source_files(Path::new("gui"))
        .iter()
        .map(|path| std::fs::read_to_string(path).unwrap())
        .collect::<String>();
    assert_eq!(
        gui.matches("useRuntimeStatus();").count(),
        1,
        "Shell must remain the sole runtime-status poller"
    );

    let resource = std::fs::read_to_string("gui/lib/use-resource.ts").unwrap();
    for required in [
        "window.addEventListener(\"focus\", refresh)",
        "document.addEventListener(\"visibilitychange\", refreshWhenVisible)",
        "data: current.data",
        "cache: \"no-store\"",
    ] {
        assert!(
            resource.contains(required),
            "resource revalidation contract is missing {required:?}"
        );
    }

    let measurements = std::fs::read_to_string("gui/app/measurements/page.tsx").unwrap();
    assert!(measurements.contains("report.path === selectedPath"));
    assert!(measurements.contains("[reports.data, selectedPath]"));
    let smoke = std::fs::read_to_string("gui/scripts/smoke.mjs").unwrap();
    for required in [
        "client_navigation_preserved_document",
        "aria_current_page",
        "setDocumentVisibility(page, \"hidden\")",
        "setDocumentVisibility(page, \"visible\")",
        "selection_retained_after_visibility",
    ] {
        assert!(
            smoke.contains(required),
            "measurement visibility smoke is missing {required:?}"
        );
    }

    let lifecycle_smoke = std::fs::read_to_string("gui/scripts/session-index-smoke.mjs").unwrap();
    for required in [
        "terminal_refresh_elapsed_ms",
        "terminal_runtime_refreshed_within_one_second",
    ] {
        assert!(
            lifecycle_smoke.contains(required),
            "runtime terminal-refresh smoke is missing {required:?}"
        );
    }

    let format = std::fs::read_to_string("gui/lib/format.ts").unwrap();
    assert_eq!(format.matches("Intl.DateTimeFormat").count(), 1);
    assert!(format.contains("export function dateTimeLabel"));
    for path in [
        "gui/app/page.tsx",
        "gui/app/runs/page.tsx",
        "gui/components/trial-gate-two.tsx",
        "gui/components/trial-session-index.tsx",
    ] {
        assert!(
            std::fs::read_to_string(path)
                .unwrap()
                .contains("dateTimeLabel("),
            "{path} does not use the shared GUI date-time formatter"
        );
    }

    let base_path = std::fs::read_to_string("gui/lib/base-path.ts").unwrap();
    for route in [
        "\"/try/\"",
        "\"/try/status/\"",
        "\"/try/history/\"",
        "\"/try/history/detail/\"",
    ] {
        assert!(
            base_path.contains(route),
            "Trial route helper is missing {route}"
        );
    }
    assert!(base_path.contains("?session=${encodeURIComponent(sessionId)}"));
    let styles = std::fs::read_to_string("gui/app/globals.css").unwrap();
    assert!(styles.contains(".session-list li:target"));
    assert!(styles.contains("@keyframes session-row-highlight"));

    let shell = std::fs::read_to_string("gui/components/shell.tsx").unwrap();
    let runs = std::fs::read_to_string("gui/app/runs/page.tsx").unwrap();
    let history = std::fs::read_to_string("docs/user/gui-history.md").unwrap();
    for source in [&shell, &runs, &history] {
        assert!(source.contains("リポジトリ実行記録"));
        assert!(!source.contains("検証・運用レポート"));
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

fn trial_ui_sources() -> String {
    [
        "gui/app/try/page.tsx",
        "gui/app/try/status/page.tsx",
        "gui/app/try/history/page.tsx",
        "gui/app/try/history/detail/page.tsx",
        "gui/components/trial-access-panel.tsx",
        "gui/components/trial-compose.tsx",
        "gui/components/trial-gate-one.tsx",
        "gui/components/trial-gate-two.tsx",
        "gui/components/trial-page-nav.tsx",
        "gui/components/trial-run.tsx",
        "gui/components/trial-terminal.tsx",
        "gui/hooks/use-trial-compose.ts",
        "gui/hooks/use-trial-monitor.ts",
        "gui/hooks/use-trial-page-routing.ts",
        "gui/hooks/use-trial-run.ts",
        "gui/hooks/use-trial-terminal.ts",
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
