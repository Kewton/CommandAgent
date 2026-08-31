use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

use anyhow::bail;
use serde::Serialize;
use serde_json::{Value, json};

use crate::eval_events;
use crate::mode::ExecutionMode;

use super::args_recovery::recover_tool_arguments;
use super::path_guard::{
    WorkspacePathNormalizationKind, normalize_absolute_workspace_glob, normalize_workspace_path,
    resolve_existing, resolve_for_create,
};
use super::repeated_read::{COMPACT_HEAD_LINES, ReadDecision, RepeatedReadCache};
use super::workspace_policy::{WorkspacePolicy, ensure_tool_path_allowed};

#[derive(Debug, Clone)]
pub struct ToolContext {
    pub root: PathBuf,
    pub mode: ExecutionMode,
    pub auto_approve: bool,
    pub interactive_approval: bool,
    pub offline: bool,
    pub workspace_policy: WorkspacePolicy,
    pub eval_events_path: Option<PathBuf>,
    pub expected_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolSpec {
    #[serde(rename = "type")]
    pub kind: String,
    pub function: FunctionSpec,
}

#[derive(Debug, Clone, Serialize)]
pub struct FunctionSpec {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

#[derive(Debug, Clone)]
pub struct ToolRegistry {
    specs: Vec<ToolSpec>,
    extensions: BTreeMap<String, super::extension::ExtensionTool>,
    repeated_reads: Arc<Mutex<RepeatedReadCache>>,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self {
            specs: default_tool_specs(),
            extensions: BTreeMap::new(),
            repeated_reads: Arc::new(Mutex::new(RepeatedReadCache::default())),
        }
    }
}

impl ToolRegistry {
    pub fn with_extensions(
        extensions: impl IntoIterator<Item = super::extension::ExtensionTool>,
    ) -> anyhow::Result<Self> {
        let mut registry = Self::default();
        for extension in extensions {
            registry.register_extension(extension)?;
        }
        Ok(registry)
    }

    pub fn register_extension(
        &mut self,
        extension: super::extension::ExtensionTool,
    ) -> anyhow::Result<()> {
        let name = extension.name().to_string();
        if self.specs.iter().any(|spec| spec.function.name == name) {
            bail!("duplicate tool name: {name}");
        }
        self.specs.push(extension.spec().clone());
        self.extensions.insert(name, extension);
        Ok(())
    }

    pub fn specs(&self) -> &[ToolSpec] {
        &self.specs
    }

    pub fn execute(
        &self,
        name: &str,
        arguments: &Value,
        context: &ToolContext,
    ) -> anyhow::Result<String> {
        self.execute_with_cancel(name, arguments, context, || false, || false)
    }

    pub fn execute_with_cancel<F, G>(
        &self,
        name: &str,
        arguments: &Value,
        context: &ToolContext,
        is_cancelled: F,
        is_force_cancelled: G,
    ) -> anyhow::Result<String>
    where
        F: Fn() -> bool,
        G: Fn() -> bool,
    {
        if let Some(extension) = self.extensions.get(name) {
            return extension.execute(arguments, context);
        }
        enforce_mode(name, context.mode)?;
        let recovered = recover_tool_arguments(name, arguments.clone());
        let arguments = &recovered.arguments;
        super::allow_policy::authorize_current(
            name,
            arguments,
            &context.root,
            context.auto_approve,
            context.interactive_approval,
        )?;
        if name != "Read" {
            self.repeated_reads().note_non_read_call();
        }
        match name {
            "Bash" => {
                let mut command = required_string(arguments, "command")?.to_string();
                if let Some(normalization) =
                    crate::tools::bash::strip_workspace_root_cd_prefix(&command, &context.root)
                {
                    eval_events::emit(
                        context.eval_events_path.as_deref(),
                        json!({
                            "event": "workspace_cd_stripped",
                            "original_prefix": eval_events::body_snippet(&normalization.original_prefix),
                            "normalized_command": eval_events::body_snippet(&normalization.normalized_command),
                        }),
                    );
                    command = normalization.normalized_command;
                }
                super::hidden_path::ensure_reference_allowed(
                    &context.root,
                    &command,
                    context.workspace_policy,
                )?;
                if let Some(normalization) =
                    crate::tools::bash::normalize_inspect_command(&command, &context.root)
                {
                    eval_events::emit(
                        context.eval_events_path.as_deref(),
                        json!({
                            "event": "inspect_command_normalized",
                            "schema_version": "1",
                            "original": eval_events::body_snippet(&normalization.original),
                            "normalized": eval_events::body_snippet(&normalization.normalized),
                        }),
                    );
                    command = normalization.normalized;
                }
                if let Some(rejection) =
                    crate::tools::bash::path_confinement_rejection(&command, &context.root)
                {
                    emit_bash_path_confinement_rejected(context, &command, &rejection);
                }
                crate::tools::bash::run_with_cancel_and_force(
                    &command,
                    &context.root,
                    context.offline,
                    is_cancelled,
                    is_force_cancelled,
                )
            }
            "Read" => {
                let raw = required_string(arguments, "path")?;
                let path =
                    resolve_policy_checked_path(context, "Read", raw, PathResolution::Existing)?;
                let start_line = optional_usize(arguments, "start_line");
                let end_line = optional_usize(arguments, "end_line");
                let decision = self
                    .repeated_reads()
                    .begin_read(&path, start_line, end_line);
                match decision {
                    ReadDecision::Unchanged(unchanged) => {
                        eval_events::emit(
                            context.eval_events_path.as_deref(),
                            json!({
                                "event": "tool_read_unchanged",
                                "schema_version": "1",
                                "path": unchanged.path,
                                "repeat_count": unchanged.repeat_count,
                                "identical_consecutive": unchanged.identical_consecutive,
                                "completion_candidate": unchanged.completion_candidate,
                                "compact_head_lines": COMPACT_HEAD_LINES,
                            }),
                        );
                        Ok(unchanged.response)
                    }
                    ReadDecision::Full(pending) => {
                        let output = crate::tools::read::run(
                            &context.root,
                            &path,
                            start_line,
                            end_line,
                            context.workspace_policy,
                        )?;
                        self.repeated_reads().record_full_read(pending, &output);
                        Ok(output)
                    }
                }
            }
            "Write" => {
                let raw = required_string(arguments, "path")?;
                let path =
                    resolve_policy_checked_path(context, "Write", raw, PathResolution::Create)?;
                let content = required_string(arguments, "content")?;
                let output = crate::tools::write::run(&context.root, &path, content)?;
                self.repeated_reads().note_successful_write(&path);
                Ok(output)
            }
            "Edit" => {
                let raw = required_string(arguments, "path")?;
                let normalized = normalize_path_arg(context, "Edit", raw)?;
                super::hidden_path::ensure_reference_allowed(
                    &context.root,
                    &normalized,
                    context.workspace_policy,
                )?;
                let path = resolve_policy_checked_path(
                    context,
                    "Edit",
                    &normalized,
                    PathResolution::Create,
                )?;
                let old = required_string(arguments, "old_string")?;
                let new = required_string(arguments, "new_string")?;
                let replace_all = arguments
                    .get("replace_all")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let output = crate::tools::edit::run(&context.root, &path, old, new, replace_all)?;
                self.repeated_reads().note_successful_write(&path);
                if output.contains("edit_anchor_salvaged") {
                    emit_edit_anchor_salvaged(context, &normalized, &output);
                }
                Ok(output)
            }
            "Glob" => {
                let pattern = required_string(arguments, "pattern")?;
                let normalized = normalize_glob_arg(context, "Glob", pattern)?;
                crate::tools::glob::run(&context.root, &normalized, context.workspace_policy)
            }
            "Grep" => {
                let pattern = required_string(arguments, "pattern")?;
                let glob = arguments
                    .get("glob")
                    .and_then(Value::as_str)
                    .map(|glob| normalize_glob_arg(context, "Grep", glob))
                    .transpose()?;
                let case_sensitive = arguments
                    .get("case_sensitive")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                crate::tools::grep::run(
                    &context.root,
                    pattern,
                    glob.as_deref(),
                    case_sensitive,
                    context.workspace_policy,
                )
            }
            other => bail!("unknown tool: {other}"),
        }
    }

