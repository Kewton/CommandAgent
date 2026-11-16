//! Issue #1013: `--operator-check` through the binary, each process with an empty environment, a
//! fake `HOME`, its own `TMPDIR`, and an operator root of its own. The only provider is a counting
//! listener on 127.0.0.1, which every provider host and proxy variable points at; no key is set.
//!
//! Every check here is compared, as whole trees before and after (each entry's type, bytes, mode,
//! and mtime, empty directories included), over the operator root, the workspace, the state
//! directory, the ledger directory, the fake `HOME`, and `TMPDIR`, where the isolation probe makes
//! and removes its temporary area. The events path must not appear.
//!
//! Where a run is the other side of a comparison it is the real run (`--prompt`), whose refusal
//! code is read from its last `error:` line.

#![cfg(unix)]

use std::collections::BTreeMap;
use std::io::Read as _;
use std::net::TcpListener;
use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const STAGE_PROFILE: &str = "check-stages";
const TOOL_PROFILE: &str = "check-one-tool";
const UPPER: &str = "check-upper-tool";
const LOWER: &str = "check-lower-tool";
const FIXED_INPUT: &str = "fixed/seed.json";
const WALLET: &str = "wallet-1013";

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Counts every connection made to it, and answers none.
struct Listener {
    url: String,
    seen: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Listener {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let seen = Arc::new(AtomicUsize::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let (thread_seen, thread_stop) = (seen.clone(), stop.clone());
        let thread = std::thread::spawn(move || {
            while !thread_stop.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok(_) => {
                        thread_seen.fetch_add(1, Ordering::SeqCst);
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(5)),
                }
            }
        });
        Self {
            url,
            seen,
            stop,
            thread: Some(thread),
        }
    }

    fn count(&self) -> usize {
        self.seen.load(Ordering::SeqCst)
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// One entry of a tree: its type, its bytes' digest (a file) or target (a link), mode, inode,
/// mtime, and ctime.
type Tree = BTreeMap<PathBuf, Entry>;
type Entry = (char, String, u32, u64, (i64, i64), (i64, i64));

fn tree(root: &Path) -> Tree {
    let mut entries = Tree::new();
    if let Ok(metadata) = std::fs::symlink_metadata(root) {
        entries.insert(PathBuf::new(), entry(root, &metadata));
        if metadata.is_dir() {
            walk(root, Path::new(""), &mut entries);
        }
    }
    entries
}

fn entry(path: &Path, metadata: &std::fs::Metadata) -> Entry {
    let kind = metadata.file_type();
    let (letter, content) = if kind.is_symlink() {
        (
            'l',
            std::fs::read_link(path)
                .unwrap()
                .to_string_lossy()
                .into_owned(),
        )
    } else if kind.is_dir() {
        ('d', String::new())
    } else {
        ('f', sha256(&std::fs::read(path).unwrap()))
    };
    (
        letter,
        content,
        metadata.mode(),
        metadata.ino(),
        (metadata.mtime(), metadata.mtime_nsec()),
        (metadata.ctime(), metadata.ctime_nsec()),
    )
}

fn walk(root: &Path, relative: &Path, entries: &mut Tree) {
    for item in std::fs::read_dir(root.join(relative)).unwrap() {
        let item = item.unwrap();
        let path = relative.join(item.file_name());
        let metadata = std::fs::symlink_metadata(root.join(&path)).unwrap();
        entries.insert(path.clone(), entry(&root.join(&path), &metadata));
        if metadata.is_dir() {
            walk(root, &path, entries);
        }
    }
}

struct World {
    _temp: tempfile::TempDir,
    base: PathBuf,
    listener: Listener,
}

impl World {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let base = std::fs::canonicalize(temp.path()).unwrap();
        for directory in [
            "workspace/fixed",
            "home",
            "tmp",
            "operator/profile-tools",
            "operator/run-budgets",
            "tools",
            "ledgers",
            "extension/profiles",
        ] {
            std::fs::create_dir_all(base.join(directory)).unwrap();
        }
        std::fs::write(base.join("workspace").join(FIXED_INPUT), b"a fixed seed").unwrap();
        let world = Self {
            _temp: temp,
            base,
            listener: Listener::start(),
        };
        world.write_profile(STAGE_PROFILE, &[UPPER, LOWER], true);
        world.write_profile(TOOL_PROFILE, &[UPPER], false);
        world
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.base.join(relative)
    }

    fn write_profile(&self, profile: &str, tools: &[&str], stages: bool) {
        let declared = tools
            .iter()
            .map(|id| format!("  {{ id = \"{id}\", effect = \"propose-artifacts\" }},\n"))
            .collect::<String>();
        let (schema, staged) = if stages {
            (
                "v2",
                format!(
                    "[profile_tools.stages]\nstages = [\n  {{ id = \"upper\", targets = [{{ id = \"main\", tool_id = \"{UPPER}\" }}] }},\n  {{ id = \"lower\", targets = [{{ id = \"main\", tool_id = \"{LOWER}\" }}] }},\n]\n"
                ),
            )
        } else {
            ("v1", String::new())
        };
        let text = format!(
            "[metadata]\nid = \"{profile}\"\ndisplay_name = \"Check {profile}\"\nschema_version = \"v2\"\ntask_family = \"generic\"\n[plan]\nintent = \"create\"\nphases = [\n  {{ id = \"implementation\", prompt = \"Write the notes for {{goal}}.\" }},\n]\n[artifacts]\nrequired = [\"notes.txt\"]\n[guidance.variants.contract]\ntriggers = [{{ condition = \"always\" }}]\nmessages = {{ instruction = \"Write notes.txt.\" }}\n[[checks.final]]\nid = \"scaffold_files_present\"\nparams = {{ files = [\"notes.txt\"] }}\n[profile_tools]\nschema_version = \"profile-tool.declaration/{schema}\"\nbuiltin_tools = [\"Read\"]\ntools = [\n{declared}]\n{staged}"
        );
        let directory = self.path(&format!("extension/profiles/{profile}"));
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("manifest.toml"), text).unwrap();
    }

    /// A pinned `propose-artifacts` descriptor, every pin as admission calculates it. `change`
    /// edits the descriptor before it is written.
    fn write_tool(
        &self,
        tool_id: &str,
        output: &str,
        inputs: &[&str],
        change: impl FnOnce(&mut Value),
    ) {
        let directory = self.path(&format!("tools/{tool_id}"));
        std::fs::create_dir_all(&directory).unwrap();
        let script = directory.join("tool.sh");
        std::fs::write(&script, "exit 0\n").unwrap();
        let shell = std::fs::canonicalize("/bin/bash").unwrap();
        let schema = json!({
            "protocol": "profile-tool.schema/v1",
            "root": {
                "type": "object",
                "additionalProperties": false,
                "required": ["mode", "expected_revision"],
                "properties": {
                    "mode": {"type": "string", "enum": ["good", "bad"]},
                    "expected_revision": {"type": "string"},
                },
            },
            "inputs": inputs,
        });
        let mut descriptor = json!({
            "protocol": "profile-tool.descriptor/v1",
            "tool_id": tool_id,
            "name": if tool_id == UPPER { "ProposeUpper" } else { "ProposeLower" },
            "version": "1.0.0",
            "description": "Propose one artifact for a synthetic fixture.",
            "effect": "propose-artifacts",
            "parameters": [],
            "schema_sha256": sha256(serde_json::to_string(&schema).unwrap().as_bytes()),
            "schema": schema,
            "runtime": {"kind": "pinned_process", "id": "fixed-script"},
            "limits": {"max_input_bytes": 1 << 20, "max_output_bytes": 64 * 1024},
            "process": {
                "protocol": "profile-tool.process/v1",
                "executable": {"path": shell, "sha256": sha256(&std::fs::read(&shell).unwrap())},
                "dependency_directories": [{
                    "path": directory,
                    "sha256": commandagent::tools::profile_tools::directory_pin(&directory).unwrap(),
                }],
                "argv": [script],
                "cwd": directory,
                "env": {"TOOL_MODE": "fixture"},
                "limits": {"timeout_ms": 10_000, "max_result_bytes": 64 * 1024},
                "outputs": [{"id": "artifact", "path": output}],
            },
            "stage": {
                "required_checks": ["shape"],
                "attempts_per_target": 4,
                "attempts_per_run": 16,
                "deadline_seconds": 600,
            },
        });
        change(&mut descriptor);
        self.write_descriptor(tool_id, &descriptor.to_string());
    }

    fn write_descriptor(&self, tool_id: &str, text: &str) {
        std::fs::write(
            self.path(&format!("operator/profile-tools/{tool_id}.json")),
            text,
        )
        .unwrap();
    }

    fn write_stage_tools(&self) {
        self.write_tool(UPPER, "out/upper.json", &[FIXED_INPUT], |_| {});
        self.write_tool(
            LOWER,
            "out/lower.json",
            &["out/upper.json", FIXED_INPUT],
            |_| {},
        );
    }

    fn declare_v2(&self, change: impl FnOnce(&mut Value)) {
        let mut policy = json!({
            "protocol": "run-budget.policy/v2",
            "accounting_basis": "usage_tariff",
            "budget_id": WALLET,
            "limits": {
                "cost_usd": "1",
                "physical_sends": 1,
                "logical_global": 1,
                "logical_per_stage": 1,
                "tokens": 1_000_000,
                "wall_seconds": 100,
                "final_repair_attempts": 1,
                "final_repair_seconds": 100
            },
            "route": {
                "api": "responses",
                "endpoint": "https://api.example.test/v1/responses",
                "model": "m-1",
                "response_models": ["m-1-snapshot"],
                "effort": "high",
                "service_tier": "default"
            },
            "tariff": {
                "revision": "synthetic-1013",
                "price_conditions": {"region": "global", "regional_surcharge": "none"},
                "cache_write_mode": "ttl_30m_12h",
                "tiers": [{
                    "above_input_tokens": 0,
                    "input_per_million": "1",
                    "cached_input_per_million": "1",
                    "cache_write_30m_per_million": "1",
                    "cache_write_12h_per_million": "1",
                    "output_per_million": "1",
                    "request_fee": "0"
                }]
            },
            "input_bound": {
                "method": "body_bytes_plus_overhead_or_provider_max",
                "input_overhead": 4,
                "provider_max_input_tokens": 1_050_000
            },
            "max_output_tokens": 500,
            "max_input_tokens": 2_000_000,
            "ledger_dir": self.path("ledgers")
        });
        change(&mut policy);
        std::fs::write(
            self.path(&format!("operator/run-budgets/{WALLET}.json")),
            policy.to_string(),
        )
        .unwrap();
    }

    /// A v1 declaration for the generic compatible Chat route, with placeholder prices.
    fn declare_v1(&self) {
        let policy = json!({
            "protocol": "run-budget.policy/v1",
            "budget_id": WALLET,
            "limits": {
                "cost_usd": "1",
                "physical_sends": 1,
                "logical_global": 1,
                "logical_per_stage": 1,
                "tokens": 1_000_000,
                "wall_seconds": 100,
                "final_repair_attempts": 1,
                "final_repair_seconds": 100
            },
            "tariff": {
                "revision": "synthetic-1013",
                "input_per_million": "1",
                "cached_input_per_million": "1",
                "cache_write_per_million": "1",
                "output_per_million": "1",
                "request_fee": "0"
            },
            "route": {
                "api": "chat_completions",
                "model": "m-1",
                "upstream_only": ["u"],
                "allow_fallbacks": false,
                "effort": "low",
                "reasoning_in_output": true
            },
            "max_output_tokens": 500,
            "max_input_tokens": 2_000_000,
            "input_overhead": 4,
            "ledger_dir": self.path("ledgers")
        });
        std::fs::write(
            self.path(&format!("operator/run-budgets/{WALLET}.json")),
            policy.to_string(),
        )
        .unwrap();
    }

    fn ledger(&self) -> PathBuf {
        self.path(&format!("ledgers/{WALLET}/ledger.json"))
    }

    /// The whole world: the operator root, the workspace (the child's cwd), the state directory,
    /// the ledgers, the fake `HOME`, the tools and the extension root, and every place an events
    /// file could appear. Only `TMPDIR`'s own entry is left out: the probe makes and removes its
    /// area there, which moves that directory's times and nothing below it.
    fn world_tree(&self) -> Tree {
        let mut all = tree(&self.base);
        all.remove(Path::new("tmp"));
        all
    }

    /// One process, holding the whole world to what it was and the listener to no connection.
    fn invoke(&self, mut command: Command) -> Output {
        let before = self.world_tree();
        let connections = self.listener.count();
        let output = command.output().unwrap();
        let after = self.world_tree();
        let changed: Vec<_> = before
            .keys()
            .chain(after.keys())
            .filter(|path| before.get(*path) != after.get(*path))
            .collect();
        assert!(changed.is_empty(), "the check changed {changed:?}");
        assert_eq!(
            self.listener.count(),
            connections,
            "the check connected to a provider"
        );
        output
    }

    fn command(&self, wallet: bool, arguments: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_commandagent"));
        self.configure(&mut command, wallet, arguments);
        command
    }

    fn configure(&self, command: &mut Command, wallet: bool, arguments: &[&str]) {
        self.configure_with(command, wallet, arguments, false);
    }

    /// With `defaults`, no `--state-dir` and no events variable: the state directory under the
    /// fake `HOME` and the events under the workspace, as an operator's plain call has them.
    fn configure_with(
        &self,
        command: &mut Command,
        wallet: bool,
        arguments: &[&str],
        defaults: bool,
    ) {
        let url = &self.listener.url;
        command
            .env_clear()
            .current_dir(self.path("workspace"))
            .env("HOME", self.path("home"))
            .env("TMPDIR", self.path("tmp"))
            .env("PATH", "/usr/bin:/bin")
            .env("LC_ALL", "C")
            .env("LANG", "C")
            .env("COMMANDAGENT_ADAPTER_CONFIG_ROOT", self.path("operator"))
            .env("HTTPS_PROXY", url)
            .env("HTTP_PROXY", url)
            .env("ALL_PROXY", url)
            .env("NO_PROXY", "")
            .arg("--cwd")
            .arg(self.path("workspace"));
        if !defaults {
            command
                .env("COMMANDAGENT_EVAL_EVENTS", self.path("events.jsonl"))
                .arg("--state-dir")
                .arg(self.path("state"));
        }
        command
            .arg("--extension-root")
            .arg(self.path("extension"))
            .args(["--ollama-host", url, "--lm-studio-host", url])
            .args(["--allow", "read,write"])
            .args(arguments);
        if wallet {
            command.env("COMMANDAGENT_RUN_BUDGET", WALLET);
        }
    }

    /// `--operator-check --json`, holding every watched tree to what it was and the listener to
    /// no connection.
    fn check(&self, wallet: bool, arguments: &[&str]) -> (Option<i32>, Value) {
        let mut command = self.command(wallet, arguments);
        command.args(["--operator-check", "--json"]);
        self.check_with(command)
    }

    fn check_with(&self, command: Command) -> (Option<i32>, Value) {
        let output = self.invoke(command);
        let report: Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
            panic!(
                "{error}: {}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            )
        });
        assert_eq!(report["schema_version"], "commandagent.operator-check/v1");
        (output.status.code(), report)
    }

    /// The real run's last `error:` line.
    fn run(&self, wallet: bool, arguments: &[&str]) -> (Option<i32>, String) {
        let mut command = self.command(wallet, arguments);
        command.args(["--prompt", "say hi"]);
        let output = command.output().unwrap();
        (output.status.code(), last_error(&output))
    }
}

