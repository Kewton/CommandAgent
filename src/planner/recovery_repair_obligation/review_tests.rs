use super::*;

#[test]
fn issue465_post_step_verification_cannot_lose_a_resolved_obligation() {
    let (_root, config) = setup_with(|config| {
        std::fs::write(config.workspace_root.join("checks/regress.js"),
            "import {readFileSync,writeFileSync} from 'node:fs';\nconst path='src/api.js';\nwriteFileSync(path,readFileSync(path,'utf8').replace('validateShift(input, policy)','validateShift(input)'));\n").unwrap();
    });
    let mut plan = plan();
    plan.steps[0].verify = vec!["node checks/regress.js".into()];
    let result = crate::planner::run_step_plan_with_ui(
        &mut Replay::new(vec![fix_api(), AssistantReply::text("Repaired")]),
        &plan,
        &config,
        &crate::tui::NOOP_UI,
    );
    assert!(result.is_err(), "{result:?}");
    let log = events(&config);
    let resolved = log.iter().position(|e| e["status"] == "resolved").unwrap();
    assert!(
        log[resolved + 1..]
            .iter()
            .any(|e| e["reason"] == "recovery_repair_unresolved")
    );
    assert!(
        std::fs::read_to_string(config.workspace_root.join(API))
            .unwrap()
            .contains("validateShift(input)")
    );
}

#[test]
fn issue465_runner_existing_verifier_and_path_precheck_preserve_repair_obligation() {
    for repaired in [false, true] {
        let (_root, config) = setup();
        if repaired {
            let path = config.workspace_root.join(API);
            let text = std::fs::read_to_string(&path).unwrap();
            std::fs::write(
                path,
                text.replace("validateShift(input)", "validateShift(input, policy)"),
            )
            .unwrap();
        }
        let mut verifier = step("path-precheck", API);
        verifier.kind = "verify".into();
        verifier.verify = vec!["test -f src/api.js".into()];
        let plan = StepPlan {
            goal: "Confirm API repair".into(),
            steps: vec![verifier],
        };
        let result = crate::planner::run_step_plan_with_ui(
            &mut Replay::default(),
            &plan,
            &config,
            &crate::tui::NOOP_UI,
        );
        assert_eq!(result.is_ok(), repaired, "{result:?}");
        let log = events(&config);
        assert_eq!(
            log.iter().any(|e| e["event"] == "step_short_circuited"),
            repaired
        );
    }
}

#[test]
fn issue465_distinct_cats_do_not_resolve_but_then_correct_edit_proceeds() {
    let (_root, config) = setup();
    let result = run(
        &config,
        options(&config, &plan(), 0),
        vec![
            failed_edit(),
            cat("cat src/api.js"),
            cat("cat ./src/api.js"),
            cat("cat src/store.js src/types.js"),
            fix_store(),
            AssistantReply::text("Repaired"),
        ],
    );
    assert!(result.is_ok(), "{result:?}");
    let log = events(&config);
    let resolved = log.iter().position(|e| e["status"] == "resolved").unwrap();
    assert!(
        log[..resolved]
            .iter()
            .filter(|e| e["reason"] == "recovery_repair_unresolved")
            .count()
            >= 3
    );
}

pub(super) fn edit_contract(config: &Config, change: impl FnOnce(&mut serde_json::Value)) {
    let path = config.completion_contract_path.as_ref().unwrap();
    let mut value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    change(&mut value);
    std::fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap();
}

