#![cfg(unix)]

//! Issue #930: the read-path judge sent every de-quoted word to the path proof,
//! so a `sed`/`awk` program, a `grep` pattern, and the text `echo`/`printf`
//! display were refused as outside paths. A bare top-level `sed`, `awk`,
//! `grep`, `echo`, or `printf` whose argument role is proven data now leaves
//! that word out of the path candidates, while every file operand, pattern or
//! script file, redirect target, and following command keeps the proof. A
//! program whose parse finds file I/O or execution is never data: its word
//! keeps the candidate reading it had before the fix, and the files it reads
//! or writes are added to the existing proof as reads and write targets. A
//! shell, a list consumer, an inline interpreter, a sed/awk program that is not
//! proven read-only, or a compound redirect the segments cannot hold anywhere
//! in the text closes every data role, as does an unquoted tilde in the word.
//!
//! The order follows the Issue: the public decision, the runtime policy of a
//! scripted step, the real Bash tool, then the corpus rows. Shell commands run
//! only in a temp workspace under the cargo target directory; no model or
//! network is used. Every test name contains `argument_roles` so a mutation
//! filter selects these tests with the unit tests in `argument_roles/tests.rs`.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use commandagent::config::{
    Action, Config, ConfigFieldSources, NarrationMode, OllamaThink, OpenAiApi, PlanPreset,
    PromptLayout, Provider,
};
use commandagent::mode::ExecutionMode;
use commandagent::planner::runner::run_step_plan;
use commandagent::planner::step_plan::{PlanStep, StepPlan};
use commandagent::providers::{AssistantReply, ChatClient};
use commandagent::state::{ConversationMessage, ToolCall};
use commandagent::tools::allow_policy::bash_verify_command_is_auto_approvable;
use commandagent::tools::bash::path_confinement_rejection;
use commandagent::tools::registry::{ToolContext, ToolRegistry, ToolSpec, tool_error_kind};
use commandagent::tools::sensitive_path::command_references_secret;
use commandagent::tools::workspace_policy::WorkspacePolicy;
use serde::Deserialize;
use serde_json::{Value, json};

const CANARY: &str = "OUTSIDE-CANARY-930";

struct Fixture {
    dir: tempfile::TempDir,
    root: PathBuf,
}

impl Fixture {
    fn outside(&self) -> PathBuf {
        self.dir.path().join("outside")
    }

    /// The command with `{OUT}` replaced by the outside directory.
    fn command(&self, template: &str) -> String {
        template.replace("{OUT}", &self.outside().to_string_lossy())
    }

    /// The outside directory's entries and the canary's bytes, to prove a
    /// refused command neither wrote there nor changed the canary.
    fn outside_snapshot(&self) -> (Vec<String>, String) {
        let mut names: Vec<String> = std::fs::read_dir(self.outside())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        names.sort();
        let canary = std::fs::read_to_string(self.outside().join("secret")).unwrap();
        (names, canary)
    }
}

/// A tempdir under the cargo target directory, outside the system prefixes the
/// read guard admits (Issue #604).
fn tempdir() -> tempfile::TempDir {
    let dir = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).expect("create tempdir");
    assert!(
        !["/usr", "/bin", "/opt", "/etc", "/tmp"]
            .iter()
            .any(|prefix| dir.path().starts_with(prefix)),
        "the fixture tempdir must not sit under a system prefix: {}",
        dir.path().display()
    );
    dir
}

/// A workspace with `in.txt`, `index.html`, `app.py`, an inside `.env`, a
/// protected `tests/spec.rs`, and `sub/link`, a symlink to an outside
/// directory holding the canary `secret`, `patterns`, and `prog.awk`.
fn fixture() -> Fixture {
    let dir = tempdir();
    let root = dir.path().join("ws");
    for directory in ["sub", "tests"] {
        std::fs::create_dir_all(root.join(directory)).unwrap();
    }
    std::fs::write(
        root.join("in.txt"),
        "head a\nbegin\nmiddle /item/2\nend\ntail a\n",
    )
    .unwrap();
    std::fs::write(
        root.join("index.html"),
        "<html>\n<script>\nconsole.log(1);\n</script>\n</html>\n",
    )
    .unwrap();
    std::fs::write(
        root.join("app.py"),
        "import sys\nfor line in sys.stdin:\n    print('app:' + line.strip())\n",
    )
    .unwrap();
    std::fs::write(root.join(".env"), "DUMMY_930=1\n").unwrap();
    std::fs::write(root.join("tests/spec.rs"), "// spec\n").unwrap();
    let outside = dir.path().join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("secret"), format!("{CANARY}\n")).unwrap();
    std::fs::write(outside.join("patterns"), "CANARY\n").unwrap();
    std::fs::write(outside.join("prog.awk"), "{print}\n").unwrap();
    std::os::unix::fs::symlink(&outside, root.join("sub/link")).unwrap();
    let root = root.canonicalize().unwrap();
    Fixture { dir, root }
}

