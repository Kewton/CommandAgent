//! Shapes derive from the recorded content-q (campaign-G1-3a10) and game-g
//! (campaign-G1-3a8) planner admissions; the commands are synthetic.
use super::super::admission::{Admission, Decision};
use crate::config::Config;
use crate::planner::step_plan::{PlanStep, StepPlan};
use crate::providers::{AssistantReply, ChatClient};
use crate::state::ConversationMessage;
use crate::tools::registry::ToolSpec;
use clap::Parser;
use serde_json::json;
use std::collections::VecDeque;
use std::path::Path;
use std::sync::{Arc, Mutex};

const FIXTURE: &str = "tests/corpus/apps/nextjs-planner-inline-node-frozen";
const NEXTJS: &str = crate::planner::profile_descriptor::NEXTJS_PROFILE_ID;
const GOAL: &str = "Build a storage module and verify it";

fn config(root: &Path, profile: &str) -> Config {
    let mut c =
        Config::from_cli(crate::cli::Cli::parse_from(["commandagent", "--ux-demo"])).unwrap();
    c.workspace_root = root.to_path_buf();
    c.profile = profile.into();
    c.profile_explicit = true;
    c.offline = true;
    c.yes = true;
    c.max_iterations = 4;
    c.eval_events_path = Some(root.join(".commandagent/runs/issue884/events.jsonl"));
    std::fs::create_dir_all(c.eval_events_path.as_ref().unwrap().parent().unwrap()).unwrap();
    // The sanitizer drops a node check while no package manifest exists.
    std::fs::write(
        root.join("package.json"),
        r#"{"name":"app","scripts":{"build":"next build"}}"#,
    )
    .unwrap();
    c
}

