//! Maps a contract verification outcome to the loop transition shared by the
//! assistant-final and after-tool-batch call sites. The per-site differences
//! (final text, reply pop) are explicit inputs; the caller keeps the state
//! that only it can touch.
use super::*;

/// Where the contract was verified; it carries what differs between sites.
pub(super) enum ContractSite {
    /// Text-only reply: the reply text is the final text and is popped on repair.
    AssistantFinal { reply_text: String },
    /// After a tool batch: the final text is synthesized and nothing is popped.
    AfterToolBatch,
}

pub(super) struct ContractStop {
    final_text: String,
    stop_reason: RunStopReason,
    observation: ContractObservation,
    satisfied_event: Option<Value>,
}

pub(super) struct ContractRepair {
    pub(super) pop_reply: bool,
    feedback: VerifyFailureFeedback,
}

pub(super) enum ContractTransition {
    Stop(ContractStop),
    Repair(ContractRepair),
}

pub(super) fn map_contract_outcome(
    site: ContractSite,
    outcome: ContractVerificationOutcome,
    required_paths: &[String],
    verify_attempts: usize,
) -> ContractTransition {
    let pop_reply = matches!(site, ContractSite::AssistantFinal { .. });
    match outcome {
        ContractVerificationOutcome::Satisfied => ContractTransition::Stop(ContractStop {
            final_text: match site {
                ContractSite::AssistantFinal { reply_text } => reply_text,
                ContractSite::AfterToolBatch => {
                    format!(
                        "completion contract satisfied: {}",
                        required_paths.join(", ")
                    )
                }
            },
            stop_reason: RunStopReason::CompletionContractSatisfied,
            observation: ContractObservation::default(),
            satisfied_event: Some(json!({
                "event": "loop_stop",
                "reason": "completion_contract_satisfied",
                "required_paths": required_paths,
                "verify_attempts": verify_attempts,
            })),
        }),
        ContractVerificationOutcome::NeedsRepair(feedback) => {
            ContractTransition::Repair(ContractRepair {
                pop_reply,
                feedback,
            })
        }
        ContractVerificationOutcome::ObservationIncomplete(observation) => {
            ContractTransition::Stop(ContractStop {
                final_text: match site {
                    ContractSite::AssistantFinal { reply_text } => reply_text,
                    ContractSite::AfterToolBatch => format!(
                        "completion contract observed incomplete: {}",
                        observation.primary_reason
                    ),
                },
                stop_reason: RunStopReason::CompletionContractObservedIncomplete,
                observation,
                satisfied_event: None,
            })
        }
    }
}

impl ContractStop {
    pub(super) fn emit(&self, eval_events_path: Option<&Path>) {
        if let Some(event) = &self.satisfied_event {
            eval_events::emit(eval_events_path, event.clone());
        }
    }

    pub(super) fn into_outcome(
        self,
        changed_paths: Vec<String>,
        iterations: usize,
        tool_calls: usize,
        verify_attempts: usize,
        last_blocking_reason: Option<String>,
        last_provider_error: Option<String>,
    ) -> RunSessionOutcome {
        RunSessionOutcome {
            final_text: self.final_text,
            stop_reason: self.stop_reason,
            changed_paths,
            iterations,
            tool_calls,
            missing_required_paths: self.observation.missing_paths,
            missing_capabilities: self.observation.missing_capabilities,
            missing_evidence: self.observation.missing_evidence,
            missing_obligations: self.observation.missing_obligations,
            verify_attempts,
            last_blocking_reason,
            last_provider_error,
        }
    }
}

