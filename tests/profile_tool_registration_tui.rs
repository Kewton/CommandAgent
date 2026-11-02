//! Issue #972 (G1): what happens before a profile tool refusal in the TUI and on
//! the plan-file entries, on a workflow child, and on the one read a selected pack
//! costs.
//!
//! This is its own test binary because the off control needs a process that has
//! never decoded the selected pack. Everything is synthetic: a scripted provider,
//! a fake loopback provider, and temporary directories. No model, key, or external
//! provider is used.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use commandagent::config::{
    Action, Config, ConfigFieldSources, IntentId, NarrationMode, OllamaThink, OpenAiApi,
    PlanPreset, PromptLayout, Provider,
};
use commandagent::minimal_loop::loop_run::run_session_with_outcome_with_ui;
use commandagent::minimal_loop::request_pattern::{
    Classification, ClassifyFailure, PatternClassifier, RequestPatternMode, RequestPatternSettings,
    run_direct_prompt_with, run_direct_prompt_with_common,
};
use commandagent::planner::pack::exact_byte_hash;
use commandagent::planner::runner::{
    generate_and_run_step_plan, generate_and_run_step_plan_with_ui, generate_and_run_ultra_plan,
    generate_and_run_ultra_plan_with_ui, generate_ultra_plan, generate_ultra_plan_with_ui,
    run_step_plan, run_step_plan_with_ui, run_ultra_plan, run_ultra_plan_file,
    run_ultra_plan_file_with_ui, run_ultra_plan_with_ui,
};
use commandagent::planner::step_plan::{PlanStep, StepPlan, render_step_plan};
use commandagent::planner::ultra_plan::{UltraPhase, UltraPlan, render_ultra_plan};
use commandagent::providers::{AssistantReply, ChatClient};
use commandagent::state::{ConversationMessage, SessionSnapshot, ToolCall};
use commandagent::tui::NOOP_UI;
use commandagent::tui::boundary_shell::BoundaryShell;
use commandagent::tui::boundary_shell::ambiguity::{
    ClassifierProvenance, ProposalStatus, RouteProposal,
};
use commandagent::tui::boundary_shell::confirmation::{ExecutionPins, PackSelection};
use commandagent::tui::boundary_shell::directive::DirectiveContinuation;
use commandagent::tui::boundary_shell::route::{
    DeterministicResolution, ExplicitRouteBinding, RouteRequest, deterministic_route,
};
use commandagent::tui::slash::handle_command;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const OFF_GOLDEN: &str =
    include_str!("corpus/apps/profile-tool-registration/fixtures/off-golden.json");
const REGISTRY_ENV: &str = "COMMANDAGENT_ADAPTER_CONFIG_ROOT";
const PACK_ENVS: [&str; 4] = [
    "COMMANDAGENT_PACK_DIRECTORY",
    "COMMANDAGENT_PACK_ID",
    "COMMANDAGENT_PACK_VERSION",
    "COMMANDAGENT_PACK_HASH",
];
const REFUSED: &str = "tool-refused-tui";
const OFF_DRAFT: &str = "tool-off-draft";
const OFF_PACK: &str = "tool-off-pack";
const MARKER: &str = "host-ran";
/// A verify command the host policy admits, which leaves a marker when it runs.
const MARKER_VERIFY: &str = "python3 -m unittest marker_probe";
const MARKER_PROBE: &str = "import pathlib\nimport unittest\n\npathlib.Path(\"host-ran\").write_text(\"ran\")\n\n\nclass Probe(unittest.TestCase):\n    def test_ok(self):\n        pass\n";

static LOCK: Mutex<()> = Mutex::new(());

fn lock() -> MutexGuard<'static, ()> {
    LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn set_env(name: &str, value: Option<&str>) {
    // SAFETY: every env mutation in this binary holds `LOCK`, and no test here
    // reads the environment without it.
    unsafe {
        match value {
            Some(value) => std::env::set_var(name, value),
            None => std::env::remove_var(name),
        }
    }
}

#[derive(Clone)]
struct ScriptedClient {
    native: bool,
    replies: Arc<Mutex<Vec<AssistantReply>>>,
    requests: Arc<Mutex<Vec<Value>>>,
}

impl ScriptedClient {
    fn new(replies: Vec<AssistantReply>) -> Self {
        Self {
            native: true,
            replies: Arc::new(Mutex::new(replies)),
            requests: Arc::default(),
        }
    }

    fn calls(&self) -> usize {
        self.requests.lock().unwrap().len()
    }
}

impl ChatClient for ScriptedClient {
    fn label(&self) -> &str {
        "scripted"
    }

    fn supports_native_tools(&self, _model: &str) -> bool {
        self.native
    }

    fn boxed_clone(&self) -> Box<dyn ChatClient> {
        Box::new(self.clone())
    }

    fn chat(
        &mut self,
        _model: &str,
        messages: &[ConversationMessage],
        tools: &[commandagent::tools::registry::ToolSpec],
        native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        self.requests.lock().unwrap().push(json!({
            "messages": serde_json::to_string(messages).unwrap(),
            "tools": serde_json::to_string(tools).unwrap(),
            "native": native_tools_enabled,
        }));
        let mut replies = self.replies.lock().unwrap();
        if replies.is_empty() {
            anyhow::bail!("scripted client exhausted");
        }
        Ok(replies.remove(0))
    }
}

fn manifest(profile: &str, declaration: &str) -> String {
    format!(
        r#"[metadata]
id = "{profile}"
display_name = "Tool registration {profile}"
schema_version = "v2"
task_family = "generic"
[plan]
intent = "create"
phases = [
  {{ id = "implementation", prompt = "Write the requested notes for {{goal}}." }},
]
[artifacts]
required = ["notes.txt"]
[guidance.variants.contract]
triggers = [{{ condition = "always" }}]
messages = {{ instruction = "Write notes.txt." }}
[[checks.final]]
id = "scaffold_files_present"
params = {{ files = ["notes.txt"] }}
{declaration}"#
    )
}

const UNREGISTERED_DECLARATION: &str = "[profile_tools]\nschema_version = \"profile-tool.declaration/v1\"\nbuiltin_tools = [\"Read\"]\ntools = [\n  { id = \"never-registered\", effect = \"read\" },\n]\n";

fn write_profile(root: &Path, profile: &str, declaration: &str) {
    let directory = root.join(format!("profiles/{profile}"));
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(
        directory.join("manifest.toml"),
        manifest(profile, declaration),
    )
    .unwrap();
}

struct Extension {
    root: PathBuf,
    pack_directory: PathBuf,
    pack_hash: String,
}

fn extension() -> &'static Extension {
    static EXTENSION: OnceLock<Extension> = OnceLock::new();
    EXTENSION.get_or_init(|| {
        let root = std::fs::canonicalize(tempfile::tempdir().unwrap().keep()).unwrap();
        write_profile(&root, REFUSED, UNREGISTERED_DECLARATION);
        write_profile(&root, OFF_DRAFT, "");
        // The same bytes the base recording selected. Nothing in this binary
        // decodes them before the off control does.
        let assist = format!(
            "schema_version: commandagent.pack.assist/v0\npack:\n  id: {OFF_PACK}\n  version: 1.0.0\n  profile: {OFF_DRAFT}\n  intent: create\nliterals:\n  - gate: ingest_source_binding\n    example:\n      format: text\n      value: x\n"
        );
        let pack_directory = root.join(format!("packs/{OFF_PACK}/1.0.0"));
        std::fs::create_dir_all(&pack_directory).unwrap();
        std::fs::write(pack_directory.join("assist.yaml"), &assist).unwrap();
        let pack_hash = exact_byte_hash(Some(assist.as_bytes()), None);
        std::fs::write(
            pack_directory.join("pack.sha256"),
            format!("{pack_hash}\n"),
        )
        .unwrap();
        commandagent::planner::extension_profiles::register(&root).unwrap();
        Extension {
            root,
            pack_directory,
            pack_hash,
        }
    })
}

/// The run's evidence and state live outside the workspace here, so the digest
/// below sees only what a run does to the workspace itself.
fn config(root: &Path, evidence: &Path, profile: &str) -> Config {
    Config {
        workspace_root: root.to_path_buf(),
        state_dir: evidence.join("state"),
        eval_events_path: Some(evidence.join("events.jsonl")),
        completion_contract_path: None,
        yes: true,
        offline: false,
        context_budget: 100_000,
        model: "m".to_string(),
        provider: Provider::Ollama,
        tool_protocol: None,
        openai_api: OpenAiApi::ChatCompletions,
        prompt_layout: PromptLayout::Stable,
        plan_preset: PlanPreset::None,
        intent_override: None,
        planner_model: "m".to_string(),
        planner_provider: Provider::Ollama,
        planner_think: Some(OllamaThink::False),
        classifier_model: "m".to_string(),
        classifier_provider: Provider::Ollama,
        openai_compatible: None,
        ollama_host: "http://localhost:11434".to_string(),
        ollama_think: None,
        lm_studio_host: "http://localhost:1234".to_string(),
        num_predict: 100,
        max_iterations: 8,
        recovery_plan_auto_runs: 0,
        chat_timeout_secs: 30,
        chat_timeout_source: "override:test".to_string(),
        field_sources: ConfigFieldSources::default(),
        chat_retries: 1,
        stream: false,
        resume: None,
        fresh_session: false,
        no_footer: false,
        narration: NarrationMode::Normal,
        profile: profile.to_string(),
        profile_explicit: true,
        profile_inference: None,
        style: "default".to_string(),
        action: Action::Repl,
    }
}

/// Every file below `root`, with a digest of its bytes.
fn workspace_digest(root: &Path) -> BTreeMap<String, String> {
    let mut digests = BTreeMap::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(directory).unwrap().flatten() {
            let path = entry.path();
            let relative = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .to_string();
            if path.is_dir() {
                digests.insert(format!("{relative}/"), String::new());
                pending.push(path);
            } else {
                let bytes = std::fs::read(&path).unwrap();
                digests.insert(relative, format!("{:x}", Sha256::digest(&bytes)));
            }
        }
    }
    digests
}

