use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use commandagent::config::{
    Action, Config, ConfigFieldSources, NarrationMode, OllamaThink, OpenAiApi, PlanPreset,
    PromptLayout, Provider,
};
use commandagent::minimal_loop::loop_run::run_session_with_required_paths_with_ui;
use commandagent::planner::{generate_step_plan_with_ui, run_ultra_plan_with_ui};
use commandagent::providers::{AssistantReply, ChatClient};
use commandagent::state::{ConversationMessage, SessionSnapshot, ToolCall};
use commandagent::tools::registry::ToolSpec;
use commandagent::tui::markdown::TerminalMarkdownRenderer;
use commandagent::tui::status::UiStatus;
use commandagent::tui::{InteractionUi, OutputRenderer, UiGuard};
use serde_json::json;

#[path = "support/tui_interrupt_sync.rs"]
mod tui_interrupt_sync;

use tui_interrupt_sync::{
    ProviderStartInterruptUi, ShellReadyInterruptUi, StartReleaseClient, shell_child_gone,
    shell_child_marker, shell_interrupt_command, shell_ready_marker, test_tempdir,
};

include!("support/tui_profile_fixture.rs");

#[derive(Clone)]
struct FailingClient {
    label: &'static str,
    message: &'static str,
    calls: Arc<AtomicUsize>,
}

impl FailingClient {
    fn new(label: &'static str, message: &'static str) -> Self {
        Self {
            label,
            message,
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

    fn supports_native_tools(&self, _model: &str) -> bool {
        true
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
        anyhow::bail!(self.message)
    }
}

struct PanicClient {
    label: &'static str,
    message: &'static str,
}

impl PanicClient {
    fn new(label: &'static str, message: &'static str) -> Self {
        Self { label, message }
    }
}

impl ChatClient for PanicClient {
    fn label(&self) -> &str {
        self.label
    }

    fn supports_native_tools(&self, _model: &str) -> bool {
        true
    }

    fn boxed_clone(&self) -> Box<dyn ChatClient> {
        Box::new(PanicClient {
            label: self.label,
            message: self.message,
        })
    }

    fn chat(
        &mut self,
        _model: &str,
        _messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        panic!("{}", self.message);
    }
}

struct InterruptAfterUi {
    events: Mutex<Vec<String>>,
    checks: AtomicUsize,
    interrupt_after: usize,
}

impl InterruptAfterUi {
    fn new(interrupt_after: usize) -> Self {
        Self {
            events: Mutex::new(Vec::new()),
            checks: AtomicUsize::new(0),
            interrupt_after,
        }
    }
}

impl InteractionUi for InterruptAfterUi {
    fn before_model_call(&self, label: &str) -> UiGuard {
        self.events.lock().unwrap().push(format!("model:{label}"));
        UiGuard::noop()
    }

    fn before_tool_call(&self, name: &str) -> UiGuard {
        self.events.lock().unwrap().push(format!("tool:{name}"));
        UiGuard::noop()
    }

    fn publish_status(&self, status: UiStatus) {
        self.events
            .lock()
            .unwrap()
            .push(format!("status:{}:{}", status.provider, status.model));
    }

    fn interrupted(&self) -> bool {
        self.checks.fetch_add(1, Ordering::SeqCst) >= self.interrupt_after
    }
}

fn assert_exactly_one_tui_stop(text: &str, status: &str) {
    let stops = tui_command_stop_events(text);
    assert_eq!(stops.len(), 1, "{text}");
    assert_eq!(
        stops[0].get("status").and_then(|value| value.as_str()),
        Some(status),
        "{text}"
    );
}

fn assert_in_order(text: &str, needles: &[&str]) {
    let mut offset = 0usize;
    for needle in needles {
        let Some(index) = text[offset..].find(needle) else {
            panic!("missing {needle:?} after byte {offset} in:\n{text}");
        };
        offset += index + needle.len();
    }
}

fn assert_terminal_summary(summary: &str, status: &str) {
    assert!(
        summary.starts_with(&format!("{}\n", commandagent::build_info::summary_line())),
        "{summary}"
    );
    let expected = format!("Status: {status}");
    assert_eq!(
        summary.lines().find(|line| line.starts_with("Status: ")),
        Some(expected.as_str()),
        "{summary}"
    );
    assert!(!summary.contains("Status: running"), "{summary}");
}

fn assert_recovery_artifacts_exist(root: &std::path::Path, events: &str) {
    let recovery_event = events
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .find(|event| {
            event
                .get("event")
                .and_then(|value| value.as_str())
                .is_some_and(|name| name == "recovery_prompt_saved")
        })
        .unwrap_or_else(|| panic!("missing recovery_prompt_saved event:\n{events}"));
    let prompt_path = recovery_event
        .get("recovery_prompt_path")
        .and_then(|value| value.as_str())
        .filter(|value| !value.is_empty())
        .expect("recovery prompt path");
    let plan_path = recovery_event
        .get("recovery_ultra_plan_path")
        .and_then(|value| value.as_str())
        .filter(|value| !value.is_empty())
        .expect("recovery ultra plan path");
    assert!(root.join(prompt_path).is_file(), "{prompt_path}");
    assert!(root.join(plan_path).is_file(), "{plan_path}");
}

fn two_phase_ultra_plan() -> commandagent::planner::ultra_plan::UltraPlan {
    commandagent::planner::ultra_plan::UltraPlan {
        goal: "build app".to_string(),
        profile: "generic".to_string(),
        style: "default".to_string(),
        intent: "create".to_string(),
        phases: vec![
            commandagent::planner::ultra_plan::UltraPhase {
                id: "p1".to_string(),
                prompt: "phase 1".to_string(),
            },
            commandagent::planner::ultra_plan::UltraPhase {
                id: "p2".to_string(),
                prompt: "phase 2".to_string(),
            },
        ],
    }
}

fn write_ultra_plan(root: &std::path::Path) -> String {
    let path = root.join("ultra.yaml");
    std::fs::write(
        &path,
        commandagent::planner::ultra_plan::render_ultra_plan(&two_phase_ultra_plan()),
    )
    .unwrap();
    "ultra.yaml".to_string()
}

fn implement_step_plan_json() -> String {
    let mut step_plan = commandagent::planner::step_plan::StepPlan::single("write app");
    step_plan.steps[0].kind = "implement".to_string();
    step_plan.steps[0]
        .expected_paths
        .push("app.jsx".to_string());
    serde_json::to_string(&step_plan).unwrap()
}

fn interactive_app_source(label: &str) -> String {
    format!(
        r#"import {{ useState }} from "react";
export default function App() {{
  const [items, setItems] = useState([]);
  return <form onSubmit={{(event) => {{ event.preventDefault(); setItems([...items, "{label}"]); }}}}><input onChange={{() => setItems([...items, "{label}"])}} /><button type="submit">Add</button></form>;
}}
"#
    )
}

use own_locale_process::Glyphs;

/// Issue #968: each test's body runs alone in a child process of this test binary, with
/// the locale set explicitly.
///
/// The bodies install process-wide presentation and capture state, and the presentation
/// glyphs follow `LC_ALL`, then `LANG` (`tui::terminal::utf8_locale`). In one shared
/// process a panic in one body poisoned the lock that serialised them and failed every later
/// test, and the ambient locale of the runner decided the glyphs. A child of its own leaves
/// nothing behind for the next test, whatever its body does, and its locale is the one the
/// test names. The UTF-8 group keeps every expectation as written; an ASCII control turns
/// the same expectations through `tui::glyphs::for_locale(.., false)` and keeps their order.
///
/// The parent passes only when the child exits successfully *and* the body ran to its end:
/// a child that ran no test (a name `--exact` does not match) is a failure.
mod own_locale_process {
    use std::process::{Command, Stdio};

    /// Set in the child: where it writes the test name once the body has returned.
    const CHILD_ENV: &str = "COMMANDAGENT_TUI_TEST_OWN_PROCESS_RAN";
    /// Set in the child: the glyphs the parent expects of it.
    const EXPECT_ENV: &str = "COMMANDAGENT_TUI_TEST_EXPECTED_GLYPHS";

    #[derive(Clone, Copy, Debug)]
    pub enum Glyphs {
        Utf8,
        Ascii,
    }

    impl Glyphs {
        pub fn lc_all(self) -> &'static str {
            match self {
                Glyphs::Utf8 => "C.UTF-8",
                Glyphs::Ascii => "C",
            }
        }

        fn name(self) -> &'static str {
            match self {
                Glyphs::Utf8 => "utf8",
                Glyphs::Ascii => "ascii",
            }
        }
    }

