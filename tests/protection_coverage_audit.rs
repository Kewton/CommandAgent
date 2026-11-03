use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy)]
struct ProtectionRule {
    category: &'static str,
    site_predicate: &'static str,
    required_wrapper: &'static str,
    allowlist: &'static [&'static str],
    audit: fn(&AuditCorpus, &ProtectionRule) -> Vec<String>,
}

struct AuditCorpus {
    files: BTreeMap<String, String>,
}

const PROTECTION_RULES: &[ProtectionRule] = &[
    ProtectionRule {
        category: "compile_output_source_of_truth",
        site_predicate: "compile diagnostic parsers",
        required_wrapper: "FullCommandOutput",
        allowlist: &[
            "src/minimal_loop/build_verifier.rs",
            "src/planner/profile.rs",
            "src/planner/profiles/manifest_driven.rs",
            "src/planner/profiles/python_cli.rs",
            "src/planner/fix_diagnostics.rs",
            "src/planner/runner/acceptance.rs",
        ],
        audit: audit_compile_output_source_of_truth,
    },
    ProtectionRule {
        category: "verify_normalization_boundary",
        site_predicate: "executes-or-policy-checks-model-commands",
        required_wrapper: "NormalizedVerifyCommand",
        allowlist: &[],
        audit: audit_verify_normalization_boundary,
    },
    ProtectionRule {
        category: "bounded_execution_chokepoints",
        site_predicate: "provider chat and child process launch",
        required_wrapper: "provider_call / bounded_process / confirmed GUI CLI delegate",
        allowlist: &[
            "src/provider_call.rs",
            "src/bounded_process.rs",
            "src/bin/gui_server/delegate.rs",
            "src/planner/artifact_contract_checks/process.rs",
            "src/tools/profile_tools/process.rs",
        ],
        audit: audit_bounded_execution_chokepoints,
    },
    ProtectionRule {
        category: "profile_tool_effect_boundary",
        site_predicate: "propose-artifacts process launch, result acceptance, and host commit",
        required_wrapper: "env_clear / deny-default isolation / bounded process / bound result / fd commit with receipt",
        allowlist: &[
            "src/tools/profile_tools/isolation.rs",
            "src/tools/profile_tools/process.rs",
            "src/tools/profile_tools/protocol.rs",
            "src/tools/profile_tools/commit.rs",
            "src/tools/profile_tools/effects.rs",
            "src/tools/registry.rs",
        ],
        audit: audit_profile_tool_effect_boundary,
    },
    ProtectionRule {
        category: "startup_credential_boundary",
        site_predicate: "provider credentials in a process's exec-time environment",
        required_wrapper: "first-statement self exec / read-only store / no way back to an environment",
        allowlist: &[
            "src/main.rs",
            "src/bin/gui_server.rs",
            "src/startup_credentials.rs",
        ],
        audit: audit_startup_credential_boundary,
    },
    ProtectionRule {
        category: "declarative_command_check_boundary",
        site_predicate: "draft-profile and local-pack command checks",
        required_wrapper: "verify policy / normalized env / bounded process",
        allowlist: &["src/planner/declarative_command_checks.rs"],
        audit: audit_declarative_command_check_boundary,
    },
    ProtectionRule {
        category: "artifact_contract_process_boundary",
        site_predicate: "operator-registered artifact contract adapter launch and acceptance",
        required_wrapper: "env_clear / pinned realpath executable / process group / bounded streams / partial cap",
        allowlist: &[
            "src/planner/artifact_contract_checks/process.rs",
            "src/planner/artifact_contract_checks/mod.rs",
            "src/planner/artifact_contract_checks/protocol.rs",
        ],
        audit: audit_artifact_contract_process_boundary,
    },
    ProtectionRule {
        category: "fetch_network_boundary",
        site_predicate: "network fetch transport and dispatch",
        required_wrapper: "fetch_probe / bounded_process",
        allowlist: &["src/fetch_probe/mod.rs", "src/fetch_probe/transport.rs"],
        audit: audit_fetch_network_boundary,
    },
    ProtectionRule {
        category: "provider_timeout_enforcement",
        site_predicate: "ChatClient implementors",
        required_wrapper: "required boxed_clone worker path",
        allowlist: &["src/providers/mod.rs", "src/provider_call.rs"],
        audit: audit_provider_timeout_enforcement,
    },
    ProtectionRule {
        category: "workspace_policy_tool_paths",
        site_predicate: "built-in and extension tool path arguments",
        required_wrapper: "resolve_policy_checked_path / normalize_existing_workspace_path / ensure_tool_path_allowed",
        allowlist: &["src/tools/registry.rs", "src/tools/extension.rs"],
        audit: audit_workspace_policy_tool_paths,
    },
    ProtectionRule {
        category: "terminal_records",
        site_predicate: "CLI command exit paths",
        required_wrapper: "DirectCommandCompletionGuard / tui_command_stop",
        allowlist: &["src/lib.rs", "tests/conformance/mod.rs"],
        audit: audit_terminal_records,
    },
    ProtectionRule {
        category: "extension_supply_writes",
        site_predicate: "writes below an extension_root",
        required_wrapper: "planner::pack::supply::SupplyRoot / profile_manifest::supply::ProfileSupplyRoot",
        allowlist: &[
            "src/planner/pack/supply.rs",
            "src/planner/profile_manifest/supply.rs",
        ],
        audit: audit_extension_supply_writes,
    },
];

#[test]
fn protection_coverage_table_is_green_for_current_tree() {
    let corpus = AuditCorpus::from_manifest_dir();
    let violations = audit_protection_coverage(&corpus);
    assert!(
        violations.is_empty(),
        "protection coverage audit failed:\n{}",
        violations.join("\n")
    );
}

#[test]
fn protection_coverage_table_rejects_unregistered_mock_sites() {
    let corpus = AuditCorpus::from_pairs([
        (
            "src/new_provider.rs",
            r#"fn f(client: &mut dyn Client) { let _ = client.chat("m", &[], &[], false); }"#,
        ),
        (
            "src/providers/mod.rs",
            r#"pub trait ChatClient { fn boxed_clone(&self) -> Option<Box<dyn ChatClient>> { None } }"#,
        ),
        (
            "src/provider_call.rs",
            r#"fn f(client: &mut dyn ChatClient) { let Some(mut worker_client) = client.boxed_clone() else { return client.chat("m", &[], &[], false); }; }"#,
        ),
        (
            "src/new_spawn.rs",
            r#"fn f() { let _ = std::process::Command::new("sh").output(); }"#,
        ),
        (
            "src/bin/gui_server/sessions.rs",
            r#"fn f() { let _ = std::process::Command::new("commandagent").spawn(); }"#,
        ),
        (
            "src/new_fetch.rs",
            r#"fn f() { let _ = std::process::Command::new("curl").arg("https://example.test"); let _ = transport.get(root, request); }"#,
        ),
        (
            "src/new_verify.rs",
            r#"fn f(root: &std::path::Path) { let _ = crate::minimal_loop::verifier_env::run_checked("npm run build", root, false); }"#,
        ),
        (
            "src/new_runtime_bash.rs",
            r#"fn runtime_bash_policy_decision(command: &str) { let _ = crate::planner::verify::diagnose_verify_command(command); }"#,
        ),
        (
            "src/planner/lint.rs",
            r#"fn lint(command: &str) { let _ = crate::planner::verify::validate_verify_command(command); }"#,
        ),
        (
            "src/new_compile.rs",
            r#"fn f(excerpt: &str) { let _ = crate::minimal_loop::build_verifier::parse_compile_errors(excerpt); }"#,
        ),
        (
            "src/tools/registry.rs",
            r#"fn default_tool_specs() { spec("Read", "", json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"]})); spec("Write", "", json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"]})); } fn execute() { let _ = resolve_existing(&context.root, raw); }"#,
        ),
        (
            "src/config.rs",
            "pub enum Action { Repl, Prompt(String), NewEntry(String) }",
        ),
        (
            "src/lib.rs",
            r#"fn run() {} fn direct_command_for_action(action: &Action) -> Option<&'static str> { match action { Action::Repl => None, Action::Prompt(_) => Some("--prompt") } }"#,
        ),
        (
            "src/new_extension_writer.rs",
            r#"fn f(extension_root: &Path) { std::fs::write(extension_root.join("packs/x"), b"x").unwrap(); }"#,
        ),
    ]);
    let violations = audit_protection_coverage(&corpus).join("\n");
    for category in [
        "compile_output_source_of_truth",
        "verify_normalization_boundary",
        "bounded_execution_chokepoints",
        "declarative_command_check_boundary",
        "artifact_contract_process_boundary",
        "fetch_network_boundary",
        "provider_timeout_enforcement",
        "workspace_policy_tool_paths",
        "terminal_records",
        "extension_supply_writes",
    ] {
        assert!(
            violations.contains(category),
            "mock site did not trip {category}; violations:\n{violations}"
        );
    }
    assert!(
        violations.contains("src/new_fetch.rs")
            && violations.contains("constructs a fetch child outside the boundary"),
        "mock fetch site did not trip the single-boundary guard: {violations}"
    );
    assert!(
        violations.contains("src/bin/gui_server/sessions.rs")
            && violations.contains("direct child-process invocation"),
        "old GUI process location did not trip the moved delegate guard: {violations}"
    );
}

