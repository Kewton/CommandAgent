//! Issue #879: a Verify step that repeats a declared `anvil-nextjs-check:` token gets the
//! host's Rust judgement, never a shell run, and the #871 refusals stay as they were.
use std::path::Path;
use std::sync::{Arc, Mutex};

use clap::Parser;
use serde_json::{Value, json};

use crate::cli::Cli;
use crate::config::Config;
use crate::minimal_loop::dependency_setup::NodeDependencySetupAuthority;
use crate::minimal_loop::evidence::command_diagnosis;
use crate::minimal_loop::loop_run::{
    RunSessionOptions, RunSessionStepKind, RunStopReason, run_session_with_outcome_with_options,
};
use crate::planner::profile_descriptor::NEXTJS_PROFILE_ID;
use crate::providers::{AssistantReply, ChatClient};
use crate::state::{ConversationMessage, SessionSnapshot, ToolCall};
use crate::tui::NOOP_UI;

const DEV: &str = "anvil-nextjs-check:package_script_port:dev:3011";
const START: &str = "anvil-nextjs-check:package_script_port:start:3011";
const BUILD: &str = "anvil-nextjs-check:build_script";
const GOOD_PACKAGE: &str =
    r#"{"scripts":{"dev":"next dev -p 3011","start":"next start -p 3011","build":"next build"}}"#;

/// Replays a fixed list of replies; it reads no clock, so the run is deterministic.
#[derive(Clone)]
struct Scripted {
    replies: Arc<Mutex<Vec<AssistantReply>>>,
}

impl Scripted {
    fn calling(commands: &[String]) -> Self {
        let mut replies = commands
            .iter()
            .map(|command| AssistantReply {
                content: String::new(),
                tool_calls: vec![ToolCall::new("Bash", json!({ "command": command }))],
                prompt_tokens: None,
                completion_tokens: None,
            })
            .collect::<Vec<_>>();
        replies.push(AssistantReply::text("The declared checks passed."));
        replies.reverse();
        Self {
            replies: Arc::new(Mutex::new(replies)),
        }
    }
}

impl ChatClient for Scripted {
    fn label(&self) -> &str {
        "scripted"
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
        _tools: &[crate::tools::registry::ToolSpec],
        _native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        self.replies
            .lock()
            .unwrap()
            .pop()
            .ok_or_else(|| anyhow::anyhow!("the scripted client ran out of replies"))
    }
}

fn config(root: &Path, profile: &str) -> Config {
    let mut config = Config::from_cli(Cli::parse_from(["commandagent", "--ux-demo"])).unwrap();
    config.workspace_root = root.to_path_buf();
    config.profile = profile.to_string();
    config.yes = true;
    config.max_iterations = 40;
    config.eval_events_path = Some(root.join(".anvil/runs/test/events.jsonl"));
    config
}

fn workspace(package_json: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("package.json"), package_json).unwrap();
    dir
}

