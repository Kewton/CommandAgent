use commandagent::providers::{AssistantReply, ChatClient};
use commandagent::state::{ConversationMessage, ToolCall};
use commandagent::tools::registry::ToolSpec;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/corpus/apps/issue459-create-recovery-integration")
}
pub fn original() -> PathBuf {
    fixture()
        .parent()
        .unwrap()
        .join("issue456-create-recovery/original")
}
pub fn contracts() -> PathBuf {
    fixture()
        .parent()
        .unwrap()
        .join("issue457-nextjs-contracts")
}
pub fn read_json(path: impl AsRef<Path>) -> Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}
pub fn hash(path: &Path) -> String {
    format!("{:x}", Sha256::digest(std::fs::read(path).unwrap()))
}
pub fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}
pub fn source_hashes(root: &Path) -> BTreeMap<String, String> {
    fn visit(root: &Path, dir: &Path, out: &mut BTreeMap<String, String>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if [
                ".commandagent",
                "node_modules",
                ".next",
                "evidence",
                "browser-evidence",
            ]
            .contains(&entry.file_name().to_str().unwrap())
            {
                continue;
            }
            if entry.file_type().unwrap().is_dir() {
                visit(root, &path, out);
            } else {
                out.insert(
                    path.strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .into_owned(),
                    hash(&path),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    visit(root, root, &mut out);
    out
}
pub fn install_links(root: &Path) {
    let modules =
        PathBuf::from(std::env::var("ISSUE459_NODE_MODULES").expect("ISSUE459_NODE_MODULES"));
    assert_eq!(
        read_json(modules.join("typescript/package.json"))["version"],
        "5.9.3"
    );
    assert_eq!(
        read_json(modules.join("next/package.json"))["version"],
        "14.2.35"
    );
    let playwright = PathBuf::from(
        std::env::var("ISSUE459_PLAYWRIGHT_MODULE").expect("ISSUE459_PLAYWRIGHT_MODULE"),
    );
    std::fs::create_dir_all(root.join("node_modules")).unwrap();
    for entry in std::fs::read_dir(&modules).unwrap() {
        let entry = entry.unwrap();
        std::os::unix::fs::symlink(
            entry.path(),
            root.join("node_modules").join(entry.file_name()),
        )
        .unwrap();
    }
    std::os::unix::fs::symlink(playwright, root.join("node_modules/playwright")).unwrap();
}
pub fn events(root: &Path) -> Vec<Value> {
    std::fs::read_to_string(root.join(".commandagent/events.jsonl"))
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}
pub struct ReplayState {
    root: PathBuf,
    scenario: String,
    calls: BTreeMap<String, usize>,
    pub requests: Vec<Value>,
    pub inspections: Vec<Value>,
    baseline: BTreeMap<String, String>,
}
impl ReplayState {
    pub fn new(root: &Path, scenario: &str) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            root: root.into(),
            scenario: scenario.into(),
            calls: BTreeMap::new(),
            requests: vec![],
            inspections: vec![],
            baseline: source_hashes(root),
        }))
    }
}
#[derive(Clone)]
pub struct Replay {
    state: Arc<Mutex<ReplayState>>,
    planner: bool,
}
impl Replay {
    pub fn planner(state: Arc<Mutex<ReplayState>>) -> Self {
        Self {
            state,
            planner: true,
        }
    }
    pub fn execution(state: Arc<Mutex<ReplayState>>) -> Self {
        Self {
            state,
            planner: false,
        }
    }
}
fn tools(calls: Vec<ToolCall>) -> AssistantReply {
    AssistantReply {
        content: String::new(),
        tool_calls: calls,
        prompt_tokens: None,
        completion_tokens: None,
    }
}
impl ChatClient for Replay {
    fn label(&self) -> &str {
        "issue459-model-protocol-replay"
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
        let mut state = self.state.lock().unwrap();
        let recorded = events(&state.root);
        let replies = read_json(fixture().join("replies.json"));
        let attempt = recorded
            .iter()
            .filter(|e| e["event"] == "recovery_plan_auto_run_start")
            .count();
        let phase = recorded
            .iter()
            .rev()
            .find(|e| e["event"] == "ultra_phase_start")
            .and_then(|e| e["phase_id"].as_str())
            .unwrap_or("unknown")
            .to_string();
        let key = format!("{attempt}:{phase}:{}", self.planner);
        let count = state.calls.entry(key).or_default();
        *count += 1;
        let count = *count;
        state.requests.push(json!({"attempt":attempt,"phase":phase,"planner":self.planner,"turn":count,"messages":messages}));
        eprintln!(
            "replay attempt={attempt} phase={phase} planner={} turn={count}",
            self.planner
        );
        if self.planner {
            let verify = phase.starts_with("verify");
            return Ok(AssistantReply::text(json!({"goal":replies["planner_goal"], "steps":[{
                "id": if verify {"verify-project"} else {"repair-project"},
                "kind":if verify {"verify"} else {"implement"},
                "expected_result":"pass", "instruction":replies["planner_instruction"],
                "expected_paths":["src/app/page.tsx","src/app/api/projects/[id]/tasks/route.ts","src/app/api/projects/route.ts","src/app/api/tasks/[id]/route.ts","src/lib/store.ts","src/lib/types.ts"],
                "verify":if verify {vec!["npm run build"]} else {vec![]}
            }]}).to_string()));
        }
        if phase == "inspect-current-state" {
            let step = recorded
                .iter()
                .rev()
                .find(|e| e["event"] == "plan_step_started")
                .and_then(|e| e["step_id"].as_str())
                .unwrap()
                .to_owned();
            let read_turn = state
                .calls
                .entry(format!("inspection:{attempt}:{step}"))
                .or_default();
            *read_turn += 1;
            if *read_turn == 1 {
                let treatment = state.root.join(format!(
                    ".commandagent/recovery-treatments/attempt-{attempt}/workspace"
                ));
                let context = read_json(
                    treatment.join(".commandagent/recovery-runtime/inspection-context.json"),
                );
                assert_eq!(
                    source_hashes(&state.root),
                    state.baseline,
                    "control changed before attempt {attempt}"
                );
                assert_eq!(
                    source_hashes(&treatment),
                    state.baseline,
                    "attempt {attempt} must start from retained control"
                );
                assert_eq!(context["original_intent"], "create");
                assert!(
                    context["instruction"]
                        .as_str()
                        .unwrap()
                        .contains("role?: string")
                );
                for path in [
                    "src/app/api/projects/[id]/tasks/route.ts",
                    "src/lib/store.ts",
                    "src/lib/types.ts",
                ] {
                    assert!(
                        context["read_paths"]
                            .as_array()
                            .unwrap()
                            .contains(&json!(path)),
                        "missing {path}"
                    );
                }
                if count == 1 {
                    state.inspections.push(context.clone());
                }
                return Ok(tools(context["read_ranges"].as_array().unwrap().iter().filter(|r|r["path"].as_str().is_some_and(|p|p.contains("/api/")||p.starts_with("src/lib/"))).map(|r|ToolCall::new("Read",json!({"path":r["path"],"start_line":r["start_line"],"end_line":r["end_line"]}))).collect()));
            }
            return Ok(AssistantReply::text(
                replies["inspection_summary"].as_str().unwrap(),
            ));
        }
        let no_edit = attempt == 0
            || (matches!(
                state.scenario.as_str(),
                "aligned-after-no-edit" | "promoted-after-no-edit"
            ) && attempt == 1)
            || state.scenario == "no-edit";
        if no_edit {
            return Ok(tools(vec![ToolCall::new(
                "Read",
                replies["no_edit_read"].clone(),
            )]));
        }
        if state.scenario == "provider-stop" {
            anyhow::bail!("Replay provider unavailable");
        }
        if phase.starts_with("verify") && count == 1 {
            return Ok(tools(vec![ToolCall::new(
                "Bash",
                json!({"command":"npm run build"}),
            )]));
        }
        if count == 1 && !phase.starts_with("verify") {
            let treatment = state.root.join(format!(
                ".commandagent/recovery-treatments/attempt-{attempt}/workspace"
            ));
            let cases = read_json(fixture().join("cases.json"));
            let case_name = if state.scenario.starts_with("promoted-") {
                "aligned-observable"
            } else if matches!(
                state.scenario.as_str(),
                "aligned-after-no-edit" | "oracle-unexecuted" | "aligned-with-oracle"
            ) {
                "aligned"
            } else {
                state.scenario.as_str()
            };
            let case = cases
                .as_array()
                .unwrap()
                .iter()
                .find(|c| c["name"] == case_name)
                .unwrap();
            let mut files = BTreeMap::new();
            for overlay in case["overlays"].as_array().unwrap() {
                let base = if overlay == "observable" {
                    fixture()
                } else {
                    contracts()
                }
                .join("overlays")
                .join(overlay.as_str().unwrap());
                for path in source_hashes(&base).keys() {
                    files.insert(
                        path.clone(),
                        std::fs::read_to_string(base.join(path)).unwrap(),
                    );
                }
            }
            return Ok(tools(
                files
                    .into_iter()
                    .filter_map(|(path, content)| {
                        let old = std::fs::read_to_string(treatment.join(&path)).unwrap();
                        (old != content).then(|| {
                            ToolCall::new(
                                "Edit",
                                json!({"path":path,"old_string":old,"new_string":content}),
                            )
                        })
                    })
                    .collect(),
            ));
        }
        Ok(AssistantReply::text(
            replies["repair_summary"].as_str().unwrap(),
        ))
    }
}
