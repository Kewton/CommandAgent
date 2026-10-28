//! #896 (IC-42): phase StepPlan admission keeps the single python-cli entry.
use std::collections::VecDeque;
use std::path::Path;
use std::sync::{Arc, Mutex};

use clap::Parser;
use serde_json::{Value, json};

use super::{EntryIdentity, admit, assess};
use crate::config::Config;
use crate::planner::adjudication::contract::IntentId;
use crate::planner::recovery_step_plan_binding::admission::Admission;
use crate::planner::step_plan::{PlanStep, StepPlan};
use crate::providers::{AssistantReply, ChatClient};
use crate::state::{ConversationMessage, ToolCall};
use crate::tools::registry::ToolSpec;

/// The recorded L-135 cli-q ultra goal names no Python file.
const RECORDED_GOAL: &str = "CSVファイルを読み込み、数値列の合計・平均・最大・最小を集計して表形式で標準出力するCLIツールをPythonで開発してください。";

fn phase_prompt(goal: &str, phase_id: &str, task: &str, changed: &[&str]) -> String {
    let changed = changed
        .iter()
        .map(|path| format!("  - {path}\n"))
        .collect::<String>();
    format!(
        "Original ultra goal: {goal}\nProfile: python-cli\nStyle: default\nIntent: create\nPhase id: {phase_id}\nPhase task: {task}\n\nPrior ultra context:\n- Created or changed paths:\n{changed}\nProfile generation rules:\n- Profile python-cli: create pyproject.toml plus exactly one src/<package>/main.py."
    )
}

fn step(id: &str, kind: &str, instruction: &str, paths: &[&str], verify: &[&str]) -> PlanStep {
    PlanStep {
        id: id.into(),
        kind: kind.into(),
        expected_result: "pass".into(),
        instruction: instruction.into(),
        expected_paths: paths.iter().map(|p| (*p).into()).collect(),
        verify: verify.iter().map(|v| (*v).into()).collect(),
    }
}

/// The shape of the recorded phase plans: an implement step that owns the
/// entry and a verify step that runs it, both aimed at `entry`.
fn feature_plan(goal: &str, entry: &str) -> StepPlan {
    StepPlan {
        goal: goal.into(),
        steps: vec![
            step(
                "implement-feature",
                "implement",
                &format!(
                    "Implement the feature in {entry}: read CSV rows from a path argument or stdin and print a table of per-column results."
                ),
                &[entry],
                &[],
            ),
            step(
                "verify-syntax",
                "verify",
                "Verify the Python syntax using compileall.",
                &[],
                &["python3 -m compileall -q src"],
            ),
        ],
    }
}

fn workspace(entries: &[&str]) -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    for entry in entries {
        let path = root.path().join(entry);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "print('ok')\n").unwrap();
    }
    root
}

fn repairable(outcome: EntryIdentity) -> String {
    match outcome {
        EntryIdentity::Repairable(reason) => reason,
        other => panic!("expected Repairable, got {other:?}"),
    }
}

fn config(root: &Path, profile: &str) -> Config {
    let mut c =
        Config::from_cli(crate::cli::Cli::parse_from(["commandagent", "--ux-demo"])).unwrap();
    c.workspace_root = root.to_path_buf();
    c.profile = profile.into();
    c.profile_explicit = true;
    c.offline = true;
    c.yes = true;
    c.max_iterations = 4;
    c.intent_override = Some(IntentId::Create);
    c.eval_events_path = Some(root.join(".commandagent/runs/issue896/events.jsonl"));
    std::fs::create_dir_all(c.eval_events_path.as_ref().unwrap().parent().unwrap()).unwrap();
    c
}

fn events(c: &Config) -> Vec<Value> {
    std::fs::read_to_string(c.eval_events_path.as_ref().unwrap())
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

/// Every regular file under `root/src`, so a test can see that nothing ran.
fn src_files(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![root.join("src")];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                out.push(path.strip_prefix(root).unwrap().display().to_string());
            }
        }
    }
    out.sort();
    out
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
            ..Default::default()
        }
    }
}

