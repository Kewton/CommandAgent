#[cfg(test)]
mod cases {
    use std::collections::BTreeMap;
    use std::path::Path;

    use clap::Parser;

    use super::super::*;
    use crate::cli::Cli;
    use crate::planner::fix_reproducer_defect::BeforePhaseOutcome;

    const PIPE_GOAL: &str = "data/sales.csv を処理する pipeline/main.py の実行がエラーで失敗します。原因を特定して修正してください。修正後もデータ契約の既存検証が通ることを確認してください。";
    const SCHEMA_GOAL: &str = "output/results.json がデータ契約のスキーマ検証に失敗します。パイプラインを修正して正しい results.json を再生成し、既存検証が通ることを確認してください。";

    struct SynthesizedCase {
        snapshot: String,
        plans: Vec<StepPlan>,
        events: Vec<serde_json::Value>,
    }

    fn config(root: &Path, profile: &str, intent: &str, goal: &str) -> Config {
        let cwd = root.to_string_lossy().to_string();
        let mut config = Config::from_cli(Cli::parse_from([
            "commandagent",
            "--cwd",
            &cwd,
            "--offline",
            "--profile",
            profile,
            "--intent",
            intent,
            "--plan-preset",
            "profile",
            "--ultra-plan",
            goal,
        ]))
        .unwrap();
        config.eval_events_path = Some(root.join("events.jsonl"));
        config
    }

    fn workspace(root: &Path, pipeline: &str) {
        std::fs::create_dir_all(root.join("pipeline")).unwrap();
        std::fs::create_dir_all(root.join("output")).unwrap();
        std::fs::create_dir_all(root.join("data")).unwrap();
        std::fs::write(root.join("pipeline/main.py"), pipeline).unwrap();
        std::fs::write(root.join("output/results.json"), "{}\n").unwrap();
        std::fs::write(root.join("data/sales.csv"), "region,amount\n東京,10\n").unwrap();
    }

    fn unwrap_generated(result: PhasePlan) -> StepPlan {
        match result {
            PhasePlan::Generated(plan) => plan,
            PhasePlan::ModelReproducer => panic!("measured goal must resolve a catalog reproducer"),
            PhasePlan::NotApplicable => panic!("data fix profile synthesis must apply"),
        }
    }

    fn synthesize_case(goal: &str, pipeline: &str) -> SynthesizedCase {
        let root = tempfile::tempdir().unwrap();
        workspace(root.path(), pipeline);
        let config = config(root.path(), "data", "fix", goal);
        let plan = crate::planner::intent::explicit_fix_plan(goal, "data", "default");
        let mut runtime = FixRuntime::for_plan(&plan, &config).unwrap();
        let mut plans = Vec::new();

        let before_phase = &plan.phases[0];
        let mut before =
            unwrap_generated(phase_plan(&config, &plan, before_phase, Some(&runtime)).unwrap());
        crate::planner::fix_runtime::bind_step_plan(Some(&mut runtime), before_phase, &mut before);
        assert_eq!(
            runtime
                .run_before_phase(&before, &config, &plan, before_phase, 0)
                .unwrap(),
            BeforePhaseOutcome::Confirmed
        );
        plans.push(before);

        for phase in plan.phases.iter().skip(1) {
            let mut step_plan =
                unwrap_generated(phase_plan(&config, &plan, phase, Some(&runtime)).unwrap());
            crate::planner::fix_runtime::bind_step_plan(Some(&mut runtime), phase, &mut step_plan);
            let lint = crate::planner::lint::lint_step_plan_report_with_workspace(
                &step_plan,
                Some(root.path()),
            );
            assert!(lint.is_pass(), "{}", lint.primary_message());
            plans.push(step_plan);
        }

        let evidence_path = runtime.before_evidence_path().unwrap();
        assert!(root.path().join(&evidence_path).is_file());
        let mut snapshot = format!(
            "profile: data\nintent: fix\nf1_execution: confirmed\nf1_evidence: {evidence_path}\n"
        );
        for (phase, step_plan) in plan.phases.iter().zip(&plans) {
            snapshot.push_str(&format!(
                "\nphase: {}\n{}",
                phase.id,
                crate::planner::step_plan::render_step_plan(step_plan)
            ));
        }
        snapshot = snapshot.replace(&evidence_path, "evidence/fix-<run-id>-before.json");
        snapshot = snapshot.replace(
            std::fs::canonicalize(root.path())
                .unwrap()
                .to_string_lossy()
                .as_ref(),
            "<workspace>",
        );
        snapshot = snapshot.replace(root.path().to_string_lossy().as_ref(), "<workspace>");
        let events = std::fs::read_to_string(config.eval_events_path.as_ref().unwrap())
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        SynthesizedCase {
            snapshot,
            plans,
            events,
        }
    }