    fn repeated_reads(&self) -> MutexGuard<'_, RepeatedReadCache> {
        self.repeated_reads
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[derive(Debug, Clone, Copy)]
enum PathResolution {
    Existing,
    Create,
}

fn resolve_policy_checked_path(
    context: &ToolContext,
    tool: &str,
    raw: &str,
    resolution: PathResolution,
) -> anyhow::Result<PathBuf> {
    let mut normalized = normalize_path_arg(context, tool, raw)?;
    if matches!(resolution, PathResolution::Existing)
        && !context.root.join(&normalized).exists()
        && let Some(fallback) = required_path_suffix_fallback(&normalized, &context.expected_paths)
        && context.root.join(&fallback).exists()
    {
        emit_path_fallback_evaluated(
            context,
            tool,
            raw,
            "existing_required_path",
            true,
            Some(&fallback),
            Some(1),
        );
        normalized = fallback;
    }
    super::hidden_path::ensure_reference_allowed(
        &context.root,
        &normalized,
        context.workspace_policy,
    )?;
    let path = match resolution {
        PathResolution::Existing => resolve_existing(&context.root, &normalized)?,
        PathResolution::Create => resolve_for_create(&context.root, &normalized)?,
    };
    ensure_tool_path_allowed(&context.root, &path, context.workspace_policy)?;
    Ok(path)
}

fn normalize_path_arg(context: &ToolContext, tool: &str, raw: &str) -> anyhow::Result<String> {
    match normalize_workspace_path(&context.root, raw) {
        Ok(Some(normalization)) => {
            match normalization.kind {
                WorkspacePathNormalizationKind::AbsoluteInsideWorkspace => {
                    emit_path_normalized(context, tool, raw, &normalization.relative);
                }
                WorkspacePathNormalizationKind::RootAnchorSalvage => {
                    emit_path_fallback_evaluated(
                        context,
                        tool,
                        raw,
                        "root_anchor",
                        true,
                        Some(&normalization.relative),
                        None,
                    );
                    emit_path_salvaged(context, tool, raw, &normalization.relative);
                }
                WorkspacePathNormalizationKind::MissingLeadingSlashRootAnchorSalvage => {
                    emit_path_malformed(context, tool, raw, true, Some(&normalization.relative));
                    emit_path_fallback_evaluated(
                        context,
                        tool,
                        raw,
                        "root_anchor",
                        true,
                        Some(&normalization.relative),
                        None,
                    );
                    emit_path_salvaged(context, tool, raw, &normalization.relative);
                }
            }
            Ok(normalization.relative)
        }
        Ok(None) => Ok(raw.to_string()),
        Err(err) => {
            if err
                .to_string()
                .contains("tool_args_path_near_root_corruption")
            {
                emit_path_near_root_corruption(context, tool, raw);
                return Err(err);
            }
            if err.to_string().contains("tool_args_path_malformed") {
                emit_path_malformed(context, tool, raw, false, None);
            }
            if std::path::Path::new(raw).is_absolute() && !context.expected_paths.is_empty() {
                emit_path_fallback_evaluated(context, tool, raw, "root_anchor", false, None, None);
                let candidate_count =
                    required_path_suffix_candidate_count(raw, &context.expected_paths);
                if let Some(normalized) =
                    required_path_suffix_fallback(raw, &context.expected_paths)
                {
                    emit_path_fallback_evaluated(
                        context,
                        tool,
                        raw,
                        "required_path",
                        true,
                        Some(&normalized),
                        Some(candidate_count),
                    );
                    return Ok(normalized);
                }
                let nearest = nearest_expected_path(raw, &context.expected_paths);
                emit_path_fallback_evaluated(
                    context,
                    tool,
                    raw,
                    "required_path",
                    false,
                    nearest.as_deref(),
                    Some(candidate_count),
                );
                if let Some(nearest) = nearest {
                    bail!(
                        "stale_absolute_path_recoverable: rejected absolute path `{raw}` outside current workspace root `{}`; use workspace-relative path `{nearest}`",
                        context.root.display()
                    );
                }
            }
            Err(err)
        }
    }
}

fn normalize_glob_arg(context: &ToolContext, tool: &str, raw: &str) -> anyhow::Result<String> {
    let normalized = match normalize_absolute_workspace_glob(&context.root, raw)? {
        Some(normalized) => {
            emit_path_normalized(context, tool, raw, &normalized);
            normalized
        }
        None => raw.to_string(),
    };
    super::hidden_path::ensure_reference_allowed(
        &context.root,
        &normalized,
        context.workspace_policy,
    )?;
    Ok(normalized)
}

fn emit_path_normalized(context: &ToolContext, tool: &str, original: &str, normalized: &str) {
    eval_events::emit(
        context.eval_events_path.as_deref(),
        json!({
            "event": "tool_args_path_normalized",
            "tool": tool,
            "original": original,
            "normalized": normalized,
        }),
    );
}

fn emit_path_salvaged(context: &ToolContext, tool: &str, original: &str, normalized: &str) {
    eval_events::emit(
        context.eval_events_path.as_deref(),
        json!({
            "event": "tool_args_path_salvaged",
            "tool": tool,
            "original": original,
            "normalized": normalized,
            "method": "root_anchor_last_two_components",
        }),
    );
}

fn emit_path_fallback_evaluated(
    context: &ToolContext,
    tool: &str,
    original: &str,
    method: &str,
    accepted: bool,
    normalized: Option<&str>,
    candidate_count: Option<usize>,
) {
    eval_events::emit(
        context.eval_events_path.as_deref(),
        json!({
            "event": "path_fallback_evaluated",
            "tool": tool,
            "original": original,
            "method": method,
            "accepted": accepted,
            "normalized": normalized.unwrap_or(""),
            "candidate_count": candidate_count,
        }),
    );
}

fn emit_path_malformed(
    context: &ToolContext,
    tool: &str,
    original: &str,
    accepted: bool,
    normalized: Option<&str>,
) {
    eval_events::emit(
        context.eval_events_path.as_deref(),
        json!({
            "event": "tool_args_path_malformed",
            "tool": tool,
            "original": original,
            "accepted": accepted,
            "normalized": normalized.unwrap_or(""),
            "kind": "missing_leading_slash_absolute",
        }),
    );
}

fn emit_path_near_root_corruption(context: &ToolContext, tool: &str, original: &str) {
    eval_events::emit(
        context.eval_events_path.as_deref(),
        json!({
            "event": "tool_args_path_near_root_corruption",
            "tool": tool,
            "original": original,
            "root": context.root.display().to_string(),
            "accepted": false,
        }),
    );
}

fn emit_bash_path_confinement_rejected(
    context: &ToolContext,
    command: &str,
    rejection: &crate::tools::bash::BashPathConfinementRejection,
) {
    eval_events::emit(
        context.eval_events_path.as_deref(),
        json!({
            "event": "bash_path_confinement_rejected",
            "schema_version": "1",
            "blocked": true,
            "reason": rejection.reason,
            "operation": rejection.operation,
            "command": eval_events::body_snippet(command),
            "path": rejection.path,
            "root": rejection.root,
            "nearest_relative": rejection.nearest_relative,
            "guidance": rejection.guidance,
        }),
    );
}

fn emit_edit_anchor_salvaged(context: &ToolContext, path: &str, output: &str) {
    eval_events::emit(
        context.eval_events_path.as_deref(),
        json!({
            "event": "edit_anchor_salvaged",
            "path": path,
            "method": "whitespace_normalized_unique_region",
            "result": eval_events::body_snippet(output),
        }),
    );
}

fn enforce_mode(name: &str, mode: ExecutionMode) -> anyhow::Result<()> {
    if mode == ExecutionMode::Plan && !matches!(name, "Read" | "Glob" | "Grep") {
        bail!("Plan mode allows only Read, Glob, and Grep");
    }
    Ok(())
}

fn required_path_suffix_fallback(raw: &str, expected_paths: &[String]) -> Option<String> {
    let matches = required_path_suffix_matches(raw, expected_paths);
    (matches.len() == 1).then(|| matches[0].clone())
}

fn required_path_suffix_candidate_count(raw: &str, expected_paths: &[String]) -> usize {
    required_path_suffix_matches(raw, expected_paths).len()
}

fn required_path_suffix_matches(raw: &str, expected_paths: &[String]) -> Vec<String> {
    let raw_components = path_components(raw);
    expected_paths
        .iter()
        .filter(|path| super::path_guard::validate_workspace_relative(path).is_ok())
        .filter(|path| {
            let expected_components = path_components(path);
            !expected_components.is_empty()
                && raw_components.len() >= expected_components.len()
                && raw_components[raw_components.len() - expected_components.len()..]
                    == expected_components
        })
        .cloned()
        .collect()
}

fn nearest_expected_path(raw: &str, expected_paths: &[String]) -> Option<String> {
    let raw_components = path_components(raw);
    expected_paths
        .iter()
        .filter(|path| super::path_guard::validate_workspace_relative(path).is_ok())
        .max_by_key(|path| common_suffix_len(&raw_components, &path_components(path)))
        .cloned()
}

fn common_suffix_len(left: &[String], right: &[String]) -> usize {
    left.iter()
        .rev()
        .zip(right.iter().rev())
        .take_while(|(left, right)| left == right)
        .count()
}

fn path_components(raw: &str) -> Vec<String> {
    std::path::Path::new(raw)
        .components()
        .filter_map(|component| match component {
            std::path::Component::Normal(value) => Some(value.to_string_lossy().to_string()),
            _ => None,
        })
        .collect()
}

pub fn required_string<'a>(value: &'a Value, key: &str) -> anyhow::Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("missing string argument `{key}`"))
}