/// A credential read past the store fails the rule, wherever and however it is written.
#[test]
fn a_direct_credential_read_fails_the_startup_rule() {
    let real = AuditCorpus::from_manifest_dir();
    let rule = PROTECTION_RULES
        .iter()
        .find(|rule| rule.category == "startup_credential_boundary")
        .unwrap();
    let with_file = |path: &str, text: &str| {
        let mut changed = AuditCorpus {
            files: real.files.clone(),
        };
        changed.files.insert(path.to_string(), text.to_string());
        (rule.audit)(&changed, rule)
    };
    assert!((rule.audit)(&real, rule).is_empty(), "the tree");
    let direct = |violations: &[String]| {
        violations
            .iter()
            .any(|violation| violation.contains("from the environment"))
    };
    for (label, code) in [
        (
            "by name",
            r#"fn f() { let _ = std::env::var("OPENAI_API_KEY"); }"#,
        ),
        (
            "by name, for an OsString",
            r#"fn f() { let _ = std::env::var_os("LM_STUDIO_API_TOKEN"); }"#,
        ),
        (
            "over two lines",
            "fn f() {\n    let _ = std::env::var(\n        \"GEMINI_API_KEY\",\n    );\n}\n",
        ),
        (
            "by a constant",
            "fn f() { let _ = std::env::var(JEV_API_KEY_ENV); }",
        ),
        (
            "by a constant with its path",
            "fn f() { let _ = std::env::var_os(crate::providers::lm_studio::LM_STUDIO_API_TOKEN_ENV); }",
        ),
        (
            "by a constant this file declares",
            "const MY_KEY_NAME: &str = \"OPENAI_API_KEY\";\nfn f() { let _ = std::env::var(MY_KEY_NAME); }",
        ),
        (
            "through env_compat",
            r#"fn f() { let _ = crate::env_compat::var("GEMINI_API_KEY"); }"#,
        ),
        (
            "through env_compat, for an OsString",
            r#"fn f() { let _ = env_compat::var_os("OPENAI_API_KEY"); }"#,
        ),
        (
            "the configured key name",
            "fn f(api_key_env: &str) { let _ = std::env::var(api_key_env); }",
        ),
        (
            "the configured key name in a closure",
            "fn f(c: &C) { let _ = c.api_key_env.as_deref().and_then(|name| std::env::var(name).ok()); }",
        ),
        (
            "the configured key name as a path",
            "fn f(c: &C) { let _ = c.api_key_env.as_deref().map(std::env::var_os); }",
        ),
        (
            "the configured key name through env_compat",
            "fn f(api_key_env: &str) { let _ = crate::env_compat::var(api_key_env); }",
        ),
        (
            "a loop over names",
            r#"fn f() { for name in ["OPENAI_API_KEY"] { let _ = std::env::var(name); } }"#,
        ),
        (
            "beside an unrelated test item",
            "#[cfg(test)]\nmod t {}\nfn f() { let _ = std::env::var(\"OPENAI_API_KEY\"); }",
        ),
    ] {
        assert!(direct(&with_file("src/elsewhere.rs", code)), "{label}");
    }
    for (label, path, code) in [
        (
            "in the store's own file",
            "src/startup_credentials.rs",
            "fn f() { let _ = std::env::var(\"OPENAI_API_KEY\"); }",
        ),
        (
            "another variable",
            "src/elsewhere.rs",
            r#"fn f() { let _ = std::env::var("PATH"); }"#,
        ),
        (
            "a name with no read",
            "src/elsewhere.rs",
            r#"const HINT: &str = "set OPENAI_API_KEY";"#,
        ),
        (
            "a test item",
            "src/elsewhere.rs",
            "#[cfg(test)]\nfn f() { let _ = std::env::var(\"OPENAI_API_KEY\"); }\n",
        ),
        (
            "a test item for one platform",
            "src/elsewhere.rs",
            "#[cfg(all(test, unix))]\nmod t {\n    fn f() { let _ = std::env::var(\"OPENAI_API_KEY\"); }\n}\n",
        ),
        (
            "a brace in a test item's string",
            "src/elsewhere.rs",
            "#[cfg(test)]\nmod t {\n    const A: &str = \"}\";\n    const C: char = '}';\n    const R: &str = r#\"}\"#;\n    fn f() { let _ = std::env::var(\"OPENAI_API_KEY\"); }\n}\n",
        ),
        (
            "a comment",
            "src/elsewhere.rs",
            "// std::env::var(\"OPENAI_API_KEY\")\nfn f() {}\n",
        ),
    ] {
        let violations = with_file(path, code);
        assert!(!direct(&violations), "{label}: {violations:?}");
    }
    // A file only a test build compiles, and one it brings in.
    let mut changed = AuditCorpus {
        files: real.files.clone(),
    };
    for (path, text) in [
        ("src/holder.rs", "#[cfg(test)]\nmod holder_tests;\n"),
        (
            "src/holder_tests.rs",
            "mod inner;\nfn f() { let _ = std::env::var(\"OPENAI_API_KEY\"); }\n",
        ),
        (
            "src/holder_tests/inner.rs",
            "fn g() { let _ = std::env::var(\"OPENAI_API_KEY\"); }\n",
        ),
    ] {
        changed.files.insert(path.to_string(), text.to_string());
    }
    assert!(!direct(&(rule.audit)(&changed, rule)), "a test-only file");
    // The same file, brought in by production, counts.
    changed.files.insert(
        "src/holder.rs".to_string(),
        "mod holder_tests;\n".to_string(),
    );
    assert!(direct(&(rule.audit)(&changed, rule)), "a production file");
    // A file under `tests/` is not production.
    assert!(!direct(&with_file(
        "tests/elsewhere.rs",
        r#"fn f() { let _ = std::env::var("OPENAI_API_KEY"); }"#,
    )));

    let discovery = "src/bin/gui_server/trial_options.rs";
    let text = real.file(discovery);
    let read = "commandagent::startup_credentials::var_os(\"LM_STUDIO_API_TOKEN\")";
    assert!(text.contains(read));
    for (label, to) in [
        (
            "its original form",
            "std::env::var_os(\"LM_STUDIO_API_TOKEN\")".to_string(),
        ),
        (
            "another key",
            "commandagent::startup_credentials::var_os(\"OPENAI_API_KEY\")".to_string(),
        ),
        (
            "a read that reaches a child",
            format!(
                "{read}.inspect(|value| {{ std::process::Command::new(\"x\").env(\"X\", value); }})"
            ),
        ),
    ] {
        let violations = with_file(discovery, &text.replacen(read, &to, 1));
        assert!(!violations.is_empty(), "{label}");
    }
}

