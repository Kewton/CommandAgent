use super::*;

#[test]
fn issue466_generated_formation_retains_mixed_requirements_and_finite_budget() {
    let root = tempfile::tempdir().unwrap();
    let c = config(root.path());
    let mut mixed = step("mixed", "implement", &[SCRIPT, "package.json", "app.js"]);
    mixed.instruction = format!(
        "{INSTRUCTION} Also create package.json and app.js without dropping application behavior."
    );
    mixed.verify = vec![format!("node {SCRIPT}"), "test -f app.js".into()];
    let before = plan(vec![mixed.clone()]);
    let mut producer = step("verifier", "implement", &[SCRIPT]);
    producer.instruction = format!(
        "Only create the verifier in this step. Original requirement context: {}",
        mixed.instruction
    );
    let mut application = step("app-config", "implement", &["package.json", "app.js"]);
    application.instruction = format!(
        "Implement the application/configuration outputs in this step. Original requirement context: {}",
        mixed.instruction
    );
    let mut confirmation = mixed.clone();
    confirmation.id = "confirm-original-group".into();
    confirmation.kind = "verify".into();
    let after = plan(vec![producer, application, confirmation]);
    let mut planner = Replay::new(vec![proposal(&before), proposal(&after)]);
    let p = crate::planner::runner::generate_step_plan(&mut planner, &before.goal, &c)
        .unwrap_or_else(|e| panic!("{e}; events={:?}", events(&c)));
    assert_eq!(planner.requests.lock().unwrap().len(), 2);
    assert_eq!(
        p.steps
            .iter()
            .flat_map(|s| s.expected_paths.iter())
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        3
    );
    assert!(
        events(&c)
            .iter()
            .any(|e| e["event"] == "recovery_verifier_obligations_formed")
    );
    let mut dropped = after.clone();
    dropped.steps[1].expected_paths.clear();
    let mut planner = Replay::new(vec![
        proposal(&before),
        proposal(&dropped),
        proposal(&dropped),
    ]);
    assert!(crate::planner::runner::generate_step_plan(&mut planner, &before.goal, &c).is_err());
    assert_eq!(planner.requests.lock().unwrap().len(), 3);
    // Putting the original text on an unrelated owner cannot discharge app work.
    let mut misplaced = after.clone();
    misplaced.steps[1].instruction = "Create files without the original requirements".into();
    misplaced
        .steps
        .push(step("unrelated", "report", &["notes.txt"]));
    misplaced.steps[3].instruction = mixed.instruction;
    let mut planner = Replay::new(vec![
        proposal(&before),
        proposal(&misplaced),
        proposal(&misplaced),
    ]);
    assert!(crate::planner::runner::generate_step_plan(&mut planner, &before.goal, &c).is_err());
    assert!(
        serde_json::to_string(&planner.requests.lock().unwrap()[2])
            .unwrap()
            .contains("lost original requirements")
    );
}

#[test]
fn issue466_host_augmentation_is_measured_before_obligation_closure() {
    let root = tempfile::tempdir().unwrap();
    let mut c = config(root.path());
    c.profile = crate::planner::profile_descriptor::NEXTJS_PROFILE_ID.into();
    let mut producer = step("verifier", "implement", &[SCRIPT]);
    producer.verify = vec![format!("node {SCRIPT}")];
    let mut raw = plan(vec![producer]);
    raw.goal = "scaffold verifier".into();
    let mut host = raw.clone();
    crate::planner::recovery_step_plan_binding::profile_augmentation::strengthen_step_plan_for_profile(&mut host, &c);
    assert!(
        host.steps[0]
            .expected_paths
            .contains(&"package.json".into())
    );
    let mut admission = admission::Admission::default();
    assert!(matches!(
        admission.check(&c, None, &raw, &mut host, 1).unwrap(),
        admission::Decision::Retry(_)
    ));
    let log = events(&c);
    let e = log
        .iter()
        .find(|e| e["event"] == "recovery_verifier_plan_admission")
        .unwrap();
    assert_eq!(
        e["model_proposal"]["steps"][0]["expected_paths"],
        json!([SCRIPT])
    );
    assert!(
        e["host_augmented_proposal"]["steps"][0]["expected_paths"]
            .as_array()
            .unwrap()
            .contains(&json!("package.json"))
    );
    let mut corrected = host.clone();
    corrected.steps[0].expected_paths = vec![SCRIPT.into()];
    corrected.steps[0].verify.clear();
    let mut configuration = step("configuration", "implement", &["package.json"]);
    configuration.instruction = host.steps[0].instruction.clone();
    corrected.steps.push(configuration);
    let mut confirmation = host.steps[0].clone();
    confirmation.id = "confirm-group".into();
    confirmation.kind = "verify".into();
    corrected.steps.push(confirmation);
    let model = corrected.clone();
    crate::planner::recovery_step_plan_binding::profile_augmentation::strengthen_step_plan_for_profile(&mut corrected, &c);
    assert!(matches!(
        admission
            .check(&c, None, &model, &mut corrected, 2)
            .unwrap(),
        admission::Decision::Ready(false)
    ));
}

