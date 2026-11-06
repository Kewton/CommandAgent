//! Issue #976, stage 3b: the binaries under a run budget, each run as an operator starts it, with
//! an empty environment and a fake HOME. Every send goes to a fake on 127.0.0.1, which counts
//! what reaches it.
//!
//! The command line goes through `main`'s start, which on macOS hands a provider key's variable
//! over and takes it out of the environment: a key named as a control of the run budget is
//! refused, by a run and by the doctor, and nothing is sent. A selection that is there and cannot
//! be read, empty or not text, is refused and never taken for off.
//!
//! The GUI's model discovery and its delegated runs are sends no wallet counts, and under a
//! selection neither goes out; with none, both go out as before. The GUI server is built only
//! with the `gui` feature, so those run with it, as `tests/gui_server.rs` does.

#![cfg(unix)]

use std::ffi::{OsStr, OsString};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::os::unix::ffi::OsStrExt;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
#[cfg(feature = "gui")]
use std::{
    io::{BufRead, BufReader},
    net::TcpStream,
    path::Path,
    process::Child,
    sync::atomic::AtomicUsize,
};

const RUN_BUDGET_ENV: &str = "COMMANDAGENT_RUN_BUDGET";
#[cfg(feature = "gui")]
const TOKEN: &str = "commandagent-gui-test-token-000000000976";
#[cfg(feature = "gui")]
const REFUSED: &str = "the run budget refused the entry: Unmetered(GuiSend)";

/// A fake on 127.0.0.1 that answers every request with a model list and counts them.
#[cfg(feature = "gui")]
struct Fake {
    url: String,
    stop: Arc<AtomicBool>,
    thread: JoinHandle<()>,
    seen: Arc<AtomicUsize>,
}

#[cfg(feature = "gui")]
impl Fake {
    fn spawn() -> Self {
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
                let body = r#"{"models":[{"name":"m-976"}],"data":[{"id":"m-976"}]}"#;
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

    fn finish(self) -> usize {
        self.stop.store(true, Ordering::SeqCst);
        self.thread.join().unwrap();
        self.seen.load(Ordering::SeqCst)
    }
}

#[cfg(feature = "gui")]
struct Server {
    child: Child,
    port: u16,
}

#[cfg(feature = "gui")]
impl Server {
    /// The GUI server alone in an environment of its own, with the selection when `selected`.
    fn start(
        home: &Path,
        static_dir: &Path,
        selected: bool,
        provider_host: &str,
        trial: Option<(&Path, &Path)>,
    ) -> Self {
        let mut command = Command::new(env!("CARGO_BIN_EXE_gui_server"));
        command
            .env_clear()
            .env("HOME", home)
            .env("PATH", "/usr/bin:/bin")
            .args(["--port", "0", "--base-path", "/"])
            .arg("--repository-root")
            .arg(env!("CARGO_MANIFEST_DIR"))
            .arg("--static-dir")
            .arg(static_dir)
            .args(["--ollama-host", provider_host])
            .args(["--lm-studio-host", provider_host])
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        if selected {
            command.env(RUN_BUDGET_ENV, "wallet-976");
        }
        if let Some((workspace, cli)) = trial {
            command
                .arg("--execution-root")
                .arg(workspace)
                .arg("--commandagent-bin")
                .arg(cli)
                .args(["--trial-token-auth", "on"])
                .env("GUI_TRIAL_TOKEN", TOKEN);
        }
        let mut child = command.spawn().unwrap();
        let mut stdout = BufReader::new(child.stdout.take().unwrap());
        let mut line = String::new();
        let read = stdout.read_line(&mut line).unwrap();
        assert!(read > 0, "the GUI server ended before it listened");
        let port = line
            .split("127.0.0.1:")
            .nth(1)
            .and_then(|tail| tail.split('/').next())
            .and_then(|port| port.trim().parse().ok())
            .unwrap_or_else(|| panic!("no address in {line}"));
        // The rest of what it prints goes nowhere, so it never blocks on a full pipe.
        std::thread::spawn(move || {
            let mut sink = String::new();
            while matches!(stdout.read_line(&mut sink), Ok(read) if read > 0) {
                sink.clear();
            }
        });
        Self { child, port }
    }

    fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<&serde_json::Value>,
        token: bool,
    ) -> (u16, String) {
        let body = body.map(ToString::to_string).unwrap_or_default();
        let access = if token {
            format!(
                "Authorization: Bearer {TOKEN}\r\nOrigin: http://127.0.0.1:{}\r\n",
                self.port
            )
        } else {
            String::new()
        };
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).unwrap();
        write!(
            stream,
            "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\n{access}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            self.port,
            body.len()
        )
        .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        let (head, body) = response.split_once("\r\n\r\n").unwrap();
        let status = head.split_whitespace().nth(1).unwrap().parse().unwrap();
        (status, body.to_string())
    }
}

#[cfg(feature = "gui")]
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Model discovery: under a selection the list is empty and nothing reaches the fake; with none,
/// the fake answers and its model is listed.
#[cfg(feature = "gui")]
#[test]
fn the_gui_lists_no_model_under_a_run_budget() {
    for selected in [true, false] {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("home");
        std::fs::create_dir_all(&home).unwrap();
        let fake = Fake::spawn();
        let server = Server::start(
            &home,
            &root.path().join("static"),
            selected,
            &fake.url,
            None,
        );
        let mut answers = Vec::new();
        for provider in ["ollama", "lm-studio"] {
            answers.push(server.request(
                "GET",
                &format!("/api/provider-models?provider={provider}"),
                None,
                false,
            ));
        }
        drop(server);
        let sent = fake.finish();
        if selected {
            assert_eq!(sent, 0, "{answers:?}");
            assert_eq!(
                answers,
                vec![(200, "[]".to_string()), (200, "[]".to_string())]
            );
        } else {
            assert_eq!(sent, 2, "{answers:?}");
            assert_eq!(
                answers,
                vec![
                    (200, r#"["m-976"]"#.to_string()),
                    (200, r#"["m-976"]"#.to_string())
                ]
            );
        }
    }
}

#[cfg(feature = "gui")]
fn write_cli(path: &Path, marker: &Path) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::write(
        path,
        format!(
            "#!/bin/sh\nif [ \"${{1-}}\" = \"--version\" ]; then printf 'commandagent 0.1.0 test\\n'; exit 0; fi\nprintf 'ran\\n' > '{}'\nprintf '%s\\n' '{{\"event\":\"tui_command_stop\",\"ok\":false,\"status\":\"failed\",\"assurance_level\":\"none\"}}' > \"$COMMANDAGENT_EVAL_EVENTS\"\n",
            marker.display()
        ),
    )
    .unwrap();
    let mut permissions = std::fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions).unwrap();
}

/// A delegated run: under a selection the GUI refuses it before it builds the command, so the
/// CLI never starts; with none, the session is accepted as before.
#[cfg(feature = "gui")]
#[test]
fn the_gui_delegates_no_run_under_a_run_budget() {
    for selected in [true, false] {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("home");
        let workspace = root.path().join("workspace");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&workspace).unwrap();
        let (cli, marker) = (root.path().join("cli"), root.path().join("cli-ran"));
        write_cli(&cli, &marker);
        let server = Server::start(
            &home,
            &root.path().join("static"),
            selected,
            "http://127.0.0.1:9",
            Some((&workspace, &cli)),
        );
        let spec = serde_json::json!({
            "goal": "Create a CLI --pattern filter command",
            "profile": "python-cli",
            "provider": "ollama",
            "model": "fixture-executor",
            "planner_provider": "ollama",
            "planner_model": "fixture-planner"
        });
        let (status, proposal) =
            server.request("POST", "/api/session-proposals", Some(&spec), true);
        assert_eq!(status, 200, "{proposal}");
        let proposal: serde_json::Value = serde_json::from_str(&proposal).unwrap();
        let mut confirmed = spec;
        confirmed["confirmation_hash"] = proposal["card_hash"].clone();
        let (status, body) = server.request("POST", "/api/sessions", Some(&confirmed), true);
        if selected {
            assert_ne!(status, 202, "{body}");
            assert!(body.contains(REFUSED), "{body}");
            drop(server);
            assert!(!marker.exists(), "the delegated CLI ran");
        } else {
            assert_eq!(status, 202, "{body}");
            assert!(!body.contains(REFUSED), "{body}");
        }
    }
}

