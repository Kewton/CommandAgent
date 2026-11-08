//! Every test that reads the selection runs in a child process of its own, with an empty
//! environment, a fake HOME, and only the variables its row sets. Every send goes to a fake on
//! 127.0.0.1, which counts what reaches it.

use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use serde_json::json;

use super::super::admission::RUN_BUDGET_ENV;
use super::super::identity::SendPurpose;
use super::super::ledger::{CommandWrites, LedgerEntry, LedgerOpen};
use super::super::policy::RunBudgetPolicy;
use super::super::test_support::{BODY, FakeClock, WALLET, policy_value};
use super::super::transport::{BudgetedBody, RunBudgetHandle, SendRefusal};
use super::{
    EntryRefusal, Unmetered, commands_are_handed, commands_inherit, open_for_run, open_noting,
    refuse, refuse_sandbox_off,
};
use crate::config::Config;
use crate::providers::{AssistantReply, ChatClient};
use crate::state::{ConversationMessage, SessionSnapshot};
use crate::tools::os_sandbox::{
    self, SandboxAvailability, SandboxMode, SandboxPolicy, SandboxTestState,
};
use crate::tools::registry::ToolSpec;

const CHILD: &str = "COMMANDAGENT_TEST_BUDGET_ENTRY_CHILD";
const ROOT_ENV: &str = "COMMANDAGENT_ADAPTER_CONFIG_ROOT";

/// The line a child prints once all its checks are through.
fn ran(name: &str) -> String {
    format!("child-976-3b-ran {name}")
}

/// Runs the child test `name` of this module alone, with `env` its whole environment beside a
/// fake HOME and the marker, and returns what it printed.
fn run_child(name: &str, env: &[(&str, &str)]) -> String {
    let env = env
        .iter()
        .map(|(variable, value)| (*variable, OsStr::new(*value)))
        .collect::<Vec<_>>();
    run_child_os(name, &env)
}

/// [`run_child`], with values that need not be text.
fn run_child_os(name: &str, env: &[(&str, &OsStr)]) -> String {
    let home = tempfile::tempdir().unwrap();
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            &format!("structured_budget::entry::tests::{name}"),
            "--nocapture",
            "--test-threads=1",
        ])
        .env_clear()
        .env("HOME", home.path())
        .env(CHILD, "marker-976-3b")
        .envs(env.iter().copied());
    let output = tests::output_of(&mut command);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{name}\n{stdout}\n{stderr}");
    assert!(stdout.contains("1 passed"), "{name}\n{stdout}\n{stderr}");
    assert!(
        stdout.contains(&ran(name)),
        "{name}: the child did not run its checks\n{stdout}\n{stderr}"
    );
    stdout
}

fn is_child() -> bool {
    std::env::var_os(CHILD).is_some()
}

/// A fake on 127.0.0.1 that answers every request with `body` and counts them.
struct Fake {
    url: String,
    stop: Arc<AtomicBool>,
    thread: JoinHandle<()>,
    seen: Arc<AtomicUsize>,
}

impl Fake {
    fn spawn(body: &'static str) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        listener.set_nonblocking(true).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let seen = Arc::new(AtomicUsize::new(0));
        let (thread_stop, thread_seen) = (stop.clone(), seen.clone());
        let thread = std::thread::spawn(move || {
            while !thread_stop.load(Ordering::SeqCst) {
                let Ok((mut stream, _)) = listener.accept() else {
                    std::thread::sleep(Duration::from_millis(5));
                    continue;
                };
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut data = Vec::new();
                let mut buffer = [0_u8; 4096];
                while !data.windows(4).any(|window| window == b"\r\n\r\n") {
                    match stream.read(&mut buffer) {
                        Ok(0) | Err(_) => break,
                        Ok(read) => data.extend_from_slice(&buffer[..read]),
                    }
                }
                thread_seen.fetch_add(1, Ordering::SeqCst);
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
            }
        });
        Self {
            url,
            stop,
            thread,
            seen,
        }
    }

    /// Stops the fake once every request it was sent has been read, and says how many there were.
    fn finish(self) -> usize {
        self.stop.store(true, Ordering::SeqCst);
        self.thread.join().unwrap();
        self.seen.load(Ordering::SeqCst)
    }
}

const MODELS: &str = r#"{"object":"list","data":[{"id":"m-1"}],"models":[{"name":"m-1"}]}"#;

fn base() -> (tempfile::TempDir, PathBuf) {
    let root = tempfile::tempdir().unwrap();
    let base = std::fs::canonicalize(root.path()).unwrap();
    (root, base)
}

fn config(base: &Path, extra: &[&str]) -> Config {
    let (cwd, state) = (base.join("workspace"), base.join("state"));
    std::fs::create_dir_all(&cwd).unwrap();
    let mut arguments = vec![
        "commandagent".to_string(),
        "--cwd".to_string(),
        cwd.to_str().unwrap().to_string(),
        "--state-dir".to_string(),
        state.to_str().unwrap().to_string(),
        "--model".to_string(),
        "m-1".to_string(),
    ];
    arguments.extend(extra.iter().map(ToString::to_string));
    let parsed = crate::provider_cli::parse_from(arguments).unwrap();
    Config::from_cli_with_provider_options(parsed.cli, parsed.provider_options).unwrap()
}

const COMPATIBLE: [&str; 4] = [
    "--provider",
    "openai-compatible",
    "--base-url",
    "http://127.0.0.1:9/v1",
];

/// A world an open can read: an operator root holding the declaration of [`WALLET`], ledgers
/// beside it, and the sandbox of this process installed and applied on the workspace.
struct World {
    _root: tempfile::TempDir,
    _sandbox: SandboxTestState,
    base: PathBuf,
    operator: PathBuf,
    config: Config,
}

impl World {
    fn new(extra: &[&str]) -> Self {
        let sandbox = SandboxTestState::acquire();
        let (root, base) = base();
        let operator = base.join("operator");
        std::fs::create_dir_all(operator.join("run-budgets")).unwrap();
        let mut value = policy_value();
        value["ledger_dir"] = json!(base.join("ledgers").to_str().unwrap());
        std::fs::write(
            operator.join("run-budgets").join(format!("{WALLET}.json")),
            value.to_string(),
        )
        .unwrap();
        let mut arguments = COMPATIBLE.to_vec();
        arguments.extend_from_slice(&["--prompt", "say hello"]);
        arguments.extend_from_slice(extra);
        let config = config(&base, &arguments);
        let run_tmp = base.join("state/sandbox/run-first");
        std::fs::create_dir_all(&run_tmp).unwrap();
        os_sandbox::install_for_test(
            SandboxMode::Required,
            SandboxPolicy::with_home(&config.workspace_root, &run_tmp, base.join("home")),
            SandboxAvailability::Available,
        );
        point_root(&operator);
        Self {
            _root: root,
            _sandbox: sandbox,
            base,
            operator,
            config,
        }
    }

    fn touched_by_open(&mut self) -> (Result<(), EntryRefusal>, Vec<&'static str>) {
        touched_by(true, &mut self.config)
    }