/// Each way back to an environment, or a start that is not first, fails the rule.
#[test]
fn startup_credential_boundary_rejects_each_weakened_variant() {
    let real = AuditCorpus::from_manifest_dir();
    let rule = PROTECTION_RULES
        .iter()
        .find(|rule| rule.category == "startup_credential_boundary")
        .unwrap();
    assert!(
        (rule.audit)(&real, rule).is_empty(),
        "{:?}",
        (rule.audit)(&real, rule)
    );
    for (label, path, from, to) in [
        (
            "the CLI does something first",
            "src/main.rs",
            "fn main() {\n    commandagent::startup_credentials::bootstrap_cli();",
            "fn main() {\n    let _early = std::env::args();\n    commandagent::startup_credentials::bootstrap_cli();",
        ),
        (
            "the CLI skips the start",
            "src/main.rs",
            "    commandagent::startup_credentials::bootstrap_cli();\n",
            "",
        ),
        (
            "the GUI skips the start",
            "src/bin/gui_server.rs",
            "    commandagent::startup_credentials::bootstrap_gui();\n",
            "",
        ),
        (
            "the GUI starts its runtime first",
            "src/bin/gui_server.rs",
            "fn main() -> anyhow::Result<()> {\n    commandagent::startup_credentials::bootstrap_gui();",
            "#[tokio::main]\nasync fn main() -> anyhow::Result<()> {\n    commandagent::startup_credentials::bootstrap_gui();",
        ),
        (
            "a value goes back to the environment",
            "src/startup_credentials.rs",
            "if super::HELD.set(Held(pairs)).is_err() {",
            "unsafe { std::env::set_var(\"OPENAI_API_KEY\", \"x\") };\n        if super::HELD.set(Held(pairs)).is_err() {",
        ),
        (
            "the store is public",
            "src/startup_credentials.rs",
            "static HELD: OnceLock<Held>",
            "pub static HELD: OnceLock<Held>",
        ),
        (
            "the store's Debug shows it",
            "src/startup_credentials.rs",
            ".field(\"count\", &self.0.len())",
            ".field(\"values\", &self.0)",
        ),
        (
            "the exec keeps the environment",
            "src/startup_credentials.rs",
            "command.args(arguments).env_clear();",
            "command.args(arguments);",
        ),
        (
            "a failed exec goes on",
            "src/startup_credentials.rs",
            "stop(\"this executable could not be started again\")",
            "unreachable!()",
        ),
        (
            "a marker alone is believed",
            "src/startup_credentials.rs",
            "if names.iter().any(|name| present(name).is_some()) {",
            "if false {",
        ),
        (
            "another function of the delegate reads a received value",
            "src/bin/gui_server/delegate.rs",
            "fn apply_confirmed_pack(",
            "fn elsewhere(command: &mut Command) {\n    command.env(\"X\", commandagent::startup_credentials::var_os(\"OPENAI_API_KEY\").unwrap());\n}\n\nfn apply_confirmed_pack(",
        ),
        (
            "another function of the delegate reads it apart from the env call",
            "src/bin/gui_server/delegate.rs",
            "pub(super) fn check_binary(path: &Path) -> anyhow::Result<String> {",
            "pub(super) fn check_binary(path: &Path) -> anyhow::Result<String> {\n    let _early = commandagent::startup_credentials::var_os(\"OPENAI_API_KEY\");",
        ),
        (
            "another file reads a received value",
            "src/bounded_process.rs",
            "fn child_env_allowed(",
            "fn elsewhere() -> Option<std::ffi::OsString> {\n    crate::startup_credentials::var_os(\"OPENAI_API_KEY\")\n}\n\nfn child_env_allowed(",
        ),
        (
            "the delegate's restore reads one more name",
            "src/bin/gui_server/delegate.rs",
            "    for name in DELEGATE_PARENT_ENV_ALLOWLIST {\n",
            "    let _ = commandagent::startup_credentials::var(\"JEV_API_KEY\");\n    for name in DELEGATE_PARENT_ENV_ALLOWLIST {\n",
        ),
        (
            "the delegate's allowed names grow",
            "src/bin/gui_server/delegate.rs",
            "    \"LM_STUDIO_API_TOKEN\",\n];",
            "    \"LM_STUDIO_API_TOKEN\",\n    \"JEV_API_KEY\",\n];",
        ),
        (
            "the delegate's allowed names lose a key",
            "src/bin/gui_server/delegate.rs",
            "    \"GEMINI_API_KEY\",\n",
            "",
        ),
        (
            "a reader copies a value to a child",
            "src/providers/lm_studio.rs",
            "crate::startup_credentials::var(LM_STUDIO_API_TOKEN_ENV)",
            "command.env(\"X\", crate::startup_credentials::var(LM_STUDIO_API_TOKEN_ENV))",
        ),
        (
            "a version check gets a credential",
            "src/bin/gui_server/delegate.rs",
            "        command.env_remove(name);\n",
            "",
        ),
    ] {
        let text = real.file(path);
        assert!(text.contains(from), "{label}: the variant does not apply");
        let mut changed = AuditCorpus {
            files: real.files.clone(),
        };
        changed
            .files
            .insert(path.to_string(), text.replacen(from, to, 1));
        assert!(!(rule.audit)(&changed, rule).is_empty(), "{label}");
    }
}

/// Each bypass of the G3 boundary fails the rule, one file changed at a time.
#[test]
fn profile_tool_effect_boundary_rejects_each_weakened_variant() {
    let real = AuditCorpus::from_manifest_dir();
    let rule = PROTECTION_RULES
        .iter()
        .find(|rule| rule.category == "profile_tool_effect_boundary")
        .unwrap();
    assert!(
        (rule.audit)(&real, rule).is_empty(),
        "{:?}",
        (rule.audit)(&real, rule)
    );
    for (label, index, from, to) in [
        (
            "inherits the environment",
            0,
            ".env_clear()",
            ".env_remove(\"PATH\")",
        ),
        ("drops the process group", 0, ".process_group(0)", ".uid(0)"),
        ("starts open", 0, "(deny default)", "(allow default)"),
        (
            "opens the network",
            0,
            "(deny network*)",
            "(allow network*)",
        ),
        (
            "opens every write",
            0,
            r#"(allow file-write-data (literal "/dev/null"))"#,
            "(allow file-write*)",
        ),
        (
            "opens fork",
            0,
            "(allow sysctl-read)",
            "(allow sysctl-read)\n(allow process-fork)",
        ),
        (
            "borrows the old wrapper",
            0,
            "use std::time::Duration;",
            "use std::time::Duration;\nuse crate::tools::os_sandbox::wrap_command;",
        ),
        (
            "starts on any OS",
            0,
            "return Err(no_backend());",
            "return Ok(());",
        ),
        ("leaves descendants", 1, "tracker.stop_all()", "(0, 0)"),
        (
            "ignores a cancel",
            1,
            "stop = Some(Ended::Cancelled)",
            "let _ = Ended::Cancelled",
        ),
        (
            "ignores the deadline",
            1,
            "stop = Some(Ended::TimedOut)",
            "let _ = Ended::TimedOut",
        ),
        (
            "reads an error as an end",
            1,
            "budget.read_failed.store(true, Ordering::SeqCst);\n                    return kept;",
            "return kept;",
        ),
        (
            "widens the result",
            2,
            "MAX_RESULT_BYTES: usize = 64 * 1024;",
            "MAX_RESULT_BYTES: usize = 128 * 1024;",
        ),
        (
            "widens the clock",
            2,
            "MAX_TIMEOUT_MS: u64 = 30_000;",
            "MAX_TIMEOUT_MS: u64 = 60_000;",
        ),
        (
            "starts its own process in the runner",
            1,
            "let mut child = launch.into_command().spawn()?;",
            "let mut child = std::process::Command::new(\"/bin/sh\").spawn()?;",
        ),
        (
            "takes any command",
            1,
            "    launch: StrictCommand,",
            "    launch: std::process::Command,",
        ),
        (
            "keeps inherited descriptors",
            0,
            "command.pre_exec(move || mark_inherited_descriptors_close_on_exec(calls));",
            "",
        ),
        (
            "execs whatever the marking said",
            0,
            "command.pre_exec(move || mark_inherited_descriptors_close_on_exec(calls));",
            "command.pre_exec(move || { let _ = mark_inherited_descriptors_close_on_exec(calls); Ok(()) });",
        ),
        (
            "ignores a mark that failed",
            0,
            "if info.proc_fd > 2 && !(calls.mark)(info.proc_fd) {",
            "if info.proc_fd > 2 && false {",
        ),
        (
            "goes on past a full list",
            0,
            "if count == 0 || count >= CAPACITY {",
            "if false {",
        ),
        (
            "goes on past a table it may not cover",
            0,
            "if table == 0 || table > CAPACITY * entry {",
            "if false {",
        ),
        (
            "takes a limit for the highest descriptor",
            0,
            "let count = usize::try_from((calls.list)(&mut listed)).unwrap_or(0) / entry;",
            "let count = unsafe { libc::getdtablesize() } as usize;",
        ),
        (
            "keeps the temporary",
            3,
            "let _ = self.parent.unlink(&self.name);",
            "",
        ),
        (
            "claims durability",
            3,
            "let synced = parent.sync_directory().is_ok();",
            "let synced = true;",
        ),
        (
            "forgets a late cancel",
            3,
            "cancelled_after_commit: is_cancelled(),",
            "cancelled_after_commit: false,",
        ),
        (
            "drops a cancel check",
            3,
            "if is_cancelled() {",
            "if false {",
        ),
        ("drops a final check", 3, "final_check()?;", ""),
        (
            "drops an identity check",
            3,
            "if !entry_holds_candidate(",
            "if false && !entry_holds_candidate(",
        ),
        ("skips the fsync", 3, "file.sync_all()", "file.flush()"),
        (
            "writes in place",
            3,
            ".rename_to_leaf(&temporary.name)",
            ".write_leaf(\"\")",
        ),
        (
            "allows a hard link",
            3,
            "metadata.nlink() != 1",
            "metadata.nlink() == 0",
        ),
        (
            "trusts the start's answer before the process",
            4,
            "self.judge_now(context, STAGE_PROCESS)?;",
            "",
        ),
        (
            "trusts the start's answer before the commit",
            4,
            "&|| self.judge_now(context, STAGE_COMMIT),",
            "&|| Ok(()),",
        ),
        (
            "skips the pins before the commit",
            4,
            "self.verify_pins(STAGE_COMMIT)?;",
            "",
        ),
        ("authorizes as a read", 4, "\"Write\",", "\"Read\","),
        (
            "drops the force cancel",
            5,
            "is_cancelled() || is_force_cancelled()",
            "is_cancelled()",
        ),
    ] {
        let mut mutated = AuditCorpus {
            files: real.files.clone(),
        };
        let path = rule.allowlist[index];
        let text = real.file(path);
        assert!(text.contains(from), "{label}: `{from}` is not in {path}");
        mutated
            .files
            .insert(path.to_string(), text.replacen(from, to, 1));
        assert!(
            !(rule.audit)(&mutated, rule).is_empty(),
            "the rule did not see: {label}"
        );
    }
}