fn events(c: &Config) -> Vec<serde_json::Value> {
    std::fs::read_to_string(c.eval_events_path.as_ref().unwrap())
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

fn admissions(c: &Config) -> Vec<serde_json::Value> {
    events(c)
        .into_iter()
        .filter(|e| e["event"] == "recovery_verifier_plan_admission")
        .collect()
}

#[derive(Clone, Default)]
struct Replay {
    replies: Arc<Mutex<VecDeque<AssistantReply>>>,
    requests: Arc<Mutex<Vec<Vec<ConversationMessage>>>>,
}

impl Replay {
    fn new(replies: Vec<StepPlan>) -> Self {
        let replies = replies
            .iter()
            .map(|p| AssistantReply::text(serde_json::to_string(p).unwrap()))
            .collect::<VecDeque<_>>();
        Self {
            replies: Arc::new(Mutex::new(replies)),
            ..Default::default()
        }
    }

    fn calls(&self) -> usize {
        self.requests.lock().unwrap().len()
    }
}

impl ChatClient for Replay {
    fn label(&self) -> &str {
        "issue884-replay"
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
            .ok_or_else(|| anyhow::anyhow!("issue884 replay exhausted"))
    }
}

fn fixture() -> serde_json::Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(FIXTURE)
        .join("proposals.json");
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn commands(case: &str, proposal: &str) -> Vec<String> {
    fixture()[case][proposal]
        .as_array()
        .unwrap_or_else(|| panic!("fixture has no {case}.{proposal}"))
        .iter()
        .map(|c| c.as_str().unwrap().to_string())
        .collect()
}

fn owner(paths: &[&str], verify: Vec<String>) -> PlanStep {
    PlanStep {
        id: "storage-layer".into(),
        kind: "implement".into(),
        expected_result: "pass".into(),
        instruction: "Create the storage module and keep its read and write behavior correct."
            .into(),
        expected_paths: paths.iter().map(|p| (*p).into()).collect(),
        verify,
    }
}

fn plan(paths: &[&str], verify: Vec<String>) -> StepPlan {
    StepPlan {
        goal: GOAL.into(),
        steps: vec![owner(paths, verify)],
    }
}

// A node check is dropped by the sanitizer unless a Setup step precedes it.
fn proposals(case: &str) -> Vec<StepPlan> {
    ["first", "second", "third"]
        .iter()
        .map(|name| {
            let mut setup = owner(&["package.json"], vec![]);
            setup.id = "project-setup".into();
            setup.kind = "setup".into();
            setup.instruction = "Keep package.json with the build script.".into();
            let storage = owner(
                &["src/lib/storage.ts", "smoke-check.js"],
                commands(case, name),
            );
            StepPlan {
                goal: GOAL.into(),
                steps: vec![setup, storage],
            }
        })
        .collect()
}

fn generate(c: &Config, client: &mut Replay) -> anyhow::Result<StepPlan> {
    crate::planner::runner::generate_step_plan(client, GOAL, c)
}

fn check(
    admission: &mut Admission,
    c: &Config,
    proposed: &StepPlan,
    attempt: usize,
) -> anyhow::Result<Decision> {
    admission.check(c, None, proposed, &mut proposed.clone(), attempt)
}

fn rejection(
    admission: &mut Admission,
    c: &Config,
    proposed: &StepPlan,
    attempt: usize,
) -> anyhow::Error {
    match check(admission, c, proposed, attempt) {
        Err(error) => error,
        Ok(_) => panic!("attempt {attempt} was admitted"),
    }
}

#[test]
fn issue884_try_catch_require_inline_stops_at_the_first_proposal() {
    for profile in ["generic", NEXTJS] {
        let root = tempfile::tempdir().unwrap();
        let c = config(root.path(), profile);
        let mut client = Replay::new(proposals("content_q"));
        let error = generate(&c, &mut client).unwrap_err();
        assert_eq!(client.calls(), 1, "{profile}: planner was called again");
        let message = format!("{error:#}");
        assert!(
            message.contains("no acceptable form and is frozen"),
            "{profile}: {message}"
        );
        assert!(
            message.contains("a later proposal cannot replace it"),
            "{profile}: {message}"
        );
        let log = admissions(&c);
        assert_eq!(log.len(), 1, "{profile}: {log:?}");
        assert_eq!(log[0]["status"], "stopped");
        assert_eq!(log[0]["retry_allowed"], false);
        assert_eq!(log[0]["terminal_reason"], "unformable_inline_evidence");
        assert_eq!(log[0]["planner_attempt"], 1);
        assert_eq!(log[0]["recovery_budget_changed"], false);
        let frozen = &log[0]["bounded_repair_proof"]["frozen"][0];
        assert_eq!(frozen["command"], commands("content_q", "first")[0]);
        assert_eq!(frozen["repairability"], "immutable_inline_evidence");
        assert!(
            frozen["reason"]
                .as_str()
                .unwrap()
                .contains("node_smoke_without_assertion"),
            "{frozen}"
        );
    }
}

#[test]
fn issue884_throwing_guard_inline_stops_at_the_first_proposal() {
    let root = tempfile::tempdir().unwrap();
    let c = config(root.path(), NEXTJS);
    let mut client = Replay::new(proposals("game_g"));
    let error = generate(&c, &mut client).unwrap_err();
    assert_eq!(client.calls(), 1);
    assert!(format!("{error:#}").contains("no acceptable form and is frozen"));
    let log = admissions(&c);
    assert_eq!(log.len(), 1, "{log:?}");
    assert_eq!(log[0]["status"], "stopped");
    assert_eq!(log[0]["terminal_reason"], "unformable_inline_evidence");
    assert_eq!(
        log[0]["bounded_repair_proof"]["frozen"][0]["command"],
        commands("game_g", "first")[0]
    );
}

#[test]
fn issue884_a_mixed_plan_stops_when_any_inline_check_has_no_form() {
    let root = tempfile::tempdir().unwrap();
    let c = config(root.path(), "generic");
    let formable = commands("formable", "require_only")[0].clone();
    let frozen = commands("content_q", "first")[0].clone();
    let mixed = plan(
        &["src/lib/storage.ts"],
        vec![formable.clone(), frozen.clone()],
    );
    let mut admission = Admission::default();
    let error = rejection(&mut admission, &c, &mixed, 1);
    assert!(format!("{error:#}").contains("no acceptable form and is frozen"));
    let log = admissions(&c);
    let listed = log[0]["bounded_repair_proof"]["frozen"].as_array().unwrap();
    assert_eq!(listed.len(), 1, "{listed:?}");
    assert_eq!(listed[0]["command"], frozen);
}

#[test]
fn issue884_formable_inline_shapes_still_retry() {
    for (case, name) in [
        ("formable", "require_only"),
        ("formable", "dynamic_import_only"),
    ] {
        let root = tempfile::tempdir().unwrap();
        let c = config(root.path(), "generic");
        let original = plan(&["src/lib/types.ts"], commands(case, name));
        let mut admission = Admission::default();
        let decision = check(&mut admission, &c, &original, 1)
            .unwrap_or_else(|e| panic!("{name} did not retry: {e:#}"));
        assert!(matches!(decision, Decision::Retry(_)), "{name}");
        let log = admissions(&c);
        assert_eq!(log[0]["status"], "retry", "{name}");
        assert_eq!(log[0]["retry_allowed"], true, "{name}");
        assert!(log[0]["terminal_reason"].is_null(), "{name}");
    }
}

#[test]
fn issue884_formable_original_keeps_the_frozen_gate_for_a_smoke_check_file() {
    let root = tempfile::tempdir().unwrap();
    let c = config(root.path(), "generic");
    let original = plan(
        &["src/lib/types.ts", "smoke-check.js"],
        commands("formable", "require_only"),
    );
    let mut admission = Admission::default();
    assert!(matches!(
        check(&mut admission, &c, &original, 1).unwrap(),
        Decision::Retry(_)
    ));
    let rewritten = plan(
        &["src/lib/types.ts", "smoke-check.js"],
        commands("formable", "smoke_file"),
    );
    let Decision::Retry(feedback) = check(&mut admission, &c, &rewritten, 2).unwrap() else {
        panic!("the smoke-check.js proposal was admitted");
    };
    assert!(
        feedback.contains("formation lost original check"),
        "{feedback}"
    );
    let log = admissions(&c);
    assert_eq!(log[1]["status"], "retry");
    assert!(log[1]["terminal_reason"].is_null());
}

#[test]
fn issue884_no_proof_after_the_first_attempt_or_without_the_frozen_baseline() {
    let root = tempfile::tempdir().unwrap();
    let c = config(root.path(), "generic");
    let original = plan(&["src/lib/storage.ts"], commands("content_q", "first"));
    // No rejection has frozen anything yet.
    assert!(
        Admission::default()
            .prove_unformable_inline(&c, &original, 1)
            .is_none()
    );
    let mut admission = Admission::default();
    assert!(check(&mut admission, &c, &original, 1).is_err());
    assert!(
        admission
            .prove_unformable_inline(&c, &original, 1)
            .is_some()
    );
    assert!(
        admission
            .prove_unformable_inline(&c, &original, 2)
            .is_none()
    );
    let mut changed = original.clone();
    changed.steps[0].verify.clear();
    assert!(admission.prove_unformable_inline(&c, &changed, 1).is_none());
}

#[test]
fn issue884_file_backed_and_build_checks_are_not_stopped() {
    let root = tempfile::tempdir().unwrap();
    let c = config(root.path(), "generic");
    for name in ["smoke_file", "build"] {
        let original = plan(&["src/lib/storage.ts"], commands("accepted", name));
        let mut admission = Admission::default();
        let decision = check(&mut admission, &c, &original, 1)
            .unwrap_or_else(|e| panic!("{name} was stopped: {e:#}"));
        assert!(matches!(decision, Decision::Ready(_)), "{name}");
    }
}

#[test]
fn issue884_other_profiles_are_not_stopped_at_the_first_proposal() {
    let root = tempfile::tempdir().unwrap();
    let c = config(root.path(), "python-cli");
    let original = plan(&["src/lib/storage.ts"], commands("content_q", "first"));
    let mut admission = Admission::default();
    let outcome = check(&mut admission, &c, &original, 1);
    assert!(!outcome.is_err_and(|e| format!("{e:#}").contains("no acceptable form and is frozen")));
}

#[test]
fn issue884_only_the_nextjs_step_plan_prompt_names_inline_node_checks() {
    let mut prompts = Vec::new();
    for profile in [NEXTJS, "generic"] {
        let root = tempfile::tempdir().unwrap();
        let c = config(root.path(), profile);
        let mut client = Replay::new(vec![plan(&["a.txt"], vec![])]);
        let _ = generate(&c, &mut client);
        let requests = client.requests.lock().unwrap();
        let user = requests[0]
            .iter()
            .rfind(|m| m.role == "user")
            .map(|m| m.content.clone())
            .unwrap();
        prompts.push(user);
    }
    let new_item = "an inline Node behaviour check (node -e, or a node check that requires a TypeScript module)";
    assert!(prompts[0].contains("- Do not use these in verify:\n"));
    assert!(prompts[0].contains(&format!("  - {new_item}")));
    assert!(prompts[0].contains("  - yarn install\n"));
    assert!(prompts[0].contains("npm run build, a content grep (a host check token)"));
    assert!(!prompts[1].contains(new_item));
    assert!(!prompts[1].contains("smoke-check.js that asserts on plain JavaScript"));
}

#[test]
fn issue884_corpus_records_the_synthetic_origin_and_expected_status() {
    let data = fixture();
    assert!(data["synthetic"].as_str().unwrap().contains("synthetic"));
    for case in ["content_q", "game_g"] {
        assert_eq!(data[case]["expected_status"], "stopped", "{case}");
        assert_eq!(data[case]["planner_calls"], 1, "{case}");
        assert_eq!(json!(commands(case, "first").len()), 1, "{case}");
    }
}

#[test]
fn issue884_the_profile_gate_keeps_other_profiles_out_of_the_proof() {
    let root = tempfile::tempdir().unwrap();
    let original = plan(&["src/lib/storage.ts"], commands("content_q", "first"));
    // `form` never rejects for another profile, so only a direct call reaches
    // the gate: freeze the same plan under generic, then ask under others.
    let generic = config(root.path(), "generic");
    let mut admission = Admission::default();
    assert!(check(&mut admission, &generic, &original, 1).is_err());
    assert!(
        admission
            .prove_unformable_inline(&generic, &original, 1)
            .is_some()
    );
    let other = config(root.path(), "python-cli");
    assert!(
        admission
            .prove_unformable_inline(&other, &original, 1)
            .is_none()
    );
    let mut fresh = Admission::default();
    assert!(matches!(
        check(&mut fresh, &other, &original, 1),
        Ok(Decision::Ready(_))
    ));
}
