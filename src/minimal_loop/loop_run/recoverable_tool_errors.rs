use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::tools::registry::tool_error_kind;

#[derive(Debug, Default)]
pub(super) struct RecoverableToolErrorState {
    key: Option<String>,
    repeats: usize,
    // This state lives for one run_session, like the existing repeat limit.
    // Unrelated successes and different paths cannot erase unresolved misses.
    missing_reads: BTreeMap<PathBuf, usize>,
}

impl RecoverableToolErrorState {
    pub(super) fn record(&mut self, tool_name: &str, err: &anyhow::Error) -> usize {
        if let Some(missing) = crate::tools::read_missing::from_error(err) {
            let count = self.missing_reads.entry(missing.path.clone()).or_default();
            *count += 1;
            return *count;
        }
        if crate::tools::placeholder_path::is_placeholder_rejection(tool_name, err) {
            return 0;
        }
        let kind = tool_error_kind(err);
        let key = if let Some(access) = crate::tools::hidden_path::access_from_error(err) {
            format!("hidden_path:{}", access.path)
        } else if kind == "command_timeout" {
            format!(
                "{tool_name}:{kind}:{}",
                super::command_timeout_similarity_key(&err.to_string())
            )
        } else {
            format!("{tool_name}:{kind}:{err}")
        };
        if self.key.as_deref() == Some(key.as_str()) {
            self.repeats += 1;
        } else {
            self.key = Some(key);
            self.repeats = 1;
        }
        self.repeats
    }

    pub(super) fn reset(&mut self) {
        self.key = None;
        self.repeats = 0;
    }

    pub(super) fn note_success(&mut self, root: &Path, tool: &str, arguments: &Value) {
        if tool != "Read" || self.missing_reads.is_empty() {
            return;
        }
        let Some(raw) = arguments.get("path").and_then(Value::as_str) else {
            return;
        };
        let Ok(normalization) = crate::tools::path_guard::normalize_workspace_path(root, raw)
        else {
            return;
        };
        let path = normalization
            .as_ref()
            .map_or(raw, |value| value.relative.as_str());
        if let Ok(path) = crate::tools::path_guard::resolve_existing(root, path) {
            self.missing_reads.remove(&path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mode::ExecutionMode;
    use crate::tools::registry::{ToolContext, ToolRegistry};
    use crate::tools::workspace_policy::WorkspacePolicy;
    use serde_json::json;

    #[test]
    fn only_a_successful_read_of_the_missing_target_resolves_its_count() {
        let root = tempfile::tempdir().unwrap();
        let context = ToolContext {
            root: root.path().into(),
            mode: ExecutionMode::Act,
            auto_approve: true,
            interactive_approval: false,
            offline: true,
            workspace_policy: WorkspacePolicy::NormalTask,
            eval_events_path: None,
            expected_paths: vec![],
            protected_paths: vec![],
        };
        let registry = ToolRegistry::default();
        let mut state = RecoverableToolErrorState::default();
        let missing = registry
            .execute("Read", &json!({"path":"a"}), &context)
            .unwrap_err();
        assert_eq!(state.record("Read", &missing), 1);
        registry
            .execute("Write", &json!({"path":"a", "content":"created"}), &context)
            .unwrap();
        state.reset();
        state.note_success(root.path(), "Write", &json!({"path":"a"}));
        std::fs::remove_file(root.path().join("a")).unwrap();
        let missing = registry
            .execute("Read", &json!({"path":"./a"}), &context)
            .unwrap_err();
        assert_eq!(state.record("Read", &missing), 2);
        registry
            .execute(
                "Write",
                &json!({"path":"a", "content":"recreated"}),
                &context,
            )
            .unwrap();
        registry
            .execute("Read", &json!({"path":"./a"}), &context)
            .unwrap();
        state.note_success(root.path(), "Read", &json!({"path":"./a"}));
        std::fs::remove_file(root.path().join("a")).unwrap();
        let missing = registry
            .execute("Read", &json!({"path":"a"}), &context)
            .unwrap_err();
        assert_eq!(state.record("Read", &missing), 1);
    }
}
