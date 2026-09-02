use std::path::{Path, PathBuf};

use anyhow::bail;
use serde_json::{Value, json};

pub(crate) fn from_contract(
    contract: Option<&super::completion::CompletionContract>,
) -> Vec<String> {
    contract
        .map(|contract| contract.protected_paths.clone())
        .unwrap_or_default()
}

pub(crate) fn enforce_tool_mutation(
    root: &Path,
    event_path: Option<&Path>,
    tool: &str,
    arguments: &Value,
    protected_paths: &[String],
) -> anyhow::Result<()> {
    let protected = match tool {
        "Write" | "Edit" => arguments
            .get("path")
            .and_then(Value::as_str)
            .and_then(|path| matching_path(root, path, protected_paths)),
        "Bash" => arguments
            .get("command")
            .and_then(Value::as_str)
            .and_then(|command| {
                crate::tools::bash_write_guard::protected_path_mutation(
                    command,
                    root,
                    protected_paths,
                )
            }),
        _ => None,
    };
    let Some(protected) = protected else {
        return Ok(());
    };
    crate::eval_events::emit(
        event_path,
        json!({
            "event": "protected_path_mutation_rejected",
            "tool": tool,
            "protected_path": protected,
            "policy_source": "completion_contract",
        }),
    );
    bail!(
        "protected_path_mutation_rejected: `{protected}` is a frozen verification input; read or execute it, but repair an unprotected implementation artifact instead"
    )
}

fn matching_path(root: &Path, raw: &str, protected_paths: &[String]) -> Option<String> {
    let candidate = Path::new(raw);
    let relative = if candidate.is_absolute() {
        match candidate.strip_prefix(root) {
            Ok(relative) => relative.to_path_buf(),
            Err(_) => {
                return protected_paths
                    .iter()
                    .find(|protected| candidate.ends_with(protected))
                    .cloned();
            }
        }
    } else {
        normalize_relative(candidate)?
    };
    protected_paths
        .iter()
        .find(|protected| path_matches(&relative, Path::new(protected)))
        .cloned()
}

fn normalize_relative(path: &Path) -> Option<PathBuf> {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::Normal(part) => normalized.push(part),
            _ => return None,
        }
    }
    Some(normalized)
}

fn path_matches(candidate: &Path, protected: &Path) -> bool {
    candidate == protected || candidate.starts_with(protected) || protected.starts_with(candidate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_direct_mutation_but_allows_reads_and_implementation_writes() {
        let root = Path::new("/tmp/work");
        let protected = vec!["scripts/repro.py".to_string(), "tests".to_string()];
        assert!(
            enforce_tool_mutation(
                root,
                None,
                "Edit",
                &json!({"path": "scripts/repro.py"}),
                &protected,
            )
            .is_err()
        );
        assert!(
            enforce_tool_mutation(
                root,
                None,
                "Write",
                &json!({"path": "pipeline/main.py"}),
                &protected,
            )
            .is_ok()
        );
        assert!(
            enforce_tool_mutation(
                root,
                None,
                "Write",
                &json!({"path": "/stale/shared-run/before/scripts/repro.py"}),
                &protected,
            )
            .is_err()
        );
        assert!(
            enforce_tool_mutation(
                root,
                None,
                "Read",
                &json!({"path": "scripts/repro.py"}),
                &protected,
            )
            .is_ok()
        );
    }

    #[test]
    fn rejects_recognized_bash_mutation_of_protected_directory() {
        let protected = vec!["tests".to_string()];
        let error = enforce_tool_mutation(
            Path::new("/tmp/work"),
            None,
            "Bash",
            &json!({"command": "rm tests/test_pipeline.py"}),
            &protected,
        )
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("protected_path_mutation_rejected")
        );
    }
}
