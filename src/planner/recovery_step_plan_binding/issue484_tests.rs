use super::super::formation_scope::{FormationScope, ProfileAddition};
use super::super::package_script_formation::{CapturedSources, PackageScripts, hash};
use super::*;
use crate::minimal_loop::evidence::package_script_check as grammar;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/corpus/apps/issue484-package-script-formation")
            .join(name),
    )
    .unwrap()
}

fn saved(formed: bool) -> StepPlan {
    serde_json::from_str(&fixture(if formed {
        "formed-plan.json"
    } else {
        "original-plan.json"
    }))
    .unwrap()
}

fn check() -> String {
    saved(true).steps[6].verify[0].clone()
}

fn replay(c: &Config, before: &StepPlan, after: &StepPlan) -> (anyhow::Result<StepPlan>, Replay) {
    let mut client = Replay::new(vec![proposal(before), proposal(after), proposal(after)]);
    let result = crate::planner::runner::generate_step_plan_with_ui_for_phase(
        &mut client,
        &before.goal,
        c,
        &crate::tui::NOOP_UI,
        Some("core-implementation"),
        true,
        false,
    );
    (result, client)
}

#[test]
fn issue484_saved_runner_forms_exact_literal_and_records_both_sources() {
    let root = tempfile::tempdir().unwrap();
    let c = super::issue478::nextjs_config(root.path());
    let before = saved(false);
    let after = saved(true);
    // Control for the pre-fix equivalence path: pure-import projection cannot
    // discharge this saved value display, even though the candidate is valid.
    let (unformed, records) = super::super::verifier_formation::project(&before, &after, None);
    assert!(records.is_empty());
    assert!(admission::preserve(&before, &unformed).is_err());
    let (result, client) = replay(&c, &before, &after);
    let formed = result.unwrap_or_else(|e| {
        let reasons: Vec<_> = events(&c)
            .into_iter()
            .filter(|event| event["event"] == "recovery_verifier_plan_admission")
            .map(|event| event["reason"].clone())
            .collect();
        panic!("{e:#}\n{}", json!(reasons))
    });
    assert_eq!(client.requests.lock().unwrap().len(), 2);
    assert!(formed.steps[6].verify.contains(&check()));
    let log = events(&c);
    let rejected = log
        .iter()
        .find(|e| e["event"] == "recovery_verifier_plan_admission")
        .unwrap();
    assert_eq!(rejected["status"], "retry");
    assert_eq!(rejected["remaining_planner_attempts"], 2);
    assert_eq!(rejected["recovery_budget_changed"], false);
    let event = log
        .iter()
        .find(|e| e["event"] == "preclosure_verifier_replacements_validated")
        .unwrap();
    let formation = &event["package_script_formation"];
    assert_eq!(
        formation["sources"]["acquisition_stage"],
        "profile_augmentation_before_sanitization"
    );
    let plans = &formation["sources"]["plans"];
    for side in ["model", "host"] {
        let plan: StepPlan = serde_json::from_value(plans[side].clone()).unwrap();
        assert_eq!(
            formation["sources"][format!("{side}_sha256")],
            hash(&serde_json::to_vec(&plan).unwrap())
        );
    }
    let obligation = &event["replacements"][0]["package_script"];
    assert_eq!(obligation["original_command"], before.steps[4].verify[0]);
    assert_eq!(
        obligation["original_command_sha256"],
        hash(before.steps[4].verify[0].as_bytes())
    );
    assert_eq!(
        obligation["replacement_command_sha256"],
        hash(check().as_bytes())
    );
    assert_eq!(obligation["expected_literal"], "next dev -p 60302");
    assert_eq!(obligation["expected_result"], "pass");
    assert_eq!(
        obligation["original_check_owner"],
        "configure-port-and-scripts"
    );
    assert_eq!(obligation["original_step_index"], 4);
    let sources = obligation["sources"].as_array().unwrap();
    assert_eq!(sources.len(), 2);
    assert_eq!(sources[0]["fixed_literals"], json!(["next dev -p 60302"]));
    assert_eq!(
        sources[1]["fixed_literals"],
        json!([]),
        "host port constraint is not a fixed literal"
    );
    for source in sources {
        let step: PlanStep = serde_json::from_value(
            event
                .pointer(source["json_pointer"].as_str().unwrap())
                .unwrap()
                .clone(),
        )
        .unwrap();
        assert_eq!(source["owner"], step.id);
        assert_eq!(
            source["step_sha256"],
            hash(&serde_json::to_vec(&step).unwrap())
        );
        assert_eq!(
            source["instruction_sha256"],
            hash(step.instruction.as_bytes())
        );
        assert!(
            formed
                .steps
                .iter()
                .any(|owner| owner.instruction == step.instruction
                    && owner.expected_paths == ["package.json"]
                    && owner.step_kind() == StepKind::Implement)
        );
    }
    let manifest: serde_json::Value =
        serde_json::from_str(&fixture("source-manifest.json")).unwrap();
    for (name, digest) in manifest["fixture_sha256"].as_object().unwrap() {
        assert_eq!(
            hash(fixture(name).as_bytes()),
            digest.as_str().unwrap(),
            "{name}"
        );
    }
    if let Ok(path) = std::env::var("ISSUE484_FORMATION_EVIDENCE") {
        std::fs::write(
            path,
            serde_json::to_vec_pretty(&json!({
                "source_manifest":manifest, "initial_rejection":rejected,
                "validated":event, "planner_requests":2,
                "execution":"offline saved replay; no live model or application build"
            }))
            .unwrap(),
        )
        .unwrap();
    }
}

