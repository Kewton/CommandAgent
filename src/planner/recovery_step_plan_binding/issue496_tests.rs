use super::*;
use crate::planner::profile_descriptor::NEXTJS_PROFILE_ID;
use sha2::{Digest, Sha256};

const FIXTURE: &str = "tests/corpus/apps/issue496-profile-guidance-capacity/saved-plan.json";

fn saved() -> StepPlan {
    let fixture: serde_json::Value = serde_json::from_slice(
        &std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE)).unwrap(),
    )
    .unwrap();
    serde_json::from_value(fixture["plan"].clone()).unwrap()
}

fn through_policy(
    config: &Config,
    admission: &mut admission::Admission,
    raw: &StepPlan,
) -> (StepPlan, crate::planner::sanitizer::SanitizerReport) {
    let mut plan = raw.clone();
    crate::planner::step_plan::repair_generated_step_plan_contract(&mut plan);
    admission.strengthen(config, &mut plan);
    crate::planner::python_cli_plan_synthesis::canonicalize_implementation_plan(
        &mut plan,
        &config.workspace_root,
        &config.profile,
        true,
        None,
    );
    crate::planner::step_plan::repair_generated_step_plan_contract(&mut plan);
    let runtime = crate::planner::profile::resolve_profile_runtime(&config.profile);
    runtime.canonicalize_create_plan(&mut plan, true, false, None);
    let report = admission.sanitize(&mut plan, Some(&config.workspace_root));
    runtime.convert_preset_phase_setup_steps(
        &mut plan,
        &config.workspace_root,
        &raw.goal,
        Some(("core-implementation", false)),
        true,
        None,
    );
    (plan, report)
}

fn target(plan: &StepPlan) -> &PlanStep {
    plan.steps
        .iter()
        .find(|step| step.id == "implement-layout-and-config")
        .unwrap()
}

fn bounded_plan(config: &Config, total_chars: usize) -> StepPlan {
    let goal = "Implement a Next.js app on port 60302";
    let guidance = crate::planner::profile::resolve_profile_runtime(&config.profile)
        .guidance(goal)
        .unwrap();
    let suffix = format!("\n\nProfile contract:\n{guidance}");
    assert!(suffix.chars().count() < total_chars);
    StepPlan {
        goal: goal.into(),
        steps: vec![PlanStep {
            id: "implement-layout-and-config".into(),
            kind: "implement".into(),
            expected_result: "pass".into(),
            instruction: "業".repeat(total_chars - suffix.chars().count()),
            expected_paths: vec!["package.json".into()],
            verify: Vec::new(),
        }],
    }
}

#[test]
fn issue496_saved_profile_guidance_is_retained_and_rejected_for_capacity() {
    let root = tempfile::tempdir().unwrap();
    let config = super::issue478::nextjs_config(root.path());
    let raw = saved();
    assert_eq!(target(&raw).instruction.chars().count(), 529);
    let guidance = crate::planner::profile::resolve_profile_runtime(NEXTJS_PROFILE_ID)
        .guidance(&raw.goal)
        .unwrap();
    assert_eq!(guidance.chars().count(), 2_237);
    assert_eq!(
        guidance,
        include_str!(
            "../../../tests/corpus/apps/issue496-profile-guidance-capacity/host-guidance.txt"
        )
    );
    assert_eq!(
        format!("{:x}", Sha256::digest(target(&raw).instruction.as_bytes())),
        "e32851275f8e288c11417d41915e6b7e6b4a42902114f136c7663160d73f33f2"
    );
    assert_eq!(
        format!("{:x}", Sha256::digest(guidance.as_bytes())),
        "b2dd0b0c696f9c4b38489821dd2917273a8e139d0b43c8cd6bc7d66e0d0975d2"
    );
    assert_eq!(
        format!(
            "{:x}",
            Sha256::digest(&guidance.as_bytes()[guidance.len() - 344..])
        ),
        "d6e7649620e77e3565e430f70cb1a1fd63232302dd7b0a58766b8498ca9f353a"
    );

    let mut admission = admission::Admission::default();
    let (mut plan, report) = through_policy(&config, &mut admission, &raw);
    let augmented = target(&plan);
    assert_eq!(augmented.instruction.chars().count(), 2_786);
    assert!(augmented.instruction.ends_with(&guidance));
    assert!(
        report
            .instruction_truncations
            .iter()
            .all(|record| record.step_id != augmented.id)
    );

    let admission::Decision::Retry(feedback) = admission
        .check(&config, Some("core-implementation"), &raw, &mut plan, 1)
        .unwrap()
    else {
        panic!("over-capacity profile instruction unexpectedly passed")
    };
    assert!(feedback.contains("2500-character capacity"), "{feedback}");
    let serialized_guidance = serde_json::to_string(&guidance).unwrap();
    assert!(
        feedback.contains(&serialized_guidance[1..serialized_guidance.len() - 1]),
        "host guidance was not retained"
    );

    let event = events(&config)
        .into_iter()
        .find(|event| event["event"] == "recovery_verifier_plan_admission")
        .unwrap();
    assert!(
        event["reason"]
            .as_str()
            .unwrap()
            .contains("profile instruction exceeds")
    );
    assert_eq!(
        event["original_obligation_sources"]["host"]["steps"][0]["instruction"],
        guidance
    );
    assert_eq!(
        event["original_scope"]["steps"]
            .as_array()
            .unwrap()
            .iter()
            .find(|step| step["id"] == "implement-layout-and-config")
            .unwrap()["instruction"]
            .as_str()
            .unwrap()
            .chars()
            .count(),
        2_786
    );
}

