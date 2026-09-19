use super::*;
use clap::Parser;

fn setup(model: &str, host: &str) -> (tempfile::TempDir, Config, Admission, StepPlan) {
    setup_paths(model, host, &["a.txt", "b.txt"])
}

fn setup_paths(
    model: &str,
    host: &str,
    paths: &[&str],
) -> (tempfile::TempDir, Config, Admission, StepPlan) {
    let root = tempfile::tempdir().unwrap();
    let mut c =
        Config::from_cli(crate::cli::Cli::parse_from(["commandagent", "--ux-demo"])).unwrap();
    c.workspace_root = root.path().to_path_buf();
    c.profile = "generic".into();
    c.eval_events_path = Some(root.path().join("events.jsonl"));
    let raw = PlanStep {
        id: "owner".into(),
        kind: "implement".into(),
        expected_result: "pass".into(),
        instruction: model.into(),
        expected_paths: paths.iter().map(|p| (*p).into()).collect(),
        verify: vec![],
    };
    let mut plan = StepPlan {
        goal: "Keep both duties".into(),
        steps: vec![raw.clone()],
    };
    plan.steps[0].instruction = format!("{model}\n\nProfile contract:\n{host}");
    let addition = ProfileAddition::new(raw, &plan.steps[0], host.into());
    let sources = FormationScope::capture(&plan, Some(&addition));
    let mut admission = Admission {
        profile_addition: Some(addition),
        pending_sources: Some(
            super::super::super::package_script_formation::CapturedSources::new(
                &c,
                sources,
                "profile_augmentation_before_sanitization",
            ),
        ),
        ..Default::default()
    };
    admission.sanitize(&mut plan, Some(root.path()));
    admission.retain_current_contract(&c, &plan).unwrap();
    (root, c, admission, plan)
}

fn capacity(admission: &Admission, plan: &StepPlan) -> anyhow::Error {
    let reason = admission.profile_instruction_failure(plan).unwrap();
    failure(FailureClass::ProposalRepairable, reason.to_string()).context(reason)
}

#[cfg(test)]
#[test]
fn issue498_shortest_superstring_counts_containment_both_overlaps_and_scalars() {
    for (a, b, forward, reverse, minimum) in [
        ("abc", "b", 0, 0, 3),
        ("b", "abc", 0, 0, 3),
        ("abc", "abc", 3, 3, 3),
        ("abc", "cde", 1, 0, 5),
        ("cde", "abc", 0, 1, 5),
        ("ababa", "babab", 4, 4, 6),
        ("あ🙂い", "い漢あ", 1, 1, 5),
        ("", "務", 0, 0, 1),
    ] {
        let actual = common_superstring_lengths(a, b);
        assert_eq!(
            (actual.forward, actual.reverse, actual.minimum),
            (forward, reverse, minimum),
            "{a} / {b}"
        );
    }
    for total in [2499, 2500, 2501] {
        let model = "務".repeat(total - 1200);
        let host = "界".repeat(1200);
        assert!(model.len() + host.len() > 2500);
        let (_root, _c, a, p) = setup(&model, &host);
        assert_eq!(common_superstring_lengths(&model, &host).minimum, total);
        assert_eq!(
            a.prove_bounded_repair_infeasible(&capacity(&a, &p), &p, 1)
                .is_some(),
            total > 2500
        );
    }
}

