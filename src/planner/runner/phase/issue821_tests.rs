//! #821: a later phase that proposes a second creator of a registered verifier
//! path is a phase failure with a recovery handoff, not a run-wide error.
use crate::config::Config;
use crate::planner::recovery_contract_authority::verifier_obligations::{
    self as scope, VerifierObligationConflict,
};
use crate::planner::step_plan::StepPlan;
use crate::planner::ultra_plan::{UltraPhase, UltraPlan};
use crate::providers::AssistantReply;
use clap::Parser;
use std::collections::VecDeque;
use std::path::Path;
use std::sync::{Arc, Mutex};

fn plan_fixture(name: &str) -> StepPlan {
    serde_json::from_str(
        &std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/corpus/apps/issue821-verifier-obligation-conflict")
                .join(name),
        )
        .unwrap(),
    )
    .unwrap()
}

fn config(root: &Path) -> Config {
    let mut config =
        Config::from_cli(crate::cli::Cli::parse_from(["commandagent", "--ux-demo"])).unwrap();
    config.workspace_root = root.to_path_buf();
    config.eval_events_path = Some(root.join(".commandagent/runs/issue821/events.jsonl"));
    std::fs::create_dir_all(config.eval_events_path.as_ref().unwrap().parent().unwrap()).unwrap();
    config.offline = true;
    config
}

fn ultra_plan() -> UltraPlan {
    UltraPlan {
        goal: "Create a memo app".into(),
        profile: "generic".into(),
        intent: "create".into(),
        style: "default".into(),
        phases: ["project-setup", "implement-memo-logic"]
            .map(|id| UltraPhase {
                id: id.into(),
                prompt: format!("{id} task"),
            })
            .into(),
    }
}

fn events(c: &Config) -> Vec<serde_json::Value> {
    std::fs::read_to_string(c.eval_events_path.as_ref().unwrap())
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

#[test]
fn issue821_conflict_is_typed_and_keeps_registered_producer() {
    let root = tempfile::tempdir().unwrap();
    let config = config(root.path());
    let (_paths, _guard) = super::super::recovery_authority::begin(&config, &ultra_plan()).unwrap();
    let phase1 = plan_fixture("phase1-plan.json");
    let phase2 = plan_fixture("phase2-plan.json");
    super::super::recovery_authority::register_plan(&config, &phase1).unwrap();
    let registered = scope::generated(&config);
    assert_eq!(
        registered.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
        ["create-smoke-test"]
    );

    let err = super::super::recovery_authority::register_plan(&config, &phase2).unwrap_err();
    let conflict = err.downcast_ref::<VerifierObligationConflict>().unwrap();
    assert_eq!(conflict.registered_step.id, "create-smoke-test");
    assert_eq!(conflict.proposed_step, "update-smoke-check");
    assert_eq!(conflict.paths, ["smoke-check.js"]);
    assert_eq!(
        err.to_string(),
        "conflicting registered verifier creation obligations"
    );
    assert_eq!(scope::generated(&config), registered);

    // The same producer registered again stays a no-op.
    super::super::recovery_authority::register_plan(&config, &phase1).unwrap();
    assert_eq!(scope::generated(&config), registered);
}

#[test]
fn issue821_other_registration_refusals_are_not_conflicts() {
    let root = tempfile::tempdir().unwrap();
    let config = config(root.path());
    let (_paths, _guard) = super::super::recovery_authority::begin(&config, &ultra_plan()).unwrap();
    let mut plan = plan_fixture("phase1-plan.json");
    for step in &mut plan.steps {
        if step.id == "create-smoke-test" {
            step.expected_paths = vec!["smoke-check.js".into(), "index.html".into()];
        }
    }
    let err = super::super::recovery_authority::register_plan(&config, &plan).unwrap_err();
    assert!(err.downcast_ref::<VerifierObligationConflict>().is_none());
    assert!(
        err.to_string()
            .contains("mixes application/configuration scope"),
        "{err:#}"
    );
    assert!(scope::generated(&config).is_empty());
}

#[derive(Clone)]
struct Replay(Arc<Mutex<VecDeque<StepPlan>>>);
impl crate::planner::runner::ChatClient for Replay {
    fn label(&self) -> &str {
        "issue821-replay"
    }
    fn boxed_clone(&self) -> Box<dyn crate::planner::runner::ChatClient> {
        Box::new(self.clone())
    }
    fn chat(
        &mut self,
        _: &str,
        _: &[crate::state::ConversationMessage],
        _: &[crate::tools::registry::ToolSpec],
        _: bool,
    ) -> anyhow::Result<AssistantReply> {
        let plan = self.0.lock().unwrap().pop_front().expect("planned reply");
        Ok(AssistantReply::text(serde_json::to_string(&plan)?))
    }
}

fn prepare_phase(
    config: &Config,
    plan: &UltraPlan,
    index: usize,
    reply: StepPlan,
) -> anyhow::Result<Option<(StepPlan, crate::planner::profile::ProfileSnapshot)>> {
    let mut planner = Replay(Arc::new(Mutex::new([reply.clone(), reply].into())));
    let context = super::super::UltraRunContext::for_run(&config.workspace_root, &[]);
    let session = crate::state::SessionSnapshot::new();
    let mut machine = super::super::pipeline::PhaseRun::start().unwrap();
    super::prepare(
        &mut planner,
        config,
        plan,
        &plan.phases[index],
        index,
        &crate::tui::NOOP_UI,
        &context,
        &session,
        &mut machine,
        None,
        None,
        &[],
        false,
    )
}

#[test]
fn issue821_phase_conflict_emits_phase_failure_and_recovery_handoff() {
    let root = tempfile::tempdir().unwrap();
    let config = config(root.path());
    let plan = ultra_plan();
    let (_paths, _guard) = super::super::recovery_authority::begin(&config, &plan).unwrap();
    prepare_phase(&config, &plan, 0, plan_fixture("phase1-plan.json"))
        .unwrap()
        .unwrap();
    let registered = scope::generated(&config);
    assert_eq!(registered.len(), 1);

    let err = prepare_phase(&config, &plan, 1, plan_fixture("phase2-plan.json")).unwrap_err();
    let message = format!("{err:#}");
    assert!(
        message.contains("verifier obligation conflict"),
        "{message}"
    );
    assert_eq!(scope::generated(&config), registered);

    let log = events(&config);
    let failed = log
        .iter()
        .find(|e| e["event"] == "ultra_phase_failed")
        .expect("ultra_phase_failed");
    assert_eq!(failed["phase_id"], "implement-memo-logic");
    let saved = log
        .iter()
        .find(|e| e["event"] == "recovery_prompt_saved")
        .expect("recovery_prompt_saved");
    assert_eq!(
        saved["recovery_handoff_kind"],
        "verifier_obligation_conflict"
    );

    let yaml = std::fs::read_dir(root.path().join(".commandagent/plans"))
        .unwrap()
        .flatten()
        .map(|e| std::fs::read_to_string(e.path()).unwrap_or_default())
        .collect::<Vec<_>>()
        .join("\n");
    for needle in [
        "create-smoke-test",
        "update-smoke-check",
        "smoke-check.js",
        "verifier_obligation",
    ] {
        assert!(yaml.contains(needle), "{needle} missing from {yaml}");
    }
    assert!(!message.contains("direct_cli_command_failed"));
}