impl ChatClient for Replay {
    fn label(&self) -> &str {
        "issue896-replay"
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
            .ok_or_else(|| anyhow::anyhow!("issue896 replay exhausted"))
    }
}

fn proposal(plan: &StepPlan) -> AssistantReply {
    AssistantReply::text(serde_json::to_string(plan).unwrap())
}

fn generate(c: &Config, goal: &str, planner: &mut Replay) -> anyhow::Result<StepPlan> {
    crate::planner::runner::generate_step_plan_with_ui_for_phase(
        planner,
        goal,
        c,
        &crate::tui::NOOP_UI,
        Some("implement-csv-logic"),
        false,
        false,
    )
}

fn admissions(c: &Config) -> Vec<Value> {
    events(c)
        .into_iter()
        .filter(|event| event["event"] == "recovery_verifier_plan_admission")
        .collect()
}

#[test]
fn issue896_ic42_phase2_aggregator_entry_is_returned_and_canonical_retry_is_admitted() {
    let root = workspace(&["src/app/main.py"]);
    let goal = phase_prompt(
        RECORDED_GOAL,
        "implement-csv-logic",
        "Implement the core logic in src/csv_analyzer/main.py to read CSV and calculate sum, mean, max, and min.",
        &["pyproject.toml", "src/app/main.py"],
    );
    let mut recorded = feature_plan(&goal, "src/csv_aggregator/main.py");
    recorded.steps.push(step(
        "verify-execution",
        "verify",
        "Verify that the tool runs with the sample CSV file.",
        &[],
        &["python3 src/csv_aggregator/main.py test_data.csv"],
    ));
    let reason = repairable(assess(root.path(), &recorded));
    assert!(
        reason.contains("the existing entry is src/app/main.py"),
        "{reason}"
    );
    assert!(reason.contains("src/csv_aggregator/main.py"), "{reason}");
    assert!(reason.contains("verification command"), "{reason}");
    assert!(reason.contains("app.main:main"), "{reason}");
    assert!(
        reason.contains("Keep every original feature requirement"),
        "{reason}"
    );
    for field in ["expected_paths", "verify", "instruction"] {
        assert!(reason.contains(field), "{field}: {reason}");
    }

    let mut corrected = feature_plan(&goal, "src/app/main.py");
    corrected.steps.push(step(
        "verify-execution",
        "verify",
        "Verify that the tool runs with the sample CSV file.",
        &[],
        &["python3 src/app/main.py test_data.csv"],
    ));
    assert_eq!(assess(root.path(), &corrected), EntryIdentity::Admitted);
}

#[test]
fn issue896_ic42_phase3_analyzer_entry_is_returned_and_canonical_retry_is_admitted() {
    let root = workspace(&["src/app/main.py"]);
    let goal = phase_prompt(
        RECORDED_GOAL,
        "implement-table-output",
        "Format the aggregated results as a table in src/csv_analyzer/main.py.",
        &["pyproject.toml", "src/app/main.py", "test_data.csv"],
    );
    let reason = repairable(assess(
        root.path(),
        &feature_plan(&goal, "src/csv_analyzer/main.py"),
    ));
    assert!(reason.contains("src/app/main.py"), "{reason}");
    assert!(reason.contains("src/csv_analyzer/main.py"), "{reason}");
    assert_eq!(
        assess(root.path(), &feature_plan(&goal, "src/app/main.py")),
        EntryIdentity::Admitted
    );
}

