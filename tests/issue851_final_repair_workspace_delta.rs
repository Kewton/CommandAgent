//! Issue #851: the final-acceptance repair judges "did the workspace change"
//! from before/after content fingerprints that include deletions and `.py`,
//! so a Bash `rm` that fixes the entry cardinality is re-verified.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use commandagent::config::{
    Action, Config, ConfigFieldSources, NarrationMode, OllamaThink, OpenAiApi, PlanPreset,
    PromptLayout, Provider,
};
use commandagent::planner::run_ultra_plan_with_ui;
use commandagent::planner::ultra_plan::{UltraPhase, UltraPlan};
use commandagent::providers::{AssistantReply, ChatClient};
use commandagent::state::{ConversationMessage, ToolCall};
use commandagent::tools::registry::ToolSpec;
use commandagent::tui::status::UiStatus;
use commandagent::tui::{InteractionUi, UiGuard};
use serde_json::{Value, json};

const GOAL: &str = "Build a Python CLI that reads a CSV file path argument and prints sum, average, max, and min for numeric columns.";

const VALID_CLI: &str = r#"#!/usr/bin/env python3
import csv
import sys
from pathlib import Path
def fmt(value: float) -> str:
    if value.is_integer():
        return str(int(value))
    return f"{value:.3f}".rstrip("0").rstrip(".")
def main() -> None:
    if len(sys.argv) != 2:
        print("usage: csv-stats <file>", file=sys.stderr)
        raise SystemExit(2)
    path = Path(sys.argv[1])
    if not path.is_file():
        print(f"missing file: {path}", file=sys.stderr)
        raise SystemExit(1)
    with path.open(newline="") as handle:
        rows = list(csv.DictReader(handle))
    numeric = {}
    for column in (rows[0].keys() if rows else []):
        values = []
        for row in rows:
            try:
                values.append(float(row[column]))
            except ValueError:
                pass
        if values:
            numeric[column] = (sum(values), sum(values) / len(values), max(values), min(values))
    if not numeric:
        print("no numeric columns", file=sys.stderr)
        raise SystemExit(1)
    print("column | sum | average | max | min")
    for column in sorted(numeric):
        total, average, maximum, minimum = numeric[column]
        print(f"{column} | {fmt(total)} | {fmt(average)} | {fmt(maximum)} | {fmt(minimum)}")

if __name__ == "__main__":
    main()
"#;

/// Valid syntax, but the output never depends on the input file.
const CONSTANT_CLI: &str = r#"#!/usr/bin/env python3
def main() -> None:
    print("column | sum | average | max | min")
    print("a | 1 | 1 | 1 | 1")

if __name__ == "__main__":
    main()
"#;

const PYPROJECT: &str = r#"[project]
name = "csv-stats"
version = "0.1.0"

[project.scripts]
csv-stats = "csv_stats.main:main"
"#;

const FAKE_SECRET: &str = "FAKE_SECRET_VALUE_851";

fn config(root: PathBuf, events: PathBuf) -> Config {
    Config {
        workspace_root: root.clone(),
        state_dir: root.join("state"),
        eval_events_path: Some(events),
        completion_contract_path: None,
        yes: true,
        offline: false,
        context_budget: 1000,
        model: "m".to_string(),
        provider: Provider::Ollama,
        tool_protocol: None,
        openai_api: OpenAiApi::ChatCompletions,
        prompt_layout: PromptLayout::Stable,
        plan_preset: PlanPreset::None,
        intent_override: None,
        planner_model: "pm".to_string(),
        planner_provider: Provider::Gemini,
        planner_think: Some(OllamaThink::False),
        classifier_model: "pm".to_string(),
        classifier_provider: Provider::Gemini,
        openai_compatible: None,
        ollama_host: "http://localhost:11434".to_string(),
        ollama_think: None,
        lm_studio_host: "http://localhost:1234".to_string(),
        num_predict: 100,
        max_iterations: 4,
        recovery_plan_auto_runs: 0,
        chat_timeout_secs: 1,
        chat_timeout_source: "override:test".to_string(),
        field_sources: ConfigFieldSources::default(),
        chat_retries: 1,
        stream: false,
        resume: None,
        fresh_session: false,
        no_footer: false,
        narration: NarrationMode::Normal,
        profile: "python-cli".to_string(),
        profile_explicit: false,
        profile_inference: None,
        style: "default".to_string(),
        action: Action::Repl,
    }
}

const REPAIR_MARKER: &str = "Repair the final acceptance failure";

