#[cfg(test)]
mod lazy_json_preflight {
    use super::*;

    // Kept beside the preflight mutation tests, independent of #428 and #425 wiring.
    fn json_preflight_workspace(command: &str, existing: bool) -> (tempfile::TempDir, Config) {
        let root = tempfile::tempdir().unwrap();
        let fixture = Path::new("tests/corpus/apps/issue429-lazy-json-preflight");
        for path in [
            "package.json",
            "src/lib/storage.js",
            "src/app/api/staff/route.js",
            "probe.mjs",
        ] {
            let destination = root.path().join(path);
            std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
            std::fs::copy(fixture.join(path), destination).unwrap();
        }
        if existing {
            std::fs::create_dir(root.path().join("data")).unwrap();
            for name in ["staff.json", "shifts.json"] {
                std::fs::write(root.path().join("data").join(name), "[]\n").unwrap();
            }
        }
        let contract_path = root.path().join("completion-contract.json");
        std::fs::write(
            &contract_path,
            serde_json::to_vec(&json!({
                "required_paths": ["src/lib/storage.js", "src/app/api/staff/route.js", "probe.mjs"],
                "verify_commands": [command], "profile": crate::planner::profiles::nextjs::PROFILE_ID
            }))
            .unwrap(),
        )
        .unwrap();
        let mut config = config(root.path(), 1);
        config.completion_contract_path = Some(contract_path);
        (root, config)
    }

    #[test]
    fn json_preflight_first_get_allows_only_lazy_outputs_and_retains_business_failure() {
        for existing in [false, true] {
            let (root, config) =
                json_preflight_workspace("node probe.mjs business-failure", existing);
            let before =
                crate::planner::recovery_snapshot::current_source_sha256(root.path()).unwrap();
            let observed = recovery_preflight(&config, &candidate("lazy-json-failure"), 0);
            assert!(
                matches!(&observed, RecoveryPreflight::Failed { .. }),
                "{observed:?}"
            );
            assert_eq!(
                crate::planner::recovery_snapshot::current_source_sha256(root.path()).unwrap(),
                before
            );
            let observation = root
                .path()
                .join(".commandagent/recovery-observations/attempt-0/workspace");
            for path in ["data/staff.json", "data/shifts.json"] {
                assert!(observation.join(path).is_file());
                if existing {
                    assert_eq!(
                        std::fs::read_to_string(root.path().join(path)).unwrap(),
                        "[]\n"
                    );
                } else {
                    assert!(!root.path().join(path).exists());
                }
            }
            // Feed the real business failure into the existing driver state machine:
            // generated-output permission must not turn it into current success.
            let mut driver = driver(vec![success("synthetic recovery")]);
            driver.preflight = observed;
            let initial = failed(recoverable(contract_bound_candidate("lazy-json-failure")));
            assert_eq!(
                drive(&config, initial, &mut driver).unwrap(),
                "synthetic recovery"
            );
            assert_eq!(driver.starts, [1]);
            let events =
                std::fs::read_to_string(config.eval_events_path.as_ref().unwrap()).unwrap();
            let policy = events
                .lines()
                .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
                .find(|event| event["event"] == "recovery_observation_effect_policy_bound")
                .unwrap();
            let expected: serde_json::Value = serde_json::from_str(include_str!(
                "../../tests/corpus/apps/issue429-lazy-json-preflight/fixtures/lazy-json-preflight.jsonl"
            ).lines().next().unwrap()).unwrap();
            for key in [
                "source",
                "allowed_generated_paths",
                "protected_change_disposition",
                "registered_data_input_fixture",
                "external_oracle_used",
            ] {
                assert_eq!(policy[key], expected[key], "{key}");
            }
            let preflight = events
                .lines()
                .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
                .find(|event| event["event"] == "recovery_preflight_observation")
                .unwrap();
            let expected: serde_json::Value = serde_json::from_str(include_str!(
                "../../tests/corpus/apps/issue429-lazy-json-preflight/fixtures/lazy-json-preflight.jsonl"
            ).lines().nth(1).unwrap()).unwrap();
            for key in [
                "status",
                "observation_phase",
                "source",
                "read_only",
                "observation_isolated",
                "external_oracle_used",
            ] {
                assert_eq!(preflight[key], expected[key], "{key}");
            }
            assert!(
                preflight["reason"]
                    .as_str()
                    .unwrap()
                    .starts_with(expected["reason"].as_str().unwrap())
            );
        }
    }

