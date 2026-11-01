//! Issue #952: a verify entry that reads as a sentence or a deferral, in any
//! language, is a `verify_command_shape` lint error before anything runs, so
//! the existing three planner attempts repair it or the run stops. Commands
//! that may execute (builtins, PATH executables, workspace or produced
//! scripts, setup tools, catalog checks, and commands not on PATH yet) are left
//! to the runtime gate. Every case is synthetic; the planner is a scripted
//! in-process client and the public validate-plan runs with a pinned PATH.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::{Arc, Mutex};

use clap::Parser;
use commandagent::config::Config;
use commandagent::minimal_loop::completion::CompletionContract;
use commandagent::minimal_loop::dependency_setup::NodeDependencySetupAuthority;
use commandagent::minimal_loop::reachability::{
    assess_repair_reachability_at_root, reachability_failure_kind,
};
use commandagent::planner::generate_step_plan;
use commandagent::planner::plan::render_editable_step_plan;
use commandagent::planner::step_plan::{PlanStep, StepPlan};
use commandagent::planner::verify::verify_step;
use commandagent::providers::{AssistantReply, ChatClient};
use commandagent::state::ConversationMessage;
use commandagent::tools::registry::ToolSpec;
use serde_json::Value;
use sha2::{Digest, Sha256};

const CORPUS: &str = "tests/corpus/apps/verify-command-admission";
const CATEGORY: &str = "verify_command_shape";

#[derive(Clone)]
struct Replay {
    replies: Arc<Mutex<Vec<AssistantReply>>>,
    requests: Arc<Mutex<Vec<Vec<ConversationMessage>>>>,
}

impl Replay {
    fn new(plans: &[&StepPlan]) -> Self {
        let replies = plans
            .iter()
            .map(|p| AssistantReply::text(serde_json::to_string(p).unwrap()))
            .collect();
        Self {
            replies: Arc::new(Mutex::new(replies)),
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn requests(&self) -> Vec<Vec<ConversationMessage>> {
        self.requests.lock().unwrap().clone()
    }
}

impl ChatClient for Replay {
    fn label(&self) -> &str {
        "issue952-replay"
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
        messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        self.requests.lock().unwrap().push(messages.to_vec());
        let mut replies = self.replies.lock().unwrap();
        if replies.is_empty() {
            anyhow::bail!("issue952 replay exhausted");
        }
        Ok(replies.remove(0))
    }
}

fn config(root: &Path) -> Config {
    let mut c = Config::from_cli(commandagent::cli::Cli::parse_from([
        "commandagent",
        "--ux-demo",
    ]))
    .unwrap();
    c.workspace_root = root.to_path_buf();
    c.profile = "generic".into();
    c.profile_explicit = true;
    c.offline = true;
    c.yes = true;
    c.max_iterations = 4;
    c.eval_events_path = Some(root.join(".commandagent/runs/issue952/events.jsonl"));
    std::fs::create_dir_all(c.eval_events_path.as_ref().unwrap().parent().unwrap()).unwrap();
    c
}

fn corpus_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(CORPUS)
        .join(name)
}

fn fixture() -> Value {
    serde_json::from_str(&std::fs::read_to_string(corpus_path("fixtures/events.json")).unwrap())
        .unwrap()
}

fn events(c: &Config) -> Vec<Value> {
    std::fs::read_to_string(c.eval_events_path.as_ref().unwrap())
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .collect()
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .map(|v| v.as_str().unwrap().to_string())
                .collect()
        })
        .unwrap_or_default()
}

fn step(id: &str, kind: &str, paths: &[String], verify: &[String]) -> PlanStep {
    PlanStep {
        id: id.into(),
        kind: kind.into(),
        expected_result: "pass".into(),
        instruction: format!("Carry out the synthetic step {id}."),
        expected_paths: paths.to_vec(),
        verify: verify.to_vec(),
    }
}

fn case_plan(case: &Value, command: &str) -> StepPlan {
    let goal = fixture()["goal"].as_str().unwrap().to_string();
    let mut paths = strings(&case["producer_paths"]);
    if paths.is_empty() {
        paths.push(fixture()["artifact"].as_str().unwrap().to_string());
    }
    // One owner per produced path, as planner admission requires.
    let mut steps = paths
        .iter()
        .enumerate()
        .map(|(index, path)| {
            step(
                &format!("write-{index}"),
                "implement",
                std::slice::from_ref(path),
                &[],
            )
        })
        .collect::<Vec<_>>();
    steps.push(step("check-report", "verify", &[], &[command.to_string()]));
    StepPlan { goal, steps }
}