fn last_error(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr)
        .lines()
        .rfind(|line| line.starts_with("error"))
        .unwrap_or("")
        .to_string()
}

/// The refusal code of a run's error line, in the words the check reports.
fn run_code(error: &str) -> String {
    if let Some(rest) = error.split("profile_tool_refused: ").nth(1) {
        return rest.split(':').next().unwrap().to_string();
    }
    for marker in [
        "the run budget refused the entry: ",
        "the run budget refused the client: ",
    ] {
        if let Some(rest) = error.split(marker).nth(1) {
            return rest.trim().to_string();
        }
    }
    panic!("no refusal code in {error:?}")
}

fn check_of<'a>(report: &'a Value, id: &str) -> &'a Value {
    report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["id"] == id)
        .unwrap_or_else(|| panic!("no check {id} in {report:#}"))
}

/// The one check that failed, by its code.
fn failed_code(report: &Value) -> (String, String) {
    let failed: Vec<&Value> = report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|check| check["status"] == "fail" || check["status"] == "unverified")
        .collect();
    assert_eq!(failed.len(), 1, "{report:#}");
    (
        failed[0]["id"].as_str().unwrap().to_string(),
        failed[0]["code"].as_str().unwrap().to_string(),
    )
}

/// One edit of a descriptor.
type Change = fn(&mut Value);