fn events(root: &Path) -> Vec<Value> {
    std::fs::read_to_string(root.join(".anvil/runs/test/events.jsonl"))
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

struct Run {
    stop: RunStopReason,
    bash_results: Vec<String>,
    events: Vec<Value>,
}

fn run_verify(root: &Path, profile: &str, commands: &[String]) -> anyhow::Result<Run> {
    run_step(root, profile, RunSessionStepKind::Verify, commands)
}

fn run_step(
    root: &Path,
    profile: &str,
    step: RunSessionStepKind,
    commands: &[String],
) -> anyhow::Result<Run> {
    run_configured(root, profile, step, commands, false)
}

fn run_configured(
    root: &Path,
    profile: &str,
    step: RunSessionStepKind,
    commands: &[String],
    offline: bool,
) -> anyhow::Result<Run> {
    let mut config = config(root, profile);
    config.offline = offline;
    let mut client = Scripted::calling(commands);
    let mut session = SessionSnapshot::new();
    let outcome = run_session_with_outcome_with_options(
        &mut client,
        &mut session,
        "Verify the current step.",
        &[],
        &config,
        &NOOP_UI,
        RunSessionOptions::plan_step(step),
    )?;
    Ok(Run {
        stop: outcome.stop_reason,
        bash_results: session
            .messages
            .iter()
            .filter(|message| message.role == "tool" && message.name.as_deref() == Some("Bash"))
            .map(|message| message.content.clone())
            .collect(),
        events: events(root),
    })
}

/// The verify commands the host itself declares for the scaffold phase's
/// `nextjs-profile-verify` step, minus `npm run build` (it needs installed
/// dependencies and has no host judgement). These are what a Verify step repeats.
fn declared_verify_commands(root: &Path) -> Vec<String> {
    let prompt = "Phase id: project-setup\nPhase task: Create the Next.js project";
    let template = crate::planner::profiles::nextjs::deterministic_step_plan(
        prompt,
        root,
        "Build a Next.js game served on port 3011",
    )
    .expect("the project-setup phase selects the scaffold template");
    let step = template
        .plan
        .steps
        .iter()
        .find(|step| step.id == "nextjs-profile-verify")
        .expect("the scaffold template declares nextjs-profile-verify");
    assert!(step.verify.iter().any(|command| command == "npm run build"));
    step.verify
        .iter()
        .filter(|command| command.as_str() != "npm run build")
        .cloned()
        .collect()
}

/// The shapes of the G executor: one declared verify at a time, and all of them
/// joined, each typed three times.
fn declared_shapes(root: &Path) -> Vec<(String, Vec<String>)> {
    let declared = declared_verify_commands(root);
    assert_eq!(declared, [DEV, START, BUILD], "{declared:?}");
    let repeat = |text: String| vec![text; 3];
    let mut shapes = declared
        .iter()
        .map(|command| (format!("one by one: {command}"), repeat(command.clone())))
        .collect::<Vec<_>>();
    for (name, separator) in [("newline", "\n"), ("&&", " && "), (";", "; ")] {
        shapes.push((format!("{name} joined"), repeat(declared.join(separator))));
    }
    shapes
}

#[test]
fn issue879_g_shaped_repeated_declared_verifies_are_judged_by_the_host() {
    for (shape, commands) in declared_shapes(workspace(GOOD_PACKAGE).path()) {
        // A fresh workspace per shape keeps each run's events apart.
        let root = workspace(GOOD_PACKAGE);
        let run = run_verify(root.path(), NEXTJS_PROFILE_ID, &commands)
            .unwrap_or_else(|error| panic!("{shape}: {error}"));
        assert_eq!(run.stop, RunStopReason::AssistantFinal, "{shape}");
        // A passing declared check ends the Verify step, so the repeats a failing run
        // needs are never typed: one admitted, host-judged result is enough.
        assert!(!run.bash_results.is_empty(), "{shape}");
        for result in &run.bash_results {
            assert!(
                result.contains("combined_outcome: Success"),
                "{shape}: {result}"
            );
            assert!(result.contains("host check passed"), "{shape}: {result}");
            assert!(!result.contains("command not found"), "{shape}: {result}");
        }
        let policies = run
            .events
            .iter()
            .filter(|event| event["event"] == "runtime_bash_policy")
            .collect::<Vec<_>>();
        assert_eq!(policies.len(), run.bash_results.len(), "{shape}");
        for policy in policies {
            assert_eq!(policy["blocked"], false, "{shape}: {policy}");
            assert_eq!(policy["policy_error_kind"], "", "{shape}: {policy}");
        }
        assert!(
            !run.events
                .iter()
                .any(|event| event["event"] == "tool_policy_error"),
            "{shape}"
        );
    }
}

#[test]
fn issue879_a_failing_host_check_is_returned_as_a_failure_not_an_error() {
    let root = workspace(r#"{"scripts":{"dev":"next dev -p 3000","build":"next build"}}"#);
    for (shape, commands) in declared_shapes(root.path()) {
        let run = run_verify(root.path(), NEXTJS_PROFILE_ID, &commands);
        // Never `recoverable tool error repeated: verify_command_policy_error`.
        let run = run.unwrap_or_else(|error| panic!("{shape}: {error}"));
        assert!(!run.bash_results.is_empty(), "{shape}");
        for result in &run.bash_results {
            // Only the dev check is broken here; `start` is optional and `build` is fine.
            let broken = shape.contains("dev") || shape.contains("joined");
            let outcome = if broken { "CommandFailed" } else { "Success" };
            assert!(
                result.contains(&format!("combined_outcome: {outcome}")),
                "{shape}: {result}"
            );
        }
    }
}

fn only_result(run: &Run, id: &str) -> String {
    assert_eq!(run.bash_results.len(), 1, "{id}: {:?}", run.bash_results);
    run.bash_results[0].clone()
}

#[test]
fn issue879_a_lone_token_takes_the_segment_route_like_a_joined_one() {
    for step in [RunSessionStepKind::Verify, RunSessionStepKind::Report] {
        for command in [DEV, START, BUILD] {
            let root = workspace(GOOD_PACKAGE);
            let run = run_step(root.path(), NEXTJS_PROFILE_ID, step, &[command.to_string()])
                .unwrap_or_else(|error| panic!("{step:?} {command}: {error}"));
            let result = only_result(&run, command);
            assert!(result.contains("combined_outcome: Success"), "{result}");
            assert!(result.contains("segment 1 host check:"), "{result}");
            // No process ran, so no shell outcome carries a duration.
            assert!(!result.contains("elapsed_ms"), "{result}");
        }
        let root = workspace(GOOD_PACKAGE);
        let joined = [DEV, START, BUILD].join("\n");
        let run = run_step(root.path(), NEXTJS_PROFILE_ID, step, &[joined]).unwrap();
        let result = only_result(&run, "joined");
        for number in 1..=3 {
            assert!(
                result.contains(&format!("segment {number} host check:")),
                "{result}"
            );
        }
    }
}

#[test]
fn issue879_the_segment_executor_never_runs_a_token_in_a_shell() {
    let both = format!("{DEV} && {BUILD}");
    let root = workspace(GOOD_PACKAGE);
    let result = only_result(
        &run_verify(root.path(), NEXTJS_PROFILE_ID, std::slice::from_ref(&both)).unwrap(),
        "good",
    );
    assert!(result.starts_with("combined_outcome: Success"), "{result}");
    assert!(result.contains("segment 2 host check:"), "{result}");
    assert!(!result.contains("elapsed_ms"), "no process ran: {result}");

    // A failing first segment short-circuits the second, as for any `&&` chain.
    let root = workspace(r#"{"scripts":{"dev":"next dev","build":"next build"}}"#);
    let result = only_result(
        &run_verify(root.path(), NEXTJS_PROFILE_ID, &[both]).unwrap(),
        "bad",
    );
    assert!(
        result.starts_with("combined_outcome: CommandFailed"),
        "{result}"
    );
    assert!(result.contains("failing_segment: 1"), "{result}");
    assert!(
        result.contains("segment 2 skipped by && short-circuit"),
        "{result}"
    );
}

#[test]
fn issue879_an_altered_token_fails_and_is_not_run_in_a_shell() {
    let root = workspace(GOOD_PACKAGE);
    let marker = root.path().join("ran-in-shell");
    let touched = format!("{DEV} && touch {}", marker.display());
    let run = run_verify(root.path(), NEXTJS_PROFILE_ID, &[touched]).unwrap();
    assert!(!marker.exists());
    assert!(
        run.events
            .iter()
            .any(|event| event["event"] == "runtime_bash_policy" && event["blocked"] == true),
        "{:?}",
        run.events
    );
    for altered in [
        "anvil-nextjs-check:",
        "anvil-nextjs-check:unknown_check",
        "anvil-nextjs-check:build_script --lint",
        "anvil-nextjs-check:package_script_port:dev",
        "anvil-nextjs-check:package_script_port:dev:3011x",
        "anvil-nextjs-check:package_script_port:dev:03011",
    ] {
        let run = run_verify(root.path(), NEXTJS_PROFILE_ID, &[altered.to_string()]).unwrap();
        let result = only_result(&run, altered);
        assert!(
            result.contains("combined_outcome: CommandFailed"),
            "{altered}: {result}"
        );
        assert!(result.contains("malformed"), "{altered}: {result}");
    }
}

#[test]
fn issue879_a_nextjs_token_fails_outside_the_nextjs_profile() {
    let root = workspace(GOOD_PACKAGE);
    for profile in ["generic", "python-cli", "data", "ingest"] {
        let run = run_verify(root.path(), profile, &[DEV.to_string()]).unwrap();
        let result = &run.bash_results[0];
        assert!(
            result.contains("combined_outcome: CommandFailed"),
            "{profile}: {result}"
        );
        assert!(
            result.contains("outside the active nextjs profile"),
            "{profile}: {result}"
        );
    }
}

#[test]
fn issue879_model_written_inline_interpreters_stay_refused_even_when_they_look_declared() {
    let former = [
        "node -p \"String(require('./package.json').scripts.build)=='next build' ? true : process.exit(1)\"",
        "node -p \"['dev'].every(function(k){return String(Object(require('./package.json').scripts)[k]).split(' ').some(function(t,i,a){return t=='next' ? a.slice(i+1).find(function(x){return x})==k : false}) ? true : false}) ? true : process.exit(1)\"",
        "node -e \"require('./package.json').scripts.build=='next build' || process.exit(1)\"",
        "node -p '1'",
        "node -e 'process.exit(0)'",
    ];
    for step in [RunSessionStepKind::Verify, RunSessionStepKind::Report] {
        for command in former {
            let root = workspace(GOOD_PACKAGE);
            let run = run_step(root.path(), NEXTJS_PROFILE_ID, step, &[command.to_string()])
                .unwrap_or_else(|error| panic!("{step:?} {command}: {error}"));
            let policy = run
                .events
                .iter()
                .find(|event| event["event"] == "runtime_bash_policy")
                .unwrap_or_else(|| panic!("{step:?} {command}: {:?}", run.events));
            assert_eq!(policy["blocked"], true, "{step:?} {command}");
            assert_eq!(
                policy["deterministic_verifier_evidence"], false,
                "{command}"
            );
            assert_eq!(
                policy["policy_error_kind"], "verify_command_policy_error",
                "{command}"
            );
            assert!(
                run.bash_results
                    .iter()
                    .all(|result| !result.contains("outcome: Success")),
                "{command}: {:?}",
                run.bash_results
            );
        }
    }
}

#[test]
fn issue879_a_token_result_counts_as_the_same_static_evidence_as_the_node_check() {
    let root = tempfile::tempdir().unwrap();
    let commands = [DEV, START, BUILD]
        .map(str::to_string)
        .into_iter()
        .collect::<Vec<_>>();
    let diagnoses = command_diagnosis::collect(root.path(), &commands);
    for diagnosis in &diagnoses {
        assert_eq!(diagnosis.kind, "static_syntax", "{diagnosis:?}");
        assert_eq!(diagnosis.reason, None, "{diagnosis:?}");
    }
    assert_eq!(command_diagnosis::immutable_failure(&diagnoses), None);
    let altered = command_diagnosis::collect(
        root.path(),
        &["anvil-nextjs-check:package_script_port:dev:3011x".to_string()],
    );
    assert_eq!(altered[0].kind, "other", "{altered:?}");
}

#[test]
fn issue879_the_same_checks_typed_as_inline_node_are_still_refused_three_times_over() {
    // The G failure: a Verify step that types an inline `node -p` is refused, and the
    // third refusal ends the loop. The refusal is #871's and must not be exempted for
    // a text that merely looks like what the host used to declare.
    let root = workspace(GOOD_PACKAGE);
    let inline = "node -p \"String(require('./package.json').scripts.build)=='next build' ? true : process.exit(1)\"";
    let error = run_verify(root.path(), NEXTJS_PROFILE_ID, &vec![inline.to_string(); 3])
        .err()
        .expect("an inline node check typed three times must stop the loop")
        .to_string();
    assert!(
        error.contains("recoverable tool error repeated: verify_command_policy_error"),
        "{error}"
    );
}

#[test]
fn issue879_a_host_check_segment_is_not_counted_as_a_dependency_install() {
    // The loop routes a lone segment to the segment executor through
    // `install_family()`, but nothing of an install follows: no offline refusal, no
    // dependency setup, no `verify_install_substituted` event, and the telemetry names
    // the token as an ordinary normalized segment.
    let token = DEV.to_string();
    let mixed = format!("{DEV} && npm install");
    for offline in [false, true] {
        let root = workspace(GOOD_PACKAGE);
        let run = run_configured(
            root.path(),
            NEXTJS_PROFILE_ID,
            RunSessionStepKind::Verify,
            std::slice::from_ref(&token),
            offline,
        )
        .unwrap_or_else(|error| panic!("offline={offline}: {error}"));
        let result = only_result(&run, "offline");
        assert!(result.contains("combined_outcome: Success"), "{result}");
        assert!(!result.contains("install substituted"), "{result}");
        assert!(!result.contains("setup_status"), "{result}");
        for event in &run.events {
            let name = event["event"].as_str().unwrap_or_default();
            assert!(
                !name.contains("install") && !name.contains("dependency_setup"),
                "offline={offline}: {event}"
            );
        }
        let policy = run
            .events
            .iter()
            .find(|event| event["event"] == "runtime_bash_policy")
            .unwrap();
        assert_eq!(policy["normalized_commands"], serde_json::json!([DEV]));
        assert!(!root.path().join("node_modules").exists());
    }
    // Next to a real install segment, only that segment is an install substitution.
    let root = workspace(GOOD_PACKAGE);
    let run = run_configured(
        root.path(),
        NEXTJS_PROFILE_ID,
        RunSessionStepKind::Verify,
        &[mixed],
        true,
    )
    .unwrap();
    let result = only_result(&run, "mixed");
    assert!(result.contains("segment 1 host check:"), "{result}");
    assert!(
        result.contains("segment 2 install substituted:"),
        "{result}"
    );
    assert_eq!(
        run.events
            .iter()
            .filter(|event| event["event"] == "verify_install_substituted")
            .count(),
        1,
        "{:?}",
        run.events
    );
}

#[test]
fn issue879_corpus_shapes_pass_with_no_refusal_and_inline_node_is_refused() {
    let commands: Value = serde_json::from_str(include_str!(
        "../../../tests/corpus/apps/nextjs-verify-host-check-repeated/model-commands.json"
    ))
    .unwrap();
    let expected: Value = serde_json::from_str(include_str!(
        "../../../tests/corpus/apps/nextjs-verify-host-check-repeated/expected.json"
    ))
    .unwrap();
    let times = commands["typed_times"].as_u64().unwrap() as usize;
    let strings = |value: &Value| -> Vec<String> {
        value
            .as_array()
            .unwrap()
            .iter()
            .map(|command| command.as_str().unwrap().to_string())
            .collect()
    };
    let mut shapes: Vec<(String, Vec<String>)> = Vec::new();
    for command in strings(&commands["one_by_one"]) {
        shapes.push((command.clone(), vec![command; times]));
    }
    for key in ["newline", "and_and", "semicolon"] {
        let command = commands[key].as_str().unwrap().to_string();
        shapes.push((key.to_string(), vec![command; times]));
    }
    for (shape, typed) in shapes {
        let root = workspace(GOOD_PACKAGE);
        let run = run_verify(root.path(), NEXTJS_PROFILE_ID, &typed)
            .unwrap_or_else(|error| panic!("{shape}: {error}"));
        assert_eq!(expected["declared_shapes"]["refusals"], 0);
        assert!(
            !run.events.iter().any(|event| event["blocked"] == true),
            "{shape}: {:?}",
            run.events
        );
        for needle in expected["declared_shapes"]["result_contains"]
            .as_array()
            .unwrap()
        {
            let needle = needle.as_str().unwrap();
            assert!(
                run.bash_results
                    .iter()
                    .all(|result| result.contains(needle)),
                "{shape}: {needle}"
            );
        }
    }
    let refused = expected["inline_node_refused"]["refusals"]
        .as_u64()
        .unwrap() as usize;
    for inline in strings(&commands["inline_node_refused"]) {
        let root = workspace(GOOD_PACKAGE);
        let error = run_verify(root.path(), NEXTJS_PROFILE_ID, &vec![inline; refused])
            .err()
            .expect("an inline node check typed repeatedly must stop the loop")
            .to_string();
        assert!(
            error.contains(expected["inline_node_refused"]["error"].as_str().unwrap()),
            "{error}"
        );
    }
}

#[test]
fn issue879_a_repaired_package_port_check_is_bound_to_the_nextjs_profile() {
    // A grep on package.json is repaired into the dev port host check token; the host
    // judges it in Rust, and only in the nextjs profile.
    let grep = r#"grep -q '"3011"' package.json"#;
    let step = crate::planner::step_plan::PlanStep {
        id: "verify-port".to_string(),
        kind: "verify".to_string(),
        expected_result: "pass".to_string(),
        instruction: "Check the package.json port".to_string(),
        expected_paths: Vec::new(),
        verify: vec![grep.to_string()],
    };
    let verify = |profile: Option<&str>| {
        let root = workspace(r#"{"scripts":{"dev":"next dev -p 3011","build":"next build"}}"#);
        crate::planner::verify::verify_step_with_profile_setup_observed_with_offline_and_events(
            root.path(),
            &step,
            profile,
            NodeDependencySetupAuthority::None,
            false,
            None,
        )
        .0
    };
    assert!(verify(Some(NEXTJS_PROFILE_ID)).is_pass());
    for profile in [None, Some("python-cli"), Some("data")] {
        let report = verify(profile);
        assert!(!report.is_pass(), "{profile:?}: {report:?}");
        assert!(
            report
                .command_failures
                .iter()
                .any(|failure| failure.reason.contains("outside the active nextjs profile")),
            "{profile:?}: {report:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// Issue #881: the hook, includes and source checks the host rewrites a grep into.
// ---------------------------------------------------------------------------

use crate::planner::profiles::nextjs::source_checks::{
    Check, hook_action_command, hook_state_command, legacy_inline_command, script_includes_command,
    source_keydown_command, source_token_command,
};

const PAGE_SOURCE: &str = "'use client';\nimport { useState } from 'react';\nexport default function Page() {\n  const [count, setCount] = useState(0);\n  return (\n    <main data-anvil-state={count}>\n      <button data-anvil-action=\"primary\" onKeyDown={() => setCount(count + 1)}>Go</button>\n    </main>\n  );\n}\n";

/// The greps of a contract-wiring step: a hook attribute, the hook state, a package
/// script, and two source assertions.
const PLANNER_GREPS: [&str; 5] = [
    "grep -q 'data-anvil-action=\"primary\"' src/app/page.tsx",
    "grep -q 'data-anvil-state' src/app/page.tsx",
    "grep -q \"next dev\" package.json",
    "grep -q \"useState\" src/app/page.tsx",
    "grep -q 'onKeyDown' src/app/page.tsx",
];

fn wired_workspace() -> tempfile::TempDir {
    let dir = workspace(GOOD_PACKAGE);
    std::fs::create_dir_all(dir.path().join("src/app")).unwrap();
    std::fs::write(dir.path().join("src/app/page.tsx"), PAGE_SOURCE).unwrap();
    dir
}

/// What the sanitizer writes into the plan for each planner grep.
fn rewritten_tokens(root: &Path) -> Vec<String> {
    PLANNER_GREPS
        .iter()
        .map(|grep| {
            crate::planner::verify::normalize_verify_command_for_oracle_repair_with_root(grep, root)
                .unwrap_or_else(|| panic!("{grep} is not rewritten"))
                .normalized
        })
        .collect()
}

/// One typed command at a time, joined by a newline, `&&` or `;`, each typed three times.
fn typed_shapes(commands: &[String]) -> Vec<(String, Vec<String>)> {
    let repeat = |text: String| vec![text; 3];
    let mut shapes = commands
        .iter()
        .map(|command| (format!("one by one: {command}"), repeat(command.clone())))
        .collect::<Vec<_>>();
    for (name, separator) in [("newline", "\n"), ("&&", " && "), (";", "; ")] {
        shapes.push((format!("{name} joined"), repeat(commands.join(separator))));
    }
    shapes
}

fn assert_host_judged(shape: &str, run: &Run) {
    assert_eq!(run.stop, RunStopReason::AssistantFinal, "{shape}");
    assert!(!run.bash_results.is_empty(), "{shape}");
    for result in &run.bash_results {
        assert!(
            result.contains("combined_outcome: Success"),
            "{shape}: {result}"
        );
        assert!(result.contains("host check passed"), "{shape}: {result}");
        assert!(!result.contains("command not found"), "{shape}: {result}");
    }
    for policy in run
        .events
        .iter()
        .filter(|event| event["event"] == "runtime_bash_policy")
    {
        assert_eq!(policy["blocked"], false, "{shape}: {policy}");
        assert_eq!(policy["policy_error_kind"], "", "{shape}: {policy}");
    }
    assert!(
        !run.events
            .iter()
            .any(|event| event["event"] == "tool_policy_error"),
        "{shape}"
    );
}

#[test]
fn issue881_rewritten_greps_typed_by_a_verify_step_are_judged_by_the_host() {
    let tokens = rewritten_tokens(wired_workspace().path());
    assert!(
        tokens
            .iter()
            .all(|token| token.starts_with("anvil-nextjs-check:"))
    );
    for (shape, commands) in typed_shapes(&tokens) {
        // A fresh workspace per shape keeps each run's events apart.
        let root = wired_workspace();
        let run = run_verify(root.path(), NEXTJS_PROFILE_ID, &commands)
            .unwrap_or_else(|error| panic!("{shape}: {error}"));
        assert_host_judged(&shape, &run);
    }
}

#[test]
fn issue881_the_model_typing_the_original_grep_gets_the_same_host_judgement() {
    // The runtime rewrites the grep into the token, so the segment is host-judged too.
    let greps = PLANNER_GREPS.map(str::to_string);
    for (shape, commands) in typed_shapes(&greps) {
        let root = wired_workspace();
        let run = run_verify(root.path(), NEXTJS_PROFILE_ID, &commands)
            .unwrap_or_else(|error| panic!("{shape}: {error}"));
        assert_host_judged(&shape, &run);
    }
}

#[test]
fn issue881_a_failing_rewritten_check_is_a_failure_not_a_policy_error() {
    let root = workspace(GOOD_PACKAGE);
    std::fs::create_dir_all(root.path().join("src/app")).unwrap();
    std::fs::write(
        root.path().join("src/app/page.tsx"),
        "export default () => null;\n",
    )
    .unwrap();
    let tokens = rewritten_tokens(wired_workspace().path());
    for token in &tokens {
        let run = run_verify(root.path(), NEXTJS_PROFILE_ID, std::slice::from_ref(token)).unwrap();
        let result = only_result(&run, token);
        let package_check = token.contains("script_includes");
        let outcome = if package_check {
            "Success"
        } else {
            "CommandFailed"
        };
        assert!(
            result.contains(&format!("combined_outcome: {outcome}")),
            "{token}: {result}"
        );
    }
}

#[test]
fn issue881_the_source_checks_are_not_bound_to_the_nextjs_profile() {
    let tokens = rewritten_tokens(wired_workspace().path());
    for profile in ["generic", "python-cli", "data", "ingest", NEXTJS_PROFILE_ID] {
        for token in &tokens {
            let root = wired_workspace();
            let run = run_verify(root.path(), profile, std::slice::from_ref(token))
                .unwrap_or_else(|error| panic!("{profile} {token}: {error}"));
            let result = only_result(&run, token);
            assert!(
                result.contains("combined_outcome: Success"),
                "{profile} {token}: {result}"
            );
            assert!(
                !result.contains("outside the active nextjs profile"),
                "{profile}: {result}"
            );
        }
    }
    // The package script tokens of Issue #879 stay bound to the profile.
    let root = wired_workspace();
    let run = run_verify(root.path(), "generic", &[DEV.to_string()]).unwrap();
    assert!(only_result(&run, DEV).contains("outside the active nextjs profile"));
}

#[test]
fn issue881_inline_text_equal_to_the_former_rewrite_stays_refused() {
    let former = [
        Check::HookAction {
            value: "primary".to_string(),
            path: "src/app/page.tsx".to_string(),
        },
        Check::HookState {
            path: "src/app/page.tsx".to_string(),
        },
        Check::ScriptIncludes {
            script: crate::planner::profiles::nextjs::source_checks::Script::Dev,
            pattern: "next dev".to_string(),
        },
        Check::SourceKeydown {
            path: "src/app/page.tsx".to_string(),
        },
        Check::SourceToken {
            token: "useState".to_string(),
            path: "src/app/page.tsx".to_string(),
        },
    ]
    .iter()
    .map(legacy_inline_command)
    .collect::<Vec<_>>();
    for step in [RunSessionStepKind::Verify, RunSessionStepKind::Report] {
        for command in &former {
            let root = wired_workspace();
            let run = run_step(
                root.path(),
                NEXTJS_PROFILE_ID,
                step,
                std::slice::from_ref(command),
            )
            .unwrap_or_else(|error| panic!("{step:?} {command}: {error}"));
            let policy = run
                .events
                .iter()
                .find(|event| event["event"] == "runtime_bash_policy")
                .unwrap_or_else(|| panic!("{step:?} {command}: {:?}", run.events));
            assert_eq!(policy["blocked"], true, "{step:?} {command}");
            assert_eq!(
                policy["policy_error_kind"], "verify_command_policy_error",
                "{command}"
            );
            assert!(
                run.bash_results
                    .iter()
                    .all(|result| !result.contains("outcome: Success")),
                "{command}: {:?}",
                run.bash_results
            );
        }
    }
    // The repeated refusal still ends the loop, exactly as for any inline interpreter.
    let root = wired_workspace();
    let error = run_verify(root.path(), NEXTJS_PROFILE_ID, &vec![former[0].clone(); 3])
        .err()
        .expect("a typed inline hook check must stop the loop")
        .to_string();
    assert!(
        error.contains("recoverable tool error repeated: verify_command_policy_error"),
        "{error}"
    );
}

#[test]
fn issue881_an_altered_source_token_fails_and_is_not_run_in_a_shell() {
    let root = wired_workspace();
    let marker = root.path().join("ran-in-shell");
    let touched = format!(
        "{} && touch {}",
        hook_state_command("src/app/page.tsx"),
        marker.display()
    );
    let run = run_verify(root.path(), NEXTJS_PROFILE_ID, &[touched]).unwrap();
    assert!(!marker.exists());
    assert!(
        run.events
            .iter()
            .any(|event| event["event"] == "runtime_bash_policy" && event["blocked"] == true),
        "{:?}",
        run.events
    );
    for altered in [
        "anvil-nextjs-check:hook_state",
        "anvil-nextjs-check:hook_state:src/app/page.css",
        "anvil-nextjs-check:hook_state:../page.tsx",
        "anvil-nextjs-check:hook_action:primary",
        "anvil-nextjs-check:hook_action:pri%20mary:src/app/page.tsx",
        "anvil-nextjs-check:script_includes:dev:next%2Fdev",
        "anvil-nextjs-check:source_keydown:app/page.tsx",
        "anvil-nextjs-check:source_token:useState:src/app/page.tsx:x",
    ] {
        let run = run_verify(root.path(), NEXTJS_PROFILE_ID, &[altered.to_string()]).unwrap();
        let result = only_result(&run, altered);
        assert!(
            result.contains("combined_outcome: CommandFailed"),
            "{altered}: {result}"
        );
        assert!(result.contains("malformed"), "{altered}: {result}");
    }
    // Spelled by the builders, the same shapes are judged by their content.
    let ok = [
        hook_action_command("primary", "src/app/page.tsx"),
        script_includes_command("next dev").unwrap(),
        source_keydown_command("src/app/page.tsx"),
        source_token_command("useState", "src/app/page.tsx"),
    ];
    for token in ok {
        let run = run_verify(root.path(), NEXTJS_PROFILE_ID, std::slice::from_ref(&token)).unwrap();
        assert!(only_result(&run, &token).contains("combined_outcome: Success"));
    }
}

#[test]
fn issue881_corpus_rewritten_greps_pass_with_no_refusal_and_inline_node_is_refused() {
    const DIR: &str = "../../../tests/corpus/apps/nextjs-verify-rewritten-grep-repeated";
    let read = |name: &str| -> Value {
        let text = match name {
            "planner-greps.json" => include_str!(
                "../../../tests/corpus/apps/nextjs-verify-rewritten-grep-repeated/planner-greps.json"
            ),
            "rewritten-tokens.json" => include_str!(
                "../../../tests/corpus/apps/nextjs-verify-rewritten-grep-repeated/rewritten-tokens.json"
            ),
            "model-commands.json" => include_str!(
                "../../../tests/corpus/apps/nextjs-verify-rewritten-grep-repeated/model-commands.json"
            ),
            "expected.json" => include_str!(
                "../../../tests/corpus/apps/nextjs-verify-rewritten-grep-repeated/expected.json"
            ),
            _ => include_str!(
                "../../../tests/corpus/apps/nextjs-verify-rewritten-grep-repeated/sources.json"
            ),
        };
        serde_json::from_str(text).unwrap_or_else(|error| panic!("{DIR}/{name}: {error}"))
    };
    let strings = |value: &Value| -> Vec<String> {
        value
            .as_array()
            .unwrap()
            .iter()
            .map(|command| command.as_str().unwrap().to_string())
            .collect()
    };
    let (greps, tokens) = (read("planner-greps.json"), read("rewritten-tokens.json"));
    let (commands, expected, sources) = (
        read("model-commands.json"),
        read("expected.json"),
        read("sources.json"),
    );
    let corpus_workspace = |page: &str, package_json: &str| {
        let dir = workspace(package_json);
        std::fs::create_dir_all(dir.path().join("src/app")).unwrap();
        std::fs::write(dir.path().join("src/app/page.tsx"), page).unwrap();
        dir
    };
    let wired = || {
        corpus_workspace(
            sources["page_tsx"].as_str().unwrap(),
            sources["package_json"].as_str().unwrap(),
        )
    };

    // The sanitizer rewrites each planner grep into the recorded token.
    let planner = greps["greps"].as_array().unwrap();
    let recorded = strings(&tokens["tokens"]);
    assert_eq!(planner.len(), recorded.len());
    for (grep, token) in planner.iter().zip(&recorded) {
        let command = grep["command"].as_str().unwrap();
        let rewritten =
            crate::planner::verify::normalize_verify_command_for_oracle_repair_with_root(
                command,
                wired().path(),
            )
            .unwrap_or_else(|| panic!("{command} is not rewritten"));
        assert_eq!(&rewritten.normalized, token, "{command}");
    }

    let times = commands["typed_times"].as_u64().unwrap() as usize;
    let mut shapes: Vec<(String, Vec<String>)> = Vec::new();
    for key in ["one_by_one", "original_greps"] {
        for command in strings(&commands[key]) {
            shapes.push((command.clone(), vec![command; times]));
        }
    }
    for key in ["newline", "and_and", "semicolon"] {
        let command = commands[key].as_str().unwrap().to_string();
        shapes.push((key.to_string(), vec![command; times]));
    }
    assert_eq!(expected["declared_shapes"]["refusals"], 0);
    for (shape, typed) in shapes {
        let root = wired();
        let run = run_verify(root.path(), NEXTJS_PROFILE_ID, &typed)
            .unwrap_or_else(|error| panic!("{shape}: {error}"));
        assert_eq!(run.stop, RunStopReason::AssistantFinal, "{shape}");
        assert!(
            !run.events.iter().any(|event| event["blocked"] == true),
            "{shape}: {:?}",
            run.events
        );
        for needle in expected["declared_shapes"]["result_contains"]
            .as_array()
            .unwrap()
        {
            let needle = needle.as_str().unwrap();
            assert!(
                !run.bash_results.is_empty()
                    && run
                        .bash_results
                        .iter()
                        .all(|result| result.contains(needle)),
                "{shape}: {needle}"
            );
        }
    }

    // What an inline interpreter gets is unchanged: #871's refusal, three times over.
    let refused = expected["inline_node_refused"]["refusals"]
        .as_u64()
        .unwrap() as usize;
    for inline in strings(&commands["inline_node_refused"]) {
        let error = run_verify(wired().path(), NEXTJS_PROFILE_ID, &vec![inline; refused])
            .err()
            .expect("an inline node check typed repeatedly must stop the loop")
            .to_string();
        assert!(
            error.contains(expected["inline_node_refused"]["error"].as_str().unwrap()),
            "{error}"
        );
    }

    // The file table: each token is judged by what the files hold.
    for case in sources["cases"].as_array().unwrap() {
        let root = corpus_workspace(
            case["page_tsx"].as_str().unwrap(),
            case["package_json"].as_str().unwrap(),
        );
        let passes = case["passes"].as_array().unwrap();
        for (token, passes) in recorded.iter().zip(passes) {
            let verdict =
                crate::planner::profiles::nextjs::source_checks::evaluate(root.path(), token);
            assert_eq!(
                verdict.is_ok(),
                passes.as_bool().unwrap(),
                "{} / {token}: {verdict:?}",
                case["name"]
            );
        }
    }
}

/// Every caller of `run_reproducer` (FIX Before and After, investigation, origin
/// reproducer) reaches the same execution, so a token is the host's judgement there.
#[test]
fn issue881_a_fix_reproducer_token_is_judged_by_the_host_not_a_shell() {
    use crate::planner::adjudication::contract::{EvidenceStage, ExpectedOutcome};
    use crate::planner::adjudication::fix::ProbeOutcome;

    let token = hook_action_command("restart", "src/app/page.tsx");
    let run = |root: &Path, stage, expected| {
        let config = config(root, NEXTJS_PROFILE_ID);
        crate::planner::fix_diagnostics::run_reproducer(
            &config,
            "run-881",
            "reproducer_fails",
            stage,
            expected,
            1,
            &token,
            "",
            NEXTJS_PROFILE_ID,
            "Build a Next.js game",
        )
    };
    let root = wired_workspace();
    // The attribute is missing: Before fails, which is what the baseline gate wants.
    let before = run(root.path(), EvidenceStage::Before, ExpectedOutcome::Failure);
    assert_eq!(before.evidence.outcome, ProbeOutcome::Failure);
    // Investigation and the origin reproducer use the same call with the Diagnosis stage.
    let diagnosis = run(
        root.path(),
        EvidenceStage::Diagnosis,
        ExpectedOutcome::Failure,
    );
    assert_eq!(diagnosis.evidence.outcome, ProbeOutcome::Failure);
    // The attribute exists: Before succeeds, so the baseline gate stops the run.
    std::fs::write(
        root.path().join("src/app/page.tsx"),
        "<button data-anvil-action=\"restart\">Again</button>",
    )
    .unwrap();
    let present = run(root.path(), EvidenceStage::Before, ExpectedOutcome::Failure);
    assert_eq!(present.evidence.outcome, ProbeOutcome::Success);
    // After the repair the same token passes.
    let after = run(root.path(), EvidenceStage::After, ExpectedOutcome::Success);
    assert_eq!(after.evidence.outcome, ProbeOutcome::Success);
    for result in [&before, &diagnosis, &present, &after] {
        let shown = format!(
            "{:?}{}{}",
            result.evidence, result.stdout_tail, result.stderr_tail
        );
        assert!(!shown.contains("command not found"), "{shown}");
        assert!(!shown.contains("No such file or directory"), "{shown}");
    }
}

/// The completion contract's verify and the fix regressions run a command the contract
/// holds after validation, which is a token when the contract wrote a hook grep.
fn contract_with(
    verify: &[&str],
    root: &Path,
) -> crate::minimal_loop::completion::CompletionContract {
    crate::minimal_loop::completion::CompletionContract {
        required_paths: Vec::new(),
        protected_paths: Vec::new(),
        verify_commands: verify.iter().map(|command| command.to_string()).collect(),
        fix_reproducer_command: None,
        profile: Some(NEXTJS_PROFILE_ID.to_string()),
        goal: None,
        required_capabilities: Vec::new(),
        deterministic_oracles: Vec::new(),
        required_evidence: Vec::new(),
        evidence_hint_tokens: Vec::new(),
        required_obligations: Vec::new(),
        deferred_verify_requirements: Vec::new(),
        verify_repair_cap: 1,
    }
    .validate(root)
    .unwrap()
}

#[test]
fn issue881_completion_contract_verify_judges_tokens_in_the_host() {
    let hook = PLANNER_GREPS[0];
    let tokens = [DEV, START, BUILD];
    // Present: the hook grep (stored as a token) and the #879 tokens pass.
    let root = wired_workspace();
    let contract = contract_with(&[hook, PLANNER_GREPS[2], DEV, START, BUILD], root.path());
    assert!(
        contract
            .verify_commands
            .iter()
            .all(|c| c.starts_with("anvil-nextjs-check:"))
    );
    let report = contract.verify(root.path());
    // The profile gate is not under test (it wants installed dependencies).
    assert!(report.command_failures.is_empty(), "{report:?}");
    // Absent: each fails on its own, with a reason and never a shell error.
    let empty = workspace(r#"{"scripts":{"dev":"next","build":"next build --lint"}}"#);
    std::fs::create_dir_all(empty.path().join("src/app")).unwrap();
    std::fs::write(
        empty.path().join("src/app/page.tsx"),
        "export default () => null;",
    )
    .unwrap();
    for command in std::iter::once(hook).chain(tokens) {
        let contract = contract_with(&[command], empty.path());
        let report = contract.verify(empty.path());
        let shown = format!("{:?}", report.command_failures);
        assert_eq!(
            report.command_failures.len(),
            usize::from(command != START),
            "{command}: {shown}"
        );
        assert!(!shown.contains("command not found"), "{command}: {shown}");
        assert!(!shown.contains("No such file"), "{command}: {shown}");
    }
}

#[test]
fn issue881_fix_regression_verify_commands_judge_tokens_in_the_host() {
    use crate::planner::adjudication::fix::ProbeOutcome;
    use crate::planner::profile::{ProfileFixRegressionAdapter, ProfileFixRegressionBinding};
    let bindings = |commands: &[&str]| {
        commands
            .iter()
            .enumerate()
            .map(|(index, command)| ProfileFixRegressionBinding {
                id: format!("regression_{index}"),
                adapter: ProfileFixRegressionAdapter::VerifyCommand(command.to_string()),
            })
            .collect::<Vec<_>>()
    };
    let profile = crate::planner::profile::domain_profile(NEXTJS_PROFILE_ID);
    let hook = hook_action_command("primary", "src/app/page.tsx");
    let commands = [hook.as_str(), DEV, START, BUILD];
    let root = wired_workspace();
    let observed =
        profile.run_fix_regressions(root.path(), "Build a game", &bindings(&commands), false);
    assert!(
        observed.iter().all(|o| o.outcome == ProbeOutcome::Success),
        "{observed:?}"
    );
    let empty = workspace(r#"{"scripts":{"dev":"next","build":"next build --lint"}}"#);
    std::fs::create_dir_all(empty.path().join("src/app")).unwrap();
    std::fs::write(
        empty.path().join("src/app/page.tsx"),
        "export default () => null;",
    )
    .unwrap();
    let observed =
        profile.run_fix_regressions(empty.path(), "Build a game", &bindings(&commands), false);
    let outcomes = observed.iter().map(|o| o.outcome).collect::<Vec<_>>();
    assert_eq!(
        outcomes,
        [
            ProbeOutcome::Failure,
            ProbeOutcome::Failure,
            ProbeOutcome::Success,
            ProbeOutcome::Failure
        ],
        "{observed:?}"
    );
    for o in &observed {
        assert!(!o.reason.contains("command not found"), "{o:?}");
        assert!(!o.reason.contains("No such file"), "{o:?}");
    }
}

#[test]
fn issue881_a_setup_step_judges_tokens_even_when_dependencies_are_not_installed() {
    // package.json declares dependencies and node_modules is absent: the dependency
    // state path must not take a token for a command to run in a shell.
    let manifest = r#"{"scripts":{"dev":"next dev -p 3011","build":"next build"},"dependencies":{"next":"^14.2.0","react":"^18.3.0"}}"#;
    let tokens = [
        hook_action_command("primary", "src/app/page.tsx"),
        script_includes_command("next dev").unwrap(),
        source_keydown_command("src/app/page.tsx"),
        source_token_command("useState", "src/app/page.tsx"),
        DEV.to_string(),
    ];
    let verify = |page: &str, token: &str| {
        let root = workspace(manifest);
        std::fs::create_dir_all(root.path().join("src/app")).unwrap();
        std::fs::write(root.path().join("src/app/page.tsx"), page).unwrap();
        assert!(!root.path().join("node_modules").exists());
        let step = crate::planner::step_plan::PlanStep {
            id: "verify-contract".to_string(),
            kind: "verify".to_string(),
            expected_result: "pass".to_string(),
            instruction: "Check the contract".to_string(),
            expected_paths: Vec::new(),
            verify: vec![token.to_string()],
        };
        let (report, lifecycles) =
            crate::planner::verify::verify_step_with_profile_setup_observed_with_offline_and_events(
                root.path(),
                &step,
                Some(NEXTJS_PROFILE_ID),
                NodeDependencySetupAuthority::PlanSetupStep,
                true,
                None,
            );
        assert!(lifecycles.is_empty(), "{token}: {lifecycles:?}");
        assert!(!root.path().join("node_modules").exists(), "{token}");
        report
    };
    for token in &tokens {
        let ok = verify(PAGE_SOURCE, token);
        assert!(ok.is_pass(), "{token}: {ok:?}");
        let bad = verify("export default () => null;", token);
        let shown = format!("{:?}", bad.command_failures);
        assert!(!shown.contains("command not found"), "{token}: {shown}");
        assert!(!shown.contains("No such file"), "{token}: {shown}");
        // `dev` is satisfied by the manifest alone, the hooks and sources are not.
        assert_eq!(
            bad.is_pass(),
            token.contains("script") || token == DEV,
            "{token}: {bad:?}"
        );
    }
}

#[test]
fn issue881_a_package_port_grep_joined_with_other_greps_is_host_judged() {
    // The repaired port token of Issue #879 used to reach a shell (exit 127) when the
    // grep was one segment of a joined command; every joined form is now host-judged.
    let port = r#"grep -q '"3011"' package.json"#;
    let hook = PLANNER_GREPS[0];
    for joined in [
        format!("{port}\n{hook}"),
        format!("{port} && {hook}"),
        format!("{port}; {hook}"),
        format!("{hook} && {port} && {}", PLANNER_GREPS[2]),
    ] {
        let root = wired_workspace();
        let run = run_verify(root.path(), NEXTJS_PROFILE_ID, &vec![joined.clone(); 3])
            .unwrap_or_else(|error| panic!("{joined}: {error}"));
        assert_host_judged(&joined, &run);
    }
}
