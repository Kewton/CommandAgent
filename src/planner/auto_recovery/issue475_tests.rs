#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::planner::recovery_snapshot::{current_source_sha256, source_file_sha256};
    use serde_json::Value;

    include!("issue475_tests/runtime_transport.rs");

    const FIXTURE: &str = "tests/corpus/apps/issue475-store-preflight";

    /// Temp dirs must stay outside the read-allowed system prefixes. On Linux
    /// `tempfile::tempdir()` lives under `/tmp`, which lets the product read
    /// guard admit a path outside the workspace as a system path and fail only
    /// there (Issue #604). `CARGO_TARGET_TMPDIR` is set for integration tests
    /// only, so a unit test falls back to the test binary's target directory.
    fn issue475_tempdir() -> tempfile::TempDir {
        let parent = std::env::var_os("CARGO_TARGET_TMPDIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::env::current_exe()
                    .expect("test executable path")
                    .parent()
                    .and_then(std::path::Path::parent)
                    .expect("target profile directory")
                    .join("tmp")
            });
        std::fs::create_dir_all(&parent).unwrap();
        let dir = tempfile::tempdir_in(&parent).expect("create issue475 tempdir");
        assert!(
            !["/usr", "/bin", "/opt", "/etc", "/tmp"]
                .iter()
                .any(|prefix| dir.path().starts_with(prefix)),
            "issue475 tempdir must not sit under a system prefix: {}",
            dir.path().display()
        );
        dir
    }

    fn workspace(mode: &str) -> (tempfile::TempDir, Config) {
        let root = issue475_tempdir();
        for p in [
            "src/lib/store.ts",
            "src/lib/types.ts",
            "src/app/api/projects/route.ts",
            "src/app/api/tasks/route.ts",
        ] {
            let dest = root.path().join(p);
            std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
            std::fs::copy(Path::new(FIXTURE).join("historical").join(p), dest).unwrap();
        }
        for p in ["probe.mjs", "observer.mjs", "interaction.sh"] {
            std::fs::copy(Path::new(FIXTURE).join(p), root.path().join(p)).unwrap();
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(
                root.path().join("interaction.sh"),
                std::fs::Permissions::from_mode(0o755),
            )
            .unwrap();
        }
        std::fs::write(root.path().join("package.json"), r#"{"type":"module","scripts":{"build":"node observer.mjs build","start":"node observer.mjs start"}}"#).unwrap();
        std::fs::write(root.path().join("control.txt"), "original").unwrap();
        let contract = root.path().join("completion-contract.json");
        std::fs::write(&contract, json!({"profile":"nextjs", "required_paths":["src/lib/store.ts","probe.mjs"], "verify_commands":[format!("node probe.mjs {mode}")]}).to_string()).unwrap();
        let mut config = config(root.path(), 1);
        config.completion_contract_path = Some(contract);
        config.offline = true;
        (root, config)
    }

    fn events(config: &Config) -> Vec<Value> {
        std::fs::read_to_string(config.eval_events_path.as_ref().unwrap())
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    fn audit(events: &[Value]) -> &Value {
        let events: Vec<_> = events
            .iter()
            .filter(|e| e["event"] == "recovery_preflight_control_audit")
            .collect();
        assert_eq!(events.len(), 1);
        events[0]
    }

    #[test]
    fn issue475_real_generic_preflight_reuses_all_control_and_restore_exits() {
        let mut cases: Vec<Value> = serde_json::from_str(include_str!(
            "../../../tests/corpus/apps/issue467-json-control/audit-cases.json"
        ))
        .unwrap();
        for mode in ["source", "config"] {
            cases.push(json!({"name":mode,"mode":mode,"script":"","outcome":"unavailable","reason":"preflight_source_mutation_rejected_and_restored","control":"unchanged","restore":false,"retained":true}));
        }
        for case in cases {
            if case["unix"] == true && !cfg!(unix) {
                continue;
            }
            let name = case["name"].as_str().unwrap();
            let (root, config) = workspace(case["mode"].as_str().unwrap());
            let probe = root.path().join("probe.mjs");
            let extra = format!(
                "\nconst control = {};\n{}\n",
                serde_json::to_string(root.path()).unwrap(),
                case["script"].as_str().unwrap()
            );
            std::fs::write(&probe, std::fs::read_to_string(&probe).unwrap() + &extra).unwrap();
            let before = current_source_sha256(root.path()).unwrap();
            let result = recovery_preflight(&config, &candidate(name), 0);
            match case["outcome"].as_str().unwrap() {
                "pass" => assert!(
                    matches!(result, RecoveryPreflight::CurrentSuccess { .. }),
                    "{name}: {result:?}"
                ),
                "fail" => assert!(
                    matches!(result, RecoveryPreflight::Failed { .. }),
                    "{name}: {result:?}"
                ),
                _ => assert!(
                    matches!(&result, RecoveryPreflight::Unavailable{reason} if reason.starts_with(case["reason"].as_str().unwrap())),
                    "{name}: {result:?}"
                ),
            }
            let events = events(&config);
            let audit = audit(&events);
            assert_eq!(audit["control_before_sha256"], before, "{name}");
            assert_eq!(audit["control_status"], case["control"], "{name}");
            assert_eq!(audit["restore_invoked"], case["restore"], "{name}");
            assert_eq!(audit["control_retained"], case["retained"], "{name}");
            assert_eq!(
                audit["restore_succeeded"],
                case["restore"] == true && case["retained"] == true,
                "{name}"
            );
            if case["restore"] == false {
                assert!(audit.get("control_after_restore_sha256").is_none());
                assert!(audit.get("restored_file_count").is_none());
            }
            if case["retained"] == true {
                assert_eq!(
                    current_source_sha256(root.path()).unwrap(),
                    before,
                    "{name}"
                );
            }
            if let Some(bytes) = case["control_bytes_after"].as_str() {
                assert_eq!(
                    std::fs::read_to_string(root.path().join("control.txt")).unwrap(),
                    bytes
                );
            }
            assert!(!root.path().join("data/projects.json").exists());
            println!(
                "ISSUE475_AUDIT {}",
                json!({"case":name,"control":audit["control_status"],"restore_invoked":audit["restore_invoked"],"restore_succeeded":audit["restore_succeeded"],"retained":audit["control_retained"]})
            );
        }
    }

    #[test]
    fn issue475_existing_and_missing_data_are_exact_outputs_without_business_promotion() {
        for existing in [false, true] {
            let (root, config) = workspace("business-failure");
            if existing {
                std::fs::create_dir(root.path().join("data")).unwrap();
                for p in ["projects", "tasks"] {
                    std::fs::write(root.path().join(format!("data/{p}.json")), "[]").unwrap();
                }
            }
            let before = source_file_sha256(root.path()).unwrap();
            let result = recovery_preflight(&config, &candidate("storage-only"), 0);
            assert!(
                matches!(result, RecoveryPreflight::Failed { .. }),
                "{result:?}"
            );
            assert_eq!(source_file_sha256(root.path()).unwrap(), before);
            let events = events(&config);
            let effect = events
                .iter()
                .find(|e| {
                    e["event"] == "recovery_preflight_effect_observation"
                        && e["stage"] == "registered_verification"
                })
                .unwrap();
            assert_eq!(
                effect["allowed_generated_paths"],
                json!(["data/projects.json", "data/tasks.json"])
            );
            let changes = effect["changes"].as_array().unwrap();
            assert_eq!(changes.len(), if existing { 1 } else { 2 });
            let observation = root
                .path()
                .join(".commandagent/recovery-observations/attempt-0/workspace");
            let after = source_file_sha256(&observation).unwrap();
            for change in changes {
                let path = change["path"].as_str().unwrap();
                assert_eq!(change["before_sha256"], json!(before.get(path)));
                assert_eq!(change["after_sha256"], after[path]);
                assert_eq!(change["allowed_generated"], true);
            }
            assert_eq!(audit(&events)["restore_invoked"], false);
        }
    }

    /// Build one observer case with the dynamic-port transport installed before
    /// the Recovery snapshot. `logical_port` is the goal/contract port; the
    /// transport never binds it.
    #[cfg(unix)]
    fn prepare_observer_case(
        existing: bool,
        permitted: bool,
        logical_port: u16,
    ) -> (tempfile::TempDir, Config) {
        let (root, mut config) = workspace("pass");
        config.offline = false;
        // Explicit test transport markers. No Next.js compiler is invoked; an
        // accidental next invocation fails instead of resolving a host toolchain.
        std::fs::create_dir_all(root.path().join("node_modules/next")).unwrap();
        std::fs::create_dir_all(root.path().join("node_modules/.bin")).unwrap();
        let next = root.path().join("node_modules/.bin/next");
        std::fs::write(&next, "#!/bin/sh\nexit 98\n").unwrap();
        issue475_set_executable(&next);
        if existing {
            std::fs::create_dir(root.path().join("data")).unwrap();
            for p in ["projects", "tasks"] {
                std::fs::write(root.path().join(format!("data/{p}.json")), "[]").unwrap();
            }
        }
        install_issue475_observer_transport(root.path(), logical_port);
        let contract = config.completion_contract_path.as_ref().unwrap();
        let mut value: Value = serde_json::from_slice(&std::fs::read(contract).unwrap()).unwrap();
        value["required_capabilities"] = json!(["stateful_interaction"]);
        if !permitted {
            value["protected_paths"] = json!(["data/projects.json"]);
            value["required_paths"]
                .as_array_mut()
                .unwrap()
                .push(json!("data/projects.json"));
        }
        value["goal"] = json!(format!("Observe the app on port {logical_port}"));
        // Keep later verification read-only so the capability stage owns the writes.
        value["verify_commands"] = json!(["node --check probe.mjs"]);
        std::fs::write(contract, value.to_string()).unwrap();
        (root, config)
    }

    #[cfg(unix)]
    fn observation_workspace(root: &Path) -> PathBuf {
        root.join(".commandagent/recovery-observations/attempt-0/workspace")
    }

    #[cfg(unix)]
    fn expect_business_unobserved(result: &RecoveryPreflight) {
        assert!(
            matches!(result, RecoveryPreflight::Failed{reason,..} if reason.contains("issue475_business_unobserved")),
            "{result:?}"
        );
    }

    #[test]
    #[cfg(unix)]
    fn issue475_product_nextjs_observer_correlates_operations_with_stage_and_hashes() {
        // Hold the logical goal port for the whole test: the dynamic transport must
        // succeed without ever binding it.
        let logical = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let logical_port = logical.local_addr().unwrap().port();
        for (existing, permitted) in [(false, true), (true, true), (false, false)] {
            let (root, config) = prepare_observer_case(existing, permitted, logical_port);
            let before = source_file_sha256(root.path()).unwrap();
            let result = recovery_preflight(&config, &candidate("observer-control"), 0);
            if permitted {
                expect_business_unobserved(&result);
            } else {
                assert!(
                    matches!(&result, RecoveryPreflight::Unavailable{reason} if reason == "preflight_source_mutation_rejected_and_restored"),
                    "{result:?}"
                );
            }
            assert_eq!(source_file_sha256(root.path()).unwrap(), before);
            let observation = observation_workspace(root.path());
            let actual_port = issue475_announced_port(&observation);
            assert_ne!(actual_port, logical_port, "transport bound the logical port");
            let readiness: Value = serde_json::from_slice(
                &std::fs::read(
                    crate::minimal_loop::browser_probe::browser_readiness_evidence_path(
                        &observation,
                    ),
                )
                .unwrap(),
            )
            .unwrap();
            assert_eq!(readiness["dev_server"]["child_spawned"], true);
            assert_eq!(readiness["dev_server"]["child_reaped"], true);
            let operations: Vec<Value> = std::fs::read_to_string(
                observation.join(".commandagent/evidence/issue475-operations.jsonl"),
            )
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
            for op in ["build:import-store", "start:import-store", "GET /"] {
                let event = operations.iter().find(|e| e["operation"] == op).unwrap();
                assert_eq!(event["before"], event["after"], "{op}");
            }
            let post = operations
                .iter()
                .find(|e| e["operation"] == "POST /api/projects")
                .unwrap();
            assert_ne!(
                post["before"]["data/projects.json"],
                post["after"]["data/projects.json"]
            );
            for p in ["projects", "tasks"] {
                let get = operations
                    .iter()
                    .find(|e| e["operation"] == format!("GET /api/{p}"))
                    .unwrap();
                assert_eq!(get["before"] == get["after"], existing);
            }
            let events = events(&config);
            let effects: Vec<_> = events
                .iter()
                .filter(|e| e["event"] == "recovery_preflight_effect_observation")
                .collect();
            let stage = effects
                .iter()
                .find(|e| e["stage"] == "nextjs_capabilities")
                .unwrap();
            let changes = stage["changes"].as_array().unwrap();
            assert_eq!(changes.len(), if existing { 1 } else { 2 });
            for change in changes {
                let path = change["path"].as_str().unwrap();
                assert_eq!(change["before_sha256"], json!(before.get(path)));
                assert_eq!(change["after_sha256"], post["after"][path]);
                assert_eq!(
                    change["allowed_generated"],
                    permitted || path == "data/tasks.json"
                );
            }
            assert_eq!(stage["first_write_within_stage"], "unknown");
            assert!(
                effects
                    .iter()
                    .filter(|e| e["stage"] == "registered_verification")
                    .all(|e| e["changes"] == json!([]))
            );
            assert_eq!(audit(&events)["control_retained"], true);
            assert_eq!(audit(&events)["restore_invoked"], false);
            println!(
                "ISSUE475_OBSERVER {}",
                json!({"existing":existing,"permitted":permitted,"actual_port":actual_port,"logical_port":logical_port,"transport":"scripted; no Next.js compilation or browser","operations":operations,"stage":stage,"historical_operation":"unknown"})
            );
        }
    }

    #[test]
    #[cfg(unix)]
    fn issue475_observer_transport_isolates_two_concurrent_processes() {
        // Two independent CommandAgent processes run the observer at once while the
        // logical goal port stays occupied. Each transport must bind its own actual
        // port, so the runs never contend for a freed logical number.
        let logical = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let logical_port = logical.local_addr().unwrap().port();
        let output = issue475_tempdir();
        let exe = std::env::current_exe().unwrap();
        let child_test =
            "planner::auto_recovery::tests::issue475::tests::issue475_observer_transport_child";
        let spawn = |tag: &str| {
            let path = output.path().join(format!("{tag}.json"));
            let child = std::process::Command::new(&exe)
                .args(["--ignored", "--exact", child_test, "--nocapture"])
                .env("COMMANDAGENT_ISSUE475_TRANSPORT_CHILD", "1")
                .env("COMMANDAGENT_ISSUE475_LOGICAL_PORT", logical_port.to_string())
                .env("COMMANDAGENT_ISSUE475_CHILD_OUTPUT", &path)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap();
            (child, path)
        };
        let (first, first_path) = spawn("first");
        let (second, second_path) = spawn("second");
        let first = first.wait_with_output().unwrap();
        let second = second.wait_with_output().unwrap();
        assert!(
            first.status.success(),
            "first observer child failed: {}",
            String::from_utf8_lossy(&first.stderr)
        );
        assert!(
            second.status.success(),
            "second observer child failed: {}",
            String::from_utf8_lossy(&second.stderr)
        );
        let first_port = child_announced_port(&first_path);
        let second_port = child_announced_port(&second_path);
        assert_ne!(
            first_port, second_port,
            "concurrent transports shared an actual port"
        );
        assert_ne!(first_port, logical_port);
        assert_ne!(second_port, logical_port);
    }

    #[cfg(unix)]
    fn child_announced_port(path: &Path) -> u16 {
        let value: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(value["ok"], true, "{value}");
        u16::try_from(value["port"].as_u64().unwrap()).unwrap()
    }

    #[test]
    #[ignore]
    #[cfg(unix)]
    fn issue475_observer_transport_child() {
        if std::env::var_os("COMMANDAGENT_ISSUE475_TRANSPORT_CHILD").is_none() {
            return;
        }
        let logical_port: u16 = std::env::var("COMMANDAGENT_ISSUE475_LOGICAL_PORT")
            .unwrap()
            .parse()
            .unwrap();
        let (root, config) = prepare_observer_case(true, true, logical_port);
        let result = recovery_preflight(&config, &candidate("observer-control"), 0);
        let actual_port = issue475_announced_port(&observation_workspace(root.path()));
        expect_business_unobserved(&result);
        std::fs::write(
            std::env::var("COMMANDAGENT_ISSUE475_CHILD_OUTPUT").unwrap(),
            json!({"ok": true, "port": actual_port}).to_string(),
        )
        .unwrap();
        println!(
            "ISSUE475_TRANSPORT_CHILD {}",
            json!({"logical_port": logical_port, "actual_port": actual_port})
        );
    }
}
