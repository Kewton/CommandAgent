//! #831: the profile addition must not land on a step that creates a verifier
//! script, because admission rejects such a producer as a mixed scope.
use std::path::Path;

use clap::Parser;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::config::Config;
use crate::minimal_loop::completion::CompletionContract;
use crate::planner::recovery_contract_authority::verifier_obligations;
use crate::planner::recovery_step_plan_binding::profile_augmentation::strengthen_step_plan_for_profile;
use crate::planner::recovery_step_plan_binding::profile_contract_reference::PROFILE_CONTRACT_REFERENCE;
use crate::planner::step_plan::{PlanStep, StepPlan};

const FIXTURE: &str = "tests/corpus/apps/issue831-profile-owner-skips-verifier-producer";
const GAME_Q: &str = "game-q-g1-game-ui-implementation";

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(FIXTURE)
            .join(name),
    )
    .unwrap()
}

fn saved() -> StepPlan {
    let plans: Value = serde_json::from_str(&fixture("original-plan.json")).unwrap();
    serde_json::from_value(plans[GAME_Q].clone()).unwrap()
}

fn nextjs_config(root: &Path) -> Config {
    let mut c =
        Config::from_cli(crate::cli::Cli::parse_from(["commandagent", "--ux-demo"])).unwrap();
    c.workspace_root = root.to_path_buf();
    c.profile = crate::planner::profile_descriptor::NEXTJS_PROFILE_ID.into();
    c.profile_explicit = true;
    c.offline = true;
    c
}

fn step(id: &str, kind: &str, paths: &[&str], verify: &[&str]) -> PlanStep {
    PlanStep {
        id: id.into(),
        kind: kind.into(),
        expected_result: "pass".into(),
        instruction: format!("Do {id}."),
        expected_paths: paths.iter().map(|p| (*p).into()).collect(),
        verify: verify.iter().map(|v| (*v).into()).collect(),
    }
}

fn by_id<'a>(plan: &'a StepPlan, id: &str) -> &'a PlanStep {
    plan.steps.iter().find(|s| s.id == id).unwrap()
}

#[test]
fn issue831_manifest_pins_the_saved_proposal() {
    let manifest: Value = serde_json::from_str(&fixture("source-manifest.json")).unwrap();
    for (name, digest) in manifest["fixture_sha256"].as_object().unwrap() {
        let actual = Sha256::digest(fixture(name).as_bytes());
        assert_eq!(format!("{actual:x}"), digest.as_str().unwrap(), "{name}");
    }
    let plan = saved();
    assert_eq!(plan.goal.chars().count(), 8668);
    assert!(plan.goal.to_ascii_lowercase().contains("scaffold"));
    assert_eq!(
        by_id(&plan, "create-verification-script").expected_paths,
        ["smoke-check.js"]
    );
}

#[test]
fn issue831_game_q_profile_addition_skips_the_verifier_producer_and_admits() {
    let root = tempfile::tempdir().unwrap();
    let c = nextjs_config(root.path());
    let mut plan = saved();
    let addition = strengthen_step_plan_for_profile(&mut plan, &c);
    assert!(addition.is_some());
    assert_eq!(
        by_id(&plan, "create-verification-script").expected_paths,
        ["smoke-check.js"]
    );
    let owner = by_id(&plan, "implement-game-ui");
    assert!(owner.expected_paths.iter().any(|p| p == "package.json"));
    assert!(owner.instruction.contains(PROFILE_CONTRACT_REFERENCE));
    let contract: CompletionContract = serde_json::from_value(json!({
        "goal": "g", "profile": crate::planner::profile_descriptor::NEXTJS_PROFILE_ID, "required_paths": ["smoke-check.js"],
        "verify_commands": ["node smoke-check.js"], "protected_paths": []
    }))
    .unwrap();
    for step in &plan.steps {
        let produces = step.step_kind() != crate::planner::step_plan::StepKind::Verify
            && step.expected_paths.iter().any(|p| p == "smoke-check.js");
        if produces {
            verifier_obligations::validate_producer(&c, &contract, step)
                .unwrap_or_else(|e| panic!("admission rejects {}: {e}", step.id));
        }
    }
}

#[test]
fn issue831_scaffold_setup_owner_keeps_package_json_beside_a_verifier_producer() {
    let root = tempfile::tempdir().unwrap();
    let c = nextjs_config(root.path());
    let mut plan = StepPlan {
        goal: "Create the app (scaffold).".into(),
        steps: vec![
            step("setup-app", "setup", &["src/app/page.tsx"], &[]),
            step("write-smoke", "implement", &["smoke.js"], &[]),
            step("verify-smoke", "verify", &[], &["node smoke.js"]),
        ],
    };
    strengthen_step_plan_for_profile(&mut plan, &c).unwrap();
    assert_eq!(
        by_id(&plan, "setup-app").expected_paths,
        ["src/app/page.tsx", "package.json"]
    );
    assert_eq!(by_id(&plan, "write-smoke").expected_paths, ["smoke.js"]);
}