#[test]
fn issue466_compound_commands_share_registration_admission_and_nonexecuting_steps_cannot_register()
{
    for kind in ["implement", "verify", "inspect", "report", "setup"] {
        let root = tempfile::tempdir().unwrap();
        let mut c = config(root.path());
        let _guard = authority::begin_run(&c);
        let contract_path = generated_contract(&c);
        let mut empty: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&contract_path).unwrap()).unwrap();
        empty["verify_commands"] = json!([]);
        std::fs::write(&contract_path, serde_json::to_vec(&empty).unwrap()).unwrap();
        let producer = step("producer", "implement", &[SCRIPT]);
        let mut check = step("command-owner", kind, &[]);
        check.verify = vec!["test -f app.js && node smoke-check.js".into()];
        let mut source = plan(vec![
            step("application", "implement", &["app.js"]),
            producer,
            check,
        ]);
        if kind == "implement" {
            source.steps[1].verify = source.steps.pop().unwrap().verify;
        }
        if matches!(kind, "implement" | "verify") {
            // The actual Runner normalizes the same raw compound expression.
            let mut planner = Replay::new(vec![proposal(&source)]);
            let generated =
                crate::planner::runner::generate_step_plan(&mut planner, &source.goal, &c)
                    .unwrap_or_else(|e| panic!("{kind}: {e}; {:?}", events(&c)));
            scope::register(&c, &generated).unwrap();
            let producers = scope::generated(&c);
            assert_eq!(producers.len(), 1);
            assert_eq!(producers[0].expected_paths, [SCRIPT]);
            c.completion_contract_path = Some(contract_path);
            let contract = CompletionContract::load_for_config(&c).unwrap().unwrap();
            assert_eq!(contract.verify_commands, ["node smoke-check.js"]);
            assert!(contract.required_paths.contains(&"app.js".into()));
            bind_context(&c);
            let mut recovery = plan(vec![step("recovery-producer", "implement", &[SCRIPT])]);
            bind_generated(&c, Some("repair"), &mut recovery).unwrap();
            assert!(
                recovery
                    .steps
                    .last()
                    .unwrap()
                    .verify
                    .contains(&"node smoke-check.js".into())
            );
        } else {
            // Calling the registration boundary directly must not broaden the
            // prior Implement/Verify-only contract, even without a lint caller.
            scope::register(&c, &source).unwrap();
            assert!(scope::generated(&c).is_empty());
            c.completion_contract_path = Some(contract_path);
            assert!(
                CompletionContract::load_for_config(&c)
                    .unwrap()
                    .unwrap()
                    .verify_commands
                    .is_empty()
            );
        }
    }
}

// Issue #906: guidance toward the owner split admission already accepts. The
// separation rule, preserve, the 3-attempt budget and fallback are unchanged.
const ISSUE906_APP: &str =
    "Create index.html with a memo list saved to localStorage under the memos key.";
const ISSUE906_CHECK: &str = "Also create smoke-check.js that loads index.html with a small DOM mock and fails nonzero unless adding and deleting a memo works.";
const ISSUE906_NO_DUPLICATE: &str = "do not duplicate the combined instruction";

fn issue906_mixed() -> StepPlan {
    let mut mixed = step(
        "implement-app-and-check",
        "implement",
        &["index.html", SCRIPT],
    );
    mixed.instruction = format!("{ISSUE906_APP} {ISSUE906_CHECK}");
    let mut check = step("verify-app", "verify", &[]);
    check.instruction =
        "Run the listed deterministic verification commands without changing files.".into();
    check.verify = vec![format!("node {SCRIPT}")];
    plan(vec![mixed, check])
}

