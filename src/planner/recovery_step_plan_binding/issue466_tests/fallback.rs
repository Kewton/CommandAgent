use super::*;

#[test]
fn issue466_formation_then_invalid_replies_cannot_escape_via_setup_fallback() {
    for invalid in [
        "{invalid JSON",
        r#"{"steps":[{"id":"invalid","kind":"unknown","instruction":"Assess the scaffold","expected_paths":[],"verify":[],"expected_result":"pass"}]}"#,
    ] {
        for pending_obligation in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let mut c = config(root.path());
            c.profile = crate::planner::profile_descriptor::NEXTJS_PROFILE_ID.into();
            let goal = "Phase id: setup\nPhase task: scaffold the project";
            let mut mixed = step("mixed", "implement", &[SCRIPT, "package.json"]);
            mixed.verify = vec![format!("node {SCRIPT}")];
            let first = if pending_obligation {
                proposal(&plan(vec![mixed]))
            } else {
                AssistantReply::text(invalid)
            };
            let mut planner = Replay::new(vec![
                first,
                AssistantReply::text(invalid),
                AssistantReply::text(invalid),
            ]);
            let result = crate::planner::runner::generate_step_plan(&mut planner, goal, &c);
            assert_eq!(planner.requests.lock().unwrap().len(), 3);
            if pending_obligation {
                let error = result.unwrap_err();
                assert!(
                    error.to_string().contains("cannot return fallback"),
                    "{error}"
                );
                assert!(
                    events(&c)
                        .iter()
                        .any(|e| e["event"] == "recovery_verifier_plan_return_rejected")
                );
                assert!(!root.path().join(SCRIPT).exists());
            } else {
                let p = result.unwrap();
                assert_eq!(p.steps[0].id, "fallback-setup");
                assert!(p.steps[0].expected_paths.contains(&"package.json".into()));
            }
        }
    }
}
