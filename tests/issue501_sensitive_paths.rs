//! Issue #501 — credential-path blocking regression.
//!
//! Synthetic values only: the canary is a fake marker written and read inside an
//! isolated temp workspace. No real `.env`, API key, model, or provider request
//! is used.

use std::path::Path;
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use commandagent::mode::ExecutionMode;
use commandagent::providers::gemini::build_stream_generate_content_request;
use commandagent::state::ConversationMessage;
use commandagent::tools::extension::{
    ExtensionArgument, ExtensionArgumentKind, ExtensionNetworkAccess,
};
use commandagent::tools::mcp::{McpClient, McpToolCall, read_only_tool};
use commandagent::tools::registry::{
    ToolContext, ToolRegistry, recoverable_tool_error, tool_error_kind,
};
use commandagent::tools::workspace_policy::WorkspacePolicy;

const CANARY: &str = "H01_CANARY_JwtStyle_NonPrefix_29486";

const EVERY_POLICY: [WorkspacePolicy; 4] = [
    WorkspacePolicy::Normal,
    WorkspacePolicy::NormalTask,
    WorkspacePolicy::ControllerMetadataAllowed,
    WorkspacePolicy::GeneratedArtifactsAllowed,
];

fn context(
    root: &Path,
    mode: ExecutionMode,
    policy: WorkspacePolicy,
    auto_approve: bool,
) -> ToolContext {
    ToolContext {
        root: root.to_path_buf(),
        mode,
        auto_approve,
        interactive_approval: false,
        offline: true,
        workspace_policy: policy,
        eval_events_path: None,
        expected_paths: Vec::new(),
        protected_paths: Vec::new(),
    }
}

fn write_canary(root: &Path) {
    std::fs::write(root.join(".env"), format!("PRIVATE_TOKEN={CANARY}\n")).unwrap();
}

fn assert_secret_refusal(
    registry: &ToolRegistry,
    tool: &str,
    arguments: Value,
    context: &ToolContext,
) {
    let error = registry.execute(tool, &arguments, context).unwrap_err();
    assert_eq!(
        tool_error_kind(&error),
        "workspace_policy_blocked",
        "{tool} {arguments}: {error}"
    );
    assert!(recoverable_tool_error(&error), "{tool}: {error}");
    assert!(!error.to_string().contains(CANARY), "{tool} leaked canary");
}

#[test]
fn direct_reads_are_denied_for_every_policy_and_plan() {
    let registry = ToolRegistry::default();
    for policy in EVERY_POLICY {
        let dir = tempfile::tempdir().unwrap();
        write_canary(dir.path());
        let ctx = context(dir.path(), ExecutionMode::Act, policy, true);
        assert_secret_refusal(&registry, "Read", json!({"path": ".env"}), &ctx);
        assert_secret_refusal(&registry, "Glob", json!({"pattern": ".env"}), &ctx);
        assert_secret_refusal(
            &registry,
            "Grep",
            json!({"pattern": "TOKEN", "glob": ".env"}),
            &ctx,
        );
    }

    let dir = tempfile::tempdir().unwrap();
    write_canary(dir.path());
    let plan = context(
        dir.path(),
        ExecutionMode::Plan,
        WorkspacePolicy::NormalTask,
        false,
    );
    assert_secret_refusal(&registry, "Read", json!({"path": ".env"}), &plan);
    assert_secret_refusal(&registry, "Glob", json!({"pattern": "**/.env"}), &plan);
    assert_secret_refusal(
        &registry,
        "Grep",
        json!({"pattern": "TOKEN", "glob": "**/.env"}),
        &plan,
    );
}

#[test]
fn broad_enumeration_excludes_secret_names_and_content() {
    let dir = tempfile::tempdir().unwrap();
    write_canary(dir.path());
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::write(dir.path().join("src/app.txt"), "PUBLIC_ONLY\n").unwrap();
    let registry = ToolRegistry::default();
    let ctx = context(
        dir.path(),
        ExecutionMode::Act,
        WorkspacePolicy::NormalTask,
        true,
    );

    let listing = registry
        .execute("Read", &json!({"path": "."}), &ctx)
        .unwrap();
    assert!(listing.contains("src/"), "{listing}");
    assert!(!listing.contains(".env"), "{listing}");
    assert!(!listing.contains(CANARY), "{listing}");

    let globbed = registry
        .execute("Glob", &json!({"pattern": "**/*"}), &ctx)
        .unwrap();
    assert!(globbed.contains("src/app.txt"), "{globbed}");
    assert!(!globbed.contains(".env"), "{globbed}");

    let missed = registry
        .execute("Grep", &json!({"pattern": "PRIVATE_TOKEN"}), &ctx)
        .unwrap();
    assert!(!missed.contains(CANARY), "{missed}");
    assert!(!missed.contains(".env"), "{missed}");

    let found = registry
        .execute("Grep", &json!({"pattern": "PUBLIC_ONLY"}), &ctx)
        .unwrap();
    assert!(found.contains("src/app.txt"), "{found}");
}

