//! Pins how the CLI selects and runs each direct action (Issue #734, parent #521).
//!
//! Every case runs the real `commandagent` binary in a temporary workspace with an
//! Ollama endpoint that only counts connections. A connection therefore proves the
//! action started a provider client, and zero connections proves it did not. Exit
//! codes, error text, progress output, and the recorded run events are compared
//! after replacing machine-specific values (provider port, workspace path).

use std::io::Read;
use std::net::TcpListener;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;

use sha2::{Digest, Sha256};

struct Outcome {
    code: Option<i32>,
    stdout: String,
    stderr: String,
    provider_connections: usize,
}

fn run_action(workspace: &Path, args: &[&str]) -> Outcome {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let port = listener.local_addr().unwrap().port();
    let stop = Arc::new(AtomicBool::new(false));
    let count = Arc::new(AtomicUsize::new(0));
    let watcher = {
        let (stop, count) = (stop.clone(), count.clone());
        thread::spawn(move || {
            while !stop.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        count.fetch_add(1, Ordering::SeqCst);
                        let _ = stream.set_read_timeout(Some(Duration::from_millis(50)));
                        let mut buffer = [0u8; 256];
                        let _ = stream.read(&mut buffer);
                    }
                    Err(_) => thread::sleep(Duration::from_millis(10)),
                }
            }
        })
    };
    let home = workspace.join("home");
    let state = workspace.join("state");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&state).unwrap();
    let host = format!("http://127.0.0.1:{port}");
    let output = Command::new(env!("CARGO_BIN_EXE_commandagent"))
        .args([
            "--provider",
            "ollama",
            "--planner-provider",
            "ollama",
            "--ollama-host",
            &host,
            "--model",
            "m",
            "--planner-model",
            "m",
            "--chat-retries",
            "0",
            "--chat-timeout-secs",
            "2",
            "--no-footer",
            "--cwd",
            workspace.to_str().unwrap(),
            "--state-dir",
            state.to_str().unwrap(),
        ])
        .args(args)
        .env("HOME", &home)
        .env("PATH", "")
        .env_remove("OPENAI_API_KEY")
        .output()
        .unwrap();
    stop.store(true, Ordering::SeqCst);
    watcher.join().unwrap();
    Outcome {
        code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        provider_connections: count.load(Ordering::SeqCst),
    }
}

fn normalize(workspace: &Path, text: &str) -> String {
    let mut text = text.to_owned();
    for root in [
        workspace
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .into_owned(),
        workspace.to_string_lossy().into_owned(),
    ] {
        text = text.replace(&root, "<WS>");
    }
    const SCHEME: &str = "http://127.0.0.1:";
    while let Some(start) = text.find(SCHEME) {
        let digits = text[start + SCHEME.len()..]
            .find(|c: char| !c.is_ascii_digit())
            .map_or(text.len(), |n| start + SCHEME.len() + n);
        text.replace_range(start..digits, "<HOST>");
    }
    text
}

fn event_lines(workspace: &Path) -> Vec<String> {
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(workspace.join(".commandagent/runs")) {
        for entry in entries.flatten() {
            files.push(entry.path().join("events.jsonl"));
        }
    }
    files.sort();
    let mut lines = Vec::new();
    for file in files {
        let Ok(text) = std::fs::read_to_string(file) else {
            continue;
        };
        for line in text.lines() {
            let value: serde_json::Value = serde_json::from_str(line).unwrap();
            let name = value["event"].as_str().unwrap();
            // Sandbox availability and host environment contamination depend on the
            // machine running the test, not on the selected action.
            if matches!(
                name,
                "bash_sandbox_status" | "host_env_contamination" | "host_env_normalized"
            ) {
                continue;
            }
            // `time_profile` is emitted only when the summed time exceeds 0 ms, so drop the
            // one right after the stop; one anywhere else stays in the sequence and fails.
            if name == "time_profile"
                && lines
                    .last()
                    .is_some_and(|last: &String| last.starts_with("tui_command_stop"))
            {
                continue;
            }
            let field = |key: &str| value[key].as_str().unwrap_or("-").to_owned();
            lines.push(match name {
                "run_start" => format!("run_start action={}", field("action")),
                "tui_command_stop" => format!(
                    "tui_command_stop command={} status={} failure_kind={} stop_reason={}",
                    field("command"),
                    field("status"),
                    field("failure_kind"),
                    field("stop_reason")
                ),
                "run_stop" => format!(
                    "run_stop status={} failure_kind={} stop_reason={}",
                    field("status"),
                    field("failure_kind"),
                    field("stop_reason")
                ),
                other => other.to_owned(),
            });
        }
    }
    lines
}

