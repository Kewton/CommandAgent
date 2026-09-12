use super::*;
use crate::planner::recovery_contract_authority as authority;
use crate::planner::recovery_contract_authority::verifier_obligations as scope;
use crate::planner::recovery_inspection::verifier_obligations::{BindingFailure, FailureClass};
use crate::providers::{AssistantReply, ChatClient};
use crate::state::{ConversationMessage, ToolCall};
use crate::tools::registry::ToolSpec;
use clap::Parser;
use std::collections::VecDeque;
use std::path::Path;
use std::sync::{Arc, Mutex};

const FIXTURE: &str = "tests/corpus/apps/issue466-verifier-obligations";
const SCRIPT: &str = "smoke-check.js";
const SECOND: &str = "other-check.js";
const INSTRUCTION: &str = "Create smoke-check.js to read value.txt and fail nonzero unless its trimmed value equals 42. Preserve every assertion.";

fn config(root: &Path) -> Config {
    let mut c =
        Config::from_cli(crate::cli::Cli::parse_from(["commandagent", "--ux-demo"])).unwrap();
    c.workspace_root = root.to_path_buf();
    c.profile = "generic".into();
    c.profile_explicit = true;
    c.offline = true;
    c.yes = true;
    c.max_iterations = 4;
    c.eval_events_path = Some(root.join(".commandagent/runs/issue466/events.jsonl"));
    std::fs::create_dir_all(c.eval_events_path.as_ref().unwrap().parent().unwrap()).unwrap();
    c
}
fn step(id: &str, kind: &str, paths: &[&str]) -> PlanStep {
    PlanStep {
        id: id.into(),
        kind: kind.into(),
        expected_result: "pass".into(),
        instruction: INSTRUCTION.into(),
        expected_paths: paths.iter().map(|p| (*p).into()).collect(),
        verify: vec![],
    }
}
fn plan(steps: Vec<PlanStep>) -> StepPlan {
    StepPlan {
        goal: "Create verifier artifacts".into(),
        steps,
    }
}
fn generated_contract(c: &Config) -> std::path::PathBuf {
    let path = crate::planner::completion_contract_path::generated_path(
        &c.workspace_root,
        c.eval_events_path.as_deref(),
        "completion-contract-ultra-plan-run.json",
    );
    std::fs::write(&path, json!({"goal":"Create verifier artifacts", "profile":"generic",
        "required_paths":[SCRIPT,"value.txt"], "verify_commands":[format!("node {SCRIPT}"), format!("node {SECOND}")],
        "protected_paths":["value.txt"]}).to_string()).unwrap();
    authority::record_generated_contract(c, "ultra-plan-run", &path);
    path
}
fn bind_context(c: &Config) {
    crate::planner::recovery_inspection::bind_context(
        c,
        c,
        Some("create"),
        &crate::planner::repair::RecoveryHandoff {
            original_goal: "Create verifier artifacts".into(),
            failure_kind: "step_verification_failed".into(),
            failure_evidence: vec!["missing verifier".into()],
            ..Default::default()
        },
        None,
    )
    .unwrap();
}
fn registered(
    paths: &[&str],
    existing: &[&str],
) -> (tempfile::TempDir, Config, authority::RunAuthorityGuard) {
    let root = tempfile::tempdir().unwrap();
    let mut c = config(root.path());
    let guard = authority::begin_run(&c);
    let contract = generated_contract(&c);
    std::fs::write(root.path().join("value.txt"), "42\n").unwrap();
    let mut original = step("original-verifier", "implement", paths);
    original.verify = paths.iter().map(|p| format!("node {p}")).collect();
    scope::register(&c, &plan(vec![original])).unwrap();
    for path in existing {
        std::fs::write(root.path().join(path), "// original existing verifier\n").unwrap();
    }
    // The second command keeps multi-output inclusion cases meaningful.
    if !paths.contains(&SECOND) {
        std::fs::write(root.path().join(SECOND), "// auxiliary registered check\n").unwrap();
    }
    c.completion_contract_path = Some(contract);
    bind_context(&c);
    (root, c, guard)
}
fn events(c: &Config) -> Vec<serde_json::Value> {
    std::fs::read_to_string(c.eval_events_path.as_ref().unwrap())
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

#[derive(Clone, Default)]
struct Replay {
    replies: Arc<Mutex<VecDeque<AssistantReply>>>,
    requests: Arc<Mutex<Vec<Vec<ConversationMessage>>>>,
}
impl Replay {
    fn new(replies: Vec<AssistantReply>) -> Self {
        Self {
            replies: Arc::new(Mutex::new(replies.into())),
            ..Default::default()
        }
    }
}
impl ChatClient for Replay {
    fn label(&self) -> &str {
        "issue466-replay"
    }
    fn boxed_clone(&self) -> Box<dyn ChatClient> {
        Box::new(self.clone())
    }
    fn supports_native_tools(&self, _: &str) -> bool {
        true
    }
    fn chat(
        &mut self,
        _: &str,
        messages: &[ConversationMessage],
        _: &[ToolSpec],
        _: bool,
    ) -> anyhow::Result<AssistantReply> {
        self.requests.lock().unwrap().push(messages.to_vec());
        self.replies
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| anyhow::anyhow!("issue466 replay exhausted"))
    }
}
fn proposal(p: &StepPlan) -> AssistantReply {
    AssistantReply::text(serde_json::to_string(p).unwrap())
}
fn generate(c: &Config, client: &mut Replay) -> anyhow::Result<StepPlan> {
    crate::planner::runner::generate_step_plan_with_ui_for_phase(
        client,
        "Repair missing verifier artifacts",
        c,
        &crate::tui::NOOP_UI,
        Some("repair-artifacts"),
        false,
        true,
    )
}

mod binding;
mod fallback;
mod formation;
mod runner;