impl ContractRepair {
    /// Records the failure in the repair state and returns the feedback text.
    pub(super) fn apply(self, state: &mut VerifyRepairState, changed_paths: &[String]) -> String {
        state.pending_error_context = self.feedback.error_context.clone();
        state.pending_signature = Some(self.feedback.signature);
        state.pending_target = Some(self.feedback.target);
        state.changed_paths_at_failure = changed_paths.to_vec();
        state.no_edit_turns = 0;
        self.feedback.feedback
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use super::*;
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

    fn reply_site() -> ContractSite {
        ContractSite::AssistantFinal {
            reply_text: "reply".to_string(),
        }
    }

    fn observation() -> ContractObservation {
        ContractObservation {
            missing_paths: vec!["a.py".to_string()],
            missing_capabilities: vec!["cap".to_string()],
            missing_evidence: vec!["evidence".to_string()],
            missing_obligations: vec!["obligation".to_string()],
            primary_reason: "why".to_string(),
        }
    }

    fn repair_feedback() -> VerifyFailureFeedback {
        let report = crate::planner::verify::VerificationReport::profile_failed("broken");
        VerifyFailureFeedback {
            feedback: "fix it".to_string(),
            signature: VerificationSignature::from_report(&report),
            target: RepairTarget::EmptyApp,
            error_context: RunSessionErrorContext::default(),
        }
    }

    fn stop(transition: ContractTransition) -> ContractStop {
        match transition {
            ContractTransition::Stop(stop) => stop,
            ContractTransition::Repair(_) => panic!("expected a stop"),
        }
    }

    #[test]
    fn satisfied_maps_site_text_and_event() {
        let paths = vec!["a.py".to_string(), "b.py".to_string()];
        let map = |site| {
            stop(map_contract_outcome(
                site,
                ContractVerificationOutcome::Satisfied,
                &paths,
                3,
            ))
        };
        let reply = map(reply_site());
        let batch = map(ContractSite::AfterToolBatch);
        assert_eq!(reply.final_text, "reply");
        assert_eq!(
            batch.final_text,
            "completion contract satisfied: a.py, b.py"
        );
        for stop in [&reply, &batch] {
            assert_eq!(stop.stop_reason, RunStopReason::CompletionContractSatisfied);
            assert!(stop.observation.missing_paths.is_empty());
            assert_eq!(
                stop.satisfied_event,
                Some(json!({
                    "event": "loop_stop",
                    "reason": "completion_contract_satisfied",
                    "required_paths": ["a.py", "b.py"],
                    "verify_attempts": 3,
                }))
            );
        }
    }

    #[test]
    fn observation_incomplete_maps_site_text_and_carries_missing_lists() {
        let map = |site| {
            stop(map_contract_outcome(
                site,
                ContractVerificationOutcome::ObservationIncomplete(observation()),
                &["a.py".to_string()],
                1,
            ))
        };
        let reply = map(reply_site());
        let batch = map(ContractSite::AfterToolBatch);
        assert_eq!(reply.final_text, "reply");
        assert_eq!(
            batch.final_text,
            "completion contract observed incomplete: why"
        );
        for stop in [reply, batch] {
            assert!(stop.satisfied_event.is_none());
            let outcome = stop.into_outcome(vec!["c.py".to_string()], 4, 2, 1, None, None);
            assert_eq!(
                outcome.stop_reason,
                RunStopReason::CompletionContractObservedIncomplete
            );
            assert_eq!(outcome.missing_required_paths, vec!["a.py".to_string()]);
            assert_eq!(outcome.missing_capabilities, vec!["cap".to_string()]);
            assert_eq!(outcome.missing_evidence, vec!["evidence".to_string()]);
            assert_eq!(outcome.missing_obligations, vec!["obligation".to_string()]);
            assert_eq!(outcome.changed_paths, vec!["c.py".to_string()]);
            assert_eq!((outcome.iterations, outcome.tool_calls), (4, 2));
        }
    }

    #[test]
    fn repair_pops_only_at_the_assistant_final_site() {
        let map = |site| {
            map_contract_outcome(
                site,
                ContractVerificationOutcome::NeedsRepair(repair_feedback()),
                &[],
                1,
            )
        };
        let pops = |transition| match transition {
            ContractTransition::Repair(repair) => repair.pop_reply,
            ContractTransition::Stop(_) => panic!("expected a repair"),
        };
        assert!(pops(map(reply_site())));
        assert!(!pops(map(ContractSite::AfterToolBatch)));
    }

    #[test]
    fn repair_apply_records_failure_state_and_returns_feedback() {
        let ContractTransition::Repair(repair) = map_contract_outcome(
            ContractSite::AfterToolBatch,
            ContractVerificationOutcome::NeedsRepair(repair_feedback()),
            &[],
            1,
        ) else {
            panic!("expected a repair");
        };
        let mut state = VerifyRepairState {
            no_edit_turns: 2,
            ..VerifyRepairState::default()
        };
        let changed = vec!["a.py".to_string()];
        assert_eq!(repair.apply(&mut state, &changed), "fix it");
        assert!(state.pending_signature.is_some());
        assert_eq!(state.pending_target, Some(RepairTarget::EmptyApp));
        assert_eq!(state.changed_paths_at_failure, changed);
        assert_eq!(state.no_edit_turns, 0);
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