fn step(kind: &str, expected_paths: &[&str]) -> PlanStep {
    PlanStep {
        id: format!("step-{kind}"),
        kind: kind.to_string(),
        expected_result: "pass".to_string(),
        instruction: format!("Run the {kind} step"),
        expected_paths: expected_paths.iter().map(|path| path.to_string()).collect(),
        verify: vec![MARKER_VERIFY.to_string()],
    }
}

fn plan_with(steps: Vec<PlanStep>) -> String {
    render_step_plan(&StepPlan {
        goal: "Check the settings".to_string(),
        steps,
    })
}

/// A workspace holding the marker probe, step plans of every kind, and an
/// UltraPlan. Returned with an evidence directory outside it.
struct Fixture {
    _workspace: tempfile::TempDir,
    _evidence: tempfile::TempDir,
    root: PathBuf,
    evidence: PathBuf,
}

fn fixture() -> Fixture {
    let workspace = tempfile::tempdir().unwrap();
    let evidence_dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(workspace.path()).unwrap();
    let evidence = std::fs::canonicalize(evidence_dir.path()).unwrap();
    std::fs::write(root.join("settings.json"), "{}").unwrap();
    std::fs::write(root.join("marker_probe.py"), MARKER_PROBE).unwrap();
    for (name, steps) in [
        ("verify.yaml", vec![step("verify", &[])]),
        (
            "report-then-verify.yaml",
            vec![
                PlanStep {
                    verify: Vec::new(),
                    ..step("report", &[])
                },
                step("verify", &[]),
            ],
        ),
        (
            "report-only.yaml",
            vec![PlanStep {
                verify: Vec::new(),
                ..step("report", &[])
            }],
        ),
        ("report-writes.yaml", vec![step("report", &["report.md"])]),
        ("implement.yaml", vec![step("implement", &["out.txt"])]),
        // The profile-independent verify-first rule: a verify step with an
        // expected path runs its commands before the session.
        (
            "verify-first.yaml",
            vec![step("verify", &["settings.json"])],
        ),
        // A setup step may not run build or test verification.
        (
            "setup.yaml",
            vec![PlanStep {
                verify: Vec::new(),
                ..step("setup", &["setup.txt"])
            }],
        ),
        // An inspect step declares neither paths nor verify commands.
        (
            "inspect.yaml",
            vec![PlanStep {
                verify: Vec::new(),
                ..step("inspect", &[])
            }],
        ),
    ] {
        std::fs::write(root.join(name), plan_with(steps)).unwrap();
    }
    std::fs::write(
        root.join("ultra.yaml"),
        render_ultra_plan(&UltraPlan {
            goal: "Write the notes".to_string(),
            profile: REFUSED.to_string(),
            style: "default".to_string(),
            intent: "create".to_string(),
            phases: vec![
                UltraPhase {
                    id: "implementation".to_string(),
                    prompt: "Write the notes.".to_string(),
                },
                UltraPhase {
                    id: "verification".to_string(),
                    prompt: "Recheck the notes.".to_string(),
                },
            ],
        }),
    )
    .unwrap();
    Fixture {
        _workspace: workspace,
        _evidence: evidence_dir,
        root,
        evidence,
    }
}

fn operator_registry() -> tempfile::TempDir {
    let registry = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(registry.path().join("profile-tools")).unwrap();
    registry
}

fn assert_refused_without_side_effects(
    label: &str,
    fixture: &Fixture,
    before: &BTreeMap<String, String>,
    calls: usize,
    result: Result<String, String>,
) -> Result<(), String> {
    let refused = result
        .as_ref()
        .err()
        .is_some_and(|error| error.contains("profile_tool_refused: unknown_tool_id"));
    let host_verify_ran = fixture.root.join(MARKER).exists();
    let after = workspace_digest(&fixture.root);
    let changed = after
        .iter()
        .filter(|(path, digest)| before.get(*path) != Some(digest))
        .map(|(path, _)| regex_free_id(path))
        .chain(
            before
                .keys()
                .filter(|path| !after.contains_key(*path))
                .map(|path| format!("removed {path}")),
        )
        .collect::<Vec<_>>();
    let stop = match &result {
        Ok(text) => text.lines().next().unwrap_or_default().to_string(),
        Err(error) => error
            .lines()
            .find_map(|line| line.strip_prefix("Primary stop reason: "))
            .unwrap_or(error)
            .chars()
            .take(120)
            .collect(),
    };
    let row = format!(
        "ROUTE {label}: refused={refused} provider_calls={calls} host_verify_ran={host_verify_ran} changed={changed:?} ended={stop:?}"
    );
    println!("{row}");
    if refused && calls == 0 && !host_verify_ran && changed.is_empty() {
        Ok(())
    } else {
        Err(row)
    }
}

/// A generated file name is shown without its run-specific UUID.
fn regex_free_id(path: &str) -> String {
    regex::Regex::new(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}")
        .unwrap()
        .replace_all(path, "<ID>")
        .into_owned()
}