// The D2 template applied to the mixed plan: a role sentence plus the complete
// original instruction on every owner, with the original check kept in order.
fn issue906_template_split(mixed: &StepPlan) -> StepPlan {
    let original = &mixed.steps[0].instruction;
    let mut producer = step("create-smoke-check", "implement", &[SCRIPT]);
    producer.instruction =
        format!("Only create {SCRIPT} in this step. Original requirement context: {original}");
    let mut application = step("implement-app", "implement", &["index.html"]);
    application.instruction =
        format!("Only create index.html in this step. Original requirement context: {original}");
    plan(vec![producer, application, mixed.steps[1].clone()])
}

fn issue906_admissions(c: &Config) -> Vec<serde_json::Value> {
    events(c)
        .into_iter()
        .filter(|e| e["event"] == "recovery_verifier_plan_admission")
        .collect()
}

fn issue906_user_prompt(planner: &Replay, request: usize) -> String {
    planner.requests.lock().unwrap()[request]
        .iter()
        .filter(|m| m.role == "user")
        .map(|m| m.content.clone())
        .collect()
}

#[test]
fn issue906_template_split_after_mixed_owner_is_admitted_on_second_attempt() {
    let root = tempfile::tempdir().unwrap();
    let c = config(root.path());
    let mixed = issue906_mixed();
    let split = issue906_template_split(&mixed);
    let mut planner = Replay::new(vec![proposal(&mixed), proposal(&split)]);
    let p = crate::planner::runner::generate_step_plan(&mut planner, &mixed.goal, &c)
        .unwrap_or_else(|e| panic!("{e}; events={:?}", events(&c)));
    assert_eq!(planner.requests.lock().unwrap().len(), 2);
    let ids: Vec<_> = p.steps.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["create-smoke-check", "implement-app", "verify-app"]);
    let admissions = issue906_admissions(&c);
    assert_eq!(admissions.len(), 1);
    assert_eq!(admissions[0]["status"], "retry");
    assert_eq!(admissions[0]["planner_attempt"], 1);
    assert!(
        admissions[0]["reason"]
            .as_str()
            .unwrap()
            .contains("verifier producer mixes application/configuration scope")
    );
    assert!(
        events(&c)
            .iter()
            .any(|e| e["event"] == "recovery_verifier_obligations_formed")
    );
    assert!(!root.path().join(".commandagent/plans").exists());
}

#[test]
fn issue906_feedback_shows_the_accepted_template_with_outputs_and_checks() {
    let root = tempfile::tempdir().unwrap();
    let c = config(root.path());
    let mixed = issue906_mixed();
    let mut planner = Replay::new(vec![
        proposal(&mixed),
        proposal(&issue906_template_split(&mixed)),
    ]);
    crate::planner::runner::generate_step_plan(&mut planner, &mixed.goal, &c).unwrap();
    let feedback = issue906_user_prompt(&planner, 1);
    assert!(
        feedback.contains(&format!(
            "- Original step \"implement-app-and-check\" (implement, expected_result pass) owns [\"index.html\",\"{SCRIPT}\"] with checks []. Each owner instruction: \"Only create <this owner's expected_paths> in this step. Original requirement context: {}\"",
            mixed.steps[0].instruction
        )),
        "{feedback}"
    );
    assert!(
        feedback.contains(&format!(
            "- Original step \"verify-app\" (verify, expected_result pass): keep it with the same instruction, expected_result and checks [\"node {SCRIPT}\"]."
        )),
        "{feedback}"
    );
    assert!(feedback.contains("does not make that owner write another owner's files"));
    assert!(feedback.contains("verifier producer mixes application/configuration scope"));
    // Not a 2500-character capacity failure: no anti-duplication request.
    assert!(!feedback.contains(ISSUE906_NO_DUPLICATE), "{feedback}");
    assert!(!feedback.contains("2500 characters after deterministic notes"));
}

#[test]
fn issue906_feedback_keeps_no_duplicate_only_for_profile_instruction_capacity() {
    let root = tempfile::tempdir().unwrap();
    let c = super::issue478::nextjs_config(root.path());
    let mut raw = issue906_mixed();
    raw.steps.truncate(1);
    raw.steps[0].expected_paths = vec!["src/app/page.tsx".into()];
    let mut ambiguous = raw.clone();
    let mut admission = admission::Admission::default();
    admission.strengthen(&c, &mut ambiguous);
    ambiguous.steps.push(ambiguous.steps[0].clone());
    admission.sanitize(&mut ambiguous, Some(root.path()));
    let admission::Decision::Retry(feedback) =
        admission.check(&c, None, &raw, &mut ambiguous, 1).unwrap()
    else {
        panic!("ambiguous profile provenance unexpectedly passed")
    };
    assert!(feedback.contains("profile instruction"), "{feedback}");
    assert!(
        feedback.contains(&format!(
            "Keep model and host duties on their respective executable owners; {ISSUE906_NO_DUPLICATE}. The profile instruction must remain at most 2500 characters"
        )),
        "{feedback}"
    );
    assert!(
        feedback.contains(
            "Original requirement context: <the complete original instruction, unchanged>"
        )
    );
}

