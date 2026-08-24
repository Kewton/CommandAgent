use std::fmt;
use std::path::{Component, Path};

use serde_json::Value;

use super::workspace_policy::WorkspacePolicy;

pub const ENGINE_PRIVATE_COMPONENT: &str = ".commandagent";
pub const LEGACY_ENGINE_PRIVATE_COMPONENT: &str = ".anvil";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HiddenPathAccess {
    pub path: String,
}

impl fmt::Display for HiddenPathAccess {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "workspace_policy_blocked: hidden path `{}` is engine-private metadata",
            self.path
        )
    }
}

impl std::error::Error for HiddenPathAccess {}

pub fn access_from_error(error: &anyhow::Error) -> Option<&HiddenPathAccess> {
    error.downcast_ref::<HiddenPathAccess>()
}

pub fn ensure_reference_allowed(
    root: &Path,
    text: &str,
    policy: WorkspacePolicy,
) -> anyhow::Result<()> {
    if policy.allows_component(ENGINE_PRIVATE_COMPONENT) {
        return Ok(());
    }
    if let Some(path) = referenced_path(root, text) {
        return Err(anyhow::Error::new(HiddenPathAccess { path }));
    }
    Ok(())
}

pub fn path_error(root: &Path, path: &Path) -> anyhow::Error {
    let relative = path.strip_prefix(root).unwrap_or(path);
    anyhow::Error::new(HiddenPathAccess {
        path: normalize_display_path(relative),
    })
}

pub fn tool_arguments_reference_hidden(name: &str, arguments: &Value, root: &Path) -> bool {
    let key = match name {
        "Bash" => "command",
        "Read" | "Write" | "Edit" => "path",
        "Glob" => "pattern",
        "Grep" => "glob",
        _ => return false,
    };
    arguments
        .get(key)
        .and_then(Value::as_str)
        .and_then(|text| referenced_path(root, text))
        .is_some()
}

pub fn referenced_path(root: &Path, text: &str) -> Option<String> {
    text.split(is_shell_boundary).find_map(|token| {
        let token = token.trim_matches(is_token_wrapper);
        if token.is_empty() || !has_private_component(Path::new(token)) {
            return None;
        }
        let path = Path::new(token);
        let relative = path.strip_prefix(root).unwrap_or(path);
        Some(normalize_display_path(relative))
    })
}

fn has_private_component(path: &Path) -> bool {
    path.components().any(|component| {
        matches!(component, Component::Normal(value) if is_engine_private_component(value))
    })
}

pub fn is_engine_private_component(component: &std::ffi::OsStr) -> bool {
    component == ENGINE_PRIVATE_COMPONENT || component == LEGACY_ENGINE_PRIVATE_COMPONENT
}

fn normalize_display_path(path: &Path) -> String {
    let display = path.to_string_lossy().replace('\\', "/");
    display.strip_prefix("./").unwrap_or(&display).to_string()
}

fn is_shell_boundary(character: char) -> bool {
    character.is_whitespace() || matches!(character, '|' | '&' | ';' | '<' | '>' | '\n' | '\r')
}

fn is_token_wrapper(character: char) -> bool {
    matches!(
        character,
        '\'' | '"' | '`' | '(' | ')' | '[' | ']' | '{' | '}' | ','
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extracts_relative_and_workspace_absolute_private_paths() {
        let root = Path::new("/tmp/work");
        assert_eq!(
            referenced_path(root, "ls -la .anvil/plans/"),
            Some(".anvil/plans/".to_string())
        );
        assert_eq!(
            referenced_path(root, "cat /tmp/work/.anvil/plans/plan.yaml"),
            Some(".anvil/plans/plan.yaml".to_string())
        );
        assert_eq!(referenced_path(root, "ls notes/.anvilish"), None);
    }

    #[test]
    fn recognizes_task_tool_path_arguments() {
        let root = Path::new("/tmp/work");
        assert!(tool_arguments_reference_hidden(
            "Read",
            &json!({"path":".anvil/plans/plan.yaml"}),
            root
        ));
        assert!(tool_arguments_reference_hidden(
            "Bash",
            &json!({"command":"ls .anvil/plans/"}),
            root
        ));
        assert!(!tool_arguments_reference_hidden(
            "Glob",
            &json!({"pattern":"data/**/*.csv"}),
            root
        ));
    }
}