fn assert_all_routes(outcomes: Vec<Result<(), String>>) {
    let failures = outcomes
        .into_iter()
        .filter_map(Result::err)
        .collect::<Vec<_>>();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn run_command(
    label: &str,
    line: &str,
    config_for: impl Fn(&Fixture) -> Config,
) -> Result<(), String> {
    let fixture = fixture();
    let before = workspace_digest(&fixture.root);
    let mut planner = ScriptedClient::new(Vec::new());
    let mut execution = ScriptedClient::new(Vec::new());
    let result = handle_command(
        line,
        &config_for(&fixture),
        &mut planner,
        &mut execution,
        &NOOP_UI,
    )
    .map_err(|error| format!("{error:#}"));
    assert_refused_without_side_effects(
        label,
        &fixture,
        &before,
        planner.calls() + execution.calls(),
        result,
    )
}

fn refused_config(fixture: &Fixture) -> Config {
    config(&fixture.root, &fixture.evidence, REFUSED)
}

/// What `/profile` leaves in the REPL's config: `update_profile` in
/// `src/tui/repl.rs` changes these fields and nothing else about a dispatch.
fn after_profile_command(fixture: &Fixture) -> Config {
    let mut updated = config(&fixture.root, &fixture.evidence, "generic");
    updated.profile = REFUSED.to_string();
    updated.profile_explicit = true;
    updated.profile_inference = None;
    updated.field_sources.profile = "repl".to_string();
    updated
}

#[test]
fn the_marker_is_a_real_witness_of_a_host_verify_command() {
    let _guard = lock();
    set_env(REGISTRY_ENV, None);
    extension();
    let fixture = fixture();
    let mut planner = ScriptedClient::new(Vec::new());
    let mut execution = ScriptedClient::new(Vec::new());
    let result = handle_command(
        "/run-plan verify-first.yaml",
        &config(&fixture.root, &fixture.evidence, "generic"),
        &mut planner,
        &mut execution,
        &NOOP_UI,
    )
    .map_err(|error| format!("{error:#}"));
    assert!(result.is_ok(), "{result:?}");
    assert!(
        fixture.root.join(MARKER).exists(),
        "the off control did not run the verify command"
    );
    assert_eq!(planner.calls() + execution.calls(), 0);
}

#[test]
fn slash_commands_do_nothing_to_the_workspace_before_a_refusal() {
    let _guard = lock();
    extension();
    let registry = operator_registry();
    set_env(REGISTRY_ENV, registry.path().to_str());
    let mut outcomes = Vec::new();
    for plan in [
        "verify.yaml",
        "report-then-verify.yaml",
        "report-only.yaml",
        "report-writes.yaml",
        "implement.yaml",
        "verify-first.yaml",
        "setup.yaml",
        "inspect.yaml",
    ] {
        let line = format!("/run-plan {plan}");
        outcomes.push(run_command(&line, &line, refused_config));
    }
    for line in [
        "/plan-run Write the notes",
        "/plan-steps Write the notes",
        "/ultra-plan-run Write the notes",
        "/ultra-plan Write the notes",
    ] {
        outcomes.push(run_command(line, line, refused_config));
    }
    set_env(REGISTRY_ENV, None);
    assert_all_routes(outcomes);
}

#[test]
fn a_profile_set_by_the_profile_command_or_a_slash_flag_is_refused_the_same_way() {
    let _guard = lock();
    extension();
    let registry = operator_registry();
    set_env(REGISTRY_ENV, registry.path().to_str());
    let mut outcomes = Vec::new();
    for line in [
        "/run-plan verify.yaml",
        "/run-plan verify-first.yaml",
        "/run-plan report-then-verify.yaml",
        "/plan-run Write the notes",
        "/plan-steps Write the notes",
        "/ultra-plan-run Write the notes",
        "/ultra-plan Write the notes",
    ] {
        outcomes.push(run_command(
            &format!("/profile then {line}"),
            line,
            after_profile_command,
        ));
    }
    for line in [
        format!("/plan-run --profile {REFUSED} Write the notes"),
        format!("/ultra-plan-run --profile {REFUSED} Write the notes"),
    ] {
        outcomes.push(run_command(&line, &line, |fixture| {
            config(&fixture.root, &fixture.evidence, "generic")
        }));
    }
    set_env(REGISTRY_ENV, None);
    assert_all_routes(outcomes);
}

fn gate_one_proposal(workspace: &Path) -> RouteProposal {
    let deterministic = deterministic_route(RouteRequest {
        request: "Write the notes",
        workspace,
        explicit: ExplicitRouteBinding {
            profile: Some(commandagent::planner::profile::ProfileId::parse(REFUSED)),
            ..ExplicitRouteBinding::default()
        },
    });
    assert_eq!(deterministic.resolution, DeterministicResolution::Unique);
    RouteProposal {
        selected: deterministic.candidates.first().cloned(),
        alternatives: deterministic.candidates,
        classifier: ClassifierProvenance {
            used: false,
            provider: "ollama".to_string(),
            model: "planner".to_string(),
            prompt_version: "issue972-test",
            candidate_keys: Vec::new(),
            raw_response_hash: None,
            parse_reason: "deterministic_unique".to_string(),
        },
        status: ProposalStatus::AwaitingConfirmation,
        confirmation_required: true,
    }
}

/// The Gate 1 confirmation, through the real shell: propose, persist, dispatch,
/// with the command and config `src/tui/repl.rs` builds for a confirmed identity.
#[test]
fn a_gate_one_dispatch_does_nothing_to_the_workspace_before_a_refusal() {
    let _guard = lock();
    extension();
    let registry = operator_registry();
    set_env(REGISTRY_ENV, registry.path().to_str());
    let fixture = fixture();
    let before = workspace_digest(&fixture.root);
    let mut shell = BoundaryShell::new(fixture.evidence.join("confirmations"), None);
    let identity = shell
        .begin_gate_one(
            gate_one_proposal(&fixture.root),
            "Write the notes",
            &fixture.root,
            ExecutionPins {
                planner_provider: "ollama".to_string(),
                planner_model: "m".to_string(),
                executor_provider: "ollama".to_string(),
                executor_model: "m".to_string(),
                preset: "profile".to_string(),
                think: None,
            },
            PackSelection::None,
        )
        .unwrap()
        .clone();
    shell.confirm(&identity.card_hash().unwrap()).unwrap();
    let command = format!(
        "/ultra-plan-run --profile {} {}",
        identity.profile, identity.request
    );
    let mut planner = ScriptedClient::new(Vec::new());
    let mut execution = ScriptedClient::new(Vec::new());
    let dispatch_config = refused_config(&fixture);
    let result = shell
        .dispatch(|_| {
            handle_command(
                &command,
                &dispatch_config,
                &mut planner,
                &mut execution,
                &NOOP_UI,
            )
        })
        .map_err(|error| format!("{error:#}"));
    let outcome = assert_refused_without_side_effects(
        "Gate 1",
        &fixture,
        &before,
        planner.calls() + execution.calls(),
        result,
    );
    set_env(REGISTRY_ENV, None);
    assert_all_routes(vec![outcome]);
}

/// Gate 4 hands a saved plan to the same command. `dispatch_directive` only checks
/// the confirmed directive before it, so the command is what can have an effect.
#[test]
fn a_gate_four_continuation_command_does_nothing_to_the_workspace_before_a_refusal() {
    let _guard = lock();
    extension();
    let registry = operator_registry();
    set_env(REGISTRY_ENV, registry.path().to_str());
    let outcome = run_command(
        "Gate 4 /run-ultra-plan",
        "/run-ultra-plan ultra.yaml",
        refused_config,
    );
    set_env(REGISTRY_ENV, None);
    assert_all_routes(vec![outcome]);
}

#[test]
fn a_refused_ultra_plan_records_its_command_start_and_no_generation_attempt() {
    let _guard = lock();
    extension();
    let registry = operator_registry();
    set_env(REGISTRY_ENV, registry.path().to_str());
    let fixture = fixture();
    let mut planner = ScriptedClient::new(Vec::new());
    let mut execution = ScriptedClient::new(Vec::new());
    let result = handle_command(
        "/ultra-plan Write the notes",
        &refused_config(&fixture),
        &mut planner,
        &mut execution,
        &NOOP_UI,
    );
    set_env(REGISTRY_ENV, None);
    assert!(result.is_err());
    let events = std::fs::read_to_string(fixture.evidence.join("events.jsonl")).unwrap();
    assert!(events.contains("\"tui_command_start\""), "{events}");
    assert!(
        !events.contains("ultra_plan_generation_attempt"),
        "{events}"
    );
}

// ---------------------------------------------------------------------------
// A plan file that reaches no session or planner call.
// ---------------------------------------------------------------------------

/// The library entry. Each plan finishes without a session or planner call for a
/// profile with no declaration, so only the entry's own admission can refuse it.
#[test]
fn the_plan_file_library_entry_refuses_a_plan_that_reaches_no_session() {
    let _guard = lock();
    extension();
    let registry = operator_registry();
    set_env(REGISTRY_ENV, registry.path().to_str());
    let mut outcomes = Vec::new();
    for (entry, plan) in [
        ("library run_plan_file", "verify-first.yaml"),
        ("library run_plan_file", "report-only.yaml"),
    ] {
        let fixture = fixture();
        let before = workspace_digest(&fixture.root);
        let mut client = ScriptedClient::new(Vec::new());
        let result = commandagent::planner::run_plan_file(
            &mut client,
            Path::new(plan),
            &refused_config(&fixture),
        )
        .map_err(|error| format!("{error:#}"));
        outcomes.push(assert_refused_without_side_effects(
            &format!("{entry} {plan}"),
            &fixture,
            &before,
            client.calls(),
            result,
        ));
    }
    set_env(REGISTRY_ENV, None);
    assert_all_routes(outcomes);
}

#[test]
fn the_same_plans_finish_through_the_library_entry_with_no_declaration() {
    let _guard = lock();
    set_env(REGISTRY_ENV, None);
    extension();
    for plan in ["verify-first.yaml", "report-only.yaml"] {
        let fixture = fixture();
        let mut client = ScriptedClient::new(Vec::new());
        let result = commandagent::planner::run_plan_file(
            &mut client,
            Path::new(plan),
            &config(&fixture.root, &fixture.evidence, "generic"),
        );
        assert!(result.is_ok(), "{plan}: {result:?}");
        assert_eq!(client.calls(), 0, "{plan}");
    }
}

/// The three library entries of automatic recovery, with recovery off and on: a
/// refused run reaches neither client and leaves no recovery file.
#[test]
fn the_auto_recovery_library_entries_refuse_before_anything_runs() {
    use commandagent::planner::auto_recovery::{
        generate_and_run_with_ui, run_file_with_ui, run_plan_with_ui,
    };
    let _guard = lock();
    extension();
    let registry = operator_registry();
    set_env(REGISTRY_ENV, registry.path().to_str());
    let mut outcomes = Vec::new();
    for auto_runs in [0, 1] {
        for entry in [
            "generate_and_run_with_ui",
            "run_file_with_ui",
            "run_plan_with_ui",
        ] {
            let fixture = fixture();
            let before = workspace_digest(&fixture.root);
            let mut planner = ScriptedClient::new(Vec::new());
            let mut execution = ScriptedClient::new(Vec::new());
            let mut refused = refused_config(&fixture);
            refused.recovery_plan_auto_runs = auto_runs;
            let plan = UltraPlan {
                goal: "Write the notes".to_string(),
                profile: REFUSED.to_string(),
                style: "default".to_string(),
                intent: "create".to_string(),
                phases: vec![UltraPhase {
                    id: "implementation".to_string(),
                    prompt: "Write the notes.".to_string(),
                }],
            };
            let result = match entry {
                "generate_and_run_with_ui" => generate_and_run_with_ui(
                    &mut planner,
                    &mut execution,
                    "Write the notes",
                    &refused,
                    &NOOP_UI,
                ),
                "run_file_with_ui" => run_file_with_ui(
                    &mut planner,
                    &mut execution,
                    Path::new("ultra.yaml"),
                    &refused,
                    &NOOP_UI,
                ),
                _ => run_plan_with_ui(&mut planner, &mut execution, &plan, &refused, &NOOP_UI),
            }
            .map_err(|error| format!("{error:#}"));
            assert!(
                !fixture.root.join(".commandagent").exists(),
                "{entry}: a recovery file was written"
            );
            outcomes.push(assert_refused_without_side_effects(
                &format!("{entry} recovery_plan_auto_runs={auto_runs}"),
                &fixture,
                &before,
                planner.calls() + execution.calls(),
                result,
            ));
        }
    }
    set_env(REGISTRY_ENV, None);
    assert_all_routes(outcomes);
}

/// With recovery runs configured, `auto_recovery` records its configuration and starts
/// the recovery authority before anything below it could refuse. A refusal comes first.
#[test]
fn the_auto_recovery_entries_refuse_before_recording_their_recovery_configuration() {
    use commandagent::planner::auto_recovery::{
        generate_and_run_with_ui, run_file_with_ui, run_plan_with_ui,
    };
    let _guard = lock();
    extension();
    let registry = operator_registry();
    set_env(REGISTRY_ENV, registry.path().to_str());
    let mut outcomes = Vec::new();
    for entry in [
        "generate_and_run_with_ui",
        "run_file_with_ui",
        "run_plan_with_ui",
    ] {
        let fixture = fixture();
        let before = workspace_digest(&fixture.root);
        let mut refused = refused_config(&fixture);
        assert!(refused.eval_events_path.is_some());
        refused.recovery_plan_auto_runs = 1;
        let plan = commandagent::planner::ultra_plan::parse_ultra_plan(
            &std::fs::read_to_string(fixture.root.join("ultra.yaml")).unwrap(),
        )
        .unwrap();
        let mut planner = ScriptedClient::new(Vec::new());
        let mut execution = ScriptedClient::new(Vec::new());
        let result = match entry {
            "generate_and_run_with_ui" => generate_and_run_with_ui(
                &mut planner,
                &mut execution,
                "Write the notes",
                &refused,
                &NOOP_UI,
            ),
            "run_file_with_ui" => run_file_with_ui(
                &mut planner,
                &mut execution,
                Path::new("ultra.yaml"),
                &refused,
                &NOOP_UI,
            ),
            _ => run_plan_with_ui(&mut planner, &mut execution, &plan, &refused, &NOOP_UI),
        }
        .map_err(|error| format!("{error:#}"));
        assert_no_events(entry, &fixture);
        outcomes.push(assert_refused_without_side_effects(
            entry,
            &fixture,
            &before,
            planner.calls() + execution.calls(),
            result,
        ));
    }
    set_env(REGISTRY_ENV, None);
    assert_all_routes(outcomes);
}

/// A refused public entry leaves no event at all, so no `request_pattern_resolved`.
fn assert_no_events(label: &str, fixture: &Fixture) {
    let events = std::fs::read_to_string(fixture.evidence.join("events.jsonl")).unwrap_or_default();
    assert_eq!(events, "", "{label}");
}

/// Stands in for jev and counts what would have been a call to it.
struct CountingClassifier {
    calls: AtomicUsize,
}

impl PatternClassifier for CountingClassifier {
    fn classify(&self, _request: &str) -> Result<Classification, ClassifyFailure> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(Classification {
            pattern: None,
            confidence: 1.0,
            model: None,
        })
    }
}

