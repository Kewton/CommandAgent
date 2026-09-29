#![cfg(unix)]

//! Issue #504 focused test: one exact-value catalog scrub covers the run's
//! session/events/summary/evidence boundaries, provider-sent conversation and
//! tool schema, and the display stream callback; a rejected `${ENV}` base_url
//! never echoes its expansion value; and the shared API that #543/#544 consume
//! is fixed here.

use std::path::Path;
use std::sync::{Arc, Mutex};

use clap::Parser;
use commandagent::cli::Cli;
use commandagent::config::Config;
use commandagent::provider_call::{self, ProviderCallScope};
use commandagent::providers::{AssistantReply, ChatClient};
use commandagent::sensitive_data::{
    REDACTED, RedactionContext, SecretCatalog, StreamScrubber, install_scope, refuse_identity,
    refuse_runnable, reset_scopes_for_tests,
};
use commandagent::state::{ConversationMessage, SessionSnapshot, SessionStore, ToolCall};
use commandagent::tools::bash;
use commandagent::tools::registry::{FunctionSpec, ToolSpec};
use serde_json::{Value, json};

const CANARY: &str = "H01_CANARY_JwtStyle_NonPrefix_29486";
const BAD_BASE_URL_ENV: &str = "ISSUE504_BAD_BASE_URL";
const BAD_BASE_URL_VALUE: &str = "http://bad host/\u{1F600}";
const ENV_MODEL_SECRET: &str = "ISSUE504_MODEL_SECRET";
const ENV_MODEL_VALUE: &str = "H05_MODEL_Only_In_ENV_19832";
const PROVIDER_KEY_ENV: &str = "OPENAI_API_KEY";
const PROVIDER_KEY_VALUE: &str = "H06_PROVIDER_EXTRA_4821";

fn scope_with_canary(events: &Path) -> RedactionContext {
    let mut catalog = SecretCatalog::new();
    catalog.register(CANARY);
    install_scope(
        catalog,
        Some(Path::new("/tmp/issue504-workspace")),
        Some(events),
    )
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

#[test]
fn run_records_never_contain_a_registered_canary_even_nested_or_truncated() {
    reset_scopes_for_tests();
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join(".commandagent/runs/run-1/events.jsonl");
    scope_with_canary(&events);

    // Nested events, arrays, tool-call arguments, and a value whose canary
    // straddles a truncation boundary.
    let padded = format!("{} {} {}", "x".repeat(400), CANARY, "y".repeat(400));
    commandagent::eval_events::emit(
        Some(&events),
        json!({
            "event": "provider_turn_duration",
            "nested": {"messages": [{"content": format!("prefix {CANARY} suffix")}]},
            "tool_calls": [{"arguments": {"command": format!("echo {CANARY}")}}],
            "summary": commandagent::eval_events::body_snippet(&padded),
            "escaped": format!("quote\" {CANARY} newline\\n"),
        }),
    );

    let mut session = SessionSnapshot::new();
    session
        .messages
        .push(ConversationMessage::user(format!("use {CANARY} now")));
    session.messages.push(ConversationMessage::tool_result(
        "Bash",
        None::<String>,
        format!("output {CANARY} tail"),
    ));
    let state_dir = dir.path().join("state");
    SessionStore::new(state_dir.clone()).save(&session).unwrap();

    commandagent::eval_events::write_run_summary(
        Some(&events),
        &format!("Status: running\nAction: {CANARY}\n"),
    );

    for path in [
        events.clone(),
        state_dir
            .join("sessions")
            .join(&session.id)
            .join("session.json"),
        events.parent().unwrap().join("summary.md"),
    ] {
        let text = read(&path);
        assert!(
            !text.contains(CANARY),
            "canary leaked in {}",
            path.display()
        );
        assert!(
            !text.contains("H01_CANARY"),
            "canary fragment in {}",
            path.display()
        );
        assert!(text.contains(REDACTED), "no marker in {}", path.display());
    }
    // The event is still valid JSON with its schema unchanged.
    let first: Value = serde_json::from_str(read(&events).lines().next().unwrap()).unwrap();
    assert_eq!(first["event"], "provider_turn_duration");
    reset_scopes_for_tests();
}

#[test]
fn rejected_bash_prefix_is_empty_in_a_run_and_keeps_schema_hash_and_length() {
    reset_scopes_for_tests();
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join("events.jsonl");
    // A run scope is installed (even empty): the raw prefix must not persist.
    install_scope(SecretCatalog::new(), Some(dir.path()), Some(&events));

    let command = format!("cd /Users/<user>/x && cat {CANARY}");
    let error = bash::run(&command, dir.path(), true).unwrap_err();
    assert!(
        error.to_string().contains("placeholder path detected"),
        "{error}"
    );

    let dir_path = dir.path().join(".commandagent/evidence/bash-placeholders");
    let record: Value = std::fs::read_dir(&dir_path)
        .unwrap()
        .map(|entry| serde_json::from_str(&read(&entry.unwrap().path())).unwrap())
        .next()
        .unwrap();

    assert_eq!(record["schema_version"], "1");
    assert_eq!(record["command_prefix_bytes"], json!([]));
    assert_eq!(record["command_bytes"], command.len());
    assert_eq!(record["command_sha256"], sha256(command.as_bytes()));
    assert_eq!(record["command_prefix_bytes"].as_array().unwrap().len(), 0);
    reset_scopes_for_tests();
}

fn sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}

