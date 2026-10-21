use super::*;
use crate::minimal_loop::completion::CompletionContract;
use crate::planner::lint::{PlanQualityContext, step_plan_quality_report};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

const FIXTURE: &str = "tests/corpus/apps/issue494-require-formation";
static TRACE: OnceLock<Mutex<Vec<serde_json::Value>>> = OnceLock::new();

pub(crate) fn record(
    stage: &str,
    attempt: usize,
    original: &StepPlan,
    formed: &StepPlan,
    replacements: &serde_json::Value,
) {
    TRACE
        .get_or_init(|| Mutex::new(Vec::new()))
        .lock()
        .unwrap()
        .push(json!({
            "stage":stage, "attempt":attempt, "original":original,
            "formed":formed, "replacements":replacements,
        }));
}

fn clear_trace() {
    TRACE
        .get_or_init(|| Mutex::new(Vec::new()))
        .lock()
        .unwrap()
        .clear();
}

fn traces() -> Vec<serde_json::Value> {
    TRACE
        .get_or_init(|| Mutex::new(Vec::new()))
        .lock()
        .unwrap()
        .clone()
}

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE)
}

fn fixture() -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(fixture_root().join("cases.json")).unwrap()).unwrap()
}

fn saved_input() -> serde_json::Value {
    let bytes = std::fs::read(fixture_root().join("saved-input.json")).unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(&bytes)),
        fixture()["frozen"]["saved_input_sha256"]
    );
    serde_json::from_slice(&bytes).unwrap()
}

fn fixture_command(name: &str) -> String {
    let fixture = fixture();
    let command = fixture[name]["command"].as_str().unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(command.as_bytes())),
        fixture[name]["sha256"],
        "fixture hash drifted for {name}"
    );
    command.into()
}

fn original_plan() -> StepPlan {
    let owner: PlanStep = serde_json::from_value(saved_input()["owner"].clone()).unwrap();
    assert_eq!(owner.verify, [fixture_command("original")]);
    StepPlan {
        goal: "Implement and verify shared modules".into(),
        steps: vec![owner],
    }
}

fn positive_plan() -> StepPlan {
    let mut plan = original_plan();
    plan.steps[0].verify = vec![fixture_command("positive")];
    plan
}

fn moved_positive_plan() -> StepPlan {
    let mut plan = positive_plan();
    plan.steps[0].verify.clear();
    let mut check = step("check-shared-modules", "verify", &[]);
    check.instruction = "Run the shared module boundary check.".into();
    check.verify = vec![fixture_command("positive")];
    plan.steps.push(check);
    plan
}

fn direct(
    c: &Config,
    proposed: &StepPlan,
) -> (admission::Admission, anyhow::Result<admission::Decision>) {
    let original = original_plan();
    let mut admission = admission::Admission::default();
    let first = admission
        .check(c, None, &original, &mut original.clone(), 1)
        .unwrap();
    assert!(matches!(first, admission::Decision::Retry(_)));
    let mut candidate = proposed.clone();
    let result = admission.check(c, None, proposed, &mut candidate, 2);
    (admission, result)
}

fn empty_generated_contract(c: &Config) -> PathBuf {
    let path = generated_contract(c);
    std::fs::write(
        &path,
        json!({"profile":"generic","required_paths":[],"verify_commands":[]}).to_string(),
    )
    .unwrap();
    path
}

#[test]
fn issue494_guidance_is_loader_specific() {
    let root = tempfile::tempdir().unwrap();
    let c = config(root.path());
    let original = original_plan();
    let mut admission = admission::Admission::default();
    let admission::Decision::Retry(require_guidance) = admission
        .check(&c, None, &original, &mut original.clone(), 1)
        .unwrap()
    else {
        panic!("CommonJS original did not request formation")
    };
    assert!(require_guidance.contains("CommonJS require"));
    assert!(require_guidance.contains("const actual=require('./original-path.js')"));
    assert!(require_guidance.contains("printed value"));
    assert!(!require_guidance.contains("import('./original-path.js').then"));

    let mut dynamic = original.clone();
    dynamic.steps[0].verify = vec!["node -e \"import('./src/lib/types.ts')\"".into()];
    let mut dynamic_admission = admission::Admission::default();
    let admission::Decision::Retry(import_guidance) = dynamic_admission
        .check(&c, None, &dynamic, &mut dynamic.clone(), 1)
        .unwrap()
    else {
        panic!("dynamic import original did not request formation")
    };
    assert!(import_guidance.contains("pure dynamic import"));
    assert!(import_guidance.contains("import('./original-path.js').then"));
    assert!(!import_guidance.contains("const actual=require('./original-path.js')"));
}