// ---- the command line, through main's start (F1) ----

/// A completion a run takes as its answer, and a model list, in one body.
const REPLY: &str = r#"{"id":"c-976","object":"chat.completion","model":"m-1","choices":[{"index":0,"message":{"role":"assistant","content":"done"},"finish_reason":"stop"}],"usage":{"prompt_tokens":1,"completion_tokens":1,"cost":0.000001},"data":[{"id":"m-1"}]}"#;

/// A fake provider on 127.0.0.1 that answers everything with [`REPLY`] and keeps each request
/// line, with no header value.
struct ProviderFake {
    url: String,
    stop: Arc<AtomicBool>,
    thread: JoinHandle<()>,
    seen: Arc<Mutex<Vec<String>>>,
}

impl ProviderFake {
    fn spawn() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        listener.set_nonblocking(true).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let seen = Arc::new(Mutex::new(Vec::new()));
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
                let mut buffer = [0_u8; 8192];
                while !data.windows(4).any(|window| window == b"\r\n\r\n") {
                    match stream.read(&mut buffer) {
                        Ok(0) | Err(_) => break,
                        Ok(read) => data.extend_from_slice(&buffer[..read]),
                    }
                }
                let head = String::from_utf8_lossy(&data);
                thread_seen
                    .lock()
                    .unwrap()
                    .push(head.lines().next().unwrap_or("").to_string());
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{REPLY}",
                    REPLY.len()
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

    fn count(&self) -> usize {
        self.seen.lock().unwrap().len()
    }

    fn finish(self) -> Vec<String> {
        self.stop.store(true, Ordering::SeqCst);
        self.thread.join().unwrap();
        self.seen.lock().unwrap().clone()
    }
}

/// What one start of the binary did: its exit code, or `None` when it was stopped after a send
/// reached the fake; the requests the fake saw; its last `error` line; and what it printed.
#[derive(Debug)]
struct Started {
    code: Option<i32>,
    sent: Vec<String>,
    error: String,
    printed: String,
}

/// The command-line binary as an operator starts it, through `main`, with `env` its whole
/// environment beside a fake HOME and PATH, against a fake compatible provider.
fn start_cli(env: &[(&str, &str)], extra: &[&str]) -> Started {
    let env = env
        .iter()
        .map(|(variable, value)| (*variable, OsStr::new(*value)))
        .collect::<Vec<_>>();
    start_cli_os(&env, extra)
}

/// [`start_cli`], with values that need not be text.
fn start_cli_os(env: &[(&str, &OsStr)], extra: &[&str]) -> Started {
    let root = tempfile::tempdir().unwrap();
    let base = std::fs::canonicalize(root.path()).unwrap();
    let fake = ProviderFake::spawn();
    let (cwd, state, home) = (
        base.join("workspace"),
        base.join("state"),
        base.join("home"),
    );
    std::fs::create_dir_all(&cwd).unwrap();
    std::fs::create_dir_all(&home).unwrap();
    let url = format!("{}/v1", fake.url);
    // A file, not a pipe: nothing the child prints can make it wait on this test.
    let printed_path = base.join("printed.txt");
    let printed_file = std::fs::File::create(&printed_path).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_commandagent"))
        .env_clear()
        .env("HOME", &home)
        .env("PATH", "/usr/bin:/bin")
        .envs(env.iter().copied())
        .args(["--cwd", cwd.to_str().unwrap()])
        .args(["--state-dir", state.to_str().unwrap()])
        .args(["--provider", "openai-compatible", "--base-url", &url])
        .args(["--model", "m-1"])
        .args(extra)
        .stdin(Stdio::null())
        .stdout(Stdio::from(printed_file))
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // The verdict is what is observed, an exit or a request at the fake. The bound only turns
    // neither into a failure, and the child is stopped by its own handle.
    let deadline = Instant::now() + Duration::from_secs(60);
    let code = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status.code();
        }
        if fake.count() > 0 {
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        assert!(Instant::now() < deadline, "neither an exit nor a send");
        std::thread::sleep(Duration::from_millis(10));
    };
    let mut stderr = String::new();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr)
        .unwrap();
    let error = stderr
        .lines()
        .rfind(|line| line.starts_with("error"))
        .unwrap_or("")
        .to_string();
    Started {
        code,
        sent: fake.finish(),
        error,
        printed: std::fs::read_to_string(&printed_path).unwrap(),
    }
}