const JEV: RequestPatternSettings = RequestPatternSettings {
    mode: RequestPatternMode::Jev,
    threshold: 0.5,
};

fn direct_prompt(
    entry: &str,
    client: &mut ScriptedClient,
    config: &Config,
    classifier: &CountingClassifier,
) -> Result<String, String> {
    let mut session = SessionSnapshot::new();
    match entry {
        "run_direct_prompt_with" => run_direct_prompt_with(
            client,
            &mut session,
            "Write the notes",
            config,
            &NOOP_UI,
            JEV,
            Some(classifier),
        ),
        _ => run_direct_prompt_with_common(
            client,
            &mut session,
            "Write the notes",
            config,
            &NOOP_UI,
            JEV,
            Some(classifier),
            Some("Common guidance."),
        ),
    }
    .map(|outcome| outcome.final_text)
    .map_err(|error| format!("{error:#}"))
}

const DIRECT_PROMPT_ENTRIES: [&str; 2] =
    ["run_direct_prompt_with", "run_direct_prompt_with_common"];

#[test]
fn the_direct_prompt_library_entries_refuse_before_jev_and_the_provider() {
    let _guard = lock();
    extension();
    let registry = operator_registry();
    set_env(REGISTRY_ENV, registry.path().to_str());
    let mut outcomes = Vec::new();
    for entry in DIRECT_PROMPT_ENTRIES {
        let fixture = fixture();
        let before = workspace_digest(&fixture.root);
        let mut client = ScriptedClient::new(Vec::new());
        let classifier = CountingClassifier {
            calls: AtomicUsize::new(0),
        };
        let result = direct_prompt(entry, &mut client, &refused_config(&fixture), &classifier);
        assert_eq!(classifier.calls.load(Ordering::SeqCst), 0, "{entry}");
        assert_no_events(entry, &fixture);
        outcomes.push(assert_refused_without_side_effects(
            entry,
            &fixture,
            &before,
            client.calls(),
            result,
        ));

        // The witnesses count real traffic: with no declaration the same call reaches
        // both the classifier and the provider, and records the event.
        let open = self::fixture();
        let mut client = ScriptedClient::new(Vec::new());
        let classifier = CountingClassifier {
            calls: AtomicUsize::new(0),
        };
        let off = config(&open.root, &open.evidence, OFF_DRAFT);
        let _ = direct_prompt(entry, &mut client, &off, &classifier);
        assert_eq!(classifier.calls.load(Ordering::SeqCst), 1, "{entry}");
        assert!(client.calls() > 0, "{entry}");
        let events = std::fs::read_to_string(open.evidence.join("events.jsonl")).unwrap();
        assert!(events.contains("request_pattern_resolved"), "{entry}");
    }
    set_env(REGISTRY_ENV, None);
    assert_all_routes(outcomes);
}

/// Each way a plan can be generated: by the planner, and by the two prebuilt branches
/// that never reach a planner call.
fn generation_configs(fixture: &Fixture) -> Vec<(&'static str, Config)> {
    let planner = refused_config(fixture);
    let mut explicit_fix = refused_config(fixture);
    explicit_fix.intent_override = Some(IntentId::Fix);
    let mut profile_preset = refused_config(fixture);
    profile_preset.plan_preset = PlanPreset::Profile;
    vec![
        ("planner", planner),
        ("explicit-fix", explicit_fix),
        ("profile-preset", profile_preset),
    ]
}

#[test]
fn the_ultra_plan_generation_entries_refuse_before_any_plan_exists() {
    let _guard = lock();
    extension();
    let registry = operator_registry();
    set_env(REGISTRY_ENV, registry.path().to_str());
    let mut outcomes = Vec::new();
    for entry in [
        "generate_ultra_plan",
        "generate_ultra_plan_with_ui",
        "generate_and_run_ultra_plan",
        "generate_and_run_ultra_plan_with_ui",
    ] {
        for index in 0..3 {
            let fixture = fixture();
            let before = workspace_digest(&fixture.root);
            let (shape, refused) = generation_configs(&fixture).remove(index);
            let label = format!("{entry} {shape}");
            let mut planner = ScriptedClient::new(Vec::new());
            let mut execution = ScriptedClient::new(Vec::new());
            let goal = "Write the notes";
            let result = match entry {
                "generate_ultra_plan" => {
                    generate_ultra_plan(&mut planner, goal, &refused).map(|plan| plan.goal)
                }
                "generate_ultra_plan_with_ui" => {
                    generate_ultra_plan_with_ui(&mut planner, goal, &refused, &NOOP_UI)
                        .map(|plan| plan.goal)
                }
                "generate_and_run_ultra_plan" => {
                    generate_and_run_ultra_plan(&mut planner, &mut execution, goal, &refused)
                }
                _ => generate_and_run_ultra_plan_with_ui(
                    &mut planner,
                    &mut execution,
                    goal,
                    &refused,
                    &NOOP_UI,
                ),
            }
            .map_err(|error| format!("{error:#}"));
            assert_no_events(&label, &fixture);
            outcomes.push(assert_refused_without_side_effects(
                &label,
                &fixture,
                &before,
                planner.calls() + execution.calls(),
                result,
            ));
        }
    }
    set_env(REGISTRY_ENV, None);
    assert_all_routes(outcomes);
}

#[test]
fn the_ultra_plan_run_entries_refuse_before_anything_runs() {
    let _guard = lock();
    extension();
    let registry = operator_registry();
    set_env(REGISTRY_ENV, registry.path().to_str());
    let mut outcomes = Vec::new();
    for entry in [
        "run_ultra_plan",
        "run_ultra_plan_with_ui",
        "run_ultra_plan_file",
        "run_ultra_plan_file_with_ui",
    ] {
        let fixture = fixture();
        let before = workspace_digest(&fixture.root);
        let refused = refused_config(&fixture);
        // The plan the file holds: it passes its lint, so only the refusal stops it.
        let plan = commandagent::planner::ultra_plan::parse_ultra_plan(
            &std::fs::read_to_string(fixture.root.join("ultra.yaml")).unwrap(),
        )
        .unwrap();
        let file = Path::new("ultra.yaml");
        let mut planner = ScriptedClient::new(Vec::new());
        let mut execution = ScriptedClient::new(Vec::new());
        let result = match entry {
            "run_ultra_plan" => run_ultra_plan(&mut planner, &mut execution, &plan, &refused),
            "run_ultra_plan_with_ui" => {
                run_ultra_plan_with_ui(&mut planner, &mut execution, &plan, &refused, &NOOP_UI)
            }
            "run_ultra_plan_file" => {
                run_ultra_plan_file(&mut planner, &mut execution, file, &refused)
            }
            _ => {
                run_ultra_plan_file_with_ui(&mut planner, &mut execution, file, &refused, &NOOP_UI)
            }
        }
        .map_err(|error| format!("{error:#}"));
        assert_no_events(entry, &fixture);
        outcomes.push(assert_refused_without_side_effects(
            entry,
            &fixture,
            &before,
            planner.calls() + execution.calls(),
            result,
        ));
    }
    set_env(REGISTRY_ENV, None);
    assert_all_routes(outcomes);
}

const MODEL_PROBE_CHILD_ENV: &str = "COMMANDAGENT_TEST_972_MODEL_PROBE_CHILD";

/// A probe's scratch workspace goes below the temporary directory and is removed when
/// it ends, and `run_configured` writes below `HOME`. So this runs alone in its own
/// process with both private: nothing it does reaches the real ones, and a scratch
/// that came and went still changes the temporary directory's modification time.
#[test]
fn the_model_probe_library_entries_refuse_before_any_scratch_or_output() {
    if std::env::var_os(MODEL_PROBE_CHILD_ENV).is_none() {
        let private = tempfile::tempdir().unwrap();
        let (home, temporary) = (private.path().join("home"), private.path().join("tmp"));
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&temporary).unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "the_model_probe_library_entries_refuse_before_any_scratch_or_output",
                "--nocapture",
            ])
            .env(MODEL_PROBE_CHILD_ENV, "1")
            .env("HOME", &home)
            .env("TMPDIR", &temporary)
            .status()
            .unwrap();
        assert!(status.success(), "{status}");
        return;
    }
    let _guard = lock();
    extension();
    let registry = operator_registry();
    set_env(REGISTRY_ENV, registry.path().to_str());
    let home = PathBuf::from(std::env::var_os("HOME").unwrap());
    let mut outcomes = Vec::new();
    for entry in ["run_configured", "run_with_output_dir"] {
        let fixture = fixture();
        let before = workspace_digest(&fixture.root);
        let home_before = workspace_digest(&home);
        let output = fixture.evidence.join("probe-output");
        let refused = refused_config(&fixture);
        let temporary_before = std::fs::metadata(std::env::temp_dir())
            .unwrap()
            .modified()
            .unwrap();
        let mut planner = ScriptedClient::new(Vec::new());
        let mut execution = ScriptedClient::new(Vec::new());
        let result = match entry {
            "run_configured" => {
                commandagent::model_probe::run_configured(&refused, &mut planner, &mut execution)
            }
            _ => commandagent::model_probe::run_with_output_dir(
                &refused,
                &mut planner,
                &mut execution,
                output.clone(),
            ),
        }
        .map(|_| String::new())
        .map_err(|error| format!("{error:#}"));
        assert_eq!(
            std::fs::metadata(std::env::temp_dir())
                .unwrap()
                .modified()
                .unwrap(),
            temporary_before,
            "{entry}: a scratch workspace was made"
        );
        assert!(!output.exists(), "{entry}: the output directory was made");
        assert_eq!(workspace_digest(&home), home_before, "{entry}");
        assert_no_events(entry, &fixture);
        outcomes.push(assert_refused_without_side_effects(
            entry,
            &fixture,
            &before,
            planner.calls() + execution.calls(),
            result,
        ));
    }
    set_env(REGISTRY_ENV, None);
    assert_all_routes(outcomes);
}