#[derive(Clone)]
struct CapturingClient {
    seen_messages: Arc<Mutex<Vec<ConversationMessage>>>,
    seen_tools: Arc<Mutex<Vec<ToolSpec>>>,
}

impl ChatClient for CapturingClient {
    fn label(&self) -> &str {
        "capturing-issue504"
    }

    fn boxed_clone(&self) -> Box<dyn ChatClient> {
        Box::new(self.clone())
    }

    fn supports_streaming(&self) -> bool {
        true
    }

    fn chat(
        &mut self,
        _model: &str,
        _messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        Ok(AssistantReply::text("batch"))
    }

    fn chat_stream(
        &mut self,
        _model: &str,
        messages: &[ConversationMessage],
        tools: &[ToolSpec],
        _native_tools_enabled: bool,
        on_chunk: &mut dyn FnMut(&str) -> anyhow::Result<()>,
    ) -> anyhow::Result<AssistantReply> {
        *self.seen_messages.lock().unwrap() = messages.to_vec();
        *self.seen_tools.lock().unwrap() = tools.to_vec();
        // Split the canary across chunks to exercise the stream carry.
        on_chunk("before ")?;
        on_chunk(&CANARY[..7])?;
        on_chunk(&CANARY[7..])?;
        on_chunk(" after")?;
        Ok(AssistantReply::text("before after"))
    }
}

#[test]
fn provider_sent_conversation_and_tool_schema_are_protected() {
    reset_scopes_for_tests();
    let dir = tempfile::tempdir().unwrap();
    let cwd = dir.path().to_string_lossy().to_string();
    let config = Config::from_cli(Cli::parse_from([
        "commandagent",
        "--cwd",
        &cwd,
        "--model",
        "issue504-model",
    ]))
    .unwrap();
    let events = config.eval_events_path.clone().unwrap();
    scope_with_canary(&events);

    let seen_messages = Arc::new(Mutex::new(Vec::new()));
    let seen_tools = Arc::new(Mutex::new(Vec::new()));
    let mut client = CapturingClient {
        seen_messages: Arc::clone(&seen_messages),
        seen_tools: Arc::clone(&seen_tools),
    };
    let messages = vec![ConversationMessage::user(format!("send {CANARY} please"))];
    let tools = vec![ToolSpec {
        kind: "function".to_string(),
        function: FunctionSpec {
            name: "Read".to_string(),
            description: format!("reads {CANARY}"),
            parameters: json!({"type": "object", "examples": [CANARY]}),
        },
    }];

    // A planner scope streams without a TTY, so the provider `chat_stream`
    // surface is exercised and the sent copy can be inspected.
    let outcome = provider_call::chat_with_cancel_and_stream(
        &mut client,
        &config,
        provider_call::ProviderChatRequest {
            scope: ProviderCallScope::PlannerStep,
            model: &config.model,
            messages: &messages,
            tools: &tools,
            native_tools_enabled: true,
        },
        || false,
        &mut |_chunk| Ok(()),
    );
    assert!(outcome.result.is_ok(), "{:?}", outcome.result.err());

    // The caller's originals are untouched (authentication/execution inputs stay).
    assert!(messages[0].content.contains(CANARY));
    assert!(tools[0].function.description.contains(CANARY));

    // The provider-sent copy is scrubbed.
    let sent = seen_messages.lock().unwrap().clone();
    assert!(!sent[0].content.contains(CANARY), "{}", sent[0].content);
    assert!(sent[0].content.contains(REDACTED));
    let sent_tools = seen_tools.lock().unwrap().clone();
    assert!(!sent_tools[0].function.description.contains(CANARY));
    assert!(
        !serde_json::to_string(&sent_tools[0].function.parameters)
            .unwrap()
            .contains(CANARY)
    );
    reset_scopes_for_tests();
}