fn responses() -> [&'static str; 6] {
    [
        "--provider",
        "openai",
        "--api",
        "responses",
        "--model",
        "m-1",
    ]
}

fn lm_studio() -> [&'static str; 4] {
    ["--provider", "lm-studio", "--model", "m-1"]
}

/// Acceptance 1 and PM conditions 3 and 4: a declaration the run admits passes with no provider
/// call and nothing written, before a wallet exists and again with one a real run left there.
#[cfg(target_os = "macos")]
#[test]
fn an_accepted_operator_root_passes_with_no_provider_call_and_nothing_written() {
    let world = World::new();
    world.write_stage_tools();
    world.declare_v2(|_| {});
    let profile = ["--profile", STAGE_PROFILE];
    let arguments = [&responses()[..], &profile[..]].concat();

    let (code, report) = world.check(true, &arguments);
    assert_eq!(code, Some(0), "{report:#}");
    assert_eq!(report["status"], "pass");
    for (id, status) in [
        ("profile_tools", "pass"),
        ("isolation.probe_cleanup", "pass"),
        ("os_sandbox", "pass"),
        ("structured_stages", "pass"),
        ("run_budget.declaration", "pass"),
        ("run_budget.ledger", "pass"),
        ("run_budget.route.executor", "pass"),
        ("run_budget.route.planner", "not_checked"),
        ("request_pattern", "pass"),
        ("secret_scope", "pass"),
    ] {
        assert_eq!(check_of(&report, id)["status"], status, "{id}: {report:#}");
    }
    assert_eq!(
        check_of(&report, "isolation.probe_cleanup")["data"]["made"],
        2
    );
    let ledger = check_of(&report, "run_budget.ledger");
    assert_eq!(ledger["code"], "absent");
    assert_eq!(ledger["data"]["read_without_lock"], true);
    assert!(!world.path(&format!("ledgers/{WALLET}")).exists());
    assert!(!world.path("state").exists());

    // The real run opens the wallet and saves it, then stops for want of a key before any client
    // exists. The check then reads what it saved, and still writes nothing.
    let (code, error) = world.run(true, &arguments);
    assert_eq!(code, Some(1), "{error}");
    assert!(error.contains("OPENAI_API_KEY"), "{error}");
    assert_eq!(world.listener.count(), 0);
    assert!(world.ledger().is_file());
    std::fs::remove_file(world.path(&format!("ledgers/{WALLET}/lock"))).unwrap();
    let (code, report) = world.check(true, &arguments);
    assert_eq!(code, Some(0), "{report:#}");
    let ledger = check_of(&report, "run_budget.ledger");
    assert_eq!(ledger["code"], "continues");
    assert_eq!(ledger["data"]["read_without_lock"], true);
    assert!(!world.path(&format!("ledgers/{WALLET}/lock")).exists());
}