#[cfg(test)]
#[test]
fn issue498_only_capacity_with_complete_first_capture_can_prove_infeasibility() {
    for case in [
        "capacity",
        "provenance",
        "missing_sources",
        "missing_preservation",
        "missing_package",
        "marker_error",
        "stage",
        "duplicate_id",
        "model_text",
        "host_text",
        "paths",
        "result",
        "checks",
        "order",
        "later_attempt",
    ] {
        let (_root, c, mut a, mut p) = setup(&"M".repeat(1300), &"H".repeat(1300));
        let mut attempt = 1;
        match case {
            "capacity" => {}
            "provenance" => {
                a.profile_instruction_provenance_error =
                    Some("profile instruction provenance lost host guidance".into())
            }
            "missing_sources" => a.sources = None,
            "missing_preservation" => a.preservation_sources = None,
            "missing_package" => a.package_scripts = None,
            "marker_error" => a.marker_capture_error = Some("source acquisition failed".into()),
            "stage" => {
                let captured = super::super::super::package_script_formation::CapturedSources::new(
                    &c,
                    a.sources.clone().unwrap(),
                    "admission_before_first_retry",
                );
                a.package_scripts = Some(
                    super::super::super::package_script_formation::PackageScripts::capture(
                        &c,
                        &p,
                        captured,
                        &serde_json::from_value(json!({})).unwrap(),
                    ),
                );
            }
            "duplicate_id" => {
                p.steps.push(p.steps[0].clone());
                a.original = Some(p.clone());
            }
            "model_text" => a.preservation_sources.as_mut().unwrap().model.steps[0]
                .instruction
                .pop()
                .map(|_| ())
                .unwrap(),
            "host_text" => a.preservation_sources.as_mut().unwrap().host.steps[0]
                .instruction
                .clear(),
            "paths" => a.preservation_sources.as_mut().unwrap().model.steps[0]
                .expected_paths
                .pop()
                .map(|_| ())
                .unwrap(),
            "result" => {
                a.preservation_sources.as_mut().unwrap().model.steps[0].expected_result =
                    "fail".into()
            }
            "checks" => a.preservation_sources.as_mut().unwrap().model.steps[0]
                .verify
                .push("test -f a.txt".into()),
            "order" => a
                .preservation_sources
                .as_mut()
                .unwrap()
                .model
                .steps
                .insert(0, p.steps[0].clone()),
            "later_attempt" => attempt = 2,
            _ => unreachable!(),
        }
        let e = capacity(&a, &p);
        assert_eq!(
            a.prove_bounded_repair_infeasible(&e, &p, attempt).is_some(),
            case == "capacity",
            "{case}"
        );
        if case == "provenance" {
            assert!(matches!(
                a.profile_instruction_failure(&p),
                Some(InstructionFailure::Provenance(_))
            ));
        }
    }
}

#[cfg(test)]
#[test]
fn issue498_unknown_capacity_keeps_all_three_attempts() {
    let (_root, c, mut a, mut p) = setup_paths(&"M".repeat(1300), &"H".repeat(1300), &["a.txt"]);
    // Single-path shapes are deliberately outside this closed proof.
    let raw = p.clone();
    for attempt in 1..=3 {
        let result = a.check(&c, None, &raw, &mut p, attempt);
        if attempt < 3 {
            assert!(matches!(result.unwrap(), Decision::Retry(_)));
        } else {
            assert!(result.err().unwrap().to_string().contains("exhausted"));
        }
    }
    let log = std::fs::read_to_string(c.eval_events_path.unwrap()).unwrap();
    let events: Vec<serde_json::Value> = log
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(events.len(), 3);
    assert_eq!(events[2]["status"], "exhausted");
    assert!(events.iter().all(|e| e["terminal_reason"].is_null()));
}

#[cfg(test)]
#[test]
fn issue498_normalized_scope_order_and_aliases_keep_the_same_lower_bound() {
    for paths in [vec!["b.txt", "a.txt"], vec!["./b.txt", "./a.txt"]] {
        let (_root, c, a, mut p) = setup(&"M".repeat(1300), &"H".repeat(1300));
        let model = a.sources.unwrap().model.steps[0].clone();
        p.steps[0].expected_paths = paths.into_iter().map(String::from).collect();
        let addition = ProfileAddition::new(model, &p.steps[0], "H".repeat(1300));
        let sources = FormationScope::capture(&p, Some(&addition));
        let mut a = Admission {
            profile_addition: Some(addition),
            pending_sources: Some(
                super::super::super::package_script_formation::CapturedSources::new(
                    &c,
                    sources,
                    "profile_augmentation_before_sanitization",
                ),
            ),
            ..Default::default()
        };
        a.retain_current_contract(&c, &p).unwrap();
        assert!(
            a.prove_bounded_repair_infeasible(&capacity(&a, &p), &p, 1)
                .is_some()
        );
    }
}
