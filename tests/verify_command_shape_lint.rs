//! Issue #907: verify commands that cannot execute as written are plan lint
//! errors, so a plan carrying one never becomes the fallback plan. Commands
//! that may execute, the retry budget, the quality gate, and the post-run
//! classification are unchanged.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use clap::Parser;
use commandagent::config::Config;
use commandagent::minimal_loop::dependency_setup::NodeDependencySetupAuthority;
use commandagent::minimal_loop::reachability::{
    assess_repair_reachability_at_root, reachability_failure_kind,
};
use commandagent::planner::generate_step_plan;
use commandagent::planner::lint::{
    PlanLintReport, PlanQualityContext, PlanQualitySeverity, lint_step_plan_report_with_workspace,
    step_plan_quality_report,
};
use commandagent::planner::step_plan::{PlanStep, StepPlan, parse_step_plan};
use commandagent::planner::verify::verify_step;
use commandagent::providers::{AssistantReply, ChatClient};
use commandagent::state::ConversationMessage;
use commandagent::tools::registry::ToolSpec;
use serde_json::Value;
use sha2::{Digest, Sha256};

const CORPUS: &str = "tests/corpus/apps/verify-command-shape-lint";
const CATEGORY: &str = "verify_command_shape";
const C_MID_SENTENCE: &str = "スクリプトを指定仕様に沿って実装できたことを確認する。";

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
        "issue907-replay"
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
            anyhow::bail!("issue907 replay exhausted");
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
    c.eval_events_path = Some(root.join(".commandagent/runs/issue907/events.jsonl"));
    std::fs::create_dir_all(c.eval_events_path.as_ref().unwrap().parent().unwrap()).unwrap();
    c
}

fn corpus_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(CORPUS)
        .join(name)
}

fn read(name: &str) -> String {
    std::fs::read_to_string(corpus_path(name)).unwrap()
}

fn json(name: &str) -> Value {
    serde_json::from_str(&read(name)).unwrap()
}

fn recorded() -> Vec<Value> {
    read("fixtures/recorded-observations.jsonl")
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

fn recorded_plan(name: &str) -> StepPlan {
    parse_step_plan(&read(name)).unwrap()
}

fn with_verify(plan: &StepPlan, step: &str, command: &str) -> StepPlan {
    let mut plan = plan.clone();
    let target = plan.steps.iter_mut().find(|s| s.id == step).unwrap();
    target.verify = vec![command.to_string()];
    plan
}

fn events(c: &Config) -> Vec<Value> {
    std::fs::read_to_string(c.eval_events_path.as_ref().unwrap())
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .collect()
}

fn c_mid_workspace() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(
        root.path().join("products.csv"),
        "id,name,price,in_stock\n1,pen,120,true\n",
    )
    .unwrap();
    root
}

fn shape_errors(report: &PlanLintReport) -> Vec<String> {
    report
        .errors
        .iter()
        .filter(|e| e.category == CATEGORY)
        .map(|e| e.message.clone())
        .collect()
}

fn sha256_hex(path: &Path) -> String {
    format!("{:x}", Sha256::digest(std::fs::read(path).unwrap()))
}

fn step(id: &str, kind: &str, paths: &[&str], verify: &[&str]) -> PlanStep {
    PlanStep {
        id: id.into(),
        kind: kind.into(),
        expected_result: "pass".into(),
        instruction: format!("Carry out {id}."),
        expected_paths: paths.iter().map(|p| (*p).into()).collect(),
        verify: verify.iter().map(|v| (*v).into()).collect(),
    }
}

fn verify_plan(producer_paths: Vec<&str>, command: &str) -> StepPlan {
    StepPlan {
        goal: "Create the artifact.".into(),
        steps: vec![
            step("make", "implement", &producer_paths, &[]),
            step("check", "verify", &[], &[command]),
        ],
    }
}

fn strings(value: &Value) -> Vec<&str> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect()
}