    /// An open, and whether it made the ledgers' directory.
    fn opened(mut self) -> (Result<(), EntryRefusal>, bool) {
        let (opened, _) = self.touched_by_open();
        (opened, self.base.join("ledgers").exists())
    }
}

/// An open, and what it read, in order.
fn touched_by(
    uses_write_sandbox: bool,
    config: &mut Config,
) -> (Result<(), EntryRefusal>, Vec<&'static str>) {
    let mut touched = Vec::new();
    let opened = open_noting(uses_write_sandbox, config, &mut |what| touched.push(what));
    (opened, touched)
}

/// The operator root, as the operator sets it. Only a child sets it, alone on its one thread.
fn point_root(root: &Path) {
    // SAFETY: a child runs one test on one thread, and nothing else reads the environment then.
    unsafe { std::env::set_var(ROOT_ENV, root) };
}

// ---- off: nothing is read ----

/// Runs only as a child. With no selection, an open with everything it could read in place reads
/// none of it, makes nothing, and leaves the configuration as it was; every way out passes.
#[test]
fn off_reads_nothing_and_changes_nothing_child() {
    if !is_child() {
        return;
    }
    assert!(std::env::var_os(RUN_BUDGET_ENV).is_none());
    let mut world = World::new(&[]);
    let before = world.config.field_sources.clone();
    let (opened, touched) = world.touched_by_open();
    assert_eq!(opened, Ok(()));
    assert_eq!(touched, Vec::<&str>::new());
    assert_eq!(open_for_run(true, &mut world.config), Ok(()));
    assert_eq!(world.config.field_sources, before);
    assert!(world.config.field_sources.run_budget.is_none());
    assert!(!world.base.join("ledgers").exists());
    for way in [
        Unmetered::Workflow,
        Unmetered::ModelProbe,
        Unmetered::RequestPattern,
        Unmetered::GuiSend,
        Unmetered::ModelCompletion,
        Unmetered::ProviderCheck,
        Unmetered::StartupProbe,
    ] {
        assert_eq!(refuse(way), Ok(()), "{way:?}");
    }
    assert!(crate::refuse_unmetered_gui_send().is_ok());
    assert_eq!(refuse_sandbox_off(SandboxMode::Off), Ok(()));
    println!("{}", ran("off_reads_nothing_and_changes_nothing_child"));
}

/// The control: with the selection, the same open reads the root, the declaration, the clock, and
/// the ledger, in that order, and holds the wallet on the configuration.
#[test]
fn on_opens_the_wallet_once_after_the_sandbox_child() {
    if !is_child() {
        return;
    }
    let mut world = World::new(&[]);
    let (opened, touched) = world.touched_by_open();
    assert_eq!(opened, Ok(()));
    assert_eq!(touched, ["root", "policy", "clock", "ledger"]);
    let handle = world.config.field_sources.run_budget.clone().unwrap();
    assert!(
        world
            .base
            .join("ledgers")
            .join(WALLET)
            .join("ledger.json")
            .exists()
    );
    // Already held: not opened again.
    let (again, touched) = world.touched_by_open();
    assert_eq!(again, Ok(()));
    assert_eq!(touched, Vec::<&str>::new());
    assert!(Arc::ptr_eq(
        world.config.field_sources.run_budget.as_ref().unwrap(),
        &handle
    ));
    println!(
        "{}",
        ran("on_opens_the_wallet_once_after_the_sandbox_child")
    );
}

#[test]
fn off_reads_nothing_and_on_opens_the_wallet_once() {
    run_child("off_reads_nothing_and_changes_nothing_child", &[]);
    run_child(
        "on_opens_the_wallet_once_after_the_sandbox_child",
        &[(RUN_BUDGET_ENV, WALLET)],
    );
}

// ---- what the open refuses ----

/// Runs only as a child, with the selection. Each row is refused before a wallet is made, and
/// says which refusal it is.
#[test]
fn an_open_refuses_what_it_cannot_vouch_for_child() {
    if !is_child() {
        return;
    }
    use super::super::ledger::LedgerRefusal;
    use super::super::policy::PolicyRefusal;
    type Arrange = fn(&World);
    type Row<'a> = (&'a str, &'a [&'a str], Arrange, Result<(), EntryRefusal>);
    let rows: Vec<Row<'_>> = vec![
        ("the control", &[], |_| {}, Ok(())),
        (
            "a key of its own",
            &["--api-key-env", "GATEWAY_KEY_976"],
            |_| {},
            Ok(()),
        ),
        (
            "a key in LC_X",
            &["--api-key-env", "LC_X"],
            |_| {},
            Err(EntryRefusal::KeyReachesCommands),
        ),
        (
            "a key in npm_config_token",
            &["--api-key-env", "npm_config_token"],
            |_| {},
            Err(EntryRefusal::KeyReachesCommands),
        ),
        (
            "a key in PATH",
            &["--api-key-env", "PATH"],
            |_| {},
            Err(EntryRefusal::KeyReachesCommands),
        ),
        (
            "a key in NODE_OPTIONS",
            &["--api-key-env", "NODE_OPTIONS"],
            |_| {},
            Err(EntryRefusal::KeyReachesCommands),
        ),
        (
            "a key in TMPDIR",
            &["--api-key-env", "TMPDIR"],
            |_| {},
            Err(EntryRefusal::KeyReachesCommands),
        ),
        (
            "no sandbox applied",
            &[],
            |_| os_sandbox::reset_for_test(),
            Err(EntryRefusal::NoSandboxPolicy),
        ),
        (
            "no operator root",
            &[],
            // SAFETY: a child runs one test on one thread, and nothing else reads the environment.
            |_| unsafe { std::env::remove_var(ROOT_ENV) },
            Err(EntryRefusal::NoOperatorRoot),
        ),
        (
            "a relative operator root",
            &[],
            |_| point_root(Path::new("relative/operator")),
            Err(EntryRefusal::NoOperatorRoot),
        ),
        (
            "a declaration in the workspace",
            &[],
            |world| {
                let inside = world.config.workspace_root.join("operator");
                std::fs::create_dir_all(inside.join("run-budgets")).unwrap();
                std::fs::copy(
                    world
                        .operator
                        .join("run-budgets")
                        .join(format!("{WALLET}.json")),
                    inside.join("run-budgets").join(format!("{WALLET}.json")),
                )
                .unwrap();
                point_root(&inside);
            },
            Err(EntryRefusal::PolicyInCommandReach),
        ),
        (
            "no declaration",
            &[],
            |world| {
                let elsewhere = world.base.join("empty-operator");
                std::fs::create_dir_all(&elsewhere).unwrap();
                point_root(&elsewhere);
            },
            Err(EntryRefusal::PolicyUnreadable),
        ),
        (
            "a declaration through a link",
            &[],
            |world| {
                let linked = world.base.join("linked-operator");
                std::fs::create_dir_all(&linked).unwrap();
                std::os::unix::fs::symlink(
                    world.operator.join("run-budgets"),
                    linked.join("run-budgets"),
                )
                .unwrap();
                point_root(&linked);
            },
            Err(EntryRefusal::PolicyUnreadable),
        ),
        (
            "a declaration that does not decode",
            &[],
            |world| {
                std::fs::write(
                    world
                        .operator
                        .join("run-budgets")
                        .join(format!("{WALLET}.json")),
                    b"not a declaration",
                )
                .unwrap();
            },
            Err(EntryRefusal::Policy(PolicyRefusal::Malformed)),
        ),
        (
            "a resume with no ledger",
            &["--resume", "session-976"],
            |_| {},
            Err(EntryRefusal::Wallet(LedgerRefusal::Missing)),
        ),
    ];
    let mut wrong = Vec::new();
    for (name, extra, arrange, expected) in rows {
        let world = World::new(extra);
        arrange(&world);
        let (opened, made) = world.opened();
        // A refusal of the ledger's own comes after its place is made, as it did before.
        let reached_the_ledger = matches!(expected, Ok(()) | Err(EntryRefusal::Wallet(_)));
        if opened != expected || made != reached_the_ledger {
            wrong.push(format!("{name}: {opened:?}, ledgers made {made}"));
        }
    }

    // An action with no write sandbox, and a workflow, open nothing and read nothing.
    let mut world = World::new(&[]);
    let (opened, touched) = touched_by(false, &mut world.config);
    if opened != Ok(()) || !touched.is_empty() || world.config.field_sources.run_budget.is_some() {
        wrong.push(format!("no write sandbox: {opened:?} {touched:?}"));
    }
    world.config.action =
        crate::config::Action::Workflow(world.base.join("flow.yaml"), world.base.clone());
    let (opened, touched) = touched_by(true, &mut world.config);
    if opened != Ok(()) || !touched.is_empty() || world.config.field_sources.run_budget.is_some() {
        wrong.push(format!("a workflow: {opened:?} {touched:?}"));
    }
    drop(world);
    assert_eq!(wrong, Vec::<String>::new());
    println!("{}", ran("an_open_refuses_what_it_cannot_vouch_for_child"));
}