/// Exit code, provider connections, stdout, error lines, and event sequence.
fn snapshot(workspace: &Path, outcome: &Outcome) -> String {
    let mut out = format!(
        "exit={:?} provider_connections={}\n--stdout--\n",
        outcome.code, outcome.provider_connections
    );
    out.push_str(&normalize(workspace, &outcome.stdout));
    out.push_str("--stderr--\n");
    for line in outcome.stderr.lines() {
        if line.starts_with("error:") || line.starts_with("Hint:") {
            out.push_str(&normalize(workspace, line));
            out.push('\n');
        }
    }
    out.push_str("--events--\n");
    for line in event_lines(workspace) {
        out.push_str(&normalize(workspace, &line));
        out.push('\n');
    }
    out
}

fn run_snapshot(args: &[&str]) -> String {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("bad.yaml"), "not: a plan\n").unwrap();
    let outcome = run_action(dir.path(), args);
    snapshot(dir.path(), &outcome)
}

const HINT: &str = "Hint: Start Ollama with `ollama serve`, verify `--ollama-host <HOST>`, then run `commandagent --doctor`.";
const REQUEST_FAILED: &str =
    "Ollama request failed: error sending request for url (<HOST>/api/chat)";

fn provider_failure_snapshot(
    connections: usize,
    stdout: &str,
    action: &str,
    command: &str,
    turns: &[&str],
) -> String {
    let cause = format!("provider request failed after 2 attempts: {REQUEST_FAILED}");
    let turns = turns.iter().map(|t| format!("{t}\n")).collect::<String>();
    format!(
        "exit=Some(1) provider_connections={connections}\n--stdout--\n{stdout}--stderr--\n\
         error: {cause}\n{HINT}\n--events--\n\
         run_start action={action}\nintent_resolved\nplan_preset_resolved\n{turns}\
         tui_command_stop command={command} status=failed failure_kind=direct_cli_command_failed \
         stop_reason={cause} {HINT}\n\
         run_stop status=failed failure_kind=process_failure stop_reason={cause} {HINT}\n"
    )
}

#[test]
fn prompt_runs_the_session_loop_against_the_executor_provider() {
    let expected = format!(
        "exit=Some(1) provider_connections=1\n--stdout--\n\
         → implementing (model turn: m, up to 2s)\n--stderr--\n\
         error: {REQUEST_FAILED}\n{HINT}\n--events--\n\
         run_start action=Prompt(\"hi\")\nintent_resolved\nplan_preset_resolved\n\
         step_obligation_scope\nprovider_turn_duration\nfallback_decision\n\
         tui_command_stop command=--prompt status=failed failure_kind=direct_cli_command_failed \
         stop_reason={REQUEST_FAILED} {HINT}\n\
         run_stop status=failed failure_kind=process_failure \
         stop_reason={REQUEST_FAILED} {HINT}\n"
    );
    assert_eq!(run_snapshot(&["--prompt", "hi"]), expected);
}

#[test]
fn plan_steps_generates_a_step_plan_with_the_planner_provider() {
    let reason = "failed to generate step plan";
    let cause = format!("provider request failed after 2 attempts: {REQUEST_FAILED}");
    let expected = format!(
        "exit=Some(1) provider_connections=2\n--stdout--\n\
         → planning steps (m, up to 2s)\n→ planning steps (m, up to 2s)\n--stderr--\n\
         error: {reason}: {cause}\n{HINT}\n--events--\n\
         run_start action=PlanSteps(\"g\")\nintent_resolved\nplan_preset_resolved\n\
         provider_turn_duration\nprovider_turn_duration\n\
         tui_command_stop command=--plan-steps status=failed \
         failure_kind=direct_cli_command_failed stop_reason={reason}\n\
         run_stop status=failed failure_kind=process_failure stop_reason={reason}\n"
    );
    assert_eq!(run_snapshot(&["--plan-steps", "g"]), expected);
}

