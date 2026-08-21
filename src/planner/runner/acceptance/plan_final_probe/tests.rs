use std::path::Path;

use clap::Parser;

use crate::cli::Cli;
use crate::config::Config;
use crate::planner::profiles::python_cli::{self, runtime};
use crate::planner::step_plan::{PlanStep, StepPlan};
use crate::providers::{AssistantReply, ChatClient};
use crate::state::ConversationMessage;
use crate::tools::registry::ToolSpec;

const CORPUS_FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/corpus/apps/test0725_cli_profile_contract/fixtures/",
    "python-cli-plan-run-full.json"
);

#[derive(Clone)]
struct NoChatClient;

impl ChatClient for NoChatClient {
    fn label(&self) -> &str {
        "no-chat"
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
        anyhow::bail!("verify-only plan should short-circuit before provider chat")
    }
}

#[test]
fn python_cli_plan_run_binds_full_probe_to_event_summary_and_headless_summary() {
    let dir = tempfile::tempdir().unwrap();
    write_cli_fixture(dir.path(), passing_cli());
    let cfg = cli_config(dir.path());
    let mut client = NoChatClient;

    let result = crate::planner::runner::run_step_plan(&mut client, &verify_plan(), &cfg).unwrap();

    assert_eq!(result, "plan-run complete: 1 steps");
    let assurance: runtime::CliCheckSummary =
        serde_json::from_slice(&std::fs::read(dir.path().join(runtime::EVIDENCE_PATH)).unwrap())
            .unwrap();
    assert_eq!(assurance.assurance, runtime::CliAssurance::Full);
    assert!(
        assurance
            .evidence
            .checks
            .values()
            .all(|status| *status == runtime::CheckStatus::Pass)
    );

    let plan_final = latest_event(&cfg, "plan_final_contract");
    let expected: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(CORPUS_FIXTURE).unwrap()).unwrap();
    for key in [
        "profile",
        "runtime_acceptance_status",
        "final_acceptance_status",
        "release_gate_status",
        "assurance_level",
        "assurance_reason",
        "profile_behavior_probe_status",
        "profile_behavior_probe_evidence_path",
    ] {
        assert_eq!(
            plan_final[key], expected["plan_final_contract"][key],
            "{key}"
        );
    }
    assert_eq!(
        latest_event(&cfg, "profile_behavior_probe")["status"],
        "pass"
    );

    let projection = crate::emit_direct_command_stop_with_status(
        &cfg,
        "--plan-run",
        &Ok(()),
        crate::DirectCommandStatus::Completed,
    );
    assert_eq!(projection.assurance_level, "full");
    assert_eq!(projection.final_acceptance, "full_success");
    let summary = std::fs::read_to_string(dir.path().join("summary.md")).unwrap();
    assert!(summary.contains("Assurance: full"), "{summary}");
    assert!(!summary.contains("cli_probe_not_run"), "{summary}");

    let headless = serde_json::from_str::<serde_json::Value>(&crate::headless_summary::render(
        &crate::headless_summary::Source::from_config(&cfg, None),
    ))
    .unwrap();
    assert_eq!(
        headless["assurance"],
        expected["headless_summary"]["assurance"]
    );
    assert_eq!(headless["verdict"], expected["headless_summary"]["verdict"]);
}

#[test]
fn python_cli_plan_run_failed_probe_cannot_earn_full_assurance() {
    let dir = tempfile::tempdir().unwrap();
    write_cli_fixture(dir.path(), "print('value=7')\n");
    let cfg = cli_config(dir.path());
    let mut client = NoChatClient;

    let error = crate::planner::runner::run_step_plan(&mut client, &verify_plan(), &cfg)
        .unwrap_err()
        .to_string();

    assert!(error.contains("plan final contract failed"), "{error}");
    let assurance: runtime::CliCheckSummary =
        serde_json::from_slice(&std::fs::read(dir.path().join(runtime::EVIDENCE_PATH)).unwrap())
            .unwrap();
    assert_eq!(assurance.assurance, runtime::CliAssurance::Failed);
    let plan_final = latest_event(&cfg, "plan_final_contract");
    assert_eq!(plan_final["profile_behavior_probe_status"], "failed");
    assert_eq!(plan_final["runtime_acceptance_status"], "failed");
    assert_eq!(plan_final["release_gate_status"], "failed");
    assert_eq!(plan_final["final_acceptance_status"], "incomplete");
    assert_eq!(plan_final["assurance_level"], "failed");
    assert_ne!(plan_final["assurance_level"], "full");
}

fn write_cli_fixture(root: &Path, script: &str) {
    std::fs::create_dir_all(root.join("cli")).unwrap();
    std::fs::write(root.join("cli/main.py"), script).unwrap();
    std::fs::write(
        root.join("README.md"),
        "## Usage\n\n```console\n$ python3 cli/main.py sample.csv\nvalue=7\n```\n",
    )
    .unwrap();
    python_cli::complete_scaffold(
        root,
        &["pyproject.toml".into(), "src/anvil_app/main.py".into()],
    )
    .unwrap();
}

fn passing_cli() -> &'static str {
    "import argparse\n\
p = argparse.ArgumentParser()\n\
p.add_argument('input', nargs='?')\n\
p.parse_args()\n\
print('value=7')\n"
}

fn verify_plan() -> StepPlan {
    StepPlan {
        goal: "Create a deterministic command line tool".to_string(),
        steps: vec![PlanStep {
            id: "verify-cli".to_string(),
            kind: "verify".to_string(),
            expected_result: "pass".to_string(),
            instruction: "Verify the completed Python CLI".to_string(),
            expected_paths: vec![
                "pyproject.toml".to_string(),
                "src/anvil_app/main.py".to_string(),
                "cli/main.py".to_string(),
                "README.md".to_string(),
            ],
            verify: vec!["python3 -m compileall -q src cli".to_string()],
        }],
    }
}

fn cli_config(root: &Path) -> Config {
    let mut config = Config::from_cli(Cli::parse_from([
        "commandagent",
        "--cwd",
        root.to_str().unwrap(),
        "--profile",
        "python-cli",
    ]))
    .unwrap();
    config.eval_events_path = Some(root.join("events.jsonl"));
    config
}

fn latest_event(config: &Config, event_name: &str) -> serde_json::Value {
    std::fs::read_to_string(config.eval_events_path.as_ref().unwrap())
        .unwrap()
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .rev()
        .find(|event| event["event"] == event_name)
        .unwrap_or_else(|| panic!("missing event {event_name}"))
}
