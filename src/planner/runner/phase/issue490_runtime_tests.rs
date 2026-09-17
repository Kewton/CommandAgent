use super::super::super::super::{ContractEnforcement, StepPromptContext, run_step};
use super::*;

#[derive(Default, Clone)]
struct RepairBoundary {
    calls: Arc<Mutex<usize>>,
}
impl ChatClient for RepairBoundary {
    fn label(&self) -> &str {
        "issue490-runtime-boundary"
    }
    fn boxed_clone(&self) -> Box<dyn ChatClient> {
        Box::new(self.clone())
    }
    fn chat(
        &mut self,
        _: &str,
        messages: &[crate::state::ConversationMessage],
        _: &[crate::tools::registry::ToolSpec],
        _: bool,
    ) -> anyhow::Result<AssistantReply> {
        *self.calls.lock().unwrap() += 1;
        let prompt = serde_json::to_string(messages)?;
        assert!(prompt.contains("package.json"));
        anyhow::bail!("issue490: existing execution/repair boundary")
    }
}
#[test]
fn issue490_runtime_metadata_precheck_pass_fail_missing() {
    let first = raw("P02-D", 1);
    let proposed = raw("P02-D", 2);
    let reader = &first.steps[0];
    assert_eq!(proposed.steps[0], *reader);
    assert_eq!(proposed.steps[5].verify, reader.verify);
    let root = tempfile::tempdir().unwrap();
    copy_tree(
        &Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(FIXTURE)
            .join("scaffold"),
        root.path(),
    );
    let metadata: Vec<serde_json::Value> =
        serde_json::from_str(&fixture("runtime-metadata.json")).unwrap();
    for (step, expected) in proposed.steps.iter().zip(metadata) {
        let runtime =
            resolve_profile_runtime(crate::planner::profile_descriptor::NEXTJS_PROFILE_ID);
        let (effective, synthesized) = runtime.runtime_step_with_profile_checks(
            root.path(),
            &proposed.goal,
            step,
            Some("core-implementation"),
            None,
        );
        assert_eq!(serde_json::to_value(&effective).unwrap(), expected["step"]);
        assert_eq!(synthesized, expected["synthesized"].as_bool().unwrap());
        assert_eq!(
            runtime.step_short_circuit_precheck_applicable(&effective),
            expected["precheck"].as_bool().unwrap()
        );
    }
    for condition in ["pass", "fail", "missing"] {
        for step in [&first.steps[0], &proposed.steps[0], &proposed.steps[5]] {
            let root = tempfile::tempdir().unwrap();
            let mut c =
                Config::from_cli(crate::cli::Cli::parse_from(["commandagent", "--ux-demo"]))
                    .unwrap();
            c.workspace_root = root.path().into();
            c.state_dir = root.path().join("state");
            c.profile = crate::planner::profile_descriptor::NEXTJS_PROFILE_ID.into();
            c.offline = true;
            c.chat_retries = 0;
            c.eval_events_path = Some(root.path().join("events.jsonl"));
            if condition != "missing" {
                let mut package: serde_json::Value =
                    serde_json::from_str(&fixture("scaffold/package.json")).unwrap();
                if condition == "fail" {
                    package["scripts"]["dev"] = json!("next dev -p 3000");
                }
                std::fs::write(
                    root.path().join("package.json"),
                    serde_json::to_vec(&package).unwrap(),
                )
                .unwrap();
            }
            let runtime =
                resolve_profile_runtime(crate::planner::profile_descriptor::NEXTJS_PROFILE_ID);
            let (effective, synthesized) = runtime.runtime_step_with_profile_checks(
                root.path(),
                &first.goal,
                step,
                Some("core-implementation"),
                None,
            );
            assert!(!synthesized);
            assert_eq!(effective, *step);
            assert!(runtime.step_short_circuit_precheck_applicable(&effective));
            let report =
                crate::planner::verify::verify_step_with_profile_setup_observed_with_offline(
                    root.path(),
                    &effective,
                    Some(crate::planner::profile_descriptor::NEXTJS_PROFILE_ID),
                    crate::minimal_loop::dependency_setup::NodeDependencySetupAuthority::None,
                    true,
                )
                .0;
            assert_eq!(
                report.is_pass(),
                condition == "pass",
                "{condition}: {report:?}"
            );
            assert_eq!(
                report.missing_paths,
                if condition == "missing" {
                    vec!["package.json"]
                } else {
                    vec![]
                }
            );
            if condition == "missing" {
                assert_eq!(report.dependency_missing.len(), step.verify.len());
                assert!(
                    report
                        .dependency_missing
                        .iter()
                        .all(|m| m.contains("dependency_setup_authority_required"))
                );
            }
            if condition == "fail" {
                assert_eq!(report.command_failures.len(), 1);
                assert!(
                    report
                        .command_failures
                        .iter()
                        .all(|f| step.verify.contains(&f.command))
                );
                assert!(report.command_failures.iter().all(|f| !f.reason.is_empty()));
            }
            let plan = StepPlan {
                goal: first.goal.clone(),
                steps: vec![step.clone()],
            };
            let mut client = RepairBoundary::default();
            let result = run_step(
                &mut client,
                &mut SessionSnapshot::new(),
                &plan,
                step,
                &StepPromptContext {
                    overall_goal: first.goal.clone(),
                    ..Default::default()
                },
                &c,
                &NOOP_UI,
                "issue490-runtime",
                ContractEnforcement::Enforce,
                Some("core-implementation"),
                None,
            );
            let log = events(&c);
            if condition == "pass" {
                assert_eq!(
                    result.unwrap().stop_reason.as_deref(),
                    Some("StepShortCircuited")
                );
                assert_eq!(*client.calls.lock().unwrap(), 0);
                let e = log
                    .iter()
                    .find(|e| e["event"] == "step_short_circuited")
                    .unwrap();
                assert_eq!(e["required_paths"], json!(step.expected_paths));
                assert_eq!(e["verify_commands"], json!(step.verify));
            } else {
                let error = result.unwrap_err();
                assert!(
                    error.message.contains("existing execution/repair boundary"),
                    "{error:?}"
                );
                assert_eq!(*client.calls.lock().unwrap(), 1);
                assert!(!log.iter().any(|e| e["event"] == "step_short_circuited"));
            }
        }
    }
}
