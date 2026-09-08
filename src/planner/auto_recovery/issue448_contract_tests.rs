// Original-contract promotion coverage. Browser inputs are explicitly scripted.
#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::planner::recovery_snapshot::{self, current_source_sha256};
    use serde_json::Value;
    use std::os::unix::fs::{PermissionsExt, symlink};

    const SOURCE: &str = "tests/corpus/apps/issue448-nextjs-r0";
    const OBSERVATIONS: &str = "tests/corpus/apps/issue448-nextjs-promotion";
    const CONTRACT_HASH: &str = "eb2b04647a28ab10508c21e9c1cd026bea293a1c7f95dda261f4bd5529d8c9ab";

    fn read_json(path: impl AsRef<Path>) -> Value {
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
    }

    fn configure_runtime(root: &Path, variant: &str, scenario: &str) {
        let modules = root.join("node_modules");
        let runtime = modules.join(".issue448");
        std::fs::create_dir_all(modules.join(".bin")).unwrap();
        let expected = runtime.join("expected");
        copy_fixture_tree(&Path::new(SOURCE).join("original"), &expected);
        let real_modules = std::env::var_os("ISSUE448_REAL_NODE_MODULES");
        let base_variant = if variant == "ui-repaired" {
            "repaired"
        } else {
            variant
        };
        let cases = read_json(Path::new(SOURCE).join("cases.json"));
        let case = cases
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["name"] == base_variant)
            .unwrap();
        for overlay in case["overlays"].as_array().unwrap() {
            copy_fixture_tree(
                &Path::new(SOURCE)
                    .join("overlays")
                    .join(overlay.as_str().unwrap()),
                &expected,
            );
        }
        if variant == "ui-repaired" {
            copy_fixture_tree(&Path::new(SOURCE).join("overlays/ui-only"), &expected);
        }
        let results = read_json(Path::new(SOURCE).join("evidence/build-results.json"));
        let mut result = results["results"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["case"] == base_variant)
            .unwrap()
            .clone();
        if variant == "ui-repaired" {
            let ui = results["results"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["case"] == "ui-only")
                .unwrap();
            result["source_sha256"]["src/app/page.tsx"] =
                ui["source_sha256"]["src/app/page.tsx"].clone();
            if real_modules.is_none() {
                let measured =
                    read_json(Path::new(OBSERVATIONS).join("measured-contract-results.json"));
                let positive = &measured["results"][0];
                assert_eq!(positive["scenario"], "all_original_observations_pass");
                assert_eq!(positive["build_mode"], "real");
                assert_eq!(positive["build_exit_code"], 0);
                assert_eq!(positive["source_sha256"], result["source_sha256"]);
            }
        }
        let inputs = result["source_sha256"].as_object().unwrap();
        for (relative, hash) in inputs {
            assert_eq!(
                format!(
                    "{:x}",
                    Sha256::digest(std::fs::read(expected.join(relative)).unwrap())
                ),
                hash.as_str().unwrap()
            );
        }
        std::fs::write(
            runtime.join("inputs.txt"),
            inputs.keys().cloned().collect::<Vec<_>>().join("\n") + "\n",
        )
        .unwrap();
        std::fs::write(
            runtime.join("build-diagnostics.txt"),
            result["diagnostics"].as_str().unwrap(),
        )
        .unwrap();
        std::fs::write(
            runtime.join("build-exit.txt"),
            result["exit_code"].to_string(),
        )
        .unwrap();
        std::fs::write(
            runtime.join("test-exe.txt"),
            std::env::current_exe().unwrap().to_str().unwrap(),
        )
        .unwrap();
        std::fs::write(
            runtime.join("http-status.txt"),
            if scenario == "http_failure" {
                "500"
            } else {
                "200"
            },
        )
        .unwrap();
        std::fs::write(runtime.join("availability.json"), json!({"available": scenario != "missing_interaction", "reason": "issue448_scripted_interaction_unavailable", "location": "issue448_scripted_observation", "version": "fixture"}).to_string()).unwrap();
        let mut interaction = read_json(Path::new(OBSERVATIONS).join("interaction.json"));
        if scenario == "failed_interaction" {
            for key in [
                "ok",
                "input_state_change",
                "state_changed",
                "visible_state_changed",
            ] {
                interaction[key] = json!(false);
            }
            interaction["status"] = json!("failed");
            interaction["failure_kind"] = json!("no_state_change");
            interaction["state_dimensions_changed"] = json!([]);
            interaction["steps"] = json!([
                "surface_visible",
                "start_transition",
                "control_input_dispatched"
            ]);
        }
        std::fs::write(runtime.join("interaction.json"), interaction.to_string()).unwrap();
        // The real mode installs the original lock in disposable storage. The fast
        // mode replays measured output only after checking every frozen input.
        if let Some(real_modules) = real_modules {
            let real_modules = PathBuf::from(real_modules);
            for entry in std::fs::read_dir(&real_modules).unwrap() {
                let entry = entry.unwrap();
                if entry.file_name() != ".bin" {
                    symlink(entry.path(), modules.join(entry.file_name())).unwrap();
                }
            }
            for entry in std::fs::read_dir(real_modules.join(".bin")).unwrap() {
                let entry = entry.unwrap();
                assert_ne!(entry.file_name(), "npm");
                symlink(entry.path(), modules.join(".bin").join(entry.file_name())).unwrap();
            }
            std::fs::write(
                runtime.join("real-npm.txt"),
                std::env::var("ISSUE448_REAL_NPM").unwrap(),
            )
            .unwrap();
        } else {
            let package = read_json(root.join("package.json"));
            for kind in ["dependencies", "devDependencies"] {
                for name in package[kind].as_object().unwrap().keys() {
                    std::fs::create_dir_all(modules.join(name)).unwrap();
                }
            }
            symlink("npm", modules.join(".bin/next")).unwrap();
        }
        let npm = modules.join(".bin/npm");
        std::fs::copy(Path::new(OBSERVATIONS).join("runtime.sh"), &npm).unwrap();
        std::fs::set_permissions(&npm, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn run_case(scenario: &str, expected_reason: &str) {
        let root = tempfile::tempdir().unwrap();
        copy_fixture_tree(&Path::new(SOURCE).join("original"), root.path());
        let variant = if scenario == "build_failure" {
            "ui-only"
        } else if scenario == "missing_implementation" {
            "repaired"
        } else {
            "ui-repaired"
        };
        configure_runtime(root.path(), variant, scenario);
        let mut config = config(root.path(), 1);
        config.offline = false;
        config.profile = "nextjs".to_string();
        let contract_bytes =
            std::fs::read(Path::new(SOURCE).join("evidence/original-completion-contract.json"))
                .unwrap();
        assert_eq!(
            format!("{:x}", Sha256::digest(&contract_bytes)),
            CONTRACT_HASH
        );
        let contract: Value = serde_json::from_slice(&contract_bytes).unwrap();
        let path = root.path().join(".commandagent/completion-contract.json");
        std::fs::write(&path, &contract_bytes).unwrap();
        config.completion_contract_path = Some(path);
        let mut candidate = candidate(contract["goal"].as_str().unwrap());
        candidate.plan.profile = "nextjs".to_string();
        candidate.handoff.profile = "nextjs".to_string();
        candidate.handoff.verify_commands = vec!["npm run build".to_string()];
        let snapshot = recovery_snapshot::capture_for_transaction(root.path(), 1).unwrap();
        let before = snapshot.snapshot_sha256.clone();
        let treatment = recovery_snapshot::prepare_treatment(root.path(), &snapshot, 1).unwrap();
        copy_fixture_tree(
            &root.path().join("node_modules/.issue448/expected"),
            &treatment,
        );
        let bound =
            crate::planner::recovery_contract_binding::bind_config(&config, &treatment).unwrap();
        assert_eq!(
            std::fs::read(bound.completion_contract_path.as_ref().unwrap()).unwrap(),
            contract_bytes
        );
        assert_eq!(
            recovery_observer_identity(&config),
            recovery_observer_identity(&bound)
        );
        let mut planner = UnusedClient;
        let mut execution = UnusedClient;
        let mut driver = RunnerRecoveryDriver {
            planner: &mut planner,
            execution: &mut execution,
            config: &config,
            ui: &crate::tui::NoopUi,
            transaction_snapshot: Some(snapshot),
            transaction_treatment: Some(treatment.clone()),
            transaction_config: Some(bound.clone()),
            transaction_observer_identity: recovery_observer_identity(&config),
        };
        let outcome = driver.finish(
            1,
            &candidate,
            success("fixture repair completed; original Next.js observations required"),
        );
        let events: Vec<Value> = std::fs::read_to_string(config.eval_events_path.as_ref().unwrap())
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        let decision = events
            .iter()
            .find(|e| e["event"] == "recovery_promotion_decision")
            .unwrap();
        let observation =
            treatment.join(".commandagent/recovery-observations/attempt-129/workspace");
        let trace = std::fs::read_to_string(
            observation.join(".commandagent/evidence/issue448-commands.txt"),
        )
        .unwrap_or_default();
        assert!(trace.contains("npm run build"), "{decision}: {trace}");
        let observed_contract =
            observation.join(".commandagent/recovery-runtime/completion-contract.json");
        assert_eq!(std::fs::read(&observed_contract).unwrap(), contract_bytes);
        assert_eq!(
            std::fs::read(bound.completion_contract_path.as_ref().unwrap()).unwrap(),
            contract_bytes
        );
        let expected = root.path().join("node_modules/.issue448/expected");
        let inputs =
            std::fs::read_to_string(root.path().join("node_modules/.issue448/inputs.txt")).unwrap();
        let source_hashes: serde_json::Map<String, Value> = inputs
            .lines()
            .map(|relative| {
                let bytes = std::fs::read(expected.join(relative)).unwrap();
                assert_eq!(std::fs::read(observation.join(relative)).unwrap(), bytes);
                (
                    relative.to_string(),
                    json!(format!("{:x}", Sha256::digest(bytes))),
                )
            })
            .collect();
        let real = std::env::var_os("ISSUE448_REAL_NODE_MODULES").is_some();
        let build_exit = if scenario == "build_failure" { 1 } else { 0 };
        assert!(trace.contains(&format!(
            "npm run build [{}] exit={build_exit}",
            if real { "real" } else { "measured-replay" }
        )));
        if real {
            let output = std::fs::read_to_string(
                observation.join(".commandagent/evidence/issue448-build-output.txt"),
            )
            .unwrap();
            assert!(
                output.contains("Linting and checking validity of types"),
                "{output}"
            );
            if build_exit == 0 {
                for text in [
                    "Generating static pages (6/6)",
                    "/api/projects",
                    "/api/tasks",
                    "/api/tasks/[id]",
                ] {
                    assert!(output.contains(text), "{text}: {output}");
                }
            } else {
                assert!(output.contains("isValidTaskStatus"), "{output}");
            }
        }
        let observer = events
            .iter()
            .find(|e| e["event"] == "recovery_capability_observer_bound")
            .unwrap();
        assert_eq!(observer["observer_id"], "nextjs_browser_interaction_v1");
        assert_eq!(
            observer["required_capabilities"],
            contract["required_capabilities"]
        );
        assert_eq!(observer["port"], 60302);
        let promoted = scenario == "all_original_observations_pass";
        let after = current_source_sha256(root.path()).unwrap();
        assert_eq!(
            outcome.result.is_ok(),
            promoted,
            "{scenario}: {decision}; trace={trace}"
        );
        assert!(
            decision["reason"]
                .as_str()
                .unwrap()
                .contains(expected_reason),
            "{scenario}: {decision}"
        );
        assert_eq!(
            decision["decision"],
            if promoted { "promoted" } else { "rejected" }
        );
        assert_eq!(
            std::fs::read(config.completion_contract_path.as_ref().unwrap()).unwrap(),
            contract_bytes
        );
        if promoted {
            assert_eq!(after, current_source_sha256(&treatment).unwrap());
            assert_ne!(before, after);
        } else {
            assert_eq!(before, after);
        }
        if promoted {
            assert_eq!(
                trace
                    .lines()
                    .filter(|line| line.starts_with("npm run build ["))
                    .count(),
                2
            );
            assert!(trace.contains("npm run start"));
        }
        let evidence =
            crate::minimal_loop::interaction_probe::browser_interaction_evidence_path(&observation);
        let interaction = evidence.exists().then(|| read_json(&evidence));
        if promoted || scenario == "missing_implementation" {
            assert_eq!(interaction.as_ref().unwrap()["ok"], true);
        }
        let readiness = read_json(
            crate::minimal_loop::browser_probe::browser_readiness_evidence_path(&observation),
        );
        if scenario != "build_failure" {
            assert_eq!(readiness["dev_server"]["child_spawned"], true);
            assert_eq!(readiness["dev_server"]["child_reaped"], true);
        }
        assert_eq!(
            readiness["ok"],
            scenario != "build_failure" && scenario != "http_failure"
        );
        if scenario == "missing_interaction"
            || scenario == "http_failure"
            || scenario == "build_failure"
        {
            assert!(interaction.is_none());
        } else {
            assert_eq!(
                interaction.as_ref().unwrap()["ok"],
                scenario != "failed_interaction"
            );
        }
        // Read the same static acceptance result for a reviewable requirement-by-
        // requirement record; finish has already made its ordinary gate decision.
        let mut observation_config = bound.clone();
        observation_config.workspace_root = observation.clone();
        observation_config.completion_contract_path = Some(observed_contract);
        let acceptance = crate::planner::runner::recovery_acceptance::runtime_acceptance_report(
            &candidate.plan,
            &observation_config,
        )
        .unwrap();
        if promoted {
            assert!(acceptance.passed);
            for evidence in contract["required_evidence"].as_array().unwrap() {
                assert_eq!(
                    acceptance.evidence_tiers[evidence.as_str().unwrap()],
                    "strong"
                );
            }
            assert!(acceptance.missing_capabilities.is_empty());
            assert!(acceptance.missing_evidence.is_empty());
            assert!(acceptance.missing_obligations.is_empty());
            let original_contract = serde_json::from_slice(&contract_bytes).unwrap();
            assert!(completion_acceptance_passes_after_registered_observation(
                &acceptance,
                &original_contract
            ));
        }
        let record = json!({
            "scenario": scenario,
            "kind": "scripted HTTP and interaction inputs through original Next.js observer; no browser/model run",
            "contract_sha256": CONTRACT_HASH,
            "required_capabilities": contract["required_capabilities"],
            "required_evidence": contract["required_evidence"],
            "required_obligations": contract["required_obligations"],
            "verify_commands": contract["verify_commands"],
            "observer": observer,
            "decision": decision,
            "commands": trace.lines().collect::<Vec<_>>(),
            "build_mode": if real { "real" } else { "measured-replay" },
            "build_exit_code": build_exit,
            "source_sha256": source_hashes,
            "interaction": interaction,
            "readiness": readiness,
            "static_acceptance": {
                "passed": acceptance.passed,
                "primary_reason": acceptance.primary_reason,
                "missing_capabilities": acceptance.missing_capabilities,
                "missing_evidence": acceptance.missing_evidence,
                "missing_obligations": acceptance.missing_obligations,
                "weak_evidence": acceptance.weak_evidence,
                "evidence_tiers": acceptance.evidence_tiers,
                "artifact_obligations": acceptance.artifact_obligations,
                "capability_evidence_bindings": acceptance.capability_evidence_bindings
            },
            "control_before_sha256": before,
            "control_after_sha256": after
        });
        // Scrub disposable paths; retain hashes, requirements and decision details.
        println!(
            "ISSUE448_ORIGINAL_CONTRACT {}",
            record
                .to_string()
                .replace(
                    root.path().canonicalize().unwrap().to_str().unwrap(),
                    "<workspace>"
                )
                .replace(root.path().to_str().unwrap(), "<workspace>")
        );
    }

    #[test]
    fn issue448_original_contract_finish_matrix() {
        for (scenario, reason) in [
            (
                "all_original_observations_pass",
                "registered_final_success_passed",
            ),
            ("build_failure", "build_verifier_failed"),
            (
                "failed_interaction",
                "nextjs_interaction_observation_failed",
            ),
            (
                "missing_interaction",
                "nextjs_interaction_observation_unavailable",
            ),
            ("http_failure", "http_500"),
            (
                "missing_implementation",
                "non_implementation_obligation_only:scaffold",
            ),
        ] {
            run_case(scenario, reason);
        }
    }
}
