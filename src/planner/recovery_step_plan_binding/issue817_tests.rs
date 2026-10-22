//! #817: the weak package-script predicate is replaced during sanitization, so
//! the first saved G1 proposals no longer stop at admission for that reason.
use std::collections::VecDeque;
use std::path::Path;
use std::sync::{Arc, Mutex};

use clap::Parser;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::config::Config;
use crate::planner::sanitizer::sanitize_step_plan_against_policy;
use crate::planner::step_plan::{PlanStep, StepPlan};
use crate::providers::{AssistantReply, ChatClient};
use crate::state::ConversationMessage;
use crate::tools::registry::ToolSpec;

const FIXTURE: &str = "tests/corpus/apps/issue817-package-script-predicate-formation";
const KIND: &str = "package_json_script_predicate_assertion";
const TOOL_G: &str = "tool-g-build-todo-ui";
const GAME_Q: &str = "game-q-r1-setup-project";
const WEAK_DEV: &str = "node -p \"require('./package.json').scripts.dev.includes('3011')\"";

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(FIXTURE)
            .join(name),
    )
    .unwrap()
}

fn saved(case: &str) -> StepPlan {
    let plans: Value = serde_json::from_str(&fixture("original-plan.json")).unwrap();
    serde_json::from_value(plans[case].clone()).unwrap()
}

fn formed(script: &str, method: &str, value: &str) -> String {
    crate::minimal_loop::evidence::package_script_check::formed_predicate_command(
        script, method, value,
    )
}

fn step(id: &str, verify: &[&str]) -> PlanStep {
    PlanStep {
        id: id.into(),
        kind: "implement".into(),
        expected_result: "pass".into(),
        instruction: format!("Update package.json for {id}."),
        expected_paths: vec!["package.json".into()],
        verify: verify.iter().map(|command| (*command).into()).collect(),
    }
}

#[test]
fn issue817_manifest_pins_the_saved_proposals() {
    let manifest: Value = serde_json::from_str(&fixture("source-manifest.json")).unwrap();
    for (name, digest) in manifest["fixture_sha256"].as_object().unwrap() {
        let actual = Sha256::digest(fixture(name).as_bytes());
        assert_eq!(format!("{actual:x}"), digest.as_str().unwrap(), "{name}");
    }
    for case in [TOOL_G, GAME_Q] {
        assert!(
            saved(case)
                .steps
                .iter()
                .any(|step| step.verify.iter().any(|v| v == WEAK_DEV)),
            "{case} keeps the recorded weak verify"
        );
    }
}

#[test]
fn issue817_sanitizer_replaces_weak_predicates_and_records_one_kind_each() {
    let already = formed("dev", "includes", "3011");
    let untouched = [
        "node -p \"require('./package.json').scripts.dev\"",
        "node -p \"require('./package.json').scripts.dev.includes('30;11')\"",
        "node -p \"require('./package.json').scripts.dev.match('3011')\"",
        "npm run build",
    ];
    let mut verify: Vec<&str> = vec![
        WEAK_DEV,
        "node --print \"require('./package.json').scripts.start.startsWith('next start')\"",
        "node -p \"require('./package.json').scripts.build.endsWith('build')\"",
        already.as_str(),
    ];
    verify.extend(untouched);
    let mut plan = StepPlan {
        goal: "Configure scripts".into(),
        steps: vec![step("configure-scripts", &verify)],
    };
    let workspace = tempfile::tempdir().unwrap();
    std::fs::write(workspace.path().join("package.json"), "{}").unwrap();
    std::fs::create_dir_all(workspace.path().join("node_modules")).unwrap();
    let report = sanitize_step_plan_against_policy(&mut plan, Some(workspace.path()));
    let records: Vec<_> = report
        .normalized_commands
        .iter()
        .filter(|record| record.kind == KIND)
        .collect();
    assert_eq!(records.len(), 3, "{:?}", report.normalized_commands);
    assert_eq!(records[0].original_command, WEAK_DEV);
    assert_eq!(
        records[0].normalized_command,
        formed("dev", "includes", "3011")
    );
    assert_eq!(
        records[1].normalized_command,
        formed("start", "startsWith", "next start")
    );
    assert_eq!(
        records[2].normalized_command,
        formed("build", "endsWith", "build")
    );
    assert!(
        records
            .iter()
            .all(|record| record.step_id == "configure-scripts")
    );
    let after = &plan.steps[0].verify;
    assert!(after.contains(&already), "formed form stays as written");
    assert_eq!(after.iter().filter(|v| **v == already).count(), 1);
    for command in untouched {
        assert!(after.iter().any(|v| v == command), "{command} unchanged");
    }
    assert!(!after.iter().any(|v| v == WEAK_DEV));
}