fn rejected(root: &Path, command: &str) -> bool {
    path_confinement_rejection(command, root).is_some()
}

#[derive(Deserialize)]
struct Cases {
    schema: String,
    base_sha: String,
    recorded: Vec<RecordedCase>,
    unresolved: Vec<UnresolvedCase>,
    synthetic: Vec<SyntheticCase>,
}

#[derive(Deserialize)]
struct RecordedCase {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    observed_before_fix: Observed,
    command_excerpt: String,
    synthetic_twin: String,
}

#[derive(Deserialize)]
struct Observed {
    decision: String,
    policy_error_kind: String,
    misread_fragment: String,
}

#[derive(Deserialize)]
struct UnresolvedCase {
    #[serde(rename = "type")]
    kind: String,
    first_version: String,
}

#[derive(Deserialize)]
struct SyntheticCase {
    id: String,
    origin: String,
    command: String,
    expect: String,
    secret: bool,
}

fn cases() -> Cases {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/corpus/apps/bash-literal-argument-roles/fixtures/cases.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
    let cases: Cases = serde_json::from_str(&text).expect("cases.json must parse");
    assert_eq!(cases.schema, "bash-literal-argument-roles/v1");
    cases
}

/// A data word leaves the path candidates while the real operand of the same
/// command stays: the rejection names the real operand, not the program.
#[test]
fn argument_roles_public_decision_keeps_the_real_operand() {
    let fixture = fixture();
    let root = &fixture.root;
    for command in [
        "sed -n '/begin/,/end/p' in.txt",
        "sed '1d;$d' in.txt",
        "sed -e 's#/old#/new#g' in.txt",
        "awk '/begin/{f=1;next} /end/{f=0} f{print}' in.txt",
        "grep -n '</tag>' in.txt",
        "grep -e '/item' -- in.txt",
        "printf '%s\\n' '{\"pointer\":\"/item/2\"}'",
        "echo '{\"pointer\":\"/item\"}'",
    ] {
        assert!(!rejected(root, command), "expected allow: {command:?}");
    }
    let outside = fixture.outside().to_string_lossy().to_string();
    for (command, path) in [
        (
            "sed -n '/begin/,/end/p' sub/link/secret",
            "sub/link/secret".to_string(),
        ),
        (
            "awk '/begin/{f=1;next} f{print}' sub/link/secret",
            "sub/link/secret".to_string(),
        ),
        (
            "grep -n '/item' sub/link/secret",
            "sub/link/secret".to_string(),
        ),
        (
            &fixture.command("grep -e '/item' -- {OUT}/secret"),
            format!("{outside}/secret"),
        ),
        (
            "grep -f sub/link/patterns in.txt",
            "sub/link/patterns".to_string(),
        ),
        (
            "grep -fsub/link/patterns in.txt",
            "sub/link/patterns".to_string(),
        ),
        // An input redirect closes the data roles; its target is still proven.
        (
            "grep -n x in.txt < sub/link/secret",
            "sub/link/secret".to_string(),
        ),
        ("sed '1w /tmp/out' in.txt", "/tmp/out".to_string()),
        (
            "sed -n '1w sub/link/out' in.txt",
            "sub/link/out".to_string(),
        ),
        (
            "sed '1r sub/link/secret' in.txt",
            "sub/link/secret".to_string(),
        ),
        (
            "awk '{print > \"/tmp/out\"}' in.txt",
            "/tmp/out".to_string(),
        ),
        (
            "awk '{getline line < \"sub/link/secret\"; print line}' in.txt",
            "sub/link/secret".to_string(),
        ),
        (
            "sed -n p in.txt; cat sub/link/secret",
            "sub/link/secret".to_string(),
        ),
    ] {
        let rejection = path_confinement_rejection(command, root)
            .unwrap_or_else(|| panic!("expected rejection: {command:?}"));
        assert_eq!(rejection.path, path, "{command:?}: {}", rejection.reason);
    }
}

