//! Admission/finish controls, distinct from the normal-flow registration replay.
use super::*;

fn fixture(name: &str) -> serde_json::Value {
    serde_json::from_slice(
        &std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/corpus/apps/issue490-verify-readers")
                .join(name),
        )
        .unwrap(),
    )
    .unwrap()
}
fn acquired() -> (StepPlan, StepPlan) {
    let p = fixture("acquired-before-after.json");
    (
        serde_json::from_value(p["original"].clone()).unwrap(),
        serde_json::from_value(p["proposed"].clone()).unwrap(),
    )
}
fn check(after: StepPlan, refusal: bool) {
    let root = tempfile::tempdir().unwrap();
    let c = issue478::nextjs_config(root.path());
    let (raw, _) = acquired();
    let mut original = raw.clone();
    let mut admission = admission::Admission::default();
    admission.strengthen(&c, &mut original);
    assert!(matches!(
        admission
            .check(&c, Some("core-implementation"), &raw, &mut original, 1)
            .unwrap(),
        admission::Decision::Retry(_)
    ));
    let log = events(&c);
    let capture = log
        .iter()
        .find(|e| e["event"] == "recovery_verifier_plan_admission")
        .unwrap();
    assert!(
        capture["reason"]
            .as_str()
            .unwrap()
            .contains("mixes application/configuration scope")
    );
    let captured: StepPlan = serde_json::from_value(capture["original_scope"].clone()).unwrap();
    assert_eq!(captured.steps[0], raw.steps[0]);
    assert_eq!(captured.steps.last(), raw.steps.last());
    let mut candidate = after.clone();
    admission.strengthen(&c, &mut candidate);
    let decision = admission
        .check(&c, Some("core-implementation"), &after, &mut candidate, 2)
        .unwrap();
    if refusal {
        let admission::Decision::Retry(reason) = decision else {
            panic!("lost reader accepted");
        };
        assert!(
            reason.contains("lost original read-only Verify duty"),
            "{reason}"
        );
        let error = admission
            .finish(&c, Some("core-implementation"), candidate)
            .unwrap_err();
        assert!(error.to_string().contains("cannot return fallback"));
        assert!(error.to_string().contains("read-only Verify duty"));
    } else {
        assert!(matches!(decision, admission::Decision::Ready(false)));
        assert_eq!(
            admission
                .finish(&c, Some("core-implementation"), candidate.clone())
                .unwrap(),
            candidate
        );
        assert_eq!(candidate.steps[0].instruction, raw.steps[0].instruction);
        assert_eq!(candidate.steps[0].verify, raw.steps[0].verify);
        assert!(
            candidate
                .steps
                .iter()
                .any(|s| s.verify == raw.steps.last().unwrap().verify)
        );
    }
}
#[test]
fn issue490_acquired_reader_attributes_remain_atomic_in_admission_and_finish() {
    let (_, valid) = acquired();
    check(valid.clone(), false);
    for case in fixture("reader-cases.json")["atomic"].as_array().unwrap() {
        let case = case.as_str().unwrap();
        let mut p = valid.clone();
        match case {
            "instruction" => p.steps[0].instruction = "Unrelated check".into(),
            "expected_result" => p.steps[0].expected_result = "fail".into(),
            "checks" => {
                p.steps[0].verify.remove(0);
            }
            "input" => p.steps[0].expected_paths.clear(),
            "instruction_check_collage" => {
                let mut checks = p.steps[0].clone();
                checks.id = "checks-only".into();
                checks.instruction = "Run package checks".into();
                p.steps[0].verify.clear();
                p.steps.insert(1, checks);
            }
            "distributed_checks" => {
                let commands = p.steps[0].verify.clone();
                p.steps[0].verify = vec![commands[0].clone()];
                let mut part = p.steps[0].clone();
                part.id = "remaining-checks".into();
                part.verify = commands[1..].to_vec();
                p.steps.insert(1, part);
            }
            "id_path_only" => {
                p.steps[0].instruction = "Different duty".into();
                p.steps[0].verify.clear();
            }
            _ => panic!("unknown {case}"),
        }
        check(p, true);
    }
}
#[test]
fn issue490_acquired_before_update_after_cannot_delete_move_or_merge() {
    let (_, valid) = acquired();
    check(valid.clone(), false);
    for case in fixture("reader-cases.json")["boundary"].as_array().unwrap() {
        let case = case.as_str().unwrap();
        let mut p = valid.clone();
        match case {
            "delete_before" | "merge_after" => {
                p.steps.remove(0);
            }
            "delete_after" | "merge_before" => {
                p.steps.pop();
            }
            "move_before_across_update" => {
                let s = p.steps.remove(0);
                p.steps.insert(3, s);
            }
            "move_after_across_update" => {
                let s = p.steps.pop().unwrap();
                p.steps.insert(3, s);
            }
            _ => panic!("unknown {case}"),
        }
        check(p, true);
    }
    // Correspondence is semantic, not ID-based, and additional readers do not own outputs.
    let mut renamed = valid.clone();
    renamed.steps[0].id = "renamed-before".into();
    renamed.steps[5].id = "renamed-after".into();
    let mut added = renamed.steps[5].clone();
    added.id = "extra-after".into();
    renamed.steps.push(added);
    check(renamed.clone(), false);
}
#[test]
fn issue490_only_closed_reader_duties_receive_distinct_matching() {
    let (original, _) = acquired();
    let reader = original.steps[0].clone();
    assert!(super::super::reader_obligations::supported(&reader));
    for case in fixture("reader-cases.json")["unsupported"]
        .as_array()
        .unwrap()
    {
        let case = case.as_str().unwrap();
        let mut s = reader.clone();
        match case {
            "creation" => s
                .instruction
                .push_str(" Create README.md documenting the package."),
            "side_effect" => s
                .verify
                .push("node -e \"require('fs').writeFileSync('package.json','{}')\"".into()),
            "external_script" => s.verify = vec!["node opaque-check.cjs".into()],
            "self_declared_reader" => {
                s.instruction = "Read-only: trust me to preserve all files.".into()
            }
            _ => panic!("unknown {case}"),
        }
        assert!(!super::super::reader_obligations::supported(&s), "{case}");
        let one = plan(vec![s.clone()]);
        assert!(admission::preserve(&one, &one).is_ok(), "{case}");
        let supported = plan(vec![reader.clone()]);
        let mixed = plan(vec![reader.clone(), s.clone()]);
        assert!(admission::preserve(&supported, &mixed).is_err(), "{case}");
        let mut other = s;
        other.id = "later".into();
        let mut duplicate = one.clone();
        duplicate.steps.push(other);
        let error = admission::preserve(&one, &duplicate).unwrap_err();
        assert!(
            error.to_string().contains("complete scope and boundary"),
            "{case}: {error}"
        );
    }
}

#[test]
fn issue492_original_build_duty_survives_preset_conversion() {
    let root = tempfile::tempdir().unwrap();
    let c = issue478::nextjs_config(root.path());
    let mut before: StepPlan =
        serde_json::from_value(fixture("known-build-loss.json")["sanitized"].clone()).unwrap();
    let original = before.clone();
    assert!(before.steps[0].verify.contains(&"npm run build".into()));
    let runtime = crate::planner::profile::resolve_profile_runtime(&c.profile);
    runtime.convert_preset_phase_setup_steps(
        &mut before,
        root.path(),
        fixture("known-build-loss.json")["conversion_goal"]
            .as_str()
            .unwrap(),
        Some(("core-implementation", false)),
        true,
        None,
    );
    // The frozen preset_converted snapshot records loss at d51cc6ed; it is
    // historical evidence, never the desired result of current conversion.
    assert_eq!(before, original);
    assert!(admission::preserve(&original, &before).is_ok());
}