#[test]
fn issue465_missing_verifier_producer_has_a_finite_runner_confirmation_boundary() {
    for fix in [true, false] {
        let (_root, config) = setup_with(|config| {
            edit_contract(config, |contract| {
                contract["required_paths"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!("checks/smoke.js"));
                contract["verify_commands"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!("node checks/smoke.js"));
            })
        });
        let mut plan = plan();
        plan.steps.push(step("produce-smoke", "checks/smoke.js"));
        plan.steps.push(PlanStep {
            id: "confirm-group".into(),
            kind: "verify".into(),
            expected_result: "pass".into(),
            instruction: "Run all registered checks".into(),
            expected_paths: vec![API.into(), "checks/smoke.js".into()],
            verify: vec![
                "node checks/target.js".into(),
                "node checks/smoke.js".into(),
            ],
        });
        let first = options(&config, &plan, 0);
        assert_eq!(
            first.recovery_obligation.as_ref().unwrap().boundary,
            "confirm-group"
        );
        let repair = if fix {
            fix_api()
        } else {
            tool(
                "Edit",
                json!({"path":API,"old_string":"POST(input)","new_string":"POST(input = {})"}),
            )
        };
        let mut replay = Replay::new(vec![
            repair,
            tool(
                "Write",
                json!({"path":"checks/smoke.js","content":"console.log('smoke')"}),
            ),
            AssistantReply::text("Finished"),
        ]);
        let result = crate::planner::run_step_plan_with_ui(
            &mut replay,
            &plan,
            &config,
            &crate::tui::NOOP_UI,
        );
        assert_eq!(result.is_ok(), fix, "{result:?}");
        let log = events(&config);
        assert!(
            log.iter()
                .any(|e| e["status"] == "pending" && e["confirmation_boundary"] == "confirm-group")
        );
        assert_eq!(log.iter().any(|e| e["status"] == "resolved"), fix);
        assert!(config.workspace_root.join("checks/smoke.js").is_file());
    }
}

#[test]
fn issue465_missing_verifier_without_confirmation_boundary_is_rejected() {
    let (_root, config) = setup_with(|config| {
        edit_contract(config, |contract| {
            contract["required_paths"]
                .as_array_mut()
                .unwrap()
                .push(json!("checks/smoke.js"));
            contract["verify_commands"]
                .as_array_mut()
                .unwrap()
                .push(json!("node checks/smoke.js"));
        })
    });
    let mut plan = plan();
    plan.steps.push(step("produce-smoke", "checks/smoke.js"));
    let mut opts = RunSessionOptions::plan_step(RunSessionStepKind::Implement);
    assert!(configure(&config, &plan, &plan.steps[0], &mut opts).is_err());
}

#[test]
fn issue465_owner_alias_and_omitted_paths_cannot_drop_the_obligation() {
    for paths in [
        vec!["./src/api.js".into()],
        vec![],
        vec!["unrelated.txt".into()],
    ] {
        let (_root, config) = setup();
        let mut plan = plan();
        plan.steps[0].expected_paths = paths;
        let opts = options(&config, &plan, 0);
        assert!(opts.recovery_obligation.is_some());
        let result = run(
            &config,
            opts,
            vec![cat("cat src/api.js"), AssistantReply::text("Completed")],
        );
        assert!(result.is_err(), "{result:?}");
    }
}

#[test]
fn issue465_fresh_confirmation_reads_generated_json_even_with_unchanged_source_hash() {
    let (_root, config) = setup_with(|config| {
        std::fs::create_dir_all(config.workspace_root.join("evidence")).unwrap();
        std::fs::write(
            config.workspace_root.join("evidence/input.json"),
            "{\"hours\":8}",
        )
        .unwrap();
        std::fs::write(config.workspace_root.join("checks/input.js"), "import {readFileSync} from 'node:fs'; import assert from 'node:assert/strict'; import {POST} from '../src/api.js'; assert.equal(POST(JSON.parse(readFileSync('evidence/input.json','utf8'))), true);").unwrap();
        edit_contract(config, |contract| {
            contract["verify_commands"]
                .as_array_mut()
                .unwrap()
                .push(json!("node checks/input.js"))
        });
    });
    let source = std::fs::read_to_string(config.workspace_root.join(API)).unwrap();
    std::fs::write(
        config.workspace_root.join(API),
        source.replace("validateShift(input)", "validateShift(input, policy)"),
    )
    .unwrap();
    let opts = options(&config, &plan(), 0);
    let obligation = opts.recovery_obligation.as_ref().unwrap();
    assert!(obligation.feedback(&config, &opts).is_none());
    let hash = obligation.source_hash(&config).unwrap();
    std::fs::write(
        config.workspace_root.join("evidence/input.json"),
        "{\"hours\":99}",
    )
    .unwrap();
    assert_eq!(hash, obligation.source_hash(&config).unwrap());
    assert!(obligation.feedback(&config, &opts).is_some());
}