/// Runs only as a child, with a selection that is no ID.
#[test]
fn an_id_that_is_no_id_is_refused_child() {
    if !is_child() {
        return;
    }
    let mut world = World::new(&[]);
    let (opened, touched) = world.touched_by_open();
    assert_eq!(opened, Err(EntryRefusal::InvalidBudgetId));
    assert_eq!(touched, Vec::<&str>::new());
    println!("{}", ran("an_id_that_is_no_id_is_refused_child"));
}

#[test]
fn an_open_refuses_what_it_cannot_vouch_for() {
    run_child(
        "an_open_refuses_what_it_cannot_vouch_for_child",
        &[(RUN_BUDGET_ENV, WALLET)],
    );
    for id in ["", "../wallet-alpha", ".hidden"] {
        run_child(
            "an_id_that_is_no_id_is_refused_child",
            &[(RUN_BUDGET_ENV, id)],
        );
    }
}

/// Runs only as a child, whose selection is there and cannot be read as an ID: empty, or not
/// text. It is a selection all the same and never off: every way out is refused, the sandbox off
/// is refused, the open refuses the ID before it reads anything, and the factory makes no client.
#[test]
fn a_selection_that_cannot_be_read_is_no_less_a_selection_child() {
    if !is_child() {
        return;
    }
    assert!(super::selected());
    let mut wrong = Vec::new();
    for way in [
        Unmetered::Workflow,
        Unmetered::ModelProbe,
        Unmetered::RequestPattern,
        Unmetered::GuiSend,
        Unmetered::ModelCompletion,
        Unmetered::ProviderCheck,
        Unmetered::StartupProbe,
    ] {
        if refuse(way) != Err(EntryRefusal::Unmetered(way)) {
            wrong.push(format!("{way:?}"));
        }
    }
    if crate::refuse_unmetered_gui_send().is_ok() {
        wrong.push("the GUI's sends".to_string());
    }
    if refuse_sandbox_off(SandboxMode::Off) != Err(EntryRefusal::SandboxOff) {
        wrong.push("the sandbox off".to_string());
    }
    let mut world = World::new(&[]);
    let (opened, touched) = world.touched_by_open();
    if opened != Err(EntryRefusal::InvalidBudgetId) || !touched.is_empty() {
        wrong.push(format!("open: {opened:?} {touched:?}"));
    }
    let admitted =
        super::super::admission::admit_client(&world.config, crate::config::ProviderRole::Executor);
    if admitted != Err(super::super::admission::AdmissionRefusal::SelectedWithNoWallet) {
        wrong.push(format!("factory: {admitted:?}"));
    }
    assert!(!world.base.join("ledgers").exists());
    assert_eq!(wrong, Vec::<String>::new());
    println!(
        "{}",
        ran("a_selection_that_cannot_be_read_is_no_less_a_selection_child")
    );
}

/// A value of the selection that is not text.
#[cfg(unix)]
fn not_text() -> OsString {
    use std::os::unix::ffi::OsStrExt;
    OsStr::from_bytes(&[0xff]).to_os_string()
}

#[test]
fn a_selection_that_cannot_be_read_is_no_less_a_selection() {
    let mut values = vec![OsString::new()];
    #[cfg(unix)]
    values.push(not_text());
    for value in values {
        run_child_os(
            "a_selection_that_cannot_be_read_is_no_less_a_selection_child",
            &[(RUN_BUDGET_ENV, value.as_os_str())],
        );
    }
}

// ---- the copy of what a command inherits ----

/// Runs only as a child, whose environment the parent filled with candidates. What
/// `bounded_process` passes on, inherited and handed, is exactly what the copy here says.
#[test]
fn the_copy_of_what_commands_inherit_is_what_they_inherit_child() {
    if !is_child() {
        return;
    }
    let names_passed = |command: &mut std::process::Command| -> BTreeSet<String> {
        command.stdout(std::process::Stdio::piped());
        let child = crate::bounded_process::spawn_child(command).unwrap();
        let output = child.wait_with_output().unwrap();
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter_map(|line| line.split_once('=').map(|(name, _)| name.to_string()))
            .collect()
    };
    let inherited = names_passed(&mut std::process::Command::new("/usr/bin/env"));
    let expected: BTreeSet<String> = std::env::vars()
        .map(|(name, _)| name)
        .filter(|name| commands_inherit(name))
        .collect();
    assert_eq!(inherited, expected);
    assert!(inherited.contains("LC_X") && inherited.contains("npm_config_cache"));

    let handed = [
        "NODE_ENV",
        "NODE_OPTIONS",
        "NODE_PATH",
        "PLAYWRIGHT_BROWSERS_PATH",
        "NEXT_TELEMETRY_DISABLED",
        "PORT",
        "TMPDIR",
        "COMMANDAGENT_FOREIGN_TOOLCHAIN_ROOT",
        "COMMANDAGENT_FOREIGN_TOOLCHAIN_EVENTS",
        "OPENAI_API_KEY",
        "GATEWAY_KEY_976",
        "LC_HANDED",
    ];
    let mut command = std::process::Command::new("/usr/bin/env");
    for name in handed {
        command.env(name, "handed-976");
    }
    let passed = names_passed(&mut command);
    for name in handed {
        assert_eq!(
            passed.contains(name),
            commands_inherit(name) || commands_are_handed(name),
            "{name}"
        );
    }
    println!(
        "{}",
        ran("the_copy_of_what_commands_inherit_is_what_they_inherit_child")
    );
}

