//! Issue #908: a verify command that runs a Python script the same plan
//! creates (`python3 smoke_check.py sample.txt`) is strong verification in the
//! plan quality report, like `node smoke-check.js`. Inline code, scripts the
//! plan does not create, paths outside the workspace, and compound commands
//! stay weak; lint, policy, admission, and the budget are unchanged.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use clap::Parser;
use commandagent::config::Config;
use commandagent::planner::generate_step_plan;
use commandagent::planner::lint::{
    PlanQualityContext, PlanQualityReport, PlanQualitySeverity, step_plan_quality_report,
};
use commandagent::planner::step_plan::{PlanStep, StepPlan};
use commandagent::providers::{AssistantReply, ChatClient};
use commandagent::state::ConversationMessage;
use commandagent::tools::registry::ToolSpec;
use serde_json::Value;
use sha2::{Digest, Sha256};

const CORPUS: &str = "tests/corpus/apps/python-script-strong-verify";
const SOURCE: &str = "a520c0847829b783ece38faa8c8227894e0f9d2a";
const WEAK: &str = "weak_code_verify";
const WEAK_MESSAGE: &str = "code task lacks test, build, smoke, or compile verification";
const CONTEXT: &str = "Original requirement context:";

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

    fn requests(&self) -> usize {
        self.requests.lock().unwrap().len()
    }
}

