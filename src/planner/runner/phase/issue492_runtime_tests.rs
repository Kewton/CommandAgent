//! Opt-in real compiler controls. No model or fake compiler is involved.
use super::*;
use crate::planner::profile_descriptor::NEXTJS_PROFILE_ID;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Clone)]
struct FixedPlanner(StepPlan);
impl ChatClient for FixedPlanner {
    fn label(&self) -> &str {
        "issue492-fixed-planner"
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
        let _ = messages;
        Ok(AssistantReply::text(serde_json::to_string(&self.0)?))
    }
}

#[derive(Clone)]
struct FixedOperations {
    config: Config,
    source: String,
    calls: Arc<Mutex<BTreeMap<String, usize>>>,
}
impl ChatClient for FixedOperations {
    fn label(&self) -> &str {
        "issue492-fixed-operations"
    }
    fn boxed_clone(&self) -> Box<dyn ChatClient> {
        Box::new(self.clone())
    }
    fn chat(
        &mut self,
        _: &str,
        _: &[crate::state::ConversationMessage],
        _: &[crate::tools::registry::ToolSpec],
        _: bool,
    ) -> anyhow::Result<AssistantReply> {
        let log = events(&self.config);
        let step = log
            .iter()
            .rev()
            .find(|e| e["event"] == "plan_step_started")
            .unwrap()["step_id"]
            .as_str()
            .unwrap()
            .to_string();
        if step == "package-read-original"
            && self
                .config
                .workspace_root
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("R4")
        {
            anyhow::bail!("issue492: profile failure boundary; no configuration repair allowed");
        }
        let mut calls = self.calls.lock().unwrap();
        let count = calls.entry(step.clone()).or_default();
        *count += 1;
        if step == "implement-page" && *count == 1 {
            return Ok(AssistantReply {
                content: String::new(),
                tool_calls: vec![crate::state::ToolCall::new(
                    "Write",
                    json!({"path":"src/app/page.tsx","content":self.source}),
                )],
                prompt_tokens: None,
                completion_tokens: None,
            });
        }
        if *count > if step == "implement-page" { 2 } else { 1 } {
            anyhow::bail!(
                "issue492: stop at first failure; source repair is forbidden in this control"
            );
        }
        Ok(AssistantReply::text(
            "Fixed operation complete. Run every declared verification command.",
        ))
    }
}