#[test]
fn issue831_producer_only_plan_matches_the_pre_change_snapshot() {
    let root = tempfile::tempdir().unwrap();
    let c = nextjs_config(root.path());
    let mut plan = StepPlan {
        goal: "Create the app (scaffold).".into(),
        steps: vec![
            step("write-smoke", "implement", &["./smoke.js"], &[]),
            step("verify-smoke", "verify", &[], &["node smoke.js"]),
        ],
    };
    assert!(strengthen_step_plan_for_profile(&mut plan, &c).is_some());
    let producer = &plan.steps[0];
    assert_eq!(producer.expected_paths, ["./smoke.js", "package.json"]);
    assert_eq!(
        producer.instruction,
        format!("Do write-smoke.\n\nProfile contract:\n{PROFILE_CONTRACT_REFERENCE}")
    );
    assert!(plan.steps[1].expected_paths.is_empty());
    assert_eq!(plan.steps[1].instruction, "Do verify-smoke.");
}

/// Fixed outputs of the pre-#831 function for plans without a verifier producer.
#[test]
fn issue831_plans_without_a_producer_match_the_pre_change_snapshot() {
    let root = tempfile::tempdir().unwrap();
    let c = nextjs_config(root.path());
    let mut plan = StepPlan {
        goal: "Create the app (scaffold).".into(),
        steps: vec![
            step("setup-app", "setup", &["src/app/page.tsx"], &[]),
            step("implement-ui", "implement", &["src/app/ui.tsx"], &[]),
            step("verify-build", "verify", &[], &["npm run build"]),
        ],
    };
    strengthen_step_plan_for_profile(&mut plan, &c);
    let owner = format!("Do implement-ui.\n\nProfile contract:\n{PROFILE_CONTRACT_REFERENCE}");
    let got: Vec<(&str, &[String], &str)> = plan
        .steps
        .iter()
        .map(|s| {
            (
                s.id.as_str(),
                s.expected_paths.as_slice(),
                s.instruction.as_str(),
            )
        })
        .collect();
    assert_eq!(
        got,
        [
            (
                "setup-app",
                &["src/app/page.tsx".to_string()][..],
                "Do setup-app."
            ),
            (
                "implement-ui",
                &["src/app/ui.tsx".to_string(), "package.json".to_string()][..],
                owner.as_str()
            ),
            ("verify-build", &[][..], "Do verify-build."),
        ]
    );
}

#[test]
fn issue831_inspect_report_plan_still_falls_back_to_report() {
    let root = tempfile::tempdir().unwrap();
    let c = nextjs_config(root.path());
    let mut plan = StepPlan {
        goal: "Review the scaffold.".into(),
        steps: vec![
            step("look", "inspect", &[], &[]),
            step("write-up", "report", &[], &[]),
        ],
    };
    strengthen_step_plan_for_profile(&mut plan, &c);
    let report = &plan.steps[1];
    assert_eq!(report.kind, "implement");
    assert_eq!(report.expected_paths, ["package.json"]);
    assert_eq!(
        report.instruction,
        format!("Do write-up.\n\nProfile contract:\n{PROFILE_CONTRACT_REFERENCE}")
    );
    assert!(plan.steps[0].expected_paths.is_empty());
}

fn attempt2() -> StepPlan {
    serde_json::from_str(include_str!(
        "../../../tests/corpus/apps/issue478-model-host-obligations/attempt-2.json"
    ))
    .unwrap()
}

#[test]
fn issue831_issue478_attempt_2_gets_package_json_on_the_app_step() {
    let root = tempfile::tempdir().unwrap();
    let c = nextjs_config(root.path());
    let mut plan = attempt2();
    strengthen_step_plan_for_profile(&mut plan, &c).unwrap();
    let app = by_id(&plan, "wire-data-anvil");
    assert!(app.expected_paths.iter().any(|p| p == "package.json"));
    assert!(app.instruction.contains(PROFILE_CONTRACT_REFERENCE));
    assert_eq!(
        by_id(&plan, "create-smoke-check").expected_paths,
        ["smoke-check.js"]
    );
    let contract: CompletionContract = serde_json::from_value(json!({
        "goal": "g", "profile": crate::planner::profile_descriptor::NEXTJS_PROFILE_ID, "required_paths": ["smoke-check.js"],
        "verify_commands": ["node smoke-check.js"], "protected_paths": []
    }))
    .unwrap();
    verifier_obligations::validate_producer(&c, &contract, by_id(&plan, "create-smoke-check"))
        .unwrap();
}

#[test]
fn issue831_legacy_owner_guard_restores_the_new_choice_on_drop() {
    use crate::planner::recovery_step_plan_binding::profile_augmentation::legacy_pre_831_owner_for_issue478_fixtures;
    let root = tempfile::tempdir().unwrap();
    let c = nextjs_config(root.path());
    let owner_of = || {
        let mut plan = attempt2();
        strengthen_step_plan_for_profile(&mut plan, &c).unwrap();
        plan.steps
            .iter()
            .find(|s| s.expected_paths.iter().any(|p| p == "package.json"))
            .unwrap()
            .id
            .clone()
    };
    assert_eq!(owner_of(), "wire-data-anvil");
    {
        let _legacy = legacy_pre_831_owner_for_issue478_fixtures();
        assert_eq!(owner_of(), "create-smoke-check");
    }
    assert_eq!(owner_of(), "wire-data-anvil");
    let panicked = std::panic::catch_unwind(|| {
        let _legacy = legacy_pre_831_owner_for_issue478_fixtures();
        panic!("guard must still restore");
    });
    assert!(panicked.is_err());
    assert_eq!(owner_of(), "wire-data-anvil");
}