impl ChatClient for Replay {
    fn label(&self) -> &str {
        "issue908-replay"
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
            anyhow::bail!("issue908 replay exhausted");
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
    c.eval_events_path = Some(root.join(".commandagent/runs/issue908/events.jsonl"));
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

fn recorded() -> Vec<Value> {
    read("fixtures/recorded-attempts.jsonl")
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

fn synthetic() -> Value {
    serde_json::from_str(&read("fixtures/synthetic-cases.json")).unwrap()
}

fn synthetic_plan() -> StepPlan {
    serde_json::from_value(synthetic()["plan"].clone()).unwrap()
}

fn events(c: &Config) -> Vec<Value> {
    std::fs::read_to_string(c.eval_events_path.as_ref().unwrap())
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .collect()
}

fn sha256_hex(path: &Path) -> String {
    format!("{:x}", Sha256::digest(std::fs::read(path).unwrap()))
}

fn with_verify(plan: &StepPlan, step: &str, command: &str) -> StepPlan {
    let mut plan = plan.clone();
    let target = plan.steps.iter_mut().find(|s| s.id == step).unwrap();
    target.verify = vec![command.to_string()];
    plan
}

fn quality(plan: &StepPlan) -> PlanQualityReport {
    let context = PlanQualityContext {
        profile: "generic".into(),
        ..PlanQualityContext::default()
    };
    step_plan_quality_report(plan, &context)
}

fn weak_severity(report: &PlanQualityReport) -> Option<PlanQualitySeverity> {
    report
        .issues
        .iter()
        .find(|i| i.category == WEAK)
        .map(|i| i.severity)
}

fn commands(key: &str) -> Vec<(String, String)> {
    synthetic()[key]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            (
                c["id"].as_str().unwrap().to_string(),
                c["verify"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

// The accepted owner split (#906), applied to a recorded mixed owner: the
// verifier script and the other files get their own Implement steps, each with
// a role sentence and the complete original instruction.
fn template_split(original: &StepPlan, verifier: &str) -> StepPlan {
    let mut steps = Vec::new();
    for step in &original.steps {
        if !step.expected_paths.iter().any(|p| p == verifier) || step.expected_paths.len() < 2 {
            steps.push(step.clone());
            continue;
        }
        let others: Vec<_> = step
            .expected_paths
            .iter()
            .filter(|p| *p != verifier)
            .cloned()
            .collect();
        let owner = |id: String, paths: Vec<String>| PlanStep {
            id,
            kind: step.kind.clone(),
            expected_result: step.expected_result.clone(),
            instruction: format!(
                "Only create {} in this step. {CONTEXT} {}",
                paths.join(", "),
                step.instruction
            ),
            expected_paths: paths,
            verify: Vec::new(),
        };
        steps.push(owner(
            format!("{}-verifier", step.id),
            vec![verifier.to_string()],
        ));
        steps.push(owner(step.id.clone(), others));
        assert!(step.verify.is_empty(), "{}", step.id);
    }
    StepPlan {
        goal: original.goal.clone(),
        steps,
    }
}

fn recorded_split(row: &Value) -> (StepPlan, StepPlan, Value) {
    let original: StepPlan =
        serde_json::from_value(row["first_admission"]["original_scope"].clone()).unwrap();
    let split = synthetic()["recorded_splits"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["recorded"] == row["id"])
        .unwrap()
        .clone();
    let proposal = template_split(&original, split["verifier_path"].as_str().unwrap());
    (original, proposal, split)
}

#[test]
fn corpus_fixtures_match_the_manifest_and_the_recorded_rows() {
    let manifest: Value = serde_json::from_str(&read("source-manifest.json")).unwrap();
    let digests = manifest["fixture_sha256"].as_object().unwrap();
    assert_eq!(digests.len(), 2);
    for (name, expected) in digests {
        assert_eq!(
            &sha256_hex(&corpus_path(&format!("fixtures/{name}"))),
            expected.as_str().unwrap(),
            "{name}"
        );
    }
    let expected = [
        (
            "recorded-Aa-med-b-mid",
            ["policy", "admission", "admission"],
        ),
        ("recorded-C3-c-mid", ["policy", "quality", "admission"]),
    ];
    let rows = recorded();
    assert_eq!(rows.len(), expected.len());
    for (row, (id, classes)) in rows.iter().zip(expected) {
        assert_eq!(row["id"], id);
        assert_eq!(row["kind"], "recorded");
        assert_eq!(row["source_code"], SOURCE);
        assert_eq!(row["event_sha256"].as_str().unwrap().len(), 64);
        let sequence = row["sequence"].as_array().unwrap();
        let actual: Vec<_> = sequence
            .iter()
            .filter(|s| s["attempt"].is_number())
            .map(|s| s["class"].as_str().unwrap())
            .collect();
        assert_eq!(actual, classes, "{id}");
        let lines: Vec<_> = sequence
            .iter()
            .map(|s| s["line"].as_u64().unwrap())
            .collect();
        assert!(lines.windows(2).all(|w| w[0] < w[1]), "{id}");
        assert_eq!(sequence.last().unwrap()["status"], "failed", "{id}");
        // Attempt 1 checked inline with python3 -c and hit the policy.
        let policy = &row["attempt1_policy"];
        assert_eq!(policy["line"], sequence[0]["line"], "{id}");
        assert_eq!(policy["reason"], sequence[0]["reason"], "{id}");
        assert!(
            policy["original_command"]
                .as_str()
                .unwrap()
                .starts_with("python3 -c "),
            "{id}"
        );
        // The first recorded full plan runs a script its Implement step creates.
        let original: StepPlan =
            serde_json::from_value(row["first_admission"]["original_scope"].clone()).unwrap();
        let verify: Vec<_> = original
            .steps
            .iter()
            .flat_map(|s| s.verify.iter())
            .collect();
        assert_eq!(verify.len(), 1, "{id}");
        let script = verify[0].split_whitespace().nth(1).unwrap();
        assert!(verify[0].starts_with("python3 "), "{id}");
        assert!(
            original
                .steps
                .iter()
                .any(|s| s.kind == "implement" && s.expected_paths.iter().any(|p| p == script)),
            "{id}"
        );
    }
    assert_eq!(synthetic()["kind"], "synthetic");
}

// Supplement 1: a plan-created script run by the Verify step is strong.
#[test]
fn plan_created_python_script_is_strong_verification() {
    let plan = synthetic_plan();
    for (id, command) in commands("strong") {
        let report = quality(&with_verify(&plan, "verify-topwords", &command));
        assert_eq!(weak_severity(&report), None, "{id}: {report:?}");
        assert!(!report.has_retryable_quality(), "{id}: {report:?}");
        assert!(
            report.issues.iter().all(|i| i.message != WEAK_MESSAGE),
            "{id}"
        );
    }
    // The planner accepts the plan on attempt 1 without a quality retry.
    let root = tempfile::tempdir().unwrap();
    let c = config(root.path());
    let mut planner = Replay::new(&[&plan, &plan, &plan]);
    let accepted = generate_step_plan(&mut planner, &plan.goal, &c).unwrap();
    assert_eq!(planner.requests(), 1);
    assert_eq!(accepted.steps.last().unwrap().verify, plan.steps[2].verify);
    let log = events(&c);
    assert!(
        !log.iter().any(|e| e["event"] == "planner_quality_retry"),
        "{log:?}"
    );
}

// Supplement 2: inline code, scripts the plan does not create, paths outside
// the workspace, a bare interpreter, modules, and compound commands stay weak.
#[test]
fn other_python_commands_stay_weak_verification() {
    let plan = synthetic_plan();
    for (id, command) in commands("weak") {
        let report = quality(&with_verify(&plan, "verify-topwords", &command));
        assert_eq!(
            weak_severity(&report),
            Some(PlanQualitySeverity::RetryableQuality),
            "{id}: {report:?}"
        );
        let issue = report.issues.iter().find(|i| i.category == WEAK).unwrap();
        assert_eq!(issue.message, WEAK_MESSAGE, "{id}");
        assert_eq!(issue.evidence.as_deref(), Some(command.as_str()), "{id}");
    }
    // A script that only exists in the workspace, not created by the plan,
    // still spends a quality retry.
    let mut external = synthetic_plan();
    external.steps.remove(1);
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("smoke_check.py"), "print('ok')\n").unwrap();
    std::fs::write(root.path().join("sample.txt"), "the cat\n").unwrap();
    let c = config(root.path());
    let mut planner = Replay::new(&[&external, &external, &external]);
    let _ = generate_step_plan(&mut planner, &external.goal, &c);
    assert!(
        events(&c)
            .iter()
            .any(|e| e["event"] == "planner_quality_retry" && e["repair_attempt"] == 1),
        "{:?}",
        events(&c)
    );
}

// Supplement 3: node, unittest and pytest are judged as before.
#[test]
fn existing_strong_commands_are_unchanged() {
    let plan = synthetic_plan();
    for case in synthetic()["unchanged"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        let command = case["verify"].as_str().unwrap();
        assert_eq!(case["strong"], true, "{id}");
        let report = quality(&with_verify(&plan, "verify-topwords", command));
        assert_eq!(weak_severity(&report), None, "{id}: {report:?}");
    }
    // Without the plan-created script owner, node is still strong and a Python
    // script is not.
    let mut external = synthetic_plan();
    external.steps.remove(1);
    let node = quality(&with_verify(&external, "verify-topwords", "node x.js"));
    assert_eq!(weak_severity(&node), None, "{node:?}");
    let python = quality(&with_verify(
        &external,
        "verify-topwords",
        "python3 smoke_check.py sample.txt",
    ));
    assert_eq!(
        weak_severity(&python),
        Some(PlanQualitySeverity::RetryableQuality),
        "{python:?}"
    );
}

// Supplement 4: the recorded plan shapes. Split as #906 guides, the recorded
// script check is strong, so the split is accepted on attempt 2 with no quality
// retry (before #908 it spent a quality retry and needed attempt 3).
// Admission and the verify policy decide as recorded.
#[test]
fn recorded_plan_shapes_no_longer_spend_a_quality_retry() {
    for row in recorded() {
        let id = row["id"].as_str().unwrap();
        let (original, proposal, split) = recorded_split(&row);
        let report = quality(&proposal);
        assert_eq!(weak_severity(&report), None, "{id}: {report:?}");
        assert!(!report.has_retryable_quality(), "{id}: {report:?}");

        let root = tempfile::tempdir().unwrap();
        let c = config(root.path());
        let mut planner = Replay::new(&[&original, &proposal, &proposal]);
        let plan = generate_step_plan(&mut planner, &original.goal, &c)
            .unwrap_or_else(|e| panic!("{id}: {e}"));
        assert_eq!(planner.requests(), 2, "{id}");
        let ids: Vec<_> = plan.steps.iter().map(|s| s.id.as_str()).collect();
        let expected: Vec<_> = split["admitted_step_ids"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(ids, expected, "{id}");
        let log = events(&c);
        assert!(
            !log.iter().any(|e| e["event"] == "planner_quality_retry"),
            "{id}"
        );
        // Admission rejects the recorded mixed owner with the recorded reason.
        let admissions: Vec<_> = log
            .iter()
            .filter(|e| e["event"] == "recovery_verifier_plan_admission")
            .collect();
        assert_eq!(admissions.len(), 1, "{id}");
        assert_eq!(admissions[0]["planner_attempt"], 1, "{id}");
        assert_eq!(admissions[0]["status"], "retry", "{id}");
        let mixes = row["sequence"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| {
                s["class"] == "admission"
                    && s["reason"]
                        .as_str()
                        .unwrap()
                        .contains("verifier producer mixes")
            })
            .unwrap();
        assert_eq!(admissions[0]["reason"], mixes["reason"], "{id}");
    }
}

#[test]
fn recorded_inline_check_still_hits_the_verify_policy() {
    for row in recorded() {
        let id = row["id"].as_str().unwrap();
        let (_, proposal, _) = recorded_split(&row);
        let policy = &row["attempt1_policy"];
        let verify_step = proposal.steps.last().unwrap().id.clone();
        let inline = with_verify(
            &proposal,
            &verify_step,
            policy["original_command"].as_str().unwrap(),
        );
        let root = tempfile::tempdir().unwrap();
        let c = config(root.path());
        let mut planner = Replay::new(&[&inline, &proposal, &proposal]);
        let plan = generate_step_plan(&mut planner, &inline.goal, &c)
            .unwrap_or_else(|e| panic!("{id}: {e}"));
        assert_eq!(planner.requests(), 2, "{id}");
        assert_eq!(
            plan.steps.last().unwrap().verify,
            proposal.steps.last().unwrap().verify
        );
        let log = events(&c);
        let errors: Vec<_> = log
            .iter()
            .filter(|e| e["event"] == "planner_error" && e["planner_stage"] == "verify_policy")
            .collect();
        assert_eq!(errors.len(), 1, "{id}: {log:?}");
        assert_eq!(errors[0]["repair_attempt"], 1, "{id}");
        assert_eq!(errors[0]["planner_error_message"], policy["reason"], "{id}");
        assert!(
            !log.iter().any(|e| e["event"] == "planner_quality_retry"),
            "{id}"
        );
    }
}