#[test]
fn write_and_edit_reject_new_and_existing_secrets() {
    let dir = tempfile::tempdir().unwrap();
    write_canary(dir.path());
    std::fs::write(dir.path().join(".env.local"), "KEEP_ME=1\n").unwrap();
    let registry = ToolRegistry::default();
    let ctx = context(
        dir.path(),
        ExecutionMode::Act,
        WorkspacePolicy::NormalTask,
        true,
    );

    let new_error = registry
        .execute(
            "Write",
            &json!({"path": ".env.production", "content": "x"}),
            &ctx,
        )
        .unwrap_err();
    assert_eq!(tool_error_kind(&new_error), "workspace_policy_blocked");
    assert!(!dir.path().join(".env.production").exists());

    let existing_error = registry
        .execute(
            "Write",
            &json!({"path": ".env", "content": "overwritten"}),
            &ctx,
        )
        .unwrap_err();
    assert_eq!(tool_error_kind(&existing_error), "workspace_policy_blocked");
    assert!(
        std::fs::read_to_string(dir.path().join(".env"))
            .unwrap()
            .contains(CANARY)
    );

    let edit_error = registry
        .execute(
            "Edit",
            &json!({"path": ".env.local", "old_string": "KEEP_ME=1", "new_string": "KEEP_ME=2"}),
            &ctx,
        )
        .unwrap_err();
    assert_eq!(tool_error_kind(&edit_error), "workspace_policy_blocked");
    assert_eq!(
        std::fs::read_to_string(dir.path().join(".env.local")).unwrap(),
        "KEEP_ME=1\n"
    );
}

#[cfg(unix)]
#[test]
fn template_symlink_to_secret_is_rejected_for_read_edit_and_write() {
    let dir = tempfile::tempdir().unwrap();
    write_canary(dir.path());
    std::os::unix::fs::symlink(dir.path().join(".env"), dir.path().join(".env.example")).unwrap();
    let registry = ToolRegistry::default();
    let ctx = context(
        dir.path(),
        ExecutionMode::Act,
        WorkspacePolicy::NormalTask,
        true,
    );

    assert_secret_refusal(&registry, "Read", json!({"path": ".env.example"}), &ctx);
    let edit_error = registry
        .execute(
            "Edit",
            &json!({"path": ".env.example", "old_string": "PRIVATE_TOKEN", "new_string": "TOKEN"}),
            &ctx,
        )
        .unwrap_err();
    assert_eq!(tool_error_kind(&edit_error), "workspace_policy_blocked");
    let write_error = registry
        .execute(
            "Write",
            &json!({"path": ".env.example", "content": "TOKEN=1"}),
            &ctx,
        )
        .unwrap_err();
    assert_eq!(tool_error_kind(&write_error), "workspace_policy_blocked");
    assert!(
        std::fs::read_to_string(dir.path().join(".env"))
            .unwrap()
            .contains(CANARY)
    );
}

#[test]
fn safe_files_and_strict_templates_remain_usable() {
    let dir = tempfile::tempdir().unwrap();
    write_canary(dir.path());
    let registry = ToolRegistry::default();
    let ctx = context(
        dir.path(),
        ExecutionMode::Act,
        WorkspacePolicy::NormalTask,
        true,
    );

    registry
        .execute(
            "Write",
            &json!({"path": "src/app.txt", "content": "PUBLIC\n"}),
            &ctx,
        )
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(dir.path().join("src/app.txt")).unwrap(),
        "PUBLIC\n"
    );

    for (template, seed) in [
        (".env.example", "PLACEHOLDER=1\n"),
        (".env.sample", "PLACEHOLDER=2\n"),
        (".env.template", "PLACEHOLDER=3\n"),
    ] {
        registry
            .execute("Write", &json!({"path": template, "content": seed}), &ctx)
            .unwrap_or_else(|error| panic!("write {template}: {error}"));
        let read = registry
            .execute("Read", &json!({"path": template}), &ctx)
            .unwrap_or_else(|error| panic!("read {template}: {error}"));
        assert!(read.contains(seed.trim()), "{template}: {read}");
        assert!(!read.contains(CANARY), "{template}: {read}");
        registry
            .execute(
                "Edit",
                &json!({"path": template, "old_string": seed.trim(), "new_string": "PLACEHOLDER=9"}),
                &ctx,
            )
            .unwrap_or_else(|error| panic!("edit {template}: {error}"));
        assert!(
            std::fs::read_to_string(dir.path().join(template))
                .unwrap()
                .contains("PLACEHOLDER=9")
        );
    }
}