#[test]
fn artifact_contract_process_boundary_rejects_inheriting_or_uncapped_variants() {
    let real = AuditCorpus::from_manifest_dir();
    let rule = PROTECTION_RULES
        .iter()
        .find(|rule| rule.category == "artifact_contract_process_boundary")
        .unwrap();
    assert!((rule.audit)(&real, rule).is_empty());
    let [process, host, protocol] = [0, 1, 2].map(|index| real.file(rule.allowlist[index]));
    for (label, mutated) in [
        (
            "inherits the parent environment",
            [
                process.replace(".env_clear()", ".env_remove(\"NODE_OPTIONS\")"),
                host.to_string(),
                protocol.to_string(),
            ],
        ),
        (
            "launches through verifier_env",
            [
                process.replace(
                    "let mut command = Command::new(&launch.executable);",
                    "let mut command = crate::minimal_loop::verifier_env::normalized_command_at_root(\"node\", &launch.cwd);",
                ),
                host.to_string(),
                protocol.to_string(),
            ],
        ),
        (
            "drops the partial cap",
            [
                process.to_string(),
                host.to_string(),
                protocol.replace(
                    "DOMAIN_CAP: DomainVerdict = DomainVerdict::Partial;",
                    "DOMAIN_CAP: DomainVerdict = DomainVerdict::Full;",
                ),
            ],
        ),
    ] {
        let corpus = AuditCorpus::from_pairs([
            (rule.allowlist[0], mutated[0].as_str()),
            (rule.allowlist[1], mutated[1].as_str()),
            (rule.allowlist[2], mutated[2].as_str()),
        ]);
        assert!(
            !(rule.audit)(&corpus, rule).is_empty(),
            "{label} was not rejected"
        );
    }
}

fn audit_protection_coverage(corpus: &AuditCorpus) -> Vec<String> {
    let mut violations = Vec::new();
    for rule in PROTECTION_RULES {
        for violation in (rule.audit)(corpus, rule) {
            violations.push(format!(
                "{} [{} -> {}]: {}",
                rule.category, rule.site_predicate, rule.required_wrapper, violation
            ));
        }
    }
    violations
}

fn audit_compile_output_source_of_truth(
    corpus: &AuditCorpus,
    rule: &ProtectionRule,
) -> Vec<String> {
    let mut violations = Vec::new();
    let build_verifier = corpus.file("src/minimal_loop/build_verifier.rs");
    if !build_verifier.contains("pub fn parse_compile_errors(output: &FullCommandOutput)") {
        violations.push("parse_compile_errors must require &FullCommandOutput".to_string());
    }
    if build_verifier.contains("pub fn parse_compile_errors(output: &str)") {
        violations.push("parse_compile_errors still accepts a raw string".to_string());
    }
    if corpus
        .file("src/planner/runner/acceptance.rs")
        .contains("release_evidence_compile_excerpt_fields")
    {
        violations.push("release evidence still has an excerpt-to-parser fallback".to_string());
    }
    for (path, text) in corpus.rust_files() {
        for (line_index, line) in text.lines().enumerate() {
            if line.contains("parse_compile_errors(")
                && !rule.allowlist.contains(&path.as_str())
                && !line.trim_start().starts_with("///")
            {
                violations.push(format!("{path}:{}: {}", line_index + 1, line.trim()));
            }
            if line.contains("parse_compile_errors(") && line.contains("output_excerpt") {
                violations.push(format!(
                    "{path}:{} parses an output excerpt",
                    line_index + 1
                ));
            }
        }
    }
    violations
}

fn audit_verify_normalization_boundary(corpus: &AuditCorpus, _: &ProtectionRule) -> Vec<String> {
    let mut violations = Vec::new();
    for (path, text) in corpus.rust_files() {
        if path == "src/planner/sanitizer.rs" {
            continue;
        }
        let has_shared_verify_normalizer = text.contains("normalize_planner_verify_command(")
            || text.contains("normalize_runtime_bash_command_for_boundary(");
        let has_normalized_verify_command = text.contains("NormalizedVerifyCommand");
        for (line_index, line) in text.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("///") {
                continue;
            }
            if line.contains("validate_verify_command(")
                && !(has_shared_verify_normalizer || has_normalized_verify_command)
            {
                violations.push(format!(
                    "{path}:{} validate_verify_command call bypasses the shared normalizer",
                    line_index + 1
                ));
            }
            if line.contains("diagnose_verify_command(")
                && !(has_shared_verify_normalizer || has_normalized_verify_command)
            {
                violations.push(format!(
                    "{path}:{} diagnose_verify_command call bypasses the shared normalizer",
                    line_index + 1
                ));
            }
            let calls_executor = line.contains("verifier_env::run_checked(")
                || line.contains("run_structured_for_verify_with_profile(");
            if calls_executor && !has_normalized_verify_command {
                violations.push(format!(
                    "{path}:{}: verifier execution boundary is missing NormalizedVerifyCommand",
                    line_index + 1
                ));
            }
            if line.contains("runtime_bash_policy_decision(")
                && !line.contains("normalize_runtime_bash_command_for_boundary(")
                && !text.contains("normalize_runtime_bash_command_for_boundary(")
            {
                violations.push(format!(
                    "{path}:{} runtime bash policy entry point bypasses the shared normalizer",
                    line_index + 1
                ));
            }
        }
    }
    violations
}

fn audit_bounded_execution_chokepoints(corpus: &AuditCorpus, rule: &ProtectionRule) -> Vec<String> {
    let mut violations = Vec::new();
    for (path, text) in corpus.src_rust_files() {
        if path.starts_with("src/planner/runner/tests/") {
            continue;
        }
        let mut in_test_mod = false;
        for (line_index, line) in text.lines().enumerate() {
            if path != "src/provider_call.rs" && line.contains(".chat(") {
                violations.push(format!(
                    "{path}:{} direct provider .chat call",
                    line_index + 1
                ));
            }
            if !rule.allowlist.contains(&path.as_str())
                && raw_process_invocation(line)
                && !in_test_mod
            {
                violations.push(format!(
                    "{path}:{} direct child-process invocation: {}",
                    line_index + 1,
                    line.trim()
                ));
            }
            let trimmed = line.trim();
            if (trimmed.starts_with("mod tests") || trimmed.starts_with("pub mod tests"))
                && line.contains('{')
                && preceding_cfg_test(text, line_index)
            {
                in_test_mod = true;
            }
        }
    }
    violations
}