/// Phase replies are consumed in order until a request carries the final
/// acceptance repair prompt; from then on the repair replies are used.
#[derive(Clone)]
struct ScriptedClient {
    label: &'static str,
    phase_replies: Arc<Mutex<Vec<AssistantReply>>>,
    repair_replies: Arc<Mutex<Vec<AssistantReply>>>,
    phase_fallback: AssistantReply,
}

impl ScriptedClient {
    fn new(
        label: &'static str,
        phase_replies: Vec<AssistantReply>,
        repair_replies: Vec<AssistantReply>,
        phase_fallback: AssistantReply,
    ) -> Self {
        Self {
            label,
            phase_fallback,
            phase_replies: Arc::new(Mutex::new(phase_replies)),
            repair_replies: Arc::new(Mutex::new(repair_replies)),
        }
    }
}

impl ChatClient for ScriptedClient {
    fn label(&self) -> &str {
        self.label
    }

    fn supports_native_tools(&self, _model: &str) -> bool {
        true
    }

    fn boxed_clone(&self) -> Box<dyn ChatClient> {
        Box::new(self.clone())
    }

    fn chat(
        &mut self,
        _model: &str,
        messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        let repairing = messages
            .iter()
            .any(|message| message.content.contains(REPAIR_MARKER));
        let queue = if repairing {
            &self.repair_replies
        } else {
            &self.phase_replies
        };
        let mut replies = queue.lock().unwrap();
        if replies.is_empty() {
            return Ok(if repairing {
                AssistantReply::text("done")
            } else {
                self.phase_fallback.clone()
            });
        }
        Ok(replies.remove(0))
    }
}

#[derive(Default)]
struct QuietUi {
    interrupted: AtomicBool,
}

impl InteractionUi for QuietUi {
    fn before_model_call(&self, _label: &str) -> UiGuard {
        UiGuard::noop()
    }

    fn before_tool_call(&self, _name: &str) -> UiGuard {
        UiGuard::noop()
    }

    fn publish_status(&self, _status: UiStatus) {}

    fn interrupted(&self) -> bool {
        self.interrupted.load(Ordering::SeqCst)
    }
}

fn tool_reply(calls: Vec<ToolCall>) -> AssistantReply {
    AssistantReply {
        content: String::new(),
        tool_calls: calls,
        prompt_tokens: None,
        completion_tokens: None,
    }
}

fn write_call(path: &str, content: &str) -> ToolCall {
    ToolCall::new("Write", json!({"path": path, "content": content}))
}

fn bash_reply(command: &str) -> AssistantReply {
    tool_reply(vec![ToolCall::new("Bash", json!({"command": command}))])
}

struct Flow {
    dir: tempfile::TempDir,
    result: anyhow::Result<String>,
    events: Vec<Value>,
}

impl Flow {
    fn named(&self, name: &str) -> Vec<&Value> {
        self.events
            .iter()
            .filter(|event| event["event"] == name)
            .collect()
    }

    fn index_of(&self, name: &str) -> usize {
        self.events
            .iter()
            .position(|event| event["event"] == name)
            .unwrap_or_else(|| panic!("no {name} event"))
    }

    fn final_acceptances_after_repair(&self) -> usize {
        let repaired = self.index_of("final_acceptance_repair_complete");
        self.events[repaired..]
            .iter()
            .filter(|event| event["event"] == "ultra_final_acceptance")
            .count()
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.dir.path().join(rel)
    }
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .unwrap_or_else(|| panic!("not an array: {value}"))
        .iter()
        .map(|item| item.as_str().unwrap().to_string())
        .collect()
}