#[test]
fn the_step_plan_run_library_entries_refuse_before_a_host_command() {
    let _guard = lock();
    extension();
    let registry = operator_registry();
    set_env(REGISTRY_ENV, registry.path().to_str());
    let mut outcomes = Vec::new();
    for entry in ["run_step_plan", "run_step_plan_with_ui"] {
        let fixture = fixture();
        let before = workspace_digest(&fixture.root);
        // A verify step with an expected path runs its command before any session.
        let plan = StepPlan {
            goal: "Check the settings".to_string(),
            steps: vec![step("verify", &["settings.json"])],
        };
        let refused = refused_config(&fixture);
        let mut client = ScriptedClient::new(Vec::new());
        let result = match entry {
            "run_step_plan" => run_step_plan(&mut client, &plan, &refused),
            _ => run_step_plan_with_ui(&mut client, &plan, &refused, &NOOP_UI),
        }
        .map_err(|error| format!("{error:#}"));
        assert_no_events(entry, &fixture);
        outcomes.push(assert_refused_without_side_effects(
            entry,
            &fixture,
            &before,
            client.calls(),
            result,
        ));
    }
    set_env(REGISTRY_ENV, None);
    assert_all_routes(outcomes);
}

#[test]
fn the_generate_and_run_step_plan_entries_refuse_before_any_plan_exists() {
    let _guard = lock();
    extension();
    let registry = operator_registry();
    set_env(REGISTRY_ENV, registry.path().to_str());
    let mut outcomes = Vec::new();
    for entry in [
        "generate_and_run_step_plan",
        "generate_and_run_step_plan_with_ui",
    ] {
        let fixture = fixture();
        let before = workspace_digest(&fixture.root);
        let refused = refused_config(&fixture);
        let mut planner = ScriptedClient::new(Vec::new());
        let mut execution = ScriptedClient::new(Vec::new());
        let goal = "Check the settings";
        let result = match entry {
            "generate_and_run_step_plan" => {
                generate_and_run_step_plan(&mut planner, &mut execution, goal, &refused)
            }
            _ => generate_and_run_step_plan_with_ui(
                &mut planner,
                &mut execution,
                goal,
                &refused,
                &NOOP_UI,
            ),
        }
        .map_err(|error| format!("{error:#}"));
        outcomes.push(assert_refused_without_side_effects(
            entry,
            &fixture,
            &before,
            planner.calls() + execution.calls(),
            result,
        ));
    }
    set_env(REGISTRY_ENV, None);
    assert_all_routes(outcomes);
}

/// The Gate 4 continuation, through the real shell: a failed run, a confirmed
/// directive, and the command `src/tui/repl.rs` runs inside `dispatch_directive`.
#[test]
fn a_directive_dispatch_does_nothing_to_the_workspace_before_a_refusal() {
    let _guard = lock();
    extension();
    let registry = operator_registry();
    set_env(REGISTRY_ENV, registry.path().to_str());
    let fixture = fixture();
    let before = workspace_digest(&fixture.root);
    let audit = fixture.evidence.join("boundary-events.jsonl");
    let mut shell = BoundaryShell::new(fixture.evidence.join("confirmations"), Some(audit.clone()));
    let identity = shell
        .begin_gate_one(
            gate_one_proposal(&fixture.root),
            "Write the notes",
            &fixture.root,
            ExecutionPins {
                planner_provider: "ollama".to_string(),
                planner_model: "m".to_string(),
                executor_provider: "ollama".to_string(),
                executor_model: "m".to_string(),
                preset: "profile".to_string(),
                think: None,
            },
            PackSelection::None,
        )
        .unwrap()
        .clone();
    shell.confirm(&identity.card_hash().unwrap()).unwrap();
    shell.dispatch(|_| Ok("failed".to_string())).unwrap();
    shell
        .present_terminal(
            "# sheet\n\n## 5. Stop reason\nfailed".to_string(),
            false,
            Some("failed".to_string()),
        )
        .unwrap();
    commandagent::eval_events::emit(
        Some(&audit),
        json!({
            "event": "tui_command_stop",
            "ok": false,
            "status": "failed",
            "stop_reason": "fixture failure",
        }),
    );
    let directive = shell
        .begin_directive("Repair the notes", "run-001", 1)
        .unwrap()
        .clone();
    let continuation = DirectiveContinuation {
        plan_path: fixture.root.join("ultra.yaml"),
        plan_workspace_path: "ultra.yaml".to_string(),
        target_run_id: "run-001".to_string(),
        directive_round: 1,
        directive_hash: directive.hash().to_string(),
        regression_freeze: None,
    };
    shell.confirm_directive(directive.hash()).unwrap();
    let command = format!("/run-ultra-plan {}", continuation.plan_workspace_path);
    let dispatch_config = refused_config(&fixture);
    let mut planner = ScriptedClient::new(Vec::new());
    let mut execution = ScriptedClient::new(Vec::new());
    let result = shell
        .dispatch_directive(&continuation, || {
            handle_command(
                &command,
                &dispatch_config,
                &mut planner,
                &mut execution,
                &NOOP_UI,
            )
        })
        .map_err(|error| format!("{error:#}"));
    let outcome = assert_refused_without_side_effects(
        "dispatch_directive",
        &fixture,
        &before,
        planner.calls() + execution.calls(),
        result,
    );
    set_env(REGISTRY_ENV, None);
    assert_all_routes(vec![outcome]);
}

const OWN_PROCESS_ENV: &str = "COMMANDAGENT_TEST_972_OWN_PROCESS";

/// Runs `test` alone in its own process with these variables, because what it installs
/// (a workflow's sandbox, a fake on `PATH`) is process-wide. Returns true in the child.
fn in_own_process(test: &str, environment: &[(&str, &std::ffi::OsStr)]) -> bool {
    if std::env::var_os(OWN_PROCESS_ENV).is_some() {
        return true;
    }
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", test, "--nocapture"])
        .env(OWN_PROCESS_ENV, "1");
    for (name, value) in environment {
        command.env(name, value);
    }
    let status = command.status().unwrap();
    assert!(status.success(), "{status}");
    false
}

const REPRODUCER_MARKER: &str = "reproducer-ran";

/// An origin whose first reproducer candidate leaves a marker when a host runs it, and
/// the workflow around it. Returns the origin and the definition.
fn workflow_origin(root: &Path) -> (PathBuf, PathBuf) {
    let origin = root.join("origin");
    let run_dir = origin.join(".anvil/runs/origin-run");
    std::fs::create_dir_all(&run_dir).unwrap();
    std::fs::create_dir_all(origin.join(".anvil/plans")).unwrap();
    std::fs::create_dir_all(origin.join("pipeline")).unwrap();
    std::fs::write(
        run_dir.join("events.jsonl"),
        concat!(
            r#"{"event":"run_start","action":"Prompt(\"origin goal\")"}"#,
            "\n",
            r#"{"event":"run_stop","status":"failed"}"#,
            "\n"
        ),
    )
    .unwrap();
    std::fs::write(
        origin.join(".anvil/plans/recovery-origin.yaml"),
        "version: 1\n",
    )
    .unwrap();
    std::fs::write(
        origin.join("pipeline/main.py"),
        format!("open(\"{REPRODUCER_MARKER}\", \"w\").close()\nraise SystemExit(1)\n"),
    )
    .unwrap();
    let definition = root.join("workflow.yaml");
    std::fs::write(
        &definition,
        "workflow: outer-profile\nversion: 0.2\nentry: create\nnodes:\n  create: { intent: create, profile: generic }\n  rebuild: { intent: create, profile: generic }\nroutes:\n  - { from: create, on: failed, when: recovery_yaml_present, to: rebuild, carry: [workspace, recovery_yaml] }\nterminal: {}\n",
    )
    .unwrap();
    (origin, definition)
}

#[test]
fn the_library_workflow_entry_refuses_before_the_origin_reproducer_runs() {
    let private = tempfile::tempdir().unwrap();
    let home = private.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    if !in_own_process(
        "the_library_workflow_entry_refuses_before_the_origin_reproducer_runs",
        &[("HOME", home.as_os_str())],
    ) {
        return;
    }
    let _guard = lock();
    extension();
    let registry = operator_registry();
    set_env(REGISTRY_ENV, registry.path().to_str());

    let refused_root = std::fs::canonicalize(private.path()).unwrap();
    let (origin, definition) = workflow_origin(&refused_root);
    let before = workspace_digest(&origin);
    let fixture = fixture();
    let provider = CountingEndpoint::start();
    let mut refused = refused_config(&fixture);
    refused.ollama_host = provider.url.clone();
    let error = commandagent::workflow::orchestrator::run_workflow(&refused, &definition, &origin)
        .unwrap_err();
    assert!(
        format!("{error:#}").contains("profile_tool_refused: unknown_tool_id"),
        "{error:#}"
    );
    assert_eq!(provider.finish(), 0);
    assert!(!origin.join(REPRODUCER_MARKER).exists());
    assert!(!origin.join("evidence/workflow-events.jsonl").exists());
    assert_eq!(workspace_digest(&origin), before);
    assert_no_events("run_workflow", &fixture);

    // The marker is a real witness: with no declaration the same call runs it.
    let open = tempfile::tempdir().unwrap();
    let open_root = std::fs::canonicalize(open.path()).unwrap();
    let (origin, definition) = workflow_origin(&open_root);
    let fixture = self::fixture();
    let provider = CountingEndpoint::start();
    let mut off = config(&fixture.root, &fixture.evidence, OFF_DRAFT);
    off.ollama_host = provider.url.clone();
    let _ = commandagent::workflow::orchestrator::run_workflow(&off, &definition, &origin);
    provider.finish();
    assert!(origin.join(REPRODUCER_MARKER).exists());
    assert!(origin.join("evidence/workflow-events.jsonl").exists());
    set_env(REGISTRY_ENV, None);
}

