//! Portable replay of the frozen setup checkpoint. Hooks only observe or stop
//! after normal registration; no proposed plan or admission result is modified.
use super::*;
use crate::minimal_loop::completion::CompletionContract;
use crate::planner::recovery_contract_authority as authority;
use crate::planner::recovery_contract_authority::verifier_obligations as scope;
use crate::providers::AssistantReply;
use clap::Parser;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

const FIXTURE: &str = "tests/corpus/apps/issue490-verify-readers";
#[derive(Default)]
struct Observation {
    returned: Option<StepPlan>,
    registered: Option<StepPlan>,
    before: Option<CompletionContract>,
    after: Option<CompletionContract>,
    producers: Vec<crate::planner::step_plan::PlanStep>,
    events: Vec<serde_json::Value>,
}
thread_local! {
    static ACTIVE: RefCell<Option<Observation>> = const { RefCell::new(None) };
}
struct Reset;
impl Drop for Reset {
    fn drop(&mut self) {
        ACTIVE.with(|s| *s.borrow_mut() = None);
    }
}
fn active() -> bool {
    ACTIVE.with(|s| s.borrow().is_some())
}
fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(FIXTURE)
            .join(name),
    )
    .unwrap()
}
fn raw(case: &str, attempt: usize) -> StepPlan {
    serde_json::from_str(&fixture(&format!("{case}-attempt-{attempt}.json"))).unwrap()
}
fn events(c: &Config) -> Vec<serde_json::Value> {
    std::fs::read_to_string(c.eval_events_path.as_ref().unwrap())
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}
pub(super) fn skip_setup(index: usize) -> bool {
    active() && index == 0
}
pub(super) fn restore(
    c: &Config,
    plan: &UltraPlan,
    context: &mut UltraRunContext,
    machine: &mut pipeline::PhaseRun,
) -> anyhow::Result<()> {
    if !active() {
        return Ok(());
    }
    assert_eq!(plan.phases[0].id, "project-setup");
    let prior = crate::planner::step_plan::parse_step_plan(&fixture("setup-plan.yaml"))?;
    recovery_authority::register_plan(c, &prior)?;
    let contract: CompletionContract = serde_json::from_str(&fixture("setup-contract.json"))?;
    crate::planner::runner::bind_completion_contract_for_acceptance(
        c,
        "plan-run",
        &c.profile,
        contract.goal.as_deref().unwrap(),
        &contract.required_paths,
        &contract.required_capabilities,
        &contract.required_evidence,
        &contract.required_obligations,
    )?;
    let outcome = crate::planner::runner::StepPlanRunOutcome {
        completed_steps: 2,
        total_steps: 2,
        ..Default::default()
    };
    context.update_after_phase(&plan.phases[0], &outcome, Vec::new());
    let paths = resolve_profile_runtime(&plan.profile)
        .expected_scaffold_paths(&c.workspace_root, &plan.goal);
    recovery_authority::initialize(c, plan, &paths)?;
    let before = authority::load_for_handoff(c)?.unwrap();
    assert_eq!(
        serde_json::to_value(&before)?,
        serde_json::from_str::<serde_json::Value>(&fixture("run-contract.json"))?
    );
    assert!(scope::generated(c).is_empty());
    ACTIVE.with(|s| s.borrow_mut().as_mut().unwrap().before = Some(before));
    machine.phase_started()?;
    machine.plan_resolved()?;
    machine.plan_persisted(false, false)?;
    machine.before_phase_completed(false, false)?;
    machine.step_succeeded(false)?;
    machine.invariant_observed(true)?;
    machine.phase_committed(false, &plan.intent)?;
    Ok(())
}
pub(super) fn returned_and_saved(c: &Config, plan: &StepPlan) {
    if !active() {
        return;
    }
    let paths: Vec<_> = std::fs::read_dir(crate::runtime_paths::plans_dir(&c.workspace_root))
        .unwrap()
        .map(|p| p.unwrap().path())
        .collect();
    assert_eq!(paths.len(), 1);
    let text = std::fs::read_to_string(&paths[0]).unwrap();
    assert_eq!(
        crate::planner::step_plan::parse_step_plan(&text).unwrap(),
        *plan
    );
    ACTIVE.with(|s| s.borrow_mut().as_mut().unwrap().returned = Some(plan.clone()));
}
pub(super) fn registered(c: &Config, plan: &StepPlan) -> bool {
    if !active() {
        return false;
    }
    let after = authority::load_for_handoff(c).unwrap().unwrap();
    let path = crate::planner::completion_contract_path::generated_path(
        &c.workspace_root,
        c.eval_events_path.as_deref(),
        "completion-contract-ultra-plan-run.json",
    );
    let disk: CompletionContract = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(after, disk);
    issue492::observe_registration(c, plan, &after);
    ACTIVE.with(|s| {
        let mut s = s.borrow_mut();
        let s = s.as_mut().unwrap();
        assert_eq!(
            s.returned.as_ref(),
            Some(plan),
            "before_phase retained returned/saved plan"
        );
        s.registered = Some(plan.clone());
        s.after = Some(after);
        s.producers = scope::generated(c);
    });
    true
}
#[derive(Clone)]
struct Replay {
    replies: Arc<Mutex<VecDeque<StepPlan>>>,
    calls: Arc<Mutex<usize>>,
}
impl ChatClient for Replay {
    fn label(&self) -> &str {
        "issue490-fixed-replay"
    }
    fn boxed_clone(&self) -> Box<dyn ChatClient> {
        Box::new(self.clone())
    }
    fn chat(
        &mut self,
        _: &str,
        _: &[crate::state::ConversationMessage],
        _: &[crate::tools::registry::ToolSpec],
        _: bool,
    ) -> anyhow::Result<AssistantReply> {
        *self.calls.lock().unwrap() += 1;
        let plan = self
            .replies
            .lock()
            .unwrap()
            .pop_front()
            .expect("at most three attempts");
        Ok(AssistantReply::text(serde_json::to_string(&plan)?))
    }
}
struct Forbidden;
impl ChatClient for Forbidden {
    fn label(&self) -> &str {
        "forbidden-implementation"
    }
    fn boxed_clone(&self) -> Box<dyn ChatClient> {
        Box::new(Self)
    }
    fn chat(
        &mut self,
        _: &str,
        _: &[crate::state::ConversationMessage],
        _: &[crate::tools::registry::ToolSpec],
        _: bool,
    ) -> anyhow::Result<AssistantReply> {
        panic!("registration replay must stop before implementation")
    }
}
fn copy_tree(source: &Path, target: &Path) {
    std::fs::create_dir_all(target).unwrap();
    for entry in std::fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let dest = target.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &dest);
        } else {
            std::fs::copy(entry.path(), dest).unwrap();
        }
    }
}
fn replay(first: StepPlan, second: StepPlan, refusal: Option<&str>) -> Observation {
    let root = tempfile::tempdir().unwrap();
    copy_tree(
        &Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(FIXTURE)
            .join("scaffold"),
        root.path(),
    );
    for dir in ["node_modules", ".next", ".commandagent/runs/issue490"] {
        std::fs::create_dir_all(root.path().join(dir)).unwrap();
    }
    let mut c =
        Config::from_cli(crate::cli::Cli::parse_from(["commandagent", "--ux-demo"])).unwrap();
    c.workspace_root = root.path().into();
    c.state_dir = root.path().join("state");
    c.eval_events_path = Some(root.path().join(".commandagent/runs/issue490/events.jsonl"));
    c.profile = crate::planner::profile_descriptor::NEXTJS_PROFILE_ID.into();
    c.profile_explicit = true;
    c.offline = true;
    c.plan_preset = crate::config::PlanPreset::Profile;
    c.intent_override = Some(crate::planner::adjudication::contract::IntentId::Create);
    let plan = parse_ultra_plan(&fixture("ultra-plan.yaml")).unwrap();
    c.action = crate::config::Action::UltraPlanRun(plan.goal.clone());
    let mut planner = Replay {
        replies: Arc::new(Mutex::new([first, second.clone(), second].into())),
        calls: Arc::new(Mutex::new(0)),
    };
    ACTIVE.with(|s| *s.borrow_mut() = Some(Observation::default()));
    let _reset = Reset;
    let result = run_ultra_plan_with_ui(&mut planner, &mut Forbidden, &plan, &c, &NOOP_UI);
    let log = events(&c);
    let admissions: Vec<_> = log
        .iter()
        .filter(|e| e["event"] == "recovery_verifier_plan_admission")
        .collect();
    assert!(
        admissions[0]["reason"]
            .as_str()
            .unwrap()
            .contains("mixes application/configuration scope")
    );
    assert_eq!(admissions[0]["remaining_planner_attempts"], 2);
    assert_eq!(admissions[0]["recovery_budget_changed"], false);
    assert!(
        admissions[0]["original_obligation_sources"]["host"]["steps"][0]["instruction"]
            .as_str()
            .unwrap()
            .contains("runnable Next.js app")
    );
    let mut observation = ACTIVE.with(|s| s.borrow_mut().take().unwrap());
    observation.events = log.clone();
    if let Some(reason) = refusal {
        assert!(result.is_err(), "{reason}: {result:?}");
        assert_eq!(*planner.calls.lock().unwrap(), 3);
        assert!(
            admissions.last().unwrap()["reason"]
                .as_str()
                .unwrap()
                .contains(reason),
            "{admissions:?}"
        );
        assert!(observation.returned.is_none());
        assert!(observation.registered.is_none());
        assert!(scope::generated(&c).is_empty());
        let disk_path = crate::planner::completion_contract_path::generated_path(
            &c.workspace_root,
            c.eval_events_path.as_deref(),
            "completion-contract-ultra-plan-run.json",
        );
        let disk: CompletionContract =
            serde_json::from_slice(&std::fs::read(disk_path).unwrap()).unwrap();
        assert_eq!(observation.before.as_ref(), Some(&disk));
    } else {
        assert_eq!(
            result.unwrap_err().to_string(),
            "issue490: registered before execution"
        );
        assert_eq!(*planner.calls.lock().unwrap(), 2);
        assert!(observation.registered.is_some());
        assert_registration(&observation);
        let registered = observation.registered.as_ref().unwrap();
        let host = &admissions[0]["original_obligation_sources"]["host"]["steps"][0];
        let app = registered
            .steps
            .iter()
            .find(|s| s.id == "application-config")
            .unwrap();
        assert!(
            app.instruction
                .contains(host["instruction"].as_str().unwrap())
        );
        assert_eq!(
            app.expected_result,
            host["expected_result"].as_str().unwrap()
        );
        for p in host["expected_paths"].as_array().unwrap() {
            assert!(
                app.expected_paths
                    .contains(&p.as_str().unwrap().to_string())
            );
        }
        let model: StepPlan =
            serde_json::from_value(admissions[0]["original_obligation_sources"]["model"].clone())
                .unwrap();
        let mixed = model
            .steps
            .iter()
            .find(|s| s.id == "mixed-verifier-doc")
            .unwrap();
        for id in ["verifier-owner", "documentation-owner"] {
            assert!(
                registered
                    .steps
                    .iter()
                    .find(|s| s.id == id)
                    .unwrap()
                    .instruction
                    .contains(&mixed.instruction)
            );
        }
    }
    observation
}
fn assert_registration(o: &Observation) {
    let before = o.before.as_ref().unwrap();
    let after = o.after.as_ref().unwrap();
    for p in &before.required_paths {
        assert!(after.required_paths.contains(p));
    }
    for c in &before.verify_commands {
        assert!(after.verify_commands.contains(c));
    }
    for p in ["README.md", "verify-ui.cjs"] {
        assert!(!before.required_paths.contains(&p.into()));
        assert!(after.required_paths.contains(&p.into()));
    }
    let expected: serde_json::Value = serde_json::from_str(&fixture("registration.json")).unwrap();
    let added_paths: std::collections::BTreeSet<_> = after
        .required_paths
        .iter()
        .filter(|p| !before.required_paths.contains(p))
        .cloned()
        .collect();
    assert_eq!(
        added_paths,
        serde_json::from_value(expected["added_required_paths"].clone()).unwrap()
    );
    let added_commands: Vec<_> = after
        .verify_commands
        .iter()
        .filter(|c| !before.verify_commands.contains(c))
        .cloned()
        .collect();
    assert_eq!(json!(added_commands), expected["added_verify_commands"]);
    assert!(after.verify_commands.contains(&"node verify-ui.cjs".into()));
    assert!(!after.verify_commands.contains(&"test -f README.md".into()));
    let p = o.registered.as_ref().unwrap();
    assert!(
        p.steps
            .iter()
            .any(|s| s.verify.contains(&"test -f README.md".into()))
    );
    assert_eq!(o.producers.len(), 1);
    assert_eq!(o.producers[0].id, "verifier-owner");
    assert_eq!(o.producers[0].kind, "implement");
    assert_eq!(o.producers[0].expected_result, "pass");
    assert_eq!(o.producers[0].expected_paths, ["verify-ui.cjs"]);
    let app = p
        .steps
        .iter()
        .find(|s| s.id == "application-config")
        .unwrap();
    assert!(
        app.instruction
            .contains("Implement the UI and package configuration.")
    );
    assert!(app.instruction.contains("runnable Next.js app"));
    assert!(app.verify.contains(&"npm run build".into()));
}
#[test]
fn issue490_p02_d_normal_return_save_before_phase_registration() {
    let first = raw("P02-D", 1);
    let second = raw("P02-D", 2);
    let o = replay(first.clone(), second, None);
    let plan = o.registered.unwrap();
    assert_eq!(plan.steps, raw("P02-D", 2).steps);
    assert_eq!(plan.steps[0], first.steps[0]);
    assert_eq!(plan.steps[5].expected_paths, ["package.json"]);
    assert_eq!(plan.steps[5].verify, first.steps[0].verify);
}
#[test]
fn issue490_p01_and_n01_through_n07_registration_controls() {
    let first = raw("P01", 1);
    let second = raw("P01", 2);
    let positive = replay(first.clone(), second.clone(), None)
        .registered
        .unwrap();
    let cases: Vec<serde_json::Value> = serde_json::from_str(&fixture("variants.json")).unwrap();
    for case in cases {
        let mut next = second.clone();
        next.steps = serde_json::from_value(case["steps"].clone()).unwrap();
        let o = replay(first.clone(), next, case["refusal"].as_str());
        if case["case"] == "N06" {
            assert_eq!(
                o.registered.unwrap(),
                positive,
                "host reaugmentation is recovery, not refusal"
            );
        }
    }
}

#[path = "issue490_runtime_tests.rs"]
mod runtime;

#[test]
fn issue490_acquired_before_and_after_reach_normal_registration_separately() {
    let fixture: serde_json::Value =
        serde_json::from_str(&fixture("acquired-before-after.json")).unwrap();
    let original: StepPlan = serde_json::from_value(fixture["original"].clone()).unwrap();
    let proposed: StepPlan = serde_json::from_value(fixture["proposed"].clone()).unwrap();
    let o = replay(original.clone(), proposed.clone(), None);
    let registered = o.registered.unwrap();
    assert_eq!(registered.steps[0], original.steps[0]);
    assert_eq!(registered.steps.last(), original.steps.last());
    for which in [0, 5] {
        let mut missing = proposed.clone();
        missing.steps.remove(which);
        replay(
            original.clone(),
            missing,
            Some("lost original read-only Verify duty"),
        );
    }
    for (from, to) in [(0, 3), (5, 3)] {
        let mut moved = proposed.clone();
        let s = moved.steps.remove(from);
        moved.steps.insert(to, s);
        replay(
            original.clone(),
            moved,
            Some("lost original read-only Verify duty"),
        );
    }
}

#[path = "issue492_tests.rs"]
mod issue492;