#[test]
fn issue484_saved_runner_refuses_scope_output_failure_and_literal_changes() {
    let cases: Vec<String> = serde_json::from_str(&fixture("refusals.json")).unwrap();
    for case in cases {
        let root = tempfile::tempdir().unwrap();
        let c = super::issue478::nextjs_config(root.path());
        let before = saved(false);
        let mut after = saved(true);
        match case.as_str() {
            "wrong_target" => {
                after.steps[6].verify[0] = check().replace("./package.json", "./other.json")
            }
            "changed_literal" | "artifact_derived_literal" => {
                after.steps[6].verify[0] = check().replace("60302", "3000");
                if case == "artifact_derived_literal" {
                    std::fs::write(
                        root.path().join("package.json"),
                        fixture("package.json").replace("60302", "3000"),
                    )
                    .unwrap();
                    after.steps[4].instruction =
                        after.steps[4].instruction.replace("60302", "3000");
                }
            }
            "changed_script" => {
                after.steps[6].verify[0] = check().replace("scripts.dev", "scripts.start")
            }
            "delete_check" => after.steps[6].verify.clear(),
            "changed_result" => after.steps[4].expected_result = "fail".into(),
            "unrelated_assertion" => {
                after.steps[6].verify[0] =
                    "node -e \"require('node:assert/strict').strictEqual(1,1)\"".into()
            }
            "missing_owner" => {
                after.steps.remove(4);
            }
            "nonexecuting_owner" => after.steps[4].kind = "report".into(),
            "premature_check" => {
                let mut verification = after.steps[6].clone();
                verification.id = "premature-package-check".into();
                verification.kind = "verify".into();
                verification.expected_paths.clear();
                after.steps[6].verify.clear();
                after.steps.insert(1, verification);
            }
            "swallow_failure" => after.steps[6].verify[0] = format!("{} || true", check()),
            "wrapper" => {
                after.steps[6].verify[0] = check()
                    .replace("const actual=", "try { const actual=")
                    .replace("console.log(actual)", "console.log(actual) } catch {}")
            }
            "output_suppression" => {
                after.steps[6].verify[0] = check().replace(";console.log(actual)", "")
            }
            "append_beside_original" => after.steps[4]
                .verify
                .push(before.steps[4].verify[0].clone()),
            "drop_other_key_duty" => {
                after.steps[4].instruction = after.steps[4]
                    .instruction
                    .replace(" and start is 'next start -p 60302'", "")
            }
            _ => panic!("{case}"),
        }
        let (result, client) = replay(&c, &before, &after);
        assert!(result.is_err(), "{case} was admitted");
        assert_eq!(client.requests.lock().unwrap().len(), 3, "{case}");
    }
}

fn single_owner(instruction: &str) -> StepPlan {
    let mut p = plan(vec![saved(false).steps[4].clone()]);
    p.steps[0].instruction = instruction.into();
    p
}

fn direct(
    c: &Config,
    before: &StepPlan,
    after: &StepPlan,
) -> (admission::Decision, admission::Admission) {
    let mut admission = admission::Admission::default();
    let first = admission
        .check(c, None, before, &mut before.clone(), 1)
        .unwrap();
    assert!(matches!(first, admission::Decision::Retry(_)));
    (
        admission
            .check(c, None, after, &mut after.clone(), 2)
            .unwrap(),
        admission,
    )
}