#[test]
fn bash_direct_references_are_denied_before_execution() {
    let dir = tempfile::tempdir().unwrap();
    write_canary(dir.path());
    std::fs::write(dir.path().join(".env.example"), "PLACEHOLDER=1\n").unwrap();
    let registry = ToolRegistry::default();
    let ctx = context(
        dir.path(),
        ExecutionMode::Act,
        WorkspacePolicy::NormalTask,
        true,
    );
    let absolute = dir.path().join(".env");

    for command in [
        "cat .env".to_string(),
        "head -n 1 .env".to_string(),
        "cat './.env'".to_string(),
        "cat --file=.env".to_string(),
        "cat .env > leaked.txt".to_string(),
        format!("cat < {}", absolute.display()),
        format!("cat {}", absolute.display()),
    ] {
        let error = registry
            .execute("Bash", &json!({"command": command}), &ctx)
            .unwrap_err();
        assert_eq!(
            tool_error_kind(&error),
            "workspace_policy_blocked",
            "{command}: {error}"
        );
        assert!(!error.to_string().contains(CANARY), "{command}");
    }
    assert!(
        !dir.path().join("leaked.txt").exists(),
        "rejected command executed"
    );

    let allowed = registry
        .execute("Bash", &json!({"command": "cat .env.example"}), &ctx)
        .unwrap();
    assert!(allowed.contains("PLACEHOLDER=1"), "{allowed}");
    assert!(!allowed.contains(CANARY), "{allowed}");
}

#[test]
fn bash_broad_recursive_grep_and_listing_prune_credentials() {
    let dir = tempfile::tempdir().unwrap();
    write_canary(dir.path());
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::write(dir.path().join("src/app.txt"), "TOKEN_PUBLIC_VALUE\n").unwrap();
    let registry = ToolRegistry::default();
    let ctx = context(
        dir.path(),
        ExecutionMode::Act,
        WorkspacePolicy::NormalTask,
        true,
    );

    let recursive = registry
        .execute("Bash", &json!({"command": "grep -r TOKEN"}), &ctx)
        .unwrap();
    assert!(!recursive.contains(CANARY), "{recursive}");
    assert!(!recursive.contains(".env"), "{recursive}");
    assert!(recursive.contains("src/app.txt"), "{recursive}");

    let listing = registry
        .execute("Bash", &json!({"command": "ls -R"}), &ctx)
        .unwrap();
    assert!(listing.contains("./src/app.txt"), "{listing}");
    assert!(!listing.contains(".env"), "{listing}");
}

#[derive(Debug)]
struct RecordingClient {
    calls: Mutex<Vec<Value>>,
    result: String,
}

impl RecordingClient {
    fn new(result: &str) -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            result: result.to_string(),
        }
    }

    fn calls(&self) -> Vec<Value> {
        self.calls.lock().unwrap().clone()
    }
}

impl McpClient for RecordingClient {
    fn call_read_only_tool(&self, call: McpToolCall<'_>) -> anyhow::Result<String> {
        self.calls.lock().unwrap().push(call.arguments.clone());
        Ok(self.result.clone())
    }
}

#[test]
fn mcp_workspace_path_secrets_and_unmediated_directories_are_denied() {
    let dir = tempfile::tempdir().unwrap();
    write_canary(dir.path());
    std::fs::create_dir_all(dir.path().join("data")).unwrap();
    std::fs::write(dir.path().join("data/input.txt"), "sample").unwrap();
    let client = Arc::new(RecordingClient::new("inspection result"));
    let tool = read_only_tool(
        "InspectAsset",
        "Inspect one workspace asset without modifying it.",
        "assets",
        "inspect_asset",
        vec![ExtensionArgument::required(
            "path",
            ExtensionArgumentKind::WorkspacePath,
            "Existing workspace-relative asset path.",
        )],
        ExtensionNetworkAccess::None,
        client.clone(),
    )
    .unwrap();
    let registry = ToolRegistry::with_extensions([tool]).unwrap();
    let ctx = context(
        dir.path(),
        ExecutionMode::Act,
        WorkspacePolicy::NormalTask,
        false,
    );

    for target in [".env", "data"] {
        let error = registry
            .execute("InspectAsset", &json!({"path": target}), &ctx)
            .unwrap_err();
        assert_eq!(
            tool_error_kind(&error),
            "extension_tool_rejected",
            "{target}: {error}"
        );
        assert!(!error.to_string().contains(CANARY), "{target}");
    }
    assert!(
        client.calls().is_empty(),
        "MCP client was called for a denial"
    );

    let allowed = registry
        .execute("InspectAsset", &json!({"path": "data/input.txt"}), &ctx)
        .unwrap();
    assert_eq!(allowed, "inspection result");
    assert_eq!(client.calls().len(), 1);
}

#[test]
fn tool_feedback_and_provider_request_body_contain_no_secret() {
    let dir = tempfile::tempdir().unwrap();
    write_canary(dir.path());
    let registry = ToolRegistry::default();
    let ctx = context(
        dir.path(),
        ExecutionMode::Act,
        WorkspacePolicy::NormalTask,
        true,
    );

    let feedback = registry
        .execute("Read", &json!({"path": ".env"}), &ctx)
        .unwrap_err()
        .to_string();
    assert!(!feedback.contains(CANARY), "{feedback}");
    assert!(feedback.contains("workspace_policy_blocked"), "{feedback}");

    let messages = vec![
        ConversationMessage::user("read the workspace env"),
        ConversationMessage::tool("Read", feedback),
    ];
    let body = build_stream_generate_content_request(&messages, &[], 256);
    let rendered = serde_json::to_string(&body).unwrap();
    assert!(!rendered.contains(CANARY), "{rendered}");
}