    /// In the parent, runs `test` alone in a child under `glyphs` and fails with it; in the
    /// child, runs `body`.
    pub fn run(test: &str, glyphs: Glyphs, body: fn()) {
        if let Some(ran) = std::env::var_os(CHILD_ENV) {
            body();
            std::fs::write(ran, test).unwrap();
            return;
        }
        if let Err(error) = spawn(test, glyphs, glyphs) {
            panic!("{error}");
        }
    }

    /// In a child, `text` as the glyphs its parent expects: unchanged for UTF-8, and
    /// `for_locale(text, false)` for ASCII.
    pub fn expected(text: &str) -> String {
        let use_utf8 = match std::env::var(EXPECT_ENV).as_deref() {
            Ok("utf8") => true,
            Ok("ascii") => false,
            other => panic!("expected glyphs are known only in a child; {EXPECT_ENV}={other:?}"),
        };
        commandagent::tui::glyphs::for_locale(text, use_utf8)
    }

    /// Runs `test` in a child with `LC_ALL` set for `locale` while it expects `expect`.
    /// Returns the child's output when it passed and ran the body to its end.
    pub fn spawn(test: &str, locale: Glyphs, expect: Glyphs) -> Result<String, String> {
        let scratch = tempfile::tempdir().map_err(|error| error.to_string())?;
        let ran = scratch.path().join("ran");
        let executable = std::env::current_exe().map_err(|error| error.to_string())?;
        let output = Command::new(executable)
            .args(["--exact", test])
            .env(CHILD_ENV, &ran)
            .env(EXPECT_ENV, expect.name())
            .env("LC_ALL", locale.lc_all())
            .stdin(Stdio::null())
            .output()
            .map_err(|error| format!("{test}: cannot start its process: {error}"))?;
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let context = format!("LC_ALL={} expecting {expect:?} glyphs", locale.lc_all());
        if !output.status.success() {
            return Err(format!(
                "{test} failed in its own process ({context}): {}\n{text}",
                output.status
            ));
        }
        match std::fs::read_to_string(&ran) {
            Ok(name) if name == test => Ok(text),
            Ok(name) => Err(format!("{test}: its process ran `{name}` instead\n{text}")),
            Err(_) => Err(format!(
                "{test}: its process ({context}) exited without running the test body\n{text}"
            )),
        }
    }
}

/// The child that runs a TUI body must run exactly that body: a name that matches no test
/// fails the parent instead of passing on zero tests.
#[test]
fn a_tui_child_that_runs_no_test_fails_the_parent() {
    let error = own_locale_process::spawn("no_tui_test_has_this_name", Glyphs::Utf8, Glyphs::Utf8)
        .unwrap_err();
    assert!(
        error.contains("exited without running the test body"),
        "{error}"
    );
}

/// The synchronized control of Issue #968: the UTF-8 expectations, run in a child whose
/// locale is `C`, fail on the ASCII fallback the product chooses there. This is the Linux
/// Docker failure, reproduced on any host.
#[test]
fn a_c_locale_child_fails_the_utf8_glyph_expectations() {
    let error = own_locale_process::spawn(
        "primary_ultra_plan_run_renders_plan_then_activity_then_summary",
        Glyphs::Ascii,
        Glyphs::Utf8,
    )
    .unwrap_err();
    assert!(
        error.contains("failed in its own process (LC_ALL=C expecting Utf8 glyphs)"),
        "{error}"
    );
    assert!(error.contains("missing \"── Phase 1/2: p1 ──\""), "{error}");
    assert!(error.contains("-- Phase 1/2: p1 --"), "{error}");
}

#[test]
fn tui_integration_records_model_and_tool_boundaries() {
    own_locale_process::run(
        "tui_integration_records_model_and_tool_boundaries",
        Glyphs::Utf8,
        tui_integration_records_model_and_tool_boundaries_body,
    );
}

fn tui_integration_records_model_and_tool_boundaries_body() {
    let dir = tempfile::tempdir().unwrap();
    let mut client = FakeClient::new(
        "fake",
        vec![
            AssistantReply {
                content: String::new(),
                tool_calls: vec![ToolCall::new(
                    "Write",
                    json!({"path":"a.txt","content":"ok"}),
                )],
                prompt_tokens: Some(10),
                completion_tokens: Some(2),
            },
            AssistantReply::text("done"),
        ],
    );
    let ui = FakeUi::default();
    let mut session = SessionSnapshot::new();
    let result = run_session_with_required_paths_with_ui(
        &mut client,
        &mut session,
        "create a.txt",
        &["a.txt".to_string()],
        &config(dir.path().to_path_buf()),
        &ui,
    )
    .unwrap();
    assert_eq!(result, "done");
    let events = ui.events();
    assert!(events.iter().any(|event| event == "model:fake m"));
    assert!(events.iter().any(|event| event == "tool:Write"));
    assert!(events.iter().any(|event| event == "status:fake:m"));
}

#[test]
fn tui_markdown_raw_session_storage() {
    own_locale_process::run(
        "tui_markdown_raw_session_storage",
        Glyphs::Utf8,
        tui_markdown_raw_session_storage_body,
    );
}

fn tui_markdown_raw_session_storage_body() {
    let dir = tempfile::tempdir().unwrap();
    let raw = "<think>secret</think># done";
    let mut client = FakeClient::new("fake", vec![AssistantReply::text(raw)]);
    let ui = FakeUi::default();
    let mut session = SessionSnapshot::new();
    let reply = run_session_with_required_paths_with_ui(
        &mut client,
        &mut session,
        "answer",
        &[],
        &config(dir.path().to_path_buf()),
        &ui,
    )
    .unwrap();
    assert_eq!(reply, raw);
    assert_eq!(session.messages[1].content, raw);
    let rendered = TerminalMarkdownRenderer::new(false, true).render_to_string(raw);
    assert_eq!(rendered, "done");
    assert!(!rendered.contains("secret"));
}

