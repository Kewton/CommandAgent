use super::result::{MinimalLoopError, ToolExecutionRecord};
use crate::providers::ToolCall;
use crate::safety::path_guard::PathGuard;
use crate::tools::bash::BashTool;
use crate::tools::edit::EditTool;
use crate::tools::glob::{GlobTool, SearchOptions};
use crate::tools::grep::GrepTool;
use crate::tools::read::ReadTool;
use crate::tools::write::WriteTool;
use serde_json::Value;

pub(super) struct ToolExecutor<'a> {
    read: ReadTool<'a>,
    write: WriteTool<'a>,
    edit: EditTool<'a>,
    bash: BashTool<'a>,
    glob: GlobTool<'a>,
    grep: GrepTool<'a>,
}

impl<'a> ToolExecutor<'a> {
    pub(super) fn new(guard: &'a PathGuard) -> Self {
        Self {
            read: ReadTool::new(guard),
            write: WriteTool::new(guard),
            edit: EditTool::new(guard),
            bash: BashTool::new(guard),
            glob: GlobTool::new(guard),
            grep: GrepTool::new(guard),
        }
    }

    pub(super) fn execute(&self, call: &ToolCall) -> Result<ToolExecutionRecord, MinimalLoopError> {
        let args: Value = serde_json::from_str(&call.args_json)
            .map_err(|err| MinimalLoopError::ToolArgs(err.to_string()))?;
        let output = match call.name.as_str() {
            "Read" => self
                .read
                .read(required_str(&args, "path")?)
                .map_err(tool_err)?,
            "Write" => {
                self.write
                    .write(
                        required_str(&args, "path")?,
                        required_str(&args, "content")?,
                    )
                    .map_err(tool_err)?;
                "wrote file".to_string()
            }
            "Edit" => {
                self.edit
                    .replace_once(
                        required_str(&args, "path")?,
                        required_str(&args, "old")?,
                        required_str(&args, "new")?,
                    )
                    .map_err(tool_err)?;
                "edited file".to_string()
            }
            "Bash" => {
                let output = self
                    .bash
                    .run(required_str(&args, "command")?)
                    .map_err(tool_err)?;
                format!(
                    "status: {}\nstdout:\n{}\nstderr:\n{}",
                    output.status, output.stdout, output.stderr
                )
            }
            "Glob" => self
                .glob
                .glob(required_str(&args, "pattern")?, SearchOptions::default())
                .map_err(tool_err)?
                .into_iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join("\n"),
            "Grep" => self
                .grep
                .grep(required_str(&args, "pattern")?, SearchOptions::default())
                .map_err(tool_err)?
                .into_iter()
                .map(|m| format!("{}:{}:{}", m.path.display(), m.line_number, m.line))
                .collect::<Vec<_>>()
                .join("\n"),
            other => return Err(MinimalLoopError::Tool(format!("unknown tool: {}", other))),
        };

        Ok(ToolExecutionRecord {
            name: call.name.clone(),
            ok: true,
            output,
        })
    }
}

fn required_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, MinimalLoopError> {
    args.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| MinimalLoopError::ToolArgs(format!("missing string field `{}`", key)))
}

fn tool_err(err: impl std::fmt::Display) -> MinimalLoopError {
    MinimalLoopError::Tool(err.to_string())
}
