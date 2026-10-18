use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use commandagent::config::{
    Action, Config, ConfigFieldSources, NarrationMode, OllamaThink, OpenAiApi, PlanPreset,
    PromptLayout, Provider,
};
use commandagent::providers::{AssistantReply, ChatClient};
use commandagent::state::ConversationMessage;
use commandagent::tools::registry::ToolSpec;
use commandagent::tui::slash::{
    SLASH_COMMANDS, SlashCommandKind, handle_command, is_exit_command, parse_profile_style,
    parse_slash, parse_words, render_command_help, render_help, slash_command_spec,
};
use commandagent::tui::status::UiStatus;
use commandagent::tui::{InteractionUi, UiGuard};

static LOCK: Mutex<()> = Mutex::new(());

fn lock() -> MutexGuard<'static, ()> {
    LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn config(root: &Path, events: Option<PathBuf>) -> Config {
    Config {
        workspace_root: root.to_path_buf(),
        state_dir: root.join("state"),
        eval_events_path: events,
        completion_contract_path: None,
        yes: true,
        offline: false,
        context_budget: 1000,
        model: "m".to_string(),
        provider: Provider::Ollama,
        tool_protocol: None,
        openai_api: OpenAiApi::ChatCompletions,
        prompt_layout: PromptLayout::Stable,
        plan_preset: PlanPreset::None,
        intent_override: None,
        planner_model: "pm".to_string(),
        planner_provider: Provider::Gemini,
        planner_think: Some(OllamaThink::False),
        classifier_model: "pm".to_string(),
        classifier_provider: Provider::Gemini,
        openai_compatible: None,
        ollama_host: "http://localhost:11434".to_string(),
        ollama_think: None,
        lm_studio_host: "http://localhost:1234".to_string(),
        num_predict: 100,
        max_iterations: 4,
        recovery_plan_auto_runs: 0,
        chat_timeout_secs: 1,
        chat_timeout_source: "override:test".to_string(),
        field_sources: ConfigFieldSources::default(),
        chat_retries: 1,
        stream: false,
        resume: None,
        fresh_session: false,
        no_footer: false,
        narration: NarrationMode::Normal,
        profile: "generic".to_string(),
        profile_explicit: false,
        profile_inference: None,
        style: "default".to_string(),
        action: Action::Repl,
    }
}

#[derive(Clone)]
struct FailingClient {
    label: &'static str,
    calls: Arc<AtomicUsize>,
}