/// A program with file I/O or command execution is never data, so nothing is
/// looser than before the fix: the outside path inside an `e` command, an
/// `s///e` replacement, an awk `system` call, or an awk pipe is still refused,
/// and the files a program reads or writes are proven as reads and writes. A
/// workspace-internal `sed w` stays allowed (the Issue #871 contract).
#[test]
fn argument_roles_program_io_is_not_looser_than_before() {
    let fixture = fixture();
    let root = &fixture.root;
    for template in [
        "sed '1e cat {OUT}/secret' in.txt",
        "sed 's|.*|cat {OUT}/secret|e' in.txt",
        "awk '{system(\"cat {OUT}/secret\")}' in.txt",
        "awk '{print | \"cat {OUT}/secret\"}' in.txt",
        "awk '{\"cat {OUT}/secret\" | getline line; print line}' in.txt",
        "sed '1w {OUT}/written' in.txt",
        "sed 's/a/b/w ../written' in.txt",
        "sed -n '1W sub/link/written' in.txt",
        "sed '1R sub/link/secret' in.txt",
        "awk '{print >> \"../written\"}' in.txt",
        "awk '{print > \"sub/link/written\"}' in.txt",
        "cd sub && sed -n '1w link/written' ../in.txt",
    ] {
        let command = fixture.command(template);
        assert!(rejected(root, &command), "expected rejection: {command:?}");
    }
    for command in [
        "sed -n '1,5w out.txt' in.txt",
        "awk '{print > \"out.txt\"}' in.txt",
        "sed '1r in.txt' in.txt",
    ] {
        assert!(!rejected(root, command), "expected allow: {command:?}");
    }
}

/// The judgement depends on the role, not on what the text looks like: the
/// same spellings handed to a reader that opens them stay paths, a quoted file
/// operand is still a path, and an over-long name is still proven.
#[test]
fn argument_roles_do_not_follow_text_shape_quotes_or_lstat_errors() {
    let fixture = fixture();
    let root = &fixture.root;
    let long = "a".repeat(300);
    for command in [
        format!("printf '%s' '{long}'"),
        format!("echo '{long}'"),
        format!("grep -n '{long}' in.txt"),
        format!("printf '{long}\\n' > local.txt"),
    ] {
        assert!(!rejected(root, &command), "expected allow: {command:?}");
    }
    for command in [
        format!("cat '{long}'"),
        format!("grep -n x '{long}'"),
        format!("sed -n p '{long}'"),
        "cat '/begin/,/end/p'".to_string(),
        "cat '{\"pointer\":\"/item\"}'".to_string(),
        "cat 'sub/link/secret'".to_string(),
        "grep '/x' 'sub/link/secret'".to_string(),
        "tail -n 2 '/x/,/y/p'".to_string(),
    ] {
        assert!(rejected(root, &command), "expected rejection: {command:?}");
    }
    // A data word is not proven by a working-directory join either, while
    // the same name read by `cat` still is.
    assert!(!rejected(root, "cd sub && echo link"));
    assert!(rejected(root, "cd sub && echo link && cat link"));
    assert!(rejected(root, "cd sub && grep -n link link"));
}

/// The data role never reaches the credential scan, the verify auto-approval,
/// or the command bytes the tool runs.
#[test]
fn argument_roles_leave_credential_and_auto_approval_unchanged() {
    let fixture = fixture();
    let root = &fixture.root;
    for command in [
        "printf '%s\\n' '.env'",
        "echo .env",
        "grep -n x .env",
        "sed -n p in.txt && cat .env",
    ] {
        assert!(
            command_references_secret(command).is_some(),
            "expected credential detection: {command:?}"
        );
    }
    for command in [
        "printf 'literal-data\\n' > local.txt",
        "sed -n '/begin/,/end/p' in.txt > picked.txt",
    ] {
        assert!(!rejected(root, command));
        assert!(
            !bash_verify_command_is_auto_approvable(command, root),
            "a write must not be auto-approved: {command:?}"
        );
    }
}

#[derive(Clone)]
struct ScriptedClient {
    calls: Arc<Mutex<Vec<ToolCall>>>,
}

impl ChatClient for ScriptedClient {
    fn label(&self) -> &str {
        "issue930-scripted"
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
        let mut calls = self.calls.lock().unwrap();
        if calls.is_empty() {
            return Ok(AssistantReply::text("done"));
        }
        let call = calls.remove(0);
        Ok(AssistantReply {
            content: String::new(),
            tool_calls: vec![call],
            prompt_tokens: None,
            completion_tokens: None,
        })
    }
}