#[test]
fn issue896_zero_entries_admit_one_consistent_entry_and_return_two() {
    let root = workspace(&[]);
    let goal = phase_prompt("Build a tool", "project-setup", "Create the CLI.", &[]);
    assert_eq!(
        assess(root.path(), &feature_plan(&goal, "src/alpha/main.py")),
        EntryIdentity::Admitted
    );
    let mut two = feature_plan(&goal, "src/alpha/main.py");
    two.steps.push(step(
        "second-entry",
        "implement",
        "Create the second command.",
        &["src/beta/main.py"],
        &[],
    ));
    let reason = repairable(assess(root.path(), &two));
    assert!(reason.contains("2 different entries"), "{reason}");
    assert!(reason.contains("src/alpha/main.py"), "{reason}");
    assert!(reason.contains("src/beta/main.py"), "{reason}");
    // A plan that names no entry at all is not this check's concern.
    let readme = StepPlan {
        goal,
        steps: vec![step(
            "docs",
            "implement",
            "Write usage.",
            &["README.md"],
            &[],
        )],
    };
    assert_eq!(assess(root.path(), &readme), EntryIdentity::Admitted);
}

#[test]
fn issue896_multiple_existing_entries_are_neither_pinned_nor_stopped() {
    let root = workspace(&["src/alpha/main.py", "src/beta/main.py"]);
    let goal = phase_prompt("Build a tool", "add-output", "Add output.", &[]);
    let c = config(root.path(), "python-cli");
    // No winner is chosen: every entry, and a third one, passes admission; the
    // final `entry_cardinality` gate keeps reporting the failure (pinned in
    // tests/python_cli_phase_entry_identity.rs and final_outcome tests).
    for entry in ["src/alpha/main.py", "src/beta/main.py", "src/gamma/main.py"] {
        let plan = feature_plan(&goal, entry);
        assert_eq!(
            assess(root.path(), &plan),
            EntryIdentity::Admitted,
            "{entry}"
        );
        assert!(admit(&c, Some("add-output"), &plan).is_ok(), "{entry}");
        let admission = Admission::default();
        assert!(
            admission.finish(&c, Some("add-output"), plan).is_ok(),
            "{entry}"
        );
    }
}

#[test]
fn issue896_pyproject_distribution_name_does_not_choose_the_entry() {
    let root = workspace(&["src/alpha/main.py"]);
    std::fs::write(
        root.path().join("pyproject.toml"),
        "[project]\nname = \"beta-tool\"\n\n[project.scripts]\nbeta-tool = \"alpha.main:main\"\n",
    )
    .unwrap();
    let goal = phase_prompt("Build a tool", "add-output", "Add output.", &[]);
    assert_eq!(
        assess(root.path(), &feature_plan(&goal, "src/alpha/main.py")),
        EntryIdentity::Admitted
    );
    let reason = repairable(assess(
        root.path(),
        &feature_plan(&goal, "src/beta_tool/main.py"),
    ));
    assert!(
        reason.contains("the existing entry is src/alpha/main.py"),
        "{reason}"
    );
}

#[test]
fn issue896_explicit_user_name_conflicting_with_existing_entry_is_returned_as_ambiguous() {
    let root = workspace(&["src/alpha/main.py"]);
    let goal = phase_prompt(
        "Create beta.py that echoes input",
        "add-output",
        "Add output.",
        &[],
    );
    let c = config(root.path(), "python-cli");
    for entry in ["src/alpha/main.py", "src/beta/main.py"] {
        let plan = feature_plan(&goal, entry);
        let reason = repairable(assess(root.path(), &plan));
        assert!(reason.contains("ambiguous"), "{reason}");
        assert!(
            reason.contains("original goal names src/beta/main.py"),
            "{reason}"
        );
        assert!(
            reason.contains("only entry is src/alpha/main.py"),
            "{reason}"
        );
        let error = admit(&c, Some("add-output"), &plan).unwrap_err();
        let class = error
            .downcast_ref::<crate::planner::recovery_inspection::verifier_obligations::BindingFailure>()
            .map(|failure| failure.class);
        assert_eq!(
            class,
            Some(crate::planner::recovery_inspection::verifier_obligations::FailureClass::ProposalRepairable),
            "{error}"
        );
    }
    // A matching explicit name is the normal case.
    let goal = phase_prompt(
        "Create alpha.py that echoes input",
        "add-output",
        "Add output.",
        &[],
    );
    assert_eq!(
        assess(root.path(), &feature_plan(&goal, "src/alpha/main.py")),
        EntryIdentity::Admitted
    );
}