const KEY_IS_CONTROL: &str = "error: the run budget refused the entry: KeyIsControl";
/// The doctor's words when it refuses its provider checks for the key's name.
#[cfg(target_os = "macos")]
const DOCTOR_KEY_IS_CONTROL: &str =
    "the key's variable is a control of the run budget, so no provider check is sent";
/// The refusals the doctor's run budget check holds, as `--json` prints them.
const DOCTOR_REFUSED_BY_NAME: &str = "\"the run budget refused the entry: KeyIsControl\"";
const DOCTOR_REFUSED_BY_SELECTION: &str =
    "\"the run budget refused the entry: Unmetered(ProviderCheck)\"";

/// A value of the selection that is not text.
fn not_text() -> OsString {
    OsStr::from_bytes(&[0xff]).to_os_string()
}

/// macOS hands the key's variable over at the start and takes it out of the environment. Named as
/// the selection, the selection is gone: the run is refused by the key's name before anything is
/// sent, and so is the doctor's provider check. Another key name, under the same selection, is
/// refused as before, for what this wallet lacks. Linux hands nothing over, and is judged by the
/// tests below.
#[cfg(target_os = "macos")]
#[test]
fn a_key_named_as_the_selection_sends_nothing_through_the_start() {
    let selected = [(RUN_BUDGET_ENV, "wallet976")];
    let run = start_cli(
        &selected,
        &["--api-key-env", RUN_BUDGET_ENV, "--prompt", "say hello"],
    );
    assert_eq!((run.code, run.sent.len()), (Some(2), 0), "{run:?}");
    assert_eq!(run.error, KEY_IS_CONTROL, "{run:?}");

    let doctor = start_cli(&selected, &["--api-key-env", RUN_BUDGET_ENV, "--doctor"]);
    assert_eq!((doctor.code, doctor.sent.len()), (Some(1), 0), "{doctor:?}");
    assert_eq!(
        doctor.error, "error: doctor found failed checks",
        "{doctor:?}"
    );
    assert!(doctor.printed.contains(DOCTOR_KEY_IS_CONTROL), "{doctor:?}");

    let other = start_cli(
        &[
            (RUN_BUDGET_ENV, "wallet976"),
            ("GATEWAY_KEY_976", "synthetic-key-976"),
        ],
        &["--api-key-env", "GATEWAY_KEY_976", "--prompt", "say hello"],
    );
    assert_eq!((other.code, other.sent.len()), (Some(2), 0), "{other:?}");
    assert!(
        other
            .error
            .starts_with("error: the run budget refused the entry: ")
            && other.error != KEY_IS_CONTROL,
        "{other:?}"
    );
}

/// The names alone decide, whatever the mode: a key named as a control of the run budget, or as
/// its legacy spelling, is refused before the run on every system, with no selection at all, and
/// with the selection's own variable as the key. A key of its own name, with no selection, runs as
/// before and sends.
#[test]
fn a_key_named_as_a_control_is_refused_with_no_selection() {
    for (name, value) in [
        ("COMMANDAGENT_ADAPTER_CONFIG_ROOT", "synthetic-key-976"),
        (RUN_BUDGET_ENV, "synthetic-key-976"),
        ("ANVIL_RUN_BUDGET", "synthetic-key-976"),
    ] {
        let run = start_cli(
            &[(name, value)],
            &["--api-key-env", name, "--prompt", "say hello"],
        );
        assert_eq!((run.code, run.sent.len()), (Some(2), 0), "{name}: {run:?}");
        assert_eq!(run.error, KEY_IS_CONTROL, "{name}: {run:?}");
    }
    let run = start_cli(
        &[("GATEWAY_KEY_976", "synthetic-key-976")],
        &["--api-key-env", "GATEWAY_KEY_976", "--prompt", "say hello"],
    );
    assert!(!run.sent.is_empty(), "{run:?}");
}