/// Runs scaffold -> deps -> implement, then the final-acceptance repair
/// session answers with `repair`. `scaffold` is written in the first phase;
/// `implement_main` becomes `src/csv_stats/main.py` in the last phase.
fn run_flow(
    seed: impl FnOnce(&Path),
    scaffold: &[(String, String)],
    implement_main: &str,
    repair: Vec<AssistantReply>,
) -> Flow {
    let dir = tempfile::tempdir().unwrap();
    seed(dir.path());
    let events_path = dir.path().join(".commandagent/runs/issue851/events.jsonl");
    let cfg = config(dir.path().to_path_buf(), events_path.clone());
    let scaffold_plan = r#"{"goal":"Scaffold Python CSV CLI","steps":[{"id":"scaffold","kind":"setup","expected_result":"pass","instruction":"Create pyproject.toml and src/csv_stats/main.py for a CSV file argument CLI","expected_paths":["pyproject.toml","src/csv_stats/main.py"],"verify":[]}]}"#;
    let deps_plan = r#"{"goal":"Prepare Python CLI dependencies","steps":[{"id":"deps","kind":"verify","expected_result":"pass","instruction":"Verify dependency readiness and syntax","expected_paths":["pyproject.toml","src/csv_stats/main.py"],"verify":["python3 -m compileall -q src"]}]}"#;
    let implement_plan = r#"{"goal":"Implement Python CSV CLI behavior","steps":[{"id":"implement","kind":"implement","expected_result":"pass","instruction":"Implement the CLI so it reads a CSV path argument, prints aggregate values, and changes output when the input file changes","expected_paths":["src/csv_stats/main.py"],"verify":["python3 -m compileall -q src"]}]}"#;
    let mut planner = ScriptedClient::new(
        "planner",
        vec![
            AssistantReply::text(scaffold_plan),
            AssistantReply::text(deps_plan),
            AssistantReply::text(implement_plan),
        ],
        Vec::new(),
        AssistantReply::text("unused"),
    );
    let replies = vec![
        tool_reply(
            scaffold
                .iter()
                .map(|(path, content)| write_call(path, content))
                .collect(),
        ),
        AssistantReply::text("dependency verification complete"),
        tool_reply(vec![write_call("src/csv_stats/main.py", implement_main)]),
    ];
    let mut execution = ScriptedClient::new(
        "exec",
        replies,
        repair,
        tool_reply(vec![write_call("src/csv_stats/main.py", implement_main)]),
    );
    let phase = |id: &str, prompt: &str| UltraPhase {
        id: id.to_string(),
        prompt: prompt.to_string(),
    };
    let plan = UltraPlan {
        goal: GOAL.to_string(),
        profile: "python-cli".to_string(),
        style: "default".to_string(),
        intent: "create".to_string(),
        phases: vec![
            phase("scaffold", "Scaffold the Python CLI"),
            phase("deps", "Prepare dependencies"),
            phase("implement", "Implement CLI behavior"),
        ],
    };
    let result = run_ultra_plan_with_ui(
        &mut planner,
        &mut execution,
        &plan,
        &cfg,
        &QuietUi::default(),
    );
    let events = std::fs::read_to_string(&events_path)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();
    Flow {
        dir,
        result,
        events,
    }
}

const CORPUS: &str = "tests/corpus/apps/issue851-final-repair-workspace-delta";

fn fixture(name: &str) -> String {
    std::fs::read_to_string(Path::new(CORPUS).join("fixtures").join(name)).unwrap()
}

fn case(group: &str, id: &str) -> Value {
    let cases: Value = serde_json::from_str(&fixture("cases.json")).unwrap();
    cases[group]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["id"] == id)
        .unwrap_or_else(|| panic!("no corpus case {group}/{id}"))
        .clone()
}

/// The L-107 workspace: a real entry plus a second `src/app/main.py`.
fn two_entries(pyproject: &str, main: &str, extra: &str) -> Vec<(String, String)> {
    vec![
        ("pyproject.toml".to_string(), pyproject.to_string()),
        ("src/csv_stats/main.py".to_string(), main.to_string()),
        ("src/app/main.py".to_string(), extra.to_string()),
    ]
}

fn l107_scaffold() -> Vec<(String, String)> {
    two_entries(
        &fixture("pyproject.toml"),
        &fixture("main.py"),
        &fixture("extra-main.py"),
    )
}

fn simple_scaffold() -> Vec<(String, String)> {
    two_entries(PYPROJECT, VALID_CLI, VALID_CLI)
}

fn final_status(flow: &Flow) -> (String, String) {
    let last = flow
        .named("ultra_final_acceptance")
        .last()
        .copied()
        .expect("a final acceptance event");
    (
        last["final_acceptance_status"]
            .as_str()
            .unwrap()
            .to_string(),
        last["assurance_level"].as_str().unwrap().to_string(),
    )
}

fn assert_full_success(flow: &Flow) {
    assert_eq!(
        final_status(flow),
        ("full_success".to_string(), "full".to_string()),
        "{:?}",
        flow.result
    );
    assert!(flow.result.is_ok(), "{:?}", flow.result);
}

fn assert_not_full(flow: &Flow) {
    let (status, assurance) = final_status(flow);
    assert_ne!(status, "full_success");
    assert_ne!(assurance, "full");
}

fn repair_complete(flow: &Flow) -> &Value {
    flow.named("final_acceptance_repair_complete")[0]
}

