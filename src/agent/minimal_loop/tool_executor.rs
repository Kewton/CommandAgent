use super::config::{DependencySetupPolicy, StepToolPolicy};
use super::result::{MinimalLoopError, ToolArgError, ToolExecutionRecord};
use crate::agent::budget::{ToolResultBudget, enforce_tool_result_budget};
use crate::providers::ToolCall;
use crate::safety::path_guard::PathGuard;
use crate::tools::bash::{BashPolicy, BashTool, CommandClass, enforce_bash_policy};
use crate::tools::edit::EditTool;
use crate::tools::glob::{GlobTool, SearchOptions};
use crate::tools::grep::GrepTool;
use crate::tools::read::ReadTool;
use crate::tools::write::WriteTool;
use serde_json::Value;
use std::path::{Component, Path};

pub(super) struct ToolExecutor<'a> {
    guard: &'a PathGuard,
    dependency_policy: DependencySetupPolicy,
    step_tool_policy: StepToolPolicy,
    read: ReadTool<'a>,
    write: WriteTool<'a>,
    edit: EditTool<'a>,
    bash: BashTool<'a>,
    glob: GlobTool<'a>,
    grep: GrepTool<'a>,
}

impl<'a> ToolExecutor<'a> {
    pub(super) fn new(
        guard: &'a PathGuard,
        dependency_policy: DependencySetupPolicy,
        step_tool_policy: StepToolPolicy,
    ) -> Self {
        Self {
            guard,
            dependency_policy,
            step_tool_policy,
            read: ReadTool::new(guard),
            write: WriteTool::new(guard),
            edit: EditTool::new(guard),
            bash: BashTool::with_policy(
                guard,
                BashPolicy::normal_tool_call(dependency_policy.offline),
            ),
            glob: GlobTool::new(guard),
            grep: GrepTool::new(guard),
        }
    }

    pub(super) fn execute(&self, call: &ToolCall) -> Result<ToolExecutionRecord, MinimalLoopError> {
        let args: Value =
            serde_json::from_str(&call.args_json).map_err(|err| invalid_json(call, err))?;
        self.enforce_step_tool_policy(call.name.as_str(), &args)?;
        let mut target_paths = Vec::new();
        let output = match call.name.as_str() {
            "Read" => self
                .read
                .read(required_str_for_tool(&args, "Read", "path")?)
                .map_err(tool_err)?,
            "Write" => {
                let path = required_str_for_tool(&args, "Write", "path")?;
                self.write
                    .write(path, required_str_for_tool(&args, "Write", "content")?)
                    .map_err(tool_err)?;
                target_paths.push(normalize_tool_path(path));
                "wrote file".to_string()
            }
            "Edit" => {
                let path = required_str_for_tool(&args, "Edit", "path")?;
                self.edit
                    .replace_once(
                        path,
                        required_str_for_tool(&args, "Edit", "old")?,
                        required_str_for_tool(&args, "Edit", "new")?,
                    )
                    .map_err(tool_err)?;
                target_paths.push(normalize_tool_path(path));
                "edited file".to_string()
            }
            "Bash" => {
                let output = self
                    .bash
                    .run(required_str_for_tool(&args, "Bash", "command")?)
                    .map_err(tool_err)?;
                format!(
                    "status: {}\nstdout:\n{}\nstderr:\n{}",
                    output.status, output.stdout, output.stderr
                )
            }
            "Glob" => self
                .glob
                .glob(
                    required_str_for_tool(&args, "Glob", "pattern")?,
                    SearchOptions::default(),
                )
                .map_err(tool_err)?
                .into_iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join("\n"),
            "Grep" => self
                .grep
                .grep(
                    required_str_for_tool(&args, "Grep", "pattern")?,
                    SearchOptions::default(),
                )
                .map_err(tool_err)?
                .into_iter()
                .map(|m| format!("{}:{}:{}", m.path.display(), m.line_number, m.line))
                .collect::<Vec<_>>()
                .join("\n"),
            other => return Err(MinimalLoopError::Tool(format!("unknown tool: {}", other))),
        };

        let (output, truncation) = enforce_tool_result_budget(output, ToolResultBudget::default());

        Ok(ToolExecutionRecord {
            name: call.name.clone(),
            ok: true,
            output,
            output_truncated: truncation.truncated,
            original_output_chars: truncation.original_chars,
            target_paths,
        })
    }

