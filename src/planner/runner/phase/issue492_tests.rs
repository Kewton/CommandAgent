//! Full-plan controls reuse the witnessed #490 setup checkpoint and ordinary
//! phase entry. Only the trailing build duty is new; no registration bypass.
use super::*;
use crate::planner::profile_descriptor::NEXTJS_PROFILE_ID;
use crate::planner::setup_step_policy::issue492_tests as stages;

fn proposal(attempt: usize) -> StepPlan {
    serde_json::from_str(
        &std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
            "tests/corpus/apps/issue492-build-duty/P1-attempt-{attempt}.json"
        )))
        .unwrap(),
    )
    .unwrap()
}
fn build(plan: &StepPlan) -> &crate::planner::step_plan::PlanStep {
    plan.steps.iter().find(|s| s.id == "verify-build").unwrap()
}
fn evidence(name: &str, value: serde_json::Value) {
    if let Ok(dir) = std::env::var("ISSUE492_EVIDENCE_DIR") {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            Path::new(&dir).join(format!("{name}.json")),
            serde_json::to_vec_pretty(&value).unwrap(),
        )
        .unwrap();
    }
}
pub(super) fn observe_registration(c: &Config, plan: &StepPlan, after: &CompletionContract) {
    if !plan.steps.iter().any(|s| s.id == "verify-build") {
        return;
    }
    let before_events = events(c)
        .iter()
        .filter(|e| e["event"] == "recovery_generated_completion_contract_completed")
        .count();
    assert!(
        before_events > 0,
        "normal registration added verifier checks and paths"
    );
    let producers = scope::generated(c);
    recovery_authority::register_plan(c, plan).unwrap();
    assert_eq!(
        authority::load_for_handoff(c).unwrap().as_ref(),
        Some(after)
    );
    assert_eq!(scope::generated(c), producers);
    assert_eq!(
        events(c)
            .iter()
            .filter(|e| e["event"] == "recovery_generated_completion_contract_completed")
            .count(),
        before_events,
        "idempotent registration must take no-write branch"
    );
    let disk = crate::planner::completion_contract_path::generated_path(
        &c.workspace_root,
        c.eval_events_path.as_deref(),
        "completion-contract-ultra-plan-run.json",
    );
    assert_eq!(
        serde_json::from_slice::<CompletionContract>(&std::fs::read(disk).unwrap()).unwrap(),
        *after
    );
}

