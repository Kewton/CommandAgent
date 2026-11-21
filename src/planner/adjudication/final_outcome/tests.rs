use super::super::core::{
    final_acceptance_status_from_release_gate, next_action_from_gate_status,
    release_quality_from_gate_status,
};
use super::{final_acceptance_status_for_outcome, final_acceptance_unmet};
use crate::cli::Cli;
use crate::config::Config;
use crate::planner::runner::{run_step_plan, run_ultra_plan};
use crate::planner::step_plan::{PlanStep, StepPlan};
use crate::planner::ultra_plan::{UltraPhase, UltraPlan};
use crate::providers::{AssistantReply, ChatClient};
use crate::state::{ConversationMessage, ToolCall};
use crate::tools::registry::ToolSpec;
use clap::Parser;
use serde_json::Value;
use std::path::Path;

#[derive(Clone)]
struct Scripted(Vec<AssistantReply>);

impl ChatClient for Scripted {
    fn label(&self) -> &str {
        "scripted"
    }

    fn boxed_clone(&self) -> Box<dyn ChatClient> {
        Box::new(self.clone())
    }

    fn chat(
        &mut self,
        _model: &str,
        _messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        if self.0.is_empty() {
            anyhow::bail!("scripted client exhausted");
        }
        Ok(self.0.remove(0))
    }
}

fn reply(content: &str, tool_calls: Vec<ToolCall>) -> AssistantReply {
    AssistantReply {
        content: content.to_string(),
        tool_calls,
        prompt_tokens: None,
        completion_tokens: None,
    }
}

fn config(root: &Path, profile: &str) -> Config {
    let mut config = Config::from_cli(Cli::parse_from([
        "commandagent",
        "--yes",
        "--cwd",
        root.to_str().unwrap(),
        "--profile",
        profile,
    ]))
    .unwrap();
    config.eval_events_path = Some(root.join("events.jsonl"));
    config
}

fn latest_event(root: &Path, name: &str) -> Value {
    std::fs::read_to_string(root.join("events.jsonl"))
        .unwrap()
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .rfind(|event| event["event"] == name)
        .unwrap_or_else(|| panic!("missing event {name}"))
}

#[test]
fn outcome_status_table_matches_gate_status_only_when_the_report_passes() {
    for gate in ["pass", "not_applicable", "partial", "failed", "unknown"] {
        let derived = final_acceptance_status_from_release_gate(gate);
        assert_eq!(
            final_acceptance_status_for_outcome(derived, true),
            derived,
            "{gate}"
        );
        assert_eq!(
            final_acceptance_status_for_outcome(derived, false),
            "incomplete",
            "{gate}"
        );
    }
    assert_eq!(
        final_acceptance_status_for_outcome("full_success", true),
        "full_success"
    );
    assert_eq!(
        final_acceptance_status_for_outcome("partial", true),
        "partial"
    );
}

#[test]
fn unmet_final_is_incomplete_or_failed_only() {
    assert!(final_acceptance_unmet("incomplete"));
    assert!(final_acceptance_unmet("failed"));
    for status in ["full_success", "partial", "not_checked", ""] {
        assert!(!final_acceptance_unmet(status), "{status}");
    }
}

#[test]
fn quality_and_next_action_keep_existing_values_and_fail_unmet_final() {
    let cases = [
        ("pass", "full_success", "release_ready", "none"),
        ("not_applicable", "full_success", "release_ready", "none"),
        (
            "pass",
            "partial",
            "release_ready",
            "collect_missing_final_acceptance_evidence",
        ),
        (
            "partial",
            "full_success",
            "partial",
            "collect_missing_release_evidence_or_continue_release_recovery",
        ),
        (
            "partial",
            "incomplete",
            "partial",
            "collect_missing_release_evidence_or_continue_release_recovery",
        ),
        (
            "failed",
            "incomplete",
            "failed",
            "repair_release_gate_failure",
        ),
        (
            "pass",
            "incomplete",
            "failed",
            "repair_final_acceptance_failure",
        ),
        (
            "not_applicable",
            "incomplete",
            "failed",
            "repair_final_acceptance_failure",
        ),
        (
            "pass",
            "failed",
            "failed",
            "repair_final_acceptance_failure",
        ),
    ];
    for (gate, final_status, quality, next) in cases {
        assert_eq!(
            release_quality_from_gate_status(gate, final_status),
            quality,
            "{gate}/{final_status}"
        );
        assert_eq!(
            next_action_from_gate_status(gate, final_status),
            next,
            "{gate}/{final_status}"
        );
    }
}

