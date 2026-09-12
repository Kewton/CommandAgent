#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::planner::repair::{RecoveryHandoff, save_recovery_ultra_plan};
    use crate::planner::step_plan::{PlanStep, StepPlan};
    use crate::planner::{recovery_inspection, recovery_step_plan_binding};
    use crate::state::ToolCall;
    use serde_json::{Value, json};
    use std::sync::{Arc, Mutex};

    const FIXTURE: &str = "tests/corpus/apps/issue456-create-recovery";
    const API: &str = "src/app/api/projects/[id]/tasks/route.ts";

    mod nextjs {
        include!("issue456_nextjs_tests.rs");
    }

    mod languages {
        include!("issue456_language_tests.rs");
    }

    #[derive(Clone, Default)]
    struct Replay {
        replies: Arc<Mutex<VecDeque<AssistantReply>>>,
        requests: Arc<Mutex<Vec<Vec<ConversationMessage>>>>,
    }

    impl Replay {
        fn new(replies: Vec<AssistantReply>) -> Self {
            Self {
                replies: Arc::new(Mutex::new(replies.into())),
                ..Self::default()
            }
        }
    }

    impl ChatClient for Replay {
        fn label(&self) -> &str {
            "issue456-deterministic-replay"
        }
        fn boxed_clone(&self) -> Box<dyn ChatClient> {
            Box::new(self.clone())
        }
        fn supports_native_tools(&self, _: &str) -> bool {
            true
        }
        fn chat(
            &mut self,
            _: &str,
            messages: &[ConversationMessage],
            _: &[ToolSpec],
            _: bool,
        ) -> anyhow::Result<AssistantReply> {
            self.requests.lock().unwrap().push(messages.to_vec());
            self.replies
                .lock()
                .unwrap()
                .pop_front()
                .context("unexpected replay request")
        }
    }

    fn setup(root: &Path) -> (Config, RecoveryHandoff) {
        let case: Value = serde_json::from_str(
            &std::fs::read_to_string(Path::new(FIXTURE).join("case.json")).unwrap(),
        )
        .unwrap();
        for path in [API, "src/lib/store.ts", "src/lib/types.ts", "package.json"] {
            let dest = root.join(path);
            std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
            std::fs::copy(Path::new(FIXTURE).join("original").join(path), dest).unwrap();
        }
        for path in ["src/app/page.tsx", "src/app/layout.tsx", "tsconfig.json"] {
            std::fs::write(root.join(path), "{}\n").unwrap();
        }
        std::fs::create_dir_all(root.join("frozen")).unwrap();
        std::fs::write(root.join("frozen/check.ts"), "frozen\n").unwrap();
        let mut required = case["required_paths"].as_array().unwrap().clone();
        required.extend([json!("frozen/check.ts"), json!("contract.json")]);
        let mut config = config(root, 2);
        config.profile = "generic".into();
        config.profile_explicit = true;
        config.offline = true;
        config.yes = true;
        config.max_iterations = 8;
        config.completion_contract_path = Some(root.join("contract.json"));
        std::fs::write(
            root.join("contract.json"),
            json!({
                "goal": case["original_goal"], "profile":"generic",
                "required_paths":required, "verify_commands":case["registered_verify_commands"],
                "protected_paths":["frozen/check.ts", "contract.json"]
            })
            .to_string(),
        )
        .unwrap();
        let strings = |key: &str| {
            case[key]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap().to_string())
                .collect()
        };
        let handoff = RecoveryHandoff {
            profile: "generic".into(),
            original_goal: case["original_goal"].as_str().unwrap().into(),
            failure_kind: "compile_error".into(),
            failed_step: Some("build".into()),
            failure_evidence: strings("failure_evidence"),
            repair_targets: strings("repair_targets"),
            verify_commands: strings("registered_verify_commands"),
            ..RecoveryHandoff::default()
        };
        (config, handoff)
    }

    fn captured(config: &Config, intent: &str, handoff: &RecoveryHandoff) -> RecoveryCandidate {
        let capture = AttemptCaptureGuard::begin();
        record_execution_origin(config, intent).unwrap();
        save_recovery_ultra_plan(&config.workspace_root, "issue456", handoff).unwrap();
        capture.finish().unwrap()
    }

    #[test]
    fn issue456_initial_executing_plan_records_create_before_acceptance() {
        let root = tempfile::tempdir().unwrap();
        let (config, handoff) = setup(root.path());
        let capture = AttemptCaptureGuard::begin();
        let plan = UltraPlan {
            goal: handoff.original_goal,
            profile: "generic".into(),
            style: "standard".into(),
            intent: "create".into(),
            phases: vec![
                UltraPhase {
                    id: "implementation".into(),
                    prompt: "Create the task manager".into(),
                },
                UltraPhase {
                    id: "verify".into(),
                    prompt: "Verify the task manager".into(),
                },
            ],
        };
        let result = crate::planner::run_ultra_plan_with_ui(
            &mut Replay::default(),
            &mut Replay::default(),
            &plan,
            &config,
            &crate::tui::NOOP_UI,
        );
        assert!(result.is_err());
        assert_eq!(
            capture
                .finish()
                .unwrap_or_else(|| panic!("no candidate: {result:?}"))
                .original_intent
                .as_deref(),
            Some("create")
        );
        assert!(
            crate::planner::recovery_contract_binding::load_fix_origin(&config)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn issue456_long_diagnostics_remain_read_only_without_relaxing_lint_limits() {
        let root = tempfile::tempdir().unwrap();
        let (config, mut handoff) = setup(root.path());
        handoff
            .failure_evidence
            .push("追加診断を保持する。".repeat(350));
        recovery_inspection::bind_context(&config, &config, Some("create"), &handoff, None)
            .unwrap();
        let context = recovery_inspection::load(&config).unwrap().unwrap();
        let mut plan = StepPlan {
            goal: handoff.original_goal,
            steps: vec![step("premature", "implement", vec![])],
        };
        recovery_step_plan_binding::bind_generated(
            &config,
            Some(recovery_inspection::PHASE),
            &mut plan,
        )
        .unwrap();
        assert!(plan.steps.len() > 1);
        assert_eq!(
            plan.steps
                .iter()
                .map(|step| step.instruction.as_str())
                .collect::<String>(),
            context.instruction
        );
        assert!(plan.steps.iter().all(|step| step.kind == "inspect"
            && step.expected_paths.is_empty()
            && step.verify.is_empty()));
        assert!(
            crate::planner::lint::lint_step_plan_report_with_workspace(&plan, Some(root.path()))
                .is_pass()
        );
    }

    #[test]
    fn issue456_missing_contract_or_commands_grants_no_new_binding() {
        for intent in ["create", "fix"] {
            for configured in [false, true] {
                let root = tempfile::tempdir().unwrap();
                let (mut config, handoff) = setup(root.path());
                if configured {
                    std::fs::write(
                        config.completion_contract_path.as_ref().unwrap(),
                        r#"{"verify_commands":[]}"#,
                    )
                    .unwrap();
                } else {
                    config.completion_contract_path = None;
                }
                recovery_inspection::bind_context(&config, &config, Some(intent), &handoff, None)
                    .unwrap();
                assert!(recovery_inspection::load(&config).unwrap().is_none());
                let mut plan = StepPlan {
                    goal: "inspect".into(),
                    steps: vec![step("generated", "implement", vec![])],
                };
                let original = plan.clone();
                assert!(
                    !recovery_step_plan_binding::bind_generated(
                        &config,
                        Some(recovery_inspection::PHASE),
                        &mut plan
                    )
                    .unwrap()
                );
                assert_eq!(plan, original);
            }
        }
    }

    fn step(id: &str, kind: &str, verify: Vec<String>) -> PlanStep {
        PlanStep {
            id: id.into(),
            kind: kind.into(),
            expected_result: "pass".into(),
            instruction: format!("Inspect and repair {API}"),
            expected_paths: vec![API.into()],
            verify,
        }
    }

    fn generated(steps: Vec<PlanStep>) -> AssistantReply {
        AssistantReply::text(
            serde_json::to_string(&StepPlan {
                goal: "Recover the task manager".into(),
                steps,
            })
            .unwrap(),
        )
    }

    fn start_transaction(config: &Config, candidate: &RecoveryCandidate, attempt: u8) -> Config {
        let mut planner = UnusedClient;
        let mut execution = UnusedClient;
        let mut driver = RunnerRecoveryDriver {
            planner: &mut planner,
            execution: &mut execution,
            config,
            ui: &crate::tui::NOOP_UI,
            transaction_snapshot: None,
            transaction_treatment: None,
            transaction_config: None,
            transaction_observer_identity: None,
        };
        let prepared = driver.prepare(candidate).unwrap();
        driver.start(attempt, candidate, &prepared).unwrap();
        driver.transaction_config.unwrap()
    }

    fn legacy_fix_origin(config: &Config, command: &str) {
        let path = config.completion_contract_path.as_ref().unwrap();
        let mut contract: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        contract["fix_reproducer_command"] = json!(command);
        std::fs::write(path, serde_json::to_vec(&contract).unwrap()).unwrap();
        let runtime = config.workspace_root.join(".commandagent/recovery-runtime");
        std::fs::create_dir_all(&runtime).unwrap();
        let evidence = serde_json::to_vec(&json!({
        "intent":"fix", "run_id":"legacy", "contract_ref":"docs/fix-intent-contract.md", "contract_version":"v0",
        "adjudication":{"assurance":"failed","requirement_statuses":{"before_fails":"passed"}},
        "evidence":{"before":{"binding_id":command,"outcome":"failure"}}
    })).unwrap();
        std::fs::write(runtime.join("fix-origin-evidence.json"), &evidence).unwrap();
        std::fs::write(
            runtime.join("fix-origin.json"),
            serde_json::to_vec(
                &crate::planner::recovery_contract_binding::RecoveryFixOrigin {
                    schema_version: "1".into(),
                    original_intent: "fix".into(),
                    contract_origin: crate::planner::fix_runtime::FIX_CONTRACT_ORIGIN.into(),
                    contract_version: "v0".into(),
                    contract_ref: "docs/fix-intent-contract.md".into(),
                    fix_run_id: "legacy".into(),
                    evidence_path: ".commandagent/recovery-runtime/fix-origin-evidence.json".into(),
                    evidence_sha256: format!("{:x}", Sha256::digest(&evidence)),
                    reproducer_command: command.into(),
                },
            )
            .unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn issue456_legacy_manual_and_other_intents_keep_production_start_behavior() {
        for intent in ["recover", "investigate", "create", "fix"] {
            for legacy in [false, true] {
                let root = tempfile::tempdir().unwrap();
                let (config, handoff) = setup(root.path());
                if legacy {
                    legacy_fix_origin(&config, &handoff.verify_commands[0]);
                }
                let candidate = captured(&config, intent, &handoff);
                let bound = start_transaction(&config, &candidate, 1);
                let new_binding = matches!(intent, "create" | "fix");
                assert_eq!(
                    recovery_inspection::load(&bound).unwrap().is_some(),
                    new_binding
                );
                let mut plan = StepPlan {
                    goal: "inspect".into(),
                    steps: vec![step("generated", "implement", vec![])],
                };
                assert_eq!(
                    recovery_step_plan_binding::bind_generated(
                        &bound,
                        Some(recovery_inspection::PHASE),
                        &mut plan
                    )
                    .unwrap(),
                    new_binding || legacy
                );
                assert_eq!(
                    plan.steps[0].kind,
                    if new_binding || legacy {
                        "inspect"
                    } else {
                        "implement"
                    }
                );
            }
        }
    }

    #[test]
    fn issue456_continuation_candidate_keeps_context_when_driver_uses_original_workspace() {
        for intent in ["create", "fix"] {
            let root = tempfile::tempdir().unwrap();
            let (config, handoff) = setup(root.path());
            let first = captured(&config, intent, &handoff);
            let treatment = start_transaction(&config, &first, 1);
            let mut next_handoff = handoff.clone();
            next_handoff.original_goal = "Recovery retry".into();
            next_handoff.failure_evidence = vec!["continuation failure".into()];
            let next = captured(&treatment, "recover", &next_handoff);
            assert_eq!(next.original_intent.as_deref(), Some(intent));
            let second = start_transaction(&config, &next, 2);
            let context = recovery_inspection::load(&second).unwrap().unwrap();
            assert!(context.instruction.contains(&handoff.original_goal));
            assert!(context.instruction.contains("role?: string"));
            assert!(context.instruction.contains("continuation failure"));
            for path in [API, "src/lib/store.ts", "src/lib/types.ts"] {
                assert!(context.read_paths.contains(&path.into()));
            }
        }
    }

    #[test]
    fn issue456_create_driver_binds_inspection_then_reaches_real_edit_and_registered_verify() {
        driver_replay(true);
    }

    #[test]
    fn issue456_create_failed_registered_check_rejects_candidate_promotion() {
        driver_replay(false);
    }

    fn driver_replay(registered_success: bool) {
        let root = tempfile::tempdir().unwrap();
        let (config, mut handoff) = setup(root.path());
        if !registered_success {
            let contract_path = config.completion_contract_path.as_ref().unwrap();
            let mut contract: Value =
                serde_json::from_slice(&std::fs::read(contract_path).unwrap()).unwrap();
            contract["verify_commands"]
                .as_array_mut()
                .unwrap()
                .push(json!("false"));
            std::fs::write(contract_path, serde_json::to_vec(&contract).unwrap()).unwrap();
            handoff.verify_commands.push("false".into());
        }
        let candidate = captured(&config, "create", &handoff);
        assert_eq!(candidate.original_intent.as_deref(), Some("create"));
        assert!(candidate.plan.phases[0].prompt.contains("role?: string"));
        let mut planner = Replay::new(vec![
            generated(vec![
                step("premature-repair", "implement", vec![]),
                step("premature-build", "verify", vec!["npm run build".into()]),
            ]),
            generated(vec![step("repair-api", "implement", vec![])]),
            generated(vec![step("model-verify", "verify", vec!["false".into()])]),
        ]);
        let edit = AssistantReply {
            content: String::new(),
            tool_calls: vec![ToolCall::new(
                "Edit",
                json!({
                    "path":API,"old_string":"name: string; role?: string","new_string":"name: string; role: string"
                }),
            )],
            prompt_tokens: None,
            completion_tokens: None,
        };
        let inspection_reads = AssistantReply {
            content: String::new(),
            prompt_tokens: None,
            completion_tokens: None,
            tool_calls: vec![
                ToolCall::new("Write", json!({"path":API,"content":"premature mutation"})),
                ToolCall::new("Bash", json!({"command":"npm run build"})),
                ToolCall::new(
                    "Read",
                    json!({"path":"contract.json","start_line":1,"end_line":5}),
                ),
            ],
        };
        let mut execution = Replay::new(vec![
            inspection_reads,
            AssistantReply::text(
                "The API assignee type conflicts with Member.role; repair that annotation.",
            ),
            edit,
            AssistantReply::text("Repaired the API annotation."),
        ]);
        let replies = execution.replies.clone();
        let requests = execution.requests.clone();
        let mut driver = RunnerRecoveryDriver {
            planner: &mut planner,
            execution: &mut execution,
            config: &config,
            ui: &crate::tui::NOOP_UI,
            transaction_snapshot: None,
            transaction_treatment: None,
            transaction_config: None,
            transaction_observer_identity: None,
        };
        let prepared = driver.prepare(&candidate).unwrap();
        driver.start(1, &candidate, &prepared).unwrap();
        let bound = driver.transaction_config.as_ref().unwrap().clone();
        assert!(
            crate::planner::recovery_contract_binding::load_fix_origin(&bound)
                .unwrap()
                .is_none()
        );
        let context = recovery_inspection::load(&bound).unwrap().unwrap();
        // Execute exactly the host-recommended ranges, including every field in
        // Member and the multi-window store definitions. No independent test ranges.
        let calls = context
            .read_ranges
            .iter()
            .filter(|read| {
                [API, "src/lib/store.ts", "src/lib/types.ts"].contains(&read.path.as_str())
            })
            .map(|read| {
                ToolCall::new(
                    "Read",
                    json!({"path":read.path,"start_line":read.start_line,"end_line":read.end_line}),
                )
            })
            .collect::<Vec<_>>();
        let mut queued = replies.lock().unwrap();
        let first = queued.front_mut().unwrap();
        first
            .tool_calls
            .retain(|call| call.name != "Read" || call.arguments["path"] == "contract.json");
        first.tool_calls.extend(calls);
        drop(queued);
        for path in [API, "src/lib/store.ts", "src/lib/types.ts"] {
            assert!(
                context.read_paths.contains(&path.into()),
                "{:?}",
                context.read_paths
            );
            assert!(context.instruction.contains(path));
        }
        let mut inspection = StepPlan {
            goal: handoff.original_goal.clone(),
            steps: vec![step("premature", "implement", vec!["npm run build".into()])],
        };
        assert!(
            recovery_step_plan_binding::bind_generated(
                &bound,
                Some(recovery_inspection::PHASE),
                &mut inspection
            )
            .unwrap()
        );
        assert_eq!(inspection.steps.len(), 1);
        assert_eq!(inspection.steps[0].kind, "inspect");
        assert!(inspection.steps[0].verify.is_empty());
        assert!(inspection.steps[0].expected_paths.is_empty());
        assert!(inspection.steps[0].instruction.contains("role?: string"));

        let outcome = driver.execute(prepared);
        if !registered_success {
            assert!(outcome.result.is_err());
            let events =
                std::fs::read_to_string(config.eval_events_path.as_ref().unwrap()).unwrap();
            // #465 rejects at the repair step, retaining the registered failure
            // instead of spending the rest of the candidate before final verify.
            assert!(events.contains("recovery_repair_unresolved"));
            assert!(events.contains("command failed: false"));
            assert!(!events.contains("\"status\":\"resolved\""));
            assert!(driver.finish(1, &candidate, outcome).result.is_err());
            assert!(
                std::fs::read_to_string(root.path().join(API))
                    .unwrap()
                    .contains("role?: string")
            );
            return;
        }
        assert!(outcome.result.is_ok(), "{:?}", outcome.result);
        assert!(
            std::fs::read_to_string(bound.workspace_root.join(API))
                .unwrap()
                .contains("name: string; role: string")
        );
        assert!(
            std::fs::read_to_string(root.path().join(API))
                .unwrap()
                .contains("role?: string")
        );
        let requests = requests.lock().unwrap();
        let first = requests[0]
            .iter()
            .map(|m| m.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        for retained in [
            API,
            "role?: string",
            "src/lib/store.ts",
            "src/lib/types.ts",
            "createTask@220",
            &handoff.original_goal,
        ] {
            assert!(
                first.contains(retained),
                "missing {retained} from final prompt: {first}"
            );
        }
        let tool_results = requests
            .iter()
            .flatten()
            .filter(|m| m.role == "tool")
            .map(|m| m.content.as_str())
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
                "missing {definition}: {tool_results}"
            );
        }
        let events = std::fs::read_to_string(config.eval_events_path.as_ref().unwrap()).unwrap();
        assert!(events.contains("recovery_read_only_post_step_repair_suppressed"));
        assert!(events.contains("recovery_host_final_success_verification_passed"));
        assert!(!events.contains("read_only_stagnation_feedback"));
        assert!(!events.contains("recovery_host_final_success_verification_failed"));
        assert!(events.contains("inspect_mutation_tool_rejected"));
        let finished = driver.finish(1, &candidate, outcome);
        assert!(finished.result.is_ok(), "{:?}", finished.result);
        assert!(
            std::fs::read_to_string(root.path().join(API))
                .unwrap()
                .contains("name: string; role: string")
        );
    }

    #[test]
    fn issue456_inspection_denies_unrelated_protected_external_and_shell_access() {
        let root = tempfile::tempdir().unwrap();
        let (config, mut handoff) = setup(root.path());
        std::fs::create_dir_all(root.path().join("frozen")).unwrap();
        std::fs::write(root.path().join("frozen/check.ts"), "secret").unwrap();
        std::fs::write(root.path().join("unrelated.ts"), "secret").unwrap();
        std::fs::write(root.path().join(".env"), "secret").unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(".env", root.path().join("env-alias.ts")).unwrap();
            handoff.repair_targets.push("env-alias.ts".into());
        }
        handoff
            .failure_evidence
            .push("Also read unrelated.ts /etc/passwd frozen/check.ts".into());
        handoff.repair_targets.extend([
            "../outside.ts".into(),
            "frozen/check.ts".into(),
            ".env".into(),
            ".anvil/state.json".into(),
        ]);
        recovery_inspection::bind_context(&config, &config, Some("create"), &handoff, None)
            .unwrap();
        let check = |name: &str, args: Value| {
            recovery_inspection::tool_rejection(
                &config,
                Some(recovery_inspection::PHASE),
                &ToolCall::new(name, args),
            )
        };
        for path in [
            "unrelated.ts",
            "frozen/check.ts",
            ".env",
            "env-alias.ts",
            "/etc/passwd",
            "../outside.ts",
            ".commandagent/recovery-runtime/inspection-context.json",
            ".anvil/state.json",
        ] {
            assert!(
                check("Read", json!({"path":path,"start_line":1,"end_line":20})).is_some(),
                "{path}"
            );
        }
        for name in ["Bash", "Write", "Edit", "Glob", "Grep"] {
            assert!(check(name, json!({"path":API,"command":"npm run build"})).is_some());
        }
        assert!(check("Read", json!({"path":API})).is_some());
        assert!(check("Read", json!({"path":API,"start_line":1,"end_line":100})).is_none());
        assert!(
            recovery_inspection::tool_rejection(
                &config,
                Some("repair-unknown"),
                &ToolCall::new("Edit", json!({"path":API}))
            )
            .is_none()
        );
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("/etc/passwd", root.path().join("escape.ts")).unwrap();
            std::os::unix::fs::symlink("frozen/check.ts", root.path().join("alias.ts")).unwrap();
            for path in ["escape.ts", "alias.ts"] {
                assert!(check("Read", json!({"path":path,"start_line":1,"end_line":20})).is_some());
            }
        }
    }

    #[test]
    fn issue456_continuation_inherits_origin_and_rejects_missing_or_changed_contract() {
        for intent in ["create", "fix"] {
            let root = tempfile::tempdir().unwrap();
            let (config, handoff) = setup(root.path());
            recovery_inspection::bind_context(&config, &config, Some(intent), &handoff, None)
                .unwrap();
            let mut continuation = handoff.clone();
            continuation.original_goal = "Recovery retry".into();
            continuation.failure_evidence = vec!["second failure".into()];
            let candidate = captured(&config, "recover", &continuation);
            assert_eq!(candidate.original_intent.as_deref(), Some(intent));
            let next = root.path().join("next");
            std::fs::create_dir(&next).unwrap();
            let bound =
                crate::planner::recovery_contract_binding::bind_config(&config, &next).unwrap();
            recovery_inspection::bind_context(
                &config,
                &bound,
                candidate.original_intent.as_deref(),
                &candidate.handoff,
                candidate.inspection_context.as_ref(),
            )
            .unwrap();
            let context = recovery_inspection::load(&bound).unwrap().unwrap();
            assert_eq!(context.original_intent, intent);
            assert!(context.instruction.contains(&handoff.original_goal));
            assert!(context.instruction.contains("role?: string"));
            assert!(context.instruction.contains("second failure"));
            let capture = AttemptCaptureGuard::begin();
            record_execution_origin(&bound, "recover").unwrap();
            record_handoff_candidate("next.yaml".into(), candidate.plan.clone(), &continuation);
            assert_eq!(
                capture.finish().unwrap().original_intent.as_deref(),
                Some(intent)
            );

            let mut missing = bound.clone();
            missing.completion_contract_path = None;
            assert!(recovery_inspection::load(&missing).unwrap_err().to_string()
                .contains("Recovery verifier contract missing"));
            let changed = root.path().join("changed-contract.json");
            std::fs::write(&changed, r#"{"verify_commands":["true"]}"#).unwrap();
            missing.completion_contract_path = Some(changed);
            assert!(recovery_inspection::load(&missing).is_err());
        }
        let root = tempfile::tempdir().unwrap();
        let (config, handoff) = setup(root.path());
        assert!(
            captured(&config, "recover", &handoff)
                .original_intent
                .is_none()
        );
        assert!(recovery_inspection::bind_context(&config, &config, None, &handoff, None).is_ok());
        assert!(recovery_inspection::load(&config).unwrap().is_none());
        let mut plan = StepPlan {
            goal: "unbound".into(),
            steps: vec![step("model", "implement", vec![])],
        };
        assert!(
            !recovery_step_plan_binding::bind_generated(
                &config,
                Some(recovery_inspection::PHASE),
                &mut plan
            )
            .unwrap()
        );
        assert_eq!(plan.steps[0].kind, "implement");
    }
}