#[test]
fn issue896_phase_task_and_context_python_paths_are_not_rename_requests() {
    let root = workspace(&["src/alpha/main.py"]);
    // The phase task and the prior context name other files; only the
    // original goal can carry a user's naming request.
    let goal = phase_prompt(
        "Build a tool",
        "add-output",
        "Implement the logic in src/beta/main.py, like gamma.py in the example.",
        &["src/alpha/main.py", "src/delta/main.py"],
    );
    let plan = feature_plan(&goal, "src/alpha/main.py");
    assert_eq!(assess(root.path(), &plan), EntryIdentity::Admitted);

    // The setup fallback embeds the whole phase prompt in its instruction.
    let mut quoted = plan.clone();
    quoted.steps[0].instruction = format!(
        "Create one coherent python-cli package scaffold for this setup phase: {}. Keep src/alpha/main.py.",
        super::compact(&goal)
    );
    assert_eq!(assess(root.path(), &quoted), EntryIdentity::Admitted);

    // Host guidance derived from the phase prompt is host text as well.
    let mut guided = plan;
    guided.steps[0].instruction = format!(
        "{}\n\nKeep src/alpha/main.py.",
        crate::planner::profile::DomainProfile::guidance(&super::PythonCliProfile, &goal).unwrap()
    );
    assert_eq!(assess(root.path(), &guided), EntryIdentity::Admitted);
}

#[test]
fn issue896_dot_slash_and_backslash_spellings_are_the_same_entry() {
    let root = workspace(&["src/alpha/main.py"]);
    let goal = phase_prompt("Build a tool", "add-output", "Add output.", &[]);
    for same in [
        "./src/alpha/main.py",
        "src\\alpha\\main.py",
        "././src/alpha/main.py",
    ] {
        assert_eq!(
            assess(root.path(), &feature_plan(&goal, same)),
            EntryIdentity::Admitted,
            "{same}"
        );
    }
    for other in ["./src/beta/main.py", "src\\beta\\main.py"] {
        let reason = repairable(assess(root.path(), &feature_plan(&goal, other)));
        assert!(reason.contains("src/beta/main.py"), "{other}: {reason}");
    }
}

#[test]
fn issue896_helper_modules_and_normal_changes_to_the_same_entry_are_admitted() {
    let root = workspace(&["src/alpha/main.py"]);
    let goal = phase_prompt("Build a tool", "add-output", "Add output.", &[]);
    let plan = StepPlan {
        goal,
        steps: vec![
            step(
                "helpers",
                "implement",
                "Add src/alpha/table.py and src/beta/util.py helpers, then call them from src/alpha/main.py. Register alpha.main:main as the script.",
                &[
                    "src/alpha/table.py",
                    "src/beta/util.py",
                    "src/alpha/main.py",
                    "pyproject.toml",
                ],
                &[],
            ),
            step(
                "verify",
                "verify",
                "Run the CLI.",
                &[],
                &[
                    "python3 src/alpha/main.py data.csv",
                    "python3 -m alpha.main data.csv",
                    "python3 -m alpha.table",
                    "python3 -m compileall -q src",
                ],
            ),
        ],
    };
    assert_eq!(assess(root.path(), &plan), EntryIdentity::Admitted);
}