#[test]
fn rejected_env_base_url_never_echoes_the_expansion_value() {
    let exe = std::env::current_exe().unwrap();
    let status = std::process::Command::new(exe)
        .args([
            "--exact",
            "--ignored",
            "--nocapture",
            "rejected_env_base_url_never_echoes_the_expansion_value_child",
        ])
        .env(BAD_BASE_URL_ENV, BAD_BASE_URL_VALUE)
        .status()
        .unwrap();
    assert!(status.success(), "child exited with {status}");
}

#[test]
#[ignore]
fn rejected_env_base_url_never_echoes_the_expansion_value_child() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".commandagent")).unwrap();
    std::fs::write(
        dir.path().join(".commandagent/config.toml"),
        format!(
            "[preset.bad]\nmodel = \"issue504-model\"\nprovider = \"openai-compatible\"\nbase_url = \"${{{BAD_BASE_URL_ENV}}}\"\n"
        ),
    )
    .unwrap();
    let cwd = dir.path().to_string_lossy().to_string();

    let error = Config::from_cli(Cli::parse_from([
        "commandagent",
        "--cwd",
        &cwd,
        "--preset",
        "bad",
    ]))
    .unwrap_err();
    let rendered = format!("{error:#}");
    assert!(!rendered.contains(BAD_BASE_URL_VALUE), "{rendered}");
    assert!(rendered.contains("base-url"), "{rendered}");
}

#[test]
fn shared_api_fixes_scrub_carry_refusal_and_context_separation() {
    reset_scopes_for_tests();

    // Whole-value scrub and dynamic-key refusal.
    let mut catalog = SecretCatalog::new();
    catalog.register(CANARY);
    catalog.register("plain-secret-value-123");
    assert_eq!(catalog.scrub(&format!("a {CANARY} b")), "a <redacted> b");
    let mut value = json!({"outer": {"leaky": CANARY}});
    assert!(catalog.scrub_value(&mut value).is_ok());
    assert!(!serde_json::to_string(&value).unwrap().contains(CANARY));
    let mut keyed = json!({CANARY: "v"});
    let dynamic = catalog.scrub_value(&mut keyed).unwrap_err();
    assert!(!dynamic.to_string().contains(CANARY), "{dynamic}");

    // Debug never prints a value.
    assert!(!format!("{catalog:?}").contains(CANARY));

    // A short credential value is refused at registration; an ordinary short
    // value is not a secret.
    let mut short = SecretCatalog::new();
    assert!(short.register_credential("GATEWAY_TOKEN", "abc").is_err());
    assert!(short.register_plain("3000").is_ok());
    assert!(!short.contains("abc"));
    assert!(!short.contains("3000"));

    // Stream carry over a split.
    let mut scrubber = StreamScrubber::new(Arc::new(catalog.clone()));
    let mut rendered = String::new();
    for ch in format!("x {CANARY} y").chars() {
        let mut buffer = [0u8; 4];
        rendered.push_str(&scrubber.push(ch.encode_utf8(&mut buffer)));
    }
    rendered.push_str(&scrubber.finish());
    assert_eq!(rendered, "x <redacted> y");

    // Runnable YAML and identity refusal name the field, not the value.
    let refusal =
        refuse_runnable(&catalog, "recovery.yaml", &format!("run: {CANARY}")).unwrap_err();
    assert_eq!(refusal.field, "recovery.yaml");
    assert!(!refusal.to_string().contains(CANARY));
    assert!(refuse_identity(&catalog, "identity", CANARY).is_err());
    assert!(refuse_identity(&catalog, "identity", "clean").is_ok());

    // Context separation: two scopes resolve their own catalogs.
    let mut alpha = SecretCatalog::new();
    alpha.register("alpha-only-secret");
    let mut beta = SecretCatalog::new();
    beta.register("beta-only-secret");
    install_scope(
        alpha,
        Some(Path::new("/tmp/alpha")),
        Some(Path::new("/tmp/alpha/events.jsonl")),
    );
    install_scope(
        beta,
        Some(Path::new("/tmp/beta")),
        Some(Path::new("/tmp/beta/events.jsonl")),
    );
    let resolved_alpha =
        commandagent::sensitive_data::active_for(Some(Path::new("/tmp/alpha/events.jsonl")))
            .unwrap();
    let resolved_beta =
        commandagent::sensitive_data::active_for(Some(Path::new("/tmp/beta/events.jsonl")))
            .unwrap();
    assert!(resolved_alpha.contains("alpha-only-secret"));
    assert!(!resolved_alpha.contains("beta-only-secret"));
    assert!(resolved_beta.contains("beta-only-secret"));
    reset_scopes_for_tests();
}

