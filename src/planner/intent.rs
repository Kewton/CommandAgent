use crate::planner::ultra_plan::{UltraPhase, UltraPlan};

pub fn detect_intent(goal: &str) -> &'static str {
    let lower = goal.to_ascii_lowercase();
    if lower.contains("fix") || lower.contains("修正") {
        "fix"
    } else if lower.contains("research") || lower.contains("調査") {
        "investigate"
    } else {
        "create"
    }
}

pub(crate) fn explicit_investigation_plan(goal: &str, profile: &str, style: &str) -> UltraPlan {
    UltraPlan {
        goal: goal.to_string(),
        profile: profile.to_string(),
        style: style.to_string(),
        intent: "investigate".to_string(),
        phases: vec![
            UltraPhase {
                id: "reproduce-candidate".to_string(),
                prompt: format!("Construct and execute deterministic reproducer R for: {goal}"),
            },
            UltraPhase {
                id: "diagnose".to_string(),
                prompt: format!("Investigate existing evidence and write output/diagnosis.md for: {goal}"),
            },
            UltraPhase {
                id: "bind-verify".to_string(),
                prompt: "Bind every machine-checkable diagnosis claim to reproducer output and existing files.".to_string(),
            },
        ],
    }
}

pub(crate) fn explicit_fix_plan(goal: &str, profile: &str, style: &str) -> UltraPlan {
    UltraPlan {
        goal: goal.to_string(),
        profile: profile.to_string(),
        style: style.to_string(),
        intent: "fix".to_string(),
        phases: vec![
            UltraPhase {
                id: "reproduce-before".to_string(),
                prompt: format!(
                    "Bind and run one deterministic failing reproducer R before any workspace change for: {goal}"
                ),
            },
            UltraPhase {
                id: "isolate-cause".to_string(),
                prompt: format!(
                    "Isolate the cause of the reproduced failure without modifying the workspace for: {goal}"
                ),
            },
            UltraPhase {
                id: "repair".to_string(),
                prompt: format!("Apply the focused repair for the reproduced failure in: {goal}"),
            },
            UltraPhase {
                id: "verify-regressions".to_string(),
                prompt: format!(
                    "Verify the repaired behavior and the profile-bound regression set for: {goal}"
                ),
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use crate::planner::lint::lint_ultra_plan_report;

    #[test]
    fn create_fix_investigation_intent() {
        assert_eq!(super::detect_intent("fix parser"), "fix");
        assert_eq!(super::detect_intent("research topic"), "investigate");
        assert_eq!(super::detect_intent("make app"), "create");
    }

    #[test]
    fn explicit_fix_plan_has_contract_order_and_before_first() {
        let plan = super::explicit_fix_plan("fix parser", "generic", "default");

        assert_eq!(plan.intent, "fix");
        assert_eq!(
            plan.phases
                .iter()
                .map(|phase| phase.id.as_str())
                .collect::<Vec<_>>(),
            [
                "reproduce-before",
                "isolate-cause",
                "repair",
                "verify-regressions"
            ]
        );
        assert!(plan.phases[0].prompt.contains("reproducer R"));
        assert!(lint_ultra_plan_report(&plan).is_pass());
    }
}
