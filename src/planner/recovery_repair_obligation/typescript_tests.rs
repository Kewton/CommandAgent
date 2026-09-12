use super::*;

#[test]
#[ignore = "requires an installed TypeScript compiler; required Issue #465 verification"]
fn issue465_typescript_real_runner_replay_accepts_related_const_fix_and_rejects_bypasses() {
    for variant in [
        "store-const",
        "api-alias",
        "suppression",
        "remove-call",
        "exclude",
        "early-failure",
    ] {
        let root = tempfile::tempdir().unwrap();
        let compiler = std::env::var_os("ISSUE465_TYPESCRIPT_ROOT")
            .expect("set ISSUE465_TYPESCRIPT_ROOT to an installed TypeScript package");
        std::fs::create_dir_all(root.path().join("node_modules/.bin")).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(
            Path::new(&compiler).join("bin/tsc"),
            root.path().join("node_modules/.bin/tsc"),
        )
        .unwrap();
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
        let mut config =
            Config::from_cli(crate::cli::Cli::parse_from(["commandagent", "--ux-demo"])).unwrap();
        config.workspace_root = root.path().to_path_buf();
        config.profile = "generic".into();
        config.profile_explicit = true;
        config.offline = true;
        config.yes = true;
        config.max_iterations = 6;
        config.eval_events_path = Some(root.path().join(".commandagent/events.jsonl"));
        config.completion_contract_path = Some(root.path().join("contract.json"));
        std::fs::write(
            root.path().join("contract.json"),
            json!({
                "goal":"Repair shift API arity", "required_paths":["src/api.ts"],
                "verify_commands":["tsc --noEmit --pretty false"], "profile":"generic"
            })
            .to_string(),
        )
        .unwrap();
        let contract = CompletionContract::load_for_config(&config)
            .unwrap()
            .unwrap();
        let initial = contract.verify(root.path());
        assert!(!initial.is_pass());
        let evidence = initial
            .command_failures
            .iter()
            .map(|f| f.reason.clone())
            .collect::<Vec<_>>();
        assert!(
            evidence
                .join("\n")
                .contains("Expected 2 arguments, but got 1"),
            "{initial:?}"
        );
        let handoff = RecoveryHandoff {
            failure_kind: "compile_error".into(),
            failure_evidence: evidence,
            original_goal: "Repair shift API arity".into(),
            repair_targets: vec!["src/api.ts".into()],
            ..RecoveryHandoff::default()
        };
        crate::planner::recovery_inspection::bind_context(
            &config,
            &config,
            Some("create"),
            &handoff,
            None,
        )
        .unwrap();
        let plan = StepPlan {
            goal: "Repair shift API arity".into(),
            steps: vec![step("fix-shift-routes", "src/api.ts")],
        };
        let edit = |path: &str, old: &str, new: &str| {
            tool(
                "Edit",
                json!({"path":path,"old_string":old,"new_string":new}),
            )
        };
        let mut replies = vec![
            edit("src/api.ts", "missing anchor", "repair"),
            cat("cat src/api.ts"),
        ];
        match variant {
            "store-const" => replies.push(AssistantReply { content:String::new(), tool_calls:vec![
                ToolCall::new("Edit", json!({"path":"src/store.ts","old_string":"policy: Policy)","new_string":"policy: Policy = defaultPolicy)"})),
                ToolCall::new("Edit", json!({"path":"src/types.ts","old_string":"policy: Policy = { max: 8 }","new_string":"policy = { max: 8 } as const"}))
            ], prompt_tokens:None, completion_tokens:None }),
            "api-alias" => replies.push(tool("Write", json!({"path":"src/api.ts","content":"import { validateShift as check } from './store'; import {policy, type Shift} from './types'; export function POST(input: Shift) { return check(input, policy); }"}))),
            "suppression" => replies.push(edit("src/api.ts", "  return", "  // @ts-ignore\n  return")),
            "remove-call" => replies.push(edit("src/api.ts", "validateShift(input)", "true")),
            "exclude" => replies.push(tool("Write",json!({"path":"tsconfig.json","content":"{\"compilerOptions\":{\"strict\":true},\"files\":[\"src/types.ts\"]}"}))),
            "early-failure" => replies.push(edit("src/store.ts", "return input.hours", "return (input.hours")),
            _ => unreachable!(),
        }
        replies.push(AssistantReply::text("Completed"));
        let result = crate::planner::run_step_plan_with_ui(
            &mut Replay::new(replies),
            &plan,
            &config,
            &crate::tui::NOOP_UI,
        );
        assert_eq!(
            result.is_ok(),
            matches!(variant, "store-const" | "api-alias"),
            "{variant}: {result:?}"
        );
        let log = events(&config);
        assert!(
            log.iter()
                .any(|e| e["reason"] == "recovery_repair_unresolved")
        );
        assert_eq!(
            log.iter().any(|e| e["status"] == "resolved"),
            result.is_ok()
        );
    }
}