/// Acceptance 2: each step refused before the run starts gives the code the run gives.
#[test]
fn every_refusal_before_the_run_has_the_runs_own_code() {
    // Profile tools, each a broken pin or descriptor. A proposal tool needs the strict isolation,
    // which exists only on macOS; elsewhere both sides answer `unavailable` first.
    let cases: [(&str, Change); 4] = [
        ("schema pin", |d| d["schema_sha256"] = json!("0".repeat(64))),
        ("executable pin", |d| {
            d["process"]["executable"]["sha256"] = json!("0".repeat(64));
        }),
        ("directory pin", |d| {
            d["process"]["dependency_directories"][0]["sha256"] = json!("0".repeat(64));
        }),
        ("descriptor field", |d| d["unknown_field"] = json!(1)),
    ];
    for (name, change) in cases {
        let world = World::new();
        world.write_tool(UPPER, "out/upper.json", &[FIXED_INPUT], change);
        let arguments = [&lm_studio()[..], &["--profile", TOOL_PROFILE][..]].concat();
        let (code, report) = world.check(false, &arguments);
        assert_eq!(code, Some(1), "{name}: {report:#}");
        let (id, check) = failed_code(&report);
        assert_eq!(id, "profile_tools", "{name}");
        let (run, error) = world.run(false, &arguments);
        assert_eq!(run, Some(2), "{name}: {error}");
        assert_eq!(check, run_code(&error), "{name}");
        assert_eq!(world.listener.count(), 0, "{name}");
    }
    // A descriptor that is not JSON at all.
    let world = World::new();
    world.write_descriptor(UPPER, "{ not json");
    let arguments = [&lm_studio()[..], &["--profile", TOOL_PROFILE][..]].concat();
    let (_, report) = world.check(false, &arguments);
    let (_, check) = failed_code(&report);
    assert_eq!(check, "invalid_descriptor");
    let (_, error) = world.run(false, &arguments);
    assert_eq!(check, run_code(&error));

    // The run budget: a declaration its decoder refuses, and a route it cannot count.
    let world = World::new();
    world.declare_v2(|policy| policy["route"]["service_tier"] = json!("flex"));
    let (code, report) = world.check(true, &responses());
    assert_eq!(code, Some(1));
    let (id, check) = failed_code(&report);
    assert_eq!(id, "run_budget.declaration");
    assert_eq!(check, "PolicyV2(UnsupportedServiceTier)");
    let (run, error) = world.run(true, &responses());
    assert_eq!(run, Some(2), "{error}");
    assert_eq!(check, run_code(&error));

    let world = World::new();
    world.declare_v2(|_| {});
    let chat = [
        "--provider",
        "openai",
        "--api",
        "chat-completions",
        "--model",
        "m-1",
    ];
    let (code, report) = world.check(true, &chat);
    assert_eq!(code, Some(1));
    let failed: Vec<&Value> = report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|check| check["status"] == "fail")
        .collect();
    assert_eq!(failed.len(), 1, "{report:#}");
    assert_eq!(failed[0]["id"], "run_budget.route.executor");
    let (_, error) = world.run(true, &chat);
    assert_eq!(failed[0]["code"].as_str().unwrap(), run_code(&error));
    assert_eq!(run_code(&error), "RouteNotCountable");
    assert_eq!(world.listener.count(), 0);
}

