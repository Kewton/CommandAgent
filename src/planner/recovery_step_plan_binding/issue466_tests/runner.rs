use super::*;

#[test]
fn issue466_runner_binding_feedback_then_executes_original_checks() {
    let (root, c, _guard) = registered(&[SCRIPT], &[]);
    let wrong = plan(vec![step("wrong-kind", "report", &[SCRIPT])]);
    let correct = plan(vec![step("candidate-owner", "implement", &[SCRIPT])]);
    let mut planner = Replay::new(vec![proposal(&wrong), proposal(&correct)]);
    let p = generate(&c, &mut planner).unwrap();
    assert_eq!(planner.requests.lock().unwrap().len(), 2);
    let feedback = serde_json::to_string(&planner.requests.lock().unwrap()[1]).unwrap();
    assert!(
        feedback.contains("wrong-kind") && feedback.contains("Implement/pass"),
        "{feedback}"
    );
    assert_eq!(p.steps[0].id, "candidate-owner");
    assert!(p.steps[0].instruction.contains(INSTRUCTION));
    let content = std::fs::read_to_string(Path::new(FIXTURE).join(SCRIPT)).unwrap();
    let mut execution = Replay::new(vec![
        AssistantReply {
            content: String::new(),
            tool_calls: vec![ToolCall::new(
                "Write",
                json!({"path":SCRIPT,"content":content}),
            )],
            prompt_tokens: None,
            completion_tokens: None,
        },
        AssistantReply::text("Created verifier"),
    ]);
    let result = crate::planner::runner::run_step_plan(&mut execution, &p, &c);
    assert!(result.is_ok(), "{result:?}");
    assert!(root.path().join(SCRIPT).is_file());
    assert!(
        events(&c)
            .iter()
            .any(|e| e["event"] == "recovery_verifier_plan_admission" && e["status"] == "retry")
    );
    assert!(
        p.steps
            .last()
            .unwrap()
            .verify
            .contains(&format!("node {SCRIPT}"))
    );
}

#[test]
fn issue466_runner_exhaustion_and_unsafe_never_execute() {
    for unsafe_case in [false, true] {
        let (root, c, _guard) = registered(&[SCRIPT], &[]);
        let bad = plan(vec![step(
            "bad-owner",
            if unsafe_case { "implement" } else { "report" },
            if unsafe_case {
                &[SCRIPT, "./smoke-check.js"]
            } else {
                &[SCRIPT]
            },
        )]);
        let mut planner = Replay::new(vec![proposal(&bad); 3]);
        let mut execution = Replay::default();
        let result = generate(&c, &mut planner)
            .and_then(|p| crate::planner::runner::run_step_plan(&mut execution, &p, &c));
        assert!(result.is_err());
        assert_eq!(
            planner.requests.lock().unwrap().len(),
            if unsafe_case { 1 } else { 3 }
        );
        assert!(execution.requests.lock().unwrap().is_empty());
        assert!(!root.path().join(SCRIPT).exists());
        if !unsafe_case {
            assert!(result.unwrap_err().to_string().contains("3-attempt"));
        }
    }
}

#[test]
fn issue466_original_instruction_checks_and_host_scope_cannot_be_deleted_before_execution() {
    for removal in ["instruction", "check", "final", "owner"] {
        let (_root, c, _guard) = registered(&[SCRIPT], &[]);
        let mut p = plan(vec![step("owner", "implement", &[SCRIPT])]);
        bind_generated(&c, Some("repair"), &mut p).unwrap();
        match removal {
            "instruction" => p.steps[0].instruction = "Just print success".into(),
            "check" => p.steps[0].verify.clear(),
            "final" => {
                p.steps.pop();
            }
            "owner" => {
                p.steps.remove(0);
            }
            _ => unreachable!(),
        }
        let mut execution = Replay::default();
        assert!(
            crate::planner::runner::run_step_plan(&mut execution, &p, &c).is_err(),
            "{removal}"
        );
        assert!(execution.requests.lock().unwrap().is_empty(), "{removal}");
    }
}

#[test]
fn issue466_closed_scope_tampering_and_existing_verifier_mutation_stop() {
    for tamper in ["drop_scope", "delete_context", "verifier_bytes"] {
        let (root, c, _guard) = registered(&[SCRIPT], &[SCRIPT]);
        let path = root
            .path()
            .join(".commandagent/recovery-runtime/inspection-context.json");
        if tamper == "delete_context" {
            std::fs::remove_file(path).unwrap();
        } else if tamper == "drop_scope" {
            let mut context: serde_json::Value =
                serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
            context["verifier_steps"] = json!([]);
            let mut perms = std::fs::metadata(&path).unwrap().permissions();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                perms.set_mode(0o600);
            }
            std::fs::set_permissions(&path, perms).unwrap();
            std::fs::write(path, serde_json::to_vec(&context).unwrap()).unwrap();
        } else {
            std::fs::write(root.path().join(SCRIPT), "process.exit(0)").unwrap();
        }
        let mut planner = Replay::new(vec![proposal(&plan(vec![]))]);
        assert!(generate(&c, &mut planner).is_err(), "{tamper}");
        assert_eq!(planner.requests.lock().unwrap().len(), 0);
    }
}