fn audit_declarative_command_check_boundary(
    corpus: &AuditCorpus,
    rule: &ProtectionRule,
) -> Vec<String> {
    let path = rule.allowlist[0];
    let source = corpus.file(path);
    let mut violations = Vec::new();
    for required in [
        "normalize_verify_command(&rendered)",
        "verifier_env::normalized_command_at_root",
        "bounded_process::run_with_timeout",
        "const FIXED_TIMEOUT:",
        "shell interpreters are not registered",
        "must stay workspace-relative",
        "command output exceeded max_bytes",
    ] {
        if !source.contains(required) {
            violations.push(format!("{path} is missing `{required}`"));
        }
    }
    violations
}

fn audit_artifact_contract_process_boundary(
    corpus: &AuditCorpus,
    rule: &ProtectionRule,
) -> Vec<String> {
    let [process, host, protocol] = [0, 1, 2].map(|index| rule.allowlist[index]);
    let mut violations = Vec::new();
    for (path, required) in [
        (process, ".env_clear()"),
        (process, ".envs(child_env(&launch.tool_root))"),
        (process, ".process_group(0)"),
        (process, "terminate_process_group(&mut child)"),
        (process, "terminate_process_group_by_pid(pid)"),
        (
            process,
            "const FIXED_TIMEOUT: Duration = Duration::from_secs(30);",
        ),
        (process, "overflow.store(true, Ordering::SeqCst)"),
        (host, "fn separate_from_workspace("),
        (host, "operator tooling changed while the adapter ran"),
        (host, "changed while the adapter ran"),
        (host, "does not match its operator pin"),
        (host, "protocol::cap_domain(receipt.uncapped)"),
        (host, "\"native_effect\": \"failure_only\""),
        (
            protocol,
            "pub(crate) const DOMAIN_CAP: DomainVerdict = DomainVerdict::Partial;",
        ),
        (
            protocol,
            "pub(crate) const MAX_REQUEST_BYTES: usize = 16_384;",
        ),
        (
            protocol,
            "pub(crate) const MAX_STREAM_BYTES: usize = 24_000;",
        ),
        (
            protocol,
            "executed does not equal passed + failed + unresolved",
        ),
    ] {
        if !corpus.file(path).contains(required) {
            violations.push(format!("{path} is missing `{required}`"));
        }
    }
    // The adapter child must never pass through a helper that restores the
    // parent environment or PATH lookup.
    for (line_index, line) in corpus.file(process).lines().enumerate() {
        let code = line.trim_start();
        if code.starts_with("//") {
            continue;
        }
        for forbidden in [
            "verifier_env::",
            "spawn_child(",
            "run_with_timeout",
            "std::env::var",
            "Command::new(\"",
        ] {
            if code.contains(forbidden) {
                violations.push(format!(
                    "{process}:{} uses `{forbidden}` in the adapter launch",
                    line_index + 1
                ));
            }
        }
    }
    violations
}

/// Only these may read a received credential by name, each for its own request.
const STARTUP_CREDENTIAL_READERS: [&str; 6] = [
    "src/bin/gui_server/trial_options.rs",
    "src/config.rs",
    "src/doctor.rs",
    "src/providers/lm_studio.rs",
    "src/minimal_loop/request_pattern/jev.rs",
    "src/tools/profile_tools/effects.rs",
];

/// Every name the GUI gives a delegated child, three of them credentials.
const DELEGATE_ALLOWLIST_FIXED: &str = r#"const DELEGATE_PARENT_ENV_ALLOWLIST: &[&str] = &[
    "PATH",
    "HOME",
    "LANG",
    "LC_ALL",
    "LC_CTYPE",
    "TERM",
    "TMPDIR",
    "TZ",
    "USER",
    "LOGNAME",
    "OPENAI_API_KEY",
    "GEMINI_API_KEY",
    "LM_STUDIO_API_TOKEN",
];"#;

/// The whole function, so its body is all that is allowed and nothing can be added to it.
const DELEGATE_RESTORE_FIXED: &str = r#"fn restore_allowed_environment(command: &mut Command) {
    for name in DELEGATE_PARENT_ENV_ALLOWLIST {
        if let Some(value) = commandagent::startup_credentials::var_os(name) {
            command.env(name, value);
        }
    }
}
"#;

fn audit_startup_credential_boundary(corpus: &AuditCorpus, rule: &ProtectionRule) -> Vec<String> {
    let mut violations = Vec::new();
    let [main, gui, store] = rule.allowlist else {
        return vec!["the rule lists three files".to_string()];
    };
    for (path, required) in [
        (
            *main,
            "fn main() {\n    commandagent::startup_credentials::bootstrap_cli();",
        ),
        (
            *gui,
            "fn main() -> anyhow::Result<()> {\n    commandagent::startup_credentials::bootstrap_gui();",
        ),
        (*store, "command.args(arguments).env_clear();"),
        (*store, ".field(\"count\", &self.0.len())"),
        (
            *store,
            "stop(\"this executable could not be started again\")",
        ),
        (
            *store,
            "if names.iter().any(|name| present(name).is_some()) {",
        ),
        (*store, "\nstatic HELD: OnceLock<Held>"),
    ] {
        if !production_text(corpus.file(path)).contains(required) {
            violations.push(format!("{path} is missing `{required}`"));
        }
    }
    if corpus.file(gui).contains("#[tokio::main]\nasync fn main()") {
        violations.push(format!(
            "{gui} starts its runtime before the credential start"
        ));
    }
    if production_text(corpus.file(store)).contains("set_var") {
        violations.push(format!("{store} puts a value back in the environment"));
    }
    for name in ["OPENAI_API_KEY", "GEMINI_API_KEY", "LM_STUDIO_API_TOKEN"] {
        if corpus.file("src/bin/gui_server/delegate.rs").contains(name)
            && !corpus
                .file("src/bin/gui_server/delegate.rs")
                .contains("        command.env_remove(name);\n")
        {
            violations.push(format!(
                "src/bin/gui_server/delegate.rs gives its version check {name}"
            ));
        }
    }
    let delegate = "src/bin/gui_server/delegate.rs";
    if !corpus.file(delegate).contains(DELEGATE_ALLOWLIST_FIXED) {
        violations.push(format!("{delegate} changed the names it gives a child"));
    }
    if !corpus.file(delegate).contains(DELEGATE_RESTORE_FIXED) {
        violations.push(format!("{delegate} changed what it gives a child"));
    }
    for (path, text) in &corpus.files {
        if !path.starts_with("src/") || path == store {
            continue;
        }
        // The one place a received value goes to a child: this function's body, as fixed above,
        // and nowhere else in its file.
        let without_restore = text.replacen(DELEGATE_RESTORE_FIXED, "", 1);
        let text = if path == delegate {
            &without_restore
        } else {
            text
        };
        let code = production_text(text);
        let reads = code.contains("startup_credentials::var")
            || code.contains("startup_credentials::held_values");
        if reads && !STARTUP_CREDENTIAL_READERS.contains(&path.as_str()) {
            violations.push(format!("{path} reads a received credential"));
        }
        for line in code.lines() {
            if line.contains("startup_credentials::")
                && (line.contains(".env(") || line.contains(".envs(") || line.contains("set_var"))
            {
                violations.push(format!("{path} copies a received credential: {line}"));
            }
        }
    }
    // The GUI's model discovery reads one key for a header of its own request. It starts nothing.
    let discovery = "src/bin/gui_server/trial_options.rs";
    let code = production_text(corpus.file(discovery));
    if code.matches("startup_credentials::").count() > 1
        || (code.contains("startup_credentials::") && !code.contains(DISCOVERY_READ_FIXED))
        || (code.contains("startup_credentials::")
            && (code.contains("Command::new(") || code.contains(".env(")))
    {
        violations.push(format!("{discovery} reads more than its one header value"));
    }
    violations.extend(direct_credential_reads(corpus, store));
    violations
}

const DISCOVERY_READ_FIXED: &str =
    "commandagent::startup_credentials::var_os(\"LM_STUDIO_API_TOKEN\")";

const BUILT_IN_CREDENTIALS: [&str; 4] = [
    "OPENAI_API_KEY",
    "GEMINI_API_KEY",
    "LM_STUDIO_API_TOKEN",
    "JEV_API_KEY",
];

/// One way to read the environment, with whitespace already removed: called, or passed as a path.
const ENVIRONMENT_READS: [&str; 9] = [
    "env::var(",
    "env::var_os(",
    "env::var)",
    "env::var_os)",
    "env_compat::var(",
    "env_compat::var_os(",
    "env_compat::var_with(",
    "env_compat::var)",
    "env_compat::var_os)",
];