#[test]
fn issue896_instruction_only_and_verify_only_other_entries_are_returned() {
    let root = workspace(&["src/alpha/main.py"]);
    let goal = phase_prompt("Build a tool", "add-output", "Add output.", &[]);
    let cases = [
        (
            "instruction path",
            step(
                "impl",
                "implement",
                "Move the CLI into src/beta/main.py.",
                &["src/alpha/main.py"],
                &[],
            ),
        ),
        (
            "instruction script target",
            step(
                "impl",
                "implement",
                "Point [project.scripts] tool at beta.main:main.",
                &["pyproject.toml"],
                &[],
            ),
        ),
        (
            "verify path",
            step(
                "check",
                "verify",
                "Run the CLI.",
                &[],
                &["python3 ./src/beta/main.py data.csv"],
            ),
        ),
        (
            "verify module",
            step(
                "check",
                "verify",
                "Run the CLI.",
                &[],
                &["PYTHONPATH=src python3 -m beta.main data.csv"],
            ),
        ),
    ];
    for (label, offending) in cases {
        let mut plan = feature_plan(&goal, "src/alpha/main.py");
        plan.steps.push(offending);
        let reason = repairable(assess(root.path(), &plan));
        assert!(reason.contains("src/beta/main.py"), "{label}: {reason}");
        assert!(reason.contains("src/alpha/main.py"), "{label}: {reason}");
    }
    // Prose that merely mentions a module-like word is not a script target.
    let mut prose = feature_plan(&goal, "src/alpha/main.py");
    prose.steps[0]
        .instruction
        .push_str(" Call cli.main from the entry.");
    assert_eq!(assess(root.path(), &prose), EntryIdentity::Admitted);
}

fn recorded_phase2() -> (tempfile::TempDir, Config, String) {
    let root = workspace(&["src/app/main.py"]);
    std::fs::write(
        root.path().join("pyproject.toml"),
        "[project]\nname = \"csv-aggregator\"\nversion = \"0.1.0\"\n\n[project.scripts]\ncsv-aggregator = \"app.main:main\"\n",
    )
    .unwrap();
    let c = config(root.path(), "python-cli");
    let goal = phase_prompt(
        RECORDED_GOAL,
        "implement-csv-logic",
        "Implement the core logic in src/csv_analyzer/main.py to read CSV and calculate sum, mean, max, and min.",
        &["pyproject.toml", "src/app/main.py"],
    );
    (root, c, goal)
}

#[test]
fn issue896_scripted_planner_three_conflicts_stop_in_existing_budget_without_writes() {
    let (root, c, goal) = recorded_phase2();
    let before = src_files(root.path());
    let wrong = feature_plan(&goal, "src/csv_aggregator/main.py");
    let mut planner = Replay::new(vec![proposal(&wrong); 3]);
    let mut execution = Replay::default();
    let result = generate(&c, &goal, &mut planner)
        .and_then(|plan| crate::planner::runner::run_step_plan(&mut execution, &plan, &c));
    let error = format!("{:#}", result.unwrap_err());
    assert!(
        error.contains("existing 3-attempt planner budget"),
        "{error}"
    );
    assert!(error.contains("src/app/main.py"), "{error}");
    assert_eq!(planner.requests.lock().unwrap().len(), 3);
    assert!(
        execution.requests.lock().unwrap().is_empty(),
        "no execution turn"
    );
    assert_eq!(
        src_files(root.path()),
        before,
        "no Write reached the workspace"
    );
    let statuses = admissions(&c)
        .iter()
        .map(|event| {
            (
                event["status"].as_str().unwrap().to_owned(),
                event["classification"].as_str().unwrap().to_owned(),
                event["recovery_budget_changed"].clone(),
            )
        })
        .collect::<Vec<_>>();
    let repairable = || "proposal_repairable".to_owned();
    assert_eq!(
        statuses,
        vec![
            ("retry".to_owned(), repairable(), json!(false)),
            ("retry".to_owned(), repairable(), json!(false)),
            ("exhausted".to_owned(), repairable(), json!(false)),
        ]
    );
    // The feedback names the existing entry and keeps the original proposal.
    let feedback = serde_json::to_string(&planner.requests.lock().unwrap()[1]).unwrap();
    assert!(
        feedback.contains("the existing entry is src/app/main.py"),
        "{feedback}"
    );
    assert!(feedback.contains("read CSV rows"), "{feedback}");
}