impl FailingClient {
    fn new(label: &'static str) -> Self {
        Self {
            label,
            calls: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl ChatClient for FailingClient {
    fn label(&self) -> &str {
        self.label
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
        self.calls.fetch_add(1, Ordering::SeqCst);
        anyhow::bail!("provider unavailable ({})", self.label)
    }
}

struct RecordingUi {
    receipts: Mutex<Vec<String>>,
    interrupted: AtomicBool,
}

impl RecordingUi {
    fn new(interrupted: bool) -> Self {
        Self {
            receipts: Mutex::new(Vec::new()),
            interrupted: AtomicBool::new(interrupted),
        }
    }

    fn receipts(&self) -> Vec<String> {
        self.receipts.lock().unwrap().clone()
    }
}

impl InteractionUi for RecordingUi {
    fn before_model_call(&self, _label: &str) -> UiGuard {
        UiGuard::noop()
    }

    fn before_tool_call(&self, _name: &str) -> UiGuard {
        UiGuard::noop()
    }

    fn publish_status(&self, _status: UiStatus) {}

    fn render_command_receipt(&self, receipt: &str) -> anyhow::Result<()> {
        self.receipts.lock().unwrap().push(receipt.to_string());
        Ok(())
    }

    fn interrupted(&self) -> bool {
        self.interrupted.load(Ordering::SeqCst)
    }
}

struct Outcome {
    result: Result<String, String>,
    events: Vec<serde_json::Value>,
    planner_calls: usize,
    execution_calls: usize,
    receipts: Vec<String>,
}

impl Outcome {
    fn names(&self) -> Vec<&str> {
        self.events
            .iter()
            .map(|event| event["event"].as_str().unwrap_or(""))
            .collect()
    }

    fn event(&self, name: &str) -> &serde_json::Value {
        self.events
            .iter()
            .find(|event| event["event"] == name)
            .unwrap_or_else(|| panic!("missing event {name}: {:?}", self.names()))
    }
}

fn run(line: &str, interrupted: bool) -> Outcome {
    run_in(line, interrupted, |_| {})
}

fn run_in(line: &str, interrupted: bool, setup: impl FnOnce(&Path)) -> Outcome {
    let _guard = lock();
    let dir = tempfile::tempdir().unwrap();
    setup(dir.path());
    let events_path = dir.path().join(".anvil/runs/test/events.jsonl");
    let cfg = config(dir.path(), Some(events_path.clone()));
    let mut planner = FailingClient::new("planner");
    let mut execution = FailingClient::new("exec");
    let ui = RecordingUi::new(interrupted);
    let result = handle_command(line, &cfg, &mut planner, &mut execution, &ui)
        .map_err(|err| err.to_string());
    let events = std::fs::read_to_string(&events_path)
        .map(|raw| {
            raw.lines()
                .map(|line| serde_json::from_str(line).unwrap())
                .collect()
        })
        .unwrap_or_default();
    Outcome {
        result,
        events,
        planner_calls: planner.calls(),
        execution_calls: execution.calls(),
        receipts: ui.receipts(),
    }
}

fn pick(event: &serde_json::Value, keys: &[&str]) -> String {
    keys.iter()
        .map(|key| format!("{key}={}", event[*key]))
        .collect::<Vec<_>>()
        .join(" ")
}

const PROVIDER_REASON: &str =
    "provider request failed after 2 attempts: provider unavailable (planner)";

fn failed_block(reason: &str) -> String {
    format!(
        "================ TASK FAILED (process exited normally) ================\n\
Task status: failed\n\
Failed phase: unknown\n\
Phases completed: 0/1\n\
Primary stop reason: {reason}\n\
Recovery prompt command: missing\n\
Recovery UltraPlan command: missing\n\
Run summary: .anvil/runs/test/summary.md\n\
======================================================================="
    )
}

fn receipt(
    input: &str,
    command: &str,
    goal: &str,
    profile: &str,
    style: &str,
    layout: &str,
) -> String {
    format!(
        "Accepted command\n- Input: {input}\n- Command: {command}\n- Goal: {goal}\n- Profile: {profile}\n- Style: {style}\n- Prompt layout: {layout}\n- Run ID: test"
    )
}

fn stop_fields(outcome: &Outcome) -> String {
    pick(
        outcome.event("tui_command_stop"),
        &[
            "command",
            "ok",
            "status",
            "failure_kind",
            "stop_reason",
            "session_status",
            "task_status",
            "lifecycle_stage",
        ],
    )
}

fn start_fields(outcome: &Outcome) -> String {
    pick(
        outcome.event("tui_command_start"),
        &[
            "command",
            "profile",
            "prompt_layout",
            "profile_inferred",
            "profile_inference_source",
            "style",
            "goal",
            "lifecycle_stage",
        ],
    )
}

fn assert_no_side_effects(outcome: &Outcome) {
    assert!(outcome.events.is_empty(), "{:?}", outcome.names());
    assert_eq!(outcome.planner_calls, 0);
    assert_eq!(outcome.execution_calls, 0);
}

#[test]
fn input_that_is_not_a_known_slash_command_never_reaches_the_dispatcher() {
    let empty = run("", false);
    assert_eq!(empty.result, Err("empty command".to_string()));
    assert!(empty.receipts.is_empty());
    assert_no_side_effects(&empty);

    let blank = run("   \t ", false);
    assert_eq!(blank.result, Err("empty command".to_string()));
    assert!(blank.receipts.is_empty());

    let plain = run("hello", false);
    assert_eq!(
        plain.result,
        Ok("Input was not run: hello\nUse /ultra-plan-run <goal> or /plan-run <goal>.".to_string())
    );
    assert!(plain.receipts.is_empty());
    assert_no_side_effects(&plain);

    let unknown = run("/nope", false);
    assert_eq!(
        unknown.result,
        Ok("Unknown command: /nope\nType /help for all commands.".to_string())
    );
    assert!(unknown.receipts.is_empty());
    assert_no_side_effects(&unknown);

    let typo = run("/hepl", false);
    let typo = typo.result.clone().unwrap();
    assert_eq!(typo.lines().count(), 2, "{typo}");
    assert!(typo.starts_with("Unknown command: /hepl"), "{typo}");
    assert!(typo.contains("Did you mean /help?"), "{typo}");

    // An interrupt flag must not change input that never starts a command.
    let interrupted = run("/nope", true);
    assert_eq!(interrupted.result, unknown.result);
}

#[test]
fn help_output_is_fixed_for_the_listing_and_every_topic() {
    let listing = run("/help", false);
    assert_eq!(listing.result, Ok(render_help()));
    let rendered = listing.result.clone().unwrap();
    assert!(rendered.starts_with("Commands:\nSession:\n/help [command] - show grouped commands or detailed command help\n/status - show effective configuration and readiness\n/clear - clear the terminal screen\n/last - show the most recent REPL result again\n/exit or /quit - leave the TUI\nConfiguration:\n"));
    assert!(rendered.contains("Planning and execution:\n/confirm <hash> - confirm and execute the reviewed Gate 1 card\n/plan-steps <goal> - write a step plan\n/plan-run <goal> - generate and run a step plan\n/run-plan <path> - run an existing step plan\n/ultra-plan <goal> - write an UltraPlan\n/ultra-plan-run <goal> - generate and run an UltraPlan\n/run-ultra-plan <path> - run an existing UltraPlan\nRecovery and inspection:\n/runs - list recent runs and recovery availability\n/resume [run-id|yaml-path] - resume from a recovery UltraPlan\n/plan - show the active plan and current activity\nDiagnostics:\n"));
    assert!(rendered.ends_with("Interrupt: Ctrl-C always interrupts; Esc clears non-empty pending input, otherwise interrupts; a repeated interrupt force-finalizes."));
    assert_eq!(listing.receipts.len(), 1);
    assert_no_side_effects(&listing);

    assert_eq!(
        run("/help status", false).result,
        Ok("### /status\nUsage: /status\nshow effective configuration and readiness\nExample: /status".to_string())
    );
    for spec in SLASH_COMMANDS {
        let topic = spec.name.trim_start_matches('/');
        let outcome = run(&format!("/help {topic}"), false);
        assert_eq!(
            outcome.result,
            render_command_help(topic).map_err(|err| err.to_string()),
            "{topic}"
        );
        let text = outcome.result.clone().unwrap();
        assert!(
            text.starts_with(&format!(
                "### {}\nUsage: {}\n{}\n",
                spec.name, spec.help_usage, spec.description
            )),
            "{text}"
        );
        assert_eq!(outcome.receipts.len(), 1);
        assert_no_side_effects(&outcome);
    }
    assert_eq!(
        run("/help a b", false).result,
        Err("usage: /help [command]".to_string())
    );
    assert_eq!(
        run("/help --style compact", false).result,
        Err("usage: /help [command]".to_string())
    );
    assert_eq!(
        run("/help /nope", false).result,
        Err("unknown help topic `/nope`; use /help to list commands".to_string())
    );
}

#[test]
fn commands_that_do_not_run_report_fixed_text_without_events_or_provider_calls() {
    let cases: [(&str, Result<&str, &str>); 12] = [
        (
            "/confirm abc",
            Err("/confirm is available only after a Gate 1 card; type a plain-text request first"),
        ),
        (
            "/model x",
            Err("/model is available only in the interactive REPL"),
        ),
        (
            "/provider x",
            Err("/provider is available only in the interactive REPL"),
        ),
        (
            "/profile x",
            Err("/profile is available only in the interactive REPL"),
        ),
        (
            "/clear",
            Err("/clear is available only in the interactive REPL"),
        ),
        (
            "/last",
            Err("/last is available only in the interactive REPL"),
        ),
        (
            "/pack a@1",
            Err("/pack is available only from Gate 4; use /pack <id@version>"),
        ),
        ("/packs", Err("profile `generic` does not support packs")),
        ("/runs", Ok("No runs found for this workspace.")),
        ("/exit", Ok("")),
        ("/quit", Ok("")),
        ("/exit now", Ok("")),
    ];
    for (line, expected) in cases {
        for interrupted in [false, true] {
            let outcome = run(line, interrupted);
            assert_eq!(
                outcome.result,
                expected.map(str::to_string).map_err(str::to_string),
                "{line} interrupted={interrupted}"
            );
            assert_eq!(outcome.receipts.len(), 1, "{line}");
            assert_no_side_effects(&outcome);
        }
    }
    let plan = run("/plan", false);
    assert!(plan.result.is_ok());
    assert_eq!(plan.receipts.len(), 1);
    assert_no_side_effects(&plan);

    let status = run("/status", false).result.clone().unwrap();
    assert!(
        status.starts_with("### Status\n#### Current execution\n"),
        "{status}"
    );
    assert!(status.contains("\n#### Session configuration\n- Model: m (default)\n- Provider: ollama (default)\n- Planner model: pm (default)\n- Planner provider: gemini (default)\n- Timeout: 1s (default)\n- Context budget: 1000 (default)\n- Profile: generic (default)\n"), "{status}");

    let doctor = run("/doctor", false);
    assert!(
        doctor
            .result
            .clone()
            .unwrap()
            .starts_with("CommandAgent doctor: ")
    );
    assert_eq!(doctor.receipts.len(), 1);
    assert_no_side_effects(&doctor);
}

#[test]
fn receipt_and_start_event_carry_the_parsed_arguments() {
    let cases = [
        (
            "/plan-steps goal",
            receipt(
                "/plan-steps goal",
                "/plan-steps",
                "goal",
                "generic (effective)",
                "default (effective)",
                "stable (effective)",
            ),
            r#"command="/plan-steps" profile="generic" prompt_layout="stable" profile_inferred="" profile_inference_source="" style="default" goal="goal" lifecycle_stage="tui_command""#,
        ),
        (
            "/plan-steps --profile nextjs --style compact goal",
            receipt(
                "/plan-steps --profile nextjs --style compact goal",
                "/plan-steps",
                "goal",
                "nextjs (explicit)",
                "compact (explicit)",
                "stable (effective)",
            ),
            r#"command="/plan-steps" profile="nextjs" prompt_layout="stable" profile_inferred="" profile_inference_source="" style="compact" goal="goal" lifecycle_stage="tui_command""#,
        ),
        (
            "/plan-steps --prompt-layout legacy goal",
            receipt(
                "/plan-steps --prompt-layout legacy goal",
                "/plan-steps",
                "goal",
                "generic (effective)",
                "default (effective)",
                "legacy (explicit)",
            ),
            r#"command="/plan-steps" profile="generic" prompt_layout="legacy" profile_inferred="" profile_inference_source="" style="default" goal="goal" lifecycle_stage="tui_command""#,
        ),
        (
            "/plan-steps --prompt-layout bogus goal",
            receipt(
                "/plan-steps --prompt-layout bogus goal",
                "/plan-steps",
                "goal",
                "generic (effective)",
                "default (effective)",
                "stable (explicit)",
            ),
            r#"command="/plan-steps" profile="generic" prompt_layout="stable" profile_inferred="" profile_inference_source="" style="default" goal="goal" lifecycle_stage="tui_command""#,
        ),
        (
            "/plan-steps --profile",
            receipt(
                "/plan-steps --profile",
                "/plan-steps",
                "--profile",
                "generic (effective)",
                "default (effective)",
                "stable (effective)",
            ),
            r#"command="/plan-steps" profile="generic" prompt_layout="stable" profile_inferred="" profile_inference_source="" style="default" goal="--profile" lifecycle_stage="tui_command""#,
        ),
    ];
    for (line, expected_receipt, expected_start) in cases {
        let outcome = run(line, false);
        assert_eq!(outcome.receipts, vec![expected_receipt], "{line}");
        assert_eq!(start_fields(&outcome), expected_start, "{line}");
        assert_eq!(outcome.result, Err(failed_block(PROVIDER_REASON)), "{line}");
    }
    // Arguments of commands that do not run still reach the receipt.
    let status = run("/status --profile nextjs", false);
    assert_eq!(
        status.receipts,
        vec![format!(
            "Accepted command\n- Input: /status --profile nextjs\n- Command: /status\n- Goal: (none)\n- Profile: nextjs (explicit)\n- Style: default (effective)\n- Prompt layout: stable (effective)\n- Run ID: test"
        )]
    );
    assert!(
        status
            .result
            .unwrap()
            .contains("- Profile: nextjs (default)\n- Inference: explicit\n")
    );
    let help = run("/help --style compact", false);
    assert_eq!(
        help.receipts,
        vec![receipt(
            "/help --style compact",
            "/help",
            "(none)",
            "generic (effective)",
            "compact (explicit)",
            "stable (effective)"
        )]
    );
}

#[test]
fn goal_references_are_expanded_before_the_receipt_and_rejected_before_any_event() {
    let ok = run_in("/plan-steps $(cat g.txt)", false, |root| {
        std::fs::write(root.join("g.txt"), "from file").unwrap();
    });
    assert_eq!(
        ok.receipts,
        vec![receipt(
            "/plan-steps $(cat g.txt)",
            "/plan-steps",
            "from file",
            "generic (effective)",
            "default (effective)",
            "stable (effective)"
        )]
    );
    assert!(start_fields(&ok).contains(r#"goal="from file""#));

    let missing = run("/plan-steps $(cat nofile.txt)", false);
    assert_eq!(
        missing.result,
        Err("path does not exist: nofile.txt".to_string())
    );
    assert!(missing.receipts.is_empty());
    assert_no_side_effects(&missing);

    let escaping = run("/plan-steps $(cat ../outside.txt)", false);
    assert!(escaping.result.is_err());
    assert!(escaping.receipts.is_empty());
    assert_no_side_effects(&escaping);
}

#[test]
fn goal_inference_emits_profile_inferred_before_the_command_start() {
    let outcome = run("/plan-steps build a Next.js app on port 3011", false);
    assert_eq!(
        outcome.names(),
        vec![
            "profile_inferred",
            "tui_command_start",
            "provider_turn_duration",
            "provider_turn_duration",
            "tui_command_stop",
            "loop_stop"
        ]
    );
    let inferred = &outcome.events[0];
    assert_eq!(inferred["command"], "/plan-steps");
    assert_eq!(inferred["from"], "goal");
    assert_eq!(inferred["profile"], "nextjs");
    assert_eq!(inferred["lifecycle_stage"], "tui_command");
    assert_eq!(
        start_fields(&outcome),
        r#"command="/plan-steps" profile="nextjs" prompt_layout="stable" profile_inferred="nextjs" profile_inference_source="goal" style="default" goal="build a Next.js app on port 3011" lifecycle_stage="tui_command""#
    );
    assert_eq!(
        outcome.receipts,
        vec![
            "Accepted command\n- Input: /plan-steps build a Next.js app on port 3011\n- Command: /plan-steps\n- Goal: build a Next.js app on port 3011\n- Profile: nextjs (inferred)\n- Style: default (effective)\n- Prompt layout: stable (effective)\n- Requested port: 3011 (goal)\n- Run ID: test"
        ]
    );

    // An explicit profile suppresses inference.
    let explicit = run("/plan-steps --profile generic build a Next.js app", false);
    assert!(!explicit.names().contains(&"profile_inferred"));
    assert!(start_fields(&explicit).contains(r#"profile="generic""#));
}

#[test]
fn each_executing_command_reports_its_own_failure_with_ordered_events() {
    let provider_failure_events = vec![
        "tui_command_start",
        "provider_turn_duration",
        "provider_turn_duration",
        "tui_command_stop",
        "loop_stop",
    ];
    let generation_failure_events = vec![
        "tui_command_start",
        "ultra_plan_generation_attempt",
        "provider_turn_duration",
        "provider_turn_duration",
        "tui_command_stop",
        "loop_stop",
    ];
    let pre_run_events = vec!["tui_command_start", "tui_command_stop", "loop_stop"];
    // (line, event order, planner calls, reason)
    let cases = [
        (
            "/plan-steps goal",
            provider_failure_events.clone(),
            2,
            PROVIDER_REASON.to_string(),
        ),
        (
            "/plan-run goal",
            provider_failure_events,
            2,
            PROVIDER_REASON.to_string(),
        ),
        (
            "/ultra-plan goal",
            generation_failure_events.clone(),
            2,
            PROVIDER_REASON.to_string(),
        ),
        (
            "/ultra-plan-run goal",
            generation_failure_events,
            2,
            PROVIDER_REASON.to_string(),
        ),
        (
            "/run-plan missing.yaml",
            pre_run_events.clone(),
            0,
            "path does not exist: missing.yaml".to_string(),
        ),
        (
            "/run-ultra-plan missing.yaml",
            pre_run_events.clone(),
            0,
            "path does not exist: missing.yaml".to_string(),
        ),
        (
            "/ultra-plan",
            pre_run_events.clone(),
            0,
            "/ultra-plan requires a goal; usage: /ultra-plan <goal>".to_string(),
        ),
        (
            "/resume",
            pre_run_events,
            0,
            "no resumable run exists: no recovery UltraPlan was found; run /runs".to_string(),
        ),
    ];
    for (line, events, planner_calls, reason) in cases {
        let outcome = run(line, false);
        assert_eq!(outcome.names(), events, "{line}");
        assert_eq!(outcome.planner_calls, planner_calls, "{line}");
        assert_eq!(outcome.execution_calls, 0, "{line}");
        assert_eq!(outcome.receipts.len(), 1, "{line}");
        assert_eq!(outcome.result, Err(failed_block(&reason)), "{line}");
        let command = line.split(' ').next().unwrap();
        assert_eq!(
            stop_fields(&outcome),
            format!(
                r#"command="{command}" ok=false status="failed" failure_kind="tui_command_failed" stop_reason={:?} session_status="repl_ready" task_status="failed" lifecycle_stage="tui_command""#,
                reason
            ),
            "{line}"
        );
        let loop_stop = outcome.event("loop_stop");
        assert_eq!(loop_stop["reason"], "tui_command_failed", "{line}");
        assert_eq!(loop_stop["primary_reason"], reason.as_str(), "{line}");
    }
}

#[test]
fn model_probe_runs_as_a_command_without_a_terminal_summary() {
    let outcome = run("/model-probe", false);
    assert_eq!(
        outcome.names(),
        vec!["tui_command_start", "tui_command_stop"]
    );
    assert_eq!(outcome.planner_calls, 2);
    assert_eq!(outcome.execution_calls, 10);
    assert_eq!(outcome.receipts.len(), 1);
    let output = outcome.result.clone().unwrap();
    assert!(output.starts_with("# Model Probe Card\n"), "{output}");
    assert!(!output.contains("Terminal summary"), "{output}");
    assert!(
        stop_fields(&outcome)
            .starts_with(r#"command="/model-probe" ok=true status="completed" failure_kind="""#),
        "{}",
        stop_fields(&outcome)
    );
}

#[test]
fn interrupt_is_classified_for_every_executing_command_and_ignored_by_the_rest() {
    // The provider is never reached with the flag raised, so commands that
    // fail before it keep their own cause as the stop reason.
    for (line, reason) in [
        ("/plan-steps goal", "interrupted by user"),
        ("/plan-run goal", "interrupted by user"),
        ("/ultra-plan goal", "interrupted by user"),
        ("/ultra-plan-run goal", "interrupted by user"),
        (
            "/run-plan missing.yaml",
            "path does not exist: missing.yaml",
        ),
        (
            "/run-ultra-plan missing.yaml",
            "path does not exist: missing.yaml",
        ),
        (
            "/ultra-plan",
            "/ultra-plan requires a goal; usage: /ultra-plan <goal>",
        ),
        (
            "/resume",
            "no resumable run exists: no recovery UltraPlan was found; run /runs",
        ),
    ] {
        let outcome = run(line, true);
        assert_eq!(
            outcome.result,
            Err(format!(
                "### INTERRUPTED\n- Stop reason: {reason}\n- Rerun: {line}"
            )),
            "{line}"
        );
        assert_eq!(
            outcome.names().first().copied(),
            Some("tui_command_start"),
            "{line}"
        );
        assert_eq!(
            &outcome.names()[outcome.names().len() - 2..],
            ["tui_command_stop", "loop_stop"],
            "{line}"
        );
        let stop = stop_fields(&outcome);
        assert!(
            stop.contains(
                r#"ok=false status="interrupted" failure_kind="tui_command_interrupted""#
            ),
            "{line}: {stop}"
        );
        assert!(
            stop.contains("task_status=\"interrupted\""),
            "{line}: {stop}"
        );
        let loop_stop = outcome.event("loop_stop");
        assert_eq!(loop_stop["reason"], "tui_command_interrupted", "{line}");
        assert_eq!(outcome.receipts.len(), 1, "{line}");
    }
    // /model-probe succeeds even when the flag is raised: only an error result
    // is reclassified as interrupted.
    let probe = run("/model-probe", true);
    assert_eq!(probe.names(), vec!["tui_command_start", "tui_command_stop"]);
    assert!(probe.result.is_ok());
}

#[test]
fn slash_command_table_is_ordered_and_resolves_aliases() {
    let expected = [
        ("/help", SlashCommandKind::Help),
        ("/confirm", SlashCommandKind::Confirm),
        ("/status", SlashCommandKind::Status),
        ("/model", SlashCommandKind::Model),
        ("/provider", SlashCommandKind::Provider),
        ("/profile", SlashCommandKind::Profile),
        ("/clear", SlashCommandKind::Clear),
        ("/last", SlashCommandKind::Last),
        ("/doctor", SlashCommandKind::Doctor),
        ("/packs", SlashCommandKind::Packs),
        ("/pack", SlashCommandKind::Pack),
        ("/runs", SlashCommandKind::Runs),
        ("/resume", SlashCommandKind::Resume),
        ("/plan", SlashCommandKind::Plan),
        ("/plan-steps", SlashCommandKind::PlanSteps),
        ("/plan-run", SlashCommandKind::PlanRun),
        ("/run-plan", SlashCommandKind::RunPlan),
        ("/ultra-plan", SlashCommandKind::UltraPlan),
        ("/ultra-plan-run", SlashCommandKind::UltraPlanRun),
        ("/run-ultra-plan", SlashCommandKind::RunUltraPlan),
        (
            "/setup-interaction-probe",
            SlashCommandKind::SetupInteractionProbe,
        ),
        ("/model-probe", SlashCommandKind::ModelProbe),
        ("/exit", SlashCommandKind::Exit),
    ];
    assert_eq!(SLASH_COMMANDS.len(), expected.len());
    for (spec, (name, kind)) in SLASH_COMMANDS.iter().zip(expected) {
        assert_eq!(spec.name, name);
        assert_eq!(spec.kind, kind, "{name}");
        assert_eq!(slash_command_spec(name).map(|found| found.kind), Some(kind));
        assert_eq!(
            is_exit_command(name),
            kind == SlashCommandKind::Exit,
            "{name}"
        );
    }
    assert_eq!(
        slash_command_spec("/quit").map(|spec| spec.name),
        Some("/exit")
    );
    assert!(is_exit_command("/quit"));
    assert!(slash_command_spec("/nope").is_none());
    assert!(slash_command_spec("exit").is_none());
    assert!(!is_exit_command("/plan"));
}

#[test]
fn parse_slash_resolves_arguments_profile_and_goal() {
    let _guard = lock();
    let dir = tempfile::tempdir().unwrap();
    let cfg = config(dir.path(), None);
    assert_eq!(
        parse_slash("", &cfg).unwrap_err().to_string(),
        "empty command"
    );
    let parsed = parse_slash(
        r#"/plan-run --profile nextjs --style compact "3011 port app""#,
        &cfg,
    )
    .unwrap();
    assert_eq!(parsed.command, "/plan-run");
    assert_eq!(parsed.profile, "nextjs");
    assert!(parsed.profile_explicit);
    assert!(parsed.profile_inference.is_none());
    assert_eq!(parsed.style, "compact");
    assert!(parsed.style_explicit);
    assert_eq!(parsed.prompt_layout, PromptLayout::Stable);
    assert!(!parsed.prompt_layout_explicit);
    assert_eq!(parsed.goal, "3011 port app");

    let legacy = parse_slash("/plan-run --prompt-layout legacy do it", &cfg).unwrap();
    assert_eq!(legacy.prompt_layout, PromptLayout::Legacy);
    assert!(legacy.prompt_layout_explicit);
    assert_eq!(legacy.profile, "generic");
    assert!(!legacy.profile_explicit);
    assert_eq!(legacy.goal, "do it");

    let inferred = parse_slash("/plan-run build a Next.js app on port 3011", &cfg).unwrap();
    assert_eq!(inferred.profile, "nextjs");
    assert!(!inferred.profile_explicit);
    assert!(inferred.profile_inference.is_some());

    let bare = parse_slash("/exit", &cfg).unwrap();
    assert_eq!(bare.command, "/exit");
    assert_eq!(bare.goal, "");
    assert_eq!(bare.profile, "generic");

    assert_eq!(
        parse_words(r#"/ultra-plan-run --profile nextjs "3011 port app""#),
        vec!["/ultra-plan-run", "--profile", "nextjs", "3011 port app"]
    );
    assert_eq!(parse_words("a\tb  c"), vec!["a", "b", "c"]);
    assert_eq!(parse_words(r#"a "b c"#), vec!["a", "b c"]);
    let words = parse_words("--style compact --profile nextjs rest");
    let (profile, style, rest) = parse_profile_style(&words, &cfg);
    assert_eq!((profile.as_str(), style.as_str()), ("nextjs", "compact"));
    assert_eq!(rest, vec!["rest"]);
}