#[test]
fn the_copy_of_what_commands_inherit_is_what_they_inherit() {
    run_child(
        "the_copy_of_what_commands_inherit_is_what_they_inherit_child",
        &[
            ("PATH", "/usr/bin:/bin"),
            ("LANG", "C"),
            ("TERM", "dumb"),
            ("RUSTUP_HOME", "/nonexistent/rustup"),
            ("CARGO_HOME", "/nonexistent/cargo"),
            ("LC_X", "fake-key-976-not-real"),
            ("LC_ALL", "C"),
            ("npm_config_cache", "/nonexistent/npm"),
            ("NODE_ENV", "test"),
            ("NODE_OPTIONS", "--no-warnings"),
            ("TMPDIR", "/nonexistent/tmp"),
            ("PORT", "3000"),
            ("OPENAI_API_KEY", "fake-key-976-not-real"),
            ("GEMINI_API_KEY", "fake-key-976-not-real"),
            ("LM_STUDIO_API_TOKEN", "fake-key-976-not-real"),
            ("JEV_API_KEY", "fake-key-976-not-real"),
            ("GATEWAY_KEY_976", "fake-key-976-not-real"),
            ("USER", "someone"),
            ("SHELL", "/bin/sh"),
        ],
    );
}

/// The production allowlist of `bounded_process` as it is written: every string in
/// `child_env_allowed`, up to the names only a test build allows. A name added there is a
/// candidate here without anyone listing it.
fn allowlist_literals() -> Vec<String> {
    const SOURCE: &str = include_str!("../bounded_process.rs");
    let start = SOURCE.find("fn child_env_allowed(").unwrap();
    let end = start
        + SOURCE[start..]
            .find("fn test_control_env_allowed(")
            .unwrap();
    SOURCE[start..end]
        .split('"')
        .skip(1)
        .step_by(2)
        .map(str::to_string)
        .collect()
}

/// Every name the allowlist could let through, and the near misses beside each: the name, one
/// longer, its other cases, its legacy spelling, and the prefixes' own edges.
fn candidate_names() -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for literal in allowlist_literals() {
        let mut spellings = vec![literal.clone()];
        spellings.extend(crate::env_compat::legacy_name(&literal));
        for spelling in spellings {
            names.insert(format!("{spelling}X976"));
            names.insert(spelling.to_lowercase());
            names.insert(spelling.to_uppercase());
            names.insert(spelling);
        }
    }
    for edge in [
        "LC",
        "LCX",
        "lc_x",
        "npm_config",
        "npmconfig_x",
        "NPM_CONFIG_X",
    ] {
        names.insert(edge.to_string());
    }
    names
        .into_iter()
        .filter(|name| !name.is_empty() && !name.contains('='))
        .collect()
}

/// Runs only as a child, whose environment the parent filled with every candidate. No name a
/// child of `bounded_process` gets, inherited or handed, is one the copy would take as a key.
#[test]
fn every_name_the_allowlist_lets_through_is_refused_as_a_key_child() {
    if !is_child() {
        return;
    }
    let literals = allowlist_literals();
    assert!(
        literals.len() >= 17
            && ["PATH", "LC_", "npm_config_", "TMPDIR"]
                .iter()
                .all(|known| literals.iter().any(|literal| literal == known)),
        "the allowlist was not found where it is read: {literals:?}"
    );
    let passed = |command: &mut std::process::Command| -> BTreeSet<String> {
        command.stdout(std::process::Stdio::piped());
        let child = crate::bounded_process::spawn_child(command).unwrap();
        let output = child.wait_with_output().unwrap();
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter_map(|line| line.split_once('=').map(|(name, _)| name.to_string()))
            .collect()
    };
    let candidates = candidate_names();
    let inherited = passed(&mut std::process::Command::new("/usr/bin/env"));
    let mut handed_command = std::process::Command::new("/usr/bin/env");
    for name in &candidates {
        handed_command.env(name, "handed-976");
    }
    let handed = passed(&mut handed_command);
    let mut wrong = Vec::new();
    for name in &candidates {
        assert!(
            std::env::var_os(name).is_some(),
            "{name} is not in this child"
        );
        if inherited.contains(name) != commands_inherit(name) {
            wrong.push(format!("{name}: inherited {}", inherited.contains(name)));
        }
        if handed.contains(name) && !(commands_inherit(name) || commands_are_handed(name)) {
            wrong.push(format!(
                "{name}: handed, and the copy would take it as a key"
            ));
        }
        if commands_are_handed(name) && !handed.contains(name) {
            wrong.push(format!("{name}: the copy says handed, and it is not"));
        }
    }
    assert_eq!(wrong, Vec::<String>::new());
    println!(
        "{}",
        ran("every_name_the_allowlist_lets_through_is_refused_as_a_key_child")
    );
}

#[test]
fn every_name_the_allowlist_lets_through_is_refused_as_a_key() {
    let candidates = candidate_names()
        .into_iter()
        .map(|name| (name, "candidate-976".to_string()))
        .collect::<Vec<_>>();
    let env = candidates
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_str()))
        .collect::<Vec<_>>();
    run_child(
        "every_name_the_allowlist_lets_through_is_refused_as_a_key_child",
        &env,
    );
}

// ---- a key named as a control of the run budget (F1) ----

/// Runs only as a child, with no selection: the names alone decide, whatever the mode.
#[test]
fn a_key_named_as_a_control_is_refused_whatever_the_mode_child() {
    if !is_child() {
        return;
    }
    assert!(!super::selected());
    let (_root, base) = base();
    let mut wrong = Vec::new();
    for (key, expected) in [
        ("COMMANDAGENT_RUN_BUDGET", Err(EntryRefusal::KeyIsControl)),
        ("ANVIL_RUN_BUDGET", Err(EntryRefusal::KeyIsControl)),
        (
            "COMMANDAGENT_ADAPTER_CONFIG_ROOT",
            Err(EntryRefusal::KeyIsControl),
        ),
        ("ANVIL_ADAPTER_CONFIG_ROOT", Err(EntryRefusal::KeyIsControl)),
        ("COMMANDAGENT_RUN_BUDGETX", Ok(())),
        ("commandagent_run_budget", Ok(())),
        ("GATEWAY_KEY_976", Ok(())),
    ] {
        let mut arguments = COMPATIBLE.to_vec();
        arguments.extend_from_slice(&["--api-key-env", key]);
        let config = config(&base, &arguments);
        let got = super::refuse_a_control_named_as_key(&config);
        if got != expected {
            wrong.push(format!("{key}: {got:?}"));
        }
    }
    let config = config(&base, &COMPATIBLE);
    if super::refuse_a_control_named_as_key(&config) != Ok(()) {
        wrong.push("no key".to_string());
    }
    assert_eq!(wrong, Vec::<String>::new());
    println!(
        "{}",
        ran("a_key_named_as_a_control_is_refused_whatever_the_mode_child")
    );
}