fn assert_reverified(flow: &Flow, changed: &[String]) {
    let complete = repair_complete(flow);
    assert_eq!(strings(&complete["changed_paths"]), changed);
    assert_eq!(complete["changed_path_count"], changed.len());
    assert!(
        flow.named("final_acceptance_repair_no_source_change")
            .is_empty(),
        "{:?}",
        flow.named("final_acceptance_repair_no_source_change")
    );
    assert!(flow.final_acceptances_after_repair() >= 1);
    assert_eq!(
        strings(&flow.named("final_acceptance_cycle_complete")[0]["changed_paths"]),
        changed
    );
}

fn assert_short_circuited(flow: &Flow, label: &str) {
    let complete = repair_complete(flow);
    assert_eq!(complete["changed_path_count"], 0, "{label}: {complete}");
    assert_eq!(strings(&complete["changed_paths"]), Vec::<String>::new());
    let skipped = flow.named("final_acceptance_repair_no_source_change");
    assert_eq!(skipped.len(), 1, "{label}: {skipped:?}");
    assert_eq!(skipped[0]["reprobe_skipped"], true, "{label}");
    assert_eq!(flow.final_acceptances_after_repair(), 0, "{label}");
    assert_not_full(flow);
}

fn files_under(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let Ok(entries) = std::fs::read_dir(root) else {
        return found;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            found.extend(files_under(&path));
        } else {
            found.push(path);
        }
    }
    found
}

fn seed_secret_and_dependency_files(root: &Path) {
    for (rel, content) in [
        (".env.js", FAKE_SECRET),
        ("node_modules/pkg/index.js", "module.exports = 1;"),
    ] {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }
}

#[test]
fn l107_bash_rm_of_the_extra_entry_is_reverified_to_full_success() {
    let case = case("reverified", "bash-rm-extra-entry");
    let command = case["command"].as_str().unwrap();
    let flow = run_flow(
        seed_secret_and_dependency_files,
        &l107_scaffold(),
        &fixture("main.py"),
        vec![bash_reply(command), AssistantReply::text("done")],
    );
    assert_eq!(
        flow.named("ultra_final_acceptance_failed").len(),
        1,
        "the two-entry tree is rejected first"
    );
    assert_reverified(&flow, &strings(&case["changed_paths"]));
    assert_eq!(
        strings(&repair_complete(&flow)["reported_changed_paths"]),
        Vec::<String>::new(),
        "a Bash rm reports nothing; the delta comes from the fingerprints"
    );
    assert!(!flow.path("src/app/main.py").exists());
    assert_eq!(
        flow.named("final_acceptance_repair_complete").len(),
        1,
        "the re-verification passes in cycle 1; no second repair is needed"
    );
    assert_full_success(&flow);
}

