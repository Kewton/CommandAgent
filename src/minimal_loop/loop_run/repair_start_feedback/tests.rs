use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use super::{
    REPAIR_START_WRITE_GUIDANCE, repair_start_needs_write_guidance, repair_start_write_guidance,
};
use crate::config::Config;
use crate::minimal_loop::repair_pressure::{
    CarriedPressure, READ_ONLY_STAGNATION_WRITE_REQUIRED_THRESHOLD,
};
use crate::minimal_loop::stagnation_carryover::{EscalationCarryoverHandle, attach_to_options};
use crate::providers::{AssistantReply, ChatClient};
use crate::state::{ConversationMessage, SessionSnapshot, ToolCall};
use crate::tools::registry::ToolSpec;
use crate::tui::NOOP_UI;

use super::super::{
    RunSessionOptions, RunSessionOutcome, RunSessionStepKind, run_session_with_outcome_with_options,
};

const THRESHOLD: usize = READ_ONLY_STAGNATION_WRITE_REQUIRED_THRESHOLD;
const TARGET: &str = "src/gameLogic.ts";
const BROKEN: &str = "export const score: number = \"high\";\n";
const FIXED: &str = "export const score: number = 1;\n";
const PROMPT: &str = "Repair step `verify-types`: TS2322 Type 'string' is not assignable to type 'number' in src/gameLogic.ts.";

struct Request {
    messages: Vec<ConversationMessage>,
    tool_count: usize,
    native_tools: bool,
}

#[derive(Clone)]
struct RecordingClient {
    native: bool,
    replies: Arc<Mutex<Vec<AssistantReply>>>,
    requests: Arc<Mutex<Vec<Request>>>,
}