#[test]
fn tui_streamed_markdown_matches_batch_and_hides_cross_chunk_think() {
    own_locale_process::run(
        "tui_streamed_markdown_matches_batch_and_hides_cross_chunk_think",
        Glyphs::Utf8,
        tui_streamed_markdown_matches_batch_and_hides_cross_chunk_think_body,
    );
}

fn tui_streamed_markdown_matches_batch_and_hides_cross_chunk_think_body() {
    let raw = concat!(
        "<think>private reasoning</think># Result\n\n",
        "| Item | Count |\n| --- | ---: |\n| 日本 | 2 |\n\n",
        "- parent\n  - **child**\n"
    );
    let chunks = [
        "<thi",
        "nk>private rea",
        "soning</think># Result\n\n| Item |",
        " Count |\n| --- | ---: |\n| 日",
        "本 | 2 |\n\n- parent\n  - **child**\n",
    ];
    let renderer = TerminalMarkdownRenderer::new(false, true);
    let streamed = renderer.render_chunks_to_string(chunks);
    let batch = renderer.render_to_string(raw);
    assert_eq!(streamed, batch);
    assert!(!streamed.contains("private reasoning"));
}

#[test]
fn interrupt_boundaries_stop_before_model_call() {
    own_locale_process::run(
        "interrupt_boundaries_stop_before_model_call",
        Glyphs::Utf8,
        interrupt_boundaries_stop_before_model_call_body,
    );
}

fn interrupt_boundaries_stop_before_model_call_body() {
    let dir = tempfile::tempdir().unwrap();
    let mut client = FakeClient::new("fake", vec![AssistantReply::text("unused")]);
    let mut session = SessionSnapshot::new();
    let ui = FakeUi::default();
    ui.interrupted.store(true, Ordering::SeqCst);
    let err = run_session_with_required_paths_with_ui(
        &mut client,
        &mut session,
        "answer",
        &[],
        &config(dir.path().to_path_buf()),
        &ui,
    )
    .unwrap_err()
    .to_string();
    assert!(err.contains("interrupted by user"));
}

#[test]
fn planner_uses_ui_for_planner_model_call() {
    own_locale_process::run(
        "planner_uses_ui_for_planner_model_call",
        Glyphs::Utf8,
        planner_uses_ui_for_planner_model_call_body,
    );
}

fn planner_uses_ui_for_planner_model_call_body() {
    let dir = tempfile::tempdir().unwrap();
    let json = r#"{"goal":"test","steps":[{"id":"s1","kind":"report","instruction":"say done","expected_paths":[],"verify":[],"expected_result":"pass"}]}"#;
    let mut planner = FakeClient::new("planner", vec![AssistantReply::text(json)]);
    let ui = FakeUi::default();
    let plan =
        generate_step_plan_with_ui(&mut planner, "test", &config(dir.path().to_path_buf()), &ui)
            .unwrap();
    assert_eq!(plan.steps.len(), 1);
    assert!(
        ui.events()
            .iter()
            .any(|event| event == "model:planner planner pm")
    );
}

#[test]
fn primary_ultra_plan_run_renders_plan_then_activity_then_summary() {
    own_locale_process::run(
        "primary_ultra_plan_run_renders_plan_then_activity_then_summary",
        Glyphs::Utf8,
        primary_ultra_plan_run_renders_plan_then_activity_then_summary_body,
    );
}

fn primary_ultra_plan_run_renders_plan_then_activity_then_summary_body() {
    let dir = tempfile::tempdir().unwrap();
    let events_path = dir.path().join(".anvil/runs/test/events.jsonl");
    let mut cfg = config(dir.path().to_path_buf());
    cfg.eval_events_path = Some(events_path);
    let plan_text = commandagent::planner::ultra_plan::render_ultra_plan(&two_phase_ultra_plan());
    let step_json = implement_step_plan_json();
    let mut planner = FakeClient::new(
        "planner",
        vec![
            AssistantReply::text(plan_text),
            AssistantReply::text(step_json.clone()),
            AssistantReply::text(step_json),
        ],
    );
    let mut execution = FakeClient::new(
        "exec",
        vec![
            AssistantReply {
                content: String::new(),
                tool_calls: vec![ToolCall::new(
                    "Write",
                    json!({"path":"app.jsx","content":interactive_app_source("phase1")}),
                )],
                prompt_tokens: None,
                completion_tokens: None,
            },
            AssistantReply {
                content: String::new(),
                tool_calls: vec![ToolCall::new(
                    "Write",
                    json!({"path":"app.jsx","content":interactive_app_source("phase2")}),
                )],
                prompt_tokens: None,
                completion_tokens: None,
            },
        ],
    );
    let ui = FakeUi::default();
    let _presentation = commandagent::tui::presentation::install(&cfg);
    let capture = commandagent::tui::markdown::capture::start();

    let output = commandagent::tui::slash::handle_command(
        "/ultra-plan-run build app",
        &cfg,
        &mut planner,
        &mut execution,
        &ui,
    )
    .unwrap();
    let rendered = format!("{}\n{output}", capture.output());

    let needles = [
        "### Plan",
        "── Phase 1/2: p1 ──",
        "#### Phase: p1",
        "→ Write app.jsx",
        "✓ Write ok",
        "### Terminal summary",
    ]
    .map(own_locale_process::expected);
    assert_in_order(&rendered, &needles.each_ref().map(String::as_str));
}

/// The ASCII control of the test above: the same body and the same expectations in the
/// same order, as `tui::glyphs::for_locale(.., false)` renders them under `LC_ALL=C`.
#[test]
fn primary_ultra_plan_run_renders_plan_then_activity_then_summary_ascii() {
    own_locale_process::run(
        "primary_ultra_plan_run_renders_plan_then_activity_then_summary_ascii",
        Glyphs::Ascii,
        primary_ultra_plan_run_renders_plan_then_activity_then_summary_body,
    );
}

#[test]
fn in_flight_provider_interrupt_finishes_before_sleep_and_writes_terminal_records() {
    own_locale_process::run(
        "in_flight_provider_interrupt_finishes_before_sleep_and_writes_terminal_records",
        Glyphs::Utf8,
        in_flight_provider_interrupt_finishes_before_sleep_and_writes_terminal_records_body,
    );
}

fn in_flight_provider_interrupt_finishes_before_sleep_and_writes_terminal_records_body() {
    let dir = test_tempdir();
    let events_path = dir.path().join(".anvil/runs/test/events.jsonl");
    let plan_path = write_ultra_plan(dir.path());
    let mut cfg = config(dir.path().to_path_buf());
    cfg.eval_events_path = Some(events_path.clone());
    cfg.chat_timeout_secs = 30;
    let mut planner = StartReleaseClient::new("planner");
    let provider_started = planner.started_gate();
    let release_worker = planner.release_gate();
    let mut execution = FakeClient::new("exec", Vec::new());
    let ui = ProviderStartInterruptUi::new(provider_started);

    let started = Instant::now();
    let err = commandagent::tui::slash::handle_command(
        &format!("/run-ultra-plan {plan_path}"),
        &cfg,
        &mut planner,
        &mut execution,
        &ui,
    )
    .unwrap_err()
    .to_string();
    // The caller has returned; only now may the in-flight provider worker finish.
    release_worker.raise();

    assert!(err.contains("interrupted by user"), "{err}");
    assert!(started.elapsed() < Duration::from_secs(5));
    assert!(
        ui.events()
            .iter()
            .any(|event| event.starts_with("model:planner")),
        "the interrupt must be armed after the provider turn started: {:?}",
        ui.events()
    );
    assert_eq!(planner.calls(), 1, "provider abort must not retry");
    let events = std::fs::read_to_string(&events_path).unwrap();
    assert!(events.contains("\"event\":\"provider_turn_aborted_by_user\""));
    assert!(events.contains("\"classification\":\"aborted_by_user\""));
    assert!(!events.contains("\"event\":\"provider_turn_timeout\""));
    assert_exactly_one_tui_stop(&events, "interrupted");
    assert!(events.contains("\"failure_kind\":\"tui_command_interrupted\""));
    assert_recovery_artifacts_exist(dir.path(), &events);
    let summary =
        std::fs::read_to_string(events_path.parent().unwrap().join("summary.md")).unwrap();
    assert_terminal_summary(&summary, "interrupted");
    assert!(summary.contains("Command status: interrupted"));
}