#[test]
fn a_key_named_as_a_control_is_refused_whatever_the_mode() {
    run_child(
        "a_key_named_as_a_control_is_refused_whatever_the_mode_child",
        &[],
    );
}

/// The doctor with `extra` on its command line: how many requests reached the fake, and its
/// provider checks with the refusal of its run budget check.
fn doctor_with(base: &Path, extra: &[&str]) -> Outcome {
    let fake = Fake::spawn(MODELS);
    let url = format!("{}/v1", fake.url);
    let (cwd, state) = (base.join("workspace"), base.join("state"));
    std::fs::create_dir_all(&cwd).unwrap();
    let mut arguments = vec![
        "commandagent",
        "--cwd",
        cwd.to_str().unwrap(),
        "--state-dir",
        state.to_str().unwrap(),
        "--model",
        "m-1",
        "--provider",
        "openai-compatible",
        "--base-url",
        &url,
        "--doctor",
    ];
    arguments.extend_from_slice(extra);
    let parsed = crate::provider_cli::parse_from(arguments).unwrap();
    let report =
        crate::doctor::diagnose_cli_with_provider_options(&parsed.cli, parsed.provider_options)
            .unwrap();
    let provider_checks = report
        .checks
        .iter()
        .filter(|check| check.id.starts_with("provider."))
        .map(|check| {
            format!(
                "{} {:?} {}",
                check.id, check.status, check.details["refusal"]
            )
        })
        .collect::<Vec<_>>();
    (fake.finish(), provider_checks.join(", "))
}

/// Runs only as a child, with no selection. The doctor and the shell's model completion return
/// before the run's own check of the key's name, so each makes it: a key named as a control of the
/// run budget, or its legacy spelling, sends nothing there either, whatever the mode. A key of its
/// own name, or none, sends as before. The completion uses no key: it judges only the name the
/// start of its command line took, which the real binary's tests cover.
#[test]
fn the_early_ways_out_refuse_a_key_named_as_a_control_child() {
    if !is_child() {
        return;
    }
    assert!(!super::selected());
    let refused = "provider.run_budget Fail \"the run budget refused the entry: KeyIsControl\"";
    let mut wrong = Vec::new();
    for (key, expected_refused) in [
        ("COMMANDAGENT_RUN_BUDGET", true),
        ("ANVIL_RUN_BUDGET", true),
        ("COMMANDAGENT_ADAPTER_CONFIG_ROOT", true),
        ("ANVIL_ADAPTER_CONFIG_ROOT", true),
        ("GATEWAY_KEY_976", false),
    ] {
        let (_root, base) = base();
        let (sent, said) = doctor_with(&base, &["--api-key-env", key]);
        let got_refused = sent == 0 && said == refused;
        let sent_as_before = sent > 0 && !said.contains("provider.run_budget");
        if expected_refused != got_refused || expected_refused == sent_as_before {
            wrong.push(format!("doctor {key}: sent {sent}: {said}"));
        }
    }

    // This child's own command line is the test harness's, which names no key: the start of it
    // takes none, and the completion sends as before. The names a start takes are judged on the
    // real binary, in `tests/structured_run_budget.rs`.
    assert_eq!(super::refuse_a_control_the_start_takes(), Ok(()));
    let fake = Fake::spawn(MODELS);
    let candidates =
        crate::cli_completion::complete_model_ids_from_hosts(OsStr::new(""), &fake.url, &fake.url);
    let sent = fake.finish();
    if sent == 0 || candidates.len() != 1 {
        wrong.push(format!(
            "completion: sent {sent}, {} candidates",
            candidates.len()
        ));
    }
    assert_eq!(wrong, Vec::<String>::new());
    println!(
        "{}",
        ran("the_early_ways_out_refuse_a_key_named_as_a_control_child")
    );
}

#[test]
fn the_early_ways_out_refuse_a_key_named_as_a_control() {
    run_child(
        "the_early_ways_out_refuse_a_key_named_as_a_control_child",
        &[],
    );
}

/// The judgement of what the start took out, on every operating system. When the start said it
/// took a control, that is refused, and the name found again is not even asked for; only when it
/// said nothing is it asked for, and judged as before. Which names the start says yes to is the
/// same predicate the name checks use. It reads no environment, so it needs no child of its own.
#[test]
fn what_the_start_took_is_judged_before_any_name_found_again() {
    use super::{is_a_control, refuse_a_control_taken_or_named as judged};
    fn asked() -> Option<String> {
        panic!("the name was found again beside the start's word that it took a control")
    }
    fn selection() -> Option<String> {
        Some(RUN_BUDGET_ENV.to_string())
    }
    fn alias() -> Option<String> {
        Some("GATEWAY_KEY_976".to_string())
    }
    fn none() -> Option<String> {
        None
    }
    let refused = Err(EntryRefusal::KeyIsControl);
    assert_eq!(judged(true, asked), refused);
    assert_eq!(judged(false, selection), refused);
    assert_eq!(judged(false, alias), Ok(()));
    assert_eq!(judged(false, none), Ok(()));
    for (name, control) in [
        (RUN_BUDGET_ENV, true),
        ("ANVIL_RUN_BUDGET", true),
        (ROOT_ENV, true),
        ("ANVIL_ADAPTER_CONFIG_ROOT", true),
        ("COMMANDAGENT_RUN_BUDGETX", false),
        ("commandagent_run_budget", false),
        ("GATEWAY_KEY_976", false),
    ]
    .into_iter()
    .chain(
        crate::startup_credentials::BUILT_IN_CREDENTIAL_NAMES
            .into_iter()
            .map(|name| (name, false)),
    ) {
        assert_eq!(is_a_control(name), control, "{name}");
    }
}

/// Runs only as a child, with no selection. This child's own command line is the test harness's,
/// so the start's judgement passes (see above), and a run's refusal of a key named as a control
/// is the configuration's alone: the run is refused for it, and nothing is sent.
#[test]
fn a_run_refuses_the_configurations_control_name_by_itself_child() {
    if !is_child() {
        return;
    }
    assert!(!super::selected());
    assert_eq!(super::refuse_a_control_the_start_takes(), Ok(()));
    let mut wrong = Vec::new();
    for key in [
        RUN_BUDGET_ENV,
        "ANVIL_RUN_BUDGET",
        ROOT_ENV,
        "ANVIL_ADAPTER_CONFIG_ROOT",
    ] {
        let (_root, base) = base();
        let fake = Fake::spawn(MODELS);
        let url = format!("{}/v1", fake.url);
        let (cwd, state) = (base.join("workspace"), base.join("state"));
        std::fs::create_dir_all(&cwd).unwrap();
        let parsed = crate::provider_cli::parse_from([
            "commandagent",
            "--cwd",
            cwd.to_str().unwrap(),
            "--state-dir",
            state.to_str().unwrap(),
            "--model",
            "m-1",
            "--provider",
            "openai-compatible",
            "--base-url",
            &url,
            "--api-key-env",
            key,
            "--prompt",
            "say hello",
        ])
        .unwrap();
        let result = crate::run_with_provider_options(parsed.cli, parsed.provider_options);
        let sent = fake.finish();
        let said = result.as_ref().err().map(|error| format!("{error:#}"));
        let refused = said
            .as_deref()
            .is_some_and(|said| said.contains("the run budget refused the entry: KeyIsControl"));
        if !refused || sent != 0 {
            wrong.push(format!("{key}: sent {sent}: {said:?}"));
        }
    }
    assert_eq!(wrong, Vec::<String>::new());
    println!(
        "{}",
        ran("a_run_refuses_the_configurations_control_name_by_itself_child")
    );
}