/// Acceptance 2 for a ledger already there: one the declaration no longer continues, and (PM
/// condition 3) a truncated one, which is never a pass. Both reads are said to be unlocked.
#[test]
fn an_existing_ledger_that_does_not_continue_has_the_runs_own_code() {
    let world = World::new();
    world.declare_v2(|_| {});
    let (code, error) = world.run(true, &responses());
    assert_eq!(code, Some(1), "{error}");
    let saved = std::fs::read(world.ledger()).unwrap();

    // Another declaration under the same ID.
    world.declare_v2(|policy| policy["max_output_tokens"] = json!(400));
    let (code, report) = world.check(true, &responses());
    assert_eq!(code, Some(1));
    let (id, check) = failed_code(&report);
    assert_eq!(
        (id.as_str(), check.as_str()),
        ("run_budget.ledger", "Wallet(PolicyChanged)")
    );
    assert_eq!(
        check_of(&report, "run_budget.ledger")["data"]["read_without_lock"],
        true
    );
    let (_, error) = world.run(true, &responses());
    assert_eq!(check, run_code(&error));

    // The declaration it was made under, and the record cut in half.
    world.declare_v2(|_| {});
    std::fs::write(world.ledger(), &saved[..saved.len() / 2]).unwrap();
    let (code, report) = world.check(true, &responses());
    assert_eq!(code, Some(1));
    assert_eq!(report["status"], "fail");
    let (id, check) = failed_code(&report);
    assert_eq!(
        (id.as_str(), check.as_str()),
        ("run_budget.ledger", "Wallet(Corrupt)")
    );
    let ledger = check_of(&report, "run_budget.ledger");
    assert_eq!(ledger["data"]["read_without_lock"], true);
    assert!(
        ledger["detail"]
            .as_str()
            .unwrap()
            .contains("without the wallet's lock")
    );
    let (_, error) = world.run(true, &responses());
    assert_eq!(check, run_code(&error));
    assert_eq!(world.listener.count(), 0);
}

/// Acceptance 2: a stage set the run cannot hold, here because no OS boundary is applied, is
/// refused with the stage contract's word on both sides, before any provider call.
#[cfg(target_os = "macos")]
#[test]
fn a_stage_set_without_an_os_boundary_has_the_runs_own_code() {
    let world = World::new();
    world.write_stage_tools();
    let arguments = [
        &lm_studio()[..],
        &["--profile", STAGE_PROFILE, "--sandbox", "off"][..],
    ]
    .concat();
    let (code, report) = world.check(false, &arguments);
    assert_eq!(code, Some(1));
    let (id, check) = failed_code(&report);
    assert_eq!(
        (id.as_str(), check.as_str()),
        ("structured_stages", "stage_contract_refused")
    );
    assert!(
        check_of(&report, "structured_stages")["detail"]
            .as_str()
            .unwrap()
            .contains("OsSandboxNotEnforced"),
        "{report:#}"
    );
    let (_, error) = world.run(false, &arguments);
    assert_eq!(check, run_code(&error), "{error}");
    assert_eq!(world.listener.count(), 0, "{error}");
}