    #[test]
    fn json_preflight_nested_cwd_fails_closed_and_retains_control() {
        let (root, config) = json_preflight_workspace("node nested-probe.mjs", false);
        let nested = root.path().join("nested");
        std::fs::create_dir(&nested).unwrap();
        for name in ["package.json", "src", "probe.mjs"] {
            std::fs::rename(root.path().join(name), nested.join(name)).unwrap();
        }
        std::fs::write(
            root.path().join("nested-probe.mjs"),
            "process.chdir('nested'); await import('./nested/probe.mjs');\n",
        )
        .unwrap();
        std::fs::write(
            config.completion_contract_path.as_ref().unwrap(),
            serde_json::to_vec(&json!({
                "profile": crate::planner::profiles::nextjs::PROFILE_ID,
                "required_paths": ["nested/src/lib/storage.js", "nested-probe.mjs"],
                "verify_commands": ["node nested-probe.mjs"]
            }))
            .unwrap(),
        )
        .unwrap();
        let before = crate::planner::recovery_snapshot::current_source_sha256(root.path()).unwrap();
        let observed = recovery_preflight(&config, &candidate("nested-json-cwd"), 0);
        assert!(
            matches!(&observed, RecoveryPreflight::Unavailable { reason }
            if reason == "preflight_source_mutation_rejected_and_restored"),
            "{observed:?}"
        );
        let observation = root
            .path()
            .join(".commandagent/recovery-observations/attempt-0/workspace");
        assert!(observation.join("nested/data/staff.json").is_file());
        assert!(!observation.join("data").exists());
        assert!(!root.path().join("data").exists());
        assert!(!nested.join("data").exists());
        assert_eq!(
            crate::planner::recovery_snapshot::current_source_sha256(root.path()).unwrap(),
            before
        );
        let events = std::fs::read_to_string(config.eval_events_path.as_ref().unwrap()).unwrap();
        let policy = events
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
            .find(|event| event["event"] == "recovery_observation_effect_policy_bound")
            .unwrap();
        assert_eq!(policy["allowed_generated_paths"], json!([]));
    }

    #[test]
    fn json_preflight_success_preserves_the_original_workspace() {
        let (root, config) = json_preflight_workspace("node probe.mjs", false);
        let before = crate::planner::recovery_snapshot::current_source_sha256(root.path()).unwrap();
        let observed = recovery_preflight(&config, &candidate("lazy-json-success"), 0);
        assert!(
            matches!(observed, RecoveryPreflight::CurrentSuccess { .. }),
            "{observed:?}"
        );
        assert_eq!(
            crate::planner::recovery_snapshot::current_source_sha256(root.path()).unwrap(),
            before
        );
        assert!(!root.path().join("data").exists());
    }