fn config(root: &Path, events: &Path) -> Config {
    Config {
        workspace_root: root.to_path_buf(),
        state_dir: root.join("state"),
        eval_events_path: Some(events.to_path_buf()),
        completion_contract_path: None,
        yes: true,
        offline: false,
        context_budget: 1000,
        model: "test".to_string(),
        provider: Provider::Ollama,
        tool_protocol: None,
        openai_api: OpenAiApi::ChatCompletions,
        prompt_layout: PromptLayout::Stable,
        plan_preset: PlanPreset::None,
        intent_override: None,
        planner_model: "test".to_string(),
        planner_provider: Provider::Ollama,
        planner_think: Some(OllamaThink::False),
        classifier_model: "test".to_string(),
        classifier_provider: Provider::Ollama,
        openai_compatible: None,
        ollama_host: "http://localhost:11434".to_string(),
        ollama_think: None,
        lm_studio_host: "http://localhost:1234".to_string(),
        num_predict: 100,
        max_iterations: 20,
        recovery_plan_auto_runs: 0,
        chat_timeout_secs: 1,
        chat_timeout_source: "override:test".to_string(),
        field_sources: ConfigFieldSources::default(),
        chat_retries: 0,
        stream: false,
        resume: None,
        fresh_session: false,
        no_footer: true,
        narration: NarrationMode::Normal,
        profile: "generic".to_string(),
        profile_explicit: true,
        profile_inference: None,
        style: "default".to_string(),
        action: Action::Repl,
    }
}

/// Through the runtime Bash policy of a scripted implement step: the proven
/// data commands run, the commands with a real outside operand, redirect
/// target, program I/O, or following command are blocked before they run.
#[test]
fn argument_roles_runtime_policy_allows_data_and_blocks_real_paths() {
    let fixture = fixture();
    let root = &fixture.root;
    let before = fixture.outside_snapshot();
    let allowed = [
        "sed -n '1,2w first.txt' in.txt",
        "sed -n '/begin/,/end/p' in.txt > picked.txt",
        "awk '/begin/{f=1;next} /end/{f=0} f{print}' in.txt > flagged.txt",
        "grep -n '</script>' index.html > tag.txt",
        "printf '%s\\n' '{\"pointer\":\"/item/2\"}' > data.jsonl",
    ];
    assert_eq!(allowed.len(), 5);
    let blocked = [
        "sed -n '/begin/,/end/p' sub/link/secret > leak1.txt",
        "grep -n CANARY < sub/link/secret > leak2.txt",
        "sed '1r sub/link/secret' in.txt > leak3.txt",
        "printf 'literal-data\\n' > sub/link/written.txt",
        "grep -n x in.txt > leak4.txt; cat sub/link/secret > leak5.txt",
    ];
    // Alternate allowed and blocked calls so no run of refusals ends the step
    // as a repeated recoverable error.
    let mut calls: Vec<ToolCall> = allowed
        .iter()
        .zip(blocked.iter())
        .flat_map(|(allow, block)| [allow, block])
        .map(|command| ToolCall::new("Bash", json!({ "command": command })))
        .collect();
    calls.push(ToolCall::new(
        "Write",
        json!({ "path": "done.txt", "content": "done" }),
    ));
    let mut client = ScriptedClient {
        calls: Arc::new(Mutex::new(calls)),
    };
    let plan = StepPlan {
        goal: "Extract the marked lines into workspace files".to_string(),
        steps: vec![PlanStep {
            id: "extract-lines".to_string(),
            kind: "implement".to_string(),
            expected_result: "pass".to_string(),
            instruction: "Extract the marked lines, then write done.txt".to_string(),
            expected_paths: vec!["done.txt".to_string()],
            verify: vec!["test -f done.txt".to_string()],
        }],
    };
    let events_path = fixture.dir.path().join("events.jsonl");
    let _ = run_step_plan(&mut client, &plan, &config(root, &events_path));
    let events: Vec<Value> = std::fs::read_to_string(&events_path)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();
    let policy: Vec<&Value> = events
        .iter()
        .filter(|event| event["event"] == "runtime_bash_policy")
        .collect();
    for command in allowed {
        let event = policy
            .iter()
            .find(|event| event["command_summary"] == command)
            .unwrap_or_else(|| panic!("no policy event for {command:?}: {policy:?}"));
        assert_eq!(event["blocked"], false, "{command:?}: {event}");
    }
    for command in blocked {
        let event = policy
            .iter()
            .find(|event| event["command_summary"] == command)
            .unwrap_or_else(|| panic!("no policy event for {command:?}: {policy:?}"));
        assert_eq!(event["blocked"], true, "{command:?}: {event}");
        assert_eq!(
            event["policy_error_kind"], "bash_path_confinement_error",
            "{command:?}: {event}"
        );
    }
    assert_eq!(
        std::fs::read_to_string(root.join("first.txt")).unwrap(),
        "head a\nbegin\n"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("picked.txt")).unwrap(),
        "begin\nmiddle /item/2\nend\n"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("flagged.txt")).unwrap(),
        "middle /item/2\n"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("tag.txt")).unwrap(),
        "4:</script>\n"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("data.jsonl")).unwrap(),
        "{\"pointer\":\"/item/2\"}\n"
    );
    for leak in [
        "leak1.txt",
        "leak2.txt",
        "leak3.txt",
        "leak4.txt",
        "leak5.txt",
    ] {
        assert!(!root.join(leak).exists(), "{leak} must not be written");
    }
    assert_eq!(fixture.outside_snapshot(), before);
}

