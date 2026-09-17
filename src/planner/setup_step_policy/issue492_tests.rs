//! Test-only observation of the ordinary driver; no plan mutation or decisions.
use super::*;
use crate::planner::profile_descriptor::NEXTJS_PROFILE_ID;
use std::cell::RefCell;

thread_local! {
    static STAGES: RefCell<Option<Vec<serde_json::Value>>> = const { RefCell::new(None) };
}
pub(crate) fn record(stage: &str, plan: &StepPlan) {
    STAGES.with(|s| {
        if let Some(s) = s.borrow_mut().as_mut() {
            let attempt = s.iter().filter(|entry| entry["stage"] == "parsed").count()
                + usize::from(stage == "parsed");
            s.push(serde_json::json!({"stage":stage,"attempt":attempt,"plan":plan}));
        }
    });
}
pub(crate) fn start() {
    STAGES.with(|s| *s.borrow_mut() = Some(Vec::new()));
}
pub(crate) fn take() -> Vec<serde_json::Value> {
    STAGES.with(|s| s.borrow_mut().take().unwrap())
}

#[test]
fn issue492_nonempty_duties_are_preserved_without_command_authorization() {
    let root = tempfile::tempdir().unwrap();
    for kind in ["setup", "implement", "verify"] {
        for command in ["npm run build", "node opaque-check.cjs", "npm install"] {
            let mut step = setup_scripts_step();
            step.kind = kind.into();
            step.expected_result = "pass".into();
            step.verify = vec![command.into(), "test -f package.json".into()];
            let mut plan = StepPlan {
                goal: "Next.js on port 3011".into(),
                steps: vec![step.clone()],
            };
            assert_eq!(
                convert_preset_phase_setup_steps(
                    &mut plan,
                    root.path(),
                    NEXTJS_PROFILE_ID,
                    "Next.js on port 3011",
                    Some(("core-implementation", false)),
                    true,
                    None
                ),
                0
            );
            assert_eq!(plan.steps, [step]);
        }
    }
    assert!(crate::planner::verify::normalize_planner_verify_command("npm install").is_err());
}

#[test]
fn issue492_preset_and_phase_boundaries_keep_original_setup() {
    let root = tempfile::tempdir().unwrap();
    for (preset, phase) in [(false, "core-implementation"), (true, "final-verification")] {
        let mut plan = StepPlan {
            goal: "Next.js".into(),
            steps: vec![setup_scripts_step()],
        };
        let original = plan.clone();
        assert_eq!(
            convert_preset_phase_setup_steps(
                &mut plan,
                root.path(),
                NEXTJS_PROFILE_ID,
                "Next.js",
                Some((phase, false)),
                preset,
                None
            ),
            0
        );
        assert_eq!(plan, original);
    }
}

#[test]
fn issue492_only_exact_passing_profile_checks_allow_setup_conversion() {
    let root = tempfile::tempdir().unwrap();
    for (port, result, converted) in [
        (3011, "pass", true),
        (3011, "fail", false),
        (60302, "pass", false),
    ] {
        let mut step = setup_scripts_step();
        step.expected_result = result.into();
        step.verify = profile_setup_checks(
            root.path(),
            NEXTJS_PROFILE_ID,
            &format!("Next.js on port {port}"),
            &step,
            Some("core-implementation"),
        )
        .unwrap()
        .verify_commands;
        let mut plan = StepPlan {
            goal: "Next.js on port 3011".into(),
            steps: vec![step.clone()],
        };
        assert_eq!(
            convert_preset_phase_setup_steps(
                &mut plan,
                root.path(),
                NEXTJS_PROFILE_ID,
                "Next.js on port 3011",
                Some(("core-implementation", false)),
                true,
                None
            ),
            usize::from(converted)
        );
        if converted {
            assert_eq!(plan.steps[0].kind, "verify");
            assert_eq!(plan.steps[0].expected_paths, ["package.json"]);
            assert!(step.verify.iter().all(|c| plan.steps[0].verify.contains(c)));
        } else {
            assert_eq!(plan.steps, [step]);
        }
    }
}

#[test]
fn issue492_existing_profile_augmentation_keeps_adding_required_checks() {
    let root = tempfile::tempdir().unwrap();
    let mut step = setup_scripts_step();
    step.instruction = "Update package.json scripts so that the app uses port 3011.".into();
    step.verify = vec!["test -f package.json".into()];
    let mut plan = StepPlan {
        goal: "Next.js on port 3011".into(),
        steps: vec![step.clone()],
    };
    convert_preset_phase_setup_steps(
        &mut plan,
        root.path(),
        NEXTJS_PROFILE_ID,
        "Next.js on port 3011",
        Some(("core-implementation", false)),
        true,
        None,
    );
    assert_eq!(plan.steps[0].instruction, step.instruction);
    assert_eq!(plan.steps[0].kind, step.kind);
    assert_eq!(plan.steps[0].verify[0], step.verify[0]);
    let checks = profile_setup_checks(
        root.path(),
        NEXTJS_PROFILE_ID,
        "Next.js on port 3011",
        &step,
        Some("core-implementation"),
    )
    .unwrap();
    assert!(
        checks
            .verify_commands
            .iter()
            .all(|c| plan.steps[0].verify.contains(c))
    );
    assert_eq!(plan.steps[0].expected_paths, checks.expected_paths);
}