#[test]
fn issue494_same_loader_reaches_formation_finish_and_exact_registration() {
    clear_trace();
    let root = tempfile::tempdir().unwrap();
    let mut c = config(root.path());
    let _guard = authority::begin_run(&c);
    let contract_path = empty_generated_contract(&c);
    let original = original_plan();
    let formed = positive_plan();
    let mut client = Replay::new(vec![proposal(&original), proposal(&formed)]);
    let returned = crate::planner::runner::generate_step_plan(
        &mut client,
        "Implement and verify shared modules",
        &c,
    )
    .unwrap_or_else(|error| panic!("{error}; {:?}", events(&c)));
    assert_eq!(returned, formed);

    let relevant = traces()
        .into_iter()
        .filter(|trace| {
            trace["attempt"] == 2
                && trace["original"]["steps"][0]["verify"][0] == fixture_command("original")
                && trace["formed"] == json!(formed)
        })
        .collect::<Vec<_>>();
    assert_eq!(relevant.len(), 2, "{relevant:#?}");
    assert_eq!(relevant[0]["stage"], "form_preserve_ok");
    assert_eq!(relevant[1]["stage"], "require_formed_result=ok");
    for trace in &relevant {
        let replacement = &trace["replacements"][0];
        assert_eq!(replacement["original_command"], fixture_command("original"));
        assert_eq!(replacement["import_target"], "./src/lib/types.ts");
        assert_eq!(replacement["expected_result"], "pass");
        assert_eq!(
            replacement["replacement_command"],
            fixture_command("positive")
        );
        assert!(
            trace["formed"]["steps"][0]["verify"]
                .as_array()
                .unwrap()
                .contains(&json!(fixture_command("positive")))
        );
    }

    assert!(super::super::lint(&c, &returned, None).is_pass());
    let quality = step_plan_quality_report(&returned, &PlanQualityContext::default());
    assert!(!quality.has_retryable_quality(), "{quality:?}");

    let mut admission = admission::Admission::default();
    assert!(matches!(
        admission
            .check(&c, None, &original, &mut original.clone(), 1)
            .unwrap(),
        admission::Decision::Retry(_)
    ));
    assert!(matches!(
        admission
            .check(&c, None, &returned, &mut returned.clone(), 2)
            .unwrap(),
        admission::Decision::Ready(false)
    ));
    assert_eq!(
        admission.finish(&c, None, returned.clone()).unwrap(),
        returned
    );

    let mut deleted = returned.clone();
    deleted.steps[0].verify.remove(0);
    assert!(
        admission
            .check(&c, None, &deleted, &mut deleted.clone(), 3)
            .is_err()
    );
    assert!(admission.finish(&c, None, deleted).is_err());

    let (expectation_admission, expectation_result) = direct(&c, &returned);
    assert!(matches!(
        expectation_result.unwrap(),
        admission::Decision::Ready(false)
    ));
    let mut changed_expectation = returned.clone();
    changed_expectation.steps[0].expected_result = "fail".into();
    assert!(
        expectation_admission
            .finish(&c, None, changed_expectation)
            .is_err()
    );

    scope::register(&c, &returned).unwrap();
    c.completion_contract_path = Some(contract_path.clone());
    let contract = CompletionContract::load_for_config(&c).unwrap().unwrap();
    assert!(
        contract
            .verify_commands
            .contains(&fixture_command("positive"))
    );
    assert!(
        !contract
            .verify_commands
            .contains(&fixture_command("original"))
    );

    let bytes = std::fs::read(&contract_path).unwrap();
    assert_eq!(std::fs::read(contract_path).unwrap(), bytes);
    assert_eq!(
        contract.verify_commands,
        CompletionContract::load_for_config(&c)
            .unwrap()
            .unwrap()
            .verify_commands
    );
}

