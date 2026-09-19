use super::*;
use crate::planner::lint::lint_template_contract;

#[cfg(test)]
#[test]
fn issue496_boundaries_include_readiness_notes_and_delimiters() {
    for total in [2_499, 2_500, 2_501] {
        let root = tempfile::tempdir().unwrap();
        let c = super::super::issue478::nextjs_config(root.path());
        // Measure the actual deterministic note, including its punctuation.
        let mut probe = bounded_plan(&c, 2_400);
        probe.steps[0].expected_paths = vec!["src/app/page.tsx".into()];
        probe.steps[0].verify = vec!["npm run dev".into()];
        let mut admission = admission::Admission::default();
        let (probed, report) = through_policy(&c, &mut admission, &probe);
        assert_eq!(report.instruction_notes.len(), 1);
        let note_chars = probed.steps[0].instruction.chars().count() - 2_400;
        let mut raw = bounded_plan(&c, total - note_chars);
        raw.steps[0].expected_paths = probe.steps[0].expected_paths.clone();
        raw.steps[0].verify = probe.steps[0].verify.clone();
        let mut admission = admission::Admission::default();
        let (mut candidate, report) = through_policy(&c, &mut admission, &raw);
        assert_eq!(candidate.steps[0].instruction.chars().count(), total);
        assert!(
            candidate.steps[0]
                .instruction
                .contains(&report.instruction_notes[0].note)
        );
        assert!(
            candidate.steps[0]
                .instruction
                .contains("\n\nProfile contract:\n")
        );
        assert!(report.instruction_truncations.is_empty());
        let decision = admission.check(&c, None, &raw, &mut candidate, 1).unwrap();
        if total <= 2_500 {
            assert!(matches!(decision, admission::Decision::Ready(false)));
            let lint = lint_template_contract(&candidate, Some(root.path()));
            assert!(lint.is_pass(), "{}", lint.primary_message());
        } else {
            assert!(matches!(decision, admission::Decision::Retry(_)));
            assert!(admission.finish(&c, None, candidate).is_err());
        }
    }
}

#[cfg(test)]
#[test]
fn issue496_provenance_covers_existing_host_prefix_and_model_only() {
    for variant in ["contained", "prefix", "append", "model_only"] {
        let root = tempfile::tempdir().unwrap();
        let mut c = super::super::issue478::nextjs_config(root.path());
        let mut raw = bounded_plan(&c, 2_501);
        let guidance = crate::planner::profile::resolve_profile_runtime(&c.profile)
            .guidance(&raw.goal)
            .unwrap();
        match variant {
            "contained" => raw.steps[0].instruction = format!("{}{guidance}", "業".repeat(400)),
            "prefix" => raw.steps[0].instruction = guidance.chars().take(40).collect(),
            "model_only" => {
                c.profile = "generic".into();
                raw.steps[0].instruction = format!("Profile contract:\n{}", "業".repeat(3_000));
            }
            _ => {}
        }
        let mut admission = admission::Admission::default();
        let (mut candidate, report) = through_policy(&c, &mut admission, &raw);
        let decision = admission.check(&c, None, &raw, &mut candidate, 1).unwrap();
        match variant {
            "prefix" => {
                assert_eq!(candidate.steps[0].instruction, guidance);
                assert!(matches!(decision, admission::Decision::Ready(false)));
            }
            "model_only" => {
                assert_eq!(report.instruction_truncations.len(), 1);
                assert!(candidate.steps[0].instruction.chars().count() <= 2_500);
                assert!(matches!(decision, admission::Decision::Ready(false)));
            }
            _ => {
                assert!(candidate.steps[0].instruction.contains(&guidance));
                assert!(report.instruction_truncations.is_empty());
                assert!(matches!(decision, admission::Decision::Retry(_)));
            }
        }
    }
}

#[cfg(test)]
#[test]
fn issue496_real_python_canonicalization_moves_or_removes_target() {
    for removed in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let mut c = config(root.path());
        c.profile = "python-cli".into();
        let mut raw = plan(vec![
            step("read", "inspect", &[]),
            step("write", "implement", &["src/app/main.py", "README.md"]),
        ]);
        raw.goal = "Phase id: cli-implementation\nPhase task: Implement a command line app".into();
        raw.steps[1].instruction = "Implement the requested command line behavior.".into();
        if removed {
            raw.steps
                .push(step("removed-setup", "setup", &["pyproject.toml"]));
        }
        let mut admission = admission::Admission::default();
        let (mut candidate, _) = through_policy(&c, &mut admission, &raw);
        assert_eq!(candidate.steps[0].id, "write");
        let decision = admission.check(&c, None, &raw, &mut candidate, 1).unwrap();
        if removed {
            let admission::Decision::Retry(feedback) = decision else {
                panic!("lost target accepted")
            };
            assert!(feedback.contains("provenance lost"), "{feedback}");
            assert!(feedback.contains("removed-setup"));
            assert!(admission.finish(&c, None, candidate).is_err());
        } else {
            assert!(matches!(decision, admission::Decision::Ready(false)));
            assert!(lint_template_contract(&candidate, Some(root.path())).is_pass());
        }
    }
}