impl RecordingClient {
    fn new(native: bool, replies: Vec<AssistantReply>) -> Self {
        Self {
            native,
            replies: Arc::new(Mutex::new(replies)),
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl ChatClient for RecordingClient {
    fn label(&self) -> &str {
        "recording"
    }

    fn boxed_clone(&self) -> Box<dyn ChatClient> {
        Box::new(self.clone())
    }

    fn supports_native_tools(&self, _model: &str) -> bool {
        self.native
    }

    fn chat(
        &mut self,
        _model: &str,
        messages: &[ConversationMessage],
        tools: &[ToolSpec],
        native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        self.requests.lock().unwrap().push(Request {
            messages: messages.to_vec(),
            tool_count: tools.len(),
            native_tools: native_tools_enabled,
        });
        let mut replies = self.replies.lock().unwrap();
        if replies.is_empty() {
            anyhow::bail!("scripted client exhausted");
        }
        Ok(replies.remove(0))
    }
}

fn tool_reply(name: &str, arguments: Value) -> AssistantReply {
    AssistantReply {
        content: String::new(),
        tool_calls: vec![ToolCall::new(name, arguments)],
        prompt_tokens: None,
        completion_tokens: None,
    }
}

fn read() -> AssistantReply {
    tool_reply("Read", json!({"path": TARGET}))
}

fn write_fix() -> AssistantReply {
    tool_reply("Write", json!({"path": TARGET, "content": FIXED}))
}

fn edit_missing_anchor() -> AssistantReply {
    tool_reply(
        "Edit",
        json!({"path": TARGET, "old_string": "missing anchor", "new_string": FIXED}),
    )
}

fn config(root: &Path, budget: usize) -> Config {
    Config {
        workspace_root: root.to_path_buf(),
        state_dir: root.join("state"),
        yes: true,
        offline: false,
        context_budget: 100_000,
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
        max_iterations: budget,
        recovery_plan_auto_runs: 0,
        chat_timeout_secs: 30,
        chat_timeout_source: "override:test".to_string(),
        field_sources: crate::config::ConfigFieldSources::default(),
        chat_retries: 1,
        stream: false,
        eval_events_path: Some(root.join("events.jsonl")),
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

fn verify_repair(carried: CarriedPressure) -> RunSessionOptions {
    attach_to_options(
        RunSessionOptions::plan_step(RunSessionStepKind::Verify)
            .with_path_fallback_candidates(vec![TARGET.to_string()]),
        EscalationCarryoverHandle::from_pressure(carried),
    )
}

fn fresh_carry() -> CarriedPressure {
    CarriedPressure::default()
}

fn carried_streak(read_only_streak: usize) -> CarriedPressure {
    CarriedPressure {
        read_only_streak,
        ..CarriedPressure::default()
    }
}

struct Run {
    dir: tempfile::TempDir,
    result: Result<RunSessionOutcome, String>,
    requests: Vec<Request>,
}

impl Run {
    fn root(&self) -> &Path {
        self.dir.path()
    }

    fn events(&self) -> Vec<Value> {
        std::fs::read_to_string(self.root().join("events.jsonl"))
            .unwrap_or_default()
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .collect()
    }

    fn named(&self, name: &str) -> Vec<Value> {
        self.events()
            .into_iter()
            .filter(|event| event["event"] == name)
            .collect()
    }

    fn queued(&self) -> Vec<Value> {
        self.named("repair_start_write_guidance_queued")
    }

    fn request_has_advisory(&self, index: usize) -> bool {
        self.requests[index]
            .messages
            .iter()
            .any(|message| message.content.contains(REPAIR_START_WRITE_GUIDANCE))
    }

    fn last_user_message(&self, index: usize) -> &str {
        &self.requests[index].messages.last().unwrap().content
    }

    fn source(&self) -> String {
        std::fs::read_to_string(self.root().join(TARGET)).unwrap()
    }

    /// Every event except the new advisory record, reduced to the fields that
    /// decide enforcement, counters, rejections, and the stop.
    fn decision_trace(&self) -> Vec<String> {
        const KEYS: &[&str] = &[
            "stage",
            "read_only_streak",
            "initial_read_only_streak",
            "pre_advanced",
            "carried_streak",
            "repair_turn_budget",
            "tool_name",
            "selected_targets",
            "selection_reason",
            "write_required_no_write_attempts",
            "write_required_no_write_limit",
            "reason",
            "tool_calls",
            "name",
            "status",
            "recovery_handoff_kind",
            "recovery_yaml_roundtrip_ok",
        ];
        self.events()
            .iter()
            .filter(|event| event["event"] != "repair_start_write_guidance_queued")
            .filter(|event| event["event"] != "provider_turn_duration")
            .map(|event| {
                let mut line = event["event"].as_str().unwrap_or("").to_string();
                for key in KEYS {
                    if let Some(value) = event.get(*key) {
                        line.push_str(&format!(" {key}={value}"));
                    }
                }
                line
            })
            .collect()
    }

    /// Files left in the workspace, excluding the event log itself.
    /// Run ids in recovery artifact names are replaced with `<id>`.
    fn workspace_files(&self) -> Vec<String> {
        let id = regex::Regex::new(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}")
            .unwrap();
        let mut files = Vec::new();
        collect_files(self.root(), self.root(), &mut files);
        files.retain(|path| path != "events.jsonl");
        let mut files: Vec<String> = files
            .iter()
            .map(|path| id.replace_all(path, "<id>").into_owned())
            .collect();
        files.sort();
        files
    }

    /// The recovery prompt saved at the stop, which must not quote the advisory.
    fn recovery_prompt(&self) -> String {
        let dir = self.root().join(".commandagent/repairs");
        let entry = std::fs::read_dir(dir).unwrap().next().unwrap().unwrap();
        std::fs::read_to_string(entry.path()).unwrap()
    }
}

fn collect_files(root: &Path, dir: &Path, files: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path: PathBuf = entry.path();
        if path.is_dir() {
            collect_files(root, &path, files);
        } else if let Ok(rel) = path.strip_prefix(root) {
            files.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
}

fn run_with(
    native: bool,
    budget: usize,
    options: RunSessionOptions,
    replies: Vec<AssistantReply>,
) -> Run {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::write(dir.path().join(TARGET), BROKEN).unwrap();
    let cfg = config(dir.path(), budget);
    let mut client = RecordingClient::new(native, replies);
    let mut session = SessionSnapshot::new();
    let result = run_session_with_outcome_with_options(
        &mut client,
        &mut session,
        PROMPT,
        &[],
        &cfg,
        &NOOP_UI,
        options,
    )
    .map_err(|err| err.to_string());
    let requests = std::mem::take(&mut *client.requests.lock().unwrap());
    Run {
        dir,
        result,
        requests,
    }
}

fn run(budget: usize, options: RunSessionOptions, replies: Vec<AssistantReply>) -> Run {
    run_with(true, budget, options, replies)
}

fn reads(count: usize) -> Vec<AssistantReply> {
    (0..count).map(|_| read()).collect()
}

const SEED_6: &str = "escalation_carryover initial_read_only_streak=6 pre_advanced=true carried_streak=0 repair_turn_budget=6";
const READ_RAW: &str = r#"tool_call_raw name="Read""#;
const WRITE_REQUIRED_AT_6: &str = r#"read_only_stagnation_feedback stage="write_required" read_only_streak=6 selected_targets=["src/gameLogic.ts"] selection_reason="fallback""#;
const RECOVERY_SAVED: &str = r#"recovery_prompt_saved status="incomplete" recovery_handoff_kind="model_stagnation:read_only_loop" recovery_yaml_roundtrip_ok=true"#;

fn rejected(streak: usize, attempts: usize) -> String {
    format!(
        r#"read_only_tool_rejected stage="write_required" read_only_streak={streak} tool_name="Read" selected_targets=["src/gameLogic.ts"] selection_reason="fallback" write_required_no_write_attempts={attempts} write_required_no_write_limit=2"#
    )
}

fn stopped(streak: usize, tool_calls: usize) -> String {
    format!(
        r#"loop_stop read_only_streak={streak} selected_targets=["src/gameLogic.ts"] selection_reason="fallback" write_required_no_write_attempts=2 write_required_no_write_limit=2 reason="model_stagnation:read_only_loop" tool_calls={tool_calls}"#
    )
}

/// The trace the pre-#903 loop (develop 56f63cc5) produced for a seeded repair
/// that only reads: the first Read runs, write_required appears after it, two
/// rejections stop the session.
fn read_only_trace_before() -> Vec<String> {
    vec![
        "step_obligation_scope".to_string(),
        SEED_6.to_string(),
        READ_RAW.to_string(),
        r#"tool_execute name="Read" status="ok""#.to_string(),
        WRITE_REQUIRED_AT_6.to_string(),
        READ_RAW.to_string(),
        rejected(6, 1),
        READ_RAW.to_string(),
        rejected(6, 2),
        RECOVERY_SAVED.to_string(),
        stopped(6, 3),
    ]
}

/// The trace the pre-#903 loop (develop 56f63cc5) produced for Read, failed
/// Edit, Read, failed Edit, Read.
fn anchor_trace_before() -> Vec<String> {
    let edit_failed = [
        r#"tool_call_raw name="Edit""#.to_string(),
        r#"tool_validation_error name="Edit""#.to_string(),
        r#"edit_anchor_recovery stage="reanchor""#.to_string(),
    ];
    let mut trace = vec![
        "step_obligation_scope".to_string(),
        SEED_6.to_string(),
        READ_RAW.to_string(),
        r#"tool_execute name="Read" status="ok""#.to_string(),
        WRITE_REQUIRED_AT_6.to_string(),
    ];
    trace.extend(edit_failed.clone());
    trace.extend([READ_RAW.to_string(), rejected(0, 1)]);
    trace.extend(edit_failed);
    trace.extend([
        READ_RAW.to_string(),
        rejected(0, 2),
        RECOVERY_SAVED.to_string(),
        stopped(0, 5),
    ]);
    trace
}

#[test]
fn condition_admits_only_carry_bearing_repairs_at_threshold_minus_one_or_above() {
    let repair = verify_repair(fresh_carry());
    let observation = RunSessionOptions::plan_step(RunSessionStepKind::Verify);
    let inspect = RunSessionOptions::plan_step(RunSessionStepKind::Inspect);
    let profile_repair = RunSessionOptions::plan_step(RunSessionStepKind::Implement);
    let final_repair = attach_to_options(
        RunSessionOptions::final_acceptance_repair(),
        EscalationCarryoverHandle::new(),
    );
    let minimal = attach_to_options(
        RunSessionOptions::default(),
        EscalationCarryoverHandle::new(),
    );

    for (streak, expected) in [
        (0, false),
        (THRESHOLD - 2, false),
        (THRESHOLD - 1, true),
        (THRESHOLD, true),
        (THRESHOLD + 1, true),
    ] {
        assert_eq!(
            repair_start_needs_write_guidance(&repair, streak),
            expected,
            "streak {streak}"
        );
        assert_eq!(
            repair_start_needs_write_guidance(&final_repair, streak),
            expected,
            "final repair streak {streak}"
        );
        for options in [&observation, &inspect, &profile_repair, &minimal] {
            assert!(!repair_start_needs_write_guidance(options, streak));
        }
    }
}

#[test]
fn guidance_text_promises_no_enforcement_and_no_new_target() {
    let repair = verify_repair(fresh_carry());
    let observation = RunSessionOptions::plan_step(RunSessionStepKind::Verify);
    let text = repair_start_write_guidance(None, &repair, THRESHOLD - 1, 6).unwrap();
    assert_eq!(text, REPAIR_START_WRITE_GUIDANCE);
    assert!(repair_start_write_guidance(None, &repair, THRESHOLD - 2, 6).is_none());
    assert!(repair_start_write_guidance(None, &observation, THRESHOLD - 1, 6).is_none());
    for forbidden in ["suspended", "rejected", "only", TARGET, "`"] {
        assert!(!text.contains(forbidden), "{forbidden}");
    }
    assert!(text.contains("Write/Edit"));
    assert!(text.contains("a write alone is not success"));
}

#[test]
fn seeded_verify_repair_first_request_carries_advisory_without_enforcement() {
    let run = run(6, verify_repair(fresh_carry()), vec![read(), write_fix()]);

    assert!(run.result.is_ok(), "{:?}", run.result);
    assert_eq!(run.last_user_message(0), REPAIR_START_WRITE_GUIDANCE);
    assert!(
        !run.request_has_advisory(1),
        "advisory is first-request only"
    );
    let seed = run.named("escalation_carryover");
    assert_eq!(seed.len(), 1);
    assert_eq!(seed[0]["initial_read_only_streak"], 6);
    assert_eq!(seed[0]["pre_advanced"], true);
    let queued = run.queued();
    assert_eq!(queued.len(), 1);
    assert_eq!(queued[0]["initial_read_only_streak"], 6);
    assert_eq!(queued[0]["write_required_threshold"], THRESHOLD);
    assert_eq!(queued[0]["repair_turn_budget"], 6);
    assert_eq!(queued[0]["enforcement_changed"], false);
    assert_eq!(queued[0]["session_scope"], "plan-run-step");
    assert_eq!(queued[0]["step_kind"], "verify");
    assert!(
        REPAIR_START_WRITE_GUIDANCE.starts_with(queued[0]["guidance"].as_str().unwrap()),
        "{queued:?}"
    );
    // The first Read still runs: no enforcement before the first batch.
    let events = run.events();
    let first_feedback = events
        .iter()
        .position(|event| event["event"] == "read_only_stagnation_feedback")
        .expect("write_required still follows the first Read");
    assert_eq!(events[first_feedback]["stage"], "write_required");
    assert_eq!(events[first_feedback]["read_only_streak"], 6);
    let queued_at = events
        .iter()
        .position(|event| event["event"] == "repair_start_write_guidance_queued")
        .unwrap();
    assert!(queued_at < first_feedback);
    assert!(run.named("read_only_tool_rejected").is_empty());
    assert_eq!(run.requests.len(), 2);
    assert_eq!(run.source(), FIXED);
}

#[test]
fn first_reply_write_fixes_the_source_after_the_advisory() {
    let run = run(6, verify_repair(fresh_carry()), vec![write_fix()]);

    let outcome = run.result.as_ref().expect("write completes the session");
    assert_eq!(outcome.changed_paths, vec![TARGET.to_string()]);
    assert_eq!(run.source(), FIXED);
    assert!(run.request_has_advisory(0));
    assert_eq!(run.requests.len(), 1);
    assert!(run.named("read_only_stagnation_feedback").is_empty());
    assert!(run.named("read_only_tool_rejected").is_empty());
}

#[test]
fn reads_after_the_advisory_stop_exactly_as_before() {
    let run = run(6, verify_repair(fresh_carry()), reads(8));

    let err = run
        .result
        .as_ref()
        .expect_err("read-only repair still stops");
    assert!(err.contains("model_stagnation:read_only_loop"), "{err}");
    assert_eq!(run.decision_trace(), read_only_trace_before());
    assert_eq!(run.source(), BROKEN);
    assert_eq!(run.workspace_files(), workspace_files_before_stop());
    let recovery = run.recovery_prompt();
    assert!(recovery.contains("verify-types"), "{recovery}");
    assert!(!recovery.contains(REPAIR_START_WRITE_GUIDANCE));
    assert_eq!(run.requests.len(), 3);
    assert!(run.request_has_advisory(0));
    for index in 1..run.requests.len() {
        assert!(!run.request_has_advisory(index), "request {index}");
    }
    // The second request still carries the existing write_required feedback.
    assert!(
        run.last_user_message(1)
            .contains("Read-only stagnation has escalated to write_required")
    );
}

/// Files the pre-#903 loop (develop 56f63cc5) left after a read_only_loop stop:
/// the untouched source and the recovery artifacts.
fn workspace_files_before_stop() -> Vec<String> {
    vec![
        ".commandagent/plans/recovery-ultra-plan-read-only-stagnation-<id>.yaml".to_string(),
        ".commandagent/repairs/repair-read-only-stagnation-<id>.md".to_string(),
        TARGET.to_string(),
    ]
}

#[test]
fn failed_anchor_edits_keep_existing_counters_and_stop() {
    let run = run(
        6,
        verify_repair(fresh_carry()),
        vec![
            read(),
            edit_missing_anchor(),
            read(),
            edit_missing_anchor(),
            read(),
            read(),
            read(),
        ],
    );

    let err = run.result.as_ref().expect_err("anchor failures still stop");
    assert!(err.contains("model_stagnation:read_only_loop"), "{err}");
    assert_eq!(run.decision_trace(), anchor_trace_before());
    assert_eq!(run.source(), BROKEN, "a failed Edit is not a write");
    assert_eq!(run.workspace_files(), workspace_files_before_stop());
    assert_eq!(run.requests.len(), 5);
    assert!(run.request_has_advisory(0));
    assert_eq!(run.queued().len(), 1);
}

#[test]
fn write_that_leaves_the_failure_does_not_turn_into_success() {
    let run = run(
        6,
        verify_repair(fresh_carry()),
        vec![tool_reply(
            "Write",
            json!({"path": "src/unrelated.ts", "content": "export {};\n"}),
        )],
    );

    let outcome = run.result.as_ref().expect("session returns to the host");
    assert_eq!(outcome.changed_paths, vec!["src/unrelated.ts".to_string()]);
    assert_eq!(run.source(), BROKEN, "the original failure is kept");
    assert!(run.request_has_advisory(0));
}

#[test]
fn budget_relative_to_threshold_decides_the_advisory() {
    for (budget, advisory) in [
        (THRESHOLD - 1, true),
        (THRESHOLD, false),
        (THRESHOLD + 1, false),
    ] {
        let run = run(budget, verify_repair(fresh_carry()), vec![write_fix()]);
        assert_eq!(run.request_has_advisory(0), advisory, "budget {budget}");
        assert_eq!(run.queued().len(), usize::from(advisory), "budget {budget}");
        assert_eq!(run.source(), FIXED);
    }
}

#[test]
fn carried_streak_below_threshold_minus_one_gets_no_advisory() {
    for streak in [0, THRESHOLD - 2] {
        let options = attach_to_options(
            RunSessionOptions::plan_step(RunSessionStepKind::Implement),
            EscalationCarryoverHandle::from_pressure(carried_streak(streak)),
        );
        let run = run(THRESHOLD + 1, options, vec![write_fix()]);
        assert!(!run.request_has_advisory(0), "streak {streak}");
        assert!(run.queued().is_empty(), "streak {streak}");
    }
}

#[test]
fn real_carry_and_carried_exhaustion_both_get_the_advisory() {
    let real = run(
        THRESHOLD + 1,
        verify_repair(carried_streak(THRESHOLD - 1)),
        vec![write_fix()],
    );
    let seed = real.named("escalation_carryover");
    assert_eq!(seed[0]["pre_advanced"], false);
    assert_eq!(seed[0]["carried_streak"], THRESHOLD - 1);
    assert!(real.request_has_advisory(0));

    let exhausted = run(
        THRESHOLD + 1,
        verify_repair(CarriedPressure {
            write_required_exhausted: true,
            ..CarriedPressure::default()
        }),
        vec![write_fix()],
    );
    let seed = exhausted.named("escalation_carryover");
    assert_eq!(seed[0]["carried_write_required_exhausted"], true);
    assert_eq!(seed[0]["initial_read_only_streak"], THRESHOLD - 1);
    assert!(exhausted.request_has_advisory(0));
}

#[test]
fn stronger_first_request_feedback_wins_and_keeps_its_counters() {
    let at_threshold = run(
        THRESHOLD + 1,
        verify_repair(carried_streak(THRESHOLD)),
        vec![write_fix()],
    );
    assert!(!at_threshold.request_has_advisory(0));
    assert!(at_threshold.queued().is_empty());
    let feedback = at_threshold.named("read_only_stagnation_feedback");
    assert_eq!(feedback[0]["stage"], "write_required");
    assert_eq!(feedback[0]["read_only_streak"], THRESHOLD);
    assert!(
        at_threshold
            .last_user_message(0)
            .contains("Read-only stagnation has escalated to write_required")
    );

    let anchor = run(
        THRESHOLD + 1,
        verify_repair(CarriedPressure {
            read_only_streak: THRESHOLD,
            anchor_failures: 1,
            anchor_target: Some(TARGET.to_string()),
            ..CarriedPressure::default()
        }),
        vec![write_fix()],
    );
    assert!(!anchor.request_has_advisory(0));
    assert!(anchor.queued().is_empty());
    let feedback = anchor.named("read_only_stagnation_feedback");
    assert_eq!(feedback[0]["selection_reason"], "anchor_failure");
}

#[test]
fn implement_repair_at_threshold_gets_the_advisory() {
    let options = attach_to_options(
        RunSessionOptions::plan_step(RunSessionStepKind::Implement),
        EscalationCarryoverHandle::from_pressure(carried_streak(THRESHOLD)),
    );
    let run = run(THRESHOLD + 1, options, vec![write_fix()]);
    assert!(run.request_has_advisory(0));
    assert_eq!(run.queued()[0]["initial_read_only_streak"], THRESHOLD);
}

#[test]
fn sessions_without_carryover_get_no_advisory() {
    for options in [
        RunSessionOptions::plan_step(RunSessionStepKind::Verify),
        RunSessionOptions::plan_step(RunSessionStepKind::Inspect),
        RunSessionOptions::plan_step(RunSessionStepKind::Implement),
        RunSessionOptions::final_acceptance_repair(),
        RunSessionOptions::default(),
    ] {
        let run = run(THRESHOLD - 1, options, vec![write_fix()]);
        assert!(!run.request_has_advisory(0));
        assert!(run.queued().is_empty());
        assert!(run.named("escalation_carryover").is_empty());
    }
}

#[test]
fn carry_bearing_final_acceptance_repair_gets_the_advisory_and_still_needs_a_write() {
    let options = attach_to_options(
        RunSessionOptions::final_acceptance_repair(),
        EscalationCarryoverHandle::new(),
    );
    let run = run(THRESHOLD - 1, options, vec![write_fix()]);
    assert!(run.request_has_advisory(0));
    assert_eq!(run.queued()[0]["step_kind"], "implement");
    assert_eq!(run.source(), FIXED);
}

#[test]
fn native_and_xml_fallback_requests_both_carry_the_advisory() {
    let native = run_with(true, 6, verify_repair(fresh_carry()), vec![write_fix()]);
    assert!(native.requests[0].native_tools);
    assert!(native.requests[0].tool_count > 0);
    assert_eq!(native.last_user_message(0), REPAIR_START_WRITE_GUIDANCE);

    let xml = run_with(
        false,
        6,
        verify_repair(fresh_carry()),
        vec![AssistantReply::text(format!(
            "<anvil_tool_call name=\"Write\">{}</anvil_tool_call>",
            json!({"path": TARGET, "content": FIXED})
        ))],
    );
    assert!(!xml.requests[0].native_tools);
    assert_eq!(xml.requests[0].tool_count, 0);
    assert_eq!(xml.last_user_message(0), REPAIR_START_WRITE_GUIDANCE);
    assert!(xml.requests[0].messages[0].content.contains("Tools:"));
}