/// Acceptance 2: where the strict isolation is unavailable, both answer `unavailable`. On macOS
/// the binary runs inside a sandbox of its own, where `sandbox-exec` cannot start again, which is
/// the environment and not a seam; elsewhere there is no backend.
#[test]
fn where_the_strict_isolation_is_unavailable_both_say_unavailable() {
    let world = World::new();
    world.write_tool(UPPER, "out/upper.json", &[FIXED_INPUT], |_| {});
    let arguments = [&lm_studio()[..], &["--profile", TOOL_PROFILE][..]].concat();
    let nested = |extra: &[&str]| {
        let mut command = if cfg!(target_os = "macos") {
            let mut command = Command::new("/usr/bin/sandbox-exec");
            command
                .args(["-p", "(version 1)(allow default)"])
                .arg(env!("CARGO_BIN_EXE_commandagent"));
            command
        } else {
            Command::new(env!("CARGO_BIN_EXE_commandagent"))
        };
        world.configure(&mut command, false, &arguments);
        command.args(extra);
        command
    };
    let (code, report) = world.check_with(nested(&["--operator-check", "--json"]));
    assert_eq!(code, Some(1));
    let (id, check) = failed_code(&report);
    assert_eq!(
        (id.as_str(), check.as_str()),
        ("profile_tools", "unavailable")
    );
    let output = nested(&["--prompt", "say hi"]).output().unwrap();
    assert_eq!(check, run_code(&last_error(&output)));
    assert_eq!(world.listener.count(), 0);
}

/// Acceptance 3: a descriptor written with the values the check shows passes the real admission.
/// Wrong pins are shown with what admission calculates, and the check fails as the run does.
#[cfg(target_os = "macos")]
#[test]
fn a_descriptor_written_with_the_shown_pins_is_admitted_by_the_run() {
    let world = World::new();
    world.write_tool(UPPER, "out/upper.json", &[FIXED_INPUT], |d| {
        d["schema_sha256"] = json!("1".repeat(64));
        d["process"]["executable"]["sha256"] = json!("2".repeat(64));
        d["process"]["dependency_directories"][0]["sha256"] = json!("3".repeat(64));
    });
    let arguments = [&lm_studio()[..], &["--profile", TOOL_PROFILE][..]].concat();
    let (code, report) = world.check(false, &arguments);
    assert_eq!(code, Some(1));
    assert_eq!(failed_code(&report).1, "invalid_pin");
    let path = world.path(&format!("operator/profile-tools/{UPPER}.json"));
    let mut descriptor: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let mut runtime = None;
    for pin in report["pins"].as_array().unwrap() {
        assert_eq!(pin["tool_id"], UPPER);
        let calculated = pin["calculated"].clone();
        assert!(calculated.is_string(), "{pin:#}");
        match pin["pin"].as_str().unwrap() {
            "schema_sha256" => {
                assert_eq!(pin["declared"], "1".repeat(64));
                descriptor["schema_sha256"] = calculated;
            }
            "executable" => {
                assert_eq!(pin["declared"], "2".repeat(64));
                descriptor["process"]["executable"]["sha256"] = calculated;
            }
            "dependency_directory" => {
                assert_eq!(pin["declared"], "3".repeat(64));
                descriptor["process"]["dependency_directories"][0]["sha256"] = calculated;
            }
            "runtime_sha256" => runtime = Some(calculated),
            "descriptor_sha256" => assert_eq!(pin["declared"], Value::Null),
            other => panic!("unexpected pin {other}"),
        }
    }
    world.write_descriptor(UPPER, &descriptor.to_string());

    let (code, report) = world.check(false, &arguments);
    assert_eq!(code, Some(0), "{report:#}");
    for pin in report["pins"].as_array().unwrap() {
        if !pin["declared"].is_null() {
            assert_eq!(pin["declared"], pin["calculated"], "{pin:#}");
        }
    }
    // The shown values are the ones the run's admission binds: its own admission, in this
    // process, over the same operator root.
    let shown = |name: &str| {
        report["pins"]
            .as_array()
            .unwrap()
            .iter()
            .find(|pin| pin["pin"] == name)
            .map(|pin| pin["calculated"].as_str().unwrap().to_string())
            .unwrap()
    };
    commandagent::planner::extension_profiles::register(&world.path("extension")).unwrap();
    // SAFETY: the only variable this binary sets in its own process, read only by this test's
    // admission; every child gets an environment of its own.
    unsafe { std::env::set_var("COMMANDAGENT_ADAPTER_CONFIG_ROOT", world.path("operator")) };
    let admitted = commandagent::tools::profile_tools::admit_selected(
        &world.path("workspace"),
        TOOL_PROFILE,
        None,
    );
    // SAFETY: as above.
    unsafe { std::env::remove_var("COMMANDAGENT_ADAPTER_CONFIG_ROOT") };
    let admitted = admitted.unwrap().expect("admitted");
    let binding = &admitted.bindings()[0];
    assert_eq!(binding.schema_sha256(), shown("schema_sha256"));
    assert_eq!(binding.descriptor_sha256(), shown("descriptor_sha256"));
    assert_eq!(
        binding.runtime_sha256(),
        Some(shown("runtime_sha256").as_str())
    );
    assert!(runtime.is_some());
    // The real run admits it: it gets past every refusal before its first provider call, which
    // reaches the listener and is answered by nothing.
    let (_, error) = world.run(false, &arguments);
    assert!(!error.contains("profile_tool_refused"), "{error}");
    assert!(world.listener.count() > 0, "{error}");
}