#[test]
fn plan_run_plans_and_then_executes_the_plan() {
    let expected = provider_failure_snapshot(
        2,
        "→ planning steps (m, up to 2s)\n→ planning steps (m, up to 2s)\n",
        "PlanRun(\"g\")",
        "--plan-run",
        &["provider_turn_duration", "provider_turn_duration"],
    );
    assert_eq!(run_snapshot(&["--plan-run", "g"]), expected);
}

#[test]
fn ultra_plan_generates_an_ultra_plan_with_the_planner_provider() {
    let expected = provider_failure_snapshot(
        2,
        "→ planning the overall plan (m, up to 2s)\n→ planning the overall plan (m, up to 2s)\n",
        "UltraPlan(\"g\")",
        "--ultra-plan",
        &[
            "ultra_plan_generation_attempt",
            "provider_turn_duration",
            "provider_turn_duration",
        ],
    );
    assert_eq!(run_snapshot(&["--ultra-plan", "g"]), expected);
}

#[test]
fn ultra_plan_run_generates_and_runs_an_ultra_plan() {
    let expected = provider_failure_snapshot(
        2,
        "→ planning the overall plan (m, up to 2s)\n→ planning the overall plan (m, up to 2s)\n",
        "UltraPlanRun(\"g\")",
        "--ultra-plan-run",
        &[
            "ultra_plan_generation_attempt",
            "provider_turn_duration",
            "provider_turn_duration",
        ],
    );
    assert_eq!(run_snapshot(&["--ultra-plan-run", "g"]), expected);
}

fn plan_file_failure(action: &str, command: &str, error: &str, stop_reason: &str) -> String {
    format!(
        "exit=Some(1) provider_connections=0\n--stdout--\n--stderr--\nerror: {error}\n\
         --events--\nrun_start action={action}\nintent_resolved\nplan_preset_resolved\n\
         tui_command_stop command={command} status=failed \
         failure_kind=direct_cli_command_failed stop_reason={stop_reason}\n\
         run_stop status=failed failure_kind=process_failure stop_reason={stop_reason}\n"
    )
}

#[test]
fn run_plan_reads_a_step_plan_file_and_fails_before_any_provider_turn() {
    let missing = "path does not exist: missing.yaml";
    assert_eq!(
        run_snapshot(&["--run-plan", "missing.yaml"]),
        plan_file_failure(
            "RunPlan(\"missing.yaml\")",
            "--run-plan",
            &format!("{missing}: No such file or directory (os error 2)"),
            missing,
        )
    );
    assert_eq!(
        run_snapshot(&["--run-plan", "bad.yaml"]),
        plan_file_failure(
            "RunPlan(\"bad.yaml\")",
            "--run-plan",
            "StepPlan missing goal",
            "StepPlan missing goal",
        )
    );
}

#[test]
fn run_ultra_plan_reads_an_ultra_plan_file_and_fails_before_any_provider_turn() {
    let missing = "path does not exist: missing.yaml";
    assert_eq!(
        run_snapshot(&["--run-ultra-plan", "missing.yaml"]),
        plan_file_failure(
            "RunUltraPlan(\"missing.yaml\")",
            "--run-ultra-plan",
            &format!("{missing}: No such file or directory (os error 2)"),
            missing,
        )
    );
    assert_eq!(
        run_snapshot(&["--run-ultra-plan", "bad.yaml"]),
        plan_file_failure(
            "RunUltraPlan(\"bad.yaml\")",
            "--run-ultra-plan",
            "UltraPlan missing goal",
            "UltraPlan missing goal",
        )
    );
}

