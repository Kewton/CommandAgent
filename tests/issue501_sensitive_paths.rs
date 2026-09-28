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
use commandagent::tools::{bash, edit, read, write};

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

/// Builds the four parent-directory-symlink credential systems used by B-01 and
/// returns `(lexical alias path, real credential path)` pairs.
#[cfg(unix)]
fn install_parent_symlink_secrets(root: &Path) -> Vec<(&'static str, &'static str)> {
    for (real_dir, file) in [
        (".ssh", "id_ed25519"),
        (".aws", "credentials"),
        (".docker", "config.json"),
        (".config/gcloud", "application_default_credentials.json"),
    ] {
        let dir = root.join(real_dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(file), format!("PRIVATE_TOKEN={CANARY}\n")).unwrap();
    }
    for (alias, real_dir) in [
        ("keys", ".ssh"),
        ("cfg", ".aws"),
        ("docker_cfg", ".docker"),
        ("gcfg", ".config/gcloud"),
    ] {
        std::os::unix::fs::symlink(root.join(real_dir), root.join(alias)).unwrap();
    }
    vec![
        ("keys/id_ed25519", ".ssh/id_ed25519"),
        ("cfg/credentials", ".aws/credentials"),
        ("docker_cfg/config.json", ".docker/config.json"),
        (
            "gcfg/application_default_credentials.json",
            ".config/gcloud/application_default_credentials.json",
        ),
    ]
}

#[cfg(unix)]
#[test]
fn parent_directory_symlink_is_refused_for_write_edit_read_and_grep() {
    let registry = ToolRegistry::default();
    for policy in EVERY_POLICY {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let cases = install_parent_symlink_secrets(root);
        let ctx = context(root, ExecutionMode::Act, policy, true);

        for (lexical, real) in &cases {
            let write_error = registry
                .execute(
                    "Write",
                    &json!({"path": lexical, "content": "overwritten"}),
                    &ctx,
                )
                .unwrap_err();
            assert_eq!(
                tool_error_kind(&write_error),
                "workspace_policy_blocked",
                "Write {lexical}: {write_error}"
            );
            assert!(
                std::fs::read_to_string(root.join(real))
                    .unwrap()
                    .contains(CANARY),
                "Write {lexical} changed real bytes"
            );

            let edit_error = registry
                .execute(
                    "Edit",
                    &json!({"path": lexical, "old_string": "PRIVATE_TOKEN", "new_string": "X"}),
                    &ctx,
                )
                .unwrap_err();
            assert_eq!(
                tool_error_kind(&edit_error),
                "workspace_policy_blocked",
                "Edit {lexical}: {edit_error}"
            );

            let grep_error = registry
                .execute("Grep", &json!({"pattern": "TOKEN", "glob": lexical}), &ctx)
                .unwrap_err();
            assert_eq!(
                tool_error_kind(&grep_error),
                "workspace_policy_blocked",
                "Grep {lexical}: {grep_error}"
            );

            let alias = root.join(lexical);
            let leaf_read = read::run(root, &alias, None, None, policy).unwrap_err();
            assert_eq!(
                tool_error_kind(&leaf_read),
                "workspace_policy_blocked",
                "leaf Read {lexical}: {leaf_read}"
            );
            let leaf_write = write::write_checked(root, &alias, "x").unwrap_err();
            assert_eq!(
                tool_error_kind(&leaf_write),
                "workspace_policy_blocked",
                "leaf Write {lexical}: {leaf_write}"
            );
            let leaf_edit = edit::run(root, &alias, "PRIVATE_TOKEN", "X", false).unwrap_err();
            assert_eq!(
                tool_error_kind(&leaf_edit),
                "workspace_policy_blocked",
                "leaf Edit {lexical}: {leaf_edit}"
            );
        }

        let new_error = registry
            .execute(
                "Write",
                &json!({"path": "keys/new_key", "content": "x"}),
                &ctx,
            )
            .unwrap_err();
        assert_eq!(tool_error_kind(&new_error), "workspace_policy_blocked");
        assert!(!root.join(".ssh/new_key").exists());
    }
}