/// The human form says the same, without JSON, and a configuration the run refuses to resolve is
/// the run's exit 2. Both go through the same whole-world comparison.
#[test]
fn the_human_form_and_a_refused_configuration() {
    let world = World::new();
    let mut command = world.command(false, &lm_studio());
    command.arg("--operator-check");
    let output = world.invoke(command);
    assert_eq!(output.status.code(), Some(0));
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(
        text.starts_with("CommandAgent operator check: pass"),
        "{text}"
    );
    assert!(text.contains("Not checked by design:"), "{text}");

    let mut command = world.command(false, &["--preset", "no-such-preset"]);
    command.args(["--operator-check", "--json"]);
    let output = world.invoke(command);
    assert_eq!(output.status.code(), Some(2), "{}", last_error(&output));
    assert!(output.stdout.is_empty());
    let mut tail = String::new();
    std::io::Cursor::new(output.stderr)
        .read_to_string(&mut tail)
        .unwrap();
    assert!(tail.starts_with("error:"), "{tail}");
}

/// claude-4 S2: an operator's plain call, with the state directory and the events at their
/// defaults (under the fake `HOME` and the workspace), and a ledger directory two levels from
/// being made. Nothing anywhere in the world is made.
#[test]
fn defaults_and_a_ledger_directory_not_made_yet_leave_nothing_behind() {
    let world = World::new();
    world.declare_v2(|policy| policy["ledger_dir"] = json!(world.path("ledgers/one/two")));
    let mut command = Command::new(env!("CARGO_BIN_EXE_commandagent"));
    world.configure_with(&mut command, true, &responses(), true);
    command.args(["--operator-check", "--json"]);
    let (code, report) = world.check_with(command);
    assert_eq!(code, Some(0), "{report:#}");
    assert_eq!(check_of(&report, "run_budget.ledger")["code"], "absent");
    assert!(!world.path("ledgers/one").exists());
    assert!(!world.path("home/.commandagent").exists());
}

/// claude-4 S2: another process holds the wallet's lock while the check runs. The check takes no
/// lock, so it neither waits nor writes, and the lock file stays as it was.
#[test]
fn a_wallet_lock_held_elsewhere_is_neither_waited_for_nor_touched() {
    use std::os::fd::AsRawFd as _;
    let world = World::new();
    world.declare_v2(|_| {});
    let (code, error) = world.run(true, &responses());
    assert_eq!(code, Some(1), "{error}");
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .open(world.path(&format!("ledgers/{WALLET}/lock")))
        .unwrap();
    // SAFETY: `lock` is an open descriptor this test owns, and flock only uses it.
    let held = unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    assert_eq!(held, 0);
    let started = std::time::Instant::now();
    let (code, report) = world.check(true, &responses());
    assert!(started.elapsed() < Duration::from_secs(30));
    assert_eq!(code, Some(0), "{report:#}");
    assert_eq!(check_of(&report, "run_budget.ledger")["code"], "continues");
    // The run, by contrast, is refused while the lock is held.
    let (_, error) = world.run(true, &responses());
    assert_eq!(run_code(&error), "Wallet(Held)");
    drop(lock);
}

/// M1: a value registered as a secret in the run's scope is printed nowhere: not in a declared
/// pin, a pinned path, or the temporary parent, in either form. The report keeps its shape.
#[test]
fn a_registered_secret_is_scrubbed_from_the_whole_report() {
    const SECRET: &str = "dummy-credential-1013-value";
    let world = World::new();
    let hidden = world.path(&format!("tools/{SECRET}"));
    std::fs::create_dir_all(&hidden).unwrap();
    world.write_tool(UPPER, "out/upper.json", &[FIXED_INPUT], |d| {
        d["schema_sha256"] = json!(SECRET);
        d["process"]["dependency_directories"][0]["path"] = json!(hidden);
    });
    let temporary = world.path(&format!("tmp/{SECRET}"));
    std::fs::create_dir_all(&temporary).unwrap();
    let arguments = [&lm_studio()[..], &["--profile", TOOL_PROFILE][..]].concat();
    for json in [true, false] {
        let mut command = world.command(false, &arguments);
        command
            .env("LM_STUDIO_API_TOKEN", SECRET)
            .env("TMPDIR", &temporary)
            .arg("--operator-check");
        if json {
            command.arg("--json");
        }
        let output = world.invoke(command);
        let stdout = String::from_utf8(output.stdout).unwrap();
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(!stdout.contains(SECRET), "{stdout}");
        assert!(!stderr.contains(SECRET), "{stderr}");
        assert_eq!(output.status.code(), Some(1), "{stdout}{stderr}");
        if json {
            let report: Value = serde_json::from_str(&stdout).unwrap();
            assert_eq!(report["schema_version"], "commandagent.operator-check/v1");
            assert_eq!(check_of(&report, "profile_tools")["code"], "invalid_pin");
            let schema = report["pins"]
                .as_array()
                .unwrap()
                .iter()
                .find(|pin| pin["pin"] == "schema_sha256")
                .unwrap();
            assert!(schema["declared"].as_str().unwrap().contains("redacted"));
            assert!(
                check_of(&report, "isolation.probe_cleanup")["data"]
                    .get("temporary_parent")
                    .is_some()
            );
        } else {
            assert!(stdout.contains("profile_tools [invalid_pin]"), "{stdout}");
        }
    }
}

