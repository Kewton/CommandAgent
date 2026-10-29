//! Issue #906: the five recorded generic runs that exhausted verifier plan
//! admission still stop with the recorded reasons, and the owner split the
//! retry feedback now shows, applied to the recorded original scope, is
//! admitted on the next attempt. Admission rules and the budget are unchanged.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use clap::Parser;
use commandagent::config::Config;
use commandagent::planner::generate_step_plan;
use commandagent::planner::step_plan::{PlanStep, StepPlan};
use commandagent::providers::{AssistantReply, ChatClient};
use commandagent::state::ConversationMessage;
use commandagent::tools::registry::ToolSpec;
use serde_json::Value;
use sha2::{Digest, Sha256};

const CORPUS: &str = "tests/corpus/apps/verifier-separation-guidance";
const EXHAUSTED: &str = "verifier plan admission exhausted existing 3-attempt planner budget";
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

    fn requests(&self) -> Vec<Vec<ConversationMessage>> {
        self.requests.lock().unwrap().clone()
    }
}

impl ChatClient for Replay {
    fn label(&self) -> &str {
        "issue906-replay"
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
            anyhow::bail!("issue906 replay exhausted");
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
    c.eval_events_path = Some(root.join(".commandagent/runs/issue906/events.jsonl"));
    std::fs::create_dir_all(c.eval_events_path.as_ref().unwrap().parent().unwrap()).unwrap();
    c
}

fn corpus_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(CORPUS)
        .join(name)
}

fn jsonl(name: &str) -> Vec<Value> {
    std::fs::read_to_string(corpus_path(name))
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

fn synthetic() -> Value {
    serde_json::from_str(
        &std::fs::read_to_string(corpus_path("fixtures/synthetic-cases.json")).unwrap(),
    )
    .unwrap()
}

fn admissions(c: &Config) -> Vec<Value> {
    std::fs::read_to_string(c.eval_events_path.as_ref().unwrap())
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .filter(|e| e["event"] == "recovery_verifier_plan_admission")
        .collect()
}

fn scope(record: &Value, key: &str) -> StepPlan {
    serde_json::from_value(record[key].clone()).unwrap()
}

// The accepted owner split, applied mechanically to a recorded mixed owner:
// role sentence + complete original instruction on each owner, original
// expected_result, and the owner's checks moved unchanged to a later Verify.
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
        if !step.verify.is_empty() {
            steps.push(PlanStep {
                id: format!("{}-checks", step.id),
                kind: "verify".into(),
                expected_result: step.expected_result.clone(),
                instruction:
                    "Run the listed deterministic verification commands without changing files."
                        .into(),
                expected_paths: Vec::new(),
                verify: step.verify.clone(),
            });
        }
    }
    StepPlan {
        goal: original.goal.clone(),
        steps,
    }
}

fn sha256_hex(path: &Path) -> String {
    format!("{:x}", Sha256::digest(std::fs::read(path).unwrap()))
}

#[test]
fn corpus_fixtures_match_the_manifest_and_the_recorded_attempt_table() {
    let manifest: Value = serde_json::from_str(
        &std::fs::read_to_string(corpus_path("source-manifest.json")).unwrap(),
    )
    .unwrap();
    let digests = manifest["fixture_sha256"].as_object().unwrap();
    assert_eq!(digests.len(), 3);
    for (name, expected) in digests {
        assert_eq!(
            &sha256_hex(&corpus_path(&format!("fixtures/{name}"))),
            expected.as_str().unwrap(),
            "{name}"
        );
    }
    // Attempt classes per run, from the S-143a section 3 table.
    let expected = [
        (
            "recorded-Aa-med-a-mid",
            ["quality", "admission", "admission"],
        ),
        (
            "recorded-Aa-high-a-mid",
            ["quality", "admission", "admission"],
        ),
        (
            "recorded-Aa-max-a-mid",
            ["quality", "admission", "admission"],
        ),
        (
            "recorded-Aa-med-b-mid",
            ["policy", "admission", "admission"],
        ),
        ("recorded-C3-c-mid", ["policy", "quality", "admission"]),
    ];
    let attempts = jsonl("fixtures/recorded-attempts.jsonl");
    assert_eq!(attempts.len(), expected.len());
    for (row, (id, classes)) in attempts.iter().zip(expected) {
        assert_eq!(row["id"], id);
        assert_eq!(row["kind"], "recorded");
        assert_eq!(
            row["source_code"],
            "a520c0847829b783ece38faa8c8227894e0f9d2a"
        );
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
        let stop = &sequence[sequence.len() - 2];
        assert_eq!(stop["class"], "stop");
        assert!(
            stop["reason"].as_str().unwrap().starts_with(EXHAUSTED),
            "{id}"
        );
        assert_eq!(sequence.last().unwrap()["status"], "failed");
    }
    let records = jsonl("fixtures/recorded-admissions.jsonl");
    assert_eq!(records.len(), 5);
    for (record, attempt) in records.iter().zip(&attempts) {
        assert_eq!(record["id"], attempt["id"]);
        assert_eq!(record["event_sha256"], attempt["event_sha256"]);
        assert_eq!(record["planner_attempt"], 3);
        assert_eq!(record["status"], "exhausted");
        for key in [
            "original_scope",
            "model_proposal",
            "host_augmented_proposal",
        ] {
            assert!(record[key]["steps"].is_array(), "{key}");
        }
    }
    let cases = synthetic();
    assert_eq!(cases["kind"], "synthetic");
}