#[test]
fn in_flight_bash_interrupt_force_finalizes_without_waiting_for_grace() {
    own_locale_process::run(
        "in_flight_bash_interrupt_force_finalizes_without_waiting_for_grace",
        Glyphs::Utf8,
        in_flight_bash_interrupt_force_finalizes_without_waiting_for_grace_body,
    );
}

fn in_flight_bash_interrupt_force_finalizes_without_waiting_for_grace_body() {
    let dir = test_tempdir();
    let events_path = dir.path().join(".anvil/runs/test/events.jsonl");
    let plan_path = write_ultra_plan(dir.path());
    let mut cfg = config(dir.path().to_path_buf());
    cfg.eval_events_path = Some(events_path.clone());
    let step_json = implement_step_plan_json();
    let mut planner = FakeClient::new("planner", vec![AssistantReply::text(step_json)]);
    let mut execution = FakeClient::new(
        "exec",
        vec![AssistantReply {
            content: String::new(),
            tool_calls: vec![ToolCall::new(
                "Bash",
                json!({"command": shell_interrupt_command()}),
            )],
            prompt_tokens: None,
            completion_tokens: None,
        }],
    );
    let ui = ShellReadyInterruptUi::new(
        shell_ready_marker(dir.path()),
        Duration::ZERO,
        Duration::from_millis(100),
    );

    let started = Instant::now();
    let err = commandagent::tui::slash::handle_command(
        &format!("/run-ultra-plan {plan_path}"),
        &cfg,
        &mut planner,
        &mut execution,
        &ui,
    )
    .unwrap_err()
    .to_string();
    let finished = Instant::now();

    let ready_at = ui
        .ready_at()
        .expect("the shell must announce ready before the interrupt is armed");
    assert!(
        ui.interrupt_saw_ready(),
        "the first interrupt must be gated by the shell ready marker"
    );
    assert!(
        finished.duration_since(ready_at) < Duration::from_secs(2),
        "force must finalize without waiting the 5s user-interrupt grace"
    );
    assert!(started.elapsed() < Duration::from_secs(5));
    assert!(
        ui.events().iter().any(|event| event == "tool:Bash"),
        "{:?}",
        ui.events()
    );
    assert!(err.contains("command_aborted_by_user"), "{err}");
    assert!(err.contains("interrupted by user"), "{err}");
    assert!(
        shell_child_gone(),
        "force must leave no shell child behind: {}",
        shell_child_marker()
    );
    let events = std::fs::read_to_string(&events_path).unwrap();
    assert!(events.contains("\"error_kind\":\"command_aborted_by_user\""));
    assert_exactly_one_tui_stop(&events, "interrupted");
    assert!(events.contains("\"failure_kind\":\"tui_command_interrupted\""));
    let summary =
        std::fs::read_to_string(events_path.parent().unwrap().join("summary.md")).unwrap();
    assert_terminal_summary(&summary, "interrupted");
    assert!(summary.contains("Command status: interrupted"));
}

#[test]
fn tui_ultra_plan_run_smoke_fake_clients() {
    own_locale_process::run(
        "tui_ultra_plan_run_smoke_fake_clients",
        Glyphs::Utf8,
        tui_ultra_plan_run_smoke_fake_clients_body,
    );
}

fn tui_ultra_plan_run_smoke_fake_clients_body() {
    let dir = tempfile::tempdir().unwrap();
    let mut step_plan = commandagent::planner::step_plan::StepPlan::single("write app");
    step_plan.steps[0].kind = "implement".to_string();
    step_plan.steps[0]
        .expected_paths
        .push("app.jsx".to_string());
    let step_json = serde_json::to_string(&step_plan).unwrap();
    let plan = commandagent::planner::ultra_plan::UltraPlan {
        goal: "build app".to_string(),
        profile: "generic".to_string(),
        style: "default".to_string(),
        intent: "create".to_string(),
        phases: vec![
            commandagent::planner::ultra_plan::UltraPhase {
                id: "p1".to_string(),
                prompt: "phase 1".to_string(),
            },
            commandagent::planner::ultra_plan::UltraPhase {
                id: "p2".to_string(),
                prompt: "phase 2".to_string(),
            },
        ],
    };
    let mut planner = FakeClient::new(
        "planner",
        vec![
            AssistantReply::text(step_json.clone()),
            AssistantReply::text(step_json),
        ],
    );
    let mut execution = FakeClient::new(
        "exec",
        vec![
            AssistantReply {
                content: String::new(),
                tool_calls: vec![ToolCall::new(
                    "Write",
                    json!({"path":"app.jsx","content":interactive_app_source("phase1")}),
                )],
                prompt_tokens: None,
                completion_tokens: None,
            },
            AssistantReply {
                content: String::new(),
                tool_calls: vec![ToolCall::new(
                    "Write",
                    json!({"path":"app.jsx","content":interactive_app_source("phase2")}),
                )],
                prompt_tokens: None,
                completion_tokens: None,
            },
        ],
    );
    let ui = FakeUi::default();
    let result = run_ultra_plan_with_ui(
        &mut planner,
        &mut execution,
        &plan,
        &config(dir.path().to_path_buf()),
        &ui,
    )
    .unwrap();
    assert_eq!(result, "ultra-plan-run complete: 2 phases");
    let planner_requests = planner.requests();
    let execution_requests = execution.requests();
    assert_eq!(planner_requests.requests.len(), 2);
    assert_eq!(
        planner_requests.requests[0].len(),
        planner_requests.requests[1].len()
    );
    assert_eq!(execution_requests.requests.len(), 2);
    assert!(
        execution_requests.requests[1].len() > execution_requests.requests[0].len(),
        "phase 2 execution should reuse the ultra execution session"
    );
    let second_request = format!("{:?}", execution_requests.requests[1]);
    assert!(second_request.contains("phase1"));
}

#[test]
fn tui_runs_lists_recent_runs_without_emitting_command_events() {
    own_locale_process::run(
        "tui_runs_lists_recent_runs_without_emitting_command_events",
        Glyphs::Utf8,
        tui_runs_lists_recent_runs_without_emitting_command_events_body,
    );
}