#[test]
fn corpus_fixtures_match_the_manifest_and_the_recorded_rows() {
    let manifest = json("source-manifest.json");
    for file in manifest["files"].as_array().unwrap() {
        let name = file["file"].as_str().unwrap();
        assert_eq!(
            sha256_hex(&corpus_path(name)),
            file["sha256"].as_str().unwrap(),
            "{name}"
        );
    }
    for row in recorded() {
        assert_eq!(row["kind"], "recorded");
        assert_eq!(row["grade"]["verdict"], "correct");
        assert_eq!(row["run_status"], "failed");
        let name = row["plan"].as_str().unwrap();
        let declared = row["declared_command"].as_str().unwrap();
        let line = read(name)
            .lines()
            .nth(row["plan_line"].as_u64().unwrap() as usize - 1)
            .unwrap()
            .to_string();
        assert!(
            line.contains(&serde_json::to_string(declared).unwrap()),
            "{line}"
        );
        let plan = recorded_plan(name);
        let owner = plan.steps.iter().find(|s| s.id == row["step"]).unwrap();
        assert_eq!(owner.verify, [declared]);
    }
    assert_eq!(json("fixtures/synthetic-cases.json")["kind"], "synthetic");
}

// Supplement 1: the attempt-1 sentence plan is a lint error, so it never
// becomes last_valid_plan and the run fails at the plan stage.
#[test]
fn c_mid_sentence_plan_is_not_adopted_as_the_fallback_plan() {
    let root = c_mid_workspace();
    let c = config(root.path());
    let original = recorded_plan("fixtures/recorded-c-mid-plan.yaml");
    let other = &json("fixtures/synthetic-cases.json")["c_mid_other_lint_error"];
    let broken = with_verify(
        &original,
        other["step"].as_str().unwrap(),
        other["verify"].as_str().unwrap(),
    );
    let mut planner = Replay::new(&[&original, &broken, &broken]);
    let error = generate_step_plan(&mut planner, &original.goal, &c)
        .expect_err("a plan with an unexecutable verify command must not be returned");
    assert!(
        error
            .to_string()
            .starts_with("invalid StepPlan after corrective retries"),
        "{error}"
    );
    assert_eq!(planner.requests().len(), 3);
    let log = events(&c);
    assert!(
        !log.iter()
            .any(|e| e["event"] == "planner_quality_retry_degraded"),
        "{log:?}"
    );
    assert!(
        !log.iter().any(|e| e["event"] == "planner_quality_issue"),
        "{log:?}"
    );
    assert!(!root.path().join(".commandagent/plans").exists());
    // D2: the existing lint retry feedback names the step, the command, and
    // an executable replacement.
    let retry = planner.requests()[1]
        .iter()
        .map(|m| m.content.clone())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        retry.contains("failed deterministic lint on attempt 1/3"),
        "{retry}"
    );
    assert!(retry.contains("[verify_command_shape]"), "{retry}");
    assert!(retry.contains("step implement-converter"), "{retry}");
    assert!(retry.contains(C_MID_SENTENCE), "{retry}");
    assert!(retry.contains("python3 <script> <args>"), "{retry}");
}

#[test]
fn c_mid_corrected_command_on_attempt_2_is_accepted() {
    let root = c_mid_workspace();
    let c = config(root.path());
    let original = recorded_plan("fixtures/recorded-c-mid-plan.yaml");
    let fixed = &json("fixtures/synthetic-cases.json")["c_mid_corrected"];
    let command = fixed["verify"].as_str().unwrap();
    let corrected = with_verify(&original, fixed["step"].as_str().unwrap(), command);
    let mut planner = Replay::new(&[&original, &corrected, &corrected]);
    let plan = generate_step_plan(&mut planner, &original.goal, &c).unwrap();
    assert_eq!(planner.requests().len(), 2);
    let owner = plan
        .steps
        .iter()
        .find(|s| s.id == "implement-converter")
        .unwrap();
    assert_eq!(owner.verify, [command]);
}

// Supplement 2: the a-mid declaration is a lint error; the corrected forms pass.
#[test]
fn a_mid_operand_order_is_a_lint_error_and_corrected_forms_pass() {
    let root = tempfile::tempdir().unwrap();
    let original = recorded_plan("fixtures/recorded-a-mid-plan.yaml");
    let report = lint_step_plan_report_with_workspace(&original, Some(root.path()));
    let found = shape_errors(&report);
    assert_eq!(found.len(), 1, "{report:?}");
    assert!(found[0].contains("step verify-file-existence"), "{found:?}");
    assert!(
        found[0].contains("operand `file_index.html` comes before file operator `-f`"),
        "{found:?}"
    );
    for command in strings(&json("fixtures/synthetic-cases.json")["a_mid_corrected"]) {
        let plan = with_verify(&original, "verify-file-existence", command);
        let report = lint_step_plan_report_with_workspace(&plan, Some(root.path()));
        assert!(report.is_pass(), "{command}: {report:?}");
    }
}