/// Through the real Bash tool: proven data commands run and print what the
/// shell computes, refused commands never run, the canary is never read, and
/// nothing is written outside the workspace or to the protected file.
#[test]
fn argument_roles_real_bash_tool_runs_data_and_refuses_real_paths() {
    let fixture = fixture();
    let root = &fixture.root;
    let context = ToolContext {
        root: root.clone(),
        mode: ExecutionMode::Act,
        auto_approve: true,
        interactive_approval: false,
        offline: true,
        workspace_policy: WorkspacePolicy::NormalTask,
        eval_events_path: Some(fixture.dir.path().join("tool-events.jsonl")),
        expected_paths: Vec::new(),
        protected_paths: vec!["tests/spec.rs".to_string()],
    };
    let registry = ToolRegistry::default();
    let before = fixture.outside_snapshot();
    for (command, expected) in [
        ("sed -n '/begin/,/end/p' in.txt", "middle /item/2"),
        ("sed '1d;$d' in.txt", "middle /item/2"),
        ("sed -e 's#/item#/thing#g' in.txt", "middle /thing/2"),
        (
            "sed -n '/<script>/,/<\\/script>/p' index.html | sed '1d;$d'",
            "console.log(1);",
        ),
        (
            "awk '/<script>/{f=1;next} /<\\/script>/{f=0} f' index.html",
            "console.log(1);",
        ),
        ("grep -n '</script>' index.html", "4:</script>"),
        ("grep -e '/item' -- in.txt", "middle /item/2"),
        (
            "printf '%s\\n' '{\"pointer\":\"/item/2\"}'",
            "{\"pointer\":\"/item/2\"}",
        ),
        (
            "echo '{\"pointer\":\"/item\"}' | cat",
            "{\"pointer\":\"/item\"}",
        ),
    ] {
        let output = registry
            .execute("Bash", &json!({ "command": command }), &context)
            .unwrap_or_else(|err| panic!("{command:?} must run: {err}"));
        assert!(output.contains(expected), "{command:?}: {output}");
        assert!(!output.contains(CANARY), "{command:?}: {output}");
    }
    let outside = fixture.outside().to_string_lossy().to_string();
    for (command, kind) in [
        (
            "sed -n '/begin/,/end/p' sub/link/secret".to_string(),
            "bash_path_confinement_error",
        ),
        (
            format!("grep -e CANARY -- {outside}/secret"),
            "bash_path_confinement_error",
        ),
        (
            "grep -f sub/link/patterns in.txt".to_string(),
            "bash_path_confinement_error",
        ),
        (
            "grep -fsub/link/patterns in.txt".to_string(),
            "bash_path_confinement_error",
        ),
        (
            "sed '1r sub/link/secret' in.txt".to_string(),
            "bash_path_confinement_error",
        ),
        (
            "awk '{getline line < \"sub/link/secret\"; print line}' in.txt".to_string(),
            "bash_path_confinement_error",
        ),
        (
            format!("awk '{{print > \"{outside}/written\"}}' in.txt"),
            "bash_path_confinement_error",
        ),
        (
            "printf 'literal-data\\n' > sub/link/written".to_string(),
            "bash_path_confinement_error",
        ),
        (
            "grep -n x in.txt 2> sub/link/err".to_string(),
            "bash_path_confinement_error",
        ),
        (
            "grep -n CANARY < sub/link/secret".to_string(),
            "bash_path_confinement_error",
        ),
        (
            "sed -n p in.txt; cat sub/link/secret".to_string(),
            "bash_path_confinement_error",
        ),
        (
            "printf '%s' sub/link/secret | xargs cat".to_string(),
            "bash_path_confinement_error",
        ),
        (
            format!("sed '1e cat {outside}/secret' in.txt"),
            "bash_path_confinement_error",
        ),
        (
            format!("sed 's|.*|cat {outside}/secret|e' in.txt"),
            "bash_path_confinement_error",
        ),
        (
            format!("awk '{{system(\"cat {outside}/secret\")}}' in.txt"),
            "bash_path_confinement_error",
        ),
        (
            format!("awk '{{print | \"cat > {outside}/written\"}}' in.txt"),
            "bash_path_confinement_error",
        ),
        (
            format!("sed -n '1w {outside}/written' in.txt"),
            "bash_path_confinement_error",
        ),
        (
            "sed -n '1w sub/link/written' in.txt".to_string(),
            "bash_path_confinement_error",
        ),
        // `-i.bak` is the in-place suffix for both GNU and BSD sed.
        (
            "sed -i.bak 's/a/b/' sub/link/secret".to_string(),
            "bash_path_confinement_error",
        ),
        (
            "printf 'x\\n' > tests/spec.rs".to_string(),
            "protected_path_mutation_rejected",
        ),
    ] {
        let error = registry
            .execute("Bash", &json!({ "command": command }), &context)
            .unwrap_err();
        assert_eq!(tool_error_kind(&error), kind, "{command:?}: {error}");
        assert!(!error.to_string().contains(CANARY), "{command:?}: {error}");
    }
    let error = registry
        .execute(
            "Bash",
            &json!({ "command": "printf '%s\\n' x && cat .env" }),
            &context,
        )
        .unwrap_err();
    assert!(!error.to_string().contains("DUMMY_930"), "{error}");
    assert_eq!(fixture.outside_snapshot(), before);
    assert_eq!(
        std::fs::read_to_string(root.join("tests/spec.rs")).unwrap(),
        "// spec\n"
    );
    assert!(!root.join("sub/err").exists());
}

