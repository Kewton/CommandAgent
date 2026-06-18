use crate::safety::path_guard::PathGuard;
use crate::{agent::minimal_loop::prompt::xml_tool_call_format, providers::ToolCallMode};

pub(crate) fn future_action_feedback() -> String {
    "You described a future tool action without calling a tool. If you need to create, edit, read, run, or verify something, call the tool now. Final answers must describe completed work, not planned next steps.".to_string()
}

pub(crate) fn action_required_feedback(mode: ToolCallMode) -> String {
    let base = "This step requires concrete repository evidence, but no file-changing tool call has run yet. Call exactly one appropriate tool now, such as Write or Edit for a required file change, or finish only if the required evidence already exists.";
    if mode == ToolCallMode::XmlFallback {
        format!(
            "{base}\nWhen a file change is required, emit one complete XML fallback tool call like this:\n{}",
            xml_tool_call_format()
        )
    } else {
        base.to_string()
    }
}

pub(crate) fn requested_artifact_feedback(missing: &[String], mode: ToolCallMode) -> String {
    let mut feedback = format!(
        "The requested artifact paths are still missing:\n{}\nCreate the missing paths with Write/Edit now, or explain why they are not required.",
        missing
            .iter()
            .map(|path| format!("- {path}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
    if mode == ToolCallMode::XmlFallback {
        feedback.push_str(
            "\nWhen creating a missing path, emit one complete XML fallback tool call like this:\n",
        );
        feedback.push_str(xml_tool_call_format());
    }
    feedback
}

pub(crate) fn missing_artifacts(guard: &PathGuard, expected_artifacts: &[String]) -> Vec<String> {
    expected_artifacts
        .iter()
        .filter(|path| match guard.resolve(path.as_str()) {
            Ok(resolved) => !resolved.exists(),
            Err(_) => true,
        })
        .cloned()
        .collect()
}

pub(crate) fn is_file_change_tool(name: &str) -> bool {
    matches!(name, "Write" | "Edit")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn missing_artifacts_reports_only_absent_paths() {
        let root = temp_workspace("missing");
        fs::write(root.join("present.txt"), "ok").unwrap();
        let guard = PathGuard::new(&root).unwrap();

        let missing = missing_artifacts(
            &guard,
            &["present.txt".to_string(), "missing.txt".to_string()],
        );

        assert_eq!(missing, vec!["missing.txt"]);
    }

    #[test]
    fn file_change_tool_detection_is_narrow() {
        assert!(is_file_change_tool("Write"));
        assert!(is_file_change_tool("Edit"));
        assert!(!is_file_change_tool("Read"));
        assert!(!is_file_change_tool("Bash"));
    }

    #[test]
    fn xml_feedback_includes_fallback_example() {
        assert!(
            action_required_feedback(ToolCallMode::XmlFallback).contains("commandagent_tool_call")
        );
        assert!(
            requested_artifact_feedback(&["dist/report.md".to_string()], ToolCallMode::XmlFallback)
                .contains("\"args\"")
        );
    }

    fn temp_workspace(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "commandagent-minimal-guard-{}-{}",
            name,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }
}