#[test]
fn issue817_sanitizer_leaves_a_plan_without_the_weak_form_unchanged() {
    let already = formed("dev", "includes", "3011");
    let mut plan = StepPlan {
        goal: "Configure scripts".into(),
        steps: vec![step("configure-scripts", &[already.as_str()])],
    };
    let workspace = tempfile::tempdir().unwrap();
    std::fs::write(workspace.path().join("package.json"), "{}").unwrap();
    std::fs::create_dir_all(workspace.path().join("node_modules")).unwrap();
    let report = sanitize_step_plan_against_policy(&mut plan, Some(workspace.path()));
    assert!(
        report
            .normalized_commands
            .iter()
            .all(|record| record.kind != KIND)
    );
    assert_eq!(plan.steps[0].verify, vec![already]);
}

const SCAFFOLD_PACKAGE: &str = r#"{"name":"issue817","private":true,"scripts":{"dev":"next dev -p 3011","start":"next start -p 3011","build":"next build"},"dependencies":{"next":"15.5.20","react":"19.1.0","react-dom":"19.1.0"}}"#;

/// Workspace of the recorded tool-g admission: an installed app with routes.
fn ready_workspace(root: &Path) {
    std::fs::write(root.join("package.json"), SCAFFOLD_PACKAGE).unwrap();
    std::fs::create_dir_all(root.join("node_modules")).unwrap();
    std::fs::create_dir_all(root.join("src/app")).unwrap();
    std::fs::write(
        root.join("src/app/page.tsx"),
        "export default function Page() { return null; }",
    )
    .unwrap();
    std::fs::write(
        root.join("src/app/layout.tsx"),
        "export default function Layout({children}) { return children; }",
    )
    .unwrap();
}

/// Workspace of the recorded game-q-r1 admission (events.jsonl line 13: phase 1,
/// no changed paths; the saved goal's snapshot lists exactly these entries, with
/// no node_modules and no routes).
fn scaffold_workspace(root: &Path) {
    std::fs::write(
        root.join("package.json"),
        r#"{"name":"issue817","private":true}"#,
    )
    .unwrap();
    std::fs::write(root.join("postcss.config.js"), "module.exports = {};\n").unwrap();
    std::fs::write(root.join("tailwind.config.ts"), "export default {};\n").unwrap();
    std::fs::write(root.join("tsconfig.json"), "{}\n").unwrap();
    std::fs::create_dir_all(root.join("src")).unwrap();
}

fn nextjs_config(root: &Path) -> Config {
    let mut c =
        Config::from_cli(crate::cli::Cli::parse_from(["commandagent", "--ux-demo"])).unwrap();
    c.workspace_root = root.to_path_buf();
    c.profile = crate::planner::profile_descriptor::NEXTJS_PROFILE_ID.into();
    c.profile_explicit = true;
    c.offline = true;
    c.yes = true;
    c.max_iterations = 4;
    c.eval_events_path = Some(root.join(".commandagent/runs/issue817/events.jsonl"));
    std::fs::create_dir_all(c.eval_events_path.as_ref().unwrap().parent().unwrap()).unwrap();
    c
}