#[test]
fn issue492_p1_build_reader_save_reload_registration_and_noop() {
    let first = proposal(1);
    let second = proposal(2);
    stages::start();
    let o = replay(first.clone(), second.clone(), None);
    let stages = stages::take();
    assert_eq!(stages.len(), 8);
    let registered = o.registered.as_ref().unwrap();
    assert_eq!(registered.steps[0], first.steps[0]);
    let mut expected = build(&first).clone();
    let old: serde_json::Value = serde_json::from_str(&fixture("known-build-loss.json")).unwrap();
    expected.verify =
        serde_json::from_value(old["sanitized"]["steps"][0]["verify"].clone()).unwrap();
    assert_eq!(build(registered), &expected);
    assert!(build(registered).expected_paths.is_empty());
    assert!(
        !resolve_profile_runtime(NEXTJS_PROFILE_ID)
            .step_short_circuit_precheck_applicable(build(registered))
    );
    assert_eq!(o.returned, o.registered);
    for attempt in [1, 2] {
        let snapshots: Vec<_> = stages.iter().filter(|s| s["attempt"] == attempt).collect();
        assert_eq!(
            snapshots
                .iter()
                .map(|s| s["stage"].as_str().unwrap())
                .collect::<Vec<_>>(),
            ["parsed", "augmented", "sanitized", "converted"]
        );
        for snapshot in snapshots {
            let p: StepPlan = serde_json::from_value(snapshot["plan"].clone()).unwrap();
            let mut expected_steps = proposal(attempt as usize).steps;
            if snapshot["stage"] != "parsed" {
                expected_steps
                    .iter_mut()
                    .find(|s| s.id == "application-config")
                    .unwrap()
                    .instruction = proposal(2)
                    .steps
                    .into_iter()
                    .find(|s| s.id == "application-config")
                    .unwrap()
                    .instruction;
            }
            if snapshot["stage"] == "augmented" || snapshot["stage"] == "sanitized" {
                for step in &mut expected_steps {
                    if step.id.starts_with("package-read-") {
                        step.expected_paths.clear();
                    }
                }
            }
            assert_eq!(
                p.steps, expected_steps,
                "all instruction/check/path/order fields at {}",
                snapshot["stage"]
            );
            let target = build(&p);
            let original = build(&first);
            assert_eq!(target.instruction, original.instruction);
            assert_eq!(target.expected_result, original.expected_result);
            assert_eq!(target.expected_paths, original.expected_paths);
            assert_eq!(target.verify[0], "npm run build");
            assert_eq!(p.steps.last(), Some(target));
            if snapshot["stage"] == "sanitized" || snapshot["stage"] == "converted" {
                assert_eq!(target, &expected);
            }
        }
    }
    // Existing registration assertions pin added paths/commands and the sole
    // verifier producer. Build was already in the run contract before core;
    // only the returned/saved/registered trailing duty demonstrates retention.
    assert!(
        o.before
            .as_ref()
            .unwrap()
            .verify_commands
            .contains(&"npm run build".into())
    );
    assert!(
        o.after
            .as_ref()
            .unwrap()
            .verify_commands
            .contains(&"npm run build".into())
    );
    evidence(
        "P1",
        json!({"stages":stages,"returned":o.returned,"registered":o.registered,"before":o.before,"after":o.after,"producers":o.producers,"events":o.events,"registration_adds":true,"repeat_registration":"no_write"}),
    );
}

#[test]
fn issue492_n1_through_n4_acquire_then_reject_targeted_loss() {
    for case in ["N1", "N2", "N3", "N4"] {
        let first = proposal(1);
        let mut second = proposal(2);
        let target = second.steps.last_mut().unwrap();
        // Acquire another explicit check before varying the later proposal.
        // Keep the profile guard explicitly declared and use an independent
        // original artifact check, avoiding setup canonicalization in N4.
        let mut first = first;
        first
            .steps
            .last_mut()
            .unwrap()
            .verify
            .push("test -f src/app/page.tsx".into());
        target.verify.push("test -f src/app/page.tsx".into());
        let reason = match case {
            "N1" => {
                target.verify.remove(0);
                "formation lost original check/expected result"
            }
            "N2" => {
                let moved = second.steps.pop().unwrap();
                second.steps.insert(1, moved);
                "formation moved original check before its output owners: npm run build"
            }
            "N3" => {
                target.expected_result = "fail".into();
                "formation dropped original instruction/expected result for verify-build"
            }
            "N4" => {
                target.verify.pop();
                "formation lost original check/expected result"
            }
            _ => unreachable!(),
        };
        stages::start();
        let o = replay(first.clone(), second, Some(reason));
        let stages = stages::take();
        let admissions: Vec<_> = o
            .events
            .iter()
            .filter(|e| e["event"] == "recovery_verifier_plan_admission")
            .collect();
        assert_eq!(admissions.len(), 3);
        let acquired = &admissions[0]["original_scope"];
        let original: StepPlan = serde_json::from_value(acquired.clone()).unwrap();
        assert_eq!(build(&original).instruction, build(&first).instruction);
        assert!(build(&original).verify.contains(&"npm run build".into()));
        assert!(
            build(&original)
                .verify
                .contains(&"test -f src/app/page.tsx".into())
        );
        for event in &admissions[1..] {
            assert_eq!(event["original_scope"], *acquired);
            assert_eq!(
                event["original_obligation_sources"],
                admissions[0]["original_obligation_sources"]
            );
            assert!(event["reason"].as_str().unwrap().contains(reason));
        }
        evidence(
            case,
            json!({"stages":stages,"events":o.events,"registered":o.registered,"before":o.before,"targeted_refusal":reason}),
        );
    }
}

#[path = "issue492_runtime_tests.rs"]
mod runtime;
