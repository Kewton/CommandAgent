use super::*;

#[test]
fn issue484_capture_tracks_retained_proposal_after_ready_then_lint_retry() {
    let root = tempfile::tempdir().unwrap();
    let c = config(root.path());
    let mut admission = admission::Admission::default();
    let mut first = single_owner("Update package.json scripts so that dev is 'next dev -p 1111'.");
    first.steps[0].id.clear();
    first.steps[0].verify = vec!["test -f package.json".into()];
    let raw = first.clone();
    admission.strengthen(&c, &mut first);
    assert!(matches!(
        admission.check(&c, None, &raw, &mut first, 1).unwrap(),
        admission::Decision::Ready(false)
    ));
    assert!(!crate::planner::lint::lint_plan_for_execution(&first, Some(root.path())).is_pass());

    let raw = single_owner(&saved(false).steps[4].instruction);
    let mut second = raw.clone();
    admission.strengthen(&c, &mut second);
    assert!(matches!(
        admission.check(&c, None, &raw, &mut second, 2).unwrap(),
        admission::Decision::Retry(_)
    ));
    let retained = events(&c)
        .into_iter()
        .find(|e| e["event"] == "recovery_verifier_plan_admission")
        .unwrap();
    assert_eq!(retained["planner_attempt"], 2);
    let formation = &retained["package_script_formation"];
    assert_eq!(
        formation["obligations"][0]["expected_literal"],
        "next dev -p 60302"
    );
    assert_eq!(formation["sources"]["plans"]["model"], json!(raw));
    assert_eq!(retained["original_scope"], json!(second));

    let mut third = single_owner("Update package.json scripts so that dev is 'next dev -p 3000'.");
    third.steps[0].verify = vec![check().replace("60302", "3000")];
    let raw = third.clone();
    admission.strengthen(&c, &mut third);
    assert!(admission.check(&c, None, &raw, &mut third, 3).is_err());
    let last = events(&c)
        .into_iter()
        .rfind(|e| e["event"] == "recovery_verifier_plan_admission")
        .unwrap();
    assert_eq!(
        last["package_script_formation"], *formation,
        "retain must freeze the matching second proposal, not first or third"
    );
    assert!(admission.finish(&c, None, third).is_err());
}

#[test]
fn issue484_double_strengthen_without_check_is_an_explicit_caller_failure() {
    let root = tempfile::tempdir().unwrap();
    let c = config(root.path());
    let mut admission = admission::Admission::default();
    let mut candidate = single_owner(&saved(false).steps[4].instruction);
    let raw = candidate.clone();
    admission.strengthen(&c, &mut candidate);
    admission.strengthen(&c, &mut candidate);
    let error = admission
        .check(&c, None, &raw, &mut candidate, 1)
        .err()
        .unwrap();
    assert!(error.to_string().contains("duplicate strengthen"));
    assert!(admission.finish(&c, None, candidate).is_err());
}