fn tui_runs_lists_recent_runs_without_emitting_command_events_body() {
    let dir = tempfile::tempdir().unwrap();
    let events_path = dir.path().join(".anvil/runs/current/events.jsonl");
    let previous_run = dir.path().join(".anvil/runs/018f2222-bbbb");
    std::fs::create_dir_all(&previous_run).unwrap();
    std::fs::write(
        previous_run.join("events.jsonl"),
        serde_json::json!({
            "event": "tui_command_stop",
            "ok": false,
            "status": "failed",
            "task_status": "failed",
            "assurance_level": "full",
            "runtime_acceptance_status": "pass",
            "final_acceptance_status": "failed",
            "release_gate_status": "failed",
            "stop_reason": "failed because recovery is available",
            "recovery_ultra_plan_path": ".anvil/plans/recovery-ultra-plan-test.yaml"
        })
        .to_string()
            + "\n",
    )
    .unwrap();
    std::fs::create_dir_all(dir.path().join(".anvil/plans")).unwrap();
    std::fs::write(
        dir.path()
            .join(".anvil/plans/recovery-ultra-plan-test.yaml"),
        "goal: \"g\"\nphases:\n  - id: \"p\"\n    prompt: \"p\"\n",
    )
    .unwrap();
    let mut cfg = config(dir.path().to_path_buf());
    cfg.eval_events_path = Some(events_path.clone());
    let mut planner = FakeClient::new("planner", Vec::new());
    let mut execution = FakeClient::new("exec", Vec::new());
    let ui = FakeUi::default();

    let output =
        commandagent::tui::slash::handle_command("/runs", &cfg, &mut planner, &mut execution, &ui)
            .unwrap();

    assert!(output.contains("018f2222"), "{output}");
    assert!(output.contains("failed/partial"), "{output}");
    assert!(output.contains("yaml"), "{output}");
    assert!(
        !events_path.exists(),
        "/runs should not emit command events"
    );
}

#[test]
fn tui_help_lists_recovery_commands_without_emitting_events() {
    own_locale_process::run(
        "tui_help_lists_recovery_commands_without_emitting_events",
        Glyphs::Utf8,
        tui_help_lists_recovery_commands_without_emitting_events_body,
    );
}

fn tui_help_lists_recovery_commands_without_emitting_events_body() {
    let dir = tempfile::tempdir().unwrap();
    let events_path = dir.path().join(".anvil/runs/current/events.jsonl");
    let mut cfg = config(dir.path().to_path_buf());
    cfg.eval_events_path = Some(events_path.clone());
    let mut planner = FakeClient::new("planner", Vec::new());
    let mut execution = FakeClient::new("exec", Vec::new());
    let ui = FakeUi::default();

    let output =
        commandagent::tui::slash::handle_command("/help", &cfg, &mut planner, &mut execution, &ui)
            .unwrap();

    assert!(
        output.contains("/confirm <hash> - confirm and execute the reviewed Gate 1 card"),
        "{output}"
    );
    assert!(output.contains("/runs - list recent runs"), "{output}");
    assert!(
        output.contains("/resume [run-id|yaml-path] - resume from a recovery UltraPlan"),
        "{output}"
    );
    assert!(
        output.contains("/plan - show the active plan and current activity"),
        "{output}"
    );
    assert!(output.contains("/exit or /quit"), "{output}");
    assert!(
        !events_path.exists(),
        "/help should not emit command events"
    );
}

#[test]
fn tui_resume_runs_recovery_plan_remaining_phases_and_records_lineage() {
    own_locale_process::run(
        "tui_resume_runs_recovery_plan_remaining_phases_and_records_lineage",
        Glyphs::Utf8,
        tui_resume_runs_recovery_plan_remaining_phases_and_records_lineage_body,
    );
}

/// Issue #1048 control: `/plan` renders the activity and progress glyphs of the other TUI
/// displays, so the same body under `LC_ALL=C` expects them through
/// `tui::glyphs::for_locale(.., false)`.
#[test]
fn tui_resume_runs_recovery_plan_remaining_phases_and_records_lineage_ascii() {
    own_locale_process::run(
        "tui_resume_runs_recovery_plan_remaining_phases_and_records_lineage_ascii",
        Glyphs::Ascii,
        tui_resume_runs_recovery_plan_remaining_phases_and_records_lineage_body,
    );
}

fn tui_resume_runs_recovery_plan_remaining_phases_and_records_lineage_body() {
    let dir = tempfile::tempdir().unwrap();
    let events_path = dir.path().join(".anvil/runs/current/events.jsonl");
    std::fs::create_dir_all(dir.path().join(".anvil/plans")).unwrap();
    std::fs::write(dir.path().join("setup.txt"), "done").unwrap();
    std::fs::write(
        dir.path()
            .join(".anvil/plans/recovery-ultra-plan-test.yaml"),
        r#"
recovery_schema_version: "1"
recovery_original_goal: "build app"
recovery_failure_kind: "interrupted"
recovery_profile: "generic"
recovery_expected_completed_artifacts:
  - "setup.txt"
goal: "build app"
profile: "generic"
style: "recovery"
intent: "recover"
phases:
  - id: "repair-phase"
    prompt: "repair the remaining implementation"
  - id: "verify-recovery"
    prompt: "verify the recovered implementation"
"#
        .trim_start(),
    )
    .unwrap();
    let previous_run = dir.path().join(".anvil/runs/018f6666-resumable");
    std::fs::create_dir_all(&previous_run).unwrap();
    std::fs::write(
        previous_run.join("events.jsonl"),
        format!(
            "{}\n{}\n",
            json!({
                "event": "ultra_partial_artifact_summary",
                "completed_phase_ids": ["scaffold"],
                "failed_phase_id": "repair-phase",
                "pending_phase_ids": ["verify"],
                "recovery_ultra_plan_path": ".anvil/plans/recovery-ultra-plan-test.yaml"
            }),
            json!({
                "event": "tui_command_stop",
                "ok": false,
                "status": "interrupted",
                "assurance_level": "partial",
                "failure_kind": "tui_command_interrupted",
                "effective_profile": "generic",
                "requested_port": "",
                "recovery_ultra_plan_path": ".anvil/plans/recovery-ultra-plan-test.yaml"
            })
        ),
    )
    .unwrap();
    let mut cfg = config(dir.path().to_path_buf());
    cfg.eval_events_path = Some(events_path.clone());
    let step_json = implement_step_plan_json();
    let mut planner = FakeClient::new(
        "planner",
        vec![
            AssistantReply::text(step_json.clone()),
            AssistantReply::text(step_json),
        ],
    );
    let mut execution = FakeClient::new(
        "exec",
        vec![
            AssistantReply {
                content: String::new(),
                tool_calls: vec![ToolCall::new(
                    "Write",
                    json!({"path":"app.jsx","content":interactive_app_source("repaired")}),
                )],
                prompt_tokens: None,
                completion_tokens: None,
            },
            AssistantReply {
                content: String::new(),
                tool_calls: vec![ToolCall::new(
                    "Write",
                    json!({"path":"app.jsx","content":interactive_app_source("verified")}),
                )],
                prompt_tokens: None,
                completion_tokens: None,
            },
        ],
    );
    let ui = FakeUi::default();

    let output = commandagent::tui::slash::handle_command(
        "/resume 018f6666",
        &cfg,
        &mut planner,
        &mut execution,
        &ui,
    )
    .unwrap();

    assert!(output.contains("### Resume recovery run"), "{output}");
    assert!(
        output.contains("completed phases skipped: scaffold"),
        "{output}"
    );
    assert!(output.contains("- Resumed from: 018f6666"), "{output}");
    assert!(
        output.contains("phases to run: repair-phase, verify-recovery"),
        "{output}"
    );
    assert!(
        output.contains("ultra-plan-run complete: 2 phases"),
        "{output}"
    );
    assert_eq!(planner.requests().requests.len(), 2);
    assert_eq!(execution.requests().requests.len(), 2);
    let events = std::fs::read_to_string(&events_path).unwrap();
    assert!(events.contains("\"event\":\"resume_start\""), "{events}");
    assert!(events.contains("\"resumed_from\":\"018f6666\""), "{events}");
    let phase_starts = events
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|event| {
            event.get("event").and_then(|value| value.as_str()) == Some("ultra_phase_start")
        })
        .map(|event| {
            event
                .get("phase_id")
                .and_then(|value| value.as_str())
                .unwrap()
                .to_string()
        })
        .collect::<Vec<_>>();
    assert_eq!(phase_starts, vec!["repair-phase", "verify-recovery"]);
    let stop = tui_command_stop_events(&events).pop().unwrap();
    assert_eq!(
        stop.get("resumed_from").and_then(|value| value.as_str()),
        Some("018f6666")
    );

    let plan_output =
        commandagent::tui::slash::handle_command("/plan", &cfg, &mut planner, &mut execution, &ui)
            .unwrap();
    assert!(plan_output.contains("### Plan"), "{plan_output}");
    assert!(plan_output.contains("repair-phase"), "{plan_output}");
    assert!(plan_output.contains("verify-recovery"), "{plan_output}");
    assert!(
        plan_output.contains(&own_locale_process::expected("1. ✓ repair-phase")),
        "{plan_output}"
    );
    assert!(
        plan_output.contains(&own_locale_process::expected("2. ✓ verify-recovery")),
        "{plan_output}"
    );
    assert!(
        plan_output.contains(&own_locale_process::expected(
            "Current activity: ✓ Write ok"
        )),
        "{plan_output}"
    );
}