/// Every synthetic corpus row drives the real public decision and the
/// credential scan.
#[test]
fn argument_roles_corpus_rows_match_the_real_decision() {
    let fixture = fixture();
    let root = &fixture.root;
    let cases = cases();
    assert_eq!(cases.base_sha, "f9103ce6ad184319f0cc17de11ce0b78d0e1a885");
    let mut allowed = 0usize;
    let mut refused = 0usize;
    for case in &cases.synthetic {
        assert_eq!(case.origin, "synthetic", "{}", case.id);
        let command = fixture.command(&case.command);
        let expect_reject = match case.expect.as_str() {
            "reject" => true,
            "allow" => false,
            other => panic!("{}: unknown expectation {other:?}", case.id),
        };
        assert_eq!(
            rejected(root, &command),
            expect_reject,
            "{}: confinement decision for {command:?}",
            case.id
        );
        assert_eq!(
            command_references_secret(&command).is_some(),
            case.secret,
            "{}: credential scan for {command:?}",
            case.id
        );
        if expect_reject {
            refused += 1;
        } else {
            allowed += 1;
        }
    }
    assert!(
        allowed >= 20 && refused >= 50,
        "allowed={allowed} refused={refused}"
    );
}

/// The recorded rows keep only pre-fix observations; each names a synthetic
/// twin that is allowed now, a short excerpt is allowed now, and the seven
/// first-version misreadings stay listed as unresolved.
#[test]
fn argument_roles_recorded_rows_are_pre_fix_observations() {
    let fixture = fixture();
    let root = &fixture.root;
    let cases = cases();
    assert_eq!(cases.recorded.len(), 16);
    let b = cases
        .recorded
        .iter()
        .filter(|case| case.kind == "B")
        .count();
    let c = cases
        .recorded
        .iter()
        .filter(|case| case.kind == "C")
        .count();
    assert_eq!((b, c), (5, 11));
    for case in &cases.recorded {
        assert_eq!(case.observed_before_fix.decision, "reject", "{}", case.id);
        assert_eq!(
            case.observed_before_fix.policy_error_kind,
            "bash_path_confinement_error"
        );
        assert!(!case.observed_before_fix.misread_fragment.is_empty());
        let twin = cases
            .synthetic
            .iter()
            .find(|row| row.id == case.synthetic_twin)
            .unwrap_or_else(|| panic!("{}: missing twin", case.id));
        assert_eq!(twin.expect, "allow", "{}", case.id);
        assert!(!rejected(root, &twin.command), "{}", case.id);
        if case.command_excerpt != "<omitted>" {
            assert!(
                !rejected(root, &case.command_excerpt),
                "{}: {:?}",
                case.id,
                case.command_excerpt
            );
        }
    }
    assert_eq!(cases.unresolved.len(), 7);
    let b = cases
        .unresolved
        .iter()
        .filter(|case| case.kind == "B")
        .count();
    let c = cases
        .unresolved
        .iter()
        .filter(|case| case.kind == "C")
        .count();
    assert_eq!((b, c), (2, 5));
    assert!(
        cases
            .unresolved
            .iter()
            .all(|case| case.first_version == "not handled")
    );
}

