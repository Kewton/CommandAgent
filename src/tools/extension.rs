use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, bail};
use serde_json::{Map, Value, json};

use super::path_guard::{normalize_workspace_path, resolve_existing};
use super::registry::{FunctionSpec, ToolContext, ToolSpec};
use super::workspace_policy::ensure_tool_path_allowed;

pub const MAX_EXTENSION_RESULT_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtensionArgumentKind {
    String,
    Integer,
    Boolean,
    WorkspacePath,
}

impl ExtensionArgumentKind {
    fn json_type(self) -> &'static str {
        match self {
            Self::String | Self::WorkspacePath => "string",
            Self::Integer => "integer",
            Self::Boolean => "boolean",
        }
    }

    fn accepts(self, value: &Value) -> bool {
        match self {
            Self::String | Self::WorkspacePath => value.is_string(),
            Self::Integer => value.as_i64().is_some() || value.as_u64().is_some(),
            Self::Boolean => value.is_boolean(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionArgument {
    name: String,
    description: String,
    kind: ExtensionArgumentKind,
    required: bool,
}

impl ExtensionArgument {
    pub fn required(
        name: impl Into<String>,
        kind: ExtensionArgumentKind,
        description: impl Into<String>,
    ) -> Self {
        Self::new(name, kind, description, true)
    }

    pub fn optional(
        name: impl Into<String>,
        kind: ExtensionArgumentKind,
        description: impl Into<String>,
    ) -> Self {
        Self::new(name, kind, description, false)
    }

    fn new(
        name: impl Into<String>,
        kind: ExtensionArgumentKind,
        description: impl Into<String>,
        required: bool,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            kind,
            required,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtensionNetworkAccess {
    None,
    Required,
}

impl ExtensionNetworkAccess {
    const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Required => "required",
        }
    }
}

pub(crate) trait ExtensionExecutor: fmt::Debug + Send + Sync {
    fn transport(&self) -> &'static str;
    fn execute(&self, workspace_root: &Path, arguments: &Value) -> anyhow::Result<String>;
}

#[derive(Debug, Clone)]
pub struct ExtensionTool {
    spec: ToolSpec,
    arguments: BTreeMap<String, ExtensionArgument>,
    network_access: ExtensionNetworkAccess,
    executor: Arc<dyn ExtensionExecutor>,
}

impl ExtensionTool {
    pub(crate) fn read_only(
        name: String,
        description: String,
        arguments: Vec<ExtensionArgument>,
        network_access: ExtensionNetworkAccess,
        executor: Arc<dyn ExtensionExecutor>,
    ) -> anyhow::Result<Self> {
        validate_identifier("extension tool", &name)?;
        if description.trim().is_empty() {
            bail!("extension tool `{name}` requires a description");
        }
        let mut by_name = BTreeMap::new();
        for argument in arguments {
            validate_identifier("extension argument", &argument.name)?;
            if argument.description.trim().is_empty() {
                bail!(
                    "extension argument `{}` requires a description",
                    argument.name
                );
            }
            let argument_name = argument.name.clone();
            if by_name.insert(argument_name.clone(), argument).is_some() {
                bail!("duplicate extension argument `{argument_name}`");
            }
        }
        let parameters = argument_schema(&by_name);
        Ok(Self {
            spec: ToolSpec {
                kind: "function".to_string(),
                function: FunctionSpec {
                    name,
                    description,
                    parameters,
                },
            },
            arguments: by_name,
            network_access,
            executor,
        })
    }

    pub fn name(&self) -> &str {
        &self.spec.function.name
    }

    pub fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    pub(crate) fn execute(
        &self,
        arguments: &Value,
        context: &ToolContext,
    ) -> anyhow::Result<String> {
        emit_call(context, self, arguments);
        let prepared = match self.prepare(arguments, context) {
            Ok(prepared) => prepared,
            Err(error) => {
                emit_rejection(context, self, arguments, &error);
                bail!("extension_tool_rejected: `{}`: {error}", self.name());
            }
        };
        match self.executor.execute(&context.root, &prepared) {
            Ok(result) if result.len() <= MAX_EXTENSION_RESULT_BYTES => {
                emit_result(context, self, true, Some(result.len()), None);
                Ok(result)
            }
            Ok(result) => {
                let error = anyhow::anyhow!(
                    "extension result exceeds {MAX_EXTENSION_RESULT_BYTES} byte limit ({} bytes)",
                    result.len()
                );
                emit_result(context, self, false, Some(result.len()), Some(&error));
                Err(error)
            }
            Err(error) => {
                emit_result(context, self, false, None, Some(&error));
                Err(error).with_context(|| format!("extension tool `{}` failed", self.name()))
            }
        }
    }

    fn prepare(&self, arguments: &Value, context: &ToolContext) -> anyhow::Result<Value> {
        super::allow_policy::authorize_current(
            "Read",
            arguments,
            &context.root,
            context.auto_approve,
            context.interactive_approval,
        )?;
        if context.offline && self.network_access == ExtensionNetworkAccess::Required {
            bail!(
                "offline_policy_blocked: extension tool `{}` requires network access",
                self.name()
            );
        }
        let supplied = arguments
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("extension arguments must be an object"))?;
        let known = self.arguments.keys().cloned().collect::<BTreeSet<_>>();
        let unknown = supplied
            .keys()
            .filter(|name| !known.contains(*name))
            .cloned()
            .collect::<Vec<_>>();
        if !unknown.is_empty() {
            bail!("unknown extension arguments: {}", unknown.join(", "));
        }

        let mut prepared = Map::new();
        for (name, declaration) in &self.arguments {
            let Some(value) = supplied.get(name) else {
                if declaration.required {
                    bail!("missing extension argument `{name}`");
                }
                continue;
            };
            if !declaration.kind.accepts(value) {
                bail!(
                    "extension argument `{name}` must be {}",
                    declaration.kind.json_type()
                );
            }
            let value = if declaration.kind == ExtensionArgumentKind::WorkspacePath {
                Value::String(normalize_existing_workspace_path(
                    context,
                    value.as_str().expect("workspace path type checked"),
                )?)
            } else {
                value.clone()
            };
            prepared.insert(name.clone(), value);
        }
        Ok(Value::Object(prepared))
    }
}

fn validate_identifier(kind: &str, value: &str) -> anyhow::Result<()> {
    let mut characters = value.chars();
    let valid_first = characters
        .next()
        .is_some_and(|character| character.is_ascii_alphabetic() || character == '_');
    let valid_rest = characters
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.'));
    if !valid_first || !valid_rest || value.len() > 64 {
        bail!(
            "{kind} name `{value}` must be a 1-64 character ASCII identifier beginning with a letter or underscore"
        );
    }
    Ok(())
}

fn argument_schema(arguments: &BTreeMap<String, ExtensionArgument>) -> Value {
    let properties = arguments
        .iter()
        .map(|(name, argument)| {
            (
                name.clone(),
                json!({
                    "type": argument.kind.json_type(),
                    "description": argument.description,
                }),
            )
        })
        .collect::<Map<_, _>>();
    let required = arguments
        .iter()
        .filter(|(_, argument)| argument.required)
        .map(|(name, _)| Value::String(name.clone()))
        .collect::<Vec<_>>();
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false,
    })
}