    fn enforce_step_tool_policy(
        &self,
        tool_name: &str,
        args: &Value,
    ) -> Result<(), MinimalLoopError> {
        match self.step_tool_policy {
            StepToolPolicy::FileMutationAllowed => Ok(()),
            StepToolPolicy::ReadOnly => self.enforce_read_only(tool_name, args),
            StepToolPolicy::NoMutation => {
                if matches!(tool_name, "Write" | "Edit") {
                    return Err(policy_violation(format!(
                        "{tool_name} is not allowed in a no-mutation step"
                    )));
                }
                Ok(())
            }
            StepToolPolicy::SetupMutationOnly => self.enforce_setup_mutation_only(tool_name, args),
        }
    }

    fn enforce_read_only(&self, tool_name: &str, args: &Value) -> Result<(), MinimalLoopError> {
        match tool_name {
            "Read" | "Glob" | "Grep" => Ok(()),
            "Bash" => {
                let command = required_str_for_tool(args, "Bash", "command")?;
                let decision = enforce_bash_policy(
                    command,
                    self.guard.root(),
                    BashPolicy::normal_tool_call(self.dependency_policy.offline),
                );
                if decision.allowed && decision.class == CommandClass::ReadOnly {
                    Ok(())
                } else {
                    Err(policy_violation(format!(
                        "Bash command is not read-only for this step: {}",
                        decision
                            .message
                            .unwrap_or_else(|| format!("{:?}", decision.class))
                    )))
                }
            }
            "Write" | "Edit" => Err(policy_violation(format!(
                "{tool_name} is not allowed in a read-only step"
            ))),
            _ => Ok(()),
        }
    }

    fn enforce_setup_mutation_only(
        &self,
        tool_name: &str,
        args: &Value,
    ) -> Result<(), MinimalLoopError> {
        match tool_name {
            "Read" | "Glob" | "Grep" => Ok(()),
            "Bash" => self.enforce_read_only(tool_name, args),
            "Write" | "Edit" => {
                let path = required_str_for_tool(args, tool_name, "path")?;
                if is_setup_or_config_path(Path::new(path)) {
                    Ok(())
                } else {
                    Err(policy_violation(format!(
                        "{tool_name} may only change setup/config files in a setup step: {path}"
                    )))
                }
            }
            _ => Ok(()),
        }
    }
}

fn required_str_for_tool<'a>(
    args: &'a Value,
    tool_name: &str,
    key: &str,
) -> Result<&'a str, MinimalLoopError> {
    args.get(key).and_then(Value::as_str).ok_or_else(|| {
        MinimalLoopError::ToolArgs(ToolArgError::MissingRequiredStringField {
            tool: tool_name.to_string(),
            field: key.to_string(),
            required_fields: required_fields_for_tool(tool_name)
                .iter()
                .map(|field| (*field).to_string())
                .collect(),
        })
    })
}

fn invalid_json(call: &ToolCall, err: serde_json::Error) -> MinimalLoopError {
    MinimalLoopError::ToolArgs(ToolArgError::InvalidJson {
        tool: call.name.clone(),
        message: err.to_string(),
    })
}

fn required_fields_for_tool(tool_name: &str) -> &'static [&'static str] {
    match tool_name {
        "Read" => &["path"],
        "Write" => &["path", "content"],
        "Edit" => &["path", "old", "new"],
        "Bash" => &["command"],
        "Glob" => &["pattern"],
        "Grep" => &["pattern"],
        _ => &[],
    }
}

fn tool_err(err: impl std::fmt::Display) -> MinimalLoopError {
    MinimalLoopError::Tool(err.to_string())
}

fn policy_violation(message: String) -> MinimalLoopError {
    MinimalLoopError::Tool(format!("tool_policy_violation: {message}"))
}

fn normalize_tool_path(path: &str) -> String {
    path.trim().trim_start_matches("./").replace('\\', "/")
}

