use commandagent::planner::profiles::nextjs;

#[test]
fn business_prompts_keep_entities_integration_and_goal_neutral_observability() {
    for goal in [
        "在庫と受注を管理する",
        "スタッフとシフトを管理する",
        "部門と経費申請を管理する",
    ] {
        let plan = nextjs::preset_ultra_plan(goal, "default", "create").unwrap();
        let core = &plan.phases[1].prompt;
        assert!(core.contains(goal));
        for required in [
            "src/lib/types.ts",
            "export every function",
            "JSON response shape",
            "Response.ok",
            "{error, details?}",
        ] {
            assert!(core.contains(required), "{required}: {core}");
        }
        let text = format!(
            "{} {} {} {:?}",
            nextjs::generation_rules("create"),
            nextjs::guidance(goal),
            nextjs::runtime_contract("create", goal),
            plan.phases
        );
        for forbidden in [
            "extend the instrumented skeleton",
            "player/paddle",
            "game-over",
            "victory",
            "during play",
        ] {
            assert!(!text.contains(forbidden), "{forbidden}: {text}");
        }
        for required in [
            "ENOENT",
            "5xx",
            "read-modify-write",
            "atomically rename",
            "related data",
            "requested period",
            "parent deletion",
        ] {
            assert!(text.contains(required), "missing {required}");
        }
    }
}

#[test]
fn space_breakout_quiz_scenario_matrix_preserves_hooks_build_and_goal() {
    for goal in [
        "Create a Space Invaders game",
        "Create a Breakout game",
        "Create an interactive Quiz",
    ] {
        let plan = nextjs::preset_ultra_plan(goal, "default", "create").unwrap();
        assert_eq!(plan.phases.len(), 4);
        assert!(plan.phases[1].prompt.contains(goal));
        assert!(plan.phases[3].prompt.contains("verification-only"));
        for required in [
            "data-anvil-state",
            "immediately responds to input",
            "data-anvil-action=\"primary\"",
            "data-anvil-action=\"restart\"",
            "start_or_restart_flow",
        ] {
            assert!(
                plan.phases[2].prompt.contains(required),
                "{goal}: missing {required}"
            );
        }
        assert!(nextjs::generation_rules("create").contains("next build"));
    }
}