/// Everything a `--workflow` run shows, with only what differs by construction made
/// the same: its private directories, port, ids, build stamp, and measured times.
fn workflow_cli_observation(
    binary: &Path,
    profile: &str,
    workspace_is_origin: bool,
) -> BTreeMap<String, String> {
    let private = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(private.path()).unwrap();
    let (origin, definition) = workflow_origin(&root);
    let outer = if workspace_is_origin {
        origin.clone()
    } else {
        root.join("outer")
    };
    let home = root.join("home");
    std::fs::create_dir_all(&outer).unwrap();
    std::fs::create_dir_all(&home).unwrap();
    let registry = operator_registry();
    let provider = CountingEndpoint::start();
    let output = std::process::Command::new(binary)
        .arg("--extension-root")
        .arg(&extension().root)
        .args(["--ollama-host", &provider.url, "--model", "m"])
        .args(["--profile", profile, "--workflow"])
        .arg(&definition)
        .arg("--origin")
        .arg(&origin)
        .current_dir(&outer)
        .env_clear()
        .env("HOME", &home)
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env(REGISTRY_ENV, registry.path())
        .output()
        .unwrap();
    let provider_url = provider.url.clone();
    let provider_calls = provider.finish();
    let volatile = regex::Regex::new(concat!(
        r#"("(?:build_commit|build_dirty|build_timestamp|epoch|pid|[a-z_]*_at|[a-z_]*_epoch_ms|"#,
        r#"[a-z_]*duration_ms|[a-z_]*elapsed_ms|builds_ms|executor_ms|installs_ms|latency_ms|"#,
        r#"other_ms|planner_ms|planner_step_ms|planner_ultra_ms|probe_ms|provider_ms|repair_ms|"#,
        r#"time_profile_[a-z_]*ms|total_ms)":\s*)("[^"]*"|[a-z0-9.]+)"#,
    ))
    .unwrap();
    let identifier =
        regex::Regex::new(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}").unwrap();
    let reported_time = regex::Regex::new(r"elapsed_ms: [0-9]+").unwrap();
    let stable = |text: &str| {
        let text = text
            .replace(&root.display().to_string(), "<ROOT>")
            .replace(&registry.path().display().to_string(), "<REGISTRY>")
            .replace(&extension().root.display().to_string(), "<EXTENSION>")
            .replace(&provider_url, "<PROVIDER>");
        let text = volatile.replace_all(&text, "${1}<V>");
        let text = reported_time.replace_all(&text, "elapsed_ms: <V>");
        identifier.replace_all(&text, "<ID>").into_owned()
    };
    let mut seen = BTreeMap::new();
    seen.insert("code".to_string(), format!("{:?}", output.status.code()));
    seen.insert(
        "stdout".to_string(),
        stable(&String::from_utf8_lossy(&output.stdout)),
    );
    seen.insert(
        "stderr".to_string(),
        stable(&String::from_utf8_lossy(&output.stderr)),
    );
    seen.insert("provider_calls".to_string(), provider_calls.to_string());
    let mut pending = vec![root.clone()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(directory).unwrap().flatten() {
            let path = entry.path();
            let relative = path.strip_prefix(&root).unwrap().display().to_string();
            if path.is_dir() {
                pending.push(path);
            } else if !relative.starts_with("home/") && !relative.ends_with(".pyc") {
                let bytes = std::fs::read(&path).unwrap();
                seen.insert(
                    format!("file {}", stable(&relative)),
                    stable(&String::from_utf8_lossy(&bytes)),
                );
            }
        }
    }
    seen
}

fn assert_same_observation(
    label: &str,
    left: &BTreeMap<String, String>,
    right: &BTreeMap<String, String>,
) {
    let keys = left
        .keys()
        .chain(right.keys())
        .collect::<std::collections::BTreeSet<_>>();
    for key in keys {
        assert_eq!(left.get(key), right.get(key), "{label}: {key}");
    }
}

/// `--workflow` is admitted at `src/lib.rs:139` and then by `run_workflow`, for its
/// origin and its workspace. Those admissions add nothing: a refusal is reported once,
/// by the first, with no event; an off run reaches the reproducer and shows the same
/// bytes each time. Both with a separate workspace and with the origin as the workspace.
#[test]
fn the_workflow_cli_path_is_admitted_repeatedly_with_one_refusal_and_a_stable_off_run() {
    let binary = PathBuf::from(env!("CARGO_BIN_EXE_commandagent"));
    extension();
    for workspace_is_origin in [false, true] {
        let refused = workflow_cli_observation(&binary, REFUSED, workspace_is_origin);
        assert_eq!(refused["code"], "Some(2)", "{}", refused["stderr"]);
        assert_eq!(refused["provider_calls"], "0");
        assert_eq!(refused["stdout"], "");
        assert_eq!(
            refused["stderr"].matches("profile_tool_refused").count(),
            1,
            "{}",
            refused["stderr"]
        );
        for (key, value) in &refused {
            assert!(
                !key.contains(REPRODUCER_MARKER) && !key.contains("evidence/"),
                "{key}"
            );
            if key.starts_with("file ") {
                assert!(!value.contains("profile_tool_refused"), "{key}");
            }
        }
        assert_same_observation(
            "refused",
            &refused,
            &workflow_cli_observation(&binary, REFUSED, workspace_is_origin),
        );

        // With a separate workspace the OS sandbox refuses the reproducer's write, so what
        // shows it ran there is the recorded prevalidation. In the origin it writes.
        let off = workflow_cli_observation(&binary, OFF_DRAFT, workspace_is_origin);
        assert!(
            off["file origin/evidence/workflow-events.jsonl"]
                .contains("workflow_reproducer_prevalidated"),
            "{:?}",
            off.keys().collect::<Vec<_>>()
        );
        assert_eq!(
            off.contains_key(&format!("file origin/{REPRODUCER_MARKER}")),
            workspace_is_origin,
            "{:?}",
            off.keys().collect::<Vec<_>>()
        );
        assert!(
            off.values()
                .all(|value| !value.contains("profile_tool_refused")),
            "an off run recorded a refusal"
        );
        assert_same_observation(
            "off",
            &off,
            &workflow_cli_observation(&binary, OFF_DRAFT, workspace_is_origin),
        );
    }
}

/// A selected pack can declare tools for a profile with a port, which is what brings
/// a port preflight into a slash command. The preflight runs `lsof`, here a fake that
/// leaves a marker, and only `run_command`'s own refusal comes before it.
#[cfg(unix)]
#[test]
fn a_slash_refusal_comes_before_the_port_preflight() {
    use std::os::unix::fs::PermissionsExt;
    let private = tempfile::tempdir().unwrap();
    let bin = private.path().join("bin");
    let marker = private.path().join("lsof-ran");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::write(
        bin.join("lsof"),
        format!("#!/bin/sh\n: > '{}'\nexit 1\n", marker.display()),
    )
    .unwrap();
    std::fs::set_permissions(bin.join("lsof"), std::fs::Permissions::from_mode(0o755)).unwrap();
    let mut path = std::ffi::OsString::from(&bin);
    path.push(":");
    path.push(std::env::var_os("PATH").unwrap_or_default());
    let marker_env = marker.as_os_str().to_os_string();
    if !in_own_process(
        "a_slash_refusal_comes_before_the_port_preflight",
        &[
            ("PATH", path.as_os_str()),
            ("COMMANDAGENT_TEST_972_LSOF_MARKER", marker_env.as_os_str()),
        ],
    ) {
        return;
    }
    let marker = PathBuf::from(std::env::var_os("COMMANDAGENT_TEST_972_LSOF_MARKER").unwrap());
    let _guard = lock();
    let registry = operator_registry();
    set_env(REGISTRY_ENV, registry.path().to_str());
    let packs = tempfile::tempdir().unwrap();
    let eval = "schema_version: commandagent.pack.eval/v0\npack:\n  id: nextjs-declares\n  version: 1.0.0\n  profile: nextjs\n  intent: create\nprofile_tools:\n  schema_version: profile-tool.declaration/v1\n  builtin_tools: [Read]\n  tools:\n    - id: never-registered\n      effect: read\n";
    let hash = exact_byte_hash(None, Some(eval.as_bytes()));
    std::fs::write(packs.path().join("eval.yaml"), eval).unwrap();
    std::fs::write(packs.path().join("pack.sha256"), format!("{hash}\n")).unwrap();

    // The preflight asks `lsof` who owns a port only when the port is busy.
    let busy = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = busy.local_addr().unwrap().port();
    for name in ["/plan-run", "/ultra-plan-run"] {
        let command = format!("{name} Write the notes on port {port}");
        let command = command.as_str();
        let selection = [
            packs.path().to_str().unwrap(),
            "nextjs-declares",
            "1.0.0",
            hash.as_str(),
        ];
        for (name, value) in PACK_ENVS.iter().zip(selection) {
            set_env(name, Some(value));
        }
        let fixture = fixture();
        let before = workspace_digest(&fixture.root);
        let mut planner = ScriptedClient::new(Vec::new());
        let mut execution = ScriptedClient::new(Vec::new());
        let result = handle_command(
            command,
            &config(&fixture.root, &fixture.evidence, "nextjs"),
            &mut planner,
            &mut execution,
            &NOOP_UI,
        )
        .map_err(|error| format!("{error:#}"));
        assert!(!marker.exists(), "{command}: the preflight ran lsof");
        assert_all_routes(vec![assert_refused_without_side_effects(
            command,
            &fixture,
            &before,
            planner.calls() + execution.calls(),
            result,
        )]);

        // The marker is a real witness: with no selection the same command reaches it.
        for name in PACK_ENVS {
            set_env(name, None);
        }
        let open = self::fixture();
        let mut planner = ScriptedClient::new(Vec::new());
        let mut execution = ScriptedClient::new(Vec::new());
        let _ = handle_command(
            command,
            &config(&open.root, &open.evidence, "nextjs"),
            &mut planner,
            &mut execution,
            &NOOP_UI,
        );
        assert!(marker.exists(), "{command}: lsof was not reached");
        std::fs::remove_file(&marker).unwrap();
    }
    set_env(REGISTRY_ENV, None);
}