#[cfg(unix)]
#[test]
fn edit_error_excerpt_does_not_leak_parent_symlink_secret() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    install_parent_symlink_secrets(root);
    let old = format!("PRIVATE_TOKEN={CANARY}\nNONEXISTENT_ANCHOR");
    let error = edit::run(root, &root.join("keys/id_ed25519"), &old, "x", false).unwrap_err();
    assert_eq!(
        tool_error_kind(&error),
        "workspace_policy_blocked",
        "{error}"
    );
    assert!(!error.to_string().contains(CANARY), "{error}");
}

#[cfg(unix)]
#[test]
fn outside_symlinks_are_not_read_or_enumerated() {
    let holder = tempfile::tempdir().unwrap();
    let root = holder.path().join("workspace");
    std::fs::create_dir_all(&root).unwrap();
    let outside_dir = holder.path().join("outside");
    std::fs::create_dir_all(&outside_dir).unwrap();
    std::fs::write(
        outside_dir.join(".env"),
        format!("PRIVATE_TOKEN={CANARY}\n"),
    )
    .unwrap();
    std::fs::write(
        outside_dir.join("plain.dat"),
        format!("PRIVATE_TOKEN={CANARY}\n"),
    )
    .unwrap();
    std::os::unix::fs::symlink(outside_dir.join(".env"), root.join("outside.txt")).unwrap();
    std::os::unix::fs::symlink(&outside_dir, root.join("outside_dir")).unwrap();
    std::fs::write(root.join("safe.txt"), "PUBLIC_ONLY\n").unwrap();
    std::os::unix::fs::symlink(root.join("safe.txt"), root.join("link.txt")).unwrap();

    let registry = ToolRegistry::default();
    let ctx = context(
        &root,
        ExecutionMode::Act,
        WorkspacePolicy::NormalTask,
        false,
    );

    for (tool, arguments) in [
        ("Read", json!({"path": "outside.txt"})),
        ("Grep", json!({"pattern": "TOKEN", "glob": "outside.txt"})),
        (
            "Grep",
            json!({"pattern": "TOKEN", "glob": "outside_dir/plain.dat"}),
        ),
        ("Glob", json!({"pattern": "outside.txt"})),
    ] {
        let error = registry.execute(tool, &arguments, &ctx).unwrap_err();
        assert_eq!(
            tool_error_kind(&error),
            "path_confinement_error",
            "{tool} {arguments}: {error}"
        );
    }

    let broad_grep = registry
        .execute("Grep", &json!({"pattern": "PRIVATE_TOKEN"}), &ctx)
        .unwrap();
    assert!(!broad_grep.contains(CANARY), "{broad_grep}");

    let broad_glob = registry
        .execute("Glob", &json!({"pattern": "**/*"}), &ctx)
        .unwrap();
    assert!(!broad_glob.contains("outside.txt"), "{broad_glob}");
    assert!(!broad_glob.contains("outside_dir"), "{broad_glob}");
    assert!(broad_glob.contains("safe.txt"), "{broad_glob}");
    assert!(
        broad_glob.contains("excluded by workspace policy"),
        "{broad_glob}"
    );

    let leaf = read::run(
        &root,
        &root.join("outside.txt"),
        None,
        None,
        WorkspacePolicy::NormalTask,
    )
    .unwrap_err();
    assert_eq!(tool_error_kind(&leaf), "path_confinement_error", "{leaf}");

    let inward = registry
        .execute("Read", &json!({"path": "link.txt"}), &ctx)
        .unwrap();
    assert!(inward.contains("PUBLIC_ONLY"), "{inward}");
}

