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
fn gui_fetch_failures_use_one_actionable_error_descriptor() {
    let descriptor = std::fs::read_to_string("gui/lib/errors.ts").unwrap();
    for required in [
        "export function describeError",
        "export async function responseError",
        "trial_token_invalid",
        "GUI_TRIAL_ALLOWED_ORIGINS",
        "--commandagent-bin",
        "trial_workspace_conflict",
        "reconnectSessionId",
    ] {
        assert!(
            descriptor.contains(required),
            "shared GUI error descriptor is missing {required:?}"
        );
    }

    for path in [
        "gui/lib/use-resource.ts",
        "gui/app/try/page.tsx",
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
fn trial_ui_keeps_gate_one_confirmation_and_has_no_intervention_surface() {
    let source = std::fs::read_to_string("gui/app/try/page.tsx").unwrap();
    for required in [
        "if (!confirmed || proposal === null)",
        "\"x-commandagent-trial-authorization\": `Bearer ${token.trim()}`",
        "confirmation_hash: proposal.card_hash",
        "data-testid=\"trial-workspace\"",
        "proposal.identity.workspace",
        "may create, modify, or delete content inside this directory",
        "<option value=\"lm-studio\">LM Studio</option>",
        "window.matchMedia(\"(max-width: 720px)\")",
        "target.scrollIntoView({ behavior: \"smooth\", block: \"start\" })",
        "Enter the runtime Trial access token before checking the contract.",
        "disabled={!confirmed || busy || stage === \"gate_2\"}",
        "Confirm and delegate to CLI",
        "Confirm D-3d continuation",
        "End without another run",
    ] {
        assert!(
            source.contains(required),
            "trial UI is missing {required:?}"
        );
    }
    assert!(
        !source.contains("disabled={trialToken === \"\""),
        "Trial token guidance must not be hidden behind a disabled button"
    );
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
        5,
        "every Trial API handler must enforce workspace and access guards"
    );
    for required in [
        "StatusCode::SERVICE_UNAVAILABLE",
        "StatusCode::UNAUTHORIZED",
        "StatusCode::FORBIDDEN",
        "complete_from_events",
    ] {
        assert!(
            delegate.contains(required),
            "missing Trial guard {required:?}"
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
        || (path != Path::new(DELEGATE_MODULE)
            && ["std::process", "Command::new", ".spawn("]
                .iter()
                .any(|token| source.contains(token)))
}