pub fn tool_error_kind(err: &anyhow::Error) -> &'static str {
    let message = err.to_string();
    if message.starts_with("missing string argument `") {
        "missing_arg"
    } else if message.contains("tool_args_path_near_root_corruption") {
        "tool_args_path_near_root_corruption"
    } else if message.contains("tool_args_path_malformed") {
        "tool_args_path_malformed"
    } else if message.contains("stale_absolute_path_recoverable") {
        "stale_absolute_path_recoverable"
    } else if message.starts_with("unknown tool:") {
        "unknown_tool"
    } else if message.starts_with("extension_tool_rejected:") {
        "extension_tool_rejected"
    } else if message.contains("bash_path_confinement_error") {
        "bash_path_confinement_error"
    } else if message.contains("path escapes workspace")
        || message.contains("path may not contain ..")
        || message.contains("absolute path is not allowed")
        || message.contains("path contains NUL byte")
        || message.contains("symlink_write_blocked")
    {
        "path_confinement_error"
    } else if message.contains("path_not_found_recoverable") {
        "path_not_found_recoverable"
    } else if message.contains("workspace_policy_blocked") {
        "workspace_policy_blocked"
    } else if message.contains("invalid glob pattern") {
        "invalid_glob"
    } else if message.contains("Is a directory") || message.contains("is a directory") {
        "read_directory"
    } else if message.contains("dangerous command blocked") {
        "dangerous_command"
    } else if message.contains("command_timeout") {
        "command_timeout"
    } else if message.contains("command_aborted_by_user") {
        "command_aborted_by_user"
    } else if message.contains("verify_command_policy_error") {
        "verify_command_policy_error"
    } else if message.contains("edit_anchor_not_found") {
        "edit_anchor_not_found"
    } else if message.contains("edit_noop") {
        "edit_noop"
    } else if message.contains("edit_ambiguous_anchor") {
        "edit_ambiguous_anchor"
    } else if message.contains("edit_already_applied") {
        "edit_already_applied"
    } else if message.contains("approval required") {
        "approval_required"
    } else {
        "tool_execution_error"
    }
}

pub fn recoverable_tool_error(err: &anyhow::Error) -> bool {
    if super::hidden_path::access_from_error(err).is_some() {
        return true;
    }
    matches!(
        tool_error_kind(err),
        "missing_arg"
            | "unknown_tool"
            | "extension_tool_rejected"
            | "tool_args_path_near_root_corruption"
            | "tool_args_path_malformed"
            | "bash_path_confinement_error"
            | "stale_absolute_path_recoverable"
            | "path_not_found_recoverable"
            | "command_timeout"
            | "verify_command_policy_error"
            | "invalid_glob"
            | "read_directory"
            | "edit_anchor_not_found"
            | "edit_noop"
            | "edit_ambiguous_anchor"
            | "edit_already_applied"
    )
}

pub fn missing_arg_name(err: &anyhow::Error) -> Option<String> {
    let message = err.to_string();
    let rest = message.strip_prefix("missing string argument `")?;
    Some(rest.split('`').next()?.to_string())
}

fn optional_usize(value: &Value, key: &str) -> Option<usize> {
    value.get(key).and_then(Value::as_u64).map(|n| n as usize)
}

fn default_tool_specs() -> Vec<ToolSpec> {
    vec![
        spec(
            "Bash",
            "Run a local build/test/read-only shell command in the workspace.",
            json!({"type":"object","properties":{"command":{"type":"string"}},"required":["command"]}),
        ),
        spec(
            "Read",
            "Read a workspace-relative file path.",
            json!({"type":"object","properties":{"path":{"type":"string"},"start_line":{"type":"integer"},"end_line":{"type":"integer"}},"required":["path"]}),
        ),
        spec(
            "Write",
            "Write a workspace-relative file path. Parent directories are created automatically.",
            json!({"type":"object","properties":{"path":{"type":"string"},"content":{"type":"string"}},"required":["path","content"]}),
        ),
        spec(
            "Edit",
            "Replace an exact string in a workspace-relative file path.",
            json!({"type":"object","properties":{"path":{"type":"string"},"old_string":{"type":"string"},"new_string":{"type":"string"},"replace_all":{"type":"boolean"}},"required":["path","old_string","new_string"]}),
        ),
        spec(
            "Glob",
            "List files matching a workspace-relative glob pattern.",
            json!({"type":"object","properties":{"pattern":{"type":"string"}},"required":["pattern"]}),
        ),
        spec(
            "Grep",
            "Search text in workspace files.",
            json!({"type":"object","properties":{"pattern":{"type":"string"},"glob":{"type":"string"},"case_sensitive":{"type":"boolean"}},"required":["pattern"]}),
        ),
    ]
}