#[cfg(unix)]
#[test]
fn explicit_compound_and_alias_globs_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    for (dir_path, file) in [
        (".aws", "credentials"),
        (".docker", "config.json"),
        (".config/gcloud", "application_default_credentials.json"),
    ] {
        std::fs::create_dir_all(root.join(dir_path)).unwrap();
        std::fs::write(root.join(dir_path).join(file), "x\n").unwrap();
    }
    write_canary(root);
    std::os::unix::fs::symlink(root.join(".env"), root.join("notes.txt")).unwrap();
    std::fs::write(root.join(".env.example"), "PLACEHOLDER=1\n").unwrap();

    let registry = ToolRegistry::default();
    let ctx = context(root, ExecutionMode::Act, WorkspacePolicy::NormalTask, true);

    for pattern in [
        ".aws/credentials",
        ".docker/config.json",
        ".config/gcloud/application_default_credentials.json",
        "notes.txt",
    ] {
        let glob_error = registry
            .execute("Glob", &json!({"pattern": pattern}), &ctx)
            .unwrap_err();
        assert_eq!(
            tool_error_kind(&glob_error),
            "workspace_policy_blocked",
            "Glob {pattern}: {glob_error}"
        );
        let grep_error = registry
            .execute("Grep", &json!({"pattern": "x", "glob": pattern}), &ctx)
            .unwrap_err();
        assert_eq!(
            tool_error_kind(&grep_error),
            "workspace_policy_blocked",
            "Grep {pattern}: {grep_error}"
        );
    }

    let template = registry
        .execute("Glob", &json!({"pattern": ".env.example"}), &ctx)
        .unwrap();
    assert!(template.contains(".env.example"), "{template}");

    let broad = registry
        .execute("Glob", &json!({"pattern": "**/*"}), &ctx)
        .unwrap();
    assert!(broad.contains(".env.example"), "{broad}");
    assert!(!broad.lines().any(|line| line == ".env"), "{broad}");
    assert!(!broad.contains("notes.txt"), "{broad}");
    assert!(broad.contains("excluded by workspace policy"), "{broad}");
}

#[cfg(unix)]
#[test]
fn bash_broad_recursive_grep_uses_shared_rules() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    for (dir_path, file) in [
        (".", ".ENV"),
        (".", ".Env.local"),
        (".", ".envrc"),
        (".aws", "credentials"),
        (".docker", "config.json"),
        (".config/gcloud", "application_default_credentials.json"),
        (".ssh", "id_ed25519"),
        (".", ".env"),
    ] {
        std::fs::create_dir_all(root.join(dir_path)).unwrap();
        std::fs::write(
            root.join(dir_path).join(file),
            format!("PRIVATE_TOKEN={CANARY}\n"),
        )
        .unwrap();
    }
    std::fs::write(root.join(".env.example"), "PUBLIC_TEMPLATE=1\n").unwrap();

    let registry = ToolRegistry::default();
    let ctx = context(root, ExecutionMode::Act, WorkspacePolicy::NormalTask, true);

    let secret = registry
        .execute("Bash", &json!({"command": "grep -r PRIVATE_TOKEN"}), &ctx)
        .unwrap();
    assert!(!secret.contains(CANARY), "{secret}");
    assert!(!secret.contains(".ENV"), "{secret}");
    assert!(!secret.contains(".envrc"), "{secret}");

    let template = registry
        .execute("Bash", &json!({"command": "grep -r PUBLIC_TEMPLATE"}), &ctx)
        .unwrap();
    assert!(template.contains(".env.example"), "{template}");
}

#[cfg(unix)]
#[test]
fn bash_appended_secret_after_generated_prefix_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write_canary(root);
    let normalized = bash::normalize_inspect_command("ls -R", root)
        .unwrap()
        .normalized;
    let command = format!("{normalized}; head -n 1 .env");
    let error = bash::run(&command, root, true).unwrap_err();
    assert!(
        error.to_string().contains("workspace_policy_blocked"),
        "{error}"
    );
    assert!(!error.to_string().contains(CANARY), "{error}");
}