#[test]
fn a_run_refuses_the_configurations_control_name_by_itself() {
    run_child(
        "a_run_refuses_the_configurations_control_name_by_itself_child",
        &[],
    );
}

// ---- the sandbox at the send (B5) ----

/// Runs only as a child. A wallet opened under the sandbox applied on it, and then the sandbox
/// changed before a send: only the very places the wallet was judged against let it begin.
#[test]
fn a_send_needs_the_sandbox_the_wallet_was_opened_under_child() {
    if !is_child() {
        return;
    }
    let _sandbox = SandboxTestState::acquire();
    type Change = fn(&Path, &SandboxPolicy);
    let rows: [(&str, Change, Option<SendRefusal>); 7] = [
        ("as opened", |_, _| {}, None),
        (
            "installed again, as it was",
            |_, policy| {
                os_sandbox::install_for_test(
                    SandboxMode::Required,
                    policy.clone(),
                    SandboxAvailability::Available,
                );
            },
            None,
        ),
        (
            "installed again on the same workspace with a new temporary",
            |base, policy| {
                let run_tmp = base.join("state/sandbox/run-second");
                std::fs::create_dir_all(&run_tmp).unwrap();
                let mut again = policy.clone();
                again.run_tmp_dir = run_tmp;
                os_sandbox::install_for_test(
                    SandboxMode::Required,
                    again,
                    SandboxAvailability::Available,
                );
            },
            Some(SendRefusal::SandboxChangedSinceOpen),
        ),
        (
            "installed again on the same workspace and temporary with a cache added",
            |base, policy| {
                let home = base.join("home");
                std::fs::create_dir_all(home.join(".npm")).unwrap();
                let again =
                    SandboxPolicy::with_home(&policy.workspace_root, &policy.run_tmp_dir, home);
                assert!(again.caches.npm.is_some());
                os_sandbox::install_for_test(
                    SandboxMode::Required,
                    again,
                    SandboxAvailability::Available,
                );
            },
            Some(SendRefusal::SandboxChangedSinceOpen),
        ),
        (
            "installed again on another workspace",
            |base, policy| {
                let other = base.join("other");
                std::fs::create_dir_all(&other).unwrap();
                let mut again = policy.clone();
                again.workspace_root = other;
                os_sandbox::install_for_test(
                    SandboxMode::Required,
                    again,
                    SandboxAvailability::Available,
                );
            },
            Some(SendRefusal::SandboxOnAnotherWorkspace),
        ),
        (
            "no policy",
            |_, _| os_sandbox::reset_for_test(),
            Some(SendRefusal::SandboxNotApplied),
        ),
        (
            "installed again as it was, with no backend to apply it",
            |_, policy| {
                os_sandbox::install_for_test(
                    SandboxMode::Required,
                    policy.clone(),
                    SandboxAvailability::Unavailable("no-backend-976-3b".to_string()),
                );
            },
            Some(SendRefusal::SandboxNotApplied),
        ),
    ];
    let mut wrong = Vec::new();
    for (name, change, expected) in rows {
        let (_root, base) = base();
        let workspace = base.join("workspace");
        let run_tmp = base.join("state/sandbox/run-first");
        std::fs::create_dir_all(&workspace).unwrap();
        std::fs::create_dir_all(&run_tmp).unwrap();
        let policy = SandboxPolicy::with_home(&workspace, &run_tmp, base.join("home"));
        os_sandbox::install_for_test(
            SandboxMode::Required,
            policy.clone(),
            SandboxAvailability::Available,
        );
        let mut value = policy_value();
        value["ledger_dir"] = json!(base.join("ledgers").to_str().unwrap());
        let handle = Arc::new(
            RunBudgetHandle::open(LedgerOpen {
                policy: Arc::new(
                    RunBudgetPolicy::decode(value.to_string().as_bytes(), WALLET).unwrap(),
                ),
                workspace: &workspace,
                explicit_profile: None,
                entry: LedgerEntry::Fresh,
                clock: FakeClock::at(7_000),
                command_writes: CommandWrites::applied(),
            })
            .unwrap(),
        );
        change(&base, &policy);
        let body: serde_json::Value = serde_json::from_slice(BODY).unwrap();
        let budgeted = BudgetedBody::new(&handle, &body, SendPurpose::Executor).unwrap();
        let got = match budgeted
            .begin(reqwest::Client::new().post("http://127.0.0.1:9/v1/chat/completions"))
        {
            Ok((_request, send)) => {
                drop(send);
                None
            }
            Err(refusal) => Some(refusal),
        };
        let reserved = handle.wallet().unwrap().physical_used();
        if got != expected || reserved != u64::from(expected.is_none()) {
            wrong.push(format!("{name}: {got:?}, reserved {reserved}"));
        }
    }
    assert_eq!(wrong, Vec::<String>::new());
    println!(
        "{}",
        ran("a_send_needs_the_sandbox_the_wallet_was_opened_under_child")
    );
}

#[test]
fn a_send_needs_the_sandbox_the_wallet_was_opened_under() {
    run_child(
        "a_send_needs_the_sandbox_the_wallet_was_opened_under_child",
        &[],
    );
}

// ---- every way out sends nothing under a selection; with none, it sends as it did ----

struct Counting(Arc<AtomicUsize>);

impl ChatClient for Counting {
    fn label(&self) -> &str {
        "counting-976"
    }

    fn boxed_clone(&self) -> Box<dyn ChatClient> {
        Box::new(Self(self.0.clone()))
    }

    fn chat(
        &mut self,
        _model: &str,
        _messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        self.0.fetch_add(1, Ordering::SeqCst);
        anyhow::bail!("the counting client sends nothing")
    }
}

fn refusal_of(error: &anyhow::Error) -> Option<EntryRefusal> {
    error
        .chain()
        .find_map(|cause| cause.downcast_ref::<EntryRefusal>())
        .copied()
}

/// What one way out did: how many requests reached the fake, and what it said.
type Outcome = (usize, String);