#[cfg(test)]
#[test]
fn issue496_does_not_protect_an_unrelated_or_duplicate_owner() {
    for variant in [
        "deleted",
        "duplicate",
        "same_id_different_text",
        "lost_model",
    ] {
        let root = tempfile::tempdir().unwrap();
        let c = super::super::issue478::nextjs_config(root.path());
        let raw = bounded_plan(&c, 2_501);
        let mut candidate = raw.clone();
        let mut admission = admission::Admission::default();
        admission.strengthen(&c, &mut candidate);
        match variant {
            "deleted" => candidate.steps[0].id = "unrelated-owner".into(),
            "duplicate" | "same_id_different_text" => {
                let mut other = candidate.steps[0].clone();
                if variant == "same_id_different_text" {
                    other.instruction = "業".repeat(3_000);
                }
                candidate.steps.push(other);
            }
            "lost_model" => {
                candidate.steps[0].instruction = candidate.steps[0]
                    .instruction
                    .replace(&raw.steps[0].instruction, "Changed model duty")
            }
            _ => unreachable!(),
        }
        let report = admission.sanitize(&mut candidate, Some(root.path()));
        if variant != "lost_model" {
            assert_eq!(report.instruction_truncations.len(), candidate.steps.len());
        }
        let admission::Decision::Retry(feedback) =
            admission.check(&c, None, &raw, &mut candidate, 1).unwrap()
        else {
            panic!("{variant} accepted")
        };
        assert!(
            feedback.contains("profile instruction provenance"),
            "{variant}: {feedback}"
        );
    }
}

#[cfg(test)]
#[test]
fn issue496_capacity_cannot_partially_accept_scaffold_mutations() {
    let root = tempfile::tempdir().unwrap();
    let c = super::super::issue478::nextjs_config(root.path());
    let raw = StepPlan {
        goal: "Complete scaffold metadata".into(),
        steps: vec![PlanStep {
            instruction: "業".repeat(600),
            ..step("scaffold-report", "report", &[])
        }],
    };
    let mut admission = admission::Admission::default();
    let (mut candidate, _) = through_policy(&c, &mut admission, &raw);
    assert_eq!(candidate.steps[0].kind, "implement");
    assert!(
        candidate.steps[0]
            .expected_paths
            .contains(&"package.json".into())
    );
    assert!(matches!(
        admission.check(&c, None, &raw, &mut candidate, 1).unwrap(),
        admission::Decision::Retry(_)
    ));
    assert_eq!(raw.steps[0].kind, "report");
    assert!(raw.steps[0].expected_paths.is_empty());
    assert!(admission.finish(&c, None, candidate.clone()).is_err());
    candidate.steps[0].instruction = raw.steps[0].instruction.clone();
    assert!(admission.finish(&c, None, candidate).is_err());
}

#[cfg(test)]
#[test]
fn issue496_ambiguous_ids_keep_the_exact_preaugmentation_model_snapshot() {
    let root = tempfile::tempdir().unwrap();
    let c = super::super::issue478::nextjs_config(root.path());
    let mut raw = bounded_plan(&c, 2_500);
    let mut other = raw.steps[0].clone();
    other.instruction = "Keep this distinct second model duty.".into();
    other.expected_paths = vec!["src/app/page.tsx".into()];
    raw.steps.push(other);
    let mut candidate = raw.clone();
    let mut admission = admission::Admission::default();
    admission.strengthen(&c, &mut candidate);
    admission.sanitize(&mut candidate, Some(root.path()));
    assert!(matches!(
        admission.check(&c, None, &raw, &mut candidate, 1).unwrap(),
        admission::Decision::Retry(_)
    ));
    let event = events(&c)
        .into_iter()
        .find(|e| e["event"] == "recovery_verifier_plan_admission")
        .unwrap();
    assert_eq!(event["original_obligation_sources"]["model"], json!(raw));
    assert_eq!(
        event["package_script_formation"]["sources"]["plans"]["model"],
        json!(raw)
    );
}