    #[test]
    fn pipe_goal_synthesized_plan_matches_executed_f1_snapshot() {
        let case = synthesize_case(PIPE_GOAL, "raise ValueError('bad sales row')\n");

        assert_eq!(
            case.snapshot,
            include_str!(
                "../../../tests/corpus/apps/test0718_d2b_data_fix_plan_synthesis/fixtures/pipe-plan.txt"
            )
        );
        assert_synthesis_event(&case.events, "goal_failure_kind:pipeline_execution");
    }

    #[test]
    fn schema_goal_synthesized_plan_matches_executed_f1_snapshot() {
        let case = synthesize_case(SCHEMA_GOAL, "print('pipeline remains available')\n");

        assert_eq!(
            case.snapshot,
            include_str!(
                "../../../tests/corpus/apps/test0718_d2b_data_fix_plan_synthesis/fixtures/schema-plan.txt"
            )
        );
        assert_synthesis_event(&case.events, "goal_profile_contract:data_results_schema");
    }

    fn assert_synthesis_event(events: &[serde_json::Value], basis: &str) {
        let synthesized = events
            .iter()
            .filter(|event| event["event"] == "fix_plan_synthesized")
            .collect::<Vec<_>>();
        assert_eq!(synthesized.len(), 1);
        assert_eq!(synthesized[0]["profile"], "data");
        assert_eq!(synthesized[0]["phase_count"], 4);
        assert_eq!(synthesized[0]["r_basis"], basis);
    }

    #[test]
    fn synthesized_plans_exclude_three_measured_malformed_shapes() {
        let pipe = synthesize_case(PIPE_GOAL, "raise TypeError('amount')\n");
        let schema = synthesize_case(SCHEMA_GOAL, "print('pipeline')\n");

        // dfix-004 v5: "duplicate expected path ownership: pipeline/main.py in
        // inspect-prior-evidence and implement-fix" (and two further variants).
        // dfix-003: "verify step requires at least one verify command".
        // dfix-004 v5: "path does not exist: output/inspection.json".
        for plans in [&pipe.plans, &schema.plans] {
            let mut owners = BTreeMap::new();
            for step in plans.iter().flat_map(|plan| &plan.steps) {
                for path in &step.expected_paths {
                    assert!(
                        owners.insert(path.clone(), step.id.clone()).is_none(),
                        "{path}"
                    );
                }
                if step.step_kind() == StepKind::Verify {
                    assert!(!step.verify.is_empty(), "{}", step.id);
                }
                let rendered = serde_json::to_string(step).unwrap();
                assert!(!rendered.contains("output/inspection.json"), "{rendered}");
            }
            assert_eq!(
                owners.get("pipeline/main.py").map(String::as_str),
                Some("implement-fix")
            );
        }
    }

    #[test]
    fn unresolved_reproducer_uses_model_only_for_r_then_rewraps_structure() {
        let root = tempfile::tempdir().unwrap();
        workspace(root.path(), "raise RuntimeError('broken')\n");
        let goal = "既存の不具合を直してください。";
        let config = config(root.path(), "data", "fix", goal);
        let plan = crate::planner::intent::explicit_fix_plan(goal, "data", "default");
        let phase = &plan.phases[0];
        let runtime = FixRuntime::for_plan(&plan, &config).unwrap();

        assert!(matches!(
            phase_plan(&config, &plan, phase, Some(&runtime)).unwrap(),
            PhasePlan::ModelReproducer
        ));
        let model_plan = StepPlan {
            goal: "model response".to_string(),
            steps: vec![PlanStep {
                id: "candidate-r".to_string(),
                kind: "verify".to_string(),
                expected_result: "pass".to_string(),
                instruction: "Try the observed failure.".to_string(),
                expected_paths: Vec::new(),
                verify: vec!["python3 -B pipeline/main.py".to_string()],
            }],
        };
        let canonical = canonicalize_model_reproducer(&config, &plan, phase, model_plan).unwrap();

        assert_eq!(canonical.steps.len(), 1);
        assert_eq!(canonical.steps[0].id, "reproduce-before");
        assert_eq!(canonical.steps[0].expected_result, "fail");
        assert_eq!(canonical.steps[0].verify, ["python3 -B pipeline/main.py"]);
    }

    #[test]
    fn nextjs_fix_and_create_paths_do_not_select_data_synthesis() {
        let root = tempfile::tempdir().unwrap();
        let next_config = config(root.path(), "nextjs", "fix", "fix the build");
        let next_plan =
            crate::planner::intent::explicit_fix_plan("fix the build", "nextjs", "default");
        assert!(matches!(
            phase_plan(&next_config, &next_plan, &next_plan.phases[0], None).unwrap(),
            PhasePlan::NotApplicable
        ));

        let create_config = config(root.path(), "data", "create", "analyze data");
        let create_plan = crate::planner::profile::profile_preset_ultra_plan(
            "data",
            "analyze data",
            "default",
            "create",
        )
        .unwrap();
        assert!(matches!(
            phase_plan(&create_config, &create_plan, &create_plan.phases[0], None).unwrap(),
            PhasePlan::NotApplicable
        ));
    }
}