#[test]
fn issue494_projection_rejects_cross_loader_saved_a_and_scope_losses() {
    let fixture = fixture();
    let original = original_plan();
    let positive = positive_plan();
    let (projected, records) =
        super::super::verifier_formation::project(&original, &positive, None, &[]);
    assert_eq!(records.len(), 1);
    assert_eq!(projected.steps[0].verify[0], fixture_command("original"));

    let moved = moved_positive_plan();
    let (moved_projected, moved_records) =
        super::super::verifier_formation::project(&original, &moved, None, &[]);
    assert_eq!(moved_records.len(), 1);
    assert_eq!(
        moved_projected.steps[1].verify[0],
        fixture_command("original")
    );
    let moved_root = tempfile::tempdir().unwrap();
    let moved_config = config(moved_root.path());
    let (_, moved_decision) = direct(&moved_config, &moved);
    assert!(matches!(
        moved_decision.unwrap(),
        admission::Decision::Ready(false)
    ));

    for name in ["saved_proposal_a", "dynamic_import_b"] {
        let mut candidate = positive.clone();
        candidate.steps[0].verify[0] = fixture_command(name);
        let (_, records) =
            super::super::verifier_formation::project(&original, &candidate, None, &[]);
        assert!(records.is_empty(), "{name}");
        let root = tempfile::tempdir().unwrap();
        let c = config(root.path());
        let (_, decision) = direct(&c, &candidate);
        let admission::Decision::Retry(reason) = decision.unwrap() else {
            panic!("{name} unexpectedly corresponded")
        };
        assert!(
            reason.contains(&fixture_command("original")),
            "{name}: {reason}"
        );
    }

    let dynamic = fixture_command("dynamic_import_b");
    assert_eq!(
        crate::minimal_loop::evidence::import_check::export_set_target(&dynamic).as_deref(),
        fixture["dynamic_import_b"]["recognized_target"].as_str()
    );
    let diagnostic_root = tempfile::tempdir().unwrap();
    let store_command = fixture["saved_store_check"].as_str().unwrap();
    let later = super::super::verifier_formation::require_formed(
        &config(diagnostic_root.path()),
        &[dynamic, store_command.into()],
    )
    .unwrap_err();
    assert!(later.to_string().contains(store_command));

    let cases = [
        "changed_expected_result",
        "explicit_output_loss",
        "incomplete_scope",
        "wrong_owner",
        "invalid_owner_order",
    ];
    assert_eq!(
        fixture["scope_refusals"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap())
            .collect::<Vec<_>>(),
        cases
    );
    for case in cases {
        let mut candidate = positive.clone();
        match case {
            "changed_expected_result" => candidate.steps[0].expected_result = "fail".into(),
            "explicit_output_loss" => {
                candidate.steps[0].expected_paths.pop();
            }
            "incomplete_scope" => candidate.steps[0].instruction = "Create only types.".into(),
            "wrong_owner" => {
                let command = candidate.steps[0].verify.remove(0);
                let mut wrong = step("unrelated-owner", "implement", &["notes.txt"]);
                wrong.instruction = "Write unrelated notes.".into();
                wrong.verify = vec![command];
                candidate.steps.push(wrong);
            }
            "invalid_owner_order" => {
                let command = candidate.steps[0].verify.remove(0);
                let mut early = step("early-check", "verify", &[]);
                early.instruction = "Run the shared module boundary check.".into();
                early.verify = vec![command];
                candidate.steps.insert(0, early);
            }
            _ => unreachable!(),
        }
        let root = tempfile::tempdir().unwrap();
        let c = config(root.path());
        let (_, decision) = direct(&c, &candidate);
        assert!(
            matches!(decision.unwrap(), admission::Decision::Retry(_)),
            "{case}"
        );
    }
}

#[test]
fn issue494_configured_and_recovery_contract_bytes_stay_frozen() {
    let fixture = fixture();
    let frozen = std::fs::read(fixture_root().join("frozen-contract.json")).unwrap();
    let expected_hash = fixture["frozen"]["contract_sha256"].as_str().unwrap();
    assert_eq!(format!("{:x}", Sha256::digest(&frozen)), expected_hash);
    for control in ["configured", "recovery"] {
        assert_eq!(
            fixture["contract_controls"][control]["sha256"],
            expected_hash
        );
        assert_eq!(
            fixture["contract_controls"][control]["verify_commands"],
            json!(["npm run build"])
        );
    }

    let configured_root = tempfile::tempdir().unwrap();
    let mut configured = config(configured_root.path());
    let configured_path = configured_root.path().join("configured.json");
    std::fs::write(&configured_path, &frozen).unwrap();
    configured.completion_contract_path = Some(configured_path.clone());
    let plan = original_plan();
    let mut admission = admission::Admission::default();
    assert!(matches!(
        admission
            .check(&configured, None, &plan, &mut plan.clone(), 1)
            .unwrap(),
        admission::Decision::Ready(false)
    ));
    assert_eq!(std::fs::read(configured_path).unwrap(), frozen);

    let recovery_root = tempfile::tempdir().unwrap();
    let mut recovery = config(recovery_root.path());
    let recovery_path = recovery_root.path().join("recovery.json");
    std::fs::write(&recovery_path, &frozen).unwrap();
    recovery.completion_contract_path = Some(recovery_path.clone());
    bind_context(&recovery);
    let mut rebound = original_plan();
    assert!(
        super::super::bind_generated(&recovery, Some("core-implementation"), &mut rebound).unwrap()
    );
    assert_eq!(rebound.steps.last().unwrap().verify, ["npm run build"]);
    assert_eq!(std::fs::read(recovery_path).unwrap(), frozen);
}
