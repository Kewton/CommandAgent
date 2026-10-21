//! Failure handoff for a step whose bounded repair is exhausted: stop event,
//! repair report, recovery YAML, run summary, and the stop message.
use super::{
    Config, RecoveryHandoff, RepairContext, RepairTarget, StepRunError, StepRunOutcome,
    VerificationReport, eval_events, handoff_commands, handoff_path, json, optional_handoff_path,
    reachability_blocked_evidence, recovery_artifact_check_summary, save_recovery_ultra_plan,
    save_repair_report_with_context, suggested_recovery_ultra_plan_command,
    suggested_ultra_recovery_command, validate_recovery_artifacts,
};

pub(super) struct StepFailureHandoff<'a> {
    pub(super) config: &'a Config,
    pub(super) plan_goal: &'a str,
    pub(super) step_id: &'a str,
    pub(super) report: &'a VerificationReport,
    pub(super) context: &'a RepairContext,
    pub(super) failure_kind: &'a str,
    pub(super) repair_target: RepairTarget,
    pub(super) repair_stop_reason: Option<&'a str>,
    pub(super) blocked_requirements: &'a [String],
}

impl StepFailureHandoff<'_> {
    pub(super) fn into_error(self, mut outcome: StepRunOutcome) -> StepRunError {
        let Self {
            config,
            plan_goal,
            step_id,
            report,
            context,
            failure_kind,
            repair_target,
            repair_stop_reason,
            blocked_requirements,
        } = self;
        eval_events::emit(
            config.eval_events_path.as_deref(),
            json!({
                "event": "loop_stop",
                "reason": failure_kind,
                "step_id": step_id,
                "repair_target": repair_target.as_str(),
                "repair_stop_reason": repair_stop_reason.unwrap_or_default(),
                "local_repair_exhausted": true,
            }),
        );
        let repair_report_path =
            match save_repair_report_with_context(&config.workspace_root, step_id, report, context)
            {
                Ok(path) => path,
                Err(err) => {
                    outcome.primary_failure = Some(err.to_string());
                    outcome.stop_reason = Some("repair_report_save_error".to_string());
                    outcome.partial = true;
                    return StepRunError {
                        message: err.to_string(),
                        outcome,
                    };
                }
            };
        let step_handoff = RecoveryHandoff {
            profile: config.profile.clone(),
            original_goal: context
                .overall_goal
                .clone()
                .unwrap_or_else(|| plan_goal.to_string()),
            failed_phase: None,
            failed_step: Some(step_id.to_string()),
            failure_kind: failure_kind.to_string(),
            failure_evidence: std::iter::once(report.primary_reason())
                .chain(reachability_blocked_evidence(blocked_requirements))
                .chain(
                    report
                        .command_failures
                        .iter()
                        .map(|failure| format!("{}: {}", failure.command, failure.reason)),
                )
                .chain(
                    report
                        .verifier_command_false_negatives
                        .iter()
                        .map(|failure| {
                            format!(
                                "deterministic_verify_command_bug: {}: {}",
                                failure.command, failure.reason
                            )
                        }),
                )
                .collect(),
            missing_paths: report.missing_paths.clone(),
            missing_capabilities: vec![repair_target.as_str().to_string()],
            verify_commands: handoff_commands(config, &context.verify_commands),
            changed_paths: context.changed_files.clone(),
            repair_targets: vec![repair_target.as_str().to_string()],
        };
        let recovery_plan_path =
            match save_recovery_ultra_plan(&config.workspace_root, step_id, &step_handoff) {
                Ok(path) => Some(path),
                Err(err) => {
                    eval_events::emit(
                        config.eval_events_path.as_deref(),
                        json!({
                            "event": "recovery_ultra_plan_save_failed",
                            "recovery_handoff_kind": failure_kind,
                            "step_id": step_id,
                            "recovery_prompt_path": handoff_path(&repair_report_path),
                            "reason": eval_events::body_snippet(&err.to_string()),
                            "recovery_yaml_missing": true,
                        }),
                    );
                    None
                }
            };
        let validation =
            validate_recovery_artifacts(&repair_report_path, recovery_plan_path.as_deref());
        let suggested_command = if validation.prompt_command_available() {
            suggested_ultra_recovery_command(&repair_report_path, &config.profile)
        } else {
            String::new()
        };
        let suggested_yaml_command = recovery_plan_path
            .as_ref()
            .filter(|_| validation.yaml_command_available())
            .map(|path| suggested_recovery_ultra_plan_command(path));
        let repair_report_display = handoff_path(&repair_report_path);
        let recovery_plan_display = optional_handoff_path(recovery_plan_path.as_ref());
        eval_events::emit(
            config.eval_events_path.as_deref(),
            json!({
                "event": "recovery_prompt_saved",
                "recovery_handoff_kind": failure_kind,
                "step_id": step_id,
                "recovery_prompt_path": &repair_report_display,
                "recovery_ultra_plan_path": &recovery_plan_display,
                "recovery_yaml_missing": recovery_plan_path.is_none(),
                "recovery_prompt_exists": validation.prompt_exists,
                "recovery_prompt_parse_ok": validation.prompt_parse_ok,
                "recovery_prompt_parse_error": validation.prompt_parse_error.as_deref().unwrap_or_default(),
                "recovery_yaml_exists": validation.yaml_exists,
                "recovery_yaml_parse_ok": validation.yaml_parse_ok,
                "recovery_yaml_parse_error": validation.yaml_parse_error.as_deref().unwrap_or_default(),
                "recovery_command_targets_valid": validation.command_targets_valid(),
                "suggested_recovery_command": suggested_command.clone(),
                "suggested_recovery_yaml_command": suggested_yaml_command.clone().unwrap_or_default(),
                "recovery_profile": config.profile,
                "local_repair_exhausted": true,
                "failure_kind": failure_kind,
                "status": "incomplete",
            }),
        );
        let yaml_summary = recovery_plan_path
            .as_ref()
            .map(|path| {
                let display = handoff_path(path);
                if validation.yaml_parse_ok {
                    format!("Recovery UltraPlan YAML saved: {display}")
                } else {
                    format!(
                        "Recovery UltraPlan YAML invalid: {} ({})",
                        display,
                        validation
                            .yaml_parse_error
                            .as_deref()
                            .unwrap_or("recovery_yaml_invalid")
                    )
                }
            })
            .unwrap_or_else(|| {
                "Recovery UltraPlan YAML missing: failed to save valid recovery plan".to_string()
            });
        let prompt_command_summary = if validation.prompt_command_available() {
            format!("Suggested command: {suggested_command}")
        } else {
            format!(
                "Suggested command: unavailable because recovery prompt validation failed ({})",
                validation
                    .prompt_parse_error
                    .as_deref()
                    .unwrap_or("recovery_prompt_invalid")
            )
        };
        let yaml_command_summary = suggested_yaml_command
            .as_ref()
            .map(|command| format!("Suggested YAML command: {command}"))
            .unwrap_or_else(|| {
                "Suggested YAML command: unavailable because recovery YAML is missing".to_string()
            });
        let artifact_check_summary = recovery_artifact_check_summary(&validation);
        eval_events::write_run_summary(
            config.eval_events_path.as_deref(),
            &format!(
                "Status: incomplete\n{}\n{}\nRecovery prompt saved: {}\n{}\n{}\nFailure: {}",
                yaml_summary,
                yaml_command_summary,
                repair_report_display,
                prompt_command_summary,
                artifact_check_summary,
                eval_events::body_snippet(&report.primary_reason())
            ),
        );
        let prompt_message = if validation.prompt_command_available() {
            format!("suggested command: {suggested_command}")
        } else {
            "suggested command unavailable because recovery prompt validation failed".to_string()
        };
        let yaml_message = suggested_yaml_command
            .as_ref()
            .map(|command| format!("suggested YAML command: {command}"))
            .unwrap_or_else(|| {
                "suggested YAML command unavailable because recovery YAML is missing".to_string()
            });
        let message = eval_events::render_stop_reason(&eval_events::StopReasonParts {
            free_text: format!(
                "step {} failed verification after bounded repair: {}; failure_kind={}; incomplete; {}",
                step_id,
                report.primary_reason(),
                failure_kind,
                artifact_check_summary
            ),
            paths: vec![
                format!("repair prompt saved: {repair_report_display}"),
                yaml_summary,
            ],
            commands: vec![prompt_message, yaml_message],
        });
        outcome.primary_failure = Some(report.primary_reason());
        outcome.stop_reason = Some(failure_kind.to_string());
        outcome.partial = true;
        StepRunError { message, outcome }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{
        ContractEnforcement, NOOP_UI, PlanStep, SessionSnapshot, StepPlan, StepPromptContext,
        run_step,
    };
    use crate::config::Config;
    use crate::providers::{AssistantReply, ChatClient};
    use crate::state::{ConversationMessage, ToolCall};
    use crate::tools::registry::ToolSpec;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};

    #[derive(Clone)]
    struct ScriptedClient {
        replies: Arc<Mutex<Vec<AssistantReply>>>,
    }

    impl ChatClient for ScriptedClient {
        fn label(&self) -> &str {
            "scripted"
        }

        fn boxed_clone(&self) -> Box<dyn ChatClient> {
            Box::new(self.clone())
        }

        fn chat(
            &mut self,
            _model: &str,
            _messages: &[ConversationMessage],
            _tools: &[ToolSpec],
            _native_tools_enabled: bool,
        ) -> anyhow::Result<AssistantReply> {
            let mut replies = self.replies.lock().unwrap();
            if replies.is_empty() {
                anyhow::bail!("scripted client exhausted")
            }
            Ok(replies.remove(0))
        }
    }

    fn config(root: PathBuf) -> Config {
        Config {
            workspace_root: root,
            state_dir: PathBuf::from("state"),
            eval_events_path: None,
            completion_contract_path: None,
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
            max_iterations: 4,
            recovery_plan_auto_runs: 0,
            chat_timeout_secs: 1,
            chat_timeout_source: "override:test".to_string(),
            field_sources: crate::config::ConfigFieldSources::default(),
            chat_retries: 1,
            stream: false,
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

    fn write_reply(path: &str) -> AssistantReply {
        AssistantReply {
            content: String::new(),
            tool_calls: vec![ToolCall::new(
                "Write",
                serde_json::json!({"path": path, "content": "unrelated"}),
            )],
            prompt_tokens: None,
            completion_tokens: None,
        }
    }

    fn exhausting_replies() -> Vec<AssistantReply> {
        vec![
            AssistantReply {
                content: String::new(),
                tool_calls: vec![ToolCall::new(
                    "Bash",
                    serde_json::json!({"command": "true"}),
                )],
                prompt_tokens: None,
                completion_tokens: None,
            },
            AssistantReply::text("initial incomplete"),
            write_reply("README.md"),
            AssistantReply::text("repair one done"),
            write_reply("docs/notes.md"),
            AssistantReply::text("repair two done"),
            write_reply("docs/notes-2.md"),
            AssistantReply::text("repair three done"),
            write_reply("docs/notes-3.md"),
            AssistantReply::text("repair four done"),
        ]
    }

    fn page_step(verify: &str) -> (StepPlan, PlanStep) {
        let step = PlanStep {
            id: "entrypoint".to_string(),
            kind: "verify".to_string(),
            expected_result: "pass".to_string(),
            instruction: "Verify and repair src/app/page.tsx if missing".to_string(),
            expected_paths: Vec::new(),
            verify: vec![verify.to_string()],
        };
        let plan = StepPlan {
            goal: "Create app entrypoint\n\nRequired final artifacts:\n- src/app/page.tsx"
                .to_string(),
            steps: vec![step.clone()],
        };
        (plan, step)
    }

    #[derive(Clone, Copy)]
    enum Block {
        None,
        RepairsDir,
        PlansDir,
    }

    #[derive(Debug)]
    struct Probe {
        message: String,
        stop_reason: Option<String>,
        primary_failure: Option<String>,
        partial: bool,
        handoff_events: Vec<String>,
        summary_details: String,
        repair_reports: usize,
        recovery_plans: usize,
    }

    fn normalize(text: &str, root: &Path) -> String {
        let text = text.replace(&root.display().to_string(), "<ROOT>");
        let uuid =
            regex::Regex::new("[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}")
                .unwrap();
        let text = uuid.replace_all(&text, "<UUID>").to_string();
        let elapsed = regex::Regex::new("elapsed_ms: [0-9]+").unwrap();
        elapsed.replace_all(&text, "elapsed_ms: N").to_string()
    }

    fn count_files(dir: &Path) -> usize {
        std::fs::read_dir(dir)
            .map(|entries| entries.flatten().filter(|e| e.path().is_file()).count())
            .unwrap_or(0)
    }

    fn probe(verify: &str, block: Block) -> Probe {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let runtime = root.join(".commandagent");
        match block {
            Block::None => {}
            Block::RepairsDir => {
                std::fs::create_dir_all(&runtime).unwrap();
                std::fs::write(runtime.join("repairs"), "not a directory").unwrap();
            }
            Block::PlansDir => {
                std::fs::create_dir_all(&runtime).unwrap();
                std::fs::write(runtime.join("plans"), "not a directory").unwrap();
            }
        }
        let events = root.join("run/events.jsonl");
        let mut cfg = config(root.to_path_buf());
        cfg.eval_events_path = Some(events.clone());
        let (plan, step) = page_step(verify);
        let context = StepPromptContext {
            overall_goal: plan.goal.clone(),
            ..StepPromptContext::default()
        };
        let mut client = ScriptedClient {
            replies: Arc::new(Mutex::new(exhausting_replies())),
        };
        let mut session = SessionSnapshot::new();
        let error = run_step(
            &mut client,
            &mut session,
            &plan,
            &step,
            &context,
            &cfg,
            &NOOP_UI,
            "test",
            ContractEnforcement::Enforce,
            None,
            None,
        )
        .unwrap_err();
        let events = std::fs::read_to_string(&events).unwrap_or_default();
        let handoff_events = events
            .lines()
            .filter(|line| {
                [
                    "loop_stop",
                    "recovery_ultra_plan_save_failed",
                    "recovery_prompt_saved",
                ]
                .iter()
                .any(|name| line.contains(&format!("\"event\":\"{name}\"")))
            })
            .filter(|line| !line.contains("step_tool_observation_completed"))
            .map(|line| normalize(line, root))
            .collect();
        let summary = std::fs::read_to_string(root.join("run/summary.md")).unwrap_or_default();
        let summary_details = summary
            .split_once("## Machine details\n")
            .map(|(_, details)| normalize(details, root))
            .unwrap_or_default();
        Probe {
            message: normalize(&error.message, root),
            stop_reason: error.outcome.stop_reason.clone(),
            primary_failure: error
                .outcome
                .primary_failure
                .as_deref()
                .map(|text| normalize(text, root)),
            partial: error.outcome.partial,
            handoff_events,
            summary_details,
            repair_reports: count_files(&runtime.join("repairs")),
            recovery_plans: count_files(&runtime.join("plans")),
        }
    }

    #[test]
    fn exhausted_step_saves_prompt_and_yaml_handoff() {
        let got = probe("test -f src/app/page.tsx", Block::None);
        assert_eq!(
            got.message,
            "step entrypoint failed verification after bounded repair: command failed: test -f src/app/page.tsx outcome: CommandFailed status: exit status: 1 elapsed_ms: N summary: command did not succeed: test -f src/app/page.tsx stdout: stderr: ; failure_kind=bounded_repair_exhausted; incomplete; Recovery artifact check: prompt_parse_ok=true, yaml_parse_ok=true, command_targets_valid=true\nPaths:\n- repair prompt saved: .commandagent/repairs/repair-entrypoint-<UUID>.md\n- Recovery UltraPlan YAML saved: .commandagent/plans/recovery-ultra-plan-entrypoint-<UUID>.yaml\nCommands:\n- suggested command: /ultra-plan-run --profile generic \"$(cat .commandagent/repairs/repair-entrypoint-<UUID>.md)\"\n- suggested YAML command: /run-ultra-plan .commandagent/plans/recovery-ultra-plan-entrypoint-<UUID>.yaml"
        );
        assert_eq!(got.stop_reason.as_deref(), Some("bounded_repair_exhausted"));
        assert_eq!(
            got.primary_failure.as_deref(),
            Some(
                "command failed: test -f src/app/page.tsx\noutcome: CommandFailed\nstatus: exit status: 1\nelapsed_ms: N\nsummary: command did not succeed: test -f src/app/page.tsx\nstdout:\n\nstderr:\n"
            )
        );
        assert!(got.partial);
        assert_eq!(
            got.handoff_events,
            vec![
                "{\"event\":\"loop_stop\",\"local_repair_exhausted\":true,\"reason\":\"bounded_repair_exhausted\",\"repair_stop_reason\":\"AssistantFinal\",\"repair_target\":\"missing_entrypoint\",\"schema_version\":\"1\",\"step_id\":\"entrypoint\"}".to_string(),
                "{\"event\":\"recovery_prompt_saved\",\"failure_kind\":\"bounded_repair_exhausted\",\"local_repair_exhausted\":true,\"recovery_command_targets_valid\":true,\"recovery_handoff_kind\":\"bounded_repair_exhausted\",\"recovery_profile\":\"generic\",\"recovery_prompt_exists\":true,\"recovery_prompt_parse_error\":\"\",\"recovery_prompt_parse_ok\":true,\"recovery_prompt_path\":\".commandagent/repairs/repair-entrypoint-<UUID>.md\",\"recovery_ultra_plan_path\":\".commandagent/plans/recovery-ultra-plan-entrypoint-<UUID>.yaml\",\"recovery_yaml_exists\":true,\"recovery_yaml_missing\":false,\"recovery_yaml_parse_error\":\"\",\"recovery_yaml_parse_ok\":true,\"schema_version\":\"1\",\"status\":\"incomplete\",\"step_id\":\"entrypoint\",\"suggested_recovery_command\":\"/ultra-plan-run --profile generic \\\"$(cat .commandagent/repairs/repair-entrypoint-<UUID>.md)\\\"\",\"suggested_recovery_yaml_command\":\"/run-ultra-plan .commandagent/plans/recovery-ultra-plan-entrypoint-<UUID>.yaml\"}".to_string(),
            ]
        );
        assert_eq!(
            got.summary_details,
            "\nStatus: incomplete\nRecovery UltraPlan YAML saved: .commandagent/plans/recovery-ultra-plan-entrypoint-<UUID>.yaml\nSuggested YAML command: /run-ultra-plan .commandagent/plans/recovery-ultra-plan-entrypoint-<UUID>.yaml\nRecovery prompt saved: .commandagent/repairs/repair-entrypoint-<UUID>.md\nSuggested command: /ultra-plan-run --profile generic \"$(cat .commandagent/repairs/repair-entrypoint-<UUID>.md)\"\nRecovery artifact check: prompt_parse_ok=true, yaml_parse_ok=true, command_targets_valid=true\nFailure: command failed: test -f src/app/page.tsx outcome: CommandFailed status: exit status: 1 elapsed_ms: N summary: command did not succeed: test -f src/app/page.tsx stdout: stderr:\n"
        );
        assert_eq!(got.repair_reports, 1);
        assert_eq!(got.recovery_plans, 1);
    }

    #[test]
    fn repair_report_save_failure_stops_before_recovery_yaml() {
        let got = probe("test -f src/app/page.tsx", Block::RepairsDir);
        assert_eq!(got.message, "File exists (os error 17)");
        assert_eq!(got.stop_reason.as_deref(), Some("repair_report_save_error"));
        assert_eq!(
            got.primary_failure.as_deref(),
            Some("File exists (os error 17)")
        );
        assert!(got.partial);
        assert_eq!(
            got.handoff_events,
            vec![
                "{\"event\":\"loop_stop\",\"local_repair_exhausted\":true,\"reason\":\"bounded_repair_exhausted\",\"repair_stop_reason\":\"AssistantFinal\",\"repair_target\":\"missing_entrypoint\",\"schema_version\":\"1\",\"step_id\":\"entrypoint\"}".to_string(),
            ]
        );
        assert_eq!(got.summary_details, "");
        assert_eq!(got.repair_reports, 0);
        assert_eq!(got.recovery_plans, 0);
    }

    #[test]
    fn recovery_yaml_save_failure_keeps_prompt_handoff_and_reports_missing_yaml() {
        let got = probe("test -f src/app/page.tsx", Block::PlansDir);
        assert_eq!(
            got.message,
            "step entrypoint failed verification after bounded repair: command failed: test -f src/app/page.tsx outcome: CommandFailed status: exit status: 1 elapsed_ms: N summary: command did not succeed: test -f src/app/page.tsx stdout: stderr: ; failure_kind=bounded_repair_exhausted; incomplete; Recovery artifact check: prompt_parse_ok=true, yaml_parse_ok=false, command_targets_valid=false\nPaths:\n- repair prompt saved: .commandagent/repairs/repair-entrypoint-<UUID>.md\n- Recovery UltraPlan YAML missing: failed to save valid recovery plan\nCommands:\n- suggested command: /ultra-plan-run --profile generic \"$(cat .commandagent/repairs/repair-entrypoint-<UUID>.md)\"\n- suggested YAML command unavailable because recovery YAML is missing"
        );
        assert_eq!(got.stop_reason.as_deref(), Some("bounded_repair_exhausted"));
        assert_eq!(
            got.primary_failure.as_deref(),
            Some(
                "command failed: test -f src/app/page.tsx\noutcome: CommandFailed\nstatus: exit status: 1\nelapsed_ms: N\nsummary: command did not succeed: test -f src/app/page.tsx\nstdout:\n\nstderr:\n"
            )
        );
        assert!(got.partial);
        assert_eq!(
            got.handoff_events,
            vec![
                "{\"event\":\"loop_stop\",\"local_repair_exhausted\":true,\"reason\":\"bounded_repair_exhausted\",\"repair_stop_reason\":\"AssistantFinal\",\"repair_target\":\"missing_entrypoint\",\"schema_version\":\"1\",\"step_id\":\"entrypoint\"}".to_string(),
                "{\"event\":\"recovery_ultra_plan_save_failed\",\"reason\":\"File exists (os error 17)\",\"recovery_handoff_kind\":\"bounded_repair_exhausted\",\"recovery_prompt_path\":\".commandagent/repairs/repair-entrypoint-<UUID>.md\",\"recovery_yaml_missing\":true,\"schema_version\":\"1\",\"step_id\":\"entrypoint\"}".to_string(),
                "{\"event\":\"recovery_prompt_saved\",\"failure_kind\":\"bounded_repair_exhausted\",\"local_repair_exhausted\":true,\"recovery_command_targets_valid\":false,\"recovery_handoff_kind\":\"bounded_repair_exhausted\",\"recovery_profile\":\"generic\",\"recovery_prompt_exists\":true,\"recovery_prompt_parse_error\":\"\",\"recovery_prompt_parse_ok\":true,\"recovery_prompt_path\":\".commandagent/repairs/repair-entrypoint-<UUID>.md\",\"recovery_ultra_plan_path\":\"\",\"recovery_yaml_exists\":false,\"recovery_yaml_missing\":true,\"recovery_yaml_parse_error\":\"recovery_yaml_missing\",\"recovery_yaml_parse_ok\":false,\"schema_version\":\"1\",\"status\":\"incomplete\",\"step_id\":\"entrypoint\",\"suggested_recovery_command\":\"/ultra-plan-run --profile generic \\\"$(cat .commandagent/repairs/repair-entrypoint-<UUID>.md)\\\"\",\"suggested_recovery_yaml_command\":\"\"}".to_string(),
            ]
        );
        assert_eq!(
            got.summary_details,
            "\nStatus: incomplete\nRecovery UltraPlan YAML missing: failed to save valid recovery plan\nSuggested YAML command: unavailable because recovery YAML is missing\nRecovery prompt saved: .commandagent/repairs/repair-entrypoint-<UUID>.md\nSuggested command: /ultra-plan-run --profile generic \"$(cat .commandagent/repairs/repair-entrypoint-<UUID>.md)\"\nRecovery artifact check: prompt_parse_ok=true, yaml_parse_ok=false, command_targets_valid=false\nFailure: command failed: test -f src/app/page.tsx outcome: CommandFailed status: exit status: 1 elapsed_ms: N summary: command did not succeed: test -f src/app/page.tsx stdout: stderr:\n"
        );
        assert_eq!(got.repair_reports, 1);
        assert_eq!(got.recovery_plans, 0);
    }

    #[test]
    fn failing_verify_command_hands_off_implementation_target() {
        let got = probe("false", Block::None);
        assert_eq!(
            got.message,
            "step entrypoint failed verification after bounded repair: command failed: false outcome: CommandFailed status: exit status: 1 elapsed_ms: N summary: command did not succeed: false stdout: stderr: ; failure_kind=bounded_repair_exhausted; incomplete; Recovery artifact check: prompt_parse_ok=true, yaml_parse_ok=true, command_targets_valid=true\nPaths:\n- repair prompt saved: .commandagent/repairs/repair-entrypoint-<UUID>.md\n- Recovery UltraPlan YAML saved: .commandagent/plans/recovery-ultra-plan-entrypoint-<UUID>.yaml\nCommands:\n- suggested command: /ultra-plan-run --profile generic \"$(cat .commandagent/repairs/repair-entrypoint-<UUID>.md)\"\n- suggested YAML command: /run-ultra-plan .commandagent/plans/recovery-ultra-plan-entrypoint-<UUID>.yaml"
        );
        assert_eq!(got.stop_reason.as_deref(), Some("bounded_repair_exhausted"));
        assert_eq!(
            got.primary_failure.as_deref(),
            Some(
                "command failed: false\noutcome: CommandFailed\nstatus: exit status: 1\nelapsed_ms: N\nsummary: command did not succeed: false\nstdout:\n\nstderr:\n"
            )
        );
        assert!(got.partial);
        assert_eq!(
            got.handoff_events,
            vec![
                "{\"event\":\"loop_stop\",\"local_repair_exhausted\":true,\"reason\":\"bounded_repair_exhausted\",\"repair_stop_reason\":\"AssistantFinal\",\"repair_target\":\"implementation\",\"schema_version\":\"1\",\"step_id\":\"entrypoint\"}".to_string(),
                "{\"event\":\"recovery_prompt_saved\",\"failure_kind\":\"bounded_repair_exhausted\",\"local_repair_exhausted\":true,\"recovery_command_targets_valid\":true,\"recovery_handoff_kind\":\"bounded_repair_exhausted\",\"recovery_profile\":\"generic\",\"recovery_prompt_exists\":true,\"recovery_prompt_parse_error\":\"\",\"recovery_prompt_parse_ok\":true,\"recovery_prompt_path\":\".commandagent/repairs/repair-entrypoint-<UUID>.md\",\"recovery_ultra_plan_path\":\".commandagent/plans/recovery-ultra-plan-entrypoint-<UUID>.yaml\",\"recovery_yaml_exists\":true,\"recovery_yaml_missing\":false,\"recovery_yaml_parse_error\":\"\",\"recovery_yaml_parse_ok\":true,\"schema_version\":\"1\",\"status\":\"incomplete\",\"step_id\":\"entrypoint\",\"suggested_recovery_command\":\"/ultra-plan-run --profile generic \\\"$(cat .commandagent/repairs/repair-entrypoint-<UUID>.md)\\\"\",\"suggested_recovery_yaml_command\":\"/run-ultra-plan .commandagent/plans/recovery-ultra-plan-entrypoint-<UUID>.yaml\"}".to_string(),
            ]
        );
        assert_eq!(
            got.summary_details,
            "\nStatus: incomplete\nRecovery UltraPlan YAML saved: .commandagent/plans/recovery-ultra-plan-entrypoint-<UUID>.yaml\nSuggested YAML command: /run-ultra-plan .commandagent/plans/recovery-ultra-plan-entrypoint-<UUID>.yaml\nRecovery prompt saved: .commandagent/repairs/repair-entrypoint-<UUID>.md\nSuggested command: /ultra-plan-run --profile generic \"$(cat .commandagent/repairs/repair-entrypoint-<UUID>.md)\"\nRecovery artifact check: prompt_parse_ok=true, yaml_parse_ok=true, command_targets_valid=true\nFailure: command failed: false outcome: CommandFailed status: exit status: 1 elapsed_ms: N summary: command did not succeed: false stdout: stderr:\n"
        );
        assert_eq!(got.repair_reports, 1);
        assert_eq!(got.recovery_plans, 1);
    }
}
