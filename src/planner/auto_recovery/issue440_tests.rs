use super::*;

#[derive(Clone)]
struct CappedClient;

impl ChatClient for CappedClient {
    fn label(&self) -> &str {
        "capped-fixture"
    }
    fn boxed_clone(&self) -> Box<dyn ChatClient> {
        Box::new(self.clone())
    }
    fn chat(
        &mut self,
        _model: &str,
        _messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _native: bool,
    ) -> anyhow::Result<AssistantReply> {
        Ok(AssistantReply {
            content: String::new(),
            tool_calls: Vec::new(),
            prompt_tokens: None,
            completion_tokens: Some(8192),
        })
    }
}

#[test]
fn max_length_stop_registers_typed_recovery_candidate() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = config(dir.path(), 1);
    config.num_predict = 8192;
    let result = capture_attempt(&crate::tui::NOOP_UI, || {
        crate::minimal_loop::run_session(
            &mut CappedClient,
            &mut crate::state::SessionSnapshot::new(),
            "Repair inventory UI",
            &config,
        )
    });
    assert!(
        result
            .result
            .unwrap_err()
            .to_string()
            .contains("max_length_no_tool_call_repeated")
    );
    let Some(AttemptFailure::Recoverable(candidate)) = result.failure else {
        panic!("early stop did not provide a typed Recovery candidate");
    };
    assert_eq!(
        candidate.handoff.failure_kind,
        "max_length_no_tool_call_repeated"
    );
    assert!(candidate.path.exists());
    assert!(candidate.handoff.failure_evidence[0].contains("2 output-limit responses"));
    assert!(candidate.handoff.failure_evidence[0].contains("total_duration_ms="));
    assert_eq!(candidate.verify_command_source, "failure_handoff");
}