#[test]
fn repair_complete_event_keeps_the_observed_name_fields_and_types() {
    let flow = run_flow(
        |_| {},
        &simple_scaffold(),
        VALID_CLI,
        vec![bash_reply("rm src/app/main.py")],
    );
    let observed = fixture("events-observed.jsonl")
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .find(|event| event["event"] == "final_acceptance_repair_complete")
        .unwrap();
    let live = repair_complete(&flow);
    let keys = |value: &Value| {
        value
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(keys(live), keys(&observed));
    for key in keys(&observed) {
        assert_eq!(
            std::mem::discriminant(&live[&key]),
            std::mem::discriminant(&observed[&key]),
            "{key}"
        );
    }
    assert_eq!(live["route_bound_changed_paths"], json!([]));
    assert_eq!(live["route_bound_source_changed"], false);
}

#[test]
fn bash_mv_of_the_extra_entry_is_a_delete_plus_add_and_is_reverified() {
    let case = case("reverified", "bash-mv-extra-entry");
    let flow = run_flow(
        |_| {},
        &simple_scaffold(),
        VALID_CLI,
        vec![bash_reply(case["command"].as_str().unwrap())],
    );
    assert_reverified(&flow, &strings(&case["changed_paths"]));
    assert!(flow.path("src/app/legacy.py").is_file());
    assert_full_success(&flow);
}

#[test]
fn bash_created_python_module_is_detected_without_a_reported_path() {
    let case = case("reverified", "bash-create-new-module");
    let flow = run_flow(
        |_| {},
        &simple_scaffold(),
        VALID_CLI,
        vec![bash_reply(case["command"].as_str().unwrap())],
    );
    assert_reverified(&flow, &strings(&case["changed_paths"]));
    assert_full_success(&flow);
}

#[test]
fn bash_rewrite_of_python_source_is_detected_and_reverified() {
    let flow = run_flow(
        |_| {},
        &[
            ("pyproject.toml".to_string(), PYPROJECT.to_string()),
            (
                "src/csv_stats/main.py".to_string(),
                CONSTANT_CLI.to_string(),
            ),
            ("fix/valid.txt".to_string(), VALID_CLI.to_string()),
        ],
        CONSTANT_CLI,
        vec![bash_reply("cp fix/valid.txt src/csv_stats/main.py")],
    );
    assert_reverified(&flow, &["src/csv_stats/main.py".to_string()]);
    assert_full_success(&flow);
}

#[test]
fn write_tool_python_fix_is_detected_and_reverified() {
    let flow = run_flow(
        |_| {},
        &[
            ("pyproject.toml".to_string(), PYPROJECT.to_string()),
            (
                "src/csv_stats/main.py".to_string(),
                CONSTANT_CLI.to_string(),
            ),
        ],
        CONSTANT_CLI,
        vec![tool_reply(vec![write_call(
            "src/csv_stats/main.py",
            VALID_CLI,
        )])],
    );
    let complete = repair_complete(&flow);
    assert_eq!(
        strings(&complete["changed_paths"]),
        ["src/csv_stats/main.py"]
    );
    assert!(flow.final_acceptances_after_repair() >= 1);
    assert_full_success(&flow);
}

#[test]
fn deletion_does_not_pass_when_the_remaining_entry_is_still_wrong() {
    let flow = run_flow(
        |_| {},
        &[
            ("pyproject.toml".to_string(), PYPROJECT.to_string()),
            (
                "src/csv_stats/main.py".to_string(),
                CONSTANT_CLI.to_string(),
            ),
            ("src/app/main.py".to_string(), VALID_CLI.to_string()),
        ],
        CONSTANT_CLI,
        vec![bash_reply("rm src/app/main.py")],
    );
    let complete = repair_complete(&flow);
    assert_eq!(strings(&complete["changed_paths"]), ["src/app/main.py"]);
    assert!(
        flow.final_acceptances_after_repair() >= 1,
        "the re-verification must run even though it cannot pass"
    );
    assert_not_full(&flow);
}

#[test]
fn deletion_does_not_pass_when_the_manifest_points_at_another_package() {
    let mismatched = PYPROJECT.replace("csv_stats.main:main", "other_pkg.main:main");
    let flow = run_flow(
        |_| {},
        &two_entries(&mismatched, VALID_CLI, VALID_CLI),
        VALID_CLI,
        vec![bash_reply("rm src/app/main.py")],
    );
    let complete = repair_complete(&flow);
    assert_eq!(strings(&complete["changed_paths"]), ["src/app/main.py"]);
    assert!(flow.final_acceptances_after_repair() >= 1);
    assert_not_full(&flow);
}

#[test]
fn read_only_repair_keeps_the_no_source_change_short_circuit() {
    let flow = run_flow(
        |_| {},
        &simple_scaffold(),
        VALID_CLI,
        vec![
            tool_reply(vec![ToolCall::new(
                "Read",
                json!({"path": "src/csv_stats/main.py"}),
            )]),
            AssistantReply::text("nothing to change"),
        ],
    );
    assert_short_circuited(&flow, "read-only");
}

#[test]
fn rewriting_the_same_content_keeps_the_short_circuit_although_a_path_is_reported() {
    let flow = run_flow(
        |_| {},
        &simple_scaffold(),
        VALID_CLI,
        vec![tool_reply(vec![write_call(
            "src/csv_stats/main.py",
            VALID_CLI,
        )])],
    );
    assert_short_circuited(&flow, "same-content write");
    assert_eq!(
        strings(&repair_complete(&flow)["reported_changed_paths"]),
        ["src/csv_stats/main.py"]
    );
}

#[test]
fn touch_create_then_delete_and_log_cache_writes_keep_the_short_circuit() {
    for id in [
        "bash-touch",
        "bash-create-then-delete",
        "bash-log-and-cache-only",
    ] {
        let case = case("unchanged", id);
        let flow = run_flow(
            |_| {},
            &simple_scaffold(),
            VALID_CLI,
            vec![bash_reply(case["command"].as_str().unwrap())],
        );
        assert_short_circuited(&flow, id);
        assert!(flow.path("src/app/main.py").is_file(), "{id}");
    }
}

#[test]
fn oversized_source_is_unknown_so_the_repair_is_reverified() {
    let flow = run_flow(
        |root| {
            let path = root.join("src/big/huge.py");
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, "a".repeat(1024 * 1024 + 1)).unwrap();
        },
        &simple_scaffold(),
        VALID_CLI,
        vec![bash_reply("touch src/csv_stats/main.py")],
    );
    let complete = repair_complete(&flow);
    assert_eq!(strings(&complete["changed_paths"]), ["src/big/huge.py"]);
    assert!(
        flow.named("final_acceptance_repair_no_source_change")
            .iter()
            .all(|event| event["cycle_index"] != 1),
        "an unjudgeable path must not be read as unchanged"
    );
    assert!(flow.final_acceptances_after_repair() >= 1);
}