#[test]
fn model_probe_drives_both_provider_roles_and_prints_the_card() {
    let dir = tempfile::tempdir().unwrap();
    let outcome = run_action(dir.path(), &["--model-probe"]);
    assert_eq!(outcome.code, Some(0));
    assert_eq!(outcome.provider_connections, 12);
    let progress: Vec<&str> = outcome
        .stdout
        .lines()
        .take_while(|line| line.starts_with('→'))
        .collect();
    let mut expected = vec!["→ implementing (model turn: m, up to 2s)"; 10];
    expected.extend(["→ planning steps (m, up to 2s)"; 2]);
    assert_eq!(progress, expected);
    for line in [
        "# Model Probe Card",
        "- Version: model-probe-v3",
        "- Executor: Ollama `m`",
        "- Planner: Ollama `m`",
        "- Classifier: Ollama `m`",
        "- Tasks: 12",
        "- No-network guarantee: passed",
    ] {
        assert!(
            outcome.stdout.lines().any(|candidate| candidate == line),
            "missing card line {line:?}"
        );
    }
    assert_eq!(
        event_lines(dir.path()),
        [
            "run_start action=ModelProbe",
            "intent_resolved",
            "plan_preset_resolved",
            "tui_command_stop command=--model-probe status=completed failure_kind= \
             stop_reason=completed",
            "run_stop status=completed failure_kind= stop_reason=completed",
        ]
    );
}

fn write_community_schema(workspace: &Path, schema: &str, pin: Option<&str>) {
    let dir = workspace.join("schema");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("app-spec.schema.yaml"), schema).unwrap();
    if let Some(pin) = pin {
        std::fs::write(dir.join("app-spec.schema.sha256"), pin).unwrap();
    }
}

