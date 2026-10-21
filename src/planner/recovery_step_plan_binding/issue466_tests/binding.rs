use super::*;

#[test]
fn issue466_closed_owner_existence_corpus_matrix_is_transactional() {
    let cases: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(Path::new(FIXTURE).join("cases.json")).unwrap(),
    )
    .unwrap();
    for case in cases.as_array().unwrap() {
        let paths: Vec<_> = case["paths"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p.as_str().unwrap())
            .collect();
        let existing: Vec<_> = case["existing"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p.as_str().unwrap())
            .collect();
        let (_root, c, _guard) = registered(&paths, &existing);
        let owners: Vec<PlanStep> = serde_json::from_value(case["owners"].clone()).unwrap();
        let mut p = plan(owners);
        let before = p.clone();
        let result = bind_generated(&c, Some("repair-artifacts"), &mut p);
        if case["expect"] == "bound" {
            assert!(result.is_ok(), "{case}: {result:?}");
            let last = p.steps.last().unwrap();
            assert_eq!(last.kind, "verify");
            for path in &paths {
                assert!(last.expected_paths.iter().any(|p| p == path));
                assert!(last.verify.contains(&format!("node {path}")));
            }
        } else {
            let error = result.unwrap_err();
            let class = error.downcast_ref::<BindingFailure>().unwrap().class;
            assert_eq!(
                serde_json::to_value(class).unwrap(),
                case["expect"],
                "{case}: {error}"
            );
            assert_eq!(p, before, "failure half-mutated {case}");
        }
    }
}

#[test]
fn issue466_closed_e_context_preserves_partial_existence_and_rejects_without_retry() {
    let historical: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(Path::new(FIXTURE).join("historical-closed-obligations.json"))
            .unwrap(),
    )
    .unwrap();
    for case in historical.as_array().unwrap() {
        let label = case["run_id"].as_str().unwrap();
        let root = tempfile::tempdir().unwrap();
        let mut c = config(root.path());
        let path = generated_contract(&c);
        c.completion_contract_path = Some(path.clone());
        std::fs::write(root.path().join("package.json"), "{}").unwrap();
        let mut context: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(Path::new(FIXTURE).join("closed-e-context.json")).unwrap(),
        )
        .unwrap();
        context["verifier_steps"] = case["verifier_steps"].clone();
        use sha2::{Digest, Sha256};
        context["contract_sha256"] = json!(format!(
            "{:x}",
            Sha256::digest(std::fs::read(path).unwrap())
        ));
        let runtime = root.path().join(".commandagent/recovery-runtime");
        std::fs::create_dir_all(&runtime).unwrap();
        std::fs::write(
            runtime.join("inspection-context.json"),
            serde_json::to_vec(&context).unwrap(),
        )
        .unwrap();
        let original = std::fs::read(runtime.join("inspection-context.json")).unwrap();
        let candidate = plan(vec![step("proposed", "implement", &[SCRIPT])]);
        let mut planner = Replay::new(vec![proposal(&candidate); 3]);
        let error = generate(&c, &mut planner).unwrap_err();
        assert!(
            error.to_string().contains("partially exists"),
            "{label}: {error}"
        );
        assert_eq!(planner.requests.lock().unwrap().len(), 1);
        assert_eq!(
            std::fs::read(runtime.join("inspection-context.json")).unwrap(),
            original
        );
        assert!(!root.path().join(SCRIPT).exists());
        // Removing the already-present part of an old closed group cannot
        // turn unavailable historical provenance into new creation authority.
        context["verifier_steps"][0]["expected_paths"] = json!([SCRIPT]);
        std::fs::write(
            runtime.join("inspection-context.json"),
            serde_json::to_vec(&context).unwrap(),
        )
        .unwrap();
        let mut planner = Replay::new(vec![proposal(&candidate)]);
        let error = generate(&c, &mut planner).unwrap_err();
        assert!(
            error.to_string().contains("unsealed legacy"),
            "{label}: {error}"
        );
        assert!(!root.path().join(SCRIPT).exists());
    }
}

#[test]
fn issue466_protected_configured_paths_and_redirects_grant_no_creation_authority() {
    for protected in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let mut c = config(root.path());
        let _guard = authority::begin_run(&c);
        let path = generated_contract(&c);
        if protected {
            let mut contract: serde_json::Value =
                serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
            contract["protected_paths"] = json!([SCRIPT]);
            std::fs::write(&path, serde_json::to_vec(&contract).unwrap()).unwrap();
            assert!(
                scope::register(&c, &plan(vec![step("source", "implement", &[SCRIPT])])).is_err()
            );
        }
        c.completion_contract_path = Some(path);
        // Configured contracts never acquire a missing producer from a proposal.
        scope::register(&c, &plan(vec![step("source", "implement", &[SCRIPT])])).unwrap();
        bind_context(&c);
        let mut p = plan(vec![step("unregistered", "implement", &[SCRIPT])]);
        let err = bind_generated(&c, Some("repair"), &mut p).unwrap_err();
        assert_eq!(
            err.downcast_ref::<BindingFailure>().unwrap().class,
            FailureClass::Unsafe
        );
    }
    #[cfg(unix)]
    {
        let (root, c, _guard) = registered(&[SCRIPT], &[]);
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("check.js"), "// check").unwrap();
        std::os::unix::fs::symlink(outside.path().join("check.js"), root.path().join(SCRIPT))
            .unwrap();
        let mut p = plan(vec![step("owner", "implement", &[SCRIPT])]);
        assert!(bind_generated(&c, Some("repair"), &mut p).is_err());
    }
}

#[test]
fn issue466_direct_script_parser_handles_quotes_without_substring_or_option_authority() {
    for (command, expected) in [
        (
            "node './checks/strict check.cjs'",
            Some("checks/strict check.cjs"),
        ),
        ("python3 ./check.py", Some("check.py")),
        ("cd app && node ./checks/smoke.js", None),
        ("node -p \"'smoke-check.js'\"", None),
        ("node ../outside.js", None),
        ("node package.json", None),
    ] {
        assert_eq!(
            crate::planner::recovery_contract_authority::local_script_path(command).as_deref(),
            expected
        );
    }
}