#[cfg(unix)]
#[test]
fn symlink_leaving_the_workspace_is_unknown_and_never_read() {
    let outside = tempfile::tempdir().unwrap();
    let target = outside.path().join("ext.py");
    std::fs::write(&target, FAKE_SECRET).unwrap();
    let flow = run_flow(
        |root| {
            std::fs::create_dir_all(root.join("src/ext")).unwrap();
            std::os::unix::fs::symlink(&target, root.join("src/ext/linked.py")).unwrap();
        },
        &simple_scaffold(),
        VALID_CLI,
        vec![bash_reply("touch src/csv_stats/main.py")],
    );
    let complete = repair_complete(&flow);
    assert_eq!(strings(&complete["changed_paths"]), ["src/ext/linked.py"]);
    assert!(flow.final_acceptances_after_repair() >= 1);
    let log =
        std::fs::read_to_string(flow.path(".commandagent/runs/issue851/events.jsonl")).unwrap();
    assert!(!log.contains(FAKE_SECRET));
}

#[test]
fn secrets_runtime_and_dependency_files_never_reach_evidence_or_the_compile_snapshot() {
    let flow = run_flow(
        seed_secret_and_dependency_files,
        &simple_scaffold(),
        VALID_CLI,
        vec![bash_reply("rm src/app/main.py")],
    );
    assert_reverified(&flow, &["src/app/main.py".to_string()]);
    let snapshots = flow.path(".commandagent/runs/issue851/snapshots");
    for path in files_under(&flow.path(".commandagent")) {
        if path.starts_with(&snapshots) {
            continue;
        }
        let bytes = std::fs::read(&path).unwrap();
        assert!(
            !String::from_utf8_lossy(&bytes).contains(FAKE_SECRET),
            "{}",
            path.display()
        );
    }
    for path in files_under(&snapshots.join("latest")) {
        assert_ne!(path.extension().and_then(|ext| ext.to_str()), Some("py"));
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(!text.contains("sha256:"), "{}", path.display());
    }
    assert!(flow.path(".env.js").is_file());
}

#[test]
fn deep_dynamic_route_does_not_defeat_the_no_source_change_short_circuit() {
    let flow = run_flow(
        |root| {
            let path = root.join("src/app/api/notes/[id]/route.ts");
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, "export async function GET() {}\n").unwrap();
        },
        &simple_scaffold(),
        VALID_CLI,
        vec![bash_reply("touch src/csv_stats/main.py")],
    );
    // An empty actual delta is what routes a repair into the compile
    // no-source-change ladder and the evidence zero-edit attempts.
    assert_short_circuited(&flow, "deep route");
    assert!(
        flow.events
            .iter()
            .all(|event| !event.to_string().contains("workspace-scan-incomplete"))
    );
}

#[cfg(unix)]
#[test]
fn credential_named_symlink_is_not_named_in_events() {
    let flow = run_flow(
        |root| {
            std::fs::create_dir_all(root.join("src/ext")).unwrap();
            std::os::unix::fs::symlink(
                "missing-target",
                root.join("src/ext/service-account-x.json"),
            )
            .unwrap();
            std::os::unix::fs::symlink("../../.env.js", root.join("src/ext/key.js")).unwrap();
            std::fs::write(root.join(".env.js"), FAKE_SECRET).unwrap();
        },
        &simple_scaffold(),
        VALID_CLI,
        vec![bash_reply("touch src/csv_stats/main.py")],
    );
    assert_short_circuited(&flow, "credential symlinks");
    let log =
        std::fs::read_to_string(flow.path(".commandagent/runs/issue851/events.jsonl")).unwrap();
    assert!(!log.contains("service-account-x"));
    assert!(!log.contains("src/ext/key.js"));
    assert!(!log.contains(FAKE_SECRET));
}