// Supplement 3: commands that may execute are not newly rejected.
#[test]
fn d3_positive_commands_are_not_rejected() {
    let cases = json("fixtures/synthetic-cases.json");
    for case in cases["positive"].as_array().unwrap() {
        let root = tempfile::tempdir().unwrap();
        if let Some(existing) = case["existing"].as_str() {
            std::fs::write(root.path().join(existing), "#!/bin/sh\nexit 0\n").unwrap();
        }
        let command = case["verify"].as_str().unwrap();
        let plan = verify_plan(strings(&case["producer_paths"]), command);
        let report = lint_step_plan_report_with_workspace(&plan, Some(root.path()));
        assert!(
            shape_errors(&report).is_empty(),
            "{}: {report:?}",
            case["id"]
        );
    }
}

// Supplement 4: the unexecutable shapes are lint errors with their reason.
#[test]
fn negative_commands_are_lint_errors() {
    let cases = json("fixtures/synthetic-cases.json");
    for case in cases["negative"].as_array().unwrap() {
        let root = tempfile::tempdir().unwrap();
        let command = case["verify"].as_str().unwrap();
        let plan = verify_plan(vec!["x"], command);
        let report = lint_step_plan_report_with_workspace(&plan, Some(root.path()));
        let found = shape_errors(&report);
        assert_eq!(found.len(), 1, "{}: {report:?}", case["id"]);
        assert!(
            found[0].contains(case["reason"].as_str().unwrap()),
            "{}: {found:?}",
            case["id"]
        );
        assert!(found[0].contains("step check"), "{found:?}");
    }
}

// Supplement 5: post-run classification and the quality gate are unchanged.
#[test]
fn exit_127_and_usage_failures_stay_verify_command_bugs() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(
        root.path().join("verify.sh"),
        "#!/bin/sh\necho 'usage: verify.sh <jsonl_file>' >&2\nexit 2\n",
    )
    .unwrap();
    for command in [C_MID_SENTENCE, "sh verify.sh"] {
        let report = verify_step(root.path(), &step("check", "verify", &[], &[command]));
        assert_eq!(
            report.verifier_command_false_negatives.len(),
            1,
            "{command}: {report:?}"
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
    }
    let report = verify_step(
        root.path(),
        &step("check", "verify", &[], &[C_MID_SENTENCE]),
    );
    assert!(
        report.verifier_command_false_negatives[0]
            .reason
            .contains("exit_127"),
        "{report:?}"
    );
}

#[test]
fn a_mid_runtime_failure_stays_an_ordinary_command_failure() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("index.html"), "<!doctype html>\n").unwrap();
    let command = "[ \"file_index.html\" -f ]";
    let report = verify_step(root.path(), &step("check", "verify", &[], &[command]));
    assert!(
        report.verifier_command_false_negatives.is_empty(),
        "{report:?}"
    );
    assert_eq!(report.command_failures.len(), 1, "{report:?}");
    let reachability = assess_repair_reachability_at_root(
        Some(root.path()),
        &report,
        None,
        NodeDependencySetupAuthority::None,
        false,
    );
    assert!(reachability.reachable, "{reachability:?}");
}

#[test]
fn c_mid_quality_judgement_is_unchanged() {
    let plan = recorded_plan("fixtures/recorded-c-mid-plan.yaml");
    let context = PlanQualityContext {
        profile: "generic".into(),
        ..PlanQualityContext::default()
    };
    let report = step_plan_quality_report(&plan, &context);
    let coupling = report
        .issues
        .iter()
        .find(|i| i.category == "verify_artifact_coupling")
        .unwrap_or_else(|| panic!("{report:?}"));
    assert_eq!(coupling.severity, PlanQualitySeverity::RetryableQuality);
    assert_eq!(coupling.evidence.as_deref(), Some(C_MID_SENTENCE));
}