#[test]
fn recorded_cli_q_run_maps_failed_report_to_incomplete() {
    let events: Vec<Value> = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/corpus/apps/issue846-final-acceptance-status-outcome/fixtures/events.jsonl"
    ))
    .lines()
    .map(|line| serde_json::from_str(line).unwrap())
    .collect();
    let named = |name: &str| events.iter().find(|event| event["event"] == name);
    let recorded = named("ultra_final_acceptance").expect("recorded final acceptance");
    let report_failed = named("ultra_final_acceptance_failed").is_some();
    let gate = recorded["release_gate_status"].as_str().unwrap();
    let derived = final_acceptance_status_from_release_gate(gate);

    let status = final_acceptance_status_for_outcome(derived, !report_failed);

    assert!(report_failed);
    assert_eq!(derived, "full_success");
    assert_eq!(status, "incomplete");
    assert_eq!(release_quality_from_gate_status(gate, status), "failed");
    assert_eq!(
        next_action_from_gate_status(gate, status),
        "repair_final_acceptance_failure"
    );
}

#[test]
fn plan_run_external_contract_failure_reports_incomplete_status() {
    let dir = tempfile::tempdir().unwrap();
    let contract = dir.path().join("contract.json");
    std::fs::write(&contract, r#"{"verify_commands": ["false"]}"#).unwrap();
    let mut cfg = config(dir.path(), "generic");
    cfg.completion_contract_path = Some(contract);
    let plan = StepPlan {
        goal: "Write a small note".to_string(),
        steps: Vec::<PlanStep>::new(),
    };

    let result = run_step_plan(&mut Scripted(Vec::new()), &plan, &cfg);

    assert!(result.is_err(), "{result:?}");
    let event = latest_event(dir.path(), "plan_final_contract");
    assert_eq!(event["release_gate_status"], "not_applicable");
    assert_eq!(event["final_acceptance_status"], "incomplete");
}

#[test]
fn failed_final_report_reports_incomplete_status_not_full_success() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("pyproject.toml"),
        "[project]\nname = \"app\"\n",
    )
    .unwrap();
    for package in ["app", "second_pkg"] {
        std::fs::create_dir_all(dir.path().join("src").join(package)).unwrap();
        std::fs::write(
            dir.path().join("src").join(package).join("main.py"),
            "print('value=7')\n",
        )
        .unwrap();
    }
    let step_plan = serde_json::to_string(&StepPlan {
        goal: "Create a command line tool".to_string(),
        steps: vec![PlanStep {
            id: "write".to_string(),
            kind: "implement".to_string(),
            expected_result: "pass".to_string(),
            instruction: "Create src/second_pkg/main.py.".to_string(),
            expected_paths: vec!["src/second_pkg/main.py".to_string()],
            verify: Vec::new(),
        }],
    })
    .unwrap();
    let write = ToolCall::new(
        "Write",
        serde_json::json!({"path": "src/second_pkg/main.py", "content": "print('value=7')\n"}),
    );
    let plan = UltraPlan {
        goal: "Create a command line tool".to_string(),
        profile: "python-cli".to_string(),
        style: "default".to_string(),
        intent: "create".to_string(),
        phases: ["first", "final"]
            .map(|id| UltraPhase {
                id: id.to_string(),
                prompt: "Create the tool".to_string(),
            })
            .to_vec(),
    };
    let cfg = config(dir.path(), "python-cli");

    let result = run_ultra_plan(
        &mut Scripted(vec![
            reply(&step_plan, Vec::new()),
            reply(&step_plan, Vec::new()),
        ]),
        &mut Scripted(vec![reply("", vec![write.clone()]), reply("", vec![write])]),
        &plan,
        &cfg,
    );

    assert!(result.is_err(), "{result:?}");
    let event = latest_event(dir.path(), "ultra_final_acceptance");
    assert_eq!(event["release_gate_status"], "pass");
    assert_eq!(event["runtime_acceptance_status"], "pass");
    assert_eq!(event["final_acceptance_status"], "incomplete");
    assert_ne!(event["assurance_level"], "full");
    assert_ne!(event["release_quality_completion"], "release_ready");
}