#[test]
fn issue896_scripted_planner_second_proposal_correction_proceeds_to_execution() {
    let (root, c, goal) = recorded_phase2();
    let wrong = feature_plan(&goal, "src/csv_aggregator/main.py");
    let corrected = feature_plan(&goal, "src/app/main.py");
    let mut planner = Replay::new(vec![proposal(&wrong), proposal(&corrected)]);
    let plan = generate(&c, &goal, &mut planner).unwrap();
    assert_eq!(planner.requests.lock().unwrap().len(), 2);
    assert!(
        plan.steps
            .iter()
            .any(|step| step.expected_paths.iter().any(|p| p == "src/app/main.py")),
        "{plan:?}"
    );
    assert_eq!(assess(root.path(), &plan), EntryIdentity::Admitted);
    let statuses = admissions(&c)
        .iter()
        .map(|event| event["status"].as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(statuses, vec!["retry".to_owned()]);

    let content = "import sys\n\ndef main():\n    print(sys.stdin.read().upper() or 'ok')\n\nif __name__ == '__main__':\n    main()\n";
    let mut execution = Replay::new(vec![
        AssistantReply {
            content: String::new(),
            tool_calls: vec![ToolCall::new(
                "Write",
                json!({"path":"src/app/main.py","content":content}),
            )],
            prompt_tokens: None,
            completion_tokens: None,
        },
        AssistantReply::text("Implemented the feature in src/app/main.py"),
    ]);
    let _ = crate::planner::runner::run_step_plan(&mut execution, &plan, &c);
    assert!(
        !execution.requests.lock().unwrap().is_empty(),
        "the admitted plan reaches execution"
    );
    assert_eq!(
        std::fs::read_to_string(root.path().join("src/app/main.py")).unwrap(),
        content
    );
    assert_eq!(src_files(root.path()), vec!["src/app/main.py".to_owned()]);
}

#[test]
fn issue896_finish_rejects_fallback_and_last_valid_plans_that_change_the_entry() {
    let (root, c, goal) = recorded_phase2();
    let admission = Admission::default();
    let wrong = feature_plan(&goal, "src/csv_aggregator/main.py");
    let error = format!(
        "{:#}",
        admission
            .finish(&c, Some("implement-csv-logic"), wrong)
            .unwrap_err()
    );
    assert!(
        error.contains("the existing entry is src/app/main.py"),
        "{error}"
    );

    // The python-cli setup fallback derives its package from the phase prompt.
    let setup_goal = phase_prompt(
        RECORDED_GOAL,
        "project-setup",
        "Set up the project scaffold in src/csv_analyzer/main.py.",
        &["src/app/main.py"],
    );
    let fallback =
        crate::planner::profile_preset::python_cli_setup_fallback(root.path(), &setup_goal)
            .expect("setup fallback");
    assert!(
        fallback.steps[0]
            .expected_paths
            .iter()
            .any(|p| p == "src/csv_analyzer/main.py"),
        "{fallback:?}"
    );
    let error = format!(
        "{:#}",
        admission
            .finish(&c, Some("project-setup"), fallback)
            .unwrap_err()
    );
    assert!(error.contains("src/csv_analyzer/main.py"), "{error}");

    let kept = feature_plan(&goal, "src/app/main.py");
    assert!(
        admission
            .finish(&c, Some("implement-csv-logic"), kept)
            .is_ok()
    );
}

#[test]
fn issue896_deterministic_template_goes_through_the_same_check() {
    let (root, c, _) = recorded_phase2();
    let manifest = crate::planner::profiles::python_cli::manifest::get();
    let setup_phase = manifest.plan.phases.first().unwrap().id.clone();
    let prompt = phase_prompt(
        "Build a tool",
        &setup_phase,
        "Create the scaffold for src/beta/main.py.",
        &["src/app/main.py"],
    );
    let template = crate::planner::python_cli_plan_synthesis::deterministic_step_plan(
        &prompt,
        root.path(),
        &prompt,
    )
    .expect("setup template");
    let mut plan = template.plan;
    assert!(
        plan.steps
            .iter()
            .any(|s| s.expected_paths.iter().any(|p| p == "src/beta/main.py")),
        "{plan:?}"
    );
    let model = plan.clone();
    let mut admission = Admission::default();
    admission.strengthen(&c, &mut plan);
    // The deterministic path bails on any Retry, so either outcome stops it.
    let result = admission.check(&c, Some(&setup_phase), &model, &mut plan, 1);
    let stopped = match result {
        Err(_) => true,
        Ok(crate::planner::recovery_step_plan_binding::admission::Decision::Retry(feedback)) => {
            assert!(feedback.contains("src/app/main.py"), "{feedback}");
            true
        }
        Ok(crate::planner::recovery_step_plan_binding::admission::Decision::Ready(_)) => false,
    };
    assert!(stopped, "the template must not bypass the entry check");
}

fn bind_recovery(c: &mut Config) -> crate::planner::recovery_contract_authority::RunAuthorityGuard {
    let guard = crate::planner::recovery_contract_authority::begin_run(c);
    let path = crate::planner::completion_contract_path::generated_path(
        &c.workspace_root,
        c.eval_events_path.as_deref(),
        "completion-contract-ultra-plan-run.json",
    );
    std::fs::write(
        &path,
        json!({"goal":"Build a tool", "profile":"python-cli", "required_paths":["src/app/main.py"], "verify_commands":["python3 -m compileall -q src"]})
            .to_string(),
    )
    .unwrap();
    crate::planner::recovery_contract_authority::record_generated_contract(
        c,
        "ultra-plan-run",
        &path,
    );
    c.completion_contract_path = Some(path);
    crate::planner::recovery_inspection::bind_context(
        c,
        c,
        Some("create"),
        &crate::planner::repair::RecoveryHandoff {
            original_goal: "Build a tool".into(),
            failure_kind: "step_verification_failed".into(),
            failure_evidence: vec!["entry failed".into()],
            ..Default::default()
        },
        None,
    )
    .unwrap();
    guard
}

#[test]
fn issue896_other_profiles_other_intents_recovery_and_unphased_plans_are_out_of_scope() {
    let root = workspace(&["src/app/main.py"]);
    let goal = phase_prompt("Build a tool", "add-output", "Add output.", &[]);
    let wrong = feature_plan(&goal, "src/beta/main.py");

    let c = config(root.path(), "python-cli");
    assert!(admit(&c, Some("add-output"), &wrong).is_err());
    assert!(admit(&c, None, &wrong).is_ok(), "only phase plans");
    for profile in ["generic", "nextjs", "data"] {
        let c = config(root.path(), profile);
        assert!(admit(&c, Some("add-output"), &wrong).is_ok(), "{profile}");
    }
    let mut fix = config(root.path(), "python-cli");
    fix.intent_override = Some(IntentId::Fix);
    assert!(admit(&fix, Some("add-output"), &wrong).is_ok(), "fix");

    let recovery_root = workspace(&["src/app/main.py"]);
    let mut recovery = config(recovery_root.path(), "python-cli");
    let _guard = bind_recovery(&mut recovery);
    assert!(
        crate::planner::recovery_inspection::has_origin(&recovery).unwrap(),
        "recovery origin is bound"
    );
    assert!(
        admit(&recovery, Some("add-output"), &wrong).is_ok(),
        "recovery"
    );
}
