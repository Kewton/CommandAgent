#[cfg(test)]
mod cases {
    use super::super::*;
    use crate::planner::ultra_plan::UltraPhase;

    fn fix_plan(goal: &str) -> UltraPlan {
        UltraPlan {
            goal: goal.to_string(),
            profile: "nextjs".to_string(),
            style: "default".to_string(),
            intent: "fix".to_string(),
            phases: vec![UltraPhase {
                id: "reproduce-before".to_string(),
                prompt: "Reproduce the failure before repair.".to_string(),
            }],
        }
    }

    #[test]
    fn hook_goal_suggests_route_bound_catalog_check() {
        let suggestion = suggestion_for(&fix_plan(
            "このNext.jsアプリはリスタート操作の契約フック（data-anvil-action=\"restart\"）が欠落しており検証に失敗します。",
        ))
        .expect("hook suggestion");

        let command = crate::planner::verify::hook_attribute_present_check_command(
            "action",
            "restart",
            "src/app/page.tsx",
        )
        .unwrap();
        assert_eq!(
            suggestion,
            ReproducerSuggestion {
                basis: "goal_contract_attribute:data-anvil-action=restart".to_string(),
                suggestion: format!(
                    "profile_catalog:hook_attribute_present(attribute=action,value=restart,path=src/app/page.tsx) => {command}"
                ),
            }
        );
    }

    #[test]
    fn build_goal_suggests_catalog_build_oracle() {
        let suggestion = suggestion_for(&fix_plan(
            "このNext.jsプロジェクトは npm run build が失敗します。コンパイル原因を修正してください。",
        ))
        .expect("build suggestion");

        assert_eq!(
            suggestion,
            ReproducerSuggestion {
                basis: "goal_failure_kind:build_or_compile".to_string(),
                suggestion: "profile_catalog:next_build_verify => npm run build".to_string(),
            }
        );
    }

    #[test]
    fn nextjs_fix_plan_shape_remains_byte_stable() {
        let plan = crate::planner::intent::explicit_fix_plan("fix the build", "nextjs", "default");

        assert_eq!(
            crate::planner::ultra_plan::render_ultra_plan(&plan),
            concat!(
                "goal: \"fix the build\"\n",
                "profile: \"nextjs\"\n",
                "style: \"default\"\n",
                "intent: \"fix\"\n",
                "phases:\n",
                "  - id: \"reproduce-before\"\n",
                "    prompt: \"Bind and run one deterministic failing reproducer R before any workspace change for: fix the build\"\n",
                "  - id: \"isolate-cause\"\n",
                "    prompt: \"Isolate the cause of the reproduced failure without modifying the workspace for: fix the build\"\n",
                "  - id: \"repair\"\n",
                "    prompt: \"Apply the focused repair for the reproduced failure in: fix the build\"\n",
                "  - id: \"verify-regressions\"\n",
                "    prompt: \"Verify the repaired behavior and the profile-bound regression set for: fix the build\"\n",
            )
        );
    }

    #[test]
    fn goal_without_contract_or_failure_kind_keeps_legacy_behavior() {
        assert!(suggestion_for(&fix_plan("既存の不具合を修正してください。")).is_none());
    }

    #[test]
    fn create_intent_never_receives_fix_reproducer_guidance() {
        let mut plan = fix_plan("npm run build の失敗を修正してください。");
        plan.intent = "create".to_string();
        assert!(suggestion_for(&plan).is_none());
    }

    #[test]
    fn first_phase_prompt_records_the_suggestion_once() {
        let dir = tempfile::tempdir().unwrap();
        let events = dir.path().join("events.jsonl");
        let plan =
            fix_plan("このNext.jsアプリはdata-anvil-action=\"restart\"欠落で検証に失敗します。");

        let prompt = attach_to_phase_prompt(
            &plan,
            &plan.phases[0],
            Some(&events),
            "base prompt".to_string(),
        );

        assert!(prompt.contains("guidance, not enforcement"));
        assert!(prompt.contains("F1 baseline gate remains authoritative"));
        let event = std::fs::read_to_string(events).unwrap();
        assert_eq!(event.matches("fix_reproducer_suggested").count(), 1);
        assert!(event.contains(r#""basis":"goal_contract_attribute:data-anvil-action=restart""#));
        assert!(event.contains(r#""suggestion":"profile_catalog:hook_attribute_present"#));
    }
}
