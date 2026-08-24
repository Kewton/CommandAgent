use std::path::Path;

use serde_json::json;

use crate::eval_events;

pub const ENGINE_PRIVATE_GUIDANCE: &str = ".anvil はエンジン私有のメタデータであり、タスクツールから参照できない。現在のフェーズと計画はプロンプトに含まれている。";

pub(crate) fn emit_for_error(
    events_path: Option<&Path>,
    profile: &str,
    tool: &str,
    error: &anyhow::Error,
    attempt: usize,
) -> Option<String> {
    let access = crate::tools::hidden_path::access_from_error(error)?;
    let continuation = crate::planner::profile::resolve_profile_runtime(profile)
        .hidden_path_continuation()
        .unwrap_or("");
    let feedback = if continuation.is_empty() {
        ENGINE_PRIVATE_GUIDANCE.to_string()
    } else {
        format!("{ENGINE_PRIVATE_GUIDANCE}{continuation}")
    };
    eval_events::emit(
        events_path,
        json!({
            "event": "hidden_path_feedback",
            "path": access.path,
            "tool": tool,
            "attempt": attempt,
            "profile": profile,
            "guidance": feedback,
        }),
    );
    Some(feedback)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_feedback_joins_core_policy_and_manifest_continuation() {
        let root = Path::new("/tmp/work");
        let error = crate::tools::hidden_path::path_error(
            root,
            Path::new("/tmp/work/.anvil/plans/plan.yaml"),
        );

        let feedback = emit_for_error(None, "data", "Read", &error, 1).unwrap();

        assert!(feedback.starts_with(ENGINE_PRIVATE_GUIDANCE));
        assert!(feedback.contains("data/ の入力から作業を続け"));
        assert!(feedback.contains("output/inspection.json を作成せよ"));
    }

    #[test]
    fn feedback_event_records_path_tool_and_attempt() {
        let dir = tempfile::tempdir().unwrap();
        let events = dir.path().join("events.jsonl");
        let error = crate::tools::hidden_path::path_error(
            dir.path(),
            &dir.path().join(".anvil/plans/plan.yaml"),
        );

        emit_for_error(Some(&events), "data", "Read", &error, 2).unwrap();

        let event: serde_json::Value =
            serde_json::from_str(std::fs::read_to_string(events).unwrap().trim()).unwrap();
        assert_eq!(event["event"], "hidden_path_feedback");
        assert_eq!(event["path"], ".anvil/plans/plan.yaml");
        assert_eq!(event["tool"], "Read");
        assert_eq!(event["attempt"], 2);
    }
}
