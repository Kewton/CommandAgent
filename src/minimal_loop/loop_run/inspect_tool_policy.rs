use super::RunSessionStepKind;

pub(super) fn mutation_rejection(
    step_kind: Option<RunSessionStepKind>,
    tool_name: &str,
    bash_command: Option<&str>,
) -> Option<&'static str> {
    if step_kind != Some(RunSessionStepKind::Inspect) {
        return None;
    }
    if matches!(tool_name, "Write" | "Edit") {
        return Some("inspect steps cannot create or edit workspace files");
    }
    if tool_name == "Bash"
        && bash_command.is_some_and(crate::tools::bash::has_recognized_workspace_mutation)
    {
        return Some("inspect steps cannot run a shell command with recognized write effects");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::{AssistantReply, ChatClient};
    use crate::state::{ConversationMessage, SessionSnapshot, ToolCall};
    use crate::tools::registry::ToolSpec;
    use clap::Parser;
    use serde_json::json;
    use std::sync::{Arc, Mutex};

    #[derive(Clone)]
    struct Fake {
        replies: Arc<Mutex<Vec<AssistantReply>>>,
    }

    impl ChatClient for Fake {
        fn label(&self) -> &str {
            "inspect-policy-fake"
        }

        fn boxed_clone(&self) -> Box<dyn ChatClient> {
            Box::new(self.clone())
        }

        fn supports_native_tools(&self, _model: &str) -> bool {
            true
        }

        fn chat(
            &mut self,
            _model: &str,
            _messages: &[ConversationMessage],
            _tools: &[ToolSpec],
            _native_tools_enabled: bool,
        ) -> anyhow::Result<AssistantReply> {
            Ok(self.replies.lock().unwrap().remove(0))
        }
    }

    #[test]
    fn inspect_rejects_mutations_but_keeps_read_only_tools() {
        assert!(mutation_rejection(Some(RunSessionStepKind::Inspect), "Write", None).is_some());
        assert!(
            mutation_rejection(
                Some(RunSessionStepKind::Inspect),
                "Bash",
                Some("printf x > app/page.js")
            )
            .is_some()
        );
        assert_eq!(
            mutation_rejection(Some(RunSessionStepKind::Inspect), "Read", None),
            None
        );
        assert_eq!(
            mutation_rejection(
                Some(RunSessionStepKind::Inspect),
                "Bash",
                Some("test -f app/page.js")
            ),
            None
        );
        assert_eq!(
            mutation_rejection(Some(RunSessionStepKind::Implement), "Write", None),
            None
        );
    }

    #[test]
    fn inspect_session_rejects_write_and_preserves_source() {
        let dir = tempfile::tempdir().unwrap();
        let events = dir.path().join("events.jsonl");
        std::fs::write(dir.path().join("app.txt"), "stable\n").unwrap();
        let mut config = crate::config::Config::from_cli(crate::cli::Cli::parse_from([
            "commandagent",
            "--ux-demo",
        ]))
        .unwrap();
        config.workspace_root = dir.path().to_path_buf();
        config.eval_events_path = Some(events.clone());
        config.max_iterations = 3;
        let mut fake = Fake {
            replies: Arc::new(Mutex::new(vec![
                AssistantReply {
                    content: String::new(),
                    tool_calls: vec![ToolCall::new(
                        "Write",
                        json!({"path":"app.txt","content":"mutated\n"}),
                    )],
                    prompt_tokens: None,
                    completion_tokens: None,
                },
                AssistantReply::text("inspection complete"),
            ])),
        };
        let mut session = SessionSnapshot::new();

        let outcome = super::super::run_session_with_outcome_with_options(
            &mut fake,
            &mut session,
            "Inspect app.txt without modifying it.",
            &[],
            &config,
            &crate::tui::NOOP_UI,
            super::super::RunSessionOptions::plan_step(RunSessionStepKind::Inspect),
        )
        .unwrap();

        assert_eq!(
            outcome.stop_reason,
            super::super::RunStopReason::AssistantFinal
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join("app.txt")).unwrap(),
            "stable\n"
        );
        assert!(
            std::fs::read_to_string(events)
                .unwrap()
                .contains("inspect_mutation_tool_rejected")
        );
    }
}
