use super::*;
use crate::minimal_loop::loop_run::{
    CompletionContractVerification, ContractEnforcement, RunStopReason,
    run_session_with_outcome_with_options,
};
use crate::planner::repair::RecoveryHandoff;
use crate::providers::{AssistantReply, ChatClient};
use crate::state::{ConversationMessage, SessionSnapshot, ToolCall};
use crate::tools::registry::ToolSpec;
use clap::Parser;
use std::collections::VecDeque;

#[path = "authority_tests.rs"]
mod authority_cases;
#[path = "configuration_tests.rs"]
mod configuration_cases;
#[path = "nextjs_tests.rs"]
mod nextjs;
#[path = "review_tests.rs"]
mod review;
#[path = "typescript_tests.rs"]
mod typescript;

const FIXTURE: &str = "tests/corpus/apps/issue465-repair-obligation";
const API: &str = "src/api.js";
const STORE: &str = "src/store.js";

#[derive(Clone, Default)]
struct Replay {
    replies: Arc<Mutex<VecDeque<AssistantReply>>>,
    requests: Arc<Mutex<Vec<Vec<ConversationMessage>>>>,
}
impl Replay {
    fn new(replies: Vec<AssistantReply>) -> Self {
        Self {
            replies: Arc::new(Mutex::new(replies.into())),
            ..Self::default()
        }
    }
}
impl ChatClient for Replay {
    fn label(&self) -> &str {
        "issue465-replay"
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
            .context("issue465 replay exhausted")
    }
}

pub(crate) fn setup() -> (tempfile::TempDir, Config) {
    setup_with(|_| {})
}

fn setup_with(customize: impl FnOnce(&Config)) -> (tempfile::TempDir, Config) {
    let root = tempfile::tempdir().unwrap();
    for path in [
        API,
        STORE,
        "src/types.js",
        "checks/target.js",
        "package.json",
    ] {
        let dest = root.path().join(path);
        std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
        std::fs::copy(Path::new(FIXTURE).join("original").join(path), dest).unwrap();
    }
    let mut config =
        Config::from_cli(crate::cli::Cli::parse_from(["commandagent", "--ux-demo"])).unwrap();
    config.workspace_root = root.path().to_path_buf();
    config.profile = "generic".into();
    config.profile_explicit = true;
    config.offline = true;
    config.yes = true;
    config.max_iterations = 6;
    config.eval_events_path = Some(root.path().join(".commandagent/events.jsonl"));
    config.completion_contract_path = Some(root.path().join("contract.json"));
    std::fs::write(
        root.path().join("contract.json"),
        json!({
            "goal":"Repair shift API validation", "profile":"generic",
            "required_paths":[API,"checks/target.js","contract.json"], "verify_commands":["node checks/target.js"],
            "protected_paths":["checks/target.js","contract.json"]
        })
        .to_string(),
    )
    .unwrap();
    customize(&config);
    let handoff = RecoveryHandoff {
        original_goal: "Repair shift API validation".into(),
        failure_kind: "compile_error".into(),
        failure_evidence: vec![
            "src/api.js:4:10: validateShift requires 2 arguments; caller supplies 1".into(),
        ],
        repair_targets: vec![API.into()],
        ..RecoveryHandoff::default()
    };
    super::super::recovery_inspection::bind_context(
        &config,
        &config,
        Some("create"),
        &handoff,
        None,
    )
    .unwrap();
    (root, config)
}