/// The security-review counterexamples of Issue #930: a compound redirect
/// (`>|`) that once let a real operand read as another command's data, a
/// shell, list consumer, or I/O program elsewhere in the text that once let
/// produced data reach a real read, and an unquoted tilde. Each stays refused
/// by the public decision and never runs through the real Bash tool: no
/// canary reaches the output and no file is created. Each refusal has an
/// allowed twin without the outside path.
#[test]
fn argument_roles_review_counterexamples_stay_refused_and_never_run() {
    let fixture = fixture();
    let root = &fixture.root;
    let context = ToolContext {
        root: root.clone(),
        mode: ExecutionMode::Act,
        auto_approve: true,
        interactive_approval: false,
        offline: true,
        workspace_policy: WorkspacePolicy::NormalTask,
        eval_events_path: Some(fixture.dir.path().join("review-events.jsonl")),
        expected_paths: Vec::new(),
        protected_paths: vec!["tests/spec.rs".to_string()],
    };
    let registry = ToolRegistry::default();
    let before = fixture.outside_snapshot();
    for command in REVIEW_REFUSED {
        assert!(rejected(root, command), "expected rejection: {command:?}");
        let error = registry
            .execute("Bash", &json!({ "command": command }), &context)
            .unwrap_err();
        assert_eq!(
            tool_error_kind(&error),
            "bash_path_confinement_error",
            "{command:?}: {error}"
        );
        assert!(!error.to_string().contains(CANARY), "{command:?}: {error}");
    }
    for name in ["printf", "script.sh", "list.txt", "out.txt", "item"] {
        assert!(!root.join(name).exists(), "{name} must not be created");
    }
    assert_eq!(fixture.outside_snapshot(), before);
    for (command, expected) in [
        (
            "sed -n p in.txt >| printed.txt && cat printed.txt",
            "middle /item/2",
        ),
        ("printf '%s\\n' '~'", "~"),
        ("grep -e '~' in.txt || echo none", "none"),
        (
            "printf '%s\\n' '{\"p\":\"/b\"}' | python3 app.py",
            "app:{\"p\":\"/b\"}",
        ),
    ] {
        let output = registry
            .execute("Bash", &json!({ "command": command }), &context)
            .unwrap_or_else(|err| panic!("{command:?} must run: {err}"));
        assert!(output.contains(expected), "{command:?}: {output}");
    }
    assert!(root.join("printed.txt").exists());
    assert!(!root.join("printf").exists());
}

/// Commands from the security review that must stay refused.
const REVIEW_REFUSED: &[&str] = &[
    "sed -n p >| printf sub/link/secret",
    "cat >| printf sub/link/secret",
    "sed -n p 2>| printf sub/link/secret",
    "sed -n p in.txt >|printf sub/link/secret",
    "grep -n x in.txt > out.txt sub/link/secret",
    "sed -n p in.txt >| sub/link/printf",
    "printf '%s' 'sed -n p ' sub/link/secret > script.sh; sh script.sh",
    "printf '%s' 'sed -n p ' sub/link/secret > script.sh; /bin/sh script.sh",
    "printf '%s' 'sed -n p ' sub/link/secret > script.sh && env /bin/sh script.sh",
    "printf '%s' 'sed -n p ' sub/link/secret > script.sh\nnice -n 5 /bin/sh script.sh",
    "printf '%s' sub/link/secret > list.txt; xargs cat < list.txt",
    "printf '%s' sub/link/secret > list.txt; /usr/bin/xargs cat < list.txt",
    "printf '%s' sub/link/secret > list.txt\ntimeout 5 xargs cat < list.txt",
    "printf '%s\\n' sub/link/secret > list.txt; awk 'BEGIN{getline item < \"list.txt\"; system(\"sed -n p \" item)}'",
    "printf '%s\\n' sub/link/secret > list.txt && sed 's/^/cat /e' list.txt",
    "printf '%s\\n' sub/link/secret > list.txt; python3 -c 'import sys'",
    "printf '%s' ~",
    "printf '%s' ~/x",
    "grep -e ~/pat in.txt",
    // Second security recheck: an option before the inline-code option, and
    // a shell spelled in another case on a case-insensitive filesystem.
    "printf '%s\\n' sub/link/secret > list.txt; python3 -B -c 'print(open(open(\"list.txt\").read().strip()).read(),end=\"\")'",
    "printf '%s\\n' sub/link/secret > list.txt; python3.12 -u -c 'import sys'",
    "printf '%s\\n' sub/link/secret > list.txt; node --no-warnings -e '1'",
    "printf '%s\\n' sub/link/secret > list.txt; perl -w -e '1'",
    "printf '%s\\n' sub/link/secret > list.txt; ruby -w -e '1'",
    "printf '%s\\n' sub/link/secret > list.txt; Python3 -c 'import sys'",
    "printf '%s' 'sed -n p ' sub/link/secret > script.sh; /bin/SH script.sh",
    "printf '%s' 'sed -n p ' sub/link/secret > script.sh; /bin/Sh script.sh",
    "printf '%s' 'sed -n p ' sub/link/secret > script.sh && env /bin/SH script.sh",
    "printf '%s' 'sed -n p ' sub/link/secret > script.sh; '/bin/SH' script.sh",
    "printf '%s' sub/link/secret > list.txt; /usr/bin/XARGS cat < list.txt",
];