#[test]
fn issue496_capacity_uses_unicode_scalars_and_preserves_bounded_plans() {
    for total in [2_499, 2_500, 2_501] {
        let root = tempfile::tempdir().unwrap();
        let config = super::issue478::nextjs_config(root.path());
        let raw = bounded_plan(&config, total);
        let mut admission = admission::Admission::default();
        let (mut plan, report) = through_policy(&config, &mut admission, &raw);
        assert_eq!(plan.steps[0].instruction.chars().count(), total);
        assert!(report.instruction_truncations.is_empty());
        let decision = admission.check(&config, None, &raw, &mut plan, 1).unwrap();
        assert_eq!(
            matches!(decision, admission::Decision::Ready(false)),
            total <= 2_500,
            "total={total}"
        );
    }
}

#[test]
fn issue496_tracks_a_moved_owner_and_rejects_ambiguous_provenance() {
    let root = tempfile::tempdir().unwrap();
    let config = super::issue478::nextjs_config(root.path());
    let raw = bounded_plan(&config, 2_500);

    let mut moved = raw.clone();
    moved.steps.push(PlanStep {
        id: "later-report".into(),
        kind: "report".into(),
        expected_result: "pass".into(),
        instruction: "Report completion.".into(),
        expected_paths: Vec::new(),
        verify: Vec::new(),
    });
    let mut admission = admission::Admission::default();
    admission.strengthen(&config, &mut moved);
    moved.steps.swap(0, 1);
    let report = admission.sanitize(&mut moved, Some(root.path()));
    assert!(report.instruction_truncations.is_empty());
    assert!(matches!(
        admission.check(&config, None, &raw, &mut moved, 1).unwrap(),
        admission::Decision::Ready(false)
    ));

    let mut ambiguous = raw.clone();
    let mut admission = admission::Admission::default();
    admission.strengthen(&config, &mut ambiguous);
    ambiguous.steps.push(ambiguous.steps[0].clone());
    let report = admission.sanitize(&mut ambiguous, Some(root.path()));
    assert!(report.instruction_truncations.is_empty());
    let admission::Decision::Retry(feedback) = admission
        .check(&config, None, &raw, &mut ambiguous, 1)
        .unwrap()
    else {
        panic!("ambiguous profile provenance unexpectedly passed")
    };
    assert!(feedback.contains("ambiguous across 2 steps"), "{feedback}");
}

#[test]
fn issue496_saved_capacity_failure_exhausts_the_existing_planner_budget() {
    let root = tempfile::tempdir().unwrap();
    let mut config = super::issue478::nextjs_config(root.path());
    config.intent_override = Some(crate::planner::adjudication::contract::IntentId::Create);
    let raw = saved();
    let mut client = Replay::new(vec![proposal(&raw), proposal(&raw), proposal(&raw)]);

    let error = crate::planner::runner::generate_step_plan_with_ui_for_phase(
        &mut client,
        &raw.goal,
        &config,
        &crate::tui::NOOP_UI,
        Some("core-implementation"),
        false,
        true,
    )
    .unwrap_err();

    assert!(error.to_string().contains("exhausted"), "{error:#}");
    assert!(
        error.to_string().contains("profile instruction exceeds"),
        "{error:#}"
    );
    assert_eq!(client.requests.lock().unwrap().len(), 3);
    let admissions: Vec<_> = events(&config)
        .into_iter()
        .filter(|event| event["event"] == "recovery_verifier_plan_admission")
        .collect();
    assert_eq!(admissions.len(), 3);
    assert!(admissions.iter().all(|event| {
        event["reason"]
            .as_str()
            .unwrap()
            .contains("profile instruction exceeds")
    }));
    assert_eq!(admissions.last().unwrap()["status"], "exhausted");
}

#[path = "issue496_tests/capacity.rs"]
mod capacity;
#[path = "issue496_tests/retry.rs"]
mod retry;