fn step(id: &str, path: &str) -> PlanStep {
    PlanStep {
        id: id.into(),
        kind: "implement".into(),
        expected_result: "pass".into(),
        instruction: "Repair the API validation relationship".into(),
        expected_paths: vec![path.into()],
        verify: vec![],
    }
}
pub(crate) fn options(config: &Config, plan: &StepPlan, index: usize) -> RunSessionOptions {
    let mut options = RunSessionOptions::plan_step_with_enforcement(
        RunSessionStepKind::Implement,
        ContractEnforcement::Observe,
        Some("repair-api".into()),
    );
    options.step_id = Some(plan.steps[index].id.clone());
    options.completion_contract_verification = CompletionContractVerification::DisabledDuringStep;
    configure(config, plan, &plan.steps[index], &mut options).unwrap();
    options
}
pub(crate) fn plan() -> StepPlan {
    StepPlan {
        goal: "Repair shift validation".into(),
        steps: vec![step("fix-api", API)],
    }
}
fn tool(name: &str, args: serde_json::Value) -> AssistantReply {
    AssistantReply {
        content: String::new(),
        tool_calls: vec![ToolCall::new(name, args)],
        prompt_tokens: None,
        completion_tokens: None,
    }
}
fn cat(command: &str) -> AssistantReply {
    tool("Bash", json!({"command":command}))
}
fn failed_edit() -> AssistantReply {
    tool(
        "Edit",
        json!({"path":API,"old_string":"missing anchor","new_string":"repair"}),
    )
}
fn fix_api() -> AssistantReply {
    tool(
        "Edit",
        json!({"path":API,"old_string":"validateShift(input)","new_string":"validateShift(input, policy)"}),
    )
}
fn fix_store() -> AssistantReply {
    tool(
        "Edit",
        json!({"path":STORE,"old_string":"input, policy)","new_string":"input, policy = defaultPolicy)"}),
    )
}
fn events(config: &Config) -> Vec<serde_json::Value> {
    std::fs::read_to_string(config.eval_events_path.as_ref().unwrap())
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}
fn run(
    config: &Config,
    options: RunSessionOptions,
    replies: Vec<AssistantReply>,
) -> anyhow::Result<crate::minimal_loop::loop_run::RunSessionOutcome> {
    run_session_with_outcome_with_options(
        &mut Replay::new(replies),
        &mut SessionSnapshot::new(),
        "Repair the API validation relationship",
        &[API.into()],
        config,
        &crate::tui::NOOP_UI,
        options,
    )
}

#[test]
fn issue465_replays_old_failed_edit_cat_completion_and_blocks_bound_control() {
    for bound in [false, true] {
        let (_root, config) = setup();
        let mut options = options(&config, &plan(), 0);
        if !bound {
            options.recovery_obligation = None;
        }
        let result = run(
            &config,
            options,
            vec![
                failed_edit(),
                cat("cat src/api.js"),
                AssistantReply::text("Repaired"),
            ],
        );
        let log = events(&config);
        assert!(
            log.iter()
                .any(|e| e.to_string().contains("edit_anchor_not_found")),
            "{log:?}"
        );
        if bound {
            assert!(result.is_err(), "{result:?}");
            assert!(
                log.iter()
                    .any(|e| e["reason"] == "recovery_repair_unresolved")
            );
            assert!(
                !log.iter()
                    .any(|e| e["reason"] == "required_artifacts_satisfied_after_tool")
            );
        } else {
            assert_eq!(
                result.unwrap().stop_reason,
                RunStopReason::RequiredArtifactsSatisfiedAfterTool
            );
        }
    }
}

#[test]
fn issue465_non_repairs_cannot_discharge_and_feedback_allows_real_repair() {
    for non_repair in [
        cat("cat src/api.js"),
        cat("cat ./src/store.js"),
        tool("Read", json!({"path":API})),
        tool(
            "Write",
            json!({"path":API,"content":std::fs::read_to_string(Path::new(FIXTURE).join("original").join(API)).unwrap()}),
        ),
        tool("Write", json!({"path":"unrelated.txt","content":"change"})),
        tool(
            "Write",
            json!({"path":"smoke.js","content":"console.log('pass')"}),
        ),
        AssistantReply::text("Already repaired and verified"),
    ] {
        let (_root, config) = setup();
        let result = run(
            &config,
            options(&config, &plan(), 0),
            vec![
                failed_edit(),
                non_repair,
                fix_api(),
                AssistantReply::text("Repaired"),
            ],
        );
        assert!(result.is_ok(), "{result:?}");
        let log = events(&config);
        let blocked = log
            .iter()
            .position(|e| e["reason"] == "recovery_repair_unresolved")
            .unwrap();
        let resolved = log.iter().position(|e| e["status"] == "resolved").unwrap();
        assert!(blocked < resolved);
        assert!(
            log[resolved]["diagnostics"]
                .as_str()
                .unwrap()
                .contains("requires 2 arguments")
        );
    }
}