#[test]
fn issue465_preserves_target_calls_without_rejecting_aliases_and_function_cleanup() {
    let (_root, config) = setup_with(|config| {
        let path = config.workspace_root.join(STORE);
        let source = std::fs::read_to_string(&path).unwrap();
        std::fs::write(
            path,
            format!("{source}\nexport function unusedHelper() {{ return Math.max(1, 2); }}"),
        )
        .unwrap();
    });
    let result = run(
        &config,
        options(&config, &plan(), 0),
        vec![
            tool(
                "Write",
                json!({"path":STORE,"content":"import { policy as defaultPolicy } from './types.js';\nexport function validateShift(input, policy = defaultPolicy) {\n  return input.hours > 0 && input.hours <= policy.max;\n}\n"}),
            ),
            AssistantReply::text("Fixed related store"),
        ],
    );
    assert!(result.is_ok(), "{result:?}");
    let (_root, config) = setup();
    let source = std::fs::read_to_string(config.workspace_root.join(API)).unwrap();
    let source = source
        .replace("{ validateShift }", "{ validateShift as check }")
        .replace("validateShift(input)", "check(input, policy)");
    let result = run(
        &config,
        options(&config, &plan(), 0),
        vec![
            tool("Write", json!({"path":API,"content":source})),
            AssistantReply::text("Fixed alias"),
        ],
    );
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn issue465_const_assertion_is_allowed_while_asserted_types_and_exclusions_are_not() {
    let (_root, config) = setup_with(|config| {
        std::fs::write(
            config.workspace_root.join("tsconfig.json"),
            "{\"compilerOptions\":{\"strict\":true},\"include\":[\"src/**/*\"]}",
        )
        .unwrap();
    });
    let obligation = load(&config).unwrap().unwrap();
    let path = config.workspace_root.join("src/types.js");
    // This checks preservation only; the TypeScript replay below also executes
    // the compiler. JavaScript is never claimed to execute a TS assertion.
    for (source, accepted) in [
        ("export const policy = { max: 8 } as const;", true),
        ("export const policy = { max: 8 } as any;", false),
        (
            "export const policy = { max: 8 } as unknown as Policy;",
            false,
        ),
    ] {
        std::fs::write(&path, source).unwrap();
        assert_eq!(preservation::verify(&config, &obligation).is_ok(), accepted);
    }
    std::fs::write(&path, "export const policy = { max: 8 };").unwrap();
    std::fs::write(
        config.workspace_root.join("tsconfig.json"),
        "{\"exclude\":[\"src/api.js\"]}",
    )
    .unwrap();
    assert!(preservation::verify(&config, &obligation).is_err());
}

#[test]
fn issue465_artifact_only_confirmation_and_changed_contract_are_not_success() {
    let (_root, config) = setup_with(|config| {
        edit_contract(config, |contract| {
            contract["verify_commands"] = json!(["test -f src/api.js"])
        })
    });
    let opts = options(&config, &plan(), 0);
    assert!(
        opts.recovery_obligation
            .as_ref()
            .unwrap()
            .feedback(&config, &opts)
            .is_some()
    );
    edit_contract(&config, |contract| {
        contract["verify_commands"] = json!(["true"])
    });
    assert!(
        opts.recovery_obligation
            .as_ref()
            .unwrap()
            .feedback(&config, &opts)
            .unwrap()
            .contains("contract changed")
    );
}
