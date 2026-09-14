#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::minimal_loop::evidence::RuntimeAcceptanceReport;
    use crate::planner::recovery_snapshot::current_source_sha256;
    use serde_json::Value;
    use std::os::unix::fs::{PermissionsExt, symlink};

    const FIXTURE: &str = "tests/corpus/apps/issue485-scaffold-completion";
    const PAGE: &str = "src/app/page.tsx";

    fn read_json(path: impl AsRef<Path>) -> Value {
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
    }

    fn fixture_json(path: &str) -> Value {
        read_json(Path::new(FIXTURE).join(path))
    }

    fn write(root: &Path, path: &str, value: impl AsRef<[u8]>) {
        let target = root.join(path);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(target, value).unwrap();
    }

    fn events(config: &Config) -> Vec<Value> {
        std::fs::read_to_string(config.eval_events_path.as_ref().unwrap())
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    fn event<'a>(events: &'a [Value], name: &str) -> &'a Value {
        events.iter().rfind(|e| e["event"] == name).unwrap()
    }

    fn configure(root: &Path, case: &str) -> (Config, RecoveryCandidate) {
        copy_fixture_tree(&Path::new(FIXTURE).join("scaffold"), root);
        if case.starts_with("business") {
            copy_fixture_tree(&Path::new(FIXTURE).join("business"), root);
        }
        if case == "unrelated-api" {
            write(
                root,
                "src/app/api/health/route.ts",
                "export function GET() { return Response.json({healthy: true}); }\n",
            );
        }
        let contract = Path::new(FIXTURE).join("completion-contract.json");
        write(
            root,
            ".commandagent/completion-contract.json",
            std::fs::read(contract).unwrap(),
        );
        let mut config = config(root, 2);
        config.profile = "nextjs".into();
        config.offline = false; // Installed test transports; no dependency or browser downloads.
        config.completion_contract_path = Some(root.join(".commandagent/completion-contract.json"));
        let plan = crate::planner::ultra_plan::parse_ultra_plan(
            &std::fs::read_to_string(Path::new(FIXTURE).join("original-plan.yaml")).unwrap(),
        )
        .unwrap();
        let boundary = fixture_json("failed-core-boundary.json");
        let mut candidate = candidate(&plan.goal);
        config.action = crate::config::Action::UltraPlanRun(plan.goal.clone());
        candidate.plan = plan;
        candidate.handoff.profile = "nextjs".into();
        candidate.handoff.failure_kind = boundary["failure_kind"].as_str().unwrap().into();
        candidate.handoff.failed_step = None;
        candidate.handoff.verify_commands = vec!["npm run build".into()];
        candidate.verify_command_source = "completion_contract".into();
        candidate.original_intent = Some("create".into());
        configure_transport(root, case);
        (config, candidate)
    }

    fn configure_transport(root: &Path, case: &str) {
        let modules = root.join("node_modules");
        let runtime = modules.join(".issue485");
        // Only the HTTP transport uses a disposable port. The original plan,
        // package scripts, contract and registered observer retain 60302.
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        write(&runtime, "transport-port.txt", port.to_string());
        write(
        root,
        ".anvil/evidence/browser-probe-command.json",
        json!({
            "program": "sh", "args": ["node_modules/.bin/npm", "run", "start"],
            "env": {"PORT": port.to_string()}, "display": "npm run start (scripted HTTP transport)",
            "port": port, "require_build": true
        })
        .to_string(),
    );
        let mut inputs = fixture_json("source-manifest.json")["files"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|f| {
                f["fixture"]
                    .as_str()
                    .unwrap()
                    .strip_prefix("scaffold/")
                    .map(str::to_owned)
            })
            .collect::<Vec<_>>();
        if case == "unrelated-api" {
            inputs.push("src/app/api/health/route.ts".into());
        }
        for path in &inputs {
            write(
                &runtime,
                &format!("expected/{path}"),
                std::fs::read(root.join(path)).unwrap(),
            );
        }
        write(&runtime, "inputs.txt", inputs.join("\n") + "\n");
        write(
            &runtime,
            "test-exe.txt",
            std::env::current_exe().unwrap().to_str().unwrap(),
        );
        write(
            &runtime,
            "build-exit.txt",
            if case == "business-build-failed" {
                "1"
            } else {
                "0"
            },
        );
        write(
            &runtime,
            "availability.json",
            json!({
                "available": !matches!(case, "business-missing-interaction" | "unrelated-api"),
                "reason": "issue485_scripted_interaction_unavailable",
                "location": "issue485_scripted_observation", "version": "fixture"
            })
            .to_string(),
        );
        let observation = if case.starts_with("business") {
            "business"
        } else {
            "saved-scaffold"
        };
        write(
            &runtime,
            "interaction.json",
            serde_json::to_vec(&fixture_json(&format!(
                "observations/{observation}-interaction.json"
            )))
            .unwrap(),
        );
        let package = read_json(root.join("package.json"));
        for kind in ["dependencies", "devDependencies"] {
            for name in package[kind].as_object().unwrap().keys() {
                std::fs::create_dir_all(modules.join(name)).unwrap();
            }
        }
        let npm = modules.join(".bin/npm");
        write(
            &modules,
            ".bin/npm",
            std::fs::read(Path::new(FIXTURE).join("runtime.sh")).unwrap(),
        );
        std::fs::set_permissions(&npm, std::fs::Permissions::from_mode(0o755)).unwrap();
        symlink("npm", modules.join(".bin/next")).unwrap();
    }

    fn observed_config(config: &Config) -> Config {
        let observation = config
            .workspace_root
            .join(".commandagent/recovery-observations/attempt-0/workspace");
        let mut bound = config.clone();
        bound.workspace_root = observation;
        bound.completion_contract_path = Some(
            bound
                .workspace_root
                .join(".commandagent/recovery-runtime/completion-contract.json"),
        );
        assert_eq!(
            std::fs::read(bound.completion_contract_path.as_ref().unwrap()).unwrap(),
            std::fs::read(config.completion_contract_path.as_ref().unwrap()).unwrap()
        );
        bound
    }

    fn runtime_acceptance(
        config: &Config,
        candidate: &RecoveryCandidate,
    ) -> RuntimeAcceptanceReport {
        crate::planner::runner::recovery_acceptance::runtime_acceptance_report(
            &candidate.plan,
            config,
        )
        .unwrap()
    }

    fn assert_control(config: &Config, before: &str) {
        assert_eq!(
            current_source_sha256(&config.workspace_root).unwrap(),
            before
        );
        let all = events(config);
        let observer = event(&all, "recovery_capability_observer_bound");
        assert_eq!(observer["port"], 60302);
        assert_eq!(observer["observer_id"], "nextjs_browser_interaction_v1");
        let audit = event(&all, "recovery_preflight_control_audit");
        assert_eq!(audit["control_before_sha256"], before);
        assert_eq!(audit["control_after_observation_sha256"], before);
        assert_eq!(audit["control_status"], "unchanged");
        assert_eq!(audit["control_retained"], true);
        assert_eq!(audit["restore_invoked"], false);
        assert_eq!(audit["restore_succeeded"], false);
        assert!(audit.get("control_after_restore_sha256").is_none());
        assert!(
            !all.iter()
                .any(|e| e["event"] == "recovery_plan_auto_run_start")
        );
    }

    fn assert_evaluation_reached(events: &[Value]) {
        let stages = events
            .iter()
            .filter(|e| e["event"] == "recovery_preflight_effect_observation")
            .map(|e| e["stage"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            stages,
            [
                "before_observation",
                "nextjs_capabilities",
                "registered_verification",
                "completion_acceptance"
            ]
        );
    }

    #[test]
    fn issue485_saved_sources_and_failed_core_boundary_are_traceable() {
        let manifest = fixture_json("source-manifest.json");
        for file in manifest["files"].as_array().unwrap() {
            let bytes =
                std::fs::read(Path::new(FIXTURE).join(file["fixture"].as_str().unwrap())).unwrap();
            assert_eq!(
                format!("{:x}", Sha256::digest(bytes)),
                file["source_sha256"]
            );
            assert_eq!(file["fixture_sha256"], file["source_sha256"]);
        }
        let boundary = fixture_json("failed-core-boundary.json");
        assert_eq!(boundary["completed_phase_ids"], json!(["project-setup"]));
        assert_eq!(boundary["failed_phase_id"], "core-implementation");
        assert_eq!(boundary["core_step_count"], Value::Null);
        assert_eq!(boundary["registered_verify_commands_from_failed_plan"], 0);
        assert_eq!(boundary["planner_attempts"].as_array().unwrap().len(), 3);
        assert_eq!(boundary["planner_attempts"][2]["status"], "exhausted");
        assert_eq!(
            manifest["historical_observations"]["internal_r0_gate"],
            "unknown"
        );
        assert_eq!(
            manifest["historical_observations"]["external_reference_goal"],
            "reference_goal_fail"
        );
    }

    #[test]
    fn issue485_saved_scaffold_stops_real_recovery_and_process_without_restore() {
        let root = tempfile::tempdir().unwrap();
        let (config, candidate) = configure(root.path(), "scaffold");
        assert!(
            crate::planner::profiles::nextjs::is_engine_owned_scaffold_page(
                PAGE,
                &std::fs::read_to_string(root.path().join(PAGE)).unwrap()
            )
        );
        let before = current_source_sha256(root.path()).unwrap();
        let mut planner = UnusedClient;
        let mut execution = UnusedClient;
        let mut driver = RunnerRecoveryDriver {
            planner: &mut planner,
            execution: &mut execution,
            config: &config,
            ui: &crate::tui::NoopUi,
            transaction_snapshot: None,
            transaction_treatment: None,
            transaction_config: None,
            transaction_observer_identity: None,
        };
        // Enter after the saved core planning failure, without replaying model plans
        // or treating the historical event stream as execution input.
        let boundary = fixture_json("failed-core-boundary.json");
        let result = drive(
            &config,
            AttemptOutcome {
                result: Err(anyhow::anyhow!(
                    boundary["error"].as_str().unwrap().to_owned()
                )),
                failure: Some(AttemptFailure::Recoverable(Box::new(candidate.clone()))),
            },
            &mut driver,
        );
        assert!(result.is_err());
        assert!(driver.transaction_snapshot.is_none());
        crate::tui::slash::emit_tui_command_stop(&config, "/ultra-plan-run", &result);
        crate::emit_run_stop(&config, &result.map(|_| ()));
        assert_control(&config, &before);
        let all = events(&config);
        assert_evaluation_reached(&all);
        let expected =
            std::fs::read_to_string(Path::new(FIXTURE).join("expected-events.jsonl")).unwrap();
        let mut cursor = 0;
        // The first two events describe the frozen input boundary, not a replayed
        // planner. Compare the remaining stable schema projections in actual order.
        for line in expected.lines().skip(2) {
            let expected: Value = serde_json::from_str(line).unwrap();
            let found = all[cursor..]
                .iter()
                .position(|actual| {
                    expected
                        .as_object()
                        .unwrap()
                        .iter()
                        .all(|(key, value)| actual.get(key) == Some(value))
                })
                .unwrap_or_else(|| panic!("missing ordered event {expected}; actual={all:?}"));
            cursor += found + 1;
        }
        assert!(!all.iter().any(|e| matches!(
            e["event"].as_str(),
            Some(
                "recovery_suppressed_current_success"
                    | "recovery_plan_auto_run_complete"
                    | "ultra_final_acceptance"
            )
        )));
        let acceptance = runtime_acceptance(&observed_config(&config), &candidate);
        assert!(!acceptance.passed);
        assert!(
            acceptance
                .missing_evidence
                .contains(&"implementation_artifact".into())
        );
        let page = acceptance
            .artifact_obligations
            .iter()
            .find(|a| a.path == PAGE)
            .unwrap();
        assert_eq!(page.role, "scaffold");
        assert!(!page.satisfies_implementation);
        println!(
            "ISSUE485_RECORD {}",
            json!({
                "case": "scaffold", "kind": "scripted transport; production decisions",
                "audit": event(&all, "recovery_preflight_control_audit"),
                "preflight": event(&all, "recovery_preflight_observation"),
                "stop": event(&all, "recovery_plan_auto_run_stopped"),
                "runtime_acceptance": {"passed": acceptance.passed, "reason": acceptance.primary_reason,
                    "missing_evidence": acceptance.missing_evidence, "page_role": page.role},
                "terminal": {"task_status": event(&all, "run_stop")["task_status"],
                    "status": event(&all, "run_stop")["status"],
                    "final_acceptance_status": event(&all, "run_stop")["final_acceptance_status"]}
            })
        );
    }

    #[test]
    fn issue485_business_artifact_and_remaining_completion_gates_are_independent() {
        for case in [
            "business-all",
            "business-missing-interaction",
            "business-build-failed",
            "unrelated-api",
        ] {
            let root = tempfile::tempdir().unwrap();
            let (config, candidate) = configure(root.path(), case);
            let before = current_source_sha256(root.path()).unwrap();
            let preflight = recovery_preflight(&config, &candidate, 0);
            assert_control(&config, &before);
            let mut observed = observed_config(&config);
            let acceptance = runtime_acceptance(&observed, &candidate);
            assert!(
                !acceptance
                    .missing_evidence
                    .contains(&"implementation_artifact".into()),
                "{case}: {acceptance:?}"
            );
            let artifact = acceptance
                .artifact_obligations
                .iter()
                .find(|a| a.path == PAGE)
                .unwrap();
            assert_eq!(artifact.satisfies_implementation, case != "unrelated-api");
            if case == "unrelated-api" {
                let api = crate::minimal_loop::evidence::artifact_obligation_evidence(
                    &observed.workspace_root,
                    &["src/app/api/health/route.ts".into()],
                    &[],
                );
                assert!(api[0].satisfies_implementation);
            }
            match case {
                "business-all" => {
                    assert!(
                        matches!(preflight, RecoveryPreflight::CurrentSuccess { .. }),
                        "{preflight:?}"
                    );
                    assert_evaluation_reached(&events(&config));
                    assert!(acceptance.passed, "{acceptance:?}");
                    for required in fixture_json("completion-contract.json")["required_evidence"]
                        .as_array()
                        .unwrap()
                    {
                        assert_eq!(
                            acceptance.evidence_tiers[required.as_str().unwrap()],
                            "strong"
                        );
                    }
                    assert!(acceptance.missing_capabilities.is_empty());
                    assert!(acceptance.missing_obligations.is_empty());
                }
                "business-build-failed" => {
                    assert!(
                        matches!(&preflight, RecoveryPreflight::Unavailable { reason }
                        if reason == "nextjs_route_observation_failed:build_verifier_failed"),
                        "{preflight:?}"
                    );
                    assert!(
                        !events(&config)
                            .iter()
                            .any(|e| e["stage"] == "completion_acceptance")
                    );
                }
                _ => {
                    assert!(
                        matches!(&preflight, RecoveryPreflight::Unavailable { reason }
                    if reason.contains("nextjs_interaction_observation_unavailable")),
                        "{case}: {preflight:?}"
                    );
                    assert_evaluation_reached(&events(&config));
                }
            }
            // Preflight's CurrentSuccess alone is never used as final acceptance.
            // Invoke the normal final gate with the identical contract and inputs.
            observed.eval_events_path = Some(
                observed
                    .workspace_root
                    .join(".commandagent/final-events.jsonl"),
            );
            let final_report =
                crate::planner::runner::recovery_acceptance::final_acceptance_report(
                    &candidate.plan,
                    &observed,
                )
                .unwrap();
            let final_events = events(&observed);
            let final_event = event(&final_events, "ultra_final_acceptance");
            // VerificationReport covers executable/static checks; missing browser
            // evidence can leave it Pass while the real final gate stays partial.
            if case == "business-all" {
                assert_eq!(final_event["external_contract_ok"], true);
                assert_eq!(final_event["runtime_acceptance_passed"], true);
                assert_eq!(final_event["final_acceptance_status"], "full_success");
                assert!(final_report.is_pass());
                assert_eq!(final_event["release_gate_status"], "pass");
            } else if case == "business-build-failed" {
                assert!(!final_report.is_pass());
                assert_eq!(final_event["external_contract_ok"], false);
            } else if case == "unrelated-api" {
                assert!(!final_report.is_pass());
                assert_eq!(final_event["final_acceptance_status"], "incomplete");
                assert_eq!(final_event["release_gate_status"], "failed");
            } else {
                assert_eq!(final_event["final_acceptance_status"], "partial");
                assert_eq!(final_event["release_gate_status"], "partial");
                assert!(
                    final_event["primary_reason"]
                        .as_str()
                        .unwrap()
                        .contains("browser_interaction_evidence_required")
                );
            }
            let terminal_result = if final_report.is_pass() {
                Ok("verification complete".into())
            } else {
                Err(anyhow::anyhow!(final_report.primary_reason()))
            };
            crate::tui::slash::emit_tui_command_stop(
                &observed,
                "/ultra-plan-run",
                &terminal_result,
            );
            let terminal_events = events(&observed);
            let terminal = event(&terminal_events, "tui_command_stop");
            assert_eq!(
                terminal["task_status"],
                if case == "business-all" {
                    "complete"
                } else if case == "business-missing-interaction" {
                    "partial"
                } else {
                    "failed"
                }
            );
            if case != "business-all" {
                assert_ne!(terminal["completion_status"], "complete");
            }
            println!(
                "ISSUE485_RECORD {}",
                json!({
                    "case": case, "kind": "scripted transport; production decisions",
                    "preflight": format!("{preflight:?}"),
                    "audit": event(&events(&config), "recovery_preflight_control_audit"),
                    "runtime_acceptance": {"passed": acceptance.passed, "missing_evidence": acceptance.missing_evidence,
                        "missing_capabilities": acceptance.missing_capabilities, "missing_obligations": acceptance.missing_obligations,
                        "evidence_tiers": acceptance.evidence_tiers, "page_role": artifact.role},
                    "verification_report_passed": final_report.is_pass(),
                    "final": {"status": final_event["final_acceptance_status"], "release_gate": final_event["release_gate_status"],
                        "external_contract_ok": final_event["external_contract_ok"], "runtime_acceptance_passed": final_event["runtime_acceptance_passed"],
                        "reason": final_event["primary_reason"], "release_gate_reasons": final_event["release_gate_reasons"],
                        "assurance": final_event["assurance_level"]},
                    "terminal": {"task_status": terminal["task_status"], "completion_status": terminal["completion_status"],
                        "command_status": terminal["status"]}
                })
            );
        }
    }
}