#[test]
fn issue465_related_store_bash_edit_and_fresh_no_edit_confirmation_pass() {
    for repair in [
        fix_store(),
        cat(
            "python3 -c 'from pathlib import Path; p=Path(\"src/api.js\"); p.write_text(p.read_text().replace(\"validateShift(input)\", \"validateShift(input, policy)\"))'",
        ),
    ] {
        let (_root, config) = setup();
        let result = run(
            &config,
            options(&config, &plan(), 0),
            vec![repair, AssistantReply::text("Repaired")],
        );
        assert!(result.is_ok(), "{result:?}");
        let fresh = run(
            &config,
            options(&config, &plan(), 0),
            vec![cat("cat src/api.js"), AssistantReply::text("Confirmed")],
        );
        assert!(fresh.is_ok(), "{fresh:?}");
        assert!(
            events(&config)
                .iter()
                .filter(|e| e["status"] == "resolved")
                .count()
                >= 2
        );
    }
}

#[test]
fn issue465_stale_pass_and_preservation_bypasses_fail() {
    let alterations = [
        (API, "export function POST(input) { return true; }"),
        (
            API,
            "// validateShift(input)\nexport function POST(input) { return true; }",
        ),
        (
            API,
            "import { validateShift } from './store.js';\n// @ts-ignore\nexport function POST(input) { return validateShift(input); }",
        ),
        ("checks/target.js", "console.log('pass');"),
        (
            "package.json",
            "{\"type\":\"module\",\"scripts\":{\"test\":\"true\"}}",
        ),
        (
            STORE,
            "import { policy as defaultPolicy } from './types.js'; export function validateShift(input, policy) { throw new Error('early failure'); if (!policy) throw new TypeError('early'); return false; }",
        ),
    ];
    for (path, content) in alterations {
        let (_root, config) = setup();
        let opts = options(&config, &plan(), 0);
        std::fs::write(config.workspace_root.join(path), content).unwrap();
        assert!(
            opts.recovery_obligation
                .as_ref()
                .unwrap()
                .feedback(&config, &opts)
                .is_some(),
            "{path}"
        );
    }
    let (_root, config) = setup();
    let before = std::fs::read_to_string(config.workspace_root.join(API)).unwrap();
    std::fs::write(
        config.workspace_root.join(API),
        before.replace("validateShift(input)", "validateShift(input, policy)"),
    )
    .unwrap();
    let opts = options(&config, &plan(), 0);
    let obligation = opts.recovery_obligation.as_ref().unwrap();
    assert!(obligation.feedback(&config, &opts).is_none());
    std::fs::write(config.workspace_root.join(API), before).unwrap();
    assert!(obligation.feedback(&config, &opts).is_some());
}

#[test]
fn issue465_dependency_group_carries_pending_to_finite_boundary() {
    let (_root, config) = setup();
    let plan = StepPlan {
        goal: "Repair dependent API and store".into(),
        steps: vec![step("api-owner", API), step("store-owner", STORE)],
    };
    let first = options(&config, &plan, 0);
    let result = run(
        &config,
        first.clone(),
        vec![tool(
            "Edit",
            json!({"path":API,"old_string":"POST(input)","new_string":"POST(input = {})"}),
        )],
    );
    assert!(result.is_ok(), "{result:?}");
    assert!(first.recovery_obligation.as_ref().unwrap().pending());
    let second = options(&config, &plan, 1);
    let obligation = second.recovery_obligation.as_ref().unwrap();
    assert!(obligation.feedback(&config, &second).is_some());
    let result = run(
        &config,
        second.clone(),
        vec![fix_store(), AssistantReply::text("Repaired")],
    );
    assert!(result.is_ok(), "{result:?}");
    assert!(!obligation.pending());
    let log = events(&config);
    let pending = log.iter().find(|e| e["status"] == "pending").unwrap();
    assert_eq!(pending["remaining_owners"], json!(["store-owner"]));
    assert_eq!(pending["confirmation_boundary"], "store-owner");
    assert_eq!(pending["remaining_step_budget"], 1);
}

#[test]
fn issue465_runner_cannot_complete_before_real_target_repair() {
    let (_root, config) = setup();
    let mut replay = Replay::new(vec![
        failed_edit(),
        cat("cat src/api.js"),
        fix_store(),
        AssistantReply::text("Repaired"),
    ]);
    let result =
        crate::planner::run_step_plan_with_ui(&mut replay, &plan(), &config, &crate::tui::NOOP_UI);
    assert!(result.is_ok(), "{result:?}");
    let log = events(&config);
    assert!(
        log.iter()
            .any(|e| e["reason"] == "recovery_repair_unresolved")
    );
    assert!(log.iter().any(|e| e["status"] == "resolved"));
}
