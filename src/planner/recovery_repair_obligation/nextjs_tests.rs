use super::*;

#[test]
#[cfg(unix)]
#[ignore = "requires installed Next.js/React/TypeScript; required Issue #465 verification"]
fn issue465_nextjs_new_ignore_build_errors_config_cannot_discharge_the_obligation() {
    let root = tempfile::tempdir().unwrap();
    let modules = std::env::var_os("ISSUE465_NEXT_NODE_MODULES").expect(
        "set ISSUE465_NEXT_NODE_MODULES to installed Next.js/React/TypeScript dependencies",
    );
    std::os::unix::fs::symlink(modules, root.path().join("node_modules")).unwrap();
    for path in [
        "src/api.ts",
        "src/store.ts",
        "src/types.ts",
        "tsconfig.json",
    ] {
        let target = root.path().join(path);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::copy(Path::new(FIXTURE).join("typescript").join(path), target).unwrap();
    }
    std::fs::create_dir_all(root.path().join("src/app")).unwrap();
    std::fs::write(
        root.path().join("src/app/page.tsx"),
        "export default function Page() { return <main>Shift API</main>; }\n",
    )
    .unwrap();
    std::fs::write(root.path().join("src/app/layout.tsx"), "export default function Layout({children}: {children: React.ReactNode}) { return <html><body>{children}</body></html>; }\n").unwrap();
    std::fs::write(root.path().join("package.json"), r#"{"name":"issue465-next","private":true,"dependencies":{"next":"15.5.20","react":"19.1.0","react-dom":"19.1.0"},"devDependencies":{"typescript":"5.9.3","@types/react":"19.1.0","@types/node":"22.0.0"}}"#).unwrap();
    let mut config =
        Config::from_cli(crate::cli::Cli::parse_from(["commandagent", "--ux-demo"])).unwrap();
    config.workspace_root = root.path().to_path_buf();
    config.profile = "generic".into();
    config.profile_explicit = true;
    config.offline = true;
    config.yes = true;
    config.max_iterations = 4;
    config.eval_events_path = Some(root.path().join(".commandagent/events.jsonl"));
    config.completion_contract_path = Some(root.path().join("contract.json"));
    std::fs::write(
        root.path().join("contract.json"),
        json!({
            "goal":"Repair shift API arity", "required_paths":["src/api.ts"], "profile":"generic",
            "verify_commands":["NEXT_TELEMETRY_DISABLED=1 CIRCLE_NODE_TOTAL=2 next build"]
        })
        .to_string(),
    )
    .unwrap();
    let contract = CompletionContract::load_for_config(&config)
        .unwrap()
        .unwrap();
    let before = contract.verify(root.path());
    let evidence: Vec<_> = before
        .command_failures
        .iter()
        .map(|f| f.reason.clone())
        .collect();
    assert!(!before.is_pass());
    assert!(
        evidence
            .join("\n")
            .contains("Expected 2 arguments, but got 1"),
        "{before:?}"
    );
    assert!(!root.path().join("next.config.js").exists());
    let original_api = std::fs::read(root.path().join("src/api.ts")).unwrap();
    crate::planner::recovery_inspection::bind_context(
        &config,
        &config,
        Some("create"),
        &RecoveryHandoff {
            failure_kind: "compile_error".into(),
            failure_evidence: evidence,
            repair_targets: vec!["src/api.ts".into()],
            ..RecoveryHandoff::default()
        },
        None,
    )
    .unwrap();
    let plan = StepPlan {
        goal: "Repair shift API arity".into(),
        steps: vec![step("repair-api", "src/api.ts")],
    };
    let result = crate::planner::run_step_plan_with_ui(
        &mut Replay::new(vec![
            tool(
                "Write",
                json!({"path":"next.config.js","content":"module.exports = { typescript: { ignoreBuildErrors: true } };"}),
            ),
            AssistantReply::text("Build passes"),
        ]),
        &plan,
        &config,
        &crate::tui::NOOP_UI,
    );
    assert!(result.is_err(), "{result:?}");
    assert_eq!(
        std::fs::read(root.path().join("src/api.ts")).unwrap(),
        original_api
    );
    let after = contract.verify(root.path());
    assert!(after.is_pass(), "{after:?}");
    let log = events(&config);
    assert!(!log.iter().any(|e| e["status"] == "resolved"));
    assert!(log.iter().any(|e| {
        e["failure_reason"]
            .as_str()
            .is_some_and(|r| r.contains("configuration added: next.config.js"))
    }));
    eprintln!(
        "ISSUE465_NEXT_CONFIG original_arity_failure=true bypass_build_passed=true api_unchanged=true runner_completed=false"
    );
    let result = crate::planner::run_step_plan_with_ui(
        &mut Replay::new(vec![
            cat("python3 -c 'from pathlib import Path; Path(\"next.config.js\").unlink()'"),
            tool(
                "Edit",
                json!({"path":"src/store.ts", "old_string":"policy: Policy)", "new_string":"policy: Policy = defaultPolicy)"}),
            ),
            AssistantReply::text("Restored original conditions and repaired related store"),
        ]),
        &plan,
        &config,
        &crate::tui::NOOP_UI,
    );
    assert!(result.is_ok(), "{result:?}");
    assert!(!root.path().join("next.config.js").exists());
    assert_eq!(
        std::fs::read(root.path().join("src/api.ts")).unwrap(),
        original_api
    );
    assert!(events(&config).iter().any(|e| e["status"] == "resolved"));
    eprintln!(
        "ISSUE465_NEXT_CONFIG original_conditions_restored=true related_store_repaired=true runner_completed=true"
    );
}
