#[cfg(test)]
mod cases {
    use super::super::*;
    use serde_json::{Value, json};

    const PAGE: &str = "src/app/page.tsx";
    const API: &str = "src/app/api/projects/route.ts";
    const IMPLEMENTED: &str =
        "export default function Page(){return <main>Agency projects</main>;}\n";

    fn step(id: &str, path: &str) -> PlanStep {
        PlanStep {
            id: id.to_string(),
            kind: "implement".to_string(),
            expected_result: "pass".to_string(),
            instruction: format!(
                "Implement the requested agency project management behavior in {path}"
            ),
            expected_paths: vec![path.to_string()],
            verify: vec![format!("test -f {path}")],
        }
    }

    fn reply(tool: &str, arguments: Value) -> AssistantReply {
        AssistantReply {
            content: String::new(),
            tool_calls: vec![crate::state::ToolCall::new(tool, arguments)],
            prompt_tokens: None,
            completion_tokens: None,
        }
    }

    fn run_plan(existing: bool) -> Vec<Value> {
        let dir = tempfile::tempdir().unwrap();
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/corpus/apps/issue420-step-completion");
        for path in [PAGE, "package.json"] {
            let target = dir.path().join(path);
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            std::fs::copy(fixture.join(path), target).unwrap();
        }
        if existing {
            std::fs::write(dir.path().join(PAGE), IMPLEMENTED).unwrap();
        }
        let plan = StepPlan {
            goal: "Implement an agency project management application".to_string(),
            steps: vec![
                step("implement-project-api", API),
                step("implement-page", PAGE),
            ],
        };
        let mut cfg = config(dir.path().to_path_buf());
        cfg.profile = crate::planner::profile_descriptor::NEXTJS_PROFILE_ID.to_string();
        cfg.eval_events_path = Some(dir.path().join("events.jsonl"));
        cfg.completion_contract_path = Some(dir.path().join("contract.json"));
        std::fs::write(cfg.completion_contract_path.as_ref().unwrap(), json!({
            "required_paths": [PAGE, API], "required_obligations": ["implementation"],
            "required_evidence": ["nextjs_route_evidence", "build_command_or_dependency_missing_boundary"],
        }).to_string()).unwrap();
        let api = std::fs::read_to_string(fixture.join(API)).unwrap();
        let mut replies = vec![
            reply("Write", json!({"path": API, "content": api})),
            reply("Read", json!({"path": PAGE})),
        ];
        if !existing {
            replies.push(reply(
                "Write",
                json!({"path": PAGE, "content": IMPLEMENTED}),
            ));
        }
        let mut client = FakeClient::new(replies);
        let result = run_step_plan_with_session_with_ui(
            &mut client,
            &mut SessionSnapshot::new(),
            &plan,
            &cfg,
            &NOOP_UI,
            false,
            "issue420",
            Some("core-implementation"),
            None,
        )
        .unwrap();
        assert_eq!(result.completed_steps, 2);
        assert_eq!(
            std::fs::read_to_string(dir.path().join(PAGE)).unwrap(),
            IMPLEMENTED
        );
        std::fs::read_to_string(cfg.eval_events_path.unwrap())
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    #[test]
    fn issue420_api_then_ui_plan_completes_with_page_change() {
        let events = run_plan(false);
        let api = events
            .iter()
            .find(|e| {
                e["event"] == "plan_step_completed" && e["step_id"] == "implement-project-api"
            })
            .unwrap();
        assert_eq!(api["changed_paths"], json!([API]));
        let page = events
            .iter()
            .find(|e| e["event"] == "plan_step_completed" && e["step_id"] == "implement-page")
            .unwrap();
        assert_eq!(page["terminal_status"], "completed");
        assert_eq!(page["changed_paths"], json!([PAGE]));
        assert_eq!(page["changed_path_count"], 1);
        assert!(
            !events
                .iter()
                .any(|e| e["event"] == "step_short_circuited" && e["step_id"] == "implement-page")
        );
    }

    #[test]
    fn issue420_existing_page_plan_records_noop_step_id_and_verified_requirements() {
        let events = run_plan(true);
        let short = events
            .iter()
            .find(|e| e["event"] == "step_short_circuited" && e["step_id"] == "implement-page")
            .unwrap();
        assert_eq!(short["at"], "iteration");
        assert_eq!(short["write_or_edit_seen"], false);
        assert_eq!(short["satisfaction_basis"], "verified_existing_artifacts");
        assert_eq!(
            short["completion_requirements"]["required_obligations"],
            json!(["implementation"])
        );
        let terminal = events
            .iter()
            .find(|e| e["event"] == "plan_step_completed" && e["step_id"] == short["step_id"])
            .unwrap();
        assert_eq!(terminal["terminal_status"], "completed");
        assert_eq!(terminal["changed_path_count"], 0);
    }
}
