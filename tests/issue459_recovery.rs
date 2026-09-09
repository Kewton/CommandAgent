//! Installed-toolchain replay uses the normally compiled library: no cfg(test)
//! browser/command overrides, synthetic events or injected execution outcomes.
#[path = "support/issue459_replay.rs"]
mod replay;

use clap::Parser;
use commandagent::config::Config;
use commandagent::planner::ultra_plan::{UltraPhase, UltraPlan};
use replay::*;
use serde_json::json;
use std::path::{Path, PathBuf};

#[test]
fn issue459_corpus_inputs_are_hash_bound() {
    let provenance = read_json(fixture().join("provenance.json"));
    for (relative, expected) in provenance["inputs_sha256"].as_object().unwrap() {
        assert_eq!(
            hash(&Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)),
            expected.as_str().unwrap(),
            "{relative}"
        );
    }
    let cases = read_json(fixture().join("cases.json"));
    assert_eq!(cases.as_array().unwrap().len(), 7);
    assert_eq!(cases[0]["diagnostics"], 33);
    assert_eq!(cases[1]["diagnostics"], 32);
}

#[test]
#[ignore = "requires frozen Node dependencies, real Chromium and a fresh ISSUE459_WORK_ROOT"]
fn issue459_installed_nextjs_create_recovery() {
    let root =
        PathBuf::from(std::env::var("ISSUE459_WORK_ROOT").expect("fresh ISSUE459_WORK_ROOT"));
    assert!(!root.exists(), "Never overwrite an existing run");
    std::fs::create_dir_all(&root).unwrap();
    let scenario =
        std::env::var("ISSUE459_SCENARIO").unwrap_or_else(|_| "aligned-after-no-edit".into());
    let cases = read_json(fixture().join("replay-cases.json"));
    let expected = cases
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["name"] == scenario)
        .expect("registered replay scenario");
    let control = root.join("control");
    std::fs::create_dir_all(control.join(".commandagent")).unwrap();
    install_links(&control);
    // Resume the exact generated create artifact, before its terminal verification.
    copy_tree(&original(), &control);
    std::fs::copy(
        contracts().join("package-lock.json"),
        control.join("package-lock.json"),
    )
    .unwrap();
    std::fs::write(
        control.join("next.config.js"),
        "module.exports = { experimental: { cpus: 2 } };\n",
    )
    .unwrap();
    let original_contract = read_json(original().parent().unwrap().join("nextjs-contract.json"));
    let mut contract = original_contract.clone();
    let business_oracle = matches!(
        scenario.as_str(),
        "missing-list"
            | "wrong-field"
            | "oracle-unexecuted"
            | "aligned-with-oracle"
            | "promoted-with-oracle"
    );
    if business_oracle {
        std::fs::create_dir_all(control.join("checks")).unwrap();
        std::fs::copy(
            "scripts/issue459_assignment_oracle.mjs",
            control.join("checks/assignment.test.mjs"),
        )
        .unwrap();
        if scenario != "oracle-unexecuted" {
            std::fs::copy(
                "scripts/issue457_nextjs_interaction.mjs",
                control.join("checks/interaction.mjs"),
            )
            .unwrap();
        }
        contract["verify_commands"].as_array_mut().unwrap().extend([
            json!("./node_modules/.bin/tsc --noEmit --incremental false --pretty false"),
            json!("node --test checks/assignment.test.mjs"),
        ]);
        let check_paths = if scenario == "oracle-unexecuted" {
            vec![json!("checks/assignment.test.mjs")]
        } else {
            vec![
                json!("checks/assignment.test.mjs"),
                json!("checks/interaction.mjs"),
            ]
        };
        contract["required_paths"]
            .as_array_mut()
            .unwrap()
            .extend(check_paths.clone());
        contract["protected_paths"]
            .as_array_mut()
            .unwrap()
            .extend(check_paths);
    }
    std::fs::write(
        control.join(".commandagent/contract.json"),
        serde_json::to_vec_pretty(&contract).unwrap(),
    )
    .unwrap();
    let mut config = Config::from_cli(commandagent::cli::Cli::parse_from([
        "commandagent",
        "--ux-demo",
    ]))
    .unwrap();
    config.workspace_root = control.clone();
    config.eval_events_path = Some(control.join(".commandagent/events.jsonl"));
    config.completion_contract_path = Some(control.join(".commandagent/contract.json"));
    config.profile = "nextjs".into();
    config.profile_explicit = true;
    config.offline = false;
    config.yes = true;
    config.max_iterations = 8;
    config.recovery_plan_auto_runs = 2;
    let before = source_hashes(&control);
    let state = ReplayState::new(&control, &scenario);
    let mut planner = Replay::planner(state.clone());
    let mut execution = Replay::execution(state.clone());
    let plan = UltraPlan {
        goal: contract["goal"].as_str().unwrap().into(),
        profile: "nextjs".into(),
        style: "standard".into(),
        intent: "create".into(),
        phases: vec![
            UltraPhase {
                id: "create-artifact".into(),
                prompt:
                    "Inspect the generated project manager and complete its build verification."
                        .into(),
            },
            UltraPhase {
                id: "verify-create".into(),
                prompt: "Run npm run build and verify the project manager.".into(),
            },
        ],
    };
    let result = commandagent::planner::auto_recovery::run_plan_with_ui(
        &mut planner,
        &mut execution,
        &plan,
        &config,
        &commandagent::tui::NOOP_UI,
    );
    let events = events(&control);
    let after = source_hashes(&control);
    let state = state.lock().unwrap();
    let starts = events
        .iter()
        .filter(|e| e["event"] == "recovery_plan_auto_run_start")
        .count();
    let candidates = (1..=starts).map(|attempt| {
        let treatment = control.join(format!(".commandagent/recovery-treatments/attempt-{attempt}/workspace"));
        let oracle = treatment.join("evidence/issue459-assignment.json");
        json!({"attempt":attempt,"source_sha256":source_hashes(&treatment),"oracle":oracle.exists().then(||read_json(oracle))})
    }).collect::<Vec<_>>();
    let report = json!({
        "scenario":scenario, "success":result.is_ok(), "result":format!("{result:?}"),
        "contract":contract, "original_contract":original_contract,
        "contract_mode":if business_oracle {"explicit fixture supplement: strict types + assignment oracle; original requirements retained"} else {"original"},
        "control_before":before, "control_after":after,
        "requests":state.requests, "bound_inspections":state.inspections,
        "candidates":candidates,
        "starts":events.iter().filter(|e|e["event"]=="recovery_plan_auto_run_start").collect::<Vec<_>>(),
        "decisions":events.iter().filter(|e|e["event"].as_str().is_some_and(|s|s.starts_with("recovery_") || s.contains("acceptance"))).collect::<Vec<_>>(),
    });
    std::fs::write(
        root.join("result.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    eprintln!(
        "Issue #459 result: {result:?}; evidence: {}",
        root.display()
    );
    assert!(
        events
            .iter()
            .any(|e| e["event"] == "recovery_plan_auto_run_start"),
        "{result:?}"
    );
    assert!(
        events
            .iter()
            .any(|e| e["event"] == "ultra_phase_execute_complete"
                && e["phase_id"] == "inspect-current-state")
    );
    assert!(events.iter().any(|e| {
        e["event"] == "ultra_phase_start"
            && e["phase_id"]
                .as_str()
                .is_some_and(|p| p.starts_with("repair-"))
    }));
    let promoted = expected["promoted"].as_bool().unwrap();
    assert_eq!(result.is_ok(), promoted, "{result:?}");
    if promoted {
        assert_ne!(before, after);
        assert_eq!(
            serde_json::to_value(&after).unwrap(),
            candidates.last().unwrap()["source_sha256"]
        );
    } else {
        assert_eq!(
            before, after,
            "rejection must preserve all control source/data bytes"
        );
    }
    assert_eq!(starts as u64, expected["starts"].as_u64().unwrap());
    assert_eq!(
        events.last().unwrap()["recovery_plan_auto_run_current"],
        expected["starts"]
    );
    assert_eq!(
        events.last().unwrap()["recovery_plan_auto_run_stop_reason"],
        expected["stop"]
    );
    let decisions = events
        .iter()
        .filter(|e| e["event"] == "recovery_promotion_decision")
        .collect::<Vec<_>>();
    assert_eq!(decisions.len(), starts);
    let start_indices = events
        .iter()
        .enumerate()
        .filter(|(_, e)| e["event"] == "recovery_plan_auto_run_start")
        .map(|(i, _)| i)
        .collect::<Vec<_>>();
    for pair in start_indices.windows(2) {
        let between = &events[pair[0]..pair[1]];
        let rejected = between
            .iter()
            .position(|e| {
                e["event"] == "recovery_promotion_decision" && e["decision"] == "rejected"
            })
            .unwrap();
        let continuation = between
            .iter()
            .position(|e| e["event"] == "recovery_continuation_prepared")
            .unwrap();
        assert!(
            rejected < continuation,
            "retain control before publishing a continuation"
        );
        let path = between[continuation]["recovery_ultra_plan_path"]
            .as_str()
            .unwrap();
        assert!(path.starts_with(".commandagent/plans/") && !path.contains("recovery-treatments"));
        assert!(control.join(path).is_file());
    }
    assert!(
        decisions[..decisions.len() - 1]
            .iter()
            .all(|e| e["decision"] == "rejected")
    );
    assert_eq!(
        decisions.last().unwrap()["decision"],
        if promoted { "promoted" } else { "rejected" }
    );
    assert_eq!(
        decisions.last().unwrap()["reason"],
        expected["finish_reason"]
    );
    assert_eq!(
        events
            .iter()
            .any(|e| e["event"] == "ultra_final_acceptance" && e["primary_reason"] == "pass"),
        expected["runtime_acceptance"].as_bool().unwrap()
    );
    assert_eq!(
        events
            .iter()
            .any(|e| e["event"] == "tool_execute" && e["name"] == "Edit" && e["status"] == "ok"),
        expected["edits"].as_bool().unwrap()
    );
    if let Some(status) = expected["oracle"].as_str() {
        assert_eq!(candidates.last().unwrap()["oracle"]["status"], status);
        for (path, source_hash) in candidates.last().unwrap()["oracle"]["source_sha256"]
            .as_object()
            .unwrap()
        {
            assert_eq!(
                &candidates.last().unwrap()["source_sha256"][path],
                source_hash,
                "oracle must observe the actual edited candidate: {path}"
            );
        }
        assert!(events.iter().any(|e| e["event"]
            == if status == "passed" {
                "recovery_host_final_success_verification_passed"
            } else {
                "recovery_host_final_success_verification_failed"
            }));
    } else {
        assert!(candidates.iter().all(|c| c["oracle"].is_null()));
    }
    if scenario == "oracle-unexecuted" {
        assert!(format!("{result:?}").contains("node --test checks/assignment.test.mjs"));
        assert!(
            events
                .iter()
                .any(|e| e["event"] == "recovery_host_final_success_verification_failed")
        );
    }
    if expected["edits"] == true {
        let matrix = read_json(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("dev-reports/issue-459/fixture-results.json"),
        );
        let variant = if promoted {
            "aligned-observable"
        } else if matches!(scenario.as_str(), "missing-list" | "wrong-field") {
            scenario.as_str()
        } else {
            "aligned"
        };
        let measured = matrix["results"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["case"] == variant)
            .unwrap();
        for (path, source_hash) in measured["source_sha256"].as_object().unwrap() {
            assert_eq!(
                &candidates.last().unwrap()["source_sha256"][path],
                source_hash,
                "real Edit result must match independently measured {variant}: {path}"
            );
        }
    }
    for inspection in &state.inspections {
        let instruction = inspection["instruction"].as_str().unwrap();
        assert!(instruction.contains(original_contract["goal"].as_str().unwrap()));
        assert!(instruction.contains("role?: string"));
        for required in [
            "src/lib/store.ts",
            "src/lib/types.ts",
            "src/app/api/projects/[id]/tasks/route.ts",
        ] {
            assert!(instruction.contains(required));
        }
    }
    for attempt in 1..=starts {
        let messages = state
            .requests
            .iter()
            .filter(|r| {
                r["attempt"] == attempt
                    && r["phase"] == "inspect-current-state"
                    && r["planner"] == false
            })
            .flat_map(|r| r["messages"].as_array().unwrap())
            .collect::<Vec<_>>();
        let instructions = messages
            .iter()
            .filter(|m| m["role"] == "user")
            .map(|m| m["content"].as_str().unwrap())
            .collect::<Vec<_>>()
            .join("\n");
        for retained in [
            original_contract["goal"].as_str().unwrap(),
            "role?: string",
            "src/lib/store.ts",
            "src/lib/types.ts",
            "src/app/api/projects/[id]/tasks/route.ts",
        ] {
            assert!(
                instructions.contains(retained),
                "attempt {attempt}: missing from actual bound provider prompts: {retained}"
            );
        }
        let tool_results = messages
            .iter()
            .filter(|m| m["role"] == "tool")
            .map(|m| m["content"].as_str().unwrap())
            .collect::<Vec<_>>()
            .join("\n");
        for definition in [
            "export function createTask",
            "export function getTasks",
            "role: string",
            "role?: string",
            "assignee?: Member",
            "error?: string",
            "data: Task[] | null",
        ] {
            assert!(
                tool_results.contains(definition),
                "attempt {attempt}: missing actual inspection Read result: {definition}"
            );
        }
    }
    let mut inspecting = false;
    for event in &events {
        if event["event"] == "ultra_phase_start" {
            inspecting = event["phase_id"] == "inspect-current-state";
        }
        if inspecting {
            assert_ne!(
                event["event"], "dependency_build_lifecycle",
                "build leaked into inspection"
            );
            if event["event"] == "tool_execute" {
                assert!(
                    !["Edit", "Write", "Bash"].contains(&event["name"].as_str().unwrap()),
                    "mutation/build leaked into inspection"
                );
            }
        }
        if event["event"] == "ultra_phase_execute_complete" {
            inspecting = false;
        }
    }
    assert!(
        events
            .iter()
            .any(|e| e["event"] == "compile_error_extraction" && e["compile_error_count"] == 33)
    );
    if matches!(
        scenario.as_str(),
        "aligned-after-no-edit" | "promoted-after-no-edit"
    ) {
        assert_eq!(state.inspections.len(), 2);
        assert!(
            events
                .iter()
                .any(|e| e["event"] == "recovery_continuation_prepared")
        );
        assert!(
            events
                .iter()
                .any(|e| e["event"] == "ultra_final_acceptance" && e["primary_reason"] == "pass")
        );
        let first_delta = events
            .iter()
            .find(|e| e["event"] == "recovery_treatment_delta")
            .unwrap();
        assert_eq!(
            first_delta["attempted_product_delta"],
            json!({"added_paths":[],"changed_paths":[],"removed_paths":[]})
        );
        assert!(
            events.iter().any(|e| e["event"] == "tool_execute"
                && e["name"] == "Edit"
                && e["status"] == "ok")
        );
    }
}