fn normalize_existing_workspace_path(context: &ToolContext, raw: &str) -> anyhow::Result<String> {
    let normalized = normalize_workspace_path(&context.root, raw)?
        .map(|normalization| normalization.relative)
        .unwrap_or_else(|| raw.to_string());
    super::hidden_path::ensure_reference_allowed(
        &context.root,
        &normalized,
        context.workspace_policy,
    )?;
    let resolved = resolve_existing(&context.root, &normalized)?;
    ensure_tool_path_allowed(&context.root, &resolved, context.workspace_policy)?;
    Ok(normalized)
}

fn emit_call(context: &ToolContext, tool: &ExtensionTool, arguments: &Value) {
    crate::eval_events::emit(
        context.eval_events_path.as_deref(),
        json!({
            "event": "extension_tool_call",
            "name": tool.name(),
            "transport": tool.executor.transport(),
            "access": "read_only",
            "network_access": tool.network_access.as_str(),
            "arguments": crate::eval_events::argument_shape(arguments),
        }),
    );
}

fn emit_rejection(
    context: &ToolContext,
    tool: &ExtensionTool,
    arguments: &Value,
    error: &anyhow::Error,
) {
    crate::eval_events::emit(
        context.eval_events_path.as_deref(),
        json!({
            "event": "extension_tool_rejected",
            "name": tool.name(),
            "transport": tool.executor.transport(),
            "access": "read_only",
            "policy": rejection_policy(error),
            "reason": crate::eval_events::body_snippet(&error.to_string()),
            "arguments": crate::eval_events::argument_shape(arguments),
        }),
    );
}