#[test]
fn issue906_generic_and_nextjs_formation_get_the_same_template() {
    let mut headers = Vec::new();
    for profile in [
        "generic",
        crate::planner::profile_descriptor::NEXTJS_PROFILE_ID,
    ] {
        let root = tempfile::tempdir().unwrap();
        let mut c = config(root.path());
        c.profile = profile.into();
        let mixed = issue906_mixed();
        let mut planner = Replay::new(vec![proposal(&mixed), proposal(&mixed), proposal(&mixed)]);
        let _ = crate::planner::runner::generate_step_plan(&mut planner, &mixed.goal, &c);
        let feedback = issue906_user_prompt(&planner, 1);
        let header = feedback
            .lines()
            .find(|l| l.starts_with("Accepted owner split:"))
            .unwrap_or_else(|| panic!("{profile}: {feedback}"))
            .to_string();
        // nextjs freezes the host-augmented instruction, so it may continue.
        assert!(
            feedback.contains(&format!(
                "Each owner instruction: \"Only create <this owner's expected_paths> in this step. Original requirement context: {}",
                mixed.steps[0].instruction
            )),
            "{profile}: {feedback}"
        );
        assert!(!feedback.contains(ISSUE906_NO_DUPLICATE), "{profile}");
        headers.push(header);
    }
    assert_eq!(headers[0], headers[1]);
}

#[test]
fn issue906_first_plan_prompt_separates_node_and_python_verifier_scripts() {
    for profile in [
        "generic",
        crate::planner::profile_descriptor::NEXTJS_PROFILE_ID,
    ] {
        let root = tempfile::tempdir().unwrap();
        let mut c = config(root.path());
        c.profile = profile.into();
        let split = issue906_template_split(&issue906_mixed());
        let mut planner = Replay::new(vec![proposal(&split), proposal(&split), proposal(&split)]);
        let _ = crate::planner::runner::generate_step_plan(&mut planner, &split.goal, &c);
        let requests = planner.requests.lock().unwrap();
        let system = &requests[0][0];
        assert_eq!(system.role, "system");
        for expected in [
            "give that script a dedicated implement step whose expected_paths contain only that script; keep application and configuration files in a separate implement step",
            "node smoke-check.js",
            "python3 smoke_check.py <args>",
            "do not put multiple statements inline in python3 -c",
        ] {
            assert!(system.content.contains(expected), "{profile}: {expected}");
        }
    }
}

fn issue906_rejected(bad: &StepPlan) -> (String, Vec<serde_json::Value>) {
    let root = tempfile::tempdir().unwrap();
    let c = config(root.path());
    let mixed = issue906_mixed();
    let mut planner = Replay::new(vec![proposal(&mixed), proposal(bad), proposal(bad)]);
    let error = crate::planner::runner::generate_step_plan(&mut planner, &mixed.goal, &c)
        .expect_err("a non-preserving split must stay rejected");
    assert_eq!(planner.requests.lock().unwrap().len(), 3);
    assert!(!root.path().join(".commandagent/plans").exists());
    (error.to_string(), issue906_admissions(&c))
}