/// A read of a credential's name that does not go through `startup_credentials`, which finds a
/// value the start received. On macOS such a read finds nothing, silently.
///
/// What this sees: in one statement of `src/` production code, an environment read together with
/// a built-in name, a constant holding one, or `api_key_env` (a run's key name, known only at
/// run time, so this looks for the variable or field by that name). A loop over names and an
/// `.and_then(|name| std::env::var(name))` on `api_key_env` are one statement, so they count.
///
/// What this does not see: a name that was put in a differently named variable in an earlier
/// statement, a name passed to a helper that reads the environment, a read in a macro or under a
/// renamed import, a name built from pieces, and a scan of every variable (`vars_os`). A string
/// holding a brace that does not balance can also end a test item's removal in the wrong place.
///
/// What is test code: an item under `#[cfg(test)]` or `#[cfg(all(test, ...))]`, by its braces,
/// and a file that a `#[cfg(test)] mod name;` brings in, with whatever that file brings in.
fn direct_credential_reads(corpus: &AuditCorpus, store: &str) -> Vec<String> {
    let test_files = test_only_files(corpus);
    let production = corpus
        .files
        .iter()
        .filter(|(path, _)| path.starts_with("src/") && !test_files.contains(*path))
        .map(|(path, text)| (path, statements(&without_test_items(text))))
        .collect::<Vec<_>>();
    let mut names = BUILT_IN_CREDENTIALS.map(str::to_string).to_vec();
    names.push("api_key_env".to_string());
    for (_, statements) in &production {
        for statement in statements {
            for built_in in BUILT_IN_CREDENTIALS {
                let value = format!(":&str=\"{built_in}\"");
                if let Some(constant) = statement.strip_suffix(&value)
                    && let Some(position) = constant.rfind("const")
                {
                    names.push(constant[position + "const".len()..].to_string());
                }
            }
        }
    }
    let mut violations = Vec::new();
    for (path, statements) in &production {
        if path.as_str() == store {
            continue;
        }
        for statement in statements {
            let reads = ENVIRONMENT_READS
                .iter()
                .any(|read| statement.contains(read));
            if let Some(name) = names.iter().find(|name| statement.contains(name.as_str()))
                && reads
            {
                violations.push(format!(
                    "{path} reads `{name}` from the environment; use startup_credentials"
                ));
            }
        }
    }
    violations
}

/// Files only a test build compiles, followed through the modules they declare.
fn test_only_files(corpus: &AuditCorpus) -> std::collections::BTreeSet<String> {
    let mut found = std::collections::BTreeSet::new();
    loop {
        let before = found.len();
        for (path, text) in &corpus.files {
            let in_test_file = found.contains(path);
            let mut under_attribute = false;
            for line in text.lines().map(str::trim) {
                let declared = line
                    .strip_prefix("pub ")
                    .or_else(|| line.strip_prefix("pub(crate) "))
                    .or_else(|| line.strip_prefix("pub(super) "))
                    .unwrap_or(line)
                    .strip_prefix("mod ")
                    .and_then(|rest| rest.strip_suffix(';'));
                if let Some(name) = declared
                    && (under_attribute || in_test_file)
                {
                    let directory = Path::new(path).parent().unwrap();
                    let own = Path::new(path).with_extension("");
                    for candidate in [
                        directory.join(format!("{name}.rs")),
                        directory.join(name).join("mod.rs"),
                        own.join(format!("{name}.rs")),
                        own.join(name).join("mod.rs"),
                    ] {
                        let candidate = candidate.to_string_lossy().replace('\\', "/");
                        if corpus.files.contains_key(&candidate) {
                            found.insert(candidate);
                        }
                    }
                }
                under_attribute =
                    is_test_attribute(line) || (under_attribute && line.starts_with("#["));
            }
        }
        if found.len() == before {
            return found;
        }
    }
}

fn is_test_attribute(line: &str) -> bool {
    line.starts_with("#[cfg(test)]") || line.starts_with("#[cfg(all(test")
}

/// The text with each test-only item removed: from its attribute through its closing brace or
/// semicolon. Braces in a string, a raw string, or a character are not counted.
fn without_test_items(text: &str) -> String {
    let mut kept = String::with_capacity(text.len());
    let mut rest = text;
    while !rest.is_empty() {
        let line_end = rest.find('\n').map_or(rest.len(), |end| end + 1);
        let (line, after) = rest.split_at(line_end);
        if is_test_attribute(line.trim_start()) {
            rest = &after[item_length(after)..];
        } else {
            kept.push_str(line);
            rest = after;
        }
    }
    kept
}

/// How far the first item in `text` reaches.
fn item_length(text: &str) -> usize {
    let bytes = text.as_bytes();
    let mut depth = 0usize;
    let mut at = 0;
    while at < bytes.len() {
        match bytes[at] {
            b'"' => at = past_string(bytes, at + 1),
            b'r' if matches!(bytes.get(at + 1), Some(b'"' | b'#')) => {
                at = past_raw_string(bytes, at + 1)
            }
            b'\'' if bytes.get(at + 2) == Some(&b'\'') => at += 3,
            b'\'' if bytes.get(at + 1) == Some(&b'\\') && bytes.get(at + 3) == Some(&b'\'') => {
                at += 4
            }
            b'/' if bytes.get(at + 1) == Some(&b'/') => {
                while at < bytes.len() && bytes[at] != b'\n' {
                    at += 1;
                }
            }
            b'{' => {
                depth += 1;
                at += 1;
            }
            b'}' => {
                at += 1;
                if depth <= 1 {
                    return at;
                }
                depth -= 1;
            }
            b';' if depth == 0 => return at + 1,
            _ => at += 1,
        }
    }
    bytes.len()
}

fn past_string(bytes: &[u8], mut at: usize) -> usize {
    while at < bytes.len() {
        match bytes[at] {
            b'\\' => at += 2,
            b'"' => return at + 1,
            _ => at += 1,
        }
    }
    at
}

fn past_raw_string(bytes: &[u8], mut at: usize) -> usize {
    let mut hashes = 0;
    while bytes.get(at) == Some(&b'#') {
        hashes += 1;
        at += 1;
    }
    if bytes.get(at) != Some(&b'"') {
        return at;
    }
    at += 1;
    while at < bytes.len() {
        if bytes[at] == b'"'
            && bytes[at + 1..]
                .iter()
                .take(hashes)
                .filter(|byte| **byte == b'#')
                .count()
                == hashes
        {
            return at + 1 + hashes;
        }
        at += 1;
    }
    at
}

/// Statements with whitespace and whole-line comments removed, so a call split over lines reads
/// as one.
fn statements(text: &str) -> Vec<String> {
    let joined = text
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .flat_map(str::chars)
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    joined.split(';').map(str::to_string).collect()
}

/// The text of a file above its test module: a unit test may start what production must not.
fn production_text(text: &str) -> &str {
    text.find("#[cfg(test)]\nmod tests")
        .or_else(|| text.find("#[cfg(all(test, unix))]\nmod tests"))
        .map_or(text, |position| &text[..position])
}