fn events(c: &Config) -> Vec<Value> {
    std::fs::read_to_string(c.eval_events_path.as_ref().unwrap())
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

#[derive(Clone, Default)]
struct Replay {
    replies: Arc<Mutex<VecDeque<AssistantReply>>>,
    requests: Arc<Mutex<usize>>,
}

impl ChatClient for Replay {
    fn label(&self) -> &str {
        "issue817-replay"
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
        _: &[ConversationMessage],
        _: &[ToolSpec],
        _: bool,
    ) -> anyhow::Result<AssistantReply> {
        *self.requests.lock().unwrap() += 1;
        self.replies
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| anyhow::anyhow!("issue817 replay exhausted"))
    }
}

/// Replays the saved first proposal three times, as the recorded runs did, and
/// returns the admission events with the number of planner requests.
fn replay(
    case: &str,
    phase: &str,
    setup: fn(&Path),
) -> (Vec<Value>, usize, anyhow::Result<StepPlan>) {
    let root = tempfile::tempdir().unwrap();
    setup(root.path());
    let c = nextjs_config(root.path());
    let plan = saved(case);
    let reply = AssistantReply::text(serde_json::to_string(&plan).unwrap());
    let mut client = Replay {
        replies: Arc::new(Mutex::new(vec![reply.clone(), reply.clone(), reply].into())),
        ..Default::default()
    };
    let result = crate::planner::runner::generate_step_plan_with_ui_for_phase(
        &mut client,
        &plan.goal,
        &c,
        &crate::tui::NOOP_UI,
        Some(phase),
        true,
        false,
    );
    let admissions = events(&c)
        .into_iter()
        .filter(|event| event["event"] == "recovery_verifier_plan_admission")
        .collect();
    let requests = *client.requests.lock().unwrap();
    (admissions, requests, result)
}

fn reasons(admissions: &[Value]) -> Vec<String> {
    admissions
        .iter()
        .map(|event| event["reason"].as_str().unwrap_or_default().to_owned())
        .collect()
}

fn assert_formed_in_place_of_weak(plan: &StepPlan, owner: &str) {
    let formed = formed("dev", "includes", "3011");
    let step = plan.steps.iter().find(|step| step.id == owner).unwrap();
    assert!(step.verify.contains(&formed), "{owner}: {:?}", step.verify);
    assert!(
        plan.steps
            .iter()
            .all(|step| step.verify.iter().all(|v| v != WEAK_DEV)),
        "the weak form is gone"
    );
}

#[test]
fn issue817_tool_g_first_proposal_is_admitted_without_a_predicate_retry() {
    let (admissions, requests, result) = replay(TOOL_G, "build-todo-ui", ready_workspace);
    let reasons = reasons(&admissions);
    assert!(
        reasons.iter().all(|r| !r.contains("scripts.dev.includes")),
        "{reasons:?}"
    );
    // G1 recorded three rejections for this proposal; now the first is admitted.
    assert_eq!((requests, reasons.len()), (1, 0), "{reasons:?}");
    assert_formed_in_place_of_weak(&result.unwrap(), "configure-scripts");
}

#[test]
fn issue817_game_q_r1_first_proposal_loses_the_includes_reason() {
    let (admissions, requests, result) = replay(GAME_Q, "setup-project", scaffold_workspace);
    let reasons = reasons(&admissions);
    assert!(
        reasons.iter().all(|r| !r.contains(".includes('3011')")),
        "{reasons:?}"
    );
    // Recorded line 16 has stage preclosure_formation, so the replay is the non-recovery
    // formation path. The recorded second reason (scripts.dev refusal) only rode along
    // with the .includes failure; without it the first proposal is admitted.
    assert_eq!((requests, reasons.len()), (1, 0), "{reasons:?}");
    let plan = result.unwrap();
    assert_formed_in_place_of_weak(&plan, "verify-build-config");
    // The printed scripts.dev check is not a failure on its own and stays as proposed.
    let owner = plan
        .steps
        .iter()
        .find(|s| s.id == "setup-dependencies")
        .unwrap();
    assert_eq!(
        owner.verify,
        ["node -p \"require('./package.json').scripts.dev\""]
    );
}