#[test]
fn tui_input_errors_do_not_start_commands_or_generate_summaries() {
    own_locale_process::run(
        "tui_input_errors_do_not_start_commands_or_generate_summaries",
        Glyphs::Utf8,
        tui_input_errors_do_not_start_commands_or_generate_summaries_body,
    );
}

fn tui_input_errors_do_not_start_commands_or_generate_summaries_body() {
    let dir = tempfile::tempdir().unwrap();
    let events_path = dir.path().join(".anvil/runs/test/events.jsonl");
    let mut cfg = config(dir.path().to_path_buf());
    cfg.eval_events_path = Some(events_path.clone());
    let mut planner = FailingClient::new("planner", "must not be called");
    let mut execution = FailingClient::new("execution", "must not be called");
    let ui = FakeUi::default();

    let typo =
        commandagent::tui::slash::handle_command("/hepl", &cfg, &mut planner, &mut execution, &ui)
            .unwrap();
    assert_eq!(typo.lines().count(), 2, "{typo}");
    assert!(typo.contains("Did you mean /help?"), "{typo}");
    assert!(!typo.contains("TASK FAILED"), "{typo}");
    assert!(!typo.contains("Terminal summary"), "{typo}");
    assert!(!typo.contains("error:"), "{typo}");

    let plain = commandagent::tui::slash::handle_command(
        "日本語の自由文\u{1b}[31m\u{202e}",
        &cfg,
        &mut planner,
        &mut execution,
        &ui,
    )
    .unwrap();
    assert_eq!(plain.lines().count(), 2, "{plain:?}");
    assert!(plain.contains("日本語の自由文?[31m?"), "{plain:?}");
    assert!(plain.contains("/ultra-plan-run <goal>"), "{plain}");
    assert!(plain.contains("/plan-run <goal>"), "{plain}");
    assert_eq!(planner.calls(), 0);
    assert_eq!(execution.calls(), 0);
    assert!(!events_path.exists(), "input errors must not create events");
    assert!(
        !events_path.parent().unwrap().join("summary.md").exists(),
        "input errors must not create a summary"
    );
}

#[test]
fn tui_provider_failure_records_run_events_and_renders_one_failure_block() {
    own_locale_process::run(
        "tui_provider_failure_records_run_events_and_renders_one_failure_block",
        Glyphs::Utf8,
        tui_provider_failure_records_run_events_and_renders_one_failure_block_body,
    );
}

fn tui_provider_failure_records_run_events_and_renders_one_failure_block_body() {
    let dir = tempfile::tempdir().unwrap();
    let events_path = dir.path().join(".anvil/runs/test/events.jsonl");
    let mut cfg = config(dir.path().to_path_buf());
    cfg.eval_events_path = Some(events_path.clone());
    let mut planner = FailingClient::new("planner", "provider connection unavailable");
    let mut execution = FakeClient::new("exec", Vec::new());
    let ui = FakeUi::default();
    let err = commandagent::tui::slash::handle_command(
        "/plan-steps test",
        &cfg,
        &mut planner,
        &mut execution,
        &ui,
    )
    .unwrap_err()
    .to_string();
    assert!(err.contains("provider connection unavailable"), "{err}");
    assert_eq!(err.matches("TASK FAILED").count(), 1, "{err}");
    assert!(!err.contains("Terminal summary"), "{err}");
    assert!(!err.contains("error:"), "{err}");
    let events = std::fs::read_to_string(&events_path).unwrap();
    assert!(events.contains("\"event\":\"tui_command_start\""));
    assert_exactly_one_tui_stop(&events, "failed");
    assert!(events.contains("\"event\":\"loop_stop\""));
    assert!(events.contains("\"failure_kind\":\"tui_command_failed\""));
    assert!(events.contains("\"lifecycle_stage\":\"tui_command\""));
    assert!(events.contains("\"task_status\":\"failed\""));
    assert!(events.contains("\"session_status\":\"repl_ready\""));
    assert!(events.contains("\"repl_status\":\"ready\""));
    assert!(events.contains("\"recovery_next_action\":\"fix_command_failure\""));
    let summary =
        std::fs::read_to_string(events_path.parent().unwrap().join("summary.md")).unwrap();
    assert_terminal_summary(&summary, "failed");
    assert!(summary.contains("Completion status: incomplete"));
    assert!(summary.contains("Command status: failed"));
    assert!(summary.contains("Task status: failed"));
    assert!(summary.contains("Process: REPL exited cleanly (not task status)"));
    assert!(summary.contains("Session/REPL status: repl_ready"));
    assert!(summary.contains("Recovery next action: fix_command_failure"));
    assert!(summary.contains("TUI command failed"));
    assert!(!summary.contains("\nStatus: complete\n"));
}

#[test]
fn tui_slash_failure_rewrites_existing_partial_summary_with_phase_breakdown() {
    own_locale_process::run(
        "tui_slash_failure_rewrites_existing_partial_summary_with_phase_breakdown",
        Glyphs::Utf8,
        tui_slash_failure_rewrites_existing_partial_summary_with_phase_breakdown_body,
    );
}