fn doctor(base: &Path) -> Outcome {
    let fake = Fake::spawn(MODELS);
    let url = format!("{}/v1", fake.url);
    let (cwd, state) = (base.join("workspace"), base.join("state"));
    std::fs::create_dir_all(&cwd).unwrap();
    let parsed = crate::provider_cli::parse_from([
        "commandagent",
        "--cwd",
        cwd.to_str().unwrap(),
        "--state-dir",
        state.to_str().unwrap(),
        "--model",
        "m-1",
        "--provider",
        "openai-compatible",
        "--base-url",
        &url,
        "--doctor",
    ])
    .unwrap();
    let report =
        crate::doctor::diagnose_cli_with_provider_options(&parsed.cli, parsed.provider_options)
            .unwrap();
    let provider_checks = report
        .checks
        .iter()
        .filter(|check| check.id.starts_with("provider."))
        .map(|check| format!("{} {:?}", check.id, check.status))
        .collect::<Vec<_>>();
    (fake.finish(), provider_checks.join(", "))
}

fn startup(base: &Path) -> Outcome {
    let fake = Fake::spawn(MODELS);
    let url = format!("{}/v1", fake.url);
    let config = config(
        base,
        &["--provider", "openai-compatible", "--base-url", &url],
    );
    assert!(matches!(config.action, crate::config::Action::Repl));
    let warnings = crate::providers::startup::warnings(&config, true);
    (fake.finish(), warnings.join("\n"))
}

