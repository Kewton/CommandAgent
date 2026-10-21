// Fixed source + measured compiler diagnostics; scripted, model-free gate replay.
// The independent real Next.js matrix is scripts/issue448_nextjs_r0.py.
#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::minimal_loop::build_verifier::{BuildVerifierRequirement, observe_requirement};
    use crate::minimal_loop::import_scan::{format_missing_import_findings, scan_relative_imports};
    use crate::planner::recovery_snapshot::{self, current_source_sha256};
    use serde_json::Value;

    const FIXTURE: &str = "tests/corpus/apps/issue448-nextjs-r0";
    const TASK_ROUTE: &str = "src/app/api/tasks/[id]/route.ts";

    fn fixture_json(path: &str) -> Value {
        serde_json::from_slice(&std::fs::read(Path::new(FIXTURE).join(path)).unwrap()).unwrap()
    }

    fn overlay(root: &Path, case: &str) {
        let cases = fixture_json("cases.json");
        let case = cases
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["name"] == case)
            .unwrap();
        for name in case["overlays"].as_array().unwrap() {
            copy_fixture_tree(
                &Path::new(FIXTURE)
                    .join("overlays")
                    .join(name.as_str().unwrap()),
                root,
            );
        }
    }

    fn build_result(case: &str) -> Value {
        fixture_json("evidence/build-results.json")["results"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["case"] == case)
            .unwrap()
            .clone()
    }

    fn write_build_replay(root: &Path, result: &Value) {
        std::fs::write(
            root.join("fixture-diagnostics.txt"),
            result["diagnostics"].as_str().unwrap(),
        )
        .unwrap();
        std::fs::write(
            root.join("fixture-build.sh"),
            format!(
                "cat fixture-diagnostics.txt\nexit {}\n",
                result["exit_code"].as_i64().unwrap()
            ),
        )
        .unwrap();
    }

    #[test]
    fn issue448_both_diagnostics_remain_repairable_and_partial_fixes_fail() {
        for case in fixture_json("cases.json").as_array().unwrap() {
            let name = case["name"].as_str().unwrap();
            let root = tempfile::tempdir().unwrap();
            copy_fixture_tree(&Path::new(FIXTURE).join("original"), root.path());
            overlay(root.path(), name);
            let missing = scan_relative_imports(root.path(), &[TASK_ROUTE.to_string()]).unwrap();
            let findings = format_missing_import_findings(root.path(), &missing).join("\n");
            let export_broken = matches!(name, "original" | "ui-only" | "promise-only");
            assert_eq!(
                findings.contains("does not export isValidTaskStatus"),
                export_broken,
                "{name}: {findings}"
            );
            if export_broken {
                assert!(findings.contains(TASK_ROUTE), "{findings}");
            } else {
                assert!(missing.is_empty(), "{name}: {findings}");
            }
            let measured = build_result(name);
            assert_eq!(measured["exit_code"], case["exit_code"]);
            for (path, expected_hash) in measured["source_sha256"].as_object().unwrap() {
                let bytes = std::fs::read(root.path().join(path)).unwrap();
                assert_eq!(
                    format!("{:x}", Sha256::digest(bytes)),
                    expected_hash.as_str().unwrap(),
                    "{name}: stale build result for {path}"
                );
            }
            write_build_replay(root.path(), &measured);
            // Enter through the bounded verifier, which retains the complete output
            // of this scripted observation before the production parser sees it.
            let observation = observe_requirement(
                root.path(),
                &BuildVerifierRequirement {
                    command: "sh fixture-build.sh".to_string(),
                    profile: Some("nextjs".to_string()),
                    reason: "measured R0 diagnostic replay".to_string(),
                    authority: "fixture".to_string(),
                    status: "required".to_string(),
                    requires_dependency_setup: false,
                    required_for_completion: true,
                },
            );
            assert!(observation.attempted, "{name}: {observation:?}");
            let errors = observation.compile_errors;
            if name == "repaired" {
                assert!(errors.is_empty());
                assert_eq!(measured["exit_code"], 0);
                assert_eq!(observation.status.as_str(), "passed");
            } else {
                assert_eq!(measured["exit_code"], 1);
                assert_eq!(observation.status.as_str(), "failed");
                assert!(Path::new(&observation.output_path).is_file());
                assert!(!errors.is_empty(), "{name}: {measured}");
                let (expected_path, expected_line) = match name {
                    "promise-only" => (TASK_ROUTE, 8),
                    "projects-and-export" => ("src/app/api/tasks/route.ts", 167),
                    _ => ("src/app/api/projects/route.ts", 23),
                };
                assert!(
                    errors
                        .iter()
                        .any(|e| e.path == expected_path && e.line == expected_line),
                    "{name}: {errors:?}"
                );
                assert!(
                    errors.iter().all(|e| e.path.starts_with("src/app/api/")),
                    "{errors:?}"
                );
                assert!(!crate::minimal_loop::compile_repair_scope::only_foreign(
                    Some(root.path()),
                    &errors
                ));
                assert_eq!(
                    crate::minimal_loop::compile_repair_scope::repairable(
                        Some(root.path()),
                        &errors
                    )
                    .len(),
                    errors.len()
                );
            }
        }
    }

    #[test]
    fn issue448_original_contract_is_carried_unchanged_into_recovery() {
        let root = tempfile::tempdir().unwrap();
        copy_fixture_tree(&Path::new(FIXTURE).join("original"), root.path());
        let mut config = config(root.path(), 1);
        let original =
            std::fs::read(Path::new(FIXTURE).join("evidence/original-completion-contract.json"))
                .unwrap();
        let contract = root.path().join(".commandagent/completion-contract.json");
        std::fs::write(&contract, &original).unwrap();
        config.completion_contract_path = Some(contract);
        let snapshot = recovery_snapshot::capture_for_transaction(root.path(), 1).unwrap();
        let treatment = recovery_snapshot::prepare_treatment(root.path(), &snapshot, 1).unwrap();
        overlay(&treatment, "repaired");
        let bound =
            crate::planner::recovery_contract_binding::bind_config(&config, &treatment).unwrap();
        assert_eq!(
            std::fs::read(bound.completion_contract_path.as_ref().unwrap()).unwrap(),
            original
        );
        assert_eq!(
            recovery_observer_identity(&config),
            recovery_observer_identity(&bound)
        );
        let (_, hash, observers, count) = recovery_observer_telemetry(&bound);
        assert_eq!(
            hash,
            "eb2b04647a28ab10508c21e9c1cd026bea293a1c7f95dda261f4bd5529d8c9ab"
        );
        assert!(observers.contains(&"nextjs_browser_interaction_v1".to_string()));
        assert_eq!(count, 1);
        assert_eq!(
            current_source_sha256(root.path()).unwrap(),
            snapshot.snapshot_sha256
        );
    }

    #[derive(Clone, Copy, Debug)]
    enum Boundary {
        ExecutionFailed,
        AllChecksPass,
        AdditionalCheckFails,
        MissingEvidence,
        DeletedApi,
        WeakenedContract,
    }

    fn replay_boundary(case: &str, boundary: Boundary, expected_reason: &str) {
        let root = tempfile::tempdir().unwrap();
        copy_fixture_tree(&Path::new(FIXTURE).join("original"), root.path());
        // These scripts explicitly replay observations, not a compiler or business oracle.
        // The corresponding real compiler exits/diagnostics are checked independently.
        let build = build_result(case);
        write_build_replay(root.path(), &build);
        std::fs::write(
            root.path().join("fixture-acceptance.sh"),
            "test -f fixture-acceptance-passed.txt\n",
        )
        .unwrap();
        if !matches!(boundary, Boundary::AdditionalCheckFails) {
            std::fs::write(
                root.path().join("fixture-acceptance-passed.txt"),
                "scripted independent check passed\n",
            )
            .unwrap();
        }
        let original_contract = fixture_json("evidence/original-completion-contract.json");
        let mut required_paths = original_contract["required_paths"]
            .as_array()
            .unwrap()
            .clone();
        required_paths.extend(
            [
                "src/lib/types.ts",
                "src/app/api/projects/route.ts",
                "src/app/api/tasks/route.ts",
                TASK_ROUTE,
            ]
            .map(|p| json!(p)),
        );
        // Deliberately generic gate control: no claim that scripted observations satisfy
        // the full R0 browser/business contract (preserved and checked above).
        let contract = json!({
            "profile": "generic",
            "goal": original_contract["goal"],
            "required_paths": required_paths,
            "verify_commands": ["sh fixture-build.sh", "sh fixture-acceptance.sh"],
            "required_evidence": if matches!(boundary, Boundary::MissingEvidence) { vec!["test_artifact"] } else { vec![] },
        });
        let mut config = config(root.path(), 1);
        config.offline = true;
        let contract_path = root.path().join(".commandagent/completion-contract.json");
        std::fs::write(&contract_path, serde_json::to_vec(&contract).unwrap()).unwrap();
        config.completion_contract_path = Some(contract_path);
        let snapshot = recovery_snapshot::capture_for_transaction(root.path(), 1).unwrap();
        let before = snapshot.snapshot_sha256.clone();
        let treatment = recovery_snapshot::prepare_treatment(root.path(), &snapshot, 1).unwrap();
        let bound =
            crate::planner::recovery_contract_binding::bind_config(&config, &treatment).unwrap();
        overlay(&treatment, case);
        if matches!(boundary, Boundary::DeletedApi) {
            std::fs::remove_file(treatment.join(TASK_ROUTE)).unwrap();
        }
        if matches!(boundary, Boundary::WeakenedContract) {
            // The host-owned copy is read-only. Simulate replacing the file in this
            // disposable treatment, which must still fail observer identity checks.
            std::fs::remove_file(bound.completion_contract_path.as_ref().unwrap()).unwrap();
            std::fs::write(
                bound.completion_contract_path.as_ref().unwrap(),
                b"{\"profile\":\"generic\",\"verify_commands\":[\"true\"]}",
            )
            .unwrap();
        } else {
            assert_eq!(
                recovery_observer_identity(&config),
                recovery_observer_identity(&bound)
            );
        }
        let mut planner = UnusedClient;
        let mut execution = UnusedClient;
        let mut driver = RunnerRecoveryDriver {
            planner: &mut planner,
            execution: &mut execution,
            config: &config,
            ui: &crate::tui::NoopUi,
            transaction_snapshot: Some(snapshot),
            transaction_treatment: Some(treatment.clone()),
            transaction_config: Some(bound),
            transaction_observer_identity: recovery_observer_identity(&config),
        };
        let outcome = if matches!(boundary, Boundary::ExecutionFailed) {
            assert_eq!(build["exit_code"], 1);
            failed(AttemptFailure::NonRecoverable)
        } else {
            success("scripted repair execution completed; final verification still required")
        };
        let outcome = driver.finish(1, &candidate("r0-regression"), outcome);
        let promoted = expected_reason == "registered_final_success_passed";
        assert_eq!(
            outcome.result.is_ok(),
            promoted,
            "{case}: {:?}",
            outcome.result
        );
        let after = current_source_sha256(root.path()).unwrap();
        if promoted {
            assert_ne!(after, before);
            assert_eq!(after, current_source_sha256(&treatment).unwrap());
        } else {
            assert_eq!(after, before, "rejected candidate changed control");
        }
        let events: Vec<Value> = std::fs::read_to_string(config.eval_events_path.as_ref().unwrap())
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        let decision = events
            .iter()
            .find(|e| e["event"] == "recovery_promotion_decision")
            .unwrap();
        assert_eq!(
            decision["decision"],
            if promoted { "promoted" } else { "rejected" }
        );
        assert!(
            decision["reason"]
                .as_str()
                .unwrap()
                .contains(expected_reason),
            "{case}: {decision}"
        );
        assert_eq!(
            events
                .iter()
                .any(|e| e["event"] == "recovery_treatment_promoted"),
            promoted
        );
        if !promoted {
            let retained = events
                .iter()
                .find(|e| e["event"] == "recovery_control_retained")
                .unwrap();
            assert_eq!(retained["control_snapshot_sha256"], before);
        }
        if case == "ui-only" {
            let delta = events
                .iter()
                .find(|e| e["event"] == "recovery_treatment_delta")
                .unwrap();
            assert_eq!(
                delta["attempted_product_delta"]["changed_paths"],
                json!(["src/app/page.tsx"])
            );
            assert_eq!(delta["attempted_product_delta"]["added_paths"], json!([]));
            assert_eq!(delta["attempted_product_delta"]["removed_paths"], json!([]));
        }
        println!(
            "ISSUE448_BOUNDARY {}",
            json!({
                "case": case, "boundary": format!("{boundary:?}"),
                "kind": "scripted observations through production Recovery gate",
                "build_exit_code": build["exit_code"], "decision": decision,
                "control_before_sha256": before, "control_after_sha256": after,
                "treatment_sha256": current_source_sha256(&treatment).unwrap(),
            })
        );
    }

    #[test]
    fn issue448_ui_only_failed_execution_retains_control() {
        replay_boundary(
            "ui-only",
            Boundary::ExecutionFailed,
            "recovery_execution_failed",
        );
    }

    #[test]
    fn issue448_claimed_success_cannot_promote_any_remaining_compile_failure() {
        for case in [
            "original",
            "ui-only",
            "export-only",
            "promise-only",
            "projects-and-export",
        ] {
            replay_boundary(
                case,
                Boundary::AllChecksPass,
                "command failed: sh fixture-build.sh",
            );
        }
    }

    #[test]
    fn issue448_repaired_build_requires_every_remaining_gate() {
        for (boundary, reason) in [
            (Boundary::AllChecksPass, "registered_final_success_passed"),
            (
                Boundary::AdditionalCheckFails,
                "command failed: sh fixture-acceptance.sh",
            ),
            (
                Boundary::MissingEvidence,
                "registered_observations_passed_but_completion_contract_acceptance_failed",
            ),
            (Boundary::DeletedApi, TASK_ROUTE),
            (
                Boundary::WeakenedContract,
                "recovery_observer_authority_changed",
            ),
        ] {
            replay_boundary("repaired", boundary, reason);
        }
    }
}