#[test]
#[ignore = "requires prepared real Next.js controls; see corpus README"]
fn issue492_real_nextjs_good_broken_missing_and_profile_controls() {
    let apps = std::path::PathBuf::from(std::env::var("ISSUE492_APPS_DIR").expect("prepared apps"));
    let logs = std::path::PathBuf::from(std::env::var("ISSUE492_LOG_ROOT").expect("raw log root"));
    let fixture_root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus/apps/issue492-build-duty");
    let raw: StepPlan =
        serde_json::from_slice(&std::fs::read(fixture_root.join("runtime-plan.json")).unwrap())
            .unwrap();
    let selected = std::env::var("ISSUE492_CASE").ok();
    for case in ["R1", "R2", "R3", "R4-port", "R4-build"] {
        if selected.as_deref().is_some_and(|s| s != case) {
            continue;
        }
        let root = apps.join(case);
        assert!(!root.join(".next").exists(), "fresh workspace required");
        assert!(!root.join("state").exists(), "fresh state required");
        let source = std::fs::read_to_string(root.join("src/app/page.tsx")).unwrap();
        let prescribed = std::fs::read_to_string(fixture_root.join(if case == "R2" {
            "broken-page.tsx"
        } else {
            "nextjs/src/app/page.tsx"
        }))
        .unwrap();
        assert_eq!(source, prescribed);
        let source_hash = format!("{:x}", Sha256::digest(source.as_bytes()));
        let mut c =
            Config::from_cli(crate::cli::Cli::parse_from(["commandagent", "--ux-demo"])).unwrap();
        c.workspace_root = root.clone();
        c.state_dir = root.join("state");
        c.eval_events_path = Some(logs.join(case).join("events.jsonl"));
        std::fs::create_dir_all(c.eval_events_path.as_ref().unwrap().parent().unwrap()).unwrap();
        c.profile = NEXTJS_PROFILE_ID.into();
        c.profile_explicit = true;
        c.offline = true;
        c.yes = true;
        c.chat_retries = 0;
        c.plan_preset = crate::config::PlanPreset::Profile;
        c.intent_override = Some(crate::planner::adjudication::contract::IntentId::Create);
        let ultra = crate::planner::profile::profile_preset_ultra_plan(
            NEXTJS_PROFILE_ID,
            &raw.goal,
            "default",
            "create",
        )
        .unwrap();
        assert!(crate::planner::ultra_preset::is_profile_preset_plan(
            &c, &ultra
        ));
        c.action = crate::config::Action::UltraPlanRun(ultra.goal.clone());
        let mut planner = FixedPlanner(raw.clone());
        let mut client = FixedOperations {
            config: c.clone(),
            source,
            calls: Default::default(),
        };
        stages::start();
        // Start the real core phase on an explicitly provisioned workspace.
        // No prior setup/build success or final-app acceptance is fabricated.
        let (paths, _authority) = recovery_authority::begin(&c, &ultra).unwrap();
        let context = UltraRunContext::for_run(&root, &paths);
        let mut session = SessionSnapshot::new();
        let mut machine = pipeline::PhaseRun::start().unwrap();
        let (registered, _) = super::super::super::prepare(
            &mut planner,
            &c,
            &ultra,
            &ultra.phases[1],
            1,
            &NOOP_UI,
            &context,
            &session,
            &mut machine,
            None,
            None,
            &paths,
            true,
        )
        .unwrap()
        .unwrap();
        assert_eq!(build(&registered), build(&raw));
        assert_eq!(registered.steps[0], raw.steps[0]);
        let saved = std::fs::read_dir(crate::runtime_paths::plans_dir(&root))
            .unwrap()
            .map(|p| p.unwrap().path())
            .find(|p| p.extension().is_some_and(|e| e == "yaml"))
            .unwrap();
        assert_eq!(
            crate::planner::step_plan::parse_step_plan(&std::fs::read_to_string(saved).unwrap())
                .unwrap(),
            registered
        );
        let profile_report = crate::planner::verify::verify_step(&root, &registered.steps[0]);
        assert_eq!(profile_report.is_pass(), !case.starts_with("R4"));
        let result =
            crate::planner::runner::phase::step_plan_execution::run_step_plan_with_session_with_ui(
                &mut client,
                &mut session,
                &registered,
                &c,
                &NOOP_UI,
                false,
                "ultra-plan-run",
                Some("core-implementation"),
                Some(&ultra.goal),
            );
        let stage_log = stages::take();
        let log = events(&c);
        let commands: Vec<serde_json::Value> =
            std::fs::read_to_string(logs.join(case).join("commands.jsonl"))
                .unwrap_or_default()
                .lines()
                .map(|l| serde_json::from_str(l).unwrap())
                .collect();
        let builds: Vec<_> = commands
            .iter()
            .filter(|e| e["status"] == "finished" && e["command"] == "npm run build")
            .collect();
        let observation = json!({"case":case,"source_hash":source_hash,"stages":stage_log,"events":log,"commands":commands,"result":format!("{result:?}"),"calls":*client.calls.lock().unwrap(),"profile_report":format!("{profile_report:?}"),"registered":registered,"binary":{"version":env!("COMMANDAGENT_VERSION"),"path":std::env::current_exe().unwrap(),"sha256":format!("{:x}", Sha256::digest(std::fs::read(std::env::current_exe().unwrap()).unwrap()))}});
        std::fs::write(
            logs.join(case).join("observation.json"),
            serde_json::to_vec_pretty(&observation).unwrap(),
        )
        .unwrap();
        assert_eq!(result.is_ok(), case == "R1", "core build only: {case}");
        assert_eq!(
            stage_log
                .iter()
                .map(|s| s["stage"].as_str().unwrap())
                .collect::<Vec<_>>(),
            ["parsed", "augmented", "sanitized", "converted"]
        );
        for stage in &stage_log {
            let p: StepPlan = serde_json::from_value(stage["plan"].clone()).unwrap();
            assert_eq!(build(&p), build(&raw));
            assert_eq!(p.steps.last(), Some(build(&p)));
        }
        if matches!(case, "R1" | "R2") {
            assert_eq!(builds.len(), 1, "{case}: {result:?}; see observation.json");
            let b = builds[0];
            assert_eq!(b["hashes"]["src/app/page.tsx"], source_hash);
            assert_eq!(b["source_after"], source_hash);
            assert_eq!(b["step"]["step_id"], "verify-build");
            assert_eq!(b["step"]["phase_id"], "core-implementation");
            assert!(
                b["completed_steps"]
                    .as_array()
                    .unwrap()
                    .contains(&json!("implement-page"))
            );
            assert_eq!(b["NODE_ENV"], serde_json::Value::Null);
            assert_eq!(b["node"], "v24.1.0");
            for path in [
                "src/app/layout.tsx",
                "package.json",
                "package-lock.json",
                "tsconfig.json",
                "next.config.mjs",
            ] {
                assert_eq!(
                    b["hashes"][path],
                    format!(
                        "{:x}",
                        Sha256::digest(
                            std::fs::read(fixture_root.join(NEXTJS_PROFILE_ID).join(path)).unwrap()
                        )
                    ),
                    "frozen build input {path}"
                );
            }
            assert_eq!(b["next"], "14.2.35");
            assert_eq!(b["typescript"], "5.9.3");
            assert_eq!(b["exit"], if case == "R1" { 0 } else { 1 });
            let lifecycle = log
                .iter()
                .find(|e| {
                    e["event"] == "dependency_build_lifecycle" && e["step_id"] == "verify-build"
                })
                .expect("targeted build lifecycle");
            assert_eq!(lifecycle["before_attempted"], true);
            assert_eq!(
                lifecycle["final_status"],
                if case == "R1" { "passed" } else { "failed" }
            );
            if case == "R2" {
                assert!(
                    lifecycle["compile_errors"]
                        .to_string()
                        .contains("not assignable"),
                    "{lifecycle}"
                );
            }
            assert!(!log.iter().any(|e| e["event"] == "step_short_circuited" && e["step_id"] == "verify-build"));
        } else if case == "R3" {
            assert!(builds.is_empty());
            assert!(
                log.iter()
                    .any(|e| e["event"] == "dependency_build_lifecycle"
                        && e["step_id"] == "verify-build"
                        && e["final_status"] == "dependency_missing"
                        && e["before_attempted"] == false),
                "{result:?}; see observation.json"
            );
            assert!(!root.join("node_modules").exists());
        } else {
            assert!(builds.is_empty());
            assert!(
                result
                    .as_ref()
                    .unwrap_err()
                    .message
                    .contains("profile failure boundary")
            );
            assert!(!profile_report.command_failures.is_empty());
            assert!(
                !log.iter().any(|e| e["event"] == "step_short_circuited"
                    && e["step_id"] == "package-read-original")
            );
        }
        assert_eq!(
            format!(
                "{:x}",
                Sha256::digest(std::fs::read(root.join("src/app/page.tsx")).unwrap())
            ),
            source_hash,
            "source must remain frozen"
        );
    }
}