/// `--run-plan` through the binary, against a counting provider.
fn binary_run_plan(fixture: &Fixture, registry: &Path, profile: &str) -> (Option<i32>, String) {
    let provider = CountingEndpoint::start();
    let home = tempfile::tempdir().unwrap();
    let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_commandagent"));
    command
        .arg("--extension-root")
        .arg(&extension().root)
        .args(["--ollama-host", &provider.url, "--model", "m"])
        .args(["--sandbox", "off", "--profile", profile])
        .args(["--run-plan", "verify-first.yaml"])
        .current_dir(&fixture.root)
        .env_clear()
        .env("HOME", home.path())
        .env(REGISTRY_ENV, registry);
    if let Some(path) = std::env::var_os("PATH") {
        command.env("PATH", path);
    }
    let output = command.output().unwrap();
    assert_eq!(provider.finish(), 0, "the provider was contacted");
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stderr).to_string(),
    )
}

#[test]
fn run_plan_through_the_binary_exits_2_before_the_host_runs_a_verify_command() {
    let _guard = lock();
    extension();
    let registry = operator_registry();
    let fixture = fixture();
    let before = workspace_digest(&fixture.root);
    let (code, stderr) = binary_run_plan(&fixture, registry.path(), REFUSED);
    assert_eq!(code, Some(2), "{stderr}");
    assert!(
        stderr.contains("profile_tool_refused: unknown_tool_id"),
        "{stderr}"
    );
    assert!(!fixture.root.join(MARKER).exists(), "the host ran verify");
    assert_eq!(workspace_digest(&fixture.root), before);

    // The control: the same plan through the same binary runs the host command.
    let control = self::fixture();
    let (code, stderr) = binary_run_plan(&control, registry.path(), "generic");
    assert_eq!(code, Some(0), "{stderr}");
    assert!(control.root.join(MARKER).exists(), "no witness: {stderr}");
}

// ---------------------------------------------------------------------------
// A workflow child: the artifact preflight's refusal against the profile tool's.
// ---------------------------------------------------------------------------

/// A loopback endpoint that counts connections and closes each one.
struct CountingEndpoint {
    url: String,
    count: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl CountingEndpoint {
    fn start() -> Self {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let count = Arc::new(AtomicUsize::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let (count, stop) = (count.clone(), stop.clone());
            std::thread::spawn(move || {
                while !stop.load(Ordering::SeqCst) {
                    match listener.accept() {
                        Ok(_) => {
                            count.fetch_add(1, Ordering::SeqCst);
                        }
                        Err(_) => std::thread::sleep(std::time::Duration::from_millis(2)),
                    }
                }
            })
        };
        Self {
            url,
            count,
            stop,
            thread: Some(thread),
        }
    }

    fn finish(mut self) -> usize {
        self.stop.store(true, Ordering::SeqCst);
        self.thread.take().unwrap().join().unwrap();
        self.count.load(Ordering::SeqCst)
    }
}

struct WorkflowRun {
    code: Option<i32>,
    stdout: String,
    stderr: String,
    provider_calls: usize,
    circle: Value,
    outer_event_names: Vec<String>,
    outer_status_events: Vec<Value>,
    evidence_text: String,
    run_starts: usize,
    existing_run_starts: usize,
}

fn collect_text(root: &Path) -> String {
    let mut text = String::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(directory).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if let Ok(content) = std::fs::read_to_string(&path) {
                text.push_str(&content);
                text.push('\n');
            }
        }
    }
    text
}