const PROFILE_TOOL_EFFECT_REQUIRED: [(usize, &str); 44] = [
    (
        0,
        "command.pre_exec(move || mark_inherited_descriptors_close_on_exec(calls));",
    ),
    (0, "libc::F_SETFD, libc::FD_CLOEXEC"),
    (0, "if info.proc_fd > 2 && !(calls.mark)(info.proc_fd) {"),
    (0, "if count == 0 || count >= CAPACITY {"),
    (0, "if table == 0 || table > CAPACITY * entry {"),
    (0, "answer == 0 || errno == Some(libc::EBADF)"),
    (1, "    launch: StrictCommand,"),
    (3, "let _ = self.parent.unlink(&self.name);"),
    (3, "let synced = parent.sync_directory().is_ok();"),
    (3, "cancelled_after_commit: is_cancelled(),"),
    (3, "if !entry_holds_candidate("),
    (0, ".env_clear()"),
    (0, ".envs(launch.env)"),
    (0, ".process_group(0)"),
    (0, "(deny default)"),
    (0, "(deny network*)"),
    (0, r#"(allow process-exec (literal (param "EXE")))"#),
    (0, r#"(allow file-write-data (literal "/dev/null"))"#),
    (0, "return Err(no_backend());"),
    (0, "judge_probe("),
    (1, "terminate_process_group(&mut child)"),
    (1, "tracker.stop_all()"),
    (1, "budget.overflow.store(true, Ordering::SeqCst)"),
    (
        1,
        "budget.read_failed.store(true, Ordering::SeqCst);\n                    return kept;",
    ),
    (1, "stop = Some(Ended::Cancelled)"),
    (1, "stop = Some(Ended::TimedOut)"),
    (1, "feed_stdin(&mut pipe"),
    (1, "descendant_left_behind"),
    (2, "pub(crate) const MAX_RESULT_BYTES: usize = 64 * 1024;"),
    (2, "pub(crate) const MAX_TIMEOUT_MS: u64 = 30_000;"),
    (2, r#"Some("passed") => {}"#),
    (2, "request_id_mismatch"),
    (2, "runtime_pin_mismatch"),
    (2, "output_digest_mismatch"),
    (3, ".create_exclusive(&temporary_name)"),
    (3, ".sync_all()"),
    (3, ".rename_to_leaf(&temporary.name)"),
    (3, "temporary.adopted = true;"),
    (3, "metadata.nlink() != 1"),
    (4, "self.judge_now(context, STAGE_PROCESS)?;"),
    (4, "&|| self.judge_now(context, STAGE_COMMIT),"),
    (4, "self.verify_pins(STAGE_COMMIT)?;"),
    (4, "\"Write\","),
    (5, "is_cancelled() || is_force_cancelled()"),
];

fn audit_profile_tool_effect_boundary(corpus: &AuditCorpus, rule: &ProtectionRule) -> Vec<String> {
    let mut violations = Vec::new();
    for (index, required) in PROFILE_TOOL_EFFECT_REQUIRED {
        let path = rule.allowlist[index];
        if !production_text(corpus.file(path)).contains(required) {
            violations.push(format!("{path} is missing `{required}`"));
        }
    }
    let isolation = rule.allowlist[0];
    if production_text(corpus.file(isolation))
        .matches("return Err(no_backend());")
        .count()
        != 2
    {
        violations.push(format!(
            "{isolation} must refuse both the probe and the launch without a backend"
        ));
    }
    for forbidden in [
        "allow default",
        "allow network",
        "process-fork",
        "mach-lookup",
        "os_sandbox::",
        "wrap_command",
        "getdtablesize",
    ] {
        let found = production_text(corpus.file(isolation))
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .any(|line| line.contains(forbidden));
        if found {
            violations.push(format!("{isolation} uses `{forbidden}`"));
        }
    }
    let process = rule.allowlist[1];
    if production_text(corpus.file(process))
        .matches("tracker.stop_all()")
        .count()
        != 2
    {
        violations.push(format!(
            "{process} must stop descendants on both ways out of a run"
        ));
    }
    for (path, forbidden) in [
        (isolation, "spawn_child("),
        (process, "spawn_child("),
        (process, "verifier_env::"),
        (process, "run_with_timeout"),
        (process, "std::env::var"),
        (process, "Command::new("),
    ] {
        if production_text(corpus.file(path)).contains(forbidden) {
            violations.push(format!("{path} uses `{forbidden}` in the launch"));
        }
    }
    let commit = production_text(corpus.file(rule.allowlist[3]));
    // Twice each: the commit's start and right before the rename; before and after it.
    for (token, count) in [
        ("final_check()?;", 2),
        ("if is_cancelled() {", 2),
        ("if !entry_holds_candidate(", 2),
    ] {
        if commit.matches(token).count() != count {
            violations.push(format!(
                "{} must hold `{token}` {count} times",
                rule.allowlist[3]
            ));
        }
    }
    let rename = commit.find(".rename_to_leaf(&temporary.name)");
    let last_gate = ["final_check()?;", "if is_cancelled() {"]
        .map(|token| commit.rfind(token))
        .into_iter()
        .max()
        .flatten();
    let first_identity = commit.find("if !entry_holds_candidate(");
    let last_identity = commit.rfind("if !entry_holds_candidate(");
    if !(last_gate < first_identity && first_identity < rename && rename < last_identity) {
        violations.push(format!(
            "{} must judge, cancel, and compare the held file before the rename, and compare it after",
            rule.allowlist[3]
        ));
    }
    let order = [
        ".create_exclusive(&temporary_name)",
        "file.write_all(plan.bytes)",
        "file.sync_all()",
        ".rename_to_leaf(&temporary.name)",
        "temporary.adopted = true;",
    ]
    .map(|step| commit.find(step));
    let in_order =
        order.iter().all(Option::is_some) && order.windows(2).all(|pair| pair[0] < pair[1]);
    if !in_order {
        violations.push(format!(
            "{} does not write, sync, and rename a temporary in that order",
            rule.allowlist[3]
        ));
    }
    let effects = production_text(corpus.file(rule.allowlist[4]));
    if effects.contains("\"Read\"") || effects.contains("access\": \"read_only") {
        violations.push(format!("{} treats the effect as a read", rule.allowlist[4]));
    }
    violations
}

fn audit_fetch_network_boundary(corpus: &AuditCorpus, rule: &ProtectionRule) -> Vec<String> {
    let mut violations = Vec::new();
    let transport = corpus.file("src/fetch_probe/transport.rs");
    if !transport.contains("impl FetchTransport for BoundedCurlTransport") {
        violations.push("bounded curl transport implementation is missing".to_string());
    }
    if !transport.contains("bounded_process::run_with_timeout(") {
        violations.push("fetch transport bypasses bounded_process".to_string());
    }
    if transport.contains("reqwest::") {
        violations.push("fetch transport uses an in-process network client".to_string());
    }
    for (path, text) in corpus.src_rust_files() {
        let mut in_test_mod = false;
        for (line_index, line) in text.lines().enumerate() {
            let trimmed = line.trim();
            if (trimmed.starts_with("mod tests") || trimmed.starts_with("pub mod tests"))
                && line.contains('{')
                && preceding_cfg_test(text, line_index)
            {
                in_test_mod = true;
            }
            if in_test_mod {
                continue;
            }
            let constructs_fetch_child =
                line.contains("Command::new(\"curl\"") || line.contains("Command::new(\"wget\"");
            if constructs_fetch_child && path != "src/fetch_probe/transport.rs" {
                violations.push(format!(
                    "{path}:{} constructs a fetch child outside the boundary",
                    line_index + 1
                ));
            }
            if line.contains("transport.get(") && path != "src/fetch_probe/mod.rs" {
                violations.push(format!(
                    "{path}:{} dispatches fetch transport outside the boundary",
                    line_index + 1
                ));
            }
        }
    }
    for path in rule.allowlist {
        if corpus.file(path).is_empty() {
            violations.push(format!("registered fetch boundary file is missing: {path}"));
        }
    }
    let registry = corpus.file("src/tools/registry.rs");
    if registry.contains("spec(\"Fetch\"") || registry.contains("spec(\"Curl\"") {
        violations.push("LLM tool registry exposes a raw network fetch tool".to_string());
    }
    violations
}

fn audit_provider_timeout_enforcement(corpus: &AuditCorpus, _: &ProtectionRule) -> Vec<String> {
    let mut violations = Vec::new();
    let providers = corpus.file("src/providers/mod.rs");
    if !providers.contains("fn boxed_clone(&self) -> Box<dyn ChatClient>;") {
        violations.push("ChatClient::boxed_clone must be a required Box return".to_string());
    }
    if providers.contains("fn boxed_clone(&self) -> Option")
        || providers.contains("boxed_clone(&self) -> Option")
    {
        violations.push("ChatClient::boxed_clone is optional".to_string());
    }
    let provider_call = corpus.file("src/provider_call.rs");
    for forbidden in [
        "let Some(mut worker_client) = client.boxed_clone() else",
        "client.chat(model, messages, tools, native_tools_enabled)",
    ] {
        if provider_call.contains(forbidden) {
            violations.push(format!(
                "provider_call contains synchronous timeout fallback marker `{forbidden}`"
            ));
        }
    }
    violations
}

fn audit_workspace_policy_tool_paths(corpus: &AuditCorpus, _: &ProtectionRule) -> Vec<String> {
    let mut violations = Vec::new();
    let registry = corpus.file("src/tools/registry.rs");
    if registry.is_empty() {
        violations.push("src/tools/registry.rs is missing from audit corpus".to_string());
        return violations;
    }
    if !registry.contains("fn resolve_policy_checked_path(") {
        violations.push("missing resolve_policy_checked_path helper".to_string());
    }
    for tool in registry_path_argument_tools(registry) {
        if !contains_policy_checked_tool_call(registry, &tool) {
            violations.push(format!(
                "ToolRegistry `{tool}` path argument does not use resolve_policy_checked_path"
            ));
        }
    }
    let helper = function_body(registry, "resolve_policy_checked_path").unwrap_or_default();
    if !helper.contains("ensure_tool_path_allowed(") {
        violations
            .push("resolve_policy_checked_path does not call ensure_tool_path_allowed".to_string());
    }
    let registry_without_helper = registry.replace(&helper, "");
    for raw_resolver in [
        "resolve_existing(&context.root",
        "resolve_for_create(&context.root",
        "resolve_optional_existing(&context.root",
    ] {
        if registry_without_helper.contains(raw_resolver) {
            violations.push(format!(
                "ToolRegistry path resolver bypasses policy helper: {raw_resolver}"
            ));
        }
    }
    let extension = corpus.file("src/tools/extension.rs");
    if extension.is_empty() {
        violations.push("src/tools/extension.rs is missing from audit corpus".to_string());
    } else {
        let helper =
            function_body(extension, "normalize_existing_workspace_path").unwrap_or_default();
        for required in [
            "normalize_workspace_path(",
            "hidden_path::ensure_reference_allowed(",
            "resolve_existing(",
            "ensure_tool_path_allowed(",
        ] {
            if !helper.contains(required) {
                violations.push(format!(
                    "extension workspace path helper is missing `{required}`"
                ));
            }
        }
    }
    violations
}

fn audit_terminal_records(corpus: &AuditCorpus, _: &ProtectionRule) -> Vec<String> {
    let mut violations = Vec::new();
    let lib = corpus.file("src/lib.rs");
    for required in [
        "DirectCommandCompletionGuard::start(&config)",
        "emit_direct_command_stop_with_status",
        "\"event\": \"tui_command_stop\"",
        "\"build_commit\": build_info::COMMIT",
        "\"build_timestamp\": build_info::TIMESTAMP",
    ] {
        if !lib.contains(required) {
            violations.push(format!("missing terminal guard marker `{required}`"));
        }
    }
    for variant in action_variants(corpus.file("src/config.rs")) {
        if !lib.contains(&format!("Action::{variant}")) {
            violations.push(format!(
                "Action::{variant} is not registered in direct_command_for_action"
            ));
        }
    }
    if !corpus
        .file("tests/conformance/mod.rs")
        .contains("conformance_honest_terminal_covers_simulated_panic_exit")
    {
        violations.push("terminal panic exit conformance pathway is missing".to_string());
    }
    violations
}

fn audit_extension_supply_writes(corpus: &AuditCorpus, rule: &ProtectionRule) -> Vec<String> {
    let mut violations = Vec::new();
    let boundary = corpus.file("src/planner/pack/supply.rs");
    for required in ["pub struct SupplyRoot", "pub mod journal", "pub fn append("] {
        if !boundary.contains(required) {
            violations.push(format!("extension supply boundary is missing `{required}`"));
        }
    }
    let profile_boundary = corpus.file("src/planner/profile_manifest/supply.rs");
    for required in [
        "pub struct ProfileSupplyRoot",
        "pub fn preview(",
        "pub fn register(",
        "install_no_replace(",
    ] {
        if !profile_boundary.contains(required) {
            violations.push(format!("profile supply boundary is missing `{required}`"));
        }
    }
    for (path, text) in corpus.src_rust_files() {
        if rule.allowlist.contains(&path.as_str()) {
            continue;
        }
        let lines = text.lines().collect::<Vec<_>>();
        for index in 0..lines.len() {
            let start = index.saturating_sub(2);
            let end = (index + 3).min(lines.len());
            let window = lines[start..end].join(" ");
            if window.contains("extension_root")
                && [
                    "std::fs::write(",
                    "fs::write(",
                    "create_dir",
                    "OpenOptions",
                    "File::create",
                    "rename(",
                    "remove_file(",
                    "remove_dir",
                ]
                .iter()
                .any(|token| lines[index].contains(token))
            {
                violations.push(format!(
                    "{path}:{} writes through extension_root outside SupplyRoot: {}",
                    index + 1,
                    lines[index].trim()
                ));
            }
        }
    }
    violations
}

fn action_variants(config_source: &str) -> Vec<String> {
    let Some(start) = config_source.find("pub enum Action") else {
        return Vec::new();
    };
    let Some(open) = config_source[start..]
        .find('{')
        .map(|index| start + index + 1)
    else {
        return Vec::new();
    };
    let Some(close) = config_source[open..].find('}').map(|index| open + index) else {
        return Vec::new();
    };
    config_source[open..close]
        .lines()
        .filter_map(|line| {
            let token = line
                .trim()
                .trim_end_matches(',')
                .split(['(', '{', ' '])
                .next()
                .unwrap_or_default();
            (!token.is_empty()).then(|| token.to_string())
        })
        .collect()
}

fn registry_path_argument_tools(registry: &str) -> Vec<String> {
    let mut tools = Vec::new();
    let mut rest = registry;
    while let Some(index) = rest.find("spec(") {
        rest = &rest[index + "spec(".len()..];
        let Some(end) = rest.find("),") else {
            break;
        };
        let call = &rest[..end];
        if call.contains(r#""path""#)
            && let Some(tool) = first_string_literal(call)
        {
            tools.push(tool);
        }
        rest = &rest[end + 2..];
    }
    tools
}

fn contains_policy_checked_tool_call(registry: &str, tool: &str) -> bool {
    let lines = registry.lines().collect::<Vec<_>>();
    lines.iter().enumerate().any(|(index, line)| {
        if !line.contains("resolve_policy_checked_path(") {
            return false;
        }
        let call = lines
            .iter()
            .skip(index)
            .take(6)
            .copied()
            .collect::<Vec<_>>()
            .join("\n");
        call.contains(&format!("\"{tool}\""))
    })
}

fn first_string_literal(value: &str) -> Option<String> {
    let start = value.find('"')? + 1;
    let end = value[start..].find('"')? + start;
    Some(value[start..end].to_string())
}

fn function_body(source: &str, name: &str) -> Option<String> {
    let start = source.find(&format!("fn {name}("))?;
    let body_start = source[start..].find('{')? + start;
    let mut depth = 0usize;
    for (offset, ch) in source[body_start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    let end = body_start + offset + 1;
                    return Some(source[start..end].to_string());
                }
            }
            _ => {}
        }
    }
    None
}

fn raw_process_invocation(line: &str) -> bool {
    if line.contains(".spawn(") && !line.contains(".spawn(move") {
        return true;
    }
    if line.contains(".output(") {
        return true;
    }
    line.contains(".status()") && !line.contains("response.status()")
}

fn preceding_cfg_test(text: &str, line_index: usize) -> bool {
    text.lines()
        .take(line_index)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .take_while(|line| line.trim().is_empty() || line.trim().starts_with("#["))
        .any(|line| line.trim() == "#[cfg(test)]")
}

impl AuditCorpus {
    fn from_manifest_dir() -> Self {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut files = BTreeMap::new();
        for dir in ["src", "tests"] {
            for path in rust_files(&root.join(dir)) {
                let relative = path
                    .strip_prefix(root)
                    .expect("relative source path")
                    .to_string_lossy()
                    .replace('\\', "/");
                let text = fs::read_to_string(&path)
                    .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
                files.insert(relative, text);
            }
        }
        Self { files }
    }

    fn from_pairs<const N: usize>(pairs: [(&str, &str); N]) -> Self {
        Self {
            files: pairs
                .into_iter()
                .map(|(path, text)| (path.to_string(), text.to_string()))
                .collect(),
        }
    }

    fn file(&self, path: &str) -> &str {
        self.files.get(path).map(String::as_str).unwrap_or("")
    }

    fn rust_files(&self) -> impl Iterator<Item = (String, &str)> {
        self.files
            .iter()
            .filter(|(path, _)| {
                path.ends_with(".rs") && path.as_str() != "tests/protection_coverage_audit.rs"
            })
            .map(|(path, text)| (path.clone(), text.as_str()))
    }

    fn src_rust_files(&self) -> impl Iterator<Item = (String, &str)> {
        self.rust_files()
            .filter(|(path, _)| path.starts_with("src/"))
    }
}

fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    collect_rust_files(dir, &mut out);
    out
}

fn collect_rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap_or_else(|err| panic!("failed to read {dir:?}: {err}")) {
        let entry = entry.expect("directory entry");
        let path = entry.path();
        if path.is_dir() {
            collect_rust_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}