#[test]
fn issue484_unsupported_commands_and_unsafe_literals_never_gain_admission() {
    let controls: serde_json::Value =
        serde_json::from_str(&fixture("unsupported-sources.json")).unwrap();
    let root = tempfile::tempdir().unwrap();
    let c = config(root.path());
    for command in controls["commands"].as_array().unwrap() {
        let mut before = single_owner(&saved(false).steps[4].instruction);
        before.steps[0].verify = vec![command.as_str().unwrap().into()];
        let mut after = before.clone();
        after.steps[0].verify = vec![check()];
        assert!(
            matches!(direct(&c, &before, &after).0, admission::Decision::Retry(_)),
            "{command}"
        );
    }
    for value in controls["unsafe_literals"].as_array().unwrap() {
        let value = value.as_str().unwrap();
        assert!(!grammar::literal(value));
        let before = single_owner(&format!(
            "Update package.json scripts so that dev is '{value}'."
        ));
        let mut after = before.clone();
        after.steps[0].verify = vec![check()];
        let (decision, admission) = direct(&c, &before, &after);
        assert!(
            matches!(decision, admission::Decision::Retry(_)),
            "{value:?}"
        );
        assert!(admission.finish(&c, None, after).is_err());
    }
    for instruction in [
        "Maintain package.json.",
        "Update package.json scripts so that dev is 'next dev -p 60302' or 'next dev --port 60302'.",
        "Update package.json scripts so that dev is 'next dev -p 60302'. Or dev may use port 3000.",
        "Update package.json scripts so that dev is 'next dev -p 60302'. Or use any port.",
        "Set dev to the currently generated value.",
    ] {
        let before = single_owner(instruction);
        let mut after = before.clone();
        after.steps[0].verify = vec![check()];
        assert!(
            matches!(direct(&c, &before, &after).0, admission::Decision::Retry(_)),
            "{instruction}"
        );
    }
}

#[test]
fn issue484_model_host_literals_are_checked_per_key_without_precedence() {
    let root = tempfile::tempdir().unwrap();
    let c = config(root.path());
    let before = single_owner(
        "Update package.json scripts so that dev is 'next dev -p 60302' and start is 'next start -p 60302'.",
    );
    let empty: CompletionContract = serde_json::from_value(json!({})).unwrap();
    for (host, accepted) in [
        (
            "Update package.json scripts so that dev is 'next dev -p 60302'.",
            true,
        ),
        (
            "Update package.json scripts so that dev is 'next dev -p 3000'.",
            false,
        ),
        (
            "Update package.json scripts so that start is 'next start -p 60302'.",
            true,
        ),
    ] {
        let addition = ProfileAddition::new(before.steps[0].clone(), &before.steps[0], host.into());
        let sources = CapturedSources::new(
            &c,
            FormationScope::capture(&before, Some(&addition)),
            "test_saved_original_boundary",
        );
        let catalog = PackageScripts::capture(&c, &before, sources, &empty);
        assert_eq!(
            catalog.obligations.len(),
            usize::from(accepted),
            "{host}: {}",
            catalog.guidance()
        );
        let mut after = before.clone();
        after.steps[0].verify = vec![check()];
        after.steps[0].instruction.push_str(&format!("\n{host}"));
        let (projected, records) =
            super::super::verifier_formation::project(&before, &after, Some(&catalog));
        assert_eq!(records.len(), usize::from(accepted));
        assert_eq!(admission::preserve(&before, &projected).is_ok(), accepted);
    }
    // Duplicate declarations of one key are checked as a set, not last-wins.
    for (extra, accepted) in [("next dev -p 60302", true), ("next dev -p 3000", false)] {
        let p = single_owner(&format!(
            "Update package.json scripts so that dev is 'next dev -p 60302' and dev is '{extra}'."
        ));
        let catalog = PackageScripts::capture(
            &c,
            &p,
            CapturedSources::new(&c, FormationScope::capture(&p, None), "test"),
            &empty,
        );
        assert_eq!(catalog.obligations.len(), usize::from(accepted));
    }
}

#[path = "issue484_runtime_tests.rs"]
mod runtime;

#[path = "issue484_owner_tests.rs"]
mod owner_scope;

#[path = "issue484_capture_tests.rs"]
mod capture;
