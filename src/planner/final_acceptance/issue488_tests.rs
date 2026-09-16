//! D4 final boundaries; generic profile isolates the actual final executor from
//! the separate Next.js build/browser gates. No synthetic app success is claimed.
#[cfg(test)]
mod tests {
    use super::super::super::*;
    use crate::planner::runner::final_acceptance::*;
    use crate::planner::runner::{
        CompileError, ProfileBehaviorProbeReport, RuntimeAcceptanceReport,
        final_acceptance_release_gate_with_runtime, mark_release_gate_profile_behavior_failed,
    };
    use clap::Parser;
    use serde_json::Value;

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/corpus/apps/issue488-d4-inline")
                .join(name),
        )
        .unwrap()
    }
    fn command(name: &str) -> String {
        let commands: Value = serde_json::from_str(&fixture("commands.json")).unwrap();
        commands[name]["command"].as_str().unwrap().into()
    }
    fn setup(root: &Path, name: &str, variant: &str) -> (Config, UltraPlan) {
        std::fs::create_dir_all(root.join("src/app")).unwrap();
        if variant != "file_missing" {
            std::fs::write(
                root.join("src/app/page.tsx"),
                fixture(&format!("fixtures/{variant}.txt")),
            )
            .unwrap();
        }
        std::fs::write(root.join("README.md"), "D4 marker fixture").unwrap();
        let mut contract: Value = serde_json::from_str(&fixture("contract.json")).unwrap();
        contract["profile"] = json!("generic");
        contract["verify_commands"] = json!([command(name)]);
        std::fs::write(root.join("contract.json"), contract.to_string()).unwrap();
        let mut c =
            Config::from_cli(crate::cli::Cli::parse_from(["commandagent", "--ux-demo"])).unwrap();
        c.workspace_root = root.into();
        c.profile = "generic".into();
        c.profile_explicit = true;
        c.offline = true;
        c.completion_contract_path = Some(root.join("contract.json"));
        c.eval_events_path = Some(root.join("events.jsonl"));
        let p = UltraPlan {
            goal: "Verify saved source markers".into(),
            profile: "generic".into(),
            intent: "create".into(),
            style: "balanced".into(),
            phases: vec![],
        };
        (c, p)
    }
    fn final_event(c: &Config) -> Value {
        std::fs::read_to_string(c.eval_events_path.as_ref().unwrap())
            .unwrap()
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .rfind(|e| e["event"] == "ultra_final_acceptance")
            .unwrap()
    }

    #[test]
    fn issue488_final_executor_keeps_weak_and_actual_command_failure_separate() {
        for name in ["O", "V1", "V2", "SWALLOW"] {
            for variant in [
                "normal",
                "primary",
                "input",
                "state",
                "snapshot",
                "file_missing",
            ] {
                let root = tempfile::tempdir().unwrap();
                let (c, p) = setup(root.path(), name, variant);
                let bytes = std::fs::read(root.path().join("contract.json")).unwrap();
                let report = ultra_final_acceptance_report(&p, &c).unwrap();
                assert_eq!(
                    report.is_pass(),
                    name == "V2" && variant == "normal",
                    "{name}/{variant}: {report:?}"
                );
                let event = final_event(&c);
                let d = &event["command_diagnoses"][0];
                assert_eq!(d["kind"], if name == "V2" { "test" } else { "weak" });
                let command_failed =
                    variant == "file_missing" || variant != "normal" && name != "SWALLOW";
                assert_eq!(
                    report
                        .command_failures
                        .iter()
                        .any(|f| f.command == command(name)),
                    command_failed
                );
                assert_eq!(d["execution_failure"].is_string(), command_failed);
                assert_eq!(
                    std::fs::read(root.path().join("contract.json")).unwrap(),
                    bytes
                );
                // A Test classification cannot bypass the independent Next.js gate.
                let profile =
                    crate::planner::profile::verify_profile(root.path(), "nextjs", &p.goal);
                assert!(!profile.is_pass());
            }
        }
    }

    #[test]
    fn issue488_other_requirements_compile_and_business_failure_remain_independent() {
        let root = tempfile::tempdir().unwrap();
        let (c, p) = setup(root.path(), "O", "normal");
        let path = root.path().join("contract.json");
        let mut contract: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        contract["required_evidence"] =
            json!(["requested_content_evidence", "stateful_update_evidence"]);
        std::fs::write(&path, contract.to_string()).unwrap();
        let mut report = ultra_final_acceptance_report(&p, &c).unwrap();
        let event = final_event(&c);
        assert!(
            event["missing_evidence"]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v == "stateful_update_evidence")
        );
        assert!(
            event["weak_evidence"]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v == "node_smoke_without_assertion")
        );
        // A synthetic compiler observation tests the existing handoff boundary;
        // the reduced text fixture is deliberately not built as an application.
        report.push_compile_errors(
            "npm run build",
            vec![CompileError {
                path: "src/app/page.tsx".into(),
                line: 2,
                column: 1,
                excerpt: String::new(),
                symbol: None,
                route_bound: Some(true),
                message: "Type string is not assignable to number".into(),
            }],
        );
        assert!(!report.is_pass());
        assert_eq!(report.compile_errors.len(), 1);
        assert!(!report.profile_failures.is_empty());

        let other = tempfile::tempdir().unwrap();
        let (c, p) = setup(other.path(), "V2", "normal");
        assert!(ultra_final_acceptance_report(&p, &c).unwrap().is_pass());
        let observations: Value = serde_json::from_str(&fixture("business-controls.json")).unwrap();
        for positive in [true, false] {
            let actual = &observations[if positive { "positive" } else { "negative" }];
            std::fs::write(other.path().join("observed.json"), actual.to_string()).unwrap();
            let oracle = "node -e \"require('node:assert/strict').deepStrictEqual(require('./observed.json'),{saved:true,reloaded:true})\"";
            let check = crate::planner::step_plan::PlanStep {
                id: "independent-oracle".into(),
                kind: "verify".into(),
                expected_result: "pass".into(),
                instruction: "Assert synthetic observations".into(),
                expected_paths: vec![],
                verify: vec![oracle.into()],
            };
            let oracle_report = crate::planner::verify::verify_step(other.path(), &check);
            assert_eq!(oracle_report.is_pass(), positive);
            let runtime = resolve_profile_runtime("generic");
            let acceptance = RuntimeAcceptanceReport {
                passed: true,
                primary_reason: "pass".into(),
                ..Default::default()
            };
            let mut gate = final_acceptance_release_gate_with_runtime(
                &c,
                runtime,
                &p.goal,
                &[],
                Some(&acceptance),
                false,
            );
            if !oracle_report.is_pass() {
                mark_release_gate_profile_behavior_failed(
                    &mut gate,
                    &ProfileBehaviorProbeReport {
                        status: "failed",
                        reasons: vec!["synthetic_saved_reload_oracle_failed".into()],
                        evidence_path: Some("observed.json".into()),
                    },
                );
            }
            assert_eq!(gate.status == "pass", positive);
            if !positive {
                assert!(
                    gate.reasons
                        .iter()
                        .any(|r| r.contains("synthetic_saved_reload_oracle_failed"))
                );
            }
        }
    }
}
