//! The short step text that points at the profile contract delivered in full
//! under `Profile guidance`, so the step does not copy it past the text limit.
use crate::planner::profile_behavior::ProfileRuntime;

pub(crate) const PROFILE_CONTRACT_REFERENCE: &str = "This step owns the active profile contract, which is given in full under Profile guidance in the system prompt; satisfy every requirement in it, including the dev/start script port.";

pub(crate) fn is_reference(text: &str) -> bool {
    text == PROFILE_CONTRACT_REFERENCE
}

/// True when an instruction carries the reference, wherever it sits.
pub(crate) fn contains_reference(instruction: &str) -> bool {
    instruction.contains(PROFILE_CONTRACT_REFERENCE)
}

/// True when `text` is the reference or the full guidance for `goal`, the two
/// forms a host-added profile duty can take in a step instruction.
pub(crate) fn is_host_contract_text(runtime: &dyn ProfileRuntime, goal: &str, text: &str) -> bool {
    is_reference(text) || runtime.guidance(goal).as_deref() == Some(text)
}

#[cfg(test)]
mod tests {
    use super::{PROFILE_CONTRACT_REFERENCE, is_host_contract_text, is_reference};
    use crate::config::Config;
    use crate::planner::profile::resolve_profile_runtime;
    use crate::planner::profile_descriptor::NEXTJS_PROFILE_ID;
    use crate::planner::recovery_step_plan_binding::admission::{Admission, Decision};
    use crate::planner::step_plan::{PlanStep, StepPlan};
    use clap::Parser;

    const CASES: &str = include_str!(
        "../../../tests/corpus/apps/issue810-nextjs-profile-contract-capacity/fixtures/cases.json"
    );
    const MARKER: &str = "\n\nProfile contract:\n";

    fn nextjs_config(root: &std::path::Path) -> Config {
        let mut c =
            Config::from_cli(crate::cli::Cli::parse_from(["commandagent", "--ux-demo"])).unwrap();
        c.workspace_root = root.to_path_buf();
        c.profile = NEXTJS_PROFILE_ID.into();
        c.eval_events_path = Some(root.join("events.jsonl"));
        c
    }

    fn plan(goal: &str, instruction: String, paths: &[&str]) -> StepPlan {
        StepPlan {
            goal: goal.into(),
            steps: vec![PlanStep {
                id: "implement-use-todos".into(),
                kind: "implement".into(),
                expected_result: "pass".into(),
                instruction,
                expected_paths: paths.iter().map(|p| p.to_string()).collect(),
                verify: Vec::new(),
            }],
        }
    }

    #[test]
    fn reference_is_short_and_recognized_only_exactly() {
        assert!(PROFILE_CONTRACT_REFERENCE.chars().count() <= 250);
        assert!(is_reference(PROFILE_CONTRACT_REFERENCE));
        assert!(!is_reference(&format!("{PROFILE_CONTRACT_REFERENCE} ")));
        assert!(!is_reference("Keep scripts on port 3011."));
    }

    #[test]
    fn host_contract_text_accepts_reference_and_full_guidance_only() {
        let runtime = resolve_profile_runtime(NEXTJS_PROFILE_ID);
        let goal = "Build a todo app on port 3011";
        let full = runtime.guidance(goal).unwrap();
        assert!(is_host_contract_text(
            runtime,
            goal,
            PROFILE_CONTRACT_REFERENCE
        ));
        assert!(is_host_contract_text(runtime, goal, &full));
        assert!(!is_host_contract_text(runtime, goal, "something else"));
    }

    /// #810 acceptance: the TOOL-G shapes (378 and 683 character model steps on
    /// port 3011) are Ready with the marker and the reference within 2,500
    /// characters. Adding the full contract instead (develop) overflows.
    #[test]
    fn issue810_tool_g_shapes_are_ready_with_marker_and_reference() {
        let cases: serde_json::Value = serde_json::from_str(CASES).unwrap();
        let nextjs = &cases["web_app"];
        let goal = nextjs["goal"].as_str().unwrap();
        let paths: Vec<&str> = nextjs["expected_paths"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p.as_str().unwrap())
            .collect();
        let full = resolve_profile_runtime(NEXTJS_PROFILE_ID)
            .guidance(goal)
            .unwrap();
        assert!(full.contains("port 3011") || full.contains("3011"));
        for shape in nextjs["shapes"].as_array().unwrap() {
            let name = shape["name"].as_str().unwrap();
            let model = shape["instruction"].as_str().unwrap().to_string();
            let model_chars = model.chars().count();
            assert!(model_chars + MARKER.chars().count() + full.chars().count() > 2_500);
            let root = tempfile::tempdir().unwrap();
            let c = nextjs_config(root.path());
            let raw = plan(goal, model.clone(), &paths);
            let mut candidate = raw.clone();
            let mut admission = Admission::default();
            admission.strengthen(&c, &mut candidate);
            let report = admission.sanitize(&mut candidate, Some(root.path()));
            assert!(report.instruction_truncations.is_empty(), "{name}");
            let instruction = &candidate.steps[0].instruction;
            assert!(instruction.starts_with(&model), "{name}");
            assert!(instruction.contains(MARKER), "{name}");
            assert!(instruction.ends_with(PROFILE_CONTRACT_REFERENCE), "{name}");
            assert!(!instruction.contains(&full), "{name}");
            assert!(instruction.chars().count() <= 2_500, "{name}");
            let decision = admission
                .check(&c, None, &raw, &mut candidate, 1)
                .unwrap_or_else(|e| panic!("{name}: {e:#}"));
            assert!(matches!(decision, Decision::Ready(false)), "{name}");
            assert!(
                crate::planner::lint::lint_template_contract(&candidate, Some(root.path()))
                    .is_pass(),
                "{name}"
            );
        }
    }

    /// A step whose own model text exceeds 2,500 characters is still refused.
    #[test]
    fn issue810_model_text_over_capacity_is_still_refused() {
        let cases: serde_json::Value = serde_json::from_str(CASES).unwrap();
        let nextjs = &cases["web_app"];
        let goal = nextjs["goal"].as_str().unwrap();
        let chars = nextjs["model_only_over_capacity_chars"].as_u64().unwrap() as usize;
        let root = tempfile::tempdir().unwrap();
        let c = nextjs_config(root.path());
        let raw = plan(goal, "業".repeat(chars), &["src/hooks/useTodos.ts"]);
        assert!(!crate::planner::lint::lint_template_contract(&raw, Some(root.path())).is_pass());
        let mut candidate = raw.clone();
        let mut admission = Admission::default();
        admission.strengthen(&c, &mut candidate);
        admission.sanitize(&mut candidate, Some(root.path()));
        let Decision::Retry(feedback) = admission.check(&c, None, &raw, &mut candidate, 1).unwrap()
        else {
            panic!("an over-capacity model step must be refused")
        };
        assert!(feedback.contains("2500-character capacity"), "{feedback}");
    }
}