#[test]
fn summary_writer_scrubs_a_multiline_value_before_splitting_lines() {
    reset_scopes_for_tests();
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join("run/events.jsonl");
    let multiline = "H05_NEWLINE_alpha\nH05_NEWLINE_beta";
    let mut catalog = SecretCatalog::new();
    catalog.register(multiline);
    install_scope(catalog, Some(Path::new("/tmp/h05-summary")), Some(&events));

    commandagent::eval_events::write_run_summary(Some(&events), multiline);

    let summary = read(&events.parent().unwrap().join("summary.md"));
    assert!(!summary.contains("H05_NEWLINE"), "{summary}");
    reset_scopes_for_tests();
}

#[test]
fn provider_and_session_protect_native_tool_call_ids() {
    reset_scopes_for_tests();
    let dir = tempfile::tempdir().unwrap();
    let cwd = dir.path().to_string_lossy().to_string();
    let config = Config::from_cli(Cli::parse_from([
        "commandagent",
        "--cwd",
        &cwd,
        "--model",
        "issue504-model",
    ]))
    .unwrap();
    let events = config.eval_events_path.clone().unwrap();
    scope_with_canary(&events);

    let calls = vec![ToolCall {
        id: CANARY.to_string(),
        name: "Read".to_string(),
        arguments: json!({"path": "normal.rs"}),
    }];
    let messages = vec![
        ConversationMessage::assistant(CANARY, calls),
        ConversationMessage::tool_result("Read", Some(CANARY), "ordinary output"),
    ];

    let seen_messages = Arc::new(Mutex::new(Vec::new()));
    let seen_tools = Arc::new(Mutex::new(Vec::new()));
    let mut client = CapturingClient {
        seen_messages: Arc::clone(&seen_messages),
        seen_tools: Arc::clone(&seen_tools),
    };
    let outcome = provider_call::chat_with_cancel_and_stream(
        &mut client,
        &config,
        provider_call::ProviderChatRequest {
            scope: ProviderCallScope::PlannerStep,
            model: &config.model,
            messages: &messages,
            tools: &[],
            native_tools_enabled: false,
        },
        || false,
        &mut |_chunk| Ok(()),
    );
    assert!(outcome.result.is_ok());

    let sent = seen_messages.lock().unwrap().clone();
    assert!(
        !serde_json::to_string(&sent).unwrap().contains(CANARY),
        "provider copy kept a tool-call id"
    );
    // The caller's model-derived ids stay unchanged.
    assert_eq!(messages[0].tool_calls[0].id, CANARY);
    assert_eq!(messages[1].tool_call_id.as_deref(), Some(CANARY));

    let mut session = SessionSnapshot::new();
    session.messages = messages.clone();
    let stored = SessionStore::new(dir.path().join("state"))
        .save(&session)
        .unwrap();
    assert!(
        !read(&stored).contains(CANARY),
        "session kept a tool-call id"
    );
    reset_scopes_for_tests();
}