#[test]
fn recorded_final_proposals_stay_rejected_with_the_recorded_reason() {
    for record in jsonl("fixtures/recorded-admissions.jsonl") {
        let id = record["id"].as_str().unwrap();
        let root = tempfile::tempdir().unwrap();
        let c = config(root.path());
        let original = scope(&record, "original_scope");
        let model = scope(&record, "model_proposal");
        let mut planner = Replay::new(&[&original, &model, &model]);
        let error = generate_step_plan(&mut planner, &original.goal, &c)
            .expect_err("a recorded rejected proposal must stay rejected");
        assert_eq!(planner.requests().len(), 3, "{id}");
        let reason = record["reason"].as_str().unwrap();
        assert_eq!(error.to_string(), format!("{EXHAUSTED}: {reason}"), "{id}");
        let last = admissions(&c).pop().unwrap();
        assert_eq!(last["status"], "exhausted", "{id}");
        assert_eq!(last["reason"], reason, "{id}");
        assert!(!root.path().join(".commandagent/plans").exists());
    }
}

#[test]
fn template_split_of_each_recorded_scope_is_admitted_on_the_next_attempt() {
    let cases = synthetic();
    let splits = cases["template_splits"].as_array().unwrap();
    assert_eq!(splits.len(), 5);
    for (record, split) in jsonl("fixtures/recorded-admissions.jsonl")
        .iter()
        .zip(splits)
    {
        let id = record["id"].as_str().unwrap();
        assert_eq!(split["recorded"], id);
        let root = tempfile::tempdir().unwrap();
        let c = config(root.path());
        let original = scope(record, "original_scope");
        let proposal = template_split(&original, split["verifier_path"].as_str().unwrap());
        let mut planner = Replay::new(&[&original, &proposal, &proposal]);
        let result = generate_step_plan(&mut planner, &original.goal, &c);
        // Admission accepts the split: its only event is the attempt-1 retry.
        let statuses: Vec<_> = admissions(&c)
            .iter()
            .map(|e| (e["planner_attempt"].clone(), e["status"].clone()))
            .collect();
        assert_eq!(statuses, [(1.into(), "retry".into())], "{id}");
        let requests = planner.requests();
        let after = split["after_admission"].as_str().unwrap();
        if after == "ready" {
            let plan = result.unwrap_or_else(|e| panic!("{id}: {e}"));
            assert_eq!(requests.len(), 2, "{id}");
            let ids: Vec<_> = plan.steps.iter().map(|s| s.id.as_str()).collect();
            let expected: Vec<_> = split["admitted_step_ids"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap())
                .collect();
            assert_eq!(ids, expected, "{id}");
        } else {
            // The recorded check itself is weak; the unchanged quality gate
            // still uses the same budget after admission accepts the split.
            assert_eq!(after, "quality_retry", "{id}");
            let log = std::fs::read_to_string(c.eval_events_path.as_ref().unwrap()).unwrap();
            assert!(
                log.lines()
                    .map(|l| serde_json::from_str::<Value>(l).unwrap())
                    .any(|e| e["event"] == "planner_quality_retry" && e["repair_attempt"] == 2),
                "{id}"
            );
            assert_eq!(requests.len(), 3, "{id}: {result:?}");
        }
        // The feedback that led there showed the same form with the full text.
        let mixed = original
            .steps
            .iter()
            .find(|s| s.expected_paths.len() > 1)
            .unwrap();
        let feedback: String = requests[1]
            .iter()
            .filter(|m| m.role == "user")
            .map(|m| m.content.clone())
            .collect();
        assert!(
            feedback.contains(&format!("{CONTEXT} {}\"", mixed.instruction)),
            "{id}"
        );
        assert!(
            !feedback.contains("do not duplicate the combined instruction"),
            "{id}"
        );
    }
}