fn is_setup_or_config_path(path: &Path) -> bool {
    if path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        return false;
    }
    let components = path.components().collect::<Vec<_>>();
    if components.len() != 1 {
        return false;
    }
    let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    let lower = file_name.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "package.json"
            | "package-lock.json"
            | "pnpm-lock.yaml"
            | "yarn.lock"
            | "tsconfig.json"
            | "jsconfig.json"
            | "cargo.toml"
            | "cargo.lock"
            | "pyproject.toml"
            | "requirements.txt"
            | "requirements-dev.txt"
            | "vite.config.js"
            | "vite.config.ts"
            | "vite.config.mjs"
            | "next.config.js"
            | "next.config.mjs"
            | "next.config.ts"
            | "postcss.config.js"
            | "postcss.config.cjs"
            | "postcss.config.mjs"
            | "tailwind.config.js"
            | "tailwind.config.cjs"
            | "tailwind.config.mjs"
            | "tailwind.config.ts"
            | "eslint.config.js"
            | "eslint.config.mjs"
            | ".eslintrc"
            | ".eslintrc.json"
            | ".prettierrc"
            | ".prettierrc.json"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::ToolCall;
    use crate::safety::path_guard::PathGuard;
    use serde_json::json;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn read_only_policy_blocks_write() {
        let root = temp_workspace("read-only-write");
        let guard = PathGuard::new(&root).unwrap();
        let executor = ToolExecutor::new(
            &guard,
            DependencySetupPolicy::default(),
            StepToolPolicy::ReadOnly,
        );

        let err = executor
            .execute(&call("Write", json!({"path":"README.md","content":"x"})))
            .unwrap_err();

        assert!(err.to_string().contains("tool_policy_violation"));
        assert!(!root.join("README.md").exists());
    }

    #[test]
    fn read_only_policy_allows_read() {
        let root = temp_workspace("read-only-read");
        fs::write(root.join("README.md"), "hello").unwrap();
        let guard = PathGuard::new(&root).unwrap();
        let executor = ToolExecutor::new(
            &guard,
            DependencySetupPolicy::default(),
            StepToolPolicy::ReadOnly,
        );

        let result = executor
            .execute(&call("Read", json!({"path":"README.md"})))
            .unwrap();

        assert!(result.output.contains("hello"));
    }

    #[test]
    fn no_mutation_policy_blocks_write() {
        let root = temp_workspace("no-mutation-write");
        let guard = PathGuard::new(&root).unwrap();
        let executor = ToolExecutor::new(
            &guard,
            DependencySetupPolicy::default(),
            StepToolPolicy::NoMutation,
        );

        let err = executor
            .execute(&call("Write", json!({"path":"README.md","content":"x"})))
            .unwrap_err();

        assert!(err.to_string().contains("no-mutation"));
        assert!(!root.join("README.md").exists());
    }

    #[test]
    fn setup_policy_allows_manifest_write_but_blocks_source_write() {
        let root = temp_workspace("setup-policy");
        let guard = PathGuard::new(&root).unwrap();
        let executor = ToolExecutor::new(
            &guard,
            DependencySetupPolicy::default(),
            StepToolPolicy::SetupMutationOnly,
        );

        executor
            .execute(&call(
                "Write",
                json!({"path":"package.json","content":"{}"}),
            ))
            .unwrap();
        let err = executor
            .execute(&call(
                "Write",
                json!({"path":"app/page.tsx","content":"export default function Page() { return null }"}),
            ))
            .unwrap_err();

        assert!(root.join("package.json").exists());
        assert!(err.to_string().contains("tool_policy_violation"));
        assert!(!root.join("app/page.tsx").exists());
    }

    #[test]
    fn write_records_exact_changed_path() {
        let root = temp_workspace("write-target-path");
        let guard = PathGuard::new(&root).unwrap();
        let executor = ToolExecutor::new(
            &guard,
            DependencySetupPolicy::default(),
            StepToolPolicy::FileMutationAllowed,
        );

        let result = executor
            .execute(&call(
                "Write",
                json!({"path":"./src/main.rs","content":"fn main() {}\n"}),
            ))
            .unwrap();

        assert_eq!(result.target_paths, vec!["src/main.rs"]);
    }

    #[test]
    fn edit_records_exact_changed_path() {
        let root = temp_workspace("edit-target-path");
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/main.rs"), "fn main() {}\n").unwrap();
        let guard = PathGuard::new(&root).unwrap();
        let executor = ToolExecutor::new(
            &guard,
            DependencySetupPolicy::default(),
            StepToolPolicy::FileMutationAllowed,
        );

        let result = executor
            .execute(&call(
                "Edit",
                json!({"path":"src/main.rs","old":"fn main() {}","new":"fn main() { println!(\"ok\"); }"}),
            ))
            .unwrap();

        assert_eq!(result.target_paths, vec!["src/main.rs"]);
    }

    #[test]
    fn write_missing_path_reports_tool_arg_schema_failure() {
        let root = temp_workspace("missing-write-path");
        let guard = PathGuard::new(&root).unwrap();
        let executor = ToolExecutor::new(
            &guard,
            DependencySetupPolicy::default(),
            StepToolPolicy::FileMutationAllowed,
        );

        let err = executor
            .execute(&call("Write", json!({"content":"x"})))
            .unwrap_err();

        let MinimalLoopError::ToolArgs(arg_error) = err else {
            panic!("expected ToolArgs, got {err:?}");
        };
        assert_eq!(arg_error.reason_code(), "tool_args_missing_required_field");
        assert_eq!(arg_error.tool_name(), "Write");
        assert_eq!(arg_error.missing_field(), Some("path"));
        assert_eq!(arg_error.required_fields().len(), 2);
        assert_eq!(arg_error.required_fields()[0], "path");
        assert_eq!(arg_error.required_fields()[1], "content");
        assert!(
            arg_error
                .to_string()
                .contains("required fields: path, content")
        );
    }

    #[test]
    fn invalid_json_reports_tool_arg_schema_failure() {
        let root = temp_workspace("invalid-json");
        let guard = PathGuard::new(&root).unwrap();
        let executor = ToolExecutor::new(
            &guard,
            DependencySetupPolicy::default(),
            StepToolPolicy::FileMutationAllowed,
        );

        let err = executor
            .execute(&ToolCall {
                id: None,
                thought_signature: None,
                name: "Write".to_string(),
                args_json: "{".to_string(),
            })
            .unwrap_err();

        let MinimalLoopError::ToolArgs(arg_error) = err else {
            panic!("expected ToolArgs, got {err:?}");
        };
        assert_eq!(arg_error.reason_code(), "tool_args_invalid_json");
        assert_eq!(arg_error.tool_name(), "Write");
        assert!(
            arg_error
                .to_string()
                .contains("arguments are not valid JSON")
        );
    }

    #[test]
    fn setup_policy_violation_stays_tool_error() {
        let root = temp_workspace("setup-policy-is-not-schema");
        let guard = PathGuard::new(&root).unwrap();
        let executor = ToolExecutor::new(
            &guard,
            DependencySetupPolicy::default(),
            StepToolPolicy::SetupMutationOnly,
        );

        let err = executor
            .execute(&call(
                "Write",
                json!({"path":"src/main.rs","content":"fn main() {}"}),
            ))
            .unwrap_err();

        assert!(matches!(err, MinimalLoopError::Tool(_)));
        assert!(err.to_string().contains("tool_policy_violation"));
    }

    #[test]
    fn required_fields_metadata_matches_current_tool_schema() {
        assert_eq!(required_fields_for_tool("Write"), ["path", "content"]);
        assert_eq!(required_fields_for_tool("Edit"), ["path", "old", "new"]);
        assert_eq!(required_fields_for_tool("Bash"), ["command"]);
    }

    fn call(name: &str, args: serde_json::Value) -> ToolCall {
        ToolCall {
            id: None,
            thought_signature: None,
            name: name.to_string(),
            args_json: serde_json::to_string(&args).unwrap(),
        }
    }

    fn temp_workspace(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "commandagent-tool-executor-{}-{}",
            name,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }
}