    #[test]
    fn json_preflight_rejects_source_config_and_unregistered_sibling_mutations() {
        for (path, extra_source) in [
            ("src/lib/storage.js", ""),
            ("data/unregistered.json", ""),
            ("data/staff.json.backup", ""),
            (
                "tsconfig.app.json",
                "\nfs.writeFile('tsconfig.app.json', '[]');\n",
            ),
            (
                "data/custom-settings.json",
                "\nfs.writeFile('data/custom-settings.json', '[]');\n",
            ),
        ] {
            let (root, config) = json_preflight_workspace("node probe.mjs", false);
            if path == "data/custom-settings.json" {
                std::fs::write(
                    root.path().join("next.config.js"),
                    "module.exports = require('./data/custom-settings.json');\n",
                )
                .unwrap();
            }
            if !extra_source.is_empty() {
                use std::io::Write;
                std::fs::OpenOptions::new()
                    .append(true)
                    .open(root.path().join("src/lib/storage.js"))
                    .unwrap()
                    .write_all(extra_source.as_bytes())
                    .unwrap();
            }
            if path.ends_with(".json") {
                std::fs::create_dir_all(root.path().join(path).parent().unwrap()).unwrap();
                std::fs::write(root.path().join(path), "{}\n").unwrap();
            }
            let probe = format!(
                "import {{ GET }} from './src/app/api/staff/route.js';\nimport {{ writeFile }} from 'fs/promises';\nawait GET();\nawait writeFile('{path}', 'changed');\n"
            );
            std::fs::write(root.path().join("probe.mjs"), probe).unwrap();
            let before =
                crate::planner::recovery_snapshot::current_source_sha256(root.path()).unwrap();
            let observed = recovery_preflight(&config, &candidate("lazy-json-mutation"), 0);
            assert!(
                matches!(&observed, RecoveryPreflight::Unavailable { reason }
                if reason == "preflight_source_mutation_rejected_and_restored"),
                "{path}: {observed:?}"
            );
            assert_eq!(
                crate::planner::recovery_snapshot::current_source_sha256(root.path()).unwrap(),
                before
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn json_preflight_rejects_observer_created_symlinks_and_directory_outputs() {
        for statement in [
            "await fs.symlink('missing-target', 'data/staff.json');",
            "await fs.mkdir('data/staff.json'); await fs.writeFile('data/staff.json/nested.json', '[]');",
        ] {
            let (root, config) = json_preflight_workspace("node probe.mjs", false);
            std::fs::write(
                root.path().join("probe.mjs"),
                format!(
                    "import {{ promises as fs }} from 'fs';\nawait fs.mkdir('data');\n{statement}\n"
                ),
            )
            .unwrap();
            let before =
                crate::planner::recovery_snapshot::current_source_sha256(root.path()).unwrap();
            let observed = recovery_preflight(&config, &candidate("lazy-json-invalid-output"), 0);
            assert!(
                matches!(&observed, RecoveryPreflight::Unavailable { reason }
                if reason.starts_with("preflight_source_observation_failed:")),
                "{observed:?}"
            );
            assert_eq!(
                crate::planner::recovery_snapshot::current_source_sha256(root.path()).unwrap(),
                before
            );
            assert!(!root.path().join("data").exists());
        }
    }
}

#[cfg(test)]
mod reopened_json_preflight {
    use super::*;

    #[test]
    fn issue429_campaign_preflight_preserves_isolation_failure_and_source_gates() {
        let fixture = Path::new("tests/corpus/apps/issue429-lazy-json-preflight/fixtures/campaign");
        let manifest: serde_json::Value =
            serde_json::from_slice(&std::fs::read(fixture.join("source-sha256.json")).unwrap())
                .unwrap();
        let expected_events: Vec<serde_json::Value> = include_str!(
            "../../tests/corpus/apps/issue429-lazy-json-preflight/fixtures/campaign-preflight.jsonl"
        )
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
        for (run, outputs) in [
            ("S1", ["data/shifts.json", "data/staff.json"]),
            ("S3", ["data/shifts.json", "data/staff.json"]),
            ("E3", ["data/departments.json", "data/expenses.json"]),
        ] {
            for existing in [false, true] {
                for disposition in ["pass", "fail", "source", "unregistered"] {
                    let root = tempfile::tempdir().unwrap();
                    for path in manifest[run].as_object().unwrap().keys() {
                        let destination = root.path().join(path);
                        std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
                        std::fs::copy(fixture.join(run).join(path), destination).unwrap();
                    }
                    std::fs::write(root.path().join("package.json"), "{}").unwrap();
                    if existing {
                        std::fs::create_dir(root.path().join("data")).unwrap();
                        for path in outputs {
                            std::fs::write(root.path().join(path), "[]\n").unwrap();
                        }
                    }
                    // Replay observation effects against unchanged real sources.
                    // This synthetic observer is not a generated-app execution.
                    let mut probe = String::from(
                        "import fs from 'fs/promises'; await fs.mkdir('data', {recursive: true});\n",
                    );
                    for path in outputs {
                        probe.push_str(&format!(
                            "await fs.writeFile('{path}', '[{{\"probe\":true}}]');\n"
                        ));
                    }
                    match disposition {
                        "fail" => probe.push_str("process.exitCode = 1;\n"),
                        "source" => probe.push_str(
                            "await fs.appendFile('src/app/page.tsx', '\\n// changed');\n",
                        ),
                        "unregistered" => {
                            probe.push_str("await fs.writeFile('data/unregistered.json', '[]');\n")
                        }
                        _ => {}
                    }
                    std::fs::write(root.path().join("probe.mjs"), probe).unwrap();
                    let contract = root.path().join("completion-contract.json");
                    std::fs::write(
                        &contract,
                        serde_json::to_vec(&json!({
                            "profile": crate::planner::profiles::nextjs::PROFILE_ID,
                            "required_paths": ["probe.mjs"], "verify_commands": ["node probe.mjs"]
                        }))
                        .unwrap(),
                    )
                    .unwrap();
                    let mut config = config(root.path(), 1);
                    config.completion_contract_path = Some(contract);
                    let before =
                        crate::planner::recovery_snapshot::current_source_sha256(root.path())
                            .unwrap();
                    let observed = recovery_preflight(&config, &candidate("campaign-json"), 0);
                    match disposition {
                        "pass" => assert!(
                            matches!(observed, RecoveryPreflight::CurrentSuccess { .. }),
                            "{run}: {observed:?}"
                        ),
                        "fail" => assert!(
                            matches!(observed, RecoveryPreflight::Failed { .. }),
                            "{run}: {observed:?}"
                        ),
                        _ => assert!(
                            matches!(&observed, RecoveryPreflight::Unavailable { reason } if reason == "preflight_source_mutation_rejected_and_restored"),
                            "{run}: {observed:?}"
                        ),
                    }
                    assert_eq!(
                        crate::planner::recovery_snapshot::current_source_sha256(root.path())
                            .unwrap(),
                        before,
                        "{run}/{disposition}"
                    );
                    let observation = root
                        .path()
                        .join(".commandagent/recovery-observations/attempt-0/workspace");
                    for path in outputs {
                        assert_eq!(
                            std::fs::read_to_string(observation.join(path)).unwrap(),
                            "[{\"probe\":true}]"
                        );
                        if existing {
                            assert_eq!(
                                std::fs::read_to_string(root.path().join(path)).unwrap(),
                                "[]\n"
                            );
                        } else {
                            assert!(!root.path().join(path).exists());
                        }
                    }
                    let mut driver = driver(vec![success("recovered fixture")]);
                    driver.preflight = observed;
                    let initial = failed(recoverable(contract_bound_candidate("campaign-json")));
                    let result = drive(&config, initial, &mut driver);
                    if disposition == "fail" {
                        assert_eq!(result.unwrap(), "recovered fixture");
                        assert_eq!(driver.starts, [1]);
                    } else {
                        assert!(result.is_err());
                        assert!(driver.starts.is_empty());
                    }
                    let events: Vec<serde_json::Value> =
                        std::fs::read_to_string(config.eval_events_path.as_ref().unwrap())
                            .unwrap()
                            .lines()
                            .map(|line| serde_json::from_str(line).unwrap())
                            .collect();
                    let policy = events
                        .iter()
                        .find(|event| event["event"] == "recovery_observation_effect_policy_bound")
                        .unwrap();
                    let expected = expected_events
                        .iter()
                        .find(|event| event["fixture_run"] == run)
                        .unwrap();
                    for key in [
                        "allowed_generated_paths",
                        "source",
                        "protected_change_disposition",
                        "external_oracle_used",
                        "registered_data_input_fixture",
                    ] {
                        assert_eq!(policy[key], expected[key], "{run}: {key}");
                    }
                    let event = events
                        .iter()
                        .find(|event| event["event"] == "recovery_preflight_observation")
                        .unwrap();
                    assert_eq!(
                        event["status"],
                        match disposition {
                            "pass" => "pass",
                            "fail" => "fail",
                            _ => "unavailable",
                        }
                    );
                    assert_eq!(event["observation_isolated"], true);
                }
            }
        }
    }
}