#[test]
fn config_env_scope_survives_the_minimal_loop_entry() {
    let exe = std::env::current_exe().unwrap();
    let status = std::process::Command::new(exe)
        .args([
            "--exact",
            "--ignored",
            "--nocapture",
            "config_env_scope_survives_the_minimal_loop_entry_child",
        ])
        .env(ENV_MODEL_SECRET, ENV_MODEL_VALUE)
        .status()
        .unwrap();
    assert!(status.success(), "child exited with {status}");
}

#[test]
#[ignore]
fn config_env_scope_survives_the_minimal_loop_entry_child() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".commandagent")).unwrap();
    std::fs::write(
        dir.path().join(".commandagent/config.toml"),
        format!("[preset.review]\nprovider = \"ollama\"\nmodel = \"${{{ENV_MODEL_SECRET}}}\"\n"),
    )
    .unwrap();
    let cwd = dir.path().to_string_lossy().to_string();
    let mut config = Config::from_cli(Cli::parse_from([
        "commandagent",
        "--cwd",
        &cwd,
        "--preset",
        "review",
        "--offline",
    ]))
    .unwrap();
    let model = config.model.clone();
    assert_eq!(model, ENV_MODEL_VALUE);
    let events = dir.path().join("run/events.jsonl");
    config.eval_events_path = Some(events.clone());

    assert!(
        commandagent::sensitive_data::current()
            .unwrap()
            .contains(&model),
        "config resolution did not register the expansion value"
    );
    // Recreating the scope at the minimal-loop entry must reuse, not discard, it.
    commandagent::config::install_run_secret_scope(&config).unwrap();
    assert!(
        commandagent::sensitive_data::current()
            .unwrap()
            .contains(&model),
        "the minimal-loop entry discarded the config expansion value"
    );
    commandagent::eval_events::emit(
        Some(&events),
        json!({"event": "h05_env_model", "model": model}),
    );
    assert!(
        !read(&events).contains(ENV_MODEL_VALUE),
        "the expansion value leaked after the entry"
    );
}

#[test]
fn direct_minimal_loop_refuses_a_collection_failure() {
    reset_scopes_for_tests();
    let dir = tempfile::tempdir().unwrap();
    let cwd = dir.path().to_string_lossy().to_string();
    let mut config = Config::from_cli(Cli::parse_from([
        "commandagent",
        "--cwd",
        &cwd,
        "--model",
        "issue504-model",
    ]))
    .unwrap();
    let events = dir.path().join("run/events.jsonl");
    config.eval_events_path = Some(events.clone());
    // The oversized root dotenv appears after the config is resolved.
    std::fs::write(
        dir.path().join(".env"),
        format!("PRIVATE_TOKEN={CANARY}\n{}", "#".repeat(1024 * 1024 + 1)),
    )
    .unwrap();
    reset_scopes_for_tests();

    let seen_messages = Arc::new(Mutex::new(Vec::new()));
    let seen_tools = Arc::new(Mutex::new(Vec::new()));
    let mut client = CapturingClient {
        seen_messages,
        seen_tools,
    };
    let mut session = SessionSnapshot::new();
    let error = commandagent::minimal_loop::run_session(
        &mut client,
        &mut session,
        "Report current state.",
        &config,
    )
    .unwrap_err();
    assert!(
        format!("{error:#}").contains("collection cap"),
        "the entry did not refuse the collection failure: {error:#}"
    );

    // The partial catalog is installed, so a later emit is still scrubbed.
    commandagent::eval_events::emit(
        Some(&events),
        json!({"event": "h06_direct_cap", "message": CANARY}),
    );
    assert!(!read(&events).contains(CANARY));
    reset_scopes_for_tests();
}

#[test]
fn combined_source_overflow_refuses_config() {
    let exe = std::env::current_exe().unwrap();
    let status = std::process::Command::new(exe)
        .args([
            "--exact",
            "--ignored",
            "--nocapture",
            "combined_source_overflow_refuses_config_child",
        ])
        .env(PROVIDER_KEY_ENV, PROVIDER_KEY_VALUE)
        .status()
        .unwrap();
    assert!(status.success(), "child exited with {status}");
}