#[test]
fn issue906_non_preserving_splits_stay_rejected() {
    let mixed = issue906_mixed();
    let accepted = issue906_template_split(&mixed);
    // Paraphrase-only split: the two halves concatenate to the original text.
    let mut paraphrase = accepted.clone();
    paraphrase.steps[0].instruction = ISSUE906_CHECK.into();
    paraphrase.steps[1].instruction = ISSUE906_APP.into();
    let mut missing_output = accepted.clone();
    missing_output.steps[1].expected_paths.clear();
    let mut missing_check = accepted.clone();
    missing_check.steps[2].verify.clear();
    let mut reversed = accepted.clone();
    reversed.steps.rotate_right(1);
    let mut unrelated = accepted.clone();
    unrelated.steps[0].instruction = format!("Only create {SCRIPT} in this step.");
    unrelated.steps[1].instruction = "Only create index.html in this step.".into();
    let mut report = step("report-requirements", "report", &[]);
    report.instruction = mixed.steps[0].instruction.clone();
    unrelated.steps.push(report);
    let lost = "owner implement-app of index.html lost original requirements/expected result from implement-app-and-check; unrelated owners cannot discharge this duty";
    for (name, bad, reason) in [
        ("paraphrase", paraphrase, lost),
        (
            "missing_output",
            missing_output,
            "formation dropped original required output owner index.html",
        ),
        (
            "missing_check",
            missing_check,
            "formation lost original check/expected result or its complete scope and boundary: node smoke-check.js; keep it on the complete owner or a Verify after all corresponding output owners",
        ),
        (
            "reversed",
            reversed,
            "formation moved original check before its output owners: node smoke-check.js",
        ),
        ("unrelated", unrelated, lost),
    ] {
        let (error, admissions) = issue906_rejected(&bad);
        assert_eq!(
            error,
            format!(
                "verifier plan admission exhausted existing 3-attempt planner budget: Recovery verifier binding ProposalRepairable: {reason}"
            ),
            "{name}"
        );
        let statuses: Vec<_> = admissions
            .iter()
            .map(|e| (e["planner_attempt"].clone(), e["status"].clone()))
            .collect();
        assert_eq!(
            statuses,
            [
                (json!(1), json!("retry")),
                (json!(2), json!("retry")),
                (json!(3), json!("exhausted"))
            ],
            "{name}"
        );
    }
}

#[test]
fn issue906_quality_policy_and_admission_share_three_attempts_without_stale_fallback() {
    let mixed = issue906_mixed();
    let mut paraphrase = issue906_template_split(&mixed);
    paraphrase.steps[0].instruction = ISSUE906_CHECK.into();
    paraphrase.steps[1].instruction = ISSUE906_APP.into();
    // Attempt 1 is a lint-clean plan with a weak check (quality retry) or a
    // shell-control check (policy retry), as in the recorded runs.
    let mut weak = step("implement-app", "implement", &["index.html", "app.js"]);
    weak.instruction = ISSUE906_APP.into();
    let mut weak_check = step("verify-app", "verify", &[]);
    weak_check.instruction = "Check the files exist.".into();
    weak_check.verify = vec!["test -f app.js".into()];
    let weak = plan(vec![weak, weak_check]);
    let mut policy = weak.clone();
    policy.steps[1].verify = vec!["python3 -c 'import sys\nsys.exit(0)".into()];
    for (first, first_event, stage) in [
        (&weak, "planner_quality_retry", "quality"),
        (&policy, "planner_error", "verify_policy"),
    ] {
        for (third, accepted) in [
            (&paraphrase, false),
            (&issue906_template_split(&mixed), true),
        ] {
            let root = tempfile::tempdir().unwrap();
            let c = config(root.path());
            let mut planner = Replay::new(vec![proposal(first), proposal(&mixed), proposal(third)]);
            let result = crate::planner::runner::generate_step_plan(&mut planner, &mixed.goal, &c);
            assert_eq!(
                planner.requests.lock().unwrap().len(),
                3,
                "{first_event}: {result:?}"
            );
            let log = events(&c);
            assert!(
                log.iter().any(|e| e["event"] == first_event
                    && e["planner_stage"] == stage
                    && e["repair_attempt"] == 1),
                "{first_event}: {log:?}"
            );
            let admissions: Vec<_> = issue906_admissions(&c)
                .into_iter()
                .map(|e| {
                    (
                        e["planner_attempt"].clone(),
                        e["status"].clone(),
                        e["remaining_planner_attempts"].clone(),
                    )
                })
                .collect();
            if accepted {
                let p = result.unwrap_or_else(|e| panic!("{first_event}: {e}"));
                assert_eq!(p.steps[0].id, "create-smoke-check");
                assert_eq!(admissions, [(json!(2), json!("retry"), json!(1))]);
            } else {
                let error = result
                    .expect_err("the third attempt cannot fall back")
                    .to_string();
                assert!(
                    error.starts_with(
                        "verifier plan admission exhausted existing 3-attempt planner budget"
                    ),
                    "{first_event}: {error}"
                );
                assert_eq!(
                    admissions,
                    [
                        (json!(2), json!("retry"), json!(1)),
                        (json!(3), json!("exhausted"), json!(0))
                    ]
                );
                assert!(!log.iter().any(|e| e["event"] == "planner_fallback_plan"));
                assert!(!root.path().join(".commandagent/plans").exists());
            }
        }
    }
}