/// macOS: a selection the start takes out of the environment and does not hand over, since it is
/// empty (with another key there to hand over) or not text. The doctor sends nothing, and fails
/// its run budget check for the key's name; a run is refused by the name too.
#[cfg(target_os = "macos")]
#[test]
fn a_selection_the_start_does_not_hand_over_sends_nothing() {
    let (empty, not_text, synthetic) =
        (OsString::new(), not_text(), OsString::from("synthetic-976"));
    let rows = [
        (
            "empty, beside another key",
            vec![
                (RUN_BUDGET_ENV, empty.as_os_str()),
                ("OPENAI_API_KEY", synthetic.as_os_str()),
            ],
        ),
        ("not text", vec![(RUN_BUDGET_ENV, not_text.as_os_str())]),
    ];
    for (label, env) in rows {
        let doctor = start_cli_os(
            &env,
            &["--api-key-env", RUN_BUDGET_ENV, "--doctor", "--json"],
        );
        assert_eq!(
            (doctor.code, doctor.sent.len()),
            (Some(1), 0),
            "{label}: {doctor:?}"
        );
        assert!(
            doctor.printed.contains("provider.run_budget")
                && doctor.printed.contains(DOCTOR_REFUSED_BY_NAME),
            "{label}: {doctor:?}"
        );
        let run = start_cli_os(
            &env,
            &["--api-key-env", RUN_BUDGET_ENV, "--prompt", "say hello"],
        );
        assert_eq!((run.code, run.sent.len()), (Some(2), 0), "{label}: {run:?}");
        assert_eq!(run.error, KEY_IS_CONTROL, "{label}: {run:?}");
    }
}

/// On every system. A selection that is there and cannot be read, empty or not text, is a
/// selection and never off: a run is refused for its ID and the doctor for the selection, and
/// nothing is sent. With no selection, a key named as a control of the run budget, or as its
/// legacy spelling, is refused by the doctor by its name alone, and nothing is sent; a key of its
/// own name sends as before.
#[test]
fn a_selection_that_cannot_be_read_and_a_key_named_as_a_control_send_nothing() {
    let synthetic = OsString::from("synthetic-key-976");
    for (label, selection) in [("empty", OsString::new()), ("not text", not_text())] {
        let env = [
            (RUN_BUDGET_ENV, selection.as_os_str()),
            ("GATEWAY_KEY_976", synthetic.as_os_str()),
        ];
        let run = start_cli_os(
            &env,
            &["--api-key-env", "GATEWAY_KEY_976", "--prompt", "say hello"],
        );
        assert_eq!((run.code, run.sent.len()), (Some(2), 0), "{label}: {run:?}");
        assert_eq!(
            run.error, "error: the run budget refused the entry: InvalidBudgetId",
            "{label}: {run:?}"
        );
        let doctor = start_cli_os(
            &env,
            &["--api-key-env", "GATEWAY_KEY_976", "--doctor", "--json"],
        );
        assert_eq!(
            (doctor.code, doctor.sent.len()),
            (Some(1), 0),
            "{label}: {doctor:?}"
        );
        assert!(
            doctor.printed.contains(DOCTOR_REFUSED_BY_SELECTION),
            "{label}: {doctor:?}"
        );
    }

    for name in [
        "COMMANDAGENT_ADAPTER_CONFIG_ROOT",
        "ANVIL_RUN_BUDGET",
        "ANVIL_ADAPTER_CONFIG_ROOT",
    ] {
        let doctor = start_cli(
            &[(name, "synthetic-key-976")],
            &["--api-key-env", name, "--doctor", "--json"],
        );
        assert_eq!(
            (doctor.code, doctor.sent.len()),
            (Some(1), 0),
            "{name}: {doctor:?}"
        );
        assert!(
            doctor.printed.contains(DOCTOR_REFUSED_BY_NAME),
            "{name}: {doctor:?}"
        );
    }

    let doctor = start_cli(
        &[("GATEWAY_KEY_976", "synthetic-key-976")],
        &["--api-key-env", "GATEWAY_KEY_976", "--doctor", "--json"],
    );
    assert!(!doctor.sent.is_empty(), "{doctor:?}");
}
