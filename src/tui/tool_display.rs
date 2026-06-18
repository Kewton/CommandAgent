use crate::tui::progress::{sanitize_for_progress, truncate_chars};
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolDisplay {
    pub action: String,
    pub target: Option<String>,
    pub note: Option<String>,
}

impl ToolDisplay {
    pub fn compact(&self) -> String {
        let mut parts = vec![self.action.clone()];
        if let Some(target) = &self.target {
            parts.push(target.clone());
        }
        if let Some(note) = &self.note {
            parts.push(format!("({note})"));
        }
        parts.join(" ")
    }
}

pub fn summarize_tool(tool_name: &str, args: &Value, cwd: &Path) -> ToolDisplay {
    match tool_name {
        "Read" => ToolDisplay {
            action: "Read".to_string(),
            target: path_arg(args, cwd),
            note: None,
        },
        "Write" => {
            let content = str_arg(args, "content");
            ToolDisplay {
                action: "Write".to_string(),
                target: path_arg(args, cwd),
                note: Some(format!("{}B {}", content.len(), preview(content, 48))),
            }
        }
        "Edit" => ToolDisplay {
            action: "Edit".to_string(),
            target: path_arg(args, cwd),
            note: Some(preview(str_arg(args, "new"), 48)),
        },
        "Bash" => ToolDisplay {
            action: "Bash".to_string(),
            target: Some(truncate_chars(
                &sanitize_for_progress(str_arg(args, "command")),
                80,
            )),
            note: None,
        },
        "Glob" => ToolDisplay {
            action: "Glob".to_string(),
            target: Some(truncate_chars(
                &sanitize_for_progress(str_arg(args, "pattern")),
                80,
            )),
            note: None,
        },
        "Grep" => ToolDisplay {
            action: "Grep".to_string(),
            target: Some(truncate_chars(
                &sanitize_for_progress(str_arg(args, "pattern")),
                80,
            )),
            note: None,
        },
        other => ToolDisplay {
            action: sanitize_for_progress(other),
            target: None,
            note: None,
        },
    }
}

fn path_arg(args: &Value, cwd: &Path) -> Option<String> {
    let raw = str_arg(args, "path");
    if raw.is_empty() {
        return None;
    }

    Some(display_path(raw, cwd))
}

fn display_path(raw: &str, cwd: &Path) -> String {
    let path = PathBuf::from(raw);
    let relative = if path.is_absolute() {
        path.strip_prefix(cwd)
            .map(Path::to_path_buf)
            .unwrap_or(path)
    } else {
        path
    };
    truncate_chars(&sanitize_for_progress(&relative.display().to_string()), 96)
}

fn str_arg<'a>(args: &'a Value, key: &str) -> &'a str {
    args.get(key).and_then(Value::as_str).unwrap_or_default()
}

fn preview(value: &str, max: usize) -> String {
    truncate_chars(&sanitize_for_progress(value), max)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn summarizes_write_with_relative_path_and_size() {
        let cwd = Path::new("/tmp/project");
        let summary = summarize_tool(
            "Write",
            &json!({"path":"/tmp/project/package.json","content":"{\"ok\":true}"}),
            cwd,
        );

        assert_eq!(summary.action, "Write");
        assert_eq!(summary.target.as_deref(), Some("package.json"));
        assert!(summary.note.unwrap().starts_with("11B"));
    }

    #[test]
    fn summarizes_edit_with_new_preview() {
        let summary = summarize_tool(
            "Edit",
            &json!({"path":"src/main.rs","old":"a","new":"hello\nworld"}),
            Path::new("/tmp/project"),
        );

        assert_eq!(summary.compact(), "Edit src/main.rs (hello world)");
    }

    #[test]
    fn summarizes_command_and_search_tools() {
        let cwd = Path::new("/tmp/project");
        assert_eq!(
            summarize_tool("Bash", &json!({"command":"cargo test\nrm"}), cwd).compact(),
            "Bash cargo test rm"
        );
        assert_eq!(
            summarize_tool("Grep", &json!({"pattern":"fn main"}), cwd).compact(),
            "Grep fn main"
        );
    }

    #[test]
    fn handles_malformed_arguments() {
        let summary = summarize_tool("Read", &json!({}), Path::new("/tmp/project"));

        assert_eq!(summary.compact(), "Read");
    }
}
