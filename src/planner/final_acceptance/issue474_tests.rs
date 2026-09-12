#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::minimal_loop::completion::CompletionContract;
    use crate::planner::runner::final_acceptance::*;
    use crate::planner::verify::VerificationReport;
    use clap::Parser;
    use serde_json::Value;

    const PAGE: &str =
        include_str!("../../../tests/corpus/apps/issue474-compound-node-hooks/src/app/page.tsx");

    fn command() -> String {
        let commands: Vec<String> = serde_json::from_str(include_str!(
        "../../../tests/corpus/apps/issue474-compound-node-hooks/fixtures/original-commands.json"
    ))
    .unwrap();
        commands[1].clone()
    }

    fn write(root: &Path, path: &str, text: &str) {
        let path = root.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    fn contract(command: &str) -> CompletionContract {
        serde_json::from_value(json!({
            "required_paths": ["src/app/page.tsx"],
            "verify_commands": [command],
            "required_evidence": ["implementation_artifact"]
        }))
        .unwrap()
    }

    fn setup(root: &Path, command: &str) -> (Config, UltraPlan) {
        write(root, "src/app/page.tsx", PAGE);
        write(
            root,
            "contract.json",
            &serde_json::to_string(&contract(command)).unwrap(),
        );
        let mut config =
            Config::from_cli(crate::cli::Cli::parse_from(["commandagent", "--ux-demo"])).unwrap();
        config.workspace_root = root.to_owned();
        config.profile = "generic".into();
        config.completion_contract_path = Some(root.join("contract.json"));
        config.eval_events_path = Some(root.join("events.jsonl"));
        let plan = UltraPlan {
            goal: "Verify the declared structural source".into(),
            profile: "generic".into(),
            intent: "create".into(),
            style: "balanced".into(),
            phases: vec![],
        };
        (config, plan)
    }

    fn final_event(config: &Config) -> Value {
        std::fs::read_to_string(config.eval_events_path.as_ref().unwrap())
            .unwrap()
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .rfind(|event| event["event"] == "ultra_final_acceptance")
            .unwrap()
    }

    #[test]
    fn issue474_three_predicates_fail_in_final_execution_and_repair_targets_the_source() {
        let root = tempfile::tempdir().unwrap();
        let command = command();
        let (config, plan) = setup(root.path(), &command);
        assert!(
            ultra_final_acceptance_report(&plan, &config)
                .unwrap()
                .is_pass()
        );
        assert_eq!(final_event(&config)["weak_evidence"], json!([]));
        // A prior final pass and a different file with all hooks cannot satisfy a
        // newly failing command whose registered source is still src/app/page.tsx.
        write(root.path(), "src/components/Other.tsx", PAGE);
        for (predicate, error) in [
            ("data-anvil-action=\"primary\"", "missing primary"),
            ("data-anvil-action=\"input\"", "missing input"),
            ("data-anvil-state", "missing state"),
        ] {
            write(
                root.path(),
                "src/app/page.tsx",
                &PAGE.replace(predicate, "data-removed"),
            );
            let raw = crate::bounded_process::run_with_timeout(
                std::process::Command::new("sh")
                    .args(["-c", &command])
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .current_dir(root.path()),
                std::time::Duration::from_secs(5),
            )
            .unwrap();
            assert!(!raw.success());
            assert!(String::from_utf8_lossy(&raw.stderr).contains(&format!("Error: {error}")));
            let failed = ultra_final_acceptance_report_with_cycle(&plan, &config, 1).unwrap();
            assert!(!failed.is_pass(), "{predicate}: {failed:?}");
            assert!(
                failed
                    .command_failures
                    .iter()
                    .any(|failure| failure.command == command
                        && failure.reason.contains("exit status: 1")),
                "{failed:?}"
            );
            let attribute_paths =
                contract_attribute_repair_target_paths(root.path(), "nextjs", &failed);
            let selection =
                crate::planner::repair_targeting::resolve_final_acceptance_repair_targets(
                    crate::planner::repair_targeting::FinalAcceptanceRepairTargetInput {
                        root: root.path(),
                        profile: "nextjs",
                        pending_evidence: &final_acceptance_repair_signals(&failed),
                        contract_attribute_paths: &attribute_paths,
                        repair_changed_paths: &["src/app/api/projects/route.ts".into()],
                        required_paths: &["src/app/page.tsx".into()],
                        diagnosis_path: None,
                    },
                );
            assert_eq!(selection.primary_target(), Some("src/app/page.tsx"));
            assert_eq!(selection.selection_reason, "contract_attribute");
            assert_eq!(final_event(&config)["weak_evidence"], json!([]));
            write(
                root.path(),
                "src/app/api/projects/route.ts",
                "export function GET(){return Response.json({ok:true});}",
            );
            assert!(
                !ultra_final_acceptance_report_with_cycle(&plan, &config, 2)
                    .unwrap()
                    .is_pass()
            );
            write(root.path(), "src/app/page.tsx", PAGE);
            assert!(
                ultra_final_acceptance_report_with_cycle(&plan, &config, 3)
                    .unwrap()
                    .is_pass()
            );
        }
        std::fs::remove_file(root.path().join("src/app/page.tsx")).unwrap();
        let missing = ultra_final_acceptance_report_with_cycle(&plan, &config, 4).unwrap();
        assert!(!missing.is_pass());
        assert!(!missing.command_failures.is_empty());
    }

    #[test]
    fn issue474_unrelated_api_edits_do_not_clear_the_same_weak_command() {
        let root = tempfile::tempdir().unwrap();
        // Equivalent executable predicates, deliberately outside the closed spelling.
        let unsupported = command().replace("const fs=", "const fs = ");
        let (config, plan) = setup(root.path(), &unsupported);
        let failed = ultra_final_acceptance_report(&plan, &config).unwrap();
        assert!(!failed.is_pass());
        let before = final_event(&config);
        assert_eq!(
            before["weak_evidence"],
            json!(["node_smoke_without_assertion"])
        );
        assert_eq!(before["weak_evidence_sources"][0]["command"], unsupported);
        write(
            root.path(),
            "src/app/api/projects/route.ts",
            "export function GET(){return Response.json({ok:true});}",
        );
        write(
            root.path(),
            "tests/unrelated.test.js",
            "const assert=require('assert');assert(true);",
        );
        let after = ultra_final_acceptance_report_with_cycle(&plan, &config, 1).unwrap();
        assert!(!after.is_pass());
        assert_eq!(
            final_event(&config)["weak_evidence_sources"],
            before["weak_evidence_sources"]
        );
        // A relevant, equivalent command spelling plus fresh execution can progress.
        // This is a newly configured fixture, not a mutation of a live run registry.
        let (corrected, plan) = setup(root.path(), &command());
        assert!(
            ultra_final_acceptance_report_with_cycle(&plan, &corrected, 2)
                .unwrap()
                .is_pass()
        );
        assert_eq!(final_event(&corrected)["weak_evidence"], json!([]));
    }

    #[test]
    fn issue474_saved_repair_selection_reproduces_before_classification_correction() {
        let root = tempfile::tempdir().unwrap();
        let events: Vec<Value> = serde_json::from_str(include_str!(
        "../../../tests/corpus/apps/issue474-compound-node-hooks/fixtures/original-repair-events.json"
    )).unwrap();
        let failure = events
            .iter()
            .find(|e| e["event"] == "ultra_final_acceptance_failed")
            .unwrap();
        let repair = events
            .iter()
            .find(|e| e["event"] == "final_acceptance_repair_start")
            .unwrap();
        let report =
            VerificationReport::profile_failed(failure["primary_reason"].as_str().unwrap());
        let pending = final_acceptance_repair_signals(&report);
        assert!(pending.is_empty());
        let changed: Vec<String> =
            serde_json::from_value(repair["selected_targets"].clone()).unwrap();
        let selection = crate::planner::repair_targeting::resolve_final_acceptance_repair_targets(
            crate::planner::repair_targeting::FinalAcceptanceRepairTargetInput {
                root: root.path(),
                profile: "nextjs",
                pending_evidence: &pending,
                contract_attribute_paths: &[],
                repair_changed_paths: &changed,
                required_paths: &["src/app/page.tsx".into()],
                diagnosis_path: None,
            },
        );
        assert_eq!(selection.selected_targets, changed);
        assert_eq!(selection.selection_reason, repair["selection_reason"]);
        // The preserved source report is a pre-fix observation, not a new live run.
        assert_eq!(repair["selected_evidence_keys"], json!([]));
    }

    #[test]
    fn issue474_failure_masks_and_removed_predicates_do_not_unlock_final_acceptance() {
        let root = tempfile::tempdir().unwrap();
        write(root.path(), "src/app/page.tsx", PAGE);
        let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../tests/corpus/apps/issue474-compound-node-hooks/fixtures/negative-commands.json"
    ))
    .unwrap();
        for case in cases {
            let command = case["command"].as_str().unwrap();
            let report = contract(command).verify_with_goal(root.path(), "Check source hooks");
            assert!(!report.is_pass(), "{case}: {report:?}");
        }
    }

    #[test]
    fn issue474_diagnostic_handoff_reproduces_distinct_attribute_mismatch() {
        // A documented follow-up reproduction, not desired diagnostic behavior.
        // The closed check stays structural and fails honestly; error-summary
        // extraction / predicate attribution is a separate existing contract.
        let cases: Vec<Value> = serde_json::from_str(include_str!(
            "../../../tests/corpus/apps/issue474-compound-node-hooks/fixtures/diagnostic-handoff.json"
        )).unwrap();
        for case in cases {
            let root = tempfile::tempdir().unwrap();
            let command = command();
            let (config, plan) = setup(root.path(), &command);
            write(
                root.path(),
                "src/app/page.tsx",
                &PAGE.replace(case["remove"].as_str().unwrap(), "data-removed"),
            );
            let raw = crate::bounded_process::run_with_timeout(
                std::process::Command::new("sh")
                    .args(["-c", &command])
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .current_dir(root.path()),
                std::time::Duration::from_secs(5),
            )
            .unwrap();
            let actual_error = case["actual_error"].as_str().unwrap();
            assert!(!raw.success());
            assert!(String::from_utf8_lossy(&raw.stderr).contains(actual_error));
            let report = ultra_final_acceptance_report(&plan, &config).unwrap();
            assert!(!report.is_pass());
            assert_eq!(final_event(&config)["weak_evidence"], json!([]));
            assert!(
                report
                    .command_failures
                    .iter()
                    .all(|failure| !failure.reason.contains(actual_error))
            );
            let diagnosis = crate::planner::contract_attribute_repair::detect(&report).unwrap();
            assert_eq!(diagnosis.path, "src/app/page.tsx");
            assert_eq!(diagnosis.attribute, case["observed_attribute"]);
            println!(
                "diagnostic handoff: actual={actual_error}, reported_attribute={}, path={}",
                diagnosis.attribute, diagnosis.path
            );
        }
    }
}
