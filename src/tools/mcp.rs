use std::fmt;
use std::path::Path;
use std::sync::Arc;

use serde_json::Value;

use super::extension::{
    ExtensionArgument, ExtensionExecutor, ExtensionNetworkAccess, ExtensionTool,
};

pub struct McpToolCall<'a> {
    pub server_id: &'a str,
    pub tool_name: &'a str,
    pub workspace_root: &'a Path,
    pub arguments: &'a Value,
}

impl fmt::Debug for McpToolCall<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("McpToolCall")
            .field("server_id", &self.server_id)
            .field("tool_name", &self.tool_name)
            .field("workspace_root", &self.workspace_root)
            .field(
                "arguments",
                &crate::eval_events::argument_shape(self.arguments),
            )
            .finish()
    }
}

pub trait McpClient: fmt::Debug + Send + Sync {
    fn call_read_only_tool(&self, call: McpToolCall<'_>) -> anyhow::Result<String>;
}

#[allow(clippy::too_many_arguments)]
pub fn read_only_tool(
    name: impl Into<String>,
    description: impl Into<String>,
    server_id: impl Into<String>,
    remote_tool_name: impl Into<String>,
    arguments: Vec<ExtensionArgument>,
    network_access: ExtensionNetworkAccess,
    client: Arc<dyn McpClient>,
) -> anyhow::Result<ExtensionTool> {
    let server_id = server_id.into();
    let remote_tool_name = remote_tool_name.into();
    if server_id.trim().is_empty() {
        anyhow::bail!("read-only MCP tool requires a server ID");
    }
    if remote_tool_name.trim().is_empty() {
        anyhow::bail!("read-only MCP tool requires a remote tool name");
    }
    ExtensionTool::read_only(
        name.into(),
        description.into(),
        arguments,
        network_access,
        Arc::new(McpExecutor {
            server_id,
            remote_tool_name,
            client,
        }),
    )
}

#[derive(Debug)]
struct McpExecutor {
    server_id: String,
    remote_tool_name: String,
    client: Arc<dyn McpClient>,
}

impl ExtensionExecutor for McpExecutor {
    fn transport(&self) -> &'static str {
        "mcp"
    }

    fn execute(&self, workspace_root: &Path, arguments: &Value) -> anyhow::Result<String> {
        self.client.call_read_only_tool(McpToolCall {
            server_id: &self.server_id,
            tool_name: &self.remote_tool_name,
            workspace_root,
            arguments,
        })
    }
}