/// M3: a `--prompt` run builds the executor's client only, so a planner on a route the wallet
/// cannot count is not a failure: the check passes, and the real run gets past client admission
/// to the missing key.
#[test]
fn a_planner_route_a_prompt_run_never_builds_does_not_fail_the_check() {
    let world = World::new();
    world.declare_v2(|_| {});
    let arguments = [
        &responses()[..],
        &["--planner-provider", "ollama", "--planner-model", "p-1"][..],
    ]
    .concat();
    let (code, report) = world.check(true, &arguments);
    assert_eq!(code, Some(0), "{report:#}");
    assert_eq!(
        check_of(&report, "run_budget.route.executor")["status"],
        "pass"
    );
    assert_eq!(
        check_of(&report, "run_budget.route.planner")["status"],
        "not_checked"
    );
    let (_, error) = world.run(true, &arguments);
    assert!(error.contains("OPENAI_API_KEY"), "{error}");
    assert!(!error.contains("RouteNotCountable"), "{error}");
    assert_eq!(world.listener.count(), 0);
}

/// M4: under a wallet, `--request-pattern jev` is refused with the run's own word, by the same
/// function, with nothing sent. The route is the generic compatible one, which needs no key.
#[test]
fn request_pattern_routing_under_a_wallet_has_the_runs_own_code() {
    let world = World::new();
    world.declare_v1();
    let base_url = format!("{}/v1", world.listener.url);
    let arguments = [
        "--provider",
        "openai-compatible",
        "--base-url",
        &base_url,
        "--api",
        "chat-completions",
        "--model",
        "m-1",
        "--request-pattern",
        "jev",
    ];
    let (code, report) = world.check(true, &arguments);
    assert_eq!(code, Some(1), "{report:#}");
    let (id, check) = failed_code(&report);
    assert_eq!(
        (id.as_str(), check.as_str()),
        ("request_pattern", "Unmetered(RequestPattern)")
    );
    let (_, error) = world.run(true, &arguments);
    assert_eq!(check, run_code(&error), "{error}");
    assert_eq!(world.listener.count(), 0);
    // Without the routing, the same wallet passes.
    let (code, report) = world.check(true, &arguments[..8]);
    assert_eq!(code, Some(0), "{report:#}");
}

/// Out of scope noted in review: `--sandbox required` where the backend cannot start (an outer
/// sandbox that refuses to start `sandbox-exec` again, a real environment and not a seam). Under a
/// wallet both refuse with `NoSandboxPolicy`; without one the run starts and so the check passes,
/// showing the sandbox as not applied.
#[cfg(target_os = "macos")]
#[test]
fn a_required_sandbox_without_a_backend_has_the_runs_own_answer() {
    let world = World::new();
    world.declare_v2(|_| {});
    let nested = |wallet: bool, arguments: &[&str], extra: &[&str]| {
        let mut command = Command::new("/usr/bin/sandbox-exec");
        command
            .args([
                "-p",
                "(version 1)(allow default)(deny process-exec (literal \"/usr/bin/sandbox-exec\"))",
            ])
            .arg(env!("CARGO_BIN_EXE_commandagent"));
        world.configure(&mut command, wallet, arguments);
        command.args(["--sandbox", "required"]).args(extra);
        command
    };
    let (code, report) =
        world.check_with(nested(true, &responses(), &["--operator-check", "--json"]));
    assert_eq!(code, Some(1));
    assert_eq!(check_of(&report, "os_sandbox")["code"], "not_applied");
    let (id, check) = failed_code(&report);
    assert_eq!(
        (id.as_str(), check.as_str()),
        ("run_budget.declaration", "NoSandboxPolicy")
    );
    let output = nested(true, &responses(), &["--prompt", "say hi"])
        .output()
        .unwrap();
    assert_eq!(check, run_code(&last_error(&output)));

    let (code, report) =
        world.check_with(nested(false, &lm_studio(), &["--operator-check", "--json"]));
    assert_eq!(code, Some(0), "{report:#}");
    assert_eq!(check_of(&report, "os_sandbox")["code"], "not_applied");
}