fn tui_slash_failure_rewrites_existing_partial_summary_with_phase_breakdown_body() {
    let dir = tempfile::tempdir().unwrap();
    let events_path = dir.path().join(".anvil/runs/test/events.jsonl");
    let mut cfg = config(dir.path().to_path_buf());
    cfg.eval_events_path = Some(events_path.clone());
    commandagent::eval_events::write_run_summary(
        cfg.eval_events_path.as_deref(),
        "Status: incomplete\nCompleted phases:\n- scaffold\nFailed phase:\n- final\nPending phases:\n- none\nRecovery next action:\n- /run-ultra-plan .anvil/plans/recovery-ultra-plan-final.yaml",
    );
    let mut planner = FailingClient::new("planner", "provider connection unavailable");
    let mut execution = FakeClient::new("exec", Vec::new());
    let ui = FakeUi::default();
    let err = commandagent::tui::slash::handle_command(
        "/plan-steps test",
        &cfg,
        &mut planner,
        &mut execution,
        &ui,
    )
    .unwrap_err()
    .to_string();
    assert!(err.contains("provider connection unavailable"), "{err}");
    let summary =
        std::fs::read_to_string(events_path.parent().unwrap().join("summary.md")).unwrap();
    assert_terminal_summary(&summary, "failed");
    assert!(summary.contains("Completed phases:\n- scaffold (completed)"));
    assert!(summary.contains("Failed phases:\n- final (failed)"));
    assert!(summary.contains("Pending phases:\n- none"));
    assert!(summary.contains("Recovery next action:"));
    assert!(summary.contains("TUI command failed"));
}

#[test]
fn tui_slash_success_with_partial_release_gate_is_not_complete_only() {
    own_locale_process::run(
        "tui_slash_success_with_partial_release_gate_is_not_complete_only",
        Glyphs::Utf8,
        tui_slash_success_with_partial_release_gate_is_not_complete_only_body,
    );
}

fn tui_slash_success_with_partial_release_gate_is_not_complete_only_body() {
    let dir = tempfile::tempdir().unwrap();
    let events_path = dir.path().join(".anvil/runs/test/events.jsonl");
    let mut cfg = config(dir.path().to_path_buf());
    cfg.eval_events_path = Some(events_path.clone());
    commandagent::eval_events::emit(
        cfg.eval_events_path.as_deref(),
        json!({
            "event": "ultra_final_acceptance",
            "runtime_acceptance_passed": true,
            "runtime_acceptance_status": "pass",
            "final_acceptance_status": "partial",
            "release_gate_status": "partial",
            "release_gate_reasons": ["browser_readiness_or_interaction_evidence_required:browser_readiness_evidence_missing"],
            "browser_readiness_status": "unavailable:browser_readiness_evidence_missing",
            "interaction_evidence_status": "unavailable:interaction_evidence_missing",
            "recovery_prompt_path": ".anvil/repairs/repair-release.yaml.md",
            "recovery_ultra_plan_path": ".anvil/plans/recovery-ultra-plan-release.yaml",
            "suggested_recovery_command": "/ultra-plan-run --profile nextjs \"$(cat .anvil/repairs/repair-release.yaml.md)\"",
            "suggested_recovery_yaml_command": "/run-ultra-plan .anvil/plans/recovery-ultra-plan-release.yaml",
        }),
    );
    let plan_json = r#"{"goal":"test","steps":[{"id":"s1","kind":"report","instruction":"say done","expected_paths":[],"verify":[],"expected_result":"pass"}]}"#;
    let mut planner = FakeClient::new("planner", vec![AssistantReply::text(plan_json)]);
    let mut execution = FakeClient::new("exec", Vec::new());
    let ui = FakeUi::default();
    let output = commandagent::tui::slash::handle_command(
        "/plan-steps test",
        &cfg,
        &mut planner,
        &mut execution,
        &ui,
    )
    .unwrap();
    assert!(output.contains("Command completion: completed"));
    assert!(output.contains("Task status: partial"));
    assert!(output.contains("Runtime acceptance: pass"));
    assert!(output.contains("Final acceptance: partial"));
    assert!(output.contains("Release gate: partial"));
    assert!(
        output
            .contains("Next action: collect_missing_release_evidence_or_continue_release_recovery")
    );
    assert!(output.contains("Recovery UltraPlan: .anvil/plans/recovery-ultra-plan-release.yaml"));
    assert!(output.contains(
        "Suggested recovery command: /run-ultra-plan .anvil/plans/recovery-ultra-plan-release.yaml"
    ));
    let events = std::fs::read_to_string(&events_path).unwrap();
    assert!(events.contains("\"event\":\"tui_command_stop\""));
    assert_exactly_one_tui_stop(&events, "completed");
    assert!(events.contains("\"completion_status\":\"complete_with_partial_release_gate\""));
    assert!(events.contains("\"task_status\":\"partial\""));
    assert!(events.contains("\"session_status\":\"repl_ready\""));
    assert!(events.contains("\"release_gate_status\":\"partial\""));
    let summary =
        std::fs::read_to_string(events_path.parent().unwrap().join("summary.md")).unwrap();
    assert_terminal_summary(&summary, "completed");
    assert!(summary.contains("Completion status: complete_with_partial_release_gate"));
    assert!(summary.contains("Session/REPL status: repl_ready"));
    assert!(summary.contains("Command status: completed"));
    assert!(summary.contains("Command completion: completed"));
    assert!(summary.contains("Task status: partial"));
    assert!(summary.contains("Final acceptance: partial"));
    assert!(summary.contains("Release gate: partial"));
    assert!(summary.contains("Recovery handoff:"));
    assert!(summary.contains(
        "Suggested YAML command: /run-ultra-plan .anvil/plans/recovery-ultra-plan-release.yaml"
    ));
    assert!(!summary.contains("\nStatus: running\n"));
}

#[test]
fn tui_slash_completion_guard_records_aborted_on_panic() {
    own_locale_process::run(
        "tui_slash_completion_guard_records_aborted_on_panic",
        Glyphs::Utf8,
        tui_slash_completion_guard_records_aborted_on_panic_body,
    );
}

fn tui_slash_completion_guard_records_aborted_on_panic_body() {
    let dir = tempfile::tempdir().unwrap();
    let events_path = dir.path().join(".anvil/runs/test/events.jsonl");
    let mut cfg = config(dir.path().to_path_buf());
    cfg.eval_events_path = Some(events_path.clone());
    let mut planner = FakeClient::new("planner", Vec::new());
    let mut execution = FakeClient::new("exec", Vec::new());
    let ui = FakeUi::default();

    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = commandagent::tui::slash::handle_command(
            "/plan-steps panic",
            &cfg,
            &mut planner,
            &mut execution,
            &ui,
        );
    }));
    assert!(panic.is_err());

    let events = std::fs::read_to_string(&events_path).unwrap();
    assert!(events.contains("\"event\":\"tui_command_start\""));
    assert_exactly_one_tui_stop(&events, "aborted");
    assert!(events.contains("\"failure_kind\":\"tui_command_aborted\""));
    let summary =
        std::fs::read_to_string(events_path.parent().unwrap().join("summary.md")).unwrap();
    assert_terminal_summary(&summary, "aborted");
    assert!(summary.contains("Command status: aborted"));
    assert!(summary.contains("Failure kind: tui_command_aborted"));
    assert!(summary.contains("Completed phases:\n- none"));
    assert!(summary.contains("Failed phases:\n- none"));
    assert!(summary.contains("Pending phases:\n- none"));
}

#[test]
fn tui_slash_ultra_panic_records_diagnostics_and_terminal_summary() {
    own_locale_process::run(
        "tui_slash_ultra_panic_records_diagnostics_and_terminal_summary",
        Glyphs::Utf8,
        tui_slash_ultra_panic_records_diagnostics_and_terminal_summary_body,
    );
}