/// The same counterexamples through the runtime Bash policy of a scripted
/// implement step: each is blocked before it runs, and its allowed twin runs.
#[test]
fn argument_roles_runtime_policy_blocks_review_counterexamples() {
    let fixture = fixture();
    let root = &fixture.root;
    let before = fixture.outside_snapshot();
    let allowed = [
        "sed -n p in.txt >| printed.txt",
        "printf '%s\\n' '~' > tilde.txt",
        "printf '%s\\n' x > list.txt; cat list.txt > copy.txt",
    ];
    let blocked = [
        "sed -n p >| printf sub/link/secret",
        "printf '%s' 'sed -n p ' sub/link/secret > script.sh; /bin/sh script.sh",
        "printf '%s\\n' sub/link/secret > item; awk 'BEGIN{getline item < \"item\"; system(\"sed -n p \" item)}'",
    ];
    let mut calls: Vec<ToolCall> = allowed
        .iter()
        .zip(blocked.iter())
        .flat_map(|(allow, block)| [allow, block])
        .map(|command| ToolCall::new("Bash", json!({ "command": command })))
        .collect();
    calls.push(ToolCall::new(
        "Write",
        json!({ "path": "done.txt", "content": "done" }),
    ));
    let mut client = ScriptedClient {
        calls: Arc::new(Mutex::new(calls)),
    };
    let plan = StepPlan {
        goal: "Copy the input lines into workspace files".to_string(),
        steps: vec![PlanStep {
            id: "copy-lines".to_string(),
            kind: "implement".to_string(),
            expected_result: "pass".to_string(),
            instruction: "Copy the input lines, then write done.txt".to_string(),
            expected_paths: vec!["done.txt".to_string()],
            verify: vec!["test -f done.txt".to_string()],
        }],
    };
    let events_path = fixture.dir.path().join("review-runtime.jsonl");
    let _ = run_step_plan(&mut client, &plan, &config(root, &events_path));
    let events: Vec<Value> = std::fs::read_to_string(&events_path)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();
    let decision = |command: &str| {
        events
            .iter()
            .find(|event| {
                event["event"] == "runtime_bash_policy" && event["command_summary"] == command
            })
            .unwrap_or_else(|| panic!("no policy event for {command:?}"))
            .clone()
    };
    for command in allowed {
        assert_eq!(decision(command)["blocked"], false, "{command:?}");
    }
    for command in blocked {
        let event = decision(command);
        assert_eq!(event["blocked"], true, "{command:?}: {event}");
        assert_eq!(event["policy_error_kind"], "bash_path_confinement_error");
    }
    assert_eq!(
        std::fs::read_to_string(root.join("printed.txt")).unwrap(),
        std::fs::read_to_string(root.join("in.txt")).unwrap()
    );
    assert_eq!(
        std::fs::read_to_string(root.join("tilde.txt")).unwrap(),
        "~\n"
    );
    for name in ["printf", "script.sh", "item"] {
        assert!(!root.join(name).exists(), "{name} must not be created");
    }
    assert_eq!(fixture.outside_snapshot(), before);
}