#[cfg(unix)]
fn executable(dir: &Path, name: &str, body: &str) {
    use std::os::unix::fs::PermissionsExt;
    let path = dir.join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// Runs the public `--validate-plan` with PATH pinned to `bin`, never the
/// terminal's PATH.
fn validate_plan(root: &Path, bin: &Path, plan: &StepPlan) -> Output {
    let file = root.join("step-plan.yaml");
    std::fs::write(&file, render_editable_step_plan(plan)).unwrap();
    Command::new(env!("CARGO_BIN_EXE_commandagent"))
        .env("PATH", bin)
        .args([
            "--cwd",
            root.to_str().unwrap(),
            "--validate-plan",
            "step-plan.yaml",
        ])
        .output()
        .unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[cfg(unix)]
#[test]
fn positive_commands_pass_the_public_validate_plan_with_a_pinned_path() {
    for case in fixture()["positive"].as_array().unwrap() {
        let root = tempfile::tempdir().unwrap();
        let bin = tempfile::tempdir().unwrap();
        let markers = tempfile::tempdir().unwrap();
        let marker = markers.path().join("ran");
        for name in strings(&case["path_bin"]) {
            executable(
                bin.path(),
                &name,
                &format!("echo ran > '{}'", marker.display()),
            );
        }
        if let Some(existing) = case["existing"].as_str() {
            executable(
                root.path(),
                existing,
                &format!("echo ran > '{}'", marker.display()),
            );
        }
        let command = case["verify"].as_str().unwrap();
        let output = validate_plan(root.path(), bin.path(), &case_plan(case, command));
        assert!(
            output.status.success(),
            "{}: {}",
            case["id"],
            stderr(&output)
        );
        // Lint resolves names only; it never runs the command it judges.
        assert!(!marker.exists(), "{}", case["id"]);
    }
}

#[test]
fn positive_commands_pass_planner_admission_on_the_first_attempt() {
    for case in fixture()["positive"].as_array().unwrap() {
        if case.get("path_bin").is_some() {
            continue;
        }
        let root = tempfile::tempdir().unwrap();
        if let Some(existing) = case["existing"].as_str() {
            std::fs::write(root.path().join(existing), "#!/bin/sh\nexit 0\n").unwrap();
        }
        let c = config(root.path());
        let command = case["verify"].as_str().unwrap();
        let plan = case_plan(case, command);
        let mut planner = Replay::new(&[&plan, &plan, &plan]);
        let admitted = generate_step_plan(&mut planner, &plan.goal, &c)
            .unwrap_or_else(|e| panic!("{}: {e}", case["id"]));
        let retried_for_shape = planner
            .requests()
            .iter()
            .flatten()
            .any(|m| m.content.contains(&format!("[{CATEGORY}]")));
        assert!(!retried_for_shape, "{}", case["id"]);
        assert!(
            admitted
                .steps
                .iter()
                .any(|s| s.verify.iter().any(|v| v == command)),
            "{}: {admitted:?}",
            case["id"]
        );
        assert!(
            !events(&c).iter().any(|e| e["event"] == "planner_error"
                && e["planner_error_message"]
                    .as_str()
                    .is_some_and(|m| m.contains(CATEGORY))),
            "{}",
            case["id"]
        );
    }
}

#[test]
fn order_controls_accept_the_earlier_producer_and_reject_the_later_one() {
    let goal = fixture()["goal"].as_str().unwrap().to_string();
    for control in fixture()["order_controls"].as_array().unwrap() {
        let producer = &control["producer_step"];
        let make = step(
            producer["id"].as_str().unwrap(),
            producer["kind"].as_str().unwrap(),
            &strings(&producer["expected_paths"]),
            &[],
        );
        let check = step(
            "check-report",
            "verify",
            &[],
            &[control["verify"].as_str().unwrap().to_string()],
        );
        let root = tempfile::tempdir().unwrap();
        let bin = tempfile::tempdir().unwrap();
        let before = StepPlan {
            goal: goal.clone(),
            steps: vec![make.clone(), check.clone()],
        };
        let output = validate_plan(root.path(), bin.path(), &before);
        assert!(
            output.status.success(),
            "{}: {}",
            control["id"],
            stderr(&output)
        );
        let after = StepPlan {
            goal: goal.clone(),
            steps: vec![check, make],
        };
        let output = validate_plan(root.path(), bin.path(), &after);
        let text = stderr(&output);
        assert!(!output.status.success(), "{}: {text}", control["id"]);
        assert!(
            text.contains(control["after_category"].as_str().unwrap()),
            "{}: {text}",
            control["id"]
        );
        assert!(
            text.contains(control["after_reason"].as_str().unwrap()),
            "{}: {text}",
            control["id"]
        );
        assert!(!text.contains(CATEGORY), "{}: {text}", control["id"]);
    }
}

fn negatives() -> Vec<Value> {
    fixture()["negative"].as_array().unwrap().clone()
}

fn first_word(command: &str) -> &str {
    command.split_whitespace().next().unwrap()
}

/// The lint message shape observed at the base for a rejected sentence.
fn base_message(command: &str) -> String {
    fixture()["base_lint_message"]
        .as_str()
        .unwrap()
        .replace("{first}", first_word(command))
        .replace("{step}", "check-report")
        .replace("{command}", command)
}

fn corrected_plan(case: &Value) -> StepPlan {
    case_plan(case, fixture()["corrected_verify"].as_str().unwrap())
}

fn shape_errors(c: &Config) -> Vec<Value> {
    events(c)
        .into_iter()
        .filter(|e| {
            e["event"] == "planner_error"
                && e["planner_error_message"]
                    .as_str()
                    .is_some_and(|m| m.starts_with("verify command cannot execute as written"))
        })
        .collect()
}

#[cfg(unix)]
#[test]
fn negative_sentences_fail_the_public_validate_plan_with_a_pinned_path() {
    for case in negatives() {
        let root = tempfile::tempdir().unwrap();
        let bin = tempfile::tempdir().unwrap();
        let command = case["verify"].as_str().unwrap();
        let output = validate_plan(root.path(), bin.path(), &case_plan(&case, command));
        let text = stderr(&output);
        assert!(!output.status.success(), "{}: {text}", case["id"]);
        assert!(
            text.contains(&format!("[{CATEGORY}]")),
            "{}: {text}",
            case["id"]
        );
        assert!(
            text.contains(&base_message(command)),
            "{}: {text}",
            case["id"]
        );
    }
}

#[test]
fn negative_sentence_is_repaired_within_the_existing_attempts_and_then_runs() {
    for case in negatives() {
        let root = tempfile::tempdir().unwrap();
        let c = config(root.path());
        let command = case["verify"].as_str().unwrap();
        let bad = case_plan(&case, command);
        let good = corrected_plan(&case);
        let mut planner = Replay::new(&[&bad, &good, &good]);
        let plan = generate_step_plan(&mut planner, &bad.goal, &c)
            .unwrap_or_else(|e| panic!("{}: {e}", case["id"]));
        // One extra call inside the existing three attempts, no more.
        assert_eq!(planner.requests().len(), 2, "{}", case["id"]);
        let retry = planner.requests()[1]
            .iter()
            .map(|m| m.content.clone())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            retry.contains("failed deterministic lint on attempt 1/3"),
            "{}: {retry}",
            case["id"]
        );
        assert!(retry.contains(&format!("[{CATEGORY}]")), "{retry}");
        assert!(retry.contains(command), "{retry}");
        let all = plan.steps.iter().flat_map(|s| s.verify.iter());
        assert!(all.clone().all(|v| v != command), "{plan:?}");
        // The diagnostic event keeps the base name, fields, and message shape.
        let errors = shape_errors(&c);
        assert_eq!(errors.len(), 1, "{}: {errors:?}", case["id"]);
        let error = &errors[0];
        assert_eq!(error["planner_stage"], "lint");
        assert_eq!(error["planner_error_kind"], "planner_lint_error");
        assert_eq!(error["repair_attempt"], 1);
        assert_eq!(error["planner_error_message"], base_message(command));
        let mut keys = error
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        keys.sort();
        assert_eq!(keys, strings(&fixture()["base_planner_error_keys"]));
        // Order: diagnosis on attempt 1, then the repaired attempt, then the run.
        let log = events(&c);
        let diagnosed = log.iter().position(|e| e["event"] == "planner_error");
        let repaired = log
            .iter()
            .position(|e| e["event"] == "planner_raw_output_shape" && e["attempt"] == 2);
        assert!(diagnosed < repaired, "{log:?}");
        assert!(diagnosed.is_some(), "{log:?}");
        let check = plan.steps.iter().find(|s| s.id == "check-report").unwrap();
        let artifact = root.path().join(fixture()["artifact"].as_str().unwrap());
        assert!(!verify_step(root.path(), check).is_pass());
        std::fs::create_dir_all(artifact.parent().unwrap()).unwrap();
        std::fs::write(&artifact, "synthetic report\n").unwrap();
        assert!(verify_step(root.path(), check).is_pass());
    }
}

// Same shape as tests/verify_command_shape_lint.rs: attempt 1 is a sentence,
// attempts 2 and 3 carry another lint error, so nothing is adopted.
#[test]
fn sentence_then_other_lint_errors_stops_without_a_fallback_plan() {
    for case in negatives() {
        let root = tempfile::tempdir().unwrap();
        let c = config(root.path());
        let bad = case_plan(&case, case["verify"].as_str().unwrap());
        let other = case_plan(
            &case,
            fixture()["other_lint_error_verify"].as_str().unwrap(),
        );
        let mut planner = Replay::new(&[&bad, &other, &other]);
        let error = generate_step_plan(&mut planner, &bad.goal, &c)
            .expect_err("no plan with an unexecutable verify command may be returned");
        assert!(
            error
                .to_string()
                .starts_with("invalid StepPlan after corrective retries"),
            "{}: {error}",
            case["id"]
        );
        assert_eq!(planner.requests().len(), 3, "{}", case["id"]);
        let errors = shape_errors(&c);
        let attempts = errors
            .iter()
            .map(|e| e["repair_attempt"].as_u64().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(attempts, [1, 2, 3], "{}: {errors:?}", case["id"]);
        let log = events(&c);
        assert!(
            !log.iter()
                .any(|e| e["event"] == "planner_quality_retry_degraded"),
            "{log:?}"
        );
        assert!(!root.path().join(".commandagent/plans").exists());
        assert!(!root.path().join("out").exists());
    }
}

// A lint-passing plan with a retryable quality issue is the only fallback;
// later sentence plans never replace it.
#[test]
fn only_a_lint_passing_plan_becomes_the_fallback() {
    let case = &negatives()[0];
    let root = tempfile::tempdir().unwrap();
    let c = config(root.path());
    let mut valid = corrected_plan(case);
    valid.steps.push(step("summarize", "report", &[], &[]));
    let bad = case_plan(case, case["verify"].as_str().unwrap());
    let mut planner = Replay::new(&[&valid, &bad, &bad]);
    let plan = generate_step_plan(&mut planner, &valid.goal, &c).unwrap();
    assert_eq!(planner.requests().len(), 3);
    let commands = plan
        .steps
        .iter()
        .flat_map(|s| s.verify.clone())
        .collect::<Vec<_>>();
    assert!(
        commands
            .iter()
            .all(|v| v != case["verify"].as_str().unwrap()),
        "{plan:?}"
    );
    assert!(
        commands
            .iter()
            .any(|v| v == fixture()["corrected_verify"].as_str().unwrap()),
        "{plan:?}"
    );
    let log = events(&c);
    assert!(
        log.iter()
            .any(|e| e["event"] == "planner_quality_retry_degraded"),
        "{log:?}"
    );
}

fn obligation_workspace() -> (tempfile::TempDir, Config) {
    let root = tempfile::tempdir().unwrap();
    let obligation = &fixture()["obligation"];
    std::fs::write(root.path().join("check_ready.sh"), "grep -q READY \"$1\"\n").unwrap();
    let contract = serde_json::json!({
        "required_paths": [fixture()["artifact"]],
        "verify_commands": [obligation["contract_verify"]],
    });
    std::fs::write(
        root.path().join("completion-contract.json"),
        serde_json::to_vec_pretty(&contract).unwrap(),
    )
    .unwrap();
    let mut c = config(root.path());
    c.completion_contract_path = Some(root.path().join("completion-contract.json"));
    (root, c)
}

fn write_artifact(root: &Path, body: &str) {
    let artifact = root.join(fixture()["artifact"].as_str().unwrap());
    std::fs::create_dir_all(artifact.parent().unwrap()).unwrap();
    std::fs::write(artifact, body).unwrap();
}

fn plan_with_verify(case: &Value, verify: &[String]) -> StepPlan {
    let mut plan = corrected_plan(case);
    plan.steps.last_mut().unwrap().verify = verify.to_vec();
    plan
}

// The host-owned check stays the obligation: a repair that deletes it,
// empties it, or swaps in `true`, `echo`, or a weaker check never satisfies it.
#[test]
fn weak_repairs_never_satisfy_the_host_owned_obligation() {
    let case = &negatives()[0];
    let obligation = &fixture()["obligation"];
    let required = strings(&serde_json::json!([obligation["contract_verify"]]));
    for weak in obligation["weak_repairs"].as_array().unwrap() {
        let (root, c) = obligation_workspace();
        let bad = case_plan(case, case["verify"].as_str().unwrap());
        let repair = plan_with_verify(case, &strings(weak));
        let mut planner = Replay::new(&[&bad, &repair, &repair]);
        let outcome = generate_step_plan(&mut planner, &bad.goal, &c);
        assert!(planner.requests().len() <= 3, "{weak}");
        assert_eq!(shape_errors(&c)[0]["repair_attempt"], 1, "{weak}");
        let contract = CompletionContract::load_for_config(&c).unwrap().unwrap();
        assert_eq!(contract.verify_commands, required, "{weak}");
        write_artifact(root.path(), "draft\n");
        if let Ok(plan) = &outcome {
            for step in &plan.steps {
                assert!(
                    step.verify.iter().all(|v| !required.contains(v)),
                    "{weak}: {plan:?}"
                );
            }
        }
        assert!(!contract.verify(root.path()).is_pass(), "{weak}");
    }
}

#[test]
fn only_the_correct_repair_keeps_the_obligation_and_passes() {
    let case = &negatives()[0];
    let obligation = &fixture()["obligation"];
    let correct = strings(&obligation["correct_repair"]);
    let (root, c) = obligation_workspace();
    let bad = case_plan(case, case["verify"].as_str().unwrap());
    let repair = plan_with_verify(case, &correct);
    let mut planner = Replay::new(&[&bad, &repair, &repair]);
    let plan = generate_step_plan(&mut planner, &bad.goal, &c).unwrap();
    assert_eq!(planner.requests().len(), 2);
    let check = plan.steps.iter().find(|s| s.id == "check-report").unwrap();
    assert_eq!(check.verify, correct);
    let contract = CompletionContract::load_for_config(&c).unwrap().unwrap();
    write_artifact(root.path(), "draft\n");
    assert!(!verify_step(root.path(), check).is_pass());
    assert!(!contract.verify(root.path()).is_pass());
    write_artifact(root.path(), "READY\n");
    assert!(verify_step(root.path(), check).is_pass());
    assert!(contract.verify(root.path()).is_pass());
}

// A command that passes lint is not assumed to succeed: a missing tool, a
// missing interpreter, and a nonzero exit still fail at runtime, and the
// missing tool keeps its existing classification.
#[test]
fn commands_left_to_runtime_still_fail_there() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("fail.sh"), "exit 3\n").unwrap();
    for (command, false_negative) in [
        ("issue952-absent-tool --strict out/report.txt", true),
        ("issue952-absent-interpreter check.py", true),
        ("sh fail.sh", false),
    ] {
        let check = step("check-report", "verify", &[], &[command.to_string()]);
        let report = verify_step(root.path(), &check);
        assert!(!report.is_pass(), "{command}: {report:?}");
        if false_negative {
            assert_eq!(
                report.verifier_command_false_negatives.len(),
                1,
                "{report:?}"
            );
            assert!(
                report.verifier_command_false_negatives[0]
                    .reason
                    .contains("exit_127"),
                "{report:?}"
            );
            let reachability = assess_repair_reachability_at_root(
                Some(root.path()),
                &report,
                None,
                NodeDependencySetupAuthority::None,
                false,
            );
            assert_eq!(
                reachability_failure_kind(&reachability),
                "deterministic_verify_command_bug",
                "{command}"
            );
        } else {
            assert_eq!(report.command_failures.len(), 1, "{report:?}");
        }
    }
}

#[test]
fn corpus_fixture_matches_its_manifest() {
    let manifest: Value = serde_json::from_str(
        &std::fs::read_to_string(corpus_path("source-manifest.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(manifest["origin"], "synthetic");
    assert_eq!(
        manifest["base_sha"],
        "04e262c8bedc25bfbb033509eac7a368debc3339"
    );
    let bytes = std::fs::read(corpus_path("fixtures/events.json")).unwrap();
    assert_eq!(
        manifest["fixture_sha256"]["events.json"],
        format!("{:x}", Sha256::digest(bytes))
    );
    let fixture = fixture();
    assert_eq!(fixture["kind"], "synthetic");
    for language in ["english", "japanese", "mixed"] {
        assert!(
            negatives().iter().any(|c| c["language"] == language),
            "{language}"
        );
    }
    // The base accepted every sentence whose first word has an ASCII letter.
    for case in negatives() {
        let ascii = first_word(case["verify"].as_str().unwrap())
            .chars()
            .any(|ch| ch.is_ascii_alphanumeric());
        let expected = if ascii { "accepted" } else { "rejected" };
        assert_eq!(case["base_verdict"], expected, "{}", case["id"]);
    }
}
