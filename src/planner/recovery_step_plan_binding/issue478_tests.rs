use super::*;
use crate::planner::profile_descriptor::NEXTJS_PROFILE_ID;
use sha2::{Digest, Sha256};

fn saved(attempt: usize) -> StepPlan {
    serde_json::from_str(match attempt {
        2 => include_str!(
            "../../../tests/corpus/apps/issue478-model-host-obligations/attempt-2.json"
        ),
        3 => include_str!(
            "../../../tests/corpus/apps/issue478-model-host-obligations/attempt-3.json"
        ),
        _ => unreachable!(),
    })
    .unwrap()
}

fn nextjs_config(root: &Path) -> Config {
    // The saved run already had its application scaffold; an empty workspace
    // exercises a different sanitizer retyping boundary.
    std::fs::write(root.join("package.json"), r#"{"name":"issue478","private":true,"scripts":{"dev":"next dev -p 60302","start":"next start -p 60302","build":"next build"},"dependencies":{"next":"15.5.20","react":"19.1.0","react-dom":"19.1.0"}}"#).unwrap();
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
    let mut c = config(root);
    c.profile = NEXTJS_PROFILE_ID.into();
    c
}

fn replay(c: &Config, after: &StepPlan) -> (anyhow::Result<StepPlan>, Replay) {
    let mut client = Replay::new(vec![proposal(&saved(2)), proposal(after), proposal(after)]);
    let result = crate::planner::runner::generate_step_plan_with_ui_for_phase(
        &mut client,
        &saved(2).goal,
        c,
        &crate::tui::NOOP_UI,
        Some("contract-wiring"),
        true,
        false,
    );
    (result, client)
}

#[test]
fn issue478_saved_split_preserves_executable_model_and_host_duties() {
    let root = tempfile::tempdir().unwrap();
    let c = nextjs_config(root.path());
    let before = saved(2);
    let after = saved(3);
    let smoke = &before.steps[2];
    assert_eq!(smoke.instruction.chars().count(), 587);
    assert_eq!(
        format!("{:x}", Sha256::digest(smoke.instruction.as_bytes())),
        "75c69f8375ad4c8492158c3f65745a6532a82f26850cab51901015e8faa492e0"
    );
    assert_eq!(after.steps[2], *smoke);
    assert!(!after.steps[3].instruction.contains(&smoke.instruction));
    let (result, client) = replay(&c, &after);
    let formed = result.unwrap_or_else(|e| {
        for event in events(&c) {
            if event["event"] == "recovery_verifier_plan_admission" {
                eprintln!("{}", serde_json::to_string_pretty(&event).unwrap());
            }
        }
        panic!("{e}")
    });
    assert_eq!(client.requests.lock().unwrap().len(), 2);
    assert_eq!(formed.steps[2], *smoke);
    assert_eq!(formed.steps[3].step_kind(), StepKind::Implement);
    assert_eq!(formed.steps[3].expected_paths, ["package.json"]);
    let original_package_checks: Vec<String> = serde_json::from_str(include_str!(
        "../../../tests/corpus/apps/issue478-model-host-obligations/package-checks.json"
    ))
    .unwrap();
    assert_eq!(formed.steps[3].verify, original_package_checks);
    assert!(
        formed.steps[3]
            .instruction
            .contains(&after.steps[3].instruction)
    );
    assert_eq!(formed.steps[4].verify, ["node smoke-check.js"]);
    assert_eq!(formed.steps[5].verify, ["npm run build"]);
    let log = events(&c);
    let retained = log
        .iter()
        .find(|e| e["event"] == "recovery_verifier_plan_admission")
        .unwrap();
    assert_eq!(retained["remaining_planner_attempts"], 2);
    assert_eq!(retained["recovery_budget_changed"], false);
    assert_eq!(
        retained["original_obligation_sources"]["model"]["steps"][2],
        json!(smoke)
    );
    let host = &retained["original_obligation_sources"]["host"]["steps"][0];
    assert_eq!(host["expected_paths"], json!(["package.json"]));
    let full_guidance = host["instruction"].as_str().unwrap();
    assert!(formed.steps[3].instruction.contains(full_guidance));
    assert!(!formed.steps[2].instruction.contains(full_guidance));
    assert!(!formed.steps[3].instruction.contains(&smoke.instruction));
    assert!(crate::planner::lint::lint_plan_for_execution(&formed, Some(root.path())).is_pass());

    // Report actual product scopes, without storing the entire raw event log.
    if let Ok(path) = std::env::var("ISSUE478_FORMATION_EVIDENCE") {
        std::fs::write(
            path,
            serde_json::to_vec_pretty(&json!({
                "source":"issue478_saved_split_preserves_executable_model_and_host_duties",
                "model_raw":before.steps,
                "host_augmented":retained["host_augmented_proposal"]["steps"],
                "original_obligation_sources":retained["original_obligation_sources"],
                "split_model_raw":after.steps,
                "formed":formed.steps,
                "planner_requests":client.requests.lock().unwrap().len(),
                "recovery_budget_changed":false,
                "execution":"formation and lint only; no application build or live model call"
            }))
            .unwrap(),
        )
        .unwrap();
    }
}

#[test]
fn issue478_runner_refuses_lost_model_duties_and_premature_checks() {
    let cases: Vec<String> = serde_json::from_str(include_str!(
        "../../../tests/corpus/apps/issue478-model-host-obligations/refusals.json"
    ))
    .unwrap();
    for case in cases {
        let root = tempfile::tempdir().unwrap();
        let c = nextjs_config(root.path());
        let mut after = saved(3);
        match case.as_str() {
            "delete_assertion" => {
                after.steps[2].instruction = after.steps[2]
                    .instruction
                    .replace("(e) it contains data-anvil-action=\"restart\".", "")
            }
            "change_result" => after.steps[2].expected_result = "fail".into(),
            "missing_output" => after.steps[2].expected_paths.clear(),
            "unrelated_owner" => {
                let mut unrelated = after.steps[2].clone();
                unrelated.id = "notes".into();
                unrelated.kind = "report".into();
                unrelated.expected_paths = vec!["notes.txt".into()];
                after.steps[2].instruction = "Write a smoke-check.js that prints success.".into();
                after.steps.push(unrelated);
            }
            "delete_check" => after.steps[4].verify.clear(),
            "change_check_result" => after.steps[4].expected_result = "fail".into(),
            "check_before_smoke" => after.steps.swap(2, 4),
            "check_before_application" => {
                let app = after.steps.remove(1);
                after.steps.insert(4, app);
            }
            _ => panic!("unknown refusal fixture: {case}"),
        }
        let (result, client) = replay(&c, &after);
        assert!(result.is_err(), "{case} unexpectedly passed");
        assert_eq!(client.requests.lock().unwrap().len(), 3, "{case}");
        assert!(!root.path().join(SCRIPT).exists(), "{case}");
    }
}

fn through_formation(c: &Config, admission: &mut admission::Admission, raw: &StepPlan) -> StepPlan {
    use crate::planner::step_plan::repair_generated_step_plan_contract;
    let mut p = raw.clone();
    repair_generated_step_plan_contract(&mut p);
    admission.strengthen(c, &mut p);
    repair_generated_step_plan_contract(&mut p);
    let runtime = crate::planner::profile::resolve_profile_runtime(&c.profile);
    runtime.canonicalize_create_plan(&mut p, true, false, None);
    crate::planner::sanitizer::sanitize_step_plan_against_policy(&mut p, Some(&c.workspace_root));
    runtime.convert_preset_phase_setup_steps(
        &mut p,
        &c.workspace_root,
        &raw.goal,
        Some(("contract-wiring", false)),
        true,
        None,
    );
    p
}

#[test]
fn issue478_admission_and_fallback_refuse_missing_or_nonexecuting_host_duties() {
    for case in [
        "instruction",
        "result",
        "output",
        "unrelated",
        "verify_only",
    ] {
        let root = tempfile::tempdir().unwrap();
        let c = nextjs_config(root.path());
        let mut admission = admission::Admission::default();
        let mut before = through_formation(&c, &mut admission, &saved(2));
        assert!(matches!(
            admission
                .check(&c, Some("contract-wiring"), &saved(2), &mut before, 1)
                .unwrap(),
            admission::Decision::Retry(_)
        ));
        let mut after = through_formation(&c, &mut admission, &saved(3));
        match case {
            "instruction" => after.steps[3].instruction = "Check package scripts only.".into(),
            "result" => after.steps[3].expected_result = "fail".into(),
            "output" => after.steps[3].expected_paths.clear(),
            "unrelated" => {
                let mut unrelated = after.steps[3].clone();
                unrelated.id = "notes".into();
                unrelated.expected_paths = vec!["notes.txt".into()];
                after.steps[3].instruction = "Check package scripts only.".into();
                after.steps.push(unrelated);
            }
            "verify_only" => {
                after.steps[3].kind = "verify".into();
                after.steps[3].verify = vec!["test -f package.json".into()];
            }
            _ => unreachable!(),
        }
        assert!(
            matches!(
                admission
                    .check(&c, Some("contract-wiring"), &saved(3), &mut after, 2)
                    .unwrap(),
                admission::Decision::Retry(_)
            ),
            "{case}"
        );
        assert!(
            admission
                .finish(&c, Some("contract-wiring"), after)
                .is_err(),
            "{case}"
        );
    }
}

#[test]
fn issue478_profile_owner_cannot_short_circuit_at_package_checks() {
    let root = tempfile::tempdir().unwrap();
    let c = nextjs_config(root.path());
    let formed = replay(&c, &saved(3)).0.unwrap();
    let owner = &formed.steps[3];
    let runtime = crate::planner::profile::resolve_profile_runtime(NEXTJS_PROFILE_ID);
    let (execution, synthesized) = runtime.runtime_step_with_profile_checks(
        root.path(),
        &formed.goal,
        owner,
        Some("contract-wiring"),
        None,
    );
    assert!(!synthesized);
    assert_eq!(execution, *owner);
    assert!(!runtime.step_short_circuit_precheck_applicable(&execution));
    let check = || {
        crate::planner::verify::verify_step_with_profile_setup_observed_with_offline(
            root.path(),
            owner,
            Some(NEXTJS_PROFILE_ID),
            crate::minimal_loop::dependency_setup::NodeDependencySetupAuthority::None,
            true,
        )
        .0
    };
    assert!(check().is_pass());
    let path = root.path().join("package.json");
    let mut package: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    package["scripts"]["dev"] = json!("next dev -p 3000");
    std::fs::write(path, serde_json::to_vec(&package).unwrap()).unwrap();
    assert!(!check().is_pass(), "the retained host port check must fail");
    for instruction in [
        owner.instruction.clone(),
        format!(
            "Maintain package.json.\n\nProfile contract:\n{}",
            owner.instruction
        ),
    ] {
        for next_field in [
            "Required final artifacts",
            "Artifacts available from previous steps",
        ] {
            let prompt = format!(
                "Current step id:\nensure-package-json\n\nCurrent step kind:\nimplement\n\nCurrent step instruction:\n{instruction}\n\n{next_field}:\n- package.json\n\nExpected paths after this step:\n- package.json\n\nVerification commands for this step:\n- none"
            );
            assert!(
                !crate::planner::setup_step_policy::prompt_references_template_owned_artifacts(
                    NEXTJS_PROFILE_ID,
                    &prompt
                )
            );
        }
    }
}

#[test]
fn issue478_model_profile_marker_is_not_host_provenance() {
    let root = tempfile::tempdir().unwrap();
    let c = nextjs_config(root.path());
    let mut raw = saved(2);
    raw.steps[2]
        .instruction
        .push_str("\n\nProfile contract:\nAlso reject an empty source file.");
    let mut admission = admission::Admission::default();
    let mut before = through_formation(&c, &mut admission, &raw);
    assert!(matches!(
        admission.check(&c, None, &raw, &mut before, 1).unwrap(),
        admission::Decision::Retry(_)
    ));
    let mut split = saved(3);
    split.steps[2] = raw.steps[2].clone();
    let mut formed = through_formation(&c, &mut admission, &split);
    assert!(matches!(
        admission.check(&c, None, &split, &mut formed, 2).unwrap(),
        admission::Decision::Ready(false)
    ));
    formed.steps[2].instruction = saved(3).steps[2].instruction.clone();
    assert!(admission.check(&c, None, &split, &mut formed, 3).is_err());
}

#[test]
fn issue478_first_split_cannot_lose_host_guidance_to_truncation() {
    let root = tempfile::tempdir().unwrap();
    let c = nextjs_config(root.path());
    let mut raw = saved(3);
    raw.steps[3]
        .instruction
        .insert_str(0, "Maintain the existing package. ");
    let mut admission = admission::Admission::default();
    let mut formed = through_formation(&c, &mut admission, &raw);
    let guidance = crate::planner::profile::resolve_profile_runtime(NEXTJS_PROFILE_ID)
        .guidance(&raw.goal)
        .unwrap();
    assert!(!formed.steps[3].instruction.contains(&guidance));
    assert!(matches!(
        admission.check(&c, None, &raw, &mut formed, 1).unwrap(),
        admission::Decision::Retry(_)
    ));
    assert!(admission.finish(&c, None, formed).is_err());
}