fn emit_result(
    context: &ToolContext,
    tool: &ExtensionTool,
    ok: bool,
    result_bytes: Option<usize>,
    error: Option<&anyhow::Error>,
) {
    crate::eval_events::emit(
        context.eval_events_path.as_deref(),
        json!({
            "event": "extension_tool_result",
            "name": tool.name(),
            "transport": tool.executor.transport(),
            "access": "read_only",
            "ok": ok,
            "result_bytes": result_bytes,
            "reason": error.map(|error| crate::eval_events::body_snippet(&error.to_string())),
        }),
    );
}

fn rejection_policy(error: &anyhow::Error) -> &'static str {
    let message = error.to_string();
    if message.contains("not permitted by --allow") {
        "allow"
    } else if message.contains("offline_policy_blocked") {
        "offline"
    } else if message.contains("path") || message.contains("workspace_policy_blocked") {
        "workspace"
    } else {
        "arguments"
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Mutex;

    use serde_json::json;

    use super::*;
    use crate::mode::ExecutionMode;
    use crate::tools::allow_policy::{AllowTarget, install};
    use crate::tools::mcp::{McpClient, McpToolCall, read_only_tool};
    use crate::tools::registry::ToolRegistry;
    use crate::tools::workspace_policy::WorkspacePolicy;

    #[derive(Debug, Clone, PartialEq)]
    struct RecordedCall {
        server_id: String,
        tool_name: String,
        workspace_root: PathBuf,
        arguments: Value,
    }

    #[derive(Debug)]
    struct RecordingClient {
        calls: Mutex<Vec<RecordedCall>>,
        result: String,
    }

    impl RecordingClient {
        fn new(result: impl Into<String>) -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
                result: result.into(),
            }
        }

        fn calls(&self) -> Vec<RecordedCall> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl McpClient for RecordingClient {
        fn call_read_only_tool(&self, call: McpToolCall<'_>) -> anyhow::Result<String> {
            self.calls.lock().unwrap().push(RecordedCall {
                server_id: call.server_id.to_string(),
                tool_name: call.tool_name.to_string(),
                workspace_root: call.workspace_root.to_path_buf(),
                arguments: call.arguments.clone(),
            });
            Ok(self.result.clone())
        }
    }

    fn tool(client: Arc<RecordingClient>, network_access: ExtensionNetworkAccess) -> ExtensionTool {
        let client: Arc<dyn McpClient> = client;
        read_only_tool(
            "InspectAsset",
            "Inspect one workspace asset without modifying it.",
            "assets",
            "inspect_asset",
            vec![
                ExtensionArgument::required(
                    "path",
                    ExtensionArgumentKind::WorkspacePath,
                    "Existing workspace-relative asset path.",
                ),
                ExtensionArgument::optional(
                    "details",
                    ExtensionArgumentKind::Boolean,
                    "Whether to return extra read-only detail.",
                ),
            ],
            network_access,
            client,
        )
        .unwrap()
    }

    fn context(root: &Path, events: &Path) -> ToolContext {
        ToolContext {
            root: root.to_path_buf(),
            mode: ExecutionMode::Plan,
            auto_approve: false,
            interactive_approval: false,
            offline: false,
            workspace_policy: WorkspacePolicy::NormalTask,
            eval_events_path: Some(events.to_path_buf()),
            expected_paths: Vec::new(),
            protected_paths: Vec::new(),
        }
    }

    fn events(path: &Path) -> Vec<Value> {
        std::fs::read_to_string(path)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    #[test]
    fn registered_read_only_mcp_tool_projects_schema_and_dispatches_normalized_path() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("data")).unwrap();
        let input = root.path().join("data/input.txt");
        std::fs::write(&input, "sample").unwrap();
        let events_path = root.path().join("events.jsonl");
        let client = Arc::new(RecordingClient::new("inspection result"));
        let registry =
            ToolRegistry::with_extensions([tool(client.clone(), ExtensionNetworkAccess::None)])
                .unwrap();

        let spec = registry
            .specs()
            .iter()
            .find(|spec| spec.function.name == "InspectAsset")
            .unwrap();
        assert_eq!(spec.function.parameters["additionalProperties"], false);
        assert_eq!(
            spec.function.parameters["properties"]["path"]["type"],
            "string"
        );
        assert_eq!(spec.function.parameters["required"], json!(["path"]));

        let output = registry
            .execute(
                "InspectAsset",
                &json!({"path": input, "details": true}),
                &context(root.path(), &events_path),
            )
            .unwrap();

        assert_eq!(output, "inspection result");
        assert_eq!(
            client.calls(),
            [RecordedCall {
                server_id: "assets".to_string(),
                tool_name: "inspect_asset".to_string(),
                workspace_root: root.path().to_path_buf(),
                arguments: json!({"path":"data/input.txt", "details":true}),
            }]
        );
        let recorded = events(&events_path);
        assert!(recorded.iter().any(|event| {
            event["event"] == "extension_tool_call"
                && event["name"] == "InspectAsset"
                && event["transport"] == "mcp"
        }));
        assert!(recorded.iter().any(|event| {
            event["event"] == "extension_tool_result"
                && event["ok"] == true
                && event["result_bytes"] == 17
        }));
        assert!(
            recorded
                .iter()
                .all(|event| event["event"] != "extension_tool_rejected")
        );
    }

    #[test]
    fn allow_offline_and_workspace_policies_reject_before_dispatch() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("input.txt"), "sample").unwrap();
        let events_path = root.path().join("events.jsonl");
        let client = Arc::new(RecordingClient::new("unused"));
        let local =
            ToolRegistry::with_extensions([tool(client.clone(), ExtensionNetworkAccess::None)])
                .unwrap();

        {
            let _policy = install(false, &[AllowTarget::Write]);
            let error = local
                .execute(
                    "InspectAsset",
                    &json!({"path":"input.txt"}),
                    &context(root.path(), &events_path),
                )
                .unwrap_err();
            assert!(error.to_string().contains("not permitted by --allow write"));
        }

        let network =
            ToolRegistry::with_extensions([tool(client.clone(), ExtensionNetworkAccess::Required)])
                .unwrap();
        let mut offline = context(root.path(), &events_path);
        offline.offline = true;
        let error = network
            .execute("InspectAsset", &json!({"path":"input.txt"}), &offline)
            .unwrap_err();
        assert!(error.to_string().contains("offline_policy_blocked"));

        let error = local
            .execute(
                "InspectAsset",
                &json!({"path":"../outside.txt"}),
                &context(root.path(), &events_path),
            )
            .unwrap_err();
        assert!(error.to_string().contains("path may not contain .."));
        assert!(client.calls().is_empty());

        let policies = events(&events_path)
            .into_iter()
            .filter(|event| event["event"] == "extension_tool_rejected")
            .map(|event| event["policy"].as_str().unwrap().to_string())
            .collect::<Vec<_>>();
        assert_eq!(policies, ["allow", "offline", "workspace"]);
    }

    #[test]
    fn closed_arguments_hidden_paths_and_oversized_results_fail_honestly() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join(".anvil")).unwrap();
        std::fs::write(root.path().join(".anvil/private.txt"), "secret").unwrap();
        std::fs::write(root.path().join("input.txt"), "sample").unwrap();
        let events_path = root.path().join("events.jsonl");
        let client = Arc::new(RecordingClient::new("unused"));
        let registry =
            ToolRegistry::with_extensions([tool(client.clone(), ExtensionNetworkAccess::None)])
                .unwrap();

        let unknown = registry
            .execute(
                "InspectAsset",
                &json!({"path":"input.txt", "extra":"no"}),
                &context(root.path(), &events_path),
            )
            .unwrap_err();
        assert!(unknown.to_string().contains("unknown extension arguments"));
        let hidden = registry
            .execute(
                "InspectAsset",
                &json!({"path":".anvil/private.txt"}),
                &context(root.path(), &events_path),
            )
            .unwrap_err();
        assert!(hidden.to_string().contains("workspace_policy_blocked"));
        assert!(client.calls().is_empty());

        let large_client = Arc::new(RecordingClient::new(
            "x".repeat(MAX_EXTENSION_RESULT_BYTES + 1),
        ));
        let large_registry = ToolRegistry::with_extensions([tool(
            large_client.clone(),
            ExtensionNetworkAccess::None,
        )])
        .unwrap();
        let oversized = large_registry
            .execute(
                "InspectAsset",
                &json!({"path":"input.txt"}),
                &context(root.path(), &events_path),
            )
            .unwrap_err();
        assert!(oversized.to_string().contains("exceeds 65536 byte limit"));
        assert_eq!(large_client.calls().len(), 1);
        assert!(events(&events_path).iter().any(|event| {
            event["event"] == "extension_tool_result"
                && event["ok"] == false
                && event["result_bytes"] == (MAX_EXTENSION_RESULT_BYTES + 1)
        }));
    }

    #[test]
    fn declaration_and_registry_reject_ambiguous_names() {
        let client = Arc::new(RecordingClient::new("unused"));
        let client_trait: Arc<dyn McpClient> = client.clone();
        let duplicate_arguments = read_only_tool(
            "InspectAsset",
            "description",
            "assets",
            "inspect",
            vec![
                ExtensionArgument::required("path", ExtensionArgumentKind::String, "first"),
                ExtensionArgument::optional("path", ExtensionArgumentKind::Boolean, "second"),
            ],
            ExtensionNetworkAccess::None,
            client_trait,
        )
        .unwrap_err();
        assert!(
            duplicate_arguments
                .to_string()
                .contains("duplicate extension argument `path`")
        );

        let built_in_collision =
            ToolRegistry::with_extensions([tool(client.clone(), ExtensionNetworkAccess::None)])
                .unwrap();
        let duplicate = built_in_collision
            .clone()
            .register_extension(tool(client.clone(), ExtensionNetworkAccess::None))
            .unwrap_err();
        assert!(duplicate.to_string().contains("duplicate tool name"));

        let client_trait: Arc<dyn McpClient> = client;
        let read_collision = read_only_tool(
            "Read",
            "collision",
            "assets",
            "read",
            Vec::new(),
            ExtensionNetworkAccess::None,
            client_trait,
        )
        .unwrap();
        let error = ToolRegistry::with_extensions([read_collision]).unwrap_err();
        assert!(error.to_string().contains("duplicate tool name: Read"));
    }
}