#[test]
#[ignore]
fn combined_source_overflow_refuses_config_child() {
    let dir = tempfile::tempdir().unwrap();
    let mut lines = String::new();
    for index in 0..1024 {
        lines.push_str(&format!(
            "SECRET_{index:04}=H06_COMBINED_VALUE_{index:04}_only\n"
        ));
    }
    std::fs::write(dir.path().join(".env"), lines).unwrap();
    let cwd = dir.path().to_string_lossy().to_string();
    let result = Config::from_cli(Cli::parse_from([
        "commandagent",
        "--cwd",
        &cwd,
        "--model",
        "h06-model",
        "--offline",
    ]));
    assert!(
        result.is_err(),
        "a combined provider+dotenv overflow must refuse the config"
    );
}

#[test]
fn install_run_secret_scope_isolates_workspaces() {
    reset_scopes_for_tests();
    // Workspace B registers its own dotenv credential value.
    let b = tempfile::tempdir().unwrap();
    std::fs::write(b.path().join(".env"), "PRIVATE_TOKEN=H06_B_VALUE_6382\n").unwrap();
    let b_cwd = b.path().to_string_lossy().to_string();
    let b_config = Config::from_cli(Cli::parse_from([
        "commandagent",
        "--cwd",
        &b_cwd,
        "--model",
        "h06-model",
        "--offline",
    ]))
    .unwrap();

    // Workspace A installs its own catalog under A's paths.
    let a = tempfile::tempdir().unwrap();
    let a_events = a.path().join("run/events.jsonl");
    let mut a_catalog = SecretCatalog::new();
    a_catalog.register("H06_ROOT_A_ONLY_7193");
    install_scope(a_catalog, Some(a.path()), Some(&a_events));

    // The B entry must reuse B's own catalog, never A's.
    let b_context = commandagent::config::install_run_secret_scope(&b_config).unwrap();
    assert!(b_context.contains("H06_B_VALUE_6382"));
    assert!(!b_context.contains("H06_ROOT_A_ONLY_7193"));
    assert_eq!(
        b_context.scrub_text("ordinary H06_ROOT_A_ONLY_7193"),
        "ordinary H06_ROOT_A_ONLY_7193"
    );
    reset_scopes_for_tests();
}

#[test]
fn provider_protocol_names_are_preserved_while_free_fields_are_scrubbed() {
    reset_scopes_for_tests();
    let dir = tempfile::tempdir().unwrap();
    let cwd = dir.path().to_string_lossy().to_string();
    let config = Config::from_cli(Cli::parse_from([
        "commandagent",
        "--cwd",
        &cwd,
        "--model",
        "issue504-model",
    ]))
    .unwrap();
    let events = config.eval_events_path.clone().unwrap();
    let secret = "inspector_tool";
    let mut catalog = SecretCatalog::new();
    catalog.register(secret);
    install_scope(catalog, Some(dir.path()), Some(&events));

    let tool_name = format!("workspace_{secret}");
    let tool = ToolSpec {
        kind: "function".to_string(),
        function: FunctionSpec {
            name: tool_name.clone(),
            description: format!("reads {secret} data"),
            parameters: json!({"type": "object"}),
        },
    };
    let messages = vec![ConversationMessage::tool_result(
        tool_name.clone(),
        None::<String>,
        "ordinary output",
    )];
    let seen_messages = Arc::new(Mutex::new(Vec::new()));
    let seen_tools = Arc::new(Mutex::new(Vec::new()));
    let mut client = CapturingClient {
        seen_messages: Arc::clone(&seen_messages),
        seen_tools: Arc::clone(&seen_tools),
    };
    let outcome = provider_call::chat_with_cancel_and_stream(
        &mut client,
        &config,
        provider_call::ProviderChatRequest {
            scope: ProviderCallScope::PlannerStep,
            model: &config.model,
            messages: &messages,
            tools: &[tool],
            native_tools_enabled: true,
        },
        || false,
        &mut |_| Ok(()),
    );
    assert!(outcome.result.is_ok());

    let sent_tools = seen_tools.lock().unwrap().clone();
    assert_eq!(sent_tools[0].function.name, tool_name);
    assert_eq!(sent_tools[0].function.description, "reads <redacted> data");
    let sent_messages = seen_messages.lock().unwrap().clone();
    assert_eq!(sent_messages[0].name.as_deref(), Some(tool_name.as_str()));
    reset_scopes_for_tests();
}

