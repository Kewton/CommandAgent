#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::providers::AssistantReply;
    use std::sync::{Arc, Mutex};

    const GOOD: &str = "def fixed():\n    return 1\n";
    const BROKEN: &str = "def broken(:\n    pass\n";

    #[derive(Clone)]
    struct Script {
        replies: Arc<Mutex<Vec<AssistantReply>>>,
        requests: Arc<Mutex<Vec<Vec<ConversationMessage>>>>,
    }

    impl Script {
        fn new(replies: Vec<AssistantReply>) -> Self {
            Self {
                replies: Arc::new(Mutex::new(replies)),
                requests: Arc::new(Mutex::new(Vec::new())),
            }
        }
    }

    impl ChatClient for Script {
        fn label(&self) -> &str {
            "contract-outcome-script"
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
            messages: &[ConversationMessage],
            _tools: &[crate::tools::registry::ToolSpec],
            _native_tools_enabled: bool,
        ) -> anyhow::Result<AssistantReply> {
            self.requests.lock().unwrap().push(messages.to_vec());
            let mut replies = self.replies.lock().unwrap();
            anyhow::ensure!(!replies.is_empty(), "script exhausted");
            Ok(replies.remove(0))
        }
    }

    fn write_reply(content: &str) -> AssistantReply {
        AssistantReply {
            content: String::new(),
            tool_calls: vec![ToolCall::new(
                "Write",
                json!({"path":"a.py","content":content}),
            )],
            prompt_tokens: None,
            completion_tokens: None,
        }
    }

    fn config(root: std::path::PathBuf) -> Config {
        Config {
            workspace_root: root,
            state_dir: std::path::PathBuf::from("state"),
            yes: true,
            offline: false,
            context_budget: 1000,
            model: "m".to_string(),
            provider: crate::config::Provider::Ollama,
            tool_protocol: None,
            openai_api: crate::config::OpenAiApi::ChatCompletions,
            prompt_layout: crate::config::PromptLayout::Stable,
            plan_preset: crate::config::PlanPreset::None,
            intent_override: None,
            planner_model: "m".to_string(),
            planner_provider: crate::config::Provider::Ollama,
            planner_think: Some(crate::config::OllamaThink::False),
            classifier_model: "m".to_string(),
            classifier_provider: crate::config::Provider::Ollama,
            openai_compatible: None,
            ollama_host: "http://localhost:11434".to_string(),
            ollama_think: None,
            lm_studio_host: "http://localhost:1234".to_string(),
            num_predict: 100,
            max_iterations: 6,
            recovery_plan_auto_runs: 0,
            chat_timeout_secs: 1,
            chat_timeout_source: "override:test".to_string(),
            field_sources: crate::config::ConfigFieldSources::default(),
            chat_retries: 1,
            stream: false,
            eval_events_path: None,
            completion_contract_path: None,
            resume: None,
            fresh_session: false,
            no_footer: false,
            narration: crate::config::NarrationMode::Normal,
            profile: "generic".to_string(),
            profile_explicit: false,
            profile_inference: None,
            style: "default".to_string(),
            action: crate::config::Action::Repl,
        }
    }

    struct Run {
        result: anyhow::Result<RunSessionOutcome>,
        session: SessionSnapshot,
        requests: Vec<Vec<ConversationMessage>>,
        events: Vec<Value>,
    }

    /// Runs one scripted session against `a.py` guarded by a py_compile
    /// contract. `existing` pre-creates `a.py` so the text-only final reply
    /// reaches the assistant-final branch instead of the tool-batch branch.
    fn run(
        existing: Option<&str>,
        prompt: &str,
        cap: usize,
        enforcement: Option<ContractEnforcement>,
        replies: Vec<AssistantReply>,
    ) -> Run {
        let dir = tempfile::tempdir().unwrap();
        if let Some(content) = existing {
            std::fs::write(dir.path().join("a.py"), content).unwrap();
        }
        let contract = dir.path().join("contract.json");
        std::fs::write(
            &contract,
            format!(
                r#"{{"required_paths":["a.py"],"verify_commands":["python3 -m py_compile a.py"],"verify_repair_cap":{cap}}}"#
            ),
        )
        .unwrap();
        let mut cfg = config(dir.path().to_path_buf());
        cfg.completion_contract_path = Some(contract);
        let events_path = dir.path().join("events.jsonl");
        cfg.eval_events_path = Some(events_path.clone());
        let mut client = Script::new(replies);
        let mut session = SessionSnapshot::new();
        let options = match enforcement {
            Some(enforcement) => RunSessionOptions::plan_step_with_enforcement(
                RunSessionStepKind::Implement,
                enforcement,
                Some("phase-one".to_string()),
            ),
            None => RunSessionOptions::default(),
        };
        let result = run_session_with_outcome_with_options(
            &mut client,
            &mut session,
            prompt,
            &["a.py".to_string()],
            &cfg,
            &NOOP_UI,
            options,
        );
        let events = std::fs::read_to_string(&events_path)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .collect();
        let requests = client.requests.lock().unwrap().clone();
        Run {
            result,
            session,
            requests,
            events,
        }
    }

    fn names(run: &Run) -> Vec<String> {
        run.events
            .iter()
            .map(|event| match event["reason"].as_str() {
                Some(reason) => format!("{}:{reason}", event["event"].as_str().unwrap()),
                None => event["event"].as_str().unwrap().to_string(),
            })
            .collect()
    }

    fn roles(messages: &[ConversationMessage]) -> Vec<&str> {
        messages.iter().map(|m| m.role.as_str()).collect()
    }

    fn ok(run: &Run) -> &RunSessionOutcome {
        run.result.as_ref().unwrap()
    }

    fn without_elapsed(text: &str) -> String {
        text.lines()
            .filter(|line| !line.starts_with("elapsed_ms:"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn assert_satisfied_stop(run: &Run, attempts: usize) {
        let stop = run
            .events
            .iter()
            .find(|event| event["event"] == "loop_stop")
            .unwrap();
        assert_eq!(stop["reason"], "completion_contract_satisfied");
        assert_eq!(stop["required_paths"], json!(["a.py"]));
        assert_eq!(stop["verify_attempts"], attempts);
        assert_eq!(
            names(run).last().unwrap(),
            "loop_stop:completion_contract_satisfied"
        );
    }

    const PROVIDER: &str = "provider_turn_duration";
    const VERIFY: &str = "completion_verify";
    const SCOPE: &str = "step_obligation_scope";
    const TOOLS: [&str; 2] = ["tool_call_raw", "tool_execute"];

    fn sequence(parts: &[&[&str]]) -> Vec<String> {
        parts
            .iter()
            .flat_map(|p| p.iter().map(|s| s.to_string()))
            .collect()
    }

    #[test]
    fn tool_batch_satisfied_uses_synthesized_final_text() {
        let run = run(None, "create a.py", 2, None, vec![write_reply(GOOD)]);
        let outcome = ok(&run);
        assert_eq!(outcome.final_text, "completion contract satisfied: a.py");
        assert_eq!(
            outcome.stop_reason,
            RunStopReason::CompletionContractSatisfied
        );
        assert_eq!(outcome.changed_paths, vec!["a.py".to_string()]);
        assert_eq!((outcome.iterations, outcome.tool_calls), (1, 1));
        assert_eq!(outcome.verify_attempts, 1);
        assert_eq!(outcome.last_blocking_reason, None);
        assert!(outcome.missing_required_paths.is_empty());
        assert!(outcome.missing_capabilities.is_empty());
        assert!(outcome.missing_evidence.is_empty());
        assert!(outcome.missing_obligations.is_empty());
        assert_eq!(
            names(&run),
            sequence(&[
                &[SCOPE, PROVIDER],
                &TOOLS,
                &[VERIFY, "loop_stop:completion_contract_satisfied"],
            ])
        );
        assert_satisfied_stop(&run, 1);
    }

    #[test]
    fn tool_batch_repair_keeps_assistant_and_tool_messages() {
        let run = run(
            None,
            "create a.py",
            2,
            None,
            vec![write_reply(BROKEN), write_reply(GOOD)],
        );
        let outcome = ok(&run);
        assert_eq!(outcome.final_text, "completion contract satisfied: a.py");
        assert_eq!(outcome.verify_attempts, 2);
        assert_eq!((outcome.iterations, outcome.tool_calls), (2, 2));
        assert_eq!(
            outcome.last_blocking_reason.as_deref(),
            Some("completion verify failed")
        );
        assert_eq!(
            roles(&run.requests[1]),
            vec!["system", "user", "assistant", "tool", "user"]
        );
        assert!(run.requests[1][4].content.starts_with(
            "Deterministic completion verification failed. Fix the implementation and retry."
        ));
        assert_eq!(
            names(&run),
            sequence(&[
                &[SCOPE, PROVIDER],
                &TOOLS,
                &[VERIFY, PROVIDER],
                &TOOLS,
                &[
                    VERIFY,
                    "step_verify_repair",
                    "loop_stop:completion_contract_satisfied"
                ],
            ])
        );
        assert_satisfied_stop(&run, 2);
    }

    #[test]
    fn assistant_final_satisfied_returns_reply_text() {
        let run = run(
            Some(GOOD),
            "review a.py",
            2,
            Some(ContractEnforcement::Enforce),
            vec![AssistantReply::text("done")],
        );
        let outcome = ok(&run);
        assert_eq!(outcome.final_text, "done");
        assert_eq!(
            outcome.stop_reason,
            RunStopReason::CompletionContractSatisfied
        );
        assert!(outcome.changed_paths.is_empty());
        assert_eq!((outcome.iterations, outcome.tool_calls), (1, 0));
        assert_eq!(outcome.verify_attempts, 1);
        assert_eq!(outcome.last_blocking_reason, None);
        assert_eq!(roles(&run.session.messages), vec!["user", "assistant"]);
        assert_eq!(run.session.messages[1].content, "done");
        assert_eq!(
            names(&run),
            sequence(&[
                &[SCOPE, PROVIDER, VERIFY],
                &["loop_stop:completion_contract_satisfied"],
            ])
        );
        assert_satisfied_stop(&run, 1);
    }

    #[test]
    fn assistant_final_repair_pops_the_reply_before_feedback() {
        let run = run(
            Some(BROKEN),
            "review a.py",
            2,
            Some(ContractEnforcement::Enforce),
            vec![AssistantReply::text("done"), write_reply(GOOD)],
        );
        let outcome = ok(&run);
        assert_eq!(outcome.final_text, "completion contract satisfied: a.py");
        assert_eq!(outcome.verify_attempts, 2);
        assert_eq!((outcome.iterations, outcome.tool_calls), (2, 1));
        assert_eq!(
            outcome.last_blocking_reason.as_deref(),
            Some("completion verify failed")
        );
        assert_eq!(roles(&run.requests[1]), vec!["system", "user", "user"]);
        assert!(run.requests[1][2].content.starts_with(
            "Deterministic completion verification failed. Fix the implementation and retry."
        ));
        assert_eq!(
            names(&run),
            sequence(&[
                &[SCOPE, PROVIDER, VERIFY, PROVIDER],
                &TOOLS,
                &[
                    VERIFY,
                    "step_verify_repair",
                    "loop_stop:completion_contract_satisfied"
                ],
            ])
        );
        assert_satisfied_stop(&run, 2);
    }

    #[test]
    fn tool_batch_observation_incomplete_uses_reason_text() {
        let run = run(
            None,
            "create a.py",
            1,
            Some(ContractEnforcement::Observe),
            vec![write_reply(BROKEN)],
        );
        let outcome = ok(&run);
        assert_eq!(
            outcome.stop_reason,
            RunStopReason::CompletionContractObservedIncomplete
        );
        assert_eq!(
            without_elapsed(&outcome.final_text),
            "completion contract observed incomplete: command failed: python3 -m py_compile a.py\noutcome: CommandFailed\nstatus: exit status: 1\nsummary: SyntaxError: invalid syntax\nstdout:\n\nstderr:\n  File \"a.py\", line 1\n    def broken(:\n               ^\nSyntaxError: invalid syntax"
        );
        assert_eq!(outcome.changed_paths, vec!["a.py".to_string()]);
        assert_eq!((outcome.iterations, outcome.tool_calls), (1, 1));
        assert_eq!(outcome.verify_attempts, 1);
        assert_eq!(outcome.last_blocking_reason, None);
        assert!(outcome.missing_required_paths.is_empty());
        assert!(outcome.missing_evidence.is_empty());
        assert_eq!(
            names(&run),
            sequence(&[
                &[SCOPE, PROVIDER],
                &TOOLS,
                &[
                    VERIFY,
                    "contract_observation_incomplete:verify_repair_exhausted",
                    "loop_stop:contract_observation_incomplete",
                ],
            ])
        );
    }

    #[test]
    fn assistant_final_observation_incomplete_returns_reply_text() {
        let run = run(
            Some(BROKEN),
            "review a.py",
            1,
            Some(ContractEnforcement::Observe),
            vec![AssistantReply::text("done")],
        );
        let outcome = ok(&run);
        assert_eq!(outcome.final_text, "done");
        assert_eq!(
            outcome.stop_reason,
            RunStopReason::CompletionContractObservedIncomplete
        );
        assert!(outcome.changed_paths.is_empty());
        assert_eq!((outcome.iterations, outcome.tool_calls), (1, 0));
        assert_eq!(outcome.verify_attempts, 1);
        assert_eq!(outcome.last_blocking_reason, None);
        assert_eq!(roles(&run.session.messages), vec!["user", "assistant"]);
        assert_eq!(
            names(&run),
            sequence(&[
                &[SCOPE, PROVIDER, VERIFY],
                &[
                    "contract_observation_incomplete:verify_repair_exhausted",
                    "loop_stop:contract_observation_incomplete",
                ],
            ])
        );
    }

    #[test]
    fn exhausted_repair_cap_under_enforce_fails_at_both_sites() {
        let batch = run(None, "create a.py", 1, None, vec![write_reply(BROKEN)]);
        let reply = run(
            Some(BROKEN),
            "review a.py",
            1,
            Some(ContractEnforcement::Enforce),
            vec![AssistantReply::text("done")],
        );
        for run in [&batch, &reply] {
            let error = run.result.as_ref().unwrap_err().to_string();
            assert!(
                error.starts_with(
                    "completion contract verify failed after 1 attempts: command failed: python3 -m py_compile a.py"
                ),
                "{error}"
            );
            assert!(error.contains("SyntaxError: invalid syntax"), "{error}");
            let events = names(run);
            assert_eq!(
                &events[events.len() - 3..],
                [
                    VERIFY,
                    "recovery_prompt_saved",
                    "loop_stop:verify_repair_exhausted"
                ]
            );
        }
        assert_eq!(
            roles(&batch.session.messages),
            vec!["user", "assistant", "tool"]
        );
        assert_eq!(roles(&reply.session.messages), vec!["user", "assistant"]);
    }
}