fn tui_slash_ultra_panic_records_diagnostics_and_terminal_summary_body() {
    let dir = tempfile::tempdir().unwrap();
    let events_path = dir.path().join(".anvil/runs/test/events.jsonl");
    let plan_path = write_ultra_plan(dir.path());
    let mut cfg = config(dir.path().to_path_buf());
    cfg.eval_events_path = Some(events_path.clone());
    let step_json = implement_step_plan_json();
    let mut planner = FakeClient::new("planner", vec![AssistantReply::text(step_json)]);
    let mut execution = PanicClient::new("exec", "simulated ultra panic 日本語");
    let ui = FakeUi::default();

    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = commandagent::tui::slash::handle_command(
            &format!("/run-ultra-plan {plan_path}"),
            &cfg,
            &mut planner,
            &mut execution,
            &ui,
        );
    }));
    assert!(panic.is_err());

    let events = std::fs::read_to_string(&events_path).unwrap();
    let panic_event = events
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .find(|event| {
            event
                .get("event")
                .and_then(|value| value.as_str())
                .is_some_and(|name| name == "panic_caught")
        })
        .unwrap_or_else(|| panic!("missing panic_caught in {events}"));
    assert_eq!(
        panic_event.get("message").and_then(|value| value.as_str()),
        Some("simulated ultra panic 日本語"),
        "{panic_event}"
    );
    assert!(
        panic_event
            .get("location")
            .and_then(|value| value.as_str())
            .is_some_and(|location| location.contains("tests/tui_integration.rs:")),
        "{panic_event}"
    );
    assert_exactly_one_tui_stop(&events, "aborted");
    assert!(events.contains("\"failure_kind\":\"tui_command_aborted\""));
    let summary =
        std::fs::read_to_string(events_path.parent().unwrap().join("summary.md")).unwrap();
    assert_terminal_summary(&summary, "aborted");
    assert!(summary.contains("Command status: aborted"));
    assert!(summary.contains("Failure kind: tui_command_aborted"));
}

#[test]
fn tui_slash_completion_guard_records_interrupted_mid_phase() {
    own_locale_process::run(
        "tui_slash_completion_guard_records_interrupted_mid_phase",
        Glyphs::Utf8,
        tui_slash_completion_guard_records_interrupted_mid_phase_body,
    );
}

fn tui_slash_completion_guard_records_interrupted_mid_phase_body() {
    let dir = tempfile::tempdir().unwrap();
    let events_path = dir.path().join(".anvil/runs/test/events.jsonl");
    let plan_path = write_ultra_plan(dir.path());
    let mut cfg = config(dir.path().to_path_buf());
    cfg.eval_events_path = Some(events_path.clone());
    let step_json = implement_step_plan_json();
    let mut planner = FakeClient::new("planner", vec![AssistantReply::text(step_json)]);
    let mut execution = FakeClient::new("exec", Vec::new());
    let ui = InterruptAfterUi::new(2);

    let err = commandagent::tui::slash::handle_command(
        &format!("/run-ultra-plan {plan_path}"),
        &cfg,
        &mut planner,
        &mut execution,
        &ui,
    )
    .unwrap_err()
    .to_string();
    assert!(err.contains("interrupted by user"), "{err}");
    assert_eq!(err.matches("INTERRUPTED").count(), 1, "{err}");
    assert!(!err.contains("TASK FAILED"), "{err}");
    assert!(err.contains("- Rerun: /run-ultra-plan"), "{err}");

    let events = std::fs::read_to_string(&events_path).unwrap();
    assert!(events.contains("\"event\":\"ultra_phase_start\""));
    assert!(events.contains("\"event\":\"ultra_phase_failed\""));
    assert_exactly_one_tui_stop(&events, "interrupted");
    assert!(events.contains("\"failure_kind\":\"tui_command_interrupted\""));
    let summary =
        std::fs::read_to_string(events_path.parent().unwrap().join("summary.md")).unwrap();
    assert_terminal_summary(&summary, "interrupted");
    assert!(summary.contains("Command status: interrupted"));
    assert!(summary.contains("Failed phases:\n- p1 (interrupted)"));
    assert!(summary.contains("Pending phases:\n- p2 (pending)"));
    assert_recovery_artifacts_exist(dir.path(), &events);
}

#[test]
fn tui_slash_ultra_plan_completion_records_phase_breakdown_and_acceptance() {
    own_locale_process::run(
        "tui_slash_ultra_plan_completion_records_phase_breakdown_and_acceptance",
        Glyphs::Utf8,
        tui_slash_ultra_plan_completion_records_phase_breakdown_and_acceptance_body,
    );
}

fn tui_slash_ultra_plan_completion_records_phase_breakdown_and_acceptance_body() {
    let dir = tempfile::tempdir().unwrap();
    let events_path = dir.path().join(".anvil/runs/test/events.jsonl");
    let plan_path = write_ultra_plan(dir.path());
    let mut cfg = config(dir.path().to_path_buf());
    cfg.eval_events_path = Some(events_path.clone());
    let step_json = implement_step_plan_json();
    let mut planner = FakeClient::new(
        "planner",
        vec![
            AssistantReply::text(step_json.clone()),
            AssistantReply::text(step_json),
        ],
    );
    let mut execution = FakeClient::new(
        "exec",
        vec![
            AssistantReply {
                content: String::new(),
                tool_calls: vec![ToolCall::new(
                    "Write",
                    json!({"path":"app.jsx","content":interactive_app_source("phase1")}),
                )],
                prompt_tokens: None,
                completion_tokens: None,
            },
            AssistantReply {
                content: String::new(),
                tool_calls: vec![ToolCall::new(
                    "Write",
                    json!({"path":"app.jsx","content":interactive_app_source("phase2")}),
                )],
                prompt_tokens: None,
                completion_tokens: None,
            },
        ],
    );
    let ui = FakeUi::default();

    let output = commandagent::tui::slash::handle_command(
        &format!("/run-ultra-plan {plan_path}"),
        &cfg,
        &mut planner,
        &mut execution,
        &ui,
    )
    .unwrap();
    assert!(output.contains("ultra-plan-run complete: 2 phases"));

    let events = std::fs::read_to_string(&events_path).unwrap();
    assert!(events.contains("\"event\":\"ultra_final_acceptance\""));
    assert_exactly_one_tui_stop(&events, "completed");
    let summary =
        std::fs::read_to_string(events_path.parent().unwrap().join("summary.md")).unwrap();
    assert_terminal_summary(&summary, "completed");
    assert!(summary.contains("Completed phases:\n- p1 (completed)\n- p2 (completed)"));
    assert!(summary.contains("Failed phases:\n- none"));
    assert!(summary.contains("Pending phases:\n- none"));
    assert!(summary.contains("Final acceptance: full_success"));
}

#[test]
fn plain_renderer_keeps_raw_output() {
    own_locale_process::run(
        "plain_renderer_keeps_raw_output",
        Glyphs::Utf8,
        plain_renderer_keeps_raw_output_body,
    );
}

fn plain_renderer_keeps_raw_output_body() {
    let renderer = commandagent::tui::markdown::PlainRenderer;
    renderer
        .render_assistant("<think>secret</think>raw")
        .expect("plain renderer writes");
}