#[test]
fn legacy_format_heuristic_uses_the_run_marker() {
    reset_scopes_for_tests();
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join("run/events.jsonl");
    let mut catalog = SecretCatalog::new();
    catalog.register("redacted");
    catalog.register(CANARY);
    install_scope(catalog, Some(dir.path()), Some(&events));
    let marker = commandagent::sensitive_data::active_marker();
    assert_ne!(marker, "<redacted>");

    let snippet = commandagent::eval_events::body_snippet("sk-h06-format-only-key");
    assert_eq!(snippet, marker);
    assert!(
        !commandagent::sensitive_data::current()
            .unwrap()
            .contains(&snippet)
    );

    commandagent::eval_events::write_run_summary(Some(&events), "sk-h06-format-only-key");
    let summary = read(&events.parent().unwrap().join("summary.md"));
    assert!(
        !commandagent::sensitive_data::current()
            .unwrap()
            .contains(&summary)
    );
    reset_scopes_for_tests();
}

#[test]
fn free_tool_arguments_are_scrubbed_in_the_provider_copy_and_session() {
    reset_scopes_for_tests();
    let dir = tempfile::tempdir().unwrap();
    let cwd = dir.path().to_string_lossy().to_string();
    let config = Config::from_cli(Cli::parse_from([
        "commandagent",
        "--cwd",
        &cwd,
        "--model",
        "issue504-model",
    ]))
    .unwrap();
    let events = config.eval_events_path.clone().unwrap();
    scope_with_canary(&events);

    let arguments = json!({
        "path": "ordinary.json",
        "status": CANARY,
        "type": CANARY,
        "action": CANARY,
        "tool_name": CANARY,
        "content": CANARY,
    });
    let calls = vec![ToolCall {
        id: "call-1".to_string(),
        name: "Write".to_string(),
        arguments: arguments.clone(),
    }];
    let messages = vec![ConversationMessage::assistant("note", calls)];

    let seen_messages = Arc::new(Mutex::new(Vec::new()));
    let seen_tools = Arc::new(Mutex::new(Vec::new()));
    let mut client = CapturingClient {
        seen_messages: Arc::clone(&seen_messages),
        seen_tools: Arc::clone(&seen_tools),
    };
    let outcome = provider_call::chat_with_cancel_and_stream(
        &mut client,
        &config,
        provider_call::ProviderChatRequest {
            scope: ProviderCallScope::PlannerStep,
            model: &config.model,
            messages: &messages,
            tools: &[],
            native_tools_enabled: false,
        },
        || false,
        &mut |_| Ok(()),
    );
    assert!(outcome.result.is_ok());
    let sent = seen_messages.lock().unwrap().clone();
    assert!(
        !serde_json::to_string(&sent).unwrap().contains(CANARY),
        "provider copy kept a free-argument secret"
    );

    let mut session = SessionSnapshot::new();
    session.messages = messages.clone();
    let stored = SessionStore::new(dir.path().join("state"))
        .save(&session)
        .unwrap();
    assert!(
        !read(&stored).contains(CANARY),
        "session kept a free-argument secret"
    );

    // The caller's execution arguments are unchanged.
    assert_eq!(messages[0].tool_calls[0].arguments["status"], CANARY);
    reset_scopes_for_tests();
}

#[test]
fn summary_status_contract_is_preserved() {
    reset_scopes_for_tests();
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join("run/events.jsonl");
    let mut catalog = SecretCatalog::new();
    catalog.register("completed");
    install_scope(catalog, Some(dir.path()), Some(&events));

    commandagent::eval_events::write_run_summary(
        Some(&events),
        "Status: completed\nStop reason: completed",
    );

    let summary = read(&events.parent().unwrap().join("summary.md"));
    assert!(
        summary.contains("Status: completed"),
        "fixed status was rewritten: {summary}"
    );
    assert!(
        !summary.contains("Stop reason: completed"),
        "the free reason was not scrubbed: {summary}"
    );
    assert!(summary.contains("<redacted>"), "{summary}");
    reset_scopes_for_tests();
}