fn jev(base: &Path, extra: &[&str], offline: bool) -> Outcome {
    use crate::minimal_loop::request_pattern::{
        JevClassifier, RequestPatternMode, RequestPatternSettings, run_direct_prompt_with,
    };
    let fake = Fake::spawn(r#"{"label":"CREATE","confidence":0.99,"model":"jev-976"}"#);
    let classifier = JevClassifier::new(
        format!("{}/v1/systemone", fake.url),
        "test-key-976-not-real",
        Duration::from_secs(5),
    );
    let mut arguments = vec!["--prompt", "Create notes.md"];
    arguments.extend_from_slice(extra);
    let mut config = config(base, &arguments);
    config.offline = offline;
    let calls = Arc::new(AtomicUsize::new(0));
    let result = run_direct_prompt_with(
        &mut Counting(calls.clone()),
        &mut SessionSnapshot::new(),
        "Create notes.md",
        &config,
        &crate::tui::NOOP_UI,
        RequestPatternSettings {
            mode: RequestPatternMode::Jev,
            threshold: 0.8,
        },
        Some(&classifier),
    );
    let refusal = result.as_ref().err().and_then(refusal_of);
    let said = format!(
        "{refusal:?}, provider calls {}",
        calls.load(Ordering::SeqCst)
    );
    (fake.finish(), said)
}

fn completion(_: &Path) -> Outcome {
    let fake = Fake::spawn(MODELS);
    let candidates = crate::cli_completion::complete_model_ids_from_hosts(
        std::ffi::OsStr::new(""),
        &fake.url,
        &fake.url,
    );
    (fake.finish(), format!("{} candidates", candidates.len()))
}

fn model_probe(base: &Path) -> Outcome {
    let config = config(base, &COMPATIBLE);
    let calls = Arc::new(AtomicUsize::new(0));
    let result = crate::model_probe::run_with_output_dir(
        &config,
        &mut Counting(calls.clone()),
        &mut Counting(calls.clone()),
        base.join("model-profiles"),
    );
    let refusal = result.as_ref().err().and_then(refusal_of);
    (calls.load(Ordering::SeqCst), format!("{refusal:?}"))
}

fn workflow(base: &Path) -> Outcome {
    let config = config(base, &COMPATIBLE);
    let origin = base.join("origin");
    std::fs::create_dir_all(&origin).unwrap();
    let result =
        crate::workflow::orchestrator::run_workflow(&config, &base.join("absent.yaml"), &origin);
    let refusal = result.as_ref().err().and_then(refusal_of);
    let made = origin.join("evidence").exists();
    (usize::from(made), format!("{refusal:?}"))
}

/// The CLI with the sandbox off, and with it required. With no selection the run is not started:
/// only the decision is read, since a run with the sandbox off would be a run.
fn sandbox_off(base: &Path) -> Outcome {
    if !super::selected() {
        return (0, format!("{:?}", refuse_sandbox_off(SandboxMode::Off)));
    }
    let (cwd, state) = (base.join("workspace"), base.join("state"));
    std::fs::create_dir_all(&cwd).unwrap();
    let mut outcomes = Vec::new();
    for mode in ["off", "required"] {
        let parsed = crate::provider_cli::parse_from([
            "commandagent",
            "--cwd",
            cwd.to_str().unwrap(),
            "--state-dir",
            state.to_str().unwrap(),
            "--model",
            "m-1",
            "--provider",
            "openai-compatible",
            "--base-url",
            "http://127.0.0.1:9/v1",
            "--sandbox",
            mode,
            "--prompt",
            "say hello",
        ])
        .unwrap();
        let error = crate::run_with_provider_options(parsed.cli, parsed.provider_options)
            .err()
            .map(|error| {
                format!(
                    "exit {} {}",
                    crate::cli_error_exit_code(&error),
                    format!("{error:#}").replace(&base.display().to_string(), "<base>")
                )
            });
        outcomes.push(format!("{mode}: {error:?}"));
    }
    (0, outcomes.join("; "))
}

type Way = fn(&Path) -> Outcome;

/// Every way out a run under a selection could send by, each with no sandbox of its own.
fn ways() -> Vec<(&'static str, Way)> {
    vec![
        ("doctor", doctor),
        ("startup", startup),
        ("jev", |base| jev(base, &[], false)),
        ("jev with an intent", |base| {
            jev(base, &["--intent", "create"], false)
        }),
        ("jev offline", |base| jev(base, &[], true)),
        ("completion", completion),
        ("model probe", model_probe),
        ("workflow", workflow),
        ("sandbox", sandbox_off),
    ]
}

/// Runs only as a child, and prints one line for each way out.
#[test]
fn every_way_out_child() {
    if !is_child() {
        return;
    }
    for (name, way) in ways() {
        let (_root, base) = base();
        let (sent, said) = way(&base);
        println!("WAY {name}: sent {sent}: {said}");
    }
    println!("{}", ran("every_way_out_child"));
}

fn ways_out(env: &[(&str, &str)]) -> Vec<String> {
    run_child("every_way_out_child", env)
        .lines()
        // The first line of a child's output follows the harness's own words on the same line.
        .filter_map(|line| line.split_once("WAY ").map(|(_, way)| way.to_string()))
        .collect()
}

/// Under a selection, nothing a way out could send is sent, and each says why in fixed words.
/// With none, each sends as it did before; jev's classifier and the doctor's and the startup's
/// model lists reach the fake. (The model probe and the workflow are refused for other reasons
/// there, or run on, which the control does not need.)
#[test]
fn every_way_out_sends_nothing_under_a_selection_and_as_before_with_none() {
    let selected = ways_out(&[(RUN_BUDGET_ENV, WALLET)]);
    let refused = |way: &str| format!("Some(Unmetered({way}))");
    assert_eq!(
        selected,
        vec![
            "doctor: sent 0: provider.run_budget Fail".to_string(),
            format!(
                "startup: sent 0: {}",
                crate::providers::startup::STARTUP_PROBE_SKIPPED
            ),
            format!(
                "jev: sent 0: {}, provider calls 0",
                refused("RequestPattern")
            ),
            format!(
                "jev with an intent: sent 0: {}, provider calls 0",
                refused("RequestPattern")
            ),
            format!(
                "jev offline: sent 0: {}, provider calls 0",
                refused("RequestPattern")
            ),
            "completion: sent 0: 0 candidates".to_string(),
            format!("model probe: sent 0: {}", refused("ModelProbe")),
            format!("workflow: sent 0: {}", refused("Workflow")),
            "sandbox: sent 0: off: Some(\"exit 2 the run budget refused the entry: SandboxOff\"); \
             required: Some(\"exit 2 the run budget refused the entry: NoSandboxPolicy\")"
                .to_string(),
        ]
    );

    let off = ways_out(&[]);
    let sent = |name: &str| {
        off.iter()
            .find(|line| line.starts_with(&format!("{name}: sent ")))
            .unwrap_or_else(|| panic!("{name} missing: {off:?}"))
            .clone()
    };
    for name in ["doctor", "startup", "jev", "completion"] {
        assert!(
            !sent(name).starts_with(&format!("{name}: sent 0:")),
            "{off:?}"
        );
    }
    assert!(
        sent("doctor").contains("provider.openai_compatible.reachable Pass")
            && !sent("doctor").contains("provider.run_budget"),
        "{off:?}"
    );
    assert!(sent("completion").ends_with("1 candidates"), "{off:?}");
    assert!(
        !sent("model probe").contains("Unmetered") && !sent("workflow").contains("Unmetered"),
        "{off:?}"
    );
    assert_eq!(sent("sandbox"), "sandbox: sent 0: Ok(())", "{off:?}");
}

// ---- 3e: whatever the entry asks, the wallet is the one on disk ----

/// A configuration of this world's workspace, under `state`, with `extra`.
fn config_under(world: &World, state: &str, extra: &[&str]) -> Config {
    let state = world.base.join(state);
    let mut arguments = vec![
        "commandagent",
        "--cwd",
        world.config.workspace_root.to_str().unwrap(),
        "--state-dir",
        state.to_str().unwrap(),
        "--model",
        "m-1",
    ];
    arguments.extend_from_slice(&COMPATIBLE);
    arguments.extend_from_slice(&["--prompt", "say hello"]);
    arguments.extend_from_slice(extra);
    let parsed = crate::provider_cli::parse_from(arguments).unwrap();
    Config::from_cli_with_provider_options(parsed.cli, parsed.provider_options).unwrap()
}

/// An open of `config` as the run's entry opens one, and what its wallet then holds: sends made,
/// and the pinned profile. The handle is let go of after.
fn opened_holding(config: &mut Config) -> Result<(u64, Option<String>), EntryRefusal> {
    touched_by(true, config).0?;
    let handle = config.field_sources.run_budget.take().unwrap();
    let wallet = handle.wallet().unwrap();
    Ok((wallet.physical_used(), wallet.profile))
}

/// Runs only as a child, with the selection. A wallet spent once is spent the same whether the
/// next entry is fresh, a resume, a fresh session named with a resume, or under another state
/// directory; another explicit profile or another workspace with the same ID is refused.
#[test]
fn a_wallet_is_the_one_on_disk_whatever_the_entry_asks_child() {
    if !is_child() {
        return;
    }
    use super::super::ledger::LedgerRefusal;
    let mut world = World::new(&["--profile", "python-cli"]);
    let (opened, _) = world.touched_by_open();
    assert_eq!(opened, Ok(()));
    let handle = world.config.field_sources.run_budget.take().unwrap();
    let body: serde_json::Value = serde_json::from_slice(BODY).unwrap();
    let budgeted = BudgetedBody::new(&handle, &body, SendPurpose::Executor).unwrap();
    let (_request, send) = budgeted
        .begin(reqwest::Client::new().post("http://127.0.0.1:9/v1/chat/completions"))
        .unwrap();
    super::super::transport::settle_body(
        &mut Some(send),
        super::super::test_support::TestWallet::KNOWN,
    )
    .unwrap();
    assert_eq!(handle.wallet().unwrap().physical_used(), 1);
    drop(budgeted);
    drop(handle);

    let pinned = Some("python-cli".to_string());
    type Row<'a> = (&'a str, &'a str, &'a [&'a str]);
    let rows: [Row<'_>; 5] = [
        ("fresh", "state", &[]),
        ("a resume", "state", &["--resume", "session-976"]),
        (
            "a fresh session named with a resume",
            "state",
            &["--resume", "session-976", "--fresh-session"],
        ),
        ("a fresh session", "state", &["--fresh-session"]),
        ("another state directory", "state-other", &[]),
    ];
    let mut wrong = Vec::new();
    for (name, state, extra) in rows {
        let mut config = config_under(&world, state, extra);
        match opened_holding(&mut config) {
            Ok((1, profile)) if profile == pinned => {}
            other => wrong.push(format!("{name}: {other:?}")),
        }
    }
    let mut other_profile = config_under(&world, "state", &["--profile", "data"]);
    let refused = opened_holding(&mut other_profile);
    if refused != Err(EntryRefusal::Wallet(LedgerRefusal::ProfileChanged)) {
        wrong.push(format!("another profile: {refused:?}"));
    }

    // Another workspace, with the sandbox on it, and the same ID.
    let other = world.base.join("other-workspace");
    std::fs::create_dir_all(&other).unwrap();
    let run_tmp = world.base.join("state/sandbox/run-other");
    std::fs::create_dir_all(&run_tmp).unwrap();
    os_sandbox::install_for_test(
        SandboxMode::Required,
        SandboxPolicy::with_home(&other, &run_tmp, world.base.join("home")),
        SandboxAvailability::Available,
    );
    world.config.workspace_root = other;
    let mut elsewhere = config_under(&world, "state", &[]);
    let refused = opened_holding(&mut elsewhere);
    if refused != Err(EntryRefusal::Wallet(LedgerRefusal::WorkspaceChanged)) {
        wrong.push(format!("another workspace: {refused:?}"));
    }
    assert_eq!(wrong, Vec::<String>::new());
    println!(
        "{}",
        ran("a_wallet_is_the_one_on_disk_whatever_the_entry_asks_child")
    );
}

#[test]
fn a_wallet_is_the_one_on_disk_whatever_the_entry_asks() {
    run_child(
        "a_wallet_is_the_one_on_disk_whatever_the_entry_asks_child",
        &[(RUN_BUDGET_ENV, WALLET)],
    );
}

/// Where this file starts a child of its own. A unit test may start what production must not, and
/// the guardrail that keeps production from starting a process directly knows a test by its
/// `mod tests`, as in the other test files that start one. Its name is that word on purpose, so it
/// shares the name of the module this file is.
#[allow(
    clippy::module_inception,
    reason = "the guardrail knows a test that starts a child by its `mod tests`"
)]
#[cfg(test)]
mod tests {
    pub(super) fn output_of(command: &mut std::process::Command) -> std::process::Output {
        command.output().unwrap()
    }
}
