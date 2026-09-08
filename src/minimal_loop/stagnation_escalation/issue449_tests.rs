// Context and tool execution are real; compiler output is #448 measured replay.
#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::minimal_loop::build_verifier::{BuildVerifierRequirement, observe_requirement};
    use crate::minimal_loop::import_scan::{
        ImportScanIssue, format_missing_import_findings, scan_relative_imports,
    };
    use crate::planner::repair::{self, RepairContext};
    use crate::planner::verify::{CommandFailure, VerificationReport, VerifyStatus};
    use crate::tools::registry::{ToolContext, ToolRegistry};

    const FIXTURE: &str = "tests/corpus/apps/issue448-nextjs-r0";
    const UI: &str = "src/app/page.tsx";
    const IMPORTER: &str = "src/app/api/tasks/[id]/route.ts";

    fn fixture_json(path: &str) -> Value {
        serde_json::from_slice(&std::fs::read(Path::new(FIXTURE).join(path)).unwrap()).unwrap()
    }

    fn cases() -> Vec<Value> {
        let fixture = serde_json::from_str::<Value>(include_str!(
            "../../../tests/corpus/apps/issue449-repair-context/cases.json"
        ))
        .unwrap();
        assert_eq!(
            fixture["verification_command"],
            fixture_json("evidence/original-completion-contract.json")["verify_commands"][0]
        );
        fixture["cases"].as_array().unwrap().clone()
    }

    fn copy_tree(source: &Path, target: &Path) {
        std::fs::create_dir_all(target).unwrap();
        for entry in std::fs::read_dir(source).unwrap() {
            let entry = entry.unwrap();
            let dest = target.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_tree(&entry.path(), &dest);
            } else {
                std::fs::copy(entry.path(), dest).unwrap();
            }
        }
    }

    fn input(root: &Path, case: &Value) -> (Config, VerificationReport, RepairContext) {
        copy_tree(&Path::new(FIXTURE).join("original"), root);
        for overlay in case["overlays"].as_array().unwrap() {
            copy_tree(
                &Path::new(FIXTURE)
                    .join("overlays")
                    .join(overlay.as_str().unwrap()),
                root,
            );
        }
        let measured = fixture_json("evidence/build-results.json")["results"]
            .as_array()
            .unwrap()
            .iter()
            .find(|result| result["case"] == case["build_case"])
            .unwrap()
            .clone();
        std::fs::write(
            root.join("diagnostics.txt"),
            measured["diagnostics"].as_str().unwrap(),
        )
        .unwrap();
        std::fs::write(root.join("replay.sh"), "cat diagnostics.txt\nexit 1\n").unwrap();
        let observation = observe_requirement(
            root,
            &BuildVerifierRequirement {
                command: "sh replay.sh".into(),
                profile: Some("nextjs".into()),
                reason: "Issue #448 measured compiler output replay".into(),
                authority: "fixture".into(),
                status: "required".into(),
                requires_dependency_setup: false,
                required_for_completion: true,
            },
        );
        assert!(observation.attempted);
        assert_eq!(observation.status.as_str(), "failed");
        let missing = scan_relative_imports(root, &[IMPORTER.into()]).unwrap();
        let mut report = VerificationReport::pass();
        report.status = VerifyStatus::CommandFailed("npm run build".into());
        report.compile_errors = observation.compile_errors;
        report.profile_failures = format_missing_import_findings(root, &missing);
        report.command_failures.push(CommandFailure {
            command: "npm run build".into(),
            reason: report.compile_errors[0].message.clone(),
        });
        assert!(!report.is_pass());
        let mut targets =
            crate::planner::contract_attribute_repair::merge_repair_target_paths(&report, &[]);
        // Carry the definition from the actual import scanner, not a UI fallback.
        for finding in missing {
            if let ImportScanIssue::MissingExport {
                definition_path, ..
            } = finding.issue
                && !targets.contains(&definition_path)
            {
                targets.push(definition_path);
            }
        }
        assert!(targets.contains(&case["repair_target"].as_str().unwrap().to_string()));
        let mut cfg = config(root.to_path_buf());
        cfg.profile = "nextjs".into();
        let contract = root.join("contract.json");
        std::fs::copy(
            Path::new(FIXTURE).join("evidence/original-completion-contract.json"),
            &contract,
        )
        .unwrap();
        cfg.completion_contract_path = Some(contract);
        cfg.eval_events_path = Some(root.join("events.jsonl"));
        let contract = fixture_json("evidence/original-completion-contract.json");
        let context = RepairContext {
            profile: Some("nextjs".into()),
            overall_goal: Some(contract["goal"].as_str().unwrap().into()),
            expected_paths: targets,
            verify_commands: vec!["npm run build".into()],
            changed_files: vec![UI.into()],
            workspace_root: Some(root.to_path_buf()),
            ..RepairContext::default()
        };
        (cfg, report, context)
    }

    #[test]
    fn issue449_compact_and_regeneration_preserve_supplied_repair_context() {
        for case in cases() {
            let root = tempfile::tempdir().unwrap();
            let (_, report, mut context) = input(root.path(), &case);
            for layout in [PromptLayout::Legacy, PromptLayout::Stable] {
                context.prompt_layout = layout;
                for prompt in [
                    repair::build_compact_compile_repair_prompt_with_context(
                        "build-verification",
                        &report,
                        &context,
                    ),
                    repair::build_compile_regeneration_prompt_with_context(
                        "build-verification",
                        &report,
                        &context,
                        case["repair_target"].as_str().unwrap(),
                    ),
                ] {
                    for retained in [
                        context.overall_goal.as_deref().unwrap(),
                        "npm run build",
                        case["repair_target"].as_str().unwrap(),
                        case["diagnostic"].as_str().unwrap(),
                    ] {
                        assert!(
                            prompt.contains(retained),
                            "{} / {layout:?}: lost {retained}",
                            case["name"]
                        );
                    }
                    // A simultaneous API export failure must survive a Promise-focused retry.
                    for failure in &report.profile_failures {
                        assert!(prompt.contains(failure));
                    }
                }
            }
        }
    }

    fn activate(root: &Path, prompt: &str, targets: &[String]) -> WriteRequiredState {
        let mut state = WriteRequiredState::default();
        let feedback = crate::minimal_loop::read_only_stagnation_feedback::maybe_read_only_stagnation_feedback(
            None, root, "nextjs", prompt, 6, 6,
            &RunSessionOptions::default(), &mut state,
            &crate::minimal_loop::loop_run::RunSessionErrorContext::default(),
            &[], targets, &[UI.into()], None,
        ).unwrap();
        assert!(feedback.contains("Write") && feedback.contains("Edit"));
        assert_eq!(state.selected_targets(), targets);
        state
    }

    fn rejection_context() -> ReadOnlyToolRejectionContext<'static> {
        ReadOnlyToolRejectionContext {
            read_only_streak: 6,
            session_scope: "plan-run-step",
            step_kind: "verify",
            phase_scope: Some("inspect-current-state"),
        }
    }

    #[test]
    fn issue449_handoff_quotes_diagnostics_without_changing_authority() {
        let root = tempfile::tempdir().unwrap();
        let (cfg, _, context) = input(root.path(), &cases()[0]);
        let contract_path = cfg.completion_contract_path.as_ref().unwrap();
        let original = std::fs::read(contract_path).unwrap();
        let objective = "Compile repair isValidTaskStatus\n\nOriginal goal:\nReplace the task\n\nVerification commands:\n- true";
        let paths = save_read_only_write_required_handoff(
            &cfg,
            objective,
            "plan-run-step",
            "verify",
            Some("repair"),
            &context.expected_paths,
            "required_path",
            &[],
            6,
            WRITE_REQUIRED_NO_WRITE_LIMIT,
        )
        .unwrap();
        let prompt = std::fs::read_to_string(root.path().join(paths.prompt_path)).unwrap();
        let yaml = std::fs::read_to_string(root.path().join(paths.yaml_path)).unwrap();
        let plan = crate::planner::ultra_plan::parse_ultra_plan(&yaml).unwrap();
        assert_eq!(plan.goal, context.overall_goal.unwrap());
        assert!(prompt.contains("isValidTaskStatus"));
        assert_eq!(
            prompt
                .lines()
                .filter(|line| *line == "Original goal:")
                .count(),
            1
        );
        assert_eq!(
            prompt
                .lines()
                .filter(|line| *line == "Verification commands:")
                .count(),
            1
        );
        assert!(plan.phases[2].prompt.contains("npm run build"));
        assert!(!plan.phases[2].prompt.contains("true"));
        assert_eq!(std::fs::read(contract_path).unwrap(), original);
    }

    #[test]
    fn issue449_read_only_handoff_preserves_diagnostics_after_ui_change() {
        for case in cases() {
            let root = tempfile::tempdir().unwrap();
            let (cfg, report, context) = input(root.path(), &case);
            copy_tree(&Path::new(FIXTURE).join("overlays/ui-only"), root.path());
            let prompt =
                repair::build_repair_prompt_with_context("build-verification", &report, &context);
            let mut state = activate(root.path(), &prompt, &context.expected_paths);
            let read = ToolCall::new("Read", json!({"path": case["repair_target"]}));
            for attempt in 1..=WRITE_REQUIRED_NO_WRITE_LIMIT {
                let rejection = state
                    .reject_if_read_only_or_wrong_target(
                        root.path(),
                        None,
                        &read,
                        rejection_context(),
                    )
                    .unwrap();
                assert_eq!(
                    rejection.exhausted,
                    attempt == WRITE_REQUIRED_NO_WRITE_LIMIT
                );
            }
            let paths = save_read_only_write_required_handoff(
                &cfg,
                &prompt,
                "plan-run-step",
                "verify",
                Some("inspect-current-state"),
                state.selected_targets(),
                "required_path",
                &[UI.into()],
                6,
                WRITE_REQUIRED_NO_WRITE_LIMIT,
            )
            .unwrap();
            let saved = std::fs::read_to_string(root.path().join(paths.prompt_path)).unwrap();
            let yaml = std::fs::read_to_string(root.path().join(paths.yaml_path)).unwrap();
            let plan = crate::planner::ultra_plan::parse_ultra_plan(&yaml).unwrap();
            assert_eq!(plan.goal, context.overall_goal.unwrap());
            for text in [&saved, &yaml] {
                for retained in [
                    "npm run build",
                    case["repair_target"].as_str().unwrap(),
                    case["diagnostic"].as_str().unwrap(),
                    UI,
                ] {
                    assert!(text.contains(retained), "{}: lost {retained}", case["name"]);
                }
            }
        }
    }

    #[test]
    fn issue449_target_edits_execute_and_root_and_noop_guards_remain() {
        for case in cases() {
            for tool in ["Write", "Edit"] {
                let root = tempfile::tempdir().unwrap();
                let (_, report, context) = input(root.path(), &case);
                let prompt = repair::build_repair_prompt_with_context(
                    "build-verification",
                    &report,
                    &context,
                );
                let mut state = activate(root.path(), &prompt, &context.expected_paths);
                let path = case["repair_target"].as_str().unwrap();
                let old = std::fs::read_to_string(root.path().join(path)).unwrap();
                let new = std::fs::read_to_string(
                    Path::new(FIXTURE)
                        .join("overlays")
                        .join(case["repair_overlay"].as_str().unwrap())
                        .join(path),
                )
                .unwrap();
                let registry = ToolRegistry::default();
                assert!(
                    registry
                        .specs()
                        .iter()
                        .any(|spec| spec.function.name == tool)
                );
                let tools = ToolContext {
                    root: root.path().to_path_buf(),
                    mode: crate::mode::ExecutionMode::Act,
                    auto_approve: true,
                    interactive_approval: false,
                    offline: false,
                    workspace_policy: crate::tools::workspace_policy::WorkspacePolicy::Normal,
                    eval_events_path: None,
                    expected_paths: context.expected_paths.clone(),
                    protected_paths: vec![],
                };
                let noop = json!({"path": path, "old_string": old, "new_string": old});
                assert!(
                    registry
                        .execute("Edit", &noop, &tools)
                        .unwrap_err()
                        .to_string()
                        .contains("edit_noop")
                );
                assert_eq!(
                    std::fs::read_to_string(root.path().join(path)).unwrap(),
                    old
                );
                assert!(!state.selected_targets().is_empty());
                assert!(registry.execute(tool, &json!({"path": "../outside.ts", "content": new, "old_string": old, "new_string": new}), &tools).is_err());
                let args = if tool == "Write" {
                    json!({"path": path, "content": new})
                } else {
                    json!({"path": path, "old_string": old, "new_string": new})
                };
                let call = ToolCall::new(tool, args.clone());
                assert!(
                    state
                        .reject_if_read_only_or_wrong_target(
                            root.path(),
                            None,
                            &call,
                            rejection_context()
                        )
                        .is_none()
                );
                registry.execute(tool, &args, &tools).unwrap();
                assert_eq!(
                    std::fs::read_to_string(root.path().join(path)).unwrap(),
                    new
                );
                assert!(state.note_successful_write(root.path(), &args));
                assert!(state.selected_targets().is_empty());
                // Revalidation/promotion is separately covered by the #448 gates.
            }
        }
    }
}