fn event_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(directory).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path
                .extension()
                .is_some_and(|extension| extension == "jsonl")
            {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

/// The same workflow shape for both arms; only the rebuild node's profile and the
/// registry the arm needs differ.
fn run_refused_child(
    root: &Path,
    profile: &str,
    extension: &Path,
    registry: &Path,
    origin: &Path,
) -> WorkflowRun {
    let outer = root.join("outer");
    let home = root.join("home");
    std::fs::create_dir_all(&outer).unwrap();
    std::fs::create_dir_all(&home).unwrap();
    let run_dir = origin.join(".anvil/runs/origin-run");
    std::fs::create_dir_all(&run_dir).unwrap();
    std::fs::create_dir_all(origin.join(".anvil/plans")).unwrap();
    std::fs::write(
        run_dir.join("events.jsonl"),
        concat!(
            r#"{"event":"run_start","action":"Prompt(\"origin goal\")"}"#,
            "\n",
            r#"{"event":"verify_default_bound","bound_checks":["test -f marker"]}"#,
            "\n",
            r#"{"event":"run_stop","status":"failed"}"#,
            "\n"
        ),
    )
    .unwrap();
    std::fs::write(
        origin.join(".anvil/plans/recovery-origin.yaml"),
        "version: 1\n",
    )
    .unwrap();
    let existing_run_starts = collect_text(origin).matches("\"run_start\"").count();
    let workflow = root.join("workflow.yaml");
    std::fs::write(
        &workflow,
        format!(
            "workflow: refused-child\nversion: 0.2\nentry: create\nnodes:\n  create: {{ intent: create, profile: generic }}\n  rebuild: {{ intent: create, profile: {profile} }}\nroutes:\n  - {{ from: create, on: failed, when: recovery_yaml_present, to: rebuild, carry: [workspace, recovery_yaml] }}\nterminal: {{}}\n"
        ),
    )
    .unwrap();
    let provider = CountingEndpoint::start();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_commandagent"))
        .arg("--extension-root")
        .arg(extension)
        .args(["--ollama-host", &provider.url, "--model", "m"])
        .arg("--workflow")
        .arg(&workflow)
        .arg("--origin")
        .arg(origin)
        .current_dir(&outer)
        .env_clear()
        .env("HOME", &home)
        .env(REGISTRY_ENV, registry)
        .output()
        .unwrap();
    let circle_path = origin.join("evidence/workflow-circle.json");
    let circle = std::fs::read_to_string(&circle_path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or(Value::Null);
    let mut outer_event_names = Vec::new();
    let mut outer_status_events = Vec::new();
    let workflow_events = origin.join("evidence/workflow-events.jsonl");
    for line in std::fs::read_to_string(&workflow_events)
        .unwrap_or_default()
        .lines()
    {
        let event: Value = serde_json::from_str(line).unwrap();
        let name = event["event"].as_str().unwrap_or_default().to_string();
        if name.contains("adjudicated") || name.contains("stop") {
            let mut kept = event.clone();
            if let Some(object) = kept.as_object_mut() {
                object.retain(|key, _| {
                    matches!(
                        key.as_str(),
                        "event" | "status" | "verdict" | "reason" | "reasons"
                    )
                });
            }
            outer_status_events.push(kept);
        }
        outer_event_names.push(name);
    }
    for file in event_files(&outer) {
        outer_event_names.push(format!("unexpected outer events file: {}", file.display()));
    }
    let evidence_text = collect_text(origin);
    WorkflowRun {
        code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        provider_calls: provider.finish(),
        circle,
        outer_event_names,
        outer_status_events,
        run_starts: evidence_text.matches("\"run_start\"").count(),
        evidence_text,
        existing_run_starts,
    }
}

fn artifact_child(root: &Path) -> WorkflowRun {
    const PROFILE: &str = "child-contract";
    const ADAPTER: &str = "child-adapter";
    let origin = root.join("origin");
    let registry = origin.join("registry");
    let tooling = root.join("tooling");
    let extension = root.join("extension");
    std::fs::create_dir_all(registry.join("adapters")).unwrap();
    std::fs::create_dir_all(&tooling).unwrap();
    std::fs::write(tooling.join("adapter.sh"), "exit 0\n").unwrap();
    let digest = |path: &Path| format!("{:x}", Sha256::digest(std::fs::read(path).unwrap()));
    let shell = std::fs::canonicalize("/bin/sh").unwrap();
    std::fs::write(
        registry.join(format!("adapters/{ADAPTER}.json")),
        serde_json::to_vec(&json!({
            "protocol": "artifact-contract.adapter/v1",
            "adapter_id": ADAPTER,
            "executable": {"path": shell, "sha256": digest(&shell)},
            "tooling_root": tooling,
            "entrypoint": "adapter.sh",
            "members": {"adapter.sh": digest(&tooling.join("adapter.sh"))},
            "required": [{"check_id": "lines/total", "targets": ["notes.txt#all"]}],
            "inputs": [{"input_id": "notes"}]
        }))
        .unwrap(),
    )
    .unwrap();
    let directory = extension.join(format!("profiles/{PROFILE}"));
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(
        directory.join("manifest.toml"),
        manifest(PROFILE, "").replace(
            "id = \"scaffold_files_present\"\nparams = { files = [\"notes.txt\"] }",
            &format!(
                "id = \"artifact_contract_check\"\nparams = {{ adapter_id = \"{ADAPTER}\", request = \"check/request.json\" }}"
            ),
        ),
    )
    .unwrap();
    run_refused_child(root, PROFILE, &extension, &registry, &origin)
}

fn profile_tool_child(root: &Path) -> WorkflowRun {
    const PROFILE: &str = "child-tool";
    let origin = root.join("origin");
    let registry = root.join("registry");
    let extension = root.join("extension");
    std::fs::create_dir_all(registry.join("profile-tools")).unwrap();
    write_profile(&extension, PROFILE, UNREGISTERED_DECLARATION);
    run_refused_child(root, PROFILE, &extension, &registry, &origin)
}

/// The circle evidence with the values that name the arm's own run replaced.
fn normalized_circle(run: &WorkflowRun, root: &Path) -> String {
    let text = serde_json::to_string_pretty(&run.circle).unwrap();
    let text = text
        .replace(&root.display().to_string(), "<ROOT>")
        .replace("child-contract", "<PROFILE>")
        .replace("child-tool", "<PROFILE>");
    let mut text = regex_free_id(&text);
    for (pattern, replacement) in [
        (r#""epoch": [0-9]+"#, r#""epoch": "<N>""#),
        (r"elapsed_ms: [0-9]+", "elapsed_ms: <N>"),
    ] {
        text = regex::Regex::new(pattern)
            .unwrap()
            .replace_all(&text, replacement)
            .into_owned();
    }
    text
}

#[test]
fn a_refused_workflow_child_matches_the_artifact_preflight_and_claims_no_success() {
    let artifact_dir = tempfile::tempdir().unwrap();
    let artifact_root = std::fs::canonicalize(artifact_dir.path()).unwrap();
    let artifact = artifact_child(&artifact_root);
    let tool_dir = tempfile::tempdir().unwrap();
    let tool_root = std::fs::canonicalize(tool_dir.path()).unwrap();
    let tool = profile_tool_child(&tool_root);

    assert!(
        artifact
            .evidence_text
            .contains("resolves inside the workspace"),
        "the control was not refused by the artifact preflight: {}\n{}",
        artifact.stderr,
        artifact.evidence_text
    );
    assert!(
        tool.evidence_text
            .contains("profile_tool_refused: unknown_tool_id"),
        "{}\n{}",
        tool.stderr,
        tool.evidence_text
    );
    for (label, run) in [("artifact", &artifact), ("tool", &tool)] {
        assert_eq!(run.provider_calls, 0, "{label}: {}", run.stderr);
        assert_eq!(
            run.run_starts, run.existing_run_starts,
            "{label}: the refused child recorded a run_start"
        );
        assert!(
            run.evidence_text.contains("node_failed:rebuild"),
            "{label}: {}",
            run.evidence_text
        );
        assert!(
            run.evidence_text.contains("circle_failed"),
            "{label}: {}",
            run.evidence_text
        );
        let circle = serde_json::to_string(&run.circle).unwrap();
        assert!(circle.contains("circle_failed"), "{label}: {circle}");
        assert!(!circle.contains("circle_closed"), "{label}: {circle}");
        assert!(!circle.contains("\"full\""), "{label}: {circle}");
        assert!(
            !run.outer_status_events
                .iter()
                .any(|event| event["status"] == "full" || event["status"] == "completed"),
            "{label}: the outer run claimed success: {:?}",
            run.outer_status_events
        );
    }
    assert_eq!(
        artifact.code, tool.code,
        "{}\n{}",
        artifact.stderr, tool.stderr
    );
    assert!(!tool.outer_event_names.is_empty(), "no workflow events");
    assert!(
        tool.outer_status_events
            .iter()
            .any(|event| event.to_string().contains("circle_failed")),
        "{:?}",
        tool.outer_status_events
    );
    assert_eq!(artifact.outer_event_names, tool.outer_event_names);
    assert_eq!(artifact.outer_status_events, tool.outer_status_events);
    assert_eq!(
        normalized_circle(&artifact, &artifact_root),
        normalized_circle(&tool, &tool_root)
    );
    assert_eq!(artifact.stdout, tool.stdout);
    println!(
        "outer exit: artifact={:?} tool={:?}\nouter events: {:?}\nouter status: {:?}\ncircle: {}",
        artifact.code,
        tool.code,
        tool.outer_event_names,
        tool.outer_status_events,
        normalized_circle(&tool, &tool_root)
    );
}

// ---------------------------------------------------------------------------
// The one extra read of an inherited, undecoded pack: byte-identical off.
// ---------------------------------------------------------------------------

/// The exact keys the base recording's comparison normalizes; nothing is left out.
const NORMALIZED_NUMBER_FIELDS: [&str; 5] = [
    "occurred_at_epoch_ms",
    "duration_ms",
    "elapsed_ms",
    "estimated_prompt_tokens_sent",
    "estimated_stable_prefix_chars",
];

fn normalize_text(root: &Path, text: &str) -> String {
    let mut text = text.replace(&root.display().to_string(), "<ROOT>");
    for (pattern, replacement) in [
        (r"elapsed_ms: [0-9]+", "elapsed_ms: <N>"),
        (r"total_duration_ms=[0-9]+", "total_duration_ms=<N>"),
        (
            r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}",
            "<ID>",
        ),
    ] {
        text = regex::Regex::new(pattern)
            .unwrap()
            .replace_all(&text, replacement)
            .into_owned();
    }
    text
}

fn normalize_event(root: &Path, value: &Value) -> Value {
    match value {
        Value::Object(object) => Value::Object(
            object
                .iter()
                .map(|(key, value)| {
                    let value =
                        if NORMALIZED_NUMBER_FIELDS.contains(&key.as_str()) && value.is_number() {
                            json!("<N>")
                        } else {
                            normalize_event(root, value)
                        };
                    (key.clone(), value)
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|item| normalize_event(root, item))
                .collect(),
        ),
        Value::String(text) => Value::String(normalize_text(root, text)),
        other => other.clone(),
    }
}

fn reply(name: &str, arguments: Value) -> AssistantReply {
    AssistantReply {
        content: String::new(),
        tool_calls: vec![ToolCall {
            id: "call-0".to_string(),
            name: name.to_string(),
            arguments,
        }],
        prompt_tokens: None,
        completion_tokens: None,
    }
}

fn record_undeclared_pack_row() -> Value {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let mut client = ScriptedClient::new(vec![
        reply("Write", json!({"path": "notes.txt", "content": "v1\n"})),
        reply("Read", json!({"path": "notes.txt"})),
        reply("Bash", json!({"command": "test -f notes.txt"})),
        AssistantReply::text("notes.txt says v1."),
    ]);
    let config = config(&root, &root, OFF_DRAFT);
    let outcome = match run_session_with_outcome_with_ui(
        &mut client,
        &mut SessionSnapshot::new(),
        "Write notes.txt and check it.",
        &[],
        &config,
        &NOOP_UI,
    ) {
        Ok(outcome) => json!({
            "stop_reason": format!("{:?}", outcome.stop_reason),
            "final_text": outcome.final_text,
            "iterations": outcome.iterations,
        }),
        Err(error) => json!({"error": normalize_text(&root, &format!("{error:#}"))}),
    };
    let requests = client
        .requests
        .lock()
        .unwrap()
        .iter()
        .map(|request| {
            json!({
                "messages": normalize_text(&root, request["messages"].as_str().unwrap()),
                "tools": request["tools"],
                "native": request["native"],
            })
        })
        .collect::<Vec<_>>();
    let recorded = std::fs::read_to_string(root.join("events.jsonl"))
        .unwrap_or_default()
        .lines()
        .map(|line| {
            let event: Value = serde_json::from_str(line).unwrap();
            serde_json::to_string(&normalize_event(&root, &event)).unwrap()
        })
        .collect::<Vec<_>>();
    let mut artifacts = std::fs::read_dir(&root)
        .unwrap()
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .filter(|entry| entry.file_name() != "events.jsonl")
        .map(|entry| {
            (
                entry.file_name().to_string_lossy().to_string(),
                std::fs::read_to_string(entry.path()).unwrap(),
            )
        })
        .collect::<Vec<_>>();
    artifacts.sort();
    json!({
        "id": "undeclared-draft-selected-pack",
        "outcome": outcome,
        "requests": requests,
        "events": recorded,
        "artifacts": artifacts,
        "unused_replies": client.replies.lock().unwrap().len(),
    })
}

/// No test in this binary decodes the pack before this one does, so the first run
/// below reaches the read admission makes to learn whether the pack declares tools.
#[test]
fn an_inherited_pack_this_process_never_decoded_stays_byte_identical_off() {
    let _guard = lock();
    set_env(REGISTRY_ENV, None);
    let extension = extension();
    let golden: Value = serde_json::from_str(OFF_GOLDEN).unwrap();
    assert_eq!(golden["source"], "104a166b");
    let expected = golden["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == "undeclared-draft-selected-pack")
        .unwrap()
        .clone();
    set_env(PACK_ENVS[0], extension.pack_directory.to_str());
    set_env(PACK_ENVS[1], Some(OFF_PACK));
    set_env(PACK_ENVS[2], Some("1.0.0"));
    set_env(PACK_ENVS[3], Some(&extension.pack_hash));

    assert_eq!(
        serde_json::to_string_pretty(&record_undeclared_pack_row()).unwrap(),
        serde_json::to_string_pretty(&expected).unwrap(),
        "the first, reading run"
    );
    assert_eq!(
        serde_json::to_string_pretty(&record_undeclared_pack_row()).unwrap(),
        serde_json::to_string_pretty(&expected).unwrap(),
        "a later run, which no longer reads"
    );
    for name in PACK_ENVS {
        set_env(name, None);
    }
}