fn sha256_hex(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

fn community_snapshot(prepare: impl Fn(&Path)) -> String {
    let dir = tempfile::tempdir().unwrap();
    prepare(dir.path());
    let outcome = run_action(
        dir.path(),
        &[
            "--prompt",
            "hi",
            "--profile",
            "community-mini-app",
            "--offline",
        ],
    );
    snapshot(dir.path(), &outcome)
}

fn community_failure(violation: &str) -> String {
    let reason = format!("community_profile_violation:{violation}");
    format!(
        "exit=Some(1) provider_connections=0\n--stdout--\n--stderr--\nerror: {reason}\n\
         --events--\nrun_start action=Prompt(\"hi\")\nintent_resolved\nplan_preset_resolved\n\
         community_profile_verification\n\
         tui_command_stop command=--prompt status=failed \
         failure_kind=direct_cli_command_failed stop_reason={reason}\n\
         run_stop status=failed failure_kind=community_profile_violation stop_reason={reason}\n"
    )
}

#[test]
fn offline_community_profile_prompt_verifies_the_workspace_without_a_provider() {
    assert_eq!(
        community_snapshot(|_| {}),
        community_failure("community_schema_missing")
    );
    let schema = "schema_version: x\n";
    assert_eq!(
        community_snapshot(|ws| write_community_schema(ws, schema, None)),
        community_failure("community_schema_pin_missing")
    );
    let wrong = "0".repeat(64);
    assert_eq!(
        community_snapshot(|ws| write_community_schema(ws, schema, Some(&wrong))),
        community_failure(&format!(
            "community_schema_pin_mismatch:{wrong}!={}",
            sha256_hex(schema)
        ))
    );
    let broken = "a: [\n";
    assert_eq!(
        community_snapshot(|ws| write_community_schema(ws, broken, Some(&sha256_hex(broken)))),
        community_failure("community_schema_invalid")
    );
    assert_eq!(
        community_snapshot(|ws| write_community_schema(ws, schema, Some(&sha256_hex(schema)))),
        community_failure("community_schema_version_invalid")
    );
}

#[test]
fn community_profile_prompt_only_short_circuits_when_offline() {
    let dir = tempfile::tempdir().unwrap();
    let outcome = run_action(
        dir.path(),
        &["--prompt", "hi", "--profile", "community-mini-app"],
    );
    assert_eq!(outcome.provider_connections, 1);
    assert!(
        !event_lines(dir.path())
            .iter()
            .any(|line| line == "community_profile_verification")
    );
}

#[test]
fn setup_interaction_probe_never_starts_a_provider() {
    let expected = "exit=Some(1) provider_connections=0\n--stdout--\n\
         interaction probe setup: using <WS>/home/.commandagent/tools/interaction-probe\n\
         interaction probe setup: npm install playwright\n--stderr--\n\
         error: failed to spawn npm: No such file or directory (os error 2)\n--events--\n\
         run_start action=SetupInteractionProbe\nintent_resolved\nplan_preset_resolved\n\
         tui_command_stop command=--setup-interaction-probe status=failed \
         failure_kind=direct_cli_command_failed stop_reason=failed to spawn npm\n\
         run_stop status=failed failure_kind=process_failure stop_reason=failed to spawn npm\n";
    assert_eq!(run_snapshot(&["--setup-interaction-probe"]), expected);
}

#[test]
fn repl_without_a_terminal_is_rejected_before_any_provider_turn() {
    let expected = "exit=Some(1) provider_connections=0\n--stdout--\n--stderr--\n\
         error: stdin is not a TTY; pass --prompt or an action flag\n--events--\n\
         run_start action=Repl\nintent_resolved\nplan_preset_resolved\n\
         run_stop status=incomplete failure_kind=process_failure \
         stop_reason=stdin is not a TTY; pass --prompt or an action flag\n";
    assert_eq!(run_snapshot(&[]), expected);
}

const UX_DEMO_BODY: [&str; 34] = [
    "model=m (flag) provider=ollama (flag) planner=m (flag) planner_provider=ollama (flag)",
    "context_budget=65536 (default) timeout=2s (flag) profile=generic (default) prompt_layout=legacy (default) narration=normal (default) footer=off (flag) stream=off (default:non_interactive) yes=false",
    "start: plain-text request → review Gate 1 → /confirm <hash> | help: /help",
    "### Plan",
    "- Goal: UX demo: build a tiny web app and recover from an interruption",
    "- Profile: generic",
    "- Port: not specified",
    "- UltraPlan accepted: 2 phases; first: scaffold",
    "- Phase list:",
    "1. ○ scaffold - Create the app shell and first route",
    "2. ○ verify - Run browser checks, repair evidence, and summarize recovery",
    "── Phase 1/2: scaffold ──",
    "#### Phase: scaffold",
    "1. ○ [implement] Write src/app/page.tsx and supporting styles (verify 1)",
    "→ Write src/app/page.tsx",
    "✓ Write ok",
    "✗ verify missing path browser-readiness.json",
    "↻ repair 1/2: browser evidence",
    "→ planning steps for phase 1/2 scaffold (m, up to 600s)",
    "planning steps 0s/10m00s",
    "planning steps 1s/10m00s",
    "… still waiting (60s/600s)",
    "planning steps 2s/10m00s",
    "[stopping: aborting current operation…] planning steps 10s/10m00s",
    "### Terminal summary",
    "- Status: interrupted · Assurance: partial (demo interrupted during provider turn)",
    "| Gate | Recorded value |",
    "| --- | --- |",
    "| task_status | interrupted |",
    "| release_gate_status | partial |",
    "| final_acceptance_status | interrupted |",
    "| browser_readiness_status | execution=not_applicable status=not_applicable |",
    "| interaction_evidence_status | execution=not_applicable status=not_applicable |",
    "| persistence_after_reload | not_applicable |",
];

#[test]
fn ux_demo_prints_the_scripted_interruption_without_recording_a_run() {
    let dir = tempfile::tempdir().unwrap();
    let outcome = run_action(dir.path(), &["--ux-demo"]);
    assert_eq!(outcome.code, Some(0));
    assert_eq!(outcome.provider_connections, 0);
    assert!(outcome.stderr.is_empty(), "{}", outcome.stderr);
    assert!(event_lines(dir.path()).is_empty());
    let stdout = normalize(dir.path(), &outcome.stdout);
    let body: Vec<&str> = stdout
        .lines()
        .filter(|line| {
            !line.starts_with("commandagent ")
                && !line.starts_with("mode=")
                && !line.starts_with("run_log=")
                && !line.starts_with("warning: build stamp is dirty")
        })
        .collect();
    assert_eq!(body[..UX_DEMO_BODY.len()], UX_DEMO_BODY);
    assert_eq!(
        body[UX_DEMO_BODY.len()..],
        [
            "- Primary stop reason: interrupted by user during demo provider turn",
            "- Phases completed: unknown",
            "- Recovery prompt: .anvil/repairs/ux-demo-recovery.md",
            "- Recovery UltraPlan: .anvil/plans/ux-demo-recovery.yaml",
            "- Suggested YAML command: /run-ultra-plan .anvil/plans/ux-demo-recovery.yaml",
            "- Next action: resume_or_rerun_command",
        ]
    );
}

#[test]
fn runs_lists_recorded_runs_without_a_provider_or_a_run_of_its_own() {
    let dir = tempfile::tempdir().unwrap();
    let outcome = run_action(dir.path(), &["--runs"]);
    assert_eq!(outcome.code, Some(0));
    assert_eq!(outcome.provider_connections, 0);
    assert_eq!(outcome.stdout, "No runs found for this workspace.\n");
    assert!(event_lines(dir.path()).is_empty());
}

#[test]
fn summary_json_is_projected_for_failed_runs_but_not_for_ux_demo_or_empty_runs() {
    let dir = tempfile::tempdir().unwrap();
    let failed = run_action(dir.path(), &["--summary-json", "--plan-steps", "g"]);
    let summary: serde_json::Value =
        serde_json::from_str(failed.stdout.lines().last().unwrap()).unwrap();
    assert_eq!(
        summary["schema_version"],
        "commandagent.headless-summary/v1"
    );
    assert_eq!(summary["status"], "failed");
    assert_eq!(summary["exit_code"], 1);
    assert_eq!(summary["verdict"], "reduced");
    assert_eq!(summary["stop_class"], "direct_cli_command_failed");
    assert_eq!(summary["stop_reason"], "failed to generate step plan");
    assert_eq!(summary["next_action"], "fix_command_failure");

    let demo = run_action(dir.path(), &["--summary-json", "--ux-demo"]);
    assert_eq!(demo.code, Some(0));
    assert!(!demo.stdout.contains("headless-summary"), "{}", demo.stdout);

    let empty = tempfile::tempdir().unwrap();
    let runs = run_action(empty.path(), &["--summary-json", "--runs"]);
    assert_eq!(runs.stdout, "No runs found for this workspace.\n");
}

#[test]
fn summary_json_reports_the_community_profile_violation_without_a_provider() {
    let dir = tempfile::tempdir().unwrap();
    let outcome = run_action(
        dir.path(),
        &[
            "--summary-json",
            "--prompt",
            "hi",
            "--profile",
            "community-mini-app",
            "--offline",
        ],
    );
    assert_eq!(outcome.code, Some(1));
    assert_eq!(outcome.provider_connections, 0);
    let summary: serde_json::Value =
        serde_json::from_str(outcome.stdout.lines().last().unwrap()).unwrap();
    assert_eq!(summary["status"], "failed");
    assert_eq!(summary["exit_code"], 1);
    assert_eq!(summary["assurance"], "partial");
    assert_eq!(summary["stop_class"], "community_profile_violation");
    assert_eq!(
        summary["stop_reason"],
        "community_profile_violation:community_schema_missing"
    );
}

#[test]
fn argument_rejections_exit_two_before_any_run_starts() {
    let dir = tempfile::tempdir().unwrap();
    let outcome = run_action(dir.path(), &["--workflow", "missing.yaml"]);
    assert_eq!(
        snapshot(dir.path(), &outcome),
        "exit=Some(2) provider_connections=0\n--stdout--\n--stderr--\n\
         error: --workflow requires --origin\n--events--\n"
    );
}

#[test]
fn doctor_reports_failed_checks_without_recording_a_run() {
    let dir = tempfile::tempdir().unwrap();
    let outcome = run_action(dir.path(), &["--doctor", "--json"]);
    assert_eq!(outcome.code, Some(1));
    let report: serde_json::Value = serde_json::from_str(&outcome.stdout).unwrap();
    assert!(
        report["checks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|check| check["id"] == "provider.ollama.reachable" && check["status"] == "fail")
    );
    assert!(
        outcome
            .stderr
            .lines()
            .any(|line| line == "error: doctor found failed checks")
    );
    assert!(event_lines(dir.path()).is_empty());
}