fn spec(name: &str, description: &str, parameters: Value) -> ToolSpec {
    ToolSpec {
        kind: "function".to_string(),
        function: FunctionSpec {
            name: name.to_string(),
            description: description.to_string(),
            parameters,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn plan_act_tool_allowlist() {
        let registry = ToolRegistry::default();
        let dir = tempfile::tempdir().unwrap();
        let context = ToolContext {
            root: dir.path().to_path_buf(),
            mode: ExecutionMode::Plan,
            auto_approve: true,
            interactive_approval: false,
            offline: false,
            workspace_policy: WorkspacePolicy::NormalTask,
            eval_events_path: None,
            expected_paths: Vec::new(),
        };
        assert!(
            registry
                .execute("Write", &json!({"path":"a","content":"b"}), &context)
                .is_err()
        );
    }

    #[test]
    fn explicit_write_allowance_blocks_bash_before_execution() {
        let registry = ToolRegistry::default();
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("bash-ran");
        let context = ToolContext {
            root: dir.path().to_path_buf(),
            mode: ExecutionMode::Act,
            auto_approve: true,
            interactive_approval: false,
            offline: false,
            workspace_policy: WorkspacePolicy::NormalTask,
            eval_events_path: None,
            expected_paths: Vec::new(),
        };
        let _policy = crate::tools::allow_policy::install(
            false,
            &[crate::tools::allow_policy::AllowTarget::Write],
        );

        registry
            .execute(
                "Write",
                &json!({"path":"allowed.txt","content":"ok"}),
                &context,
            )
            .unwrap();
        let error = registry
            .execute(
                "Bash",
                &json!({"command": format!("touch {}", marker.display())}),
                &context,
            )
            .unwrap_err()
            .to_string();

        assert!(error.contains("not permitted by --allow write"), "{error}");
        assert!(!marker.exists(), "disallowed Bash command executed");
    }

    #[test]
    fn write_parent_dir() {
        let registry = ToolRegistry::default();
        let dir = tempfile::tempdir().unwrap();
        let context = ToolContext {
            root: dir.path().to_path_buf(),
            mode: ExecutionMode::Act,
            auto_approve: true,
            interactive_approval: false,
            offline: false,
            workspace_policy: WorkspacePolicy::NormalTask,
            eval_events_path: None,
            expected_paths: Vec::new(),
        };
        registry
            .execute("Write", &json!({"path":"a/b.txt","content":"ok"}), &context)
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.path().join("a/b.txt")).unwrap(),
            "ok"
        );
    }

    #[test]
    fn repeated_unchanged_read_compacts_but_changed_file_returns_full_content() {
        let registry = ToolRegistry::default();
        let dir = tempfile::tempdir().unwrap();
        let events = dir.path().join("events.jsonl");
        let context = ToolContext {
            root: dir.path().to_path_buf(),
            mode: ExecutionMode::Act,
            auto_approve: true,
            interactive_approval: false,
            offline: false,
            workspace_policy: WorkspacePolicy::NormalTask,
            eval_events_path: Some(events.clone()),
            expected_paths: Vec::new(),
        };
        let content = (1..=25)
            .map(|line| format!("line {line}"))
            .collect::<Vec<_>>()
            .join("\n");
        registry
            .execute(
                "Write",
                &json!({"path":"sample.txt","content":content}),
                &context,
            )
            .unwrap();

        let first = registry
            .execute("Read", &json!({"path":"sample.txt"}), &context)
            .unwrap();
        let second = registry
            .execute("Read", &json!({"path":"sample.txt"}), &context)
            .unwrap();

        assert!(first.contains("line 25"), "{first}");
        assert!(!first.contains("unchanged since"), "{first}");
        assert!(second.contains("unchanged since"), "{second}");
        assert!(second.contains("completion candidate"), "{second}");
        assert!(second.contains("line 20"), "{second}");
        assert!(!second.contains("line 21"), "{second}");

        std::fs::write(dir.path().join("sample.txt"), "changed\nfull response").unwrap();
        let changed = registry
            .execute("Read", &json!({"path":"sample.txt"}), &context)
            .unwrap();
        assert!(changed.contains("changed\nfull response"), "{changed}");
        assert!(!changed.contains("unchanged since"), "{changed}");

        let event_text = std::fs::read_to_string(events).unwrap();
        assert!(event_text.contains(r#""event":"tool_read_unchanged""#));
        assert!(event_text.contains(r#""identical_consecutive":true"#));
        assert!(event_text.contains(r#""completion_candidate":true"#));
    }

    #[test]
    fn failed_edit_does_not_make_repeated_read_a_completion_candidate() {
        let registry = ToolRegistry::default();
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("sample.txt"), "content").unwrap();
        let context = ToolContext {
            root: dir.path().to_path_buf(),
            mode: ExecutionMode::Act,
            auto_approve: true,
            interactive_approval: false,
            offline: false,
            workspace_policy: WorkspacePolicy::NormalTask,
            eval_events_path: None,
            expected_paths: Vec::new(),
        };
        registry
            .execute(
                "Edit",
                &json!({
                    "path":"sample.txt",
                    "old_string":"missing",
                    "new_string":"replacement"
                }),
                &context,
            )
            .unwrap_err();

        registry
            .execute("Read", &json!({"path":"sample.txt"}), &context)
            .unwrap();
        let repeated = registry
            .execute("Read", &json!({"path":"sample.txt"}), &context)
            .unwrap();

        assert!(repeated.contains("unchanged since"), "{repeated}");
        assert!(!repeated.contains("completion candidate"), "{repeated}");
    }

    #[test]
    fn normal_workspace_policy_blocks_anvil_metadata_write() {
        let registry = ToolRegistry::default();
        let dir = tempfile::tempdir().unwrap();
        let context = ToolContext {
            root: dir.path().to_path_buf(),
            mode: ExecutionMode::Act,
            auto_approve: true,
            interactive_approval: false,
            offline: false,
            workspace_policy: WorkspacePolicy::NormalTask,
            eval_events_path: None,
            expected_paths: Vec::new(),
        };

        let err = registry
            .execute(
                "Write",
                &json!({"path":".anvil/x","content":"no"}),
                &context,
            )
            .unwrap_err();

        assert_eq!(tool_error_kind(&err), "workspace_policy_blocked");
        assert!(!dir.path().join(".anvil/x").exists());
    }

    #[test]
    fn normal_workspace_policy_blocks_git_metadata_edit() {
        let registry = ToolRegistry::default();
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".git")).unwrap();
        std::fs::write(dir.path().join(".git/config"), "old").unwrap();
        let context = ToolContext {
            root: dir.path().to_path_buf(),
            mode: ExecutionMode::Act,
            auto_approve: true,
            interactive_approval: false,
            offline: false,
            workspace_policy: WorkspacePolicy::NormalTask,
            eval_events_path: None,
            expected_paths: Vec::new(),
        };

        let err = registry
            .execute(
                "Edit",
                &json!({"path":".git/config","old_string":"old","new_string":"new"}),
                &context,
            )
            .unwrap_err();

        assert_eq!(tool_error_kind(&err), "workspace_policy_blocked");
        assert_eq!(
            std::fs::read_to_string(dir.path().join(".git/config")).unwrap(),
            "old"
        );
    }

    #[test]
    fn edit_anchor_salvage_emits_telemetry() {
        let registry = ToolRegistry::default();
        let dir = tempfile::tempdir().unwrap();
        let events = dir.path().join("events.jsonl");
        std::fs::write(dir.path().join("a.txt"), "const  x = 1;\n").unwrap();
        let context = ToolContext {
            root: dir.path().to_path_buf(),
            mode: ExecutionMode::Act,
            auto_approve: true,
            interactive_approval: false,
            offline: false,
            workspace_policy: WorkspacePolicy::NormalTask,
            eval_events_path: Some(events.clone()),
            expected_paths: Vec::new(),
        };

        let output = registry
            .execute(
                "Edit",
                &json!({"path":"a.txt","old_string":"const x = 1;","new_string":"const x = 2;"}),
                &context,
            )
            .unwrap();

        assert!(output.contains("edit_anchor_salvaged"));
        assert_eq!(
            std::fs::read_to_string(dir.path().join("a.txt")).unwrap(),
            "const x = 2;\n"
        );
        let event_text = std::fs::read_to_string(events).unwrap();
        assert!(event_text.contains(r#""event":"edit_anchor_salvaged""#));
        assert!(event_text.contains(r#""path":"a.txt""#));
    }

    #[test]
    fn absolute_internal_write_is_normalized_and_audited() {
        let registry = ToolRegistry::default();
        let dir = tempfile::tempdir().unwrap();
        let events = dir.path().join("events.jsonl");
        let context = ToolContext {
            root: dir.path().to_path_buf(),
            mode: ExecutionMode::Act,
            auto_approve: true,
            interactive_approval: false,
            offline: false,
            workspace_policy: WorkspacePolicy::NormalTask,
            eval_events_path: Some(events.clone()),
            expected_paths: Vec::new(),
        };
        let absolute = dir.path().join("src/app/page.tsx");

        registry
            .execute(
                "Write",
                &json!({"path": absolute.display().to_string(), "content":"ok"}),
                &context,
            )
            .unwrap();

        assert_eq!(std::fs::read_to_string(&absolute).unwrap(), "ok");
        let event_text = std::fs::read_to_string(events).unwrap();
        assert!(event_text.contains(r#""event":"tool_args_path_normalized""#));
        assert!(event_text.contains(r#""tool":"Write""#));
        assert!(event_text.contains(r#""normalized":"src/app/page.tsx""#));
    }

    #[test]
    fn root_anchor_absolute_write_is_salvaged_and_audited() {
        let registry = ToolRegistry::default();
        let dir = tempfile::tempdir().unwrap();
        let root = dir
            .path()
            .join("localwork/commandagent_mvp/01/test0708_013");
        std::fs::create_dir_all(&root).unwrap();
        let events = dir.path().join("events.jsonl");
        let context = ToolContext {
            root: root.clone(),
            mode: ExecutionMode::Act,
            auto_approve: true,
            interactive_approval: false,
            offline: false,
            workspace_policy: WorkspacePolicy::NormalTask,
            eval_events_path: Some(events.clone()),
            expected_paths: Vec::new(),
        };
        let raw = dir
            .path()
            .join("share/work/commandagent_mvp/01/test0708_013/package.json");

        registry
            .execute(
                "Write",
                &json!({"path": raw.display().to_string(), "content":"{}"}),
                &context,
            )
            .unwrap();

        assert_eq!(
            std::fs::read_to_string(root.join("package.json")).unwrap(),
            "{}"
        );
        let event_text = std::fs::read_to_string(events).unwrap();
        assert!(event_text.contains(r#""event":"tool_args_path_salvaged""#));
        assert!(event_text.contains(r#""tool":"Write""#));
        assert!(event_text.contains(r#""normalized":"package.json""#));
        assert!(event_text.contains(r#""method":"root_anchor_last_two_components""#));
    }

    #[test]
    fn stale_absolute_write_falls_back_to_unique_expected_path_suffix() {
        let registry = ToolRegistry::default();
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("current/workspace");
        std::fs::create_dir_all(&root).unwrap();
        let events = dir.path().join("events.jsonl");
        let context = ToolContext {
            root: root.clone(),
            mode: ExecutionMode::Act,
            auto_approve: true,
            interactive_approval: false,
            offline: false,
            workspace_policy: WorkspacePolicy::NormalTask,
            eval_events_path: Some(events.clone()),
            expected_paths: vec!["package.json".to_string(), "src/app/page.tsx".to_string()],
        };
        let raw = "/Users/example/share/work/old-run/package.json";

        registry
            .execute("Write", &json!({"path": raw, "content": "{}"}), &context)
            .unwrap();

        assert_eq!(
            std::fs::read_to_string(root.join("package.json")).unwrap(),
            "{}"
        );
        let event_text = std::fs::read_to_string(events).unwrap();
        assert!(event_text.contains(r#""event":"path_fallback_evaluated""#));
        assert!(event_text.contains(r#""method":"root_anchor""#));
        assert!(event_text.contains(r#""accepted":false"#));
        assert!(event_text.contains(r#""method":"required_path""#));
        assert!(event_text.contains(r#""accepted":true"#));
        assert!(event_text.contains(r#""normalized":"package.json""#));
    }

    #[test]
    fn nonexistent_relative_read_falls_back_to_unique_existing_expected_suffix() {
        let registry = ToolRegistry::default();
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("lib")).unwrap();
        std::fs::write(
            dir.path().join("lib/label.mjs"),
            "export const label = 'ok';\n",
        )
        .unwrap();
        let events = dir.path().join("events.jsonl");
        let context = ToolContext {
            root: dir.path().to_path_buf(),
            mode: ExecutionMode::Act,
            auto_approve: true,
            interactive_approval: false,
            offline: false,
            workspace_policy: WorkspacePolicy::NormalTask,
            eval_events_path: Some(events.clone()),
            expected_paths: vec!["lib/label.mjs".to_string()],
        };

        let output = registry
            .execute("Read", &json!({"path":"app/lib/label.mjs"}), &context)
            .unwrap();

        assert!(output.contains("export const label"));
        let events = std::fs::read_to_string(events).unwrap();
        assert!(events.contains(r#""method":"existing_required_path""#));
        assert!(events.contains(r#""normalized":"lib/label.mjs""#));
    }

    #[test]
    fn stale_absolute_write_rejection_names_current_root_and_nearest_expected_path() {
        let registry = ToolRegistry::default();
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("current/workspace");
        std::fs::create_dir_all(&root).unwrap();
        let events = dir.path().join("events.jsonl");
        let context = ToolContext {
            root: root.clone(),
            mode: ExecutionMode::Act,
            auto_approve: true,
            interactive_approval: false,
            offline: false,
            workspace_policy: WorkspacePolicy::NormalTask,
            eval_events_path: Some(events.clone()),
            expected_paths: vec!["src/app/page.tsx".to_string()],
        };
        let raw = "/Users/example/share/work/old-run/src/app/layout.tsx";

        let err = registry
            .execute("Write", &json!({"path": raw, "content": "no"}), &context)
            .unwrap_err();

        assert_eq!(tool_error_kind(&err), "stale_absolute_path_recoverable");
        assert!(recoverable_tool_error(&err));
        let message = err.to_string();
        assert!(message.contains(&root.display().to_string()), "{message}");
        assert!(message.contains("src/app/page.tsx"), "{message}");
        assert!(!root.join("src/app/layout.tsx").exists());
        let event_text = std::fs::read_to_string(events).unwrap();
        assert!(event_text.contains(r#""method":"root_anchor""#));
        assert!(event_text.contains(r#""method":"required_path""#));
        assert!(event_text.contains(r#""accepted":false"#));
        assert!(event_text.contains(r#""normalized":"src/app/page.tsx""#));
    }

    #[test]
    fn near_root_digit_variance_write_rejects_without_expected_path_salvage() {
        let registry = ToolRegistry::default();
        let dir = tempfile::tempdir().unwrap();
        let root = dir
            .path()
            .join("localwork/commandagent_mvp/01/test0710_camp_002");
        std::fs::create_dir_all(&root).unwrap();
        let events = dir.path().join("events.jsonl");
        let context = ToolContext {
            root: root.clone(),
            mode: ExecutionMode::Act,
            auto_approve: true,
            interactive_approval: false,
            offline: false,
            workspace_policy: WorkspacePolicy::NormalTask,
            eval_events_path: Some(events.clone()),
            expected_paths: vec!["src/app/page.tsx".to_string()],
        };
        let raw = dir
            .path()
            .join("localwork/commandagent_mvp/01/test0710_camp_001/src/app/page.tsx");

        let err = registry
            .execute(
                "Write",
                &json!({"path": raw.display().to_string(), "content":"no"}),
                &context,
            )
            .unwrap_err();

        assert_eq!(tool_error_kind(&err), "tool_args_path_near_root_corruption");
        assert!(recoverable_tool_error(&err));
        let message = err.to_string();
        assert!(message.contains(&root.display().to_string()), "{message}");
        assert!(!root.join("src/app/page.tsx").exists());
        let event_text = std::fs::read_to_string(events).unwrap();
        assert!(event_text.contains(r#""event":"tool_args_path_near_root_corruption""#));
        assert!(event_text.contains(r#""accepted":false"#));
        assert!(!event_text.contains(r#""method":"required_path""#));
    }

    #[test]
    fn missing_leading_slash_write_is_malformed_then_root_anchor_salvaged() {
        let registry = ToolRegistry::default();
        let dir = tempfile::tempdir().unwrap();
        let root = dir
            .path()
            .join("localwork/commandagent_mvp/01/test0709_camp_003");
        std::fs::create_dir_all(&root).unwrap();
        let events = dir.path().join("events.jsonl");
        let context = ToolContext {
            root: root.clone(),
            mode: ExecutionMode::Act,
            auto_approve: true,
            interactive_approval: false,
            offline: false,
            workspace_policy: WorkspacePolicy::NormalTask,
            eval_events_path: Some(events.clone()),
            expected_paths: Vec::new(),
        };
        let raw = "Users/maenokota/share/work/localwork/commandagent_mvp/01/test0709_camp_003/src/app/page.tsx";

        registry
            .execute("Write", &json!({"path": raw, "content":"ok"}), &context)
            .unwrap();

        assert_eq!(
            std::fs::read_to_string(root.join("src/app/page.tsx")).unwrap(),
            "ok"
        );
        let event_text = std::fs::read_to_string(events).unwrap();
        assert!(event_text.contains(r#""event":"tool_args_path_malformed""#));
        assert!(event_text.contains(r#""accepted":true"#));
        assert!(event_text.contains(r#""event":"tool_args_path_salvaged""#));
        assert!(event_text.contains(r#""normalized":"src/app/page.tsx""#));
    }

    #[test]
    fn missing_leading_slash_write_without_anchor_is_rejected_as_malformed() {
        let registry = ToolRegistry::default();
        let dir = tempfile::tempdir().unwrap();
        let root = dir
            .path()
            .join("localwork/commandagent_mvp/01/test0709_camp_003");
        std::fs::create_dir_all(&root).unwrap();
        let events = dir.path().join("events.jsonl");
        let context = ToolContext {
            root,
            mode: ExecutionMode::Act,
            auto_approve: true,
            interactive_approval: false,
            offline: false,
            workspace_policy: WorkspacePolicy::NormalTask,
            eval_events_path: Some(events.clone()),
            expected_paths: Vec::new(),
        };
        let raw = "Users/maenokota/share/work/other-run/src/App.js";

        let err = registry
            .execute("Write", &json!({"path": raw, "content":"no"}), &context)
            .unwrap_err();

        assert_eq!(tool_error_kind(&err), "tool_args_path_malformed");
        assert!(recoverable_tool_error(&err));
        let event_text = std::fs::read_to_string(events).unwrap();
        assert!(event_text.contains(r#""event":"tool_args_path_malformed""#));
        assert!(event_text.contains(r#""accepted":false"#));
    }

    #[test]
    fn absolute_outside_write_is_rejected_as_path_confinement() {
        let registry = ToolRegistry::default();
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let context = ToolContext {
            root: dir.path().to_path_buf(),
            mode: ExecutionMode::Act,
            auto_approve: true,
            interactive_approval: false,
            offline: false,
            workspace_policy: WorkspacePolicy::NormalTask,
            eval_events_path: None,
            expected_paths: Vec::new(),
        };
        let outside_path = outside.path().join("escape.txt");

        let err = registry
            .execute(
                "Write",
                &json!({"path": outside_path.display().to_string(), "content":"no"}),
                &context,
            )
            .unwrap_err();

        assert_eq!(tool_error_kind(&err), "path_confinement_error");
        assert!(err.to_string().contains("use workspace-relative paths"));
        assert!(!outside_path.exists());
    }

    #[cfg(unix)]
    #[test]
    fn absolute_internal_write_through_symlink_escape_is_rejected() {
        let registry = ToolRegistry::default();
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), dir.path().join("out")).unwrap();
        let context = ToolContext {
            root: dir.path().to_path_buf(),
            mode: ExecutionMode::Act,
            auto_approve: true,
            interactive_approval: false,
            offline: false,
            workspace_policy: WorkspacePolicy::NormalTask,
            eval_events_path: None,
            expected_paths: Vec::new(),
        };
        let raw = dir.path().join("out/escape.txt");

        let err = registry
            .execute(
                "Write",
                &json!({"path": raw.display().to_string(), "content":"no"}),
                &context,
            )
            .unwrap_err();

        assert_eq!(tool_error_kind(&err), "path_confinement_error");
        assert!(!outside.path().join("escape.txt").exists());
    }

    #[cfg(unix)]
    #[test]
    fn write_rejects_target_symlink_to_outside() {
        let registry = ToolRegistry::default();
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("escape.txt"), "outside").unwrap();
        std::os::unix::fs::symlink(outside.path().join("escape.txt"), dir.path().join("file"))
            .unwrap();
        let context = ToolContext {
            root: dir.path().to_path_buf(),
            mode: ExecutionMode::Act,
            auto_approve: true,
            interactive_approval: false,
            offline: false,
            workspace_policy: WorkspacePolicy::NormalTask,
            eval_events_path: None,
            expected_paths: Vec::new(),
        };

        let err = registry
            .execute("Write", &json!({"path":"file","content":"no"}), &context)
            .unwrap_err();

        assert!(err.to_string().contains("symlink_write_blocked"), "{err}");
        assert_eq!(
            std::fs::read_to_string(outside.path().join("escape.txt")).unwrap(),
            "outside"
        );
    }

    #[cfg(unix)]
    #[test]
    fn edit_rejects_target_symlink_to_outside_before_read() {
        let registry = ToolRegistry::default();
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("escape.txt"), "old").unwrap();
        std::os::unix::fs::symlink(outside.path().join("escape.txt"), dir.path().join("file"))
            .unwrap();
        let context = ToolContext {
            root: dir.path().to_path_buf(),
            mode: ExecutionMode::Act,
            auto_approve: true,
            interactive_approval: false,
            offline: false,
            workspace_policy: WorkspacePolicy::NormalTask,
            eval_events_path: None,
            expected_paths: Vec::new(),
        };

        let err = registry
            .execute(
                "Edit",
                &json!({"path":"file","old_string":"old","new_string":"new"}),
                &context,
            )
            .unwrap_err();

        assert!(err.to_string().contains("symlink_write_blocked"), "{err}");
        assert_eq!(
            std::fs::read_to_string(outside.path().join("escape.txt")).unwrap(),
            "old"
        );
    }

    #[test]
    fn relative_write_behavior_does_not_emit_normalization_event() {
        let registry = ToolRegistry::default();
        let dir = tempfile::tempdir().unwrap();
        let events = dir.path().join("events.jsonl");
        let context = ToolContext {
            root: dir.path().to_path_buf(),
            mode: ExecutionMode::Act,
            auto_approve: true,
            interactive_approval: false,
            offline: false,
            workspace_policy: WorkspacePolicy::NormalTask,
            eval_events_path: Some(events.clone()),
            expected_paths: Vec::new(),
        };

        let output = registry
            .execute(
                "Write",
                &json!({"path":"notes/new.md","content":"ok"}),
                &context,
            )
            .unwrap();

        assert_eq!(
            std::fs::read_to_string(dir.path().join("notes/new.md")).unwrap(),
            "ok"
        );
        assert_eq!(
            output,
            format!(
                "wrote {}",
                dir.path()
                    .canonicalize()
                    .unwrap()
                    .join("notes/new.md")
                    .display()
            )
        );
        assert!(
            !events.exists(),
            "relative paths must not be rewritten or audited as normalized"
        );
    }

    #[test]
    fn dot_dot_traversal_still_rejected() {
        let registry = ToolRegistry::default();
        let dir = tempfile::tempdir().unwrap();
        let context = ToolContext {
            root: dir.path().to_path_buf(),
            mode: ExecutionMode::Act,
            auto_approve: true,
            interactive_approval: false,
            offline: false,
            workspace_policy: WorkspacePolicy::NormalTask,
            eval_events_path: None,
            expected_paths: Vec::new(),
        };

        let err = registry
            .execute(
                "Write",
                &json!({"path":"safe/../escape.txt","content":"no"}),
                &context,
            )
            .unwrap_err();

        assert_eq!(tool_error_kind(&err), "path_confinement_error");
        assert!(err.to_string().contains("use workspace-relative paths"));
        assert!(!dir.path().join("escape.txt").exists());
    }

    #[test]
    fn dangerous_command() {
        assert!(crate::tools::bash::blocked_reason("rm -rf /", false).is_some());
    }

    #[test]
    fn bash_rejects_absolute_path_outside_workspace_with_feedback_and_telemetry() {
        let registry = ToolRegistry::default();
        let dir = tempfile::tempdir().unwrap();
        let root = dir
            .path()
            .join("localwork/commandagent_mvp/01/test0709_camp_003");
        std::fs::create_dir_all(&root).unwrap();
        let events = dir.path().join("events.jsonl");
        let context = ToolContext {
            root: root.clone(),
            mode: ExecutionMode::Act,
            auto_approve: true,
            interactive_approval: false,
            offline: false,
            workspace_policy: WorkspacePolicy::NormalTask,
            eval_events_path: Some(events.clone()),
            expected_paths: Vec::new(),
        };
        let command =
            "ls /Users/maenokota/share/work/localwork/commandagent_mvp/01/test0709_camp_003";

        let err = registry
            .execute("Bash", &json!({"command": command}), &context)
            .unwrap_err();

        assert_eq!(tool_error_kind(&err), "bash_path_confinement_error");
        assert!(recoverable_tool_error(&err));
        let message = err.to_string();
        assert!(message.contains(&root.display().to_string()), "{message}");
        assert!(message.contains("test0709_camp_003"), "{message}");
        let event_text = std::fs::read_to_string(events).unwrap();
        assert!(event_text.contains(r#""event":"bash_path_confinement_rejected""#));
    }

    #[test]
    fn bash_strips_workspace_root_cd_prefix_and_records_event() {
        let registry = ToolRegistry::default();
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("workspace");
        std::fs::create_dir_all(&root).unwrap();
        let events = dir.path().join("events.jsonl");
        let context = ToolContext {
            root: root.clone(),
            mode: ExecutionMode::Act,
            auto_approve: true,
            interactive_approval: false,
            offline: false,
            workspace_policy: WorkspacePolicy::NormalTask,
            eval_events_path: Some(events.clone()),
            expected_paths: Vec::new(),
        };
        let command = format!("cd '{}' && printf data6-ok > output.txt", root.display());

        let output = registry
            .execute("Bash", &json!({"command": command}), &context)
            .unwrap();

        assert!(output.contains("outcome: Success"), "{output}");
        assert_eq!(
            std::fs::read_to_string(root.join("output.txt")).unwrap(),
            "data6-ok"
        );
        let event_text = std::fs::read_to_string(events).unwrap();
        let event: Value = event_text
            .lines()
            .find(|line| line.contains(r#""event":"workspace_cd_stripped""#))
            .map(|line| serde_json::from_str(line).unwrap())
            .expect("workspace_cd_stripped event");
        let expected_prefix = format!("cd '{}' &&", root.display());
        assert_eq!(
            event.get("original_prefix").and_then(Value::as_str),
            Some(expected_prefix.as_str())
        );
        assert_eq!(
            event.get("normalized_command").and_then(Value::as_str),
            Some("printf data6-ok > output.txt")
        );
    }

    #[test]
    fn bash_keeps_outside_root_cd_rejected_with_relative_retry_guidance() {
        let registry = ToolRegistry::default();
        let fixture_parent = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
        std::fs::create_dir_all(&fixture_parent).unwrap();
        let dir = tempfile::Builder::new()
            .prefix("registry-outside-root-")
            .tempdir_in(fixture_parent)
            .unwrap();
        let root = dir.path().join("workspace");
        let outside = dir.path().join("outside");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        let events = dir.path().join("events.jsonl");
        let context = ToolContext {
            root: root.clone(),
            mode: ExecutionMode::Act,
            auto_approve: true,
            interactive_approval: false,
            offline: false,
            workspace_policy: WorkspacePolicy::NormalTask,
            eval_events_path: Some(events.clone()),
            expected_paths: Vec::new(),
        };
        let command = format!("cd '{}' && printf forbidden", outside.display());

        let err = registry
            .execute("Bash", &json!({"command": command}), &context)
            .unwrap_err();

        assert_eq!(tool_error_kind(&err), "bash_path_confinement_error");
        assert!(recoverable_tool_error(&err));
        assert!(
            err.to_string()
                .contains("workspace相対で再実行せよ: outside"),
            "{err}"
        );
        let event_text = std::fs::read_to_string(events).unwrap();
        assert!(event_text.contains(r#""event":"bash_path_confinement_rejected""#));
        assert!(event_text.contains(r#""nearest_relative":"outside""#));
        assert!(event_text.contains(r#""guidance":"workspace相対で再実行せよ: outside""#));
        assert!(!event_text.contains(r#""event":"workspace_cd_stripped""#));
    }

    #[test]
    fn bash_normalized_inspection_command_emits_eval_event() {
        let registry = ToolRegistry::default();
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src/app")).unwrap();
        std::fs::create_dir_all(dir.path().join("node_modules/pkg")).unwrap();
        std::fs::write(dir.path().join("src/app/page.tsx"), "page").unwrap();
        std::fs::write(dir.path().join("node_modules/pkg/index.js"), "pkg").unwrap();
        let events = dir.path().join("events.jsonl");
        let context = ToolContext {
            root: dir.path().to_path_buf(),
            mode: ExecutionMode::Act,
            auto_approve: true,
            interactive_approval: false,
            offline: false,
            workspace_policy: WorkspacePolicy::NormalTask,
            eval_events_path: Some(events.clone()),
            expected_paths: Vec::new(),
        };

        let output = registry
            .execute("Bash", &json!({"command":"ls -R"}), &context)
            .unwrap();

        assert!(output.contains("./src/app"), "{output}");
        assert!(!output.contains("node_modules"), "{output}");
        let event_text = std::fs::read_to_string(events).unwrap();
        assert!(event_text.contains(r#""event":"inspect_command_normalized""#));
        assert!(event_text.contains(r#""original":"ls -R""#));
        assert!(event_text.contains("find . -maxdepth 3"));
    }

    #[test]
    fn bash_allows_system_prefixes_and_workspace_absolute_paths() {
        let registry = ToolRegistry::default();
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("package.json"), "{}").unwrap();
        let context = ToolContext {
            root: dir.path().to_path_buf(),
            mode: ExecutionMode::Act,
            auto_approve: true,
            interactive_approval: false,
            offline: false,
            workspace_policy: WorkspacePolicy::NormalTask,
            eval_events_path: None,
            expected_paths: Vec::new(),
        };
        let package = dir.path().join("package.json");

        registry
            .execute("Bash", &json!({"command":"test -e /bin/sh"}), &context)
            .unwrap();
        registry
            .execute(
                "Bash",
                &json!({"command": format!("test -f {}", package.display())}),
                &context,
            )
            .unwrap();
    }

    #[test]
    fn classifies_recoverable_validation_errors() {
        let err = required_string(&json!({}), "pattern").unwrap_err();
        assert_eq!(tool_error_kind(&err), "missing_arg");
        assert!(recoverable_tool_error(&err));
        assert_eq!(missing_arg_name(&err).as_deref(), Some("pattern"));
    }

    #[test]
    fn classifies_bash_command_timeout_as_recoverable() {
        let err = anyhow::anyhow!("command_timeout: sleep 999");
        assert_eq!(tool_error_kind(&err), "command_timeout");
        assert!(recoverable_tool_error(&err));
    }

    #[test]
    fn normal_workspace_policy_blocks_anvil_metadata_read() {
        let registry = ToolRegistry::default();
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".anvil")).unwrap();
        std::fs::write(dir.path().join(".anvil/session.json"), "{}").unwrap();
        let context = ToolContext {
            root: dir.path().to_path_buf(),
            mode: ExecutionMode::Act,
            auto_approve: true,
            interactive_approval: false,
            offline: false,
            workspace_policy: WorkspacePolicy::NormalTask,
            eval_events_path: None,
            expected_paths: Vec::new(),
        };
        let err = registry
            .execute("Read", &json!({"path":".anvil/session.json"}), &context)
            .unwrap_err();
        assert_eq!(tool_error_kind(&err), "workspace_policy_blocked");
        assert!(recoverable_tool_error(&err));
    }

    #[test]
    fn normal_task_blocks_anvil_consistently_for_read_glob_and_bash() {
        let registry = ToolRegistry::default();
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".anvil/plans")).unwrap();
        std::fs::write(dir.path().join(".anvil/plans/plan.yaml"), "secret").unwrap();
        let context = ToolContext {
            root: dir.path().to_path_buf(),
            mode: ExecutionMode::Act,
            auto_approve: true,
            interactive_approval: false,
            offline: false,
            workspace_policy: WorkspacePolicy::NormalTask,
            eval_events_path: None,
            expected_paths: Vec::new(),
        };

        for (tool, arguments) in [
            ("Read", json!({"path":".anvil/plans/plan.yaml"})),
            ("Glob", json!({"pattern":".anvil/plans/*.yaml"})),
            ("Bash", json!({"command":"ls .anvil/plans/"})),
        ] {
            let error = registry.execute(tool, &arguments, &context).unwrap_err();
            assert_eq!(tool_error_kind(&error), "workspace_policy_blocked");
            assert!(recoverable_tool_error(&error));
            let access = crate::tools::hidden_path::access_from_error(&error).unwrap();
            assert!(access.path.contains(".anvil/plans"), "{access:?}");
        }
    }

    #[test]
    fn glob_uses_ignore_walker() {
        let registry = ToolRegistry::default();
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("node_modules/pkg")).unwrap();
        std::fs::write(dir.path().join("node_modules/pkg/index.js"), "").unwrap();
        std::fs::write(dir.path().join("my-node_modules-note.md"), "").unwrap();
        let context = ToolContext {
            root: dir.path().to_path_buf(),
            mode: ExecutionMode::Plan,
            auto_approve: true,
            interactive_approval: false,
            offline: false,
            workspace_policy: WorkspacePolicy::NormalTask,
            eval_events_path: None,
            expected_paths: Vec::new(),
        };
        let output = registry
            .execute("Glob", &json!({"pattern":"**/*"}), &context)
            .unwrap();
        assert!(!output.contains("node_modules/pkg/index.js"));
        assert!(output.contains("my-node_modules-note.md"));
    }

    #[test]
    fn workdir_prefix_path_miss_is_recoverable_with_hint() {
        let registry = ToolRegistry::default();
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "ok").unwrap();
        let context = ToolContext {
            root: dir.path().to_path_buf(),
            mode: ExecutionMode::Act,
            auto_approve: true,
            interactive_approval: false,
            offline: false,
            workspace_policy: WorkspacePolicy::NormalTask,
            eval_events_path: None,
            expected_paths: Vec::new(),
        };
        let err = registry
            .execute("Read", &json!({"path":"workdir/a.txt"}), &context)
            .unwrap_err();
        assert_eq!(tool_error_kind(&err), "path_not_found_recoverable");
        assert!(recoverable_tool_error(&err));
    }

    #[test]
    fn absolute_path_escape_remains_hard_failure() {
        let err = crate::tools::path_guard::resolve_existing(
            tempfile::tempdir().unwrap().path(),
            "/etc/passwd",
        )
        .unwrap_err();
        assert!(!recoverable_tool_error(&err));
    }

    #[test]
    fn recoverable_provider_aliases_are_executed() {
        let registry = ToolRegistry::default();
        let dir = tempfile::tempdir().unwrap();
        let context = ToolContext {
            root: dir.path().to_path_buf(),
            mode: ExecutionMode::Act,
            auto_approve: true,
            interactive_approval: false,
            offline: false,
            workspace_policy: WorkspacePolicy::NormalTask,
            eval_events_path: None,
            expected_paths: Vec::new(),
        };
        registry
            .execute(
                "Write",
                &json!({"arguments":{"file":"notes.txt","body":"ok"}}),
                &context,
            )
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.path().join("notes.txt")).unwrap(),
            "ok"
        );
    }

    #[test]
    fn unsafe_alias_path_is_not_recoverable() {
        let registry = ToolRegistry::default();
        let dir = tempfile::tempdir().unwrap();
        let context = ToolContext {
            root: dir.path().to_path_buf(),
            mode: ExecutionMode::Act,
            auto_approve: true,
            interactive_approval: false,
            offline: false,
            workspace_policy: WorkspacePolicy::NormalTask,
            eval_events_path: None,
            expected_paths: Vec::new(),
        };
        let err = registry
            .execute(
                "Write",
                &json!({"arguments":{"file":"../secret.txt","body":"no"}}),
                &context,
            )
            .unwrap_err();
        assert_eq!(tool_error_kind(&err), "path_confinement_error");
        assert!(!recoverable_tool_error(&err));
    }
}
