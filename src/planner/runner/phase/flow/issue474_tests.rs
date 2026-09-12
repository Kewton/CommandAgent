#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::minimal_loop::completion::CompletionContract;
    use crate::planner::recovery_contract_authority as authority;
    use crate::planner::step_plan::PlanStep;
    use clap::Parser;

    fn commands() -> Vec<String> {
        serde_json::from_str(include_str!(
        "../../../../../tests/corpus/apps/issue474-compound-node-hooks/fixtures/original-commands.json"
    ))
    .unwrap()
    }

    fn step(commands: Vec<String>) -> StepPlan {
        StepPlan {
            goal: "Verify source hooks".into(),
            steps: vec![PlanStep {
                id: "check-hooks".into(),
                kind: "verify".into(),
                instruction: "Verify the three declared source hooks".into(),
                expected_result: "pass".into(),
                expected_paths: vec![],
                verify: commands,
            }],
        }
    }

    #[test]
    fn issue474_admission_refresh_and_recovery_preserve_the_original_registry() {
        let root = tempfile::tempdir().unwrap();
        let mut config =
            Config::from_cli(crate::cli::Cli::parse_from(["commandagent", "--ux-demo"])).unwrap();
        config.workspace_root = root.path().to_owned();
        config.eval_events_path =
            Some(root.path().join(".commandagent/runs/issue474/events.jsonl"));
        std::fs::create_dir_all(config.eval_events_path.as_ref().unwrap().parent().unwrap())
            .unwrap();
        let plan = UltraPlan {
            goal: "Create an interactive Next.js form".into(),
            profile: "nextjs".into(),
            intent: "create".into(),
            style: "balanced".into(),
            phases: vec![],
        };
        let (_paths, _guard) = begin(&config, &plan).unwrap();
        let original = commands();
        register_plan(&config, &step(original.clone())).unwrap();
        let first = authority::load_for_handoff(&config).unwrap().unwrap();
        assert_eq!(first.verify_commands, original);
        // Subsequent plans cannot erase an admitted check. New unrelated genuine
        // Tests retain their own identity and do not stand in for the hook command.
        register_plan(&config, &step(vec![])).unwrap();
        register_plan(
            &config,
            &step(vec!["npm test".into(), "node --test".into()]),
        )
        .unwrap();
        initialize(&config, &plan, &first.required_paths).unwrap();
        let expected = authority::load_for_handoff(&config).unwrap().unwrap();
        assert!(expected.verify_commands.contains(&original[1]));
        assert!(expected.verify_commands.contains(&"npm test".into()));
        assert!(expected.verify_commands.contains(&"node --test".into()));
        for replacement in [
            vec![],
            vec!["node -e \"console.log('ok')\"".into()],
            vec![original[1].replace("src/app/page.tsx", "src/components/Other.tsx")],
        ] {
            let bound = authority::bind_for_recovery(&config, &replacement).unwrap();
            let actual = CompletionContract::load_for_config(&bound)
                .unwrap()
                .unwrap();
            assert_eq!(actual.verify_commands, expected.verify_commands);
        }
        // The structural check also remains path-specific at actual execution.
        let page = include_str!(
            "../../../../../tests/corpus/apps/issue474-compound-node-hooks/src/app/page.tsx"
        );
        std::fs::create_dir_all(root.path().join("src/components")).unwrap();
        std::fs::create_dir_all(root.path().join("src/app")).unwrap();
        std::fs::write(root.path().join("src/components/Other.tsx"), page).unwrap();
        std::fs::write(
            root.path().join("src/app/page.tsx"),
            "export default function Page(){return <main/>}",
        )
        .unwrap();
        let alternate = original[1].replace("src/app/page.tsx", "src/components/Other.tsx");
        let alternate_report =
            crate::planner::verify::verify_step(root.path(), &step(vec![alternate]).steps[0]);
        assert!(alternate_report.is_pass(), "{alternate_report:?}");
        let retained_report = crate::planner::verify::verify_step(
            root.path(),
            &step(vec![original[1].clone()]).steps[0],
        );
        assert!(!retained_report.is_pass());
        assert_eq!(retained_report.command_failures[0].command, original[1]);
    }
}
