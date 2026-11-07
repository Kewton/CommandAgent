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
    let mut arguments = vec![
        "--provider",
        "openai-compatible",
        "--base-url",
        "<FAKE>/v1",
        "--model",
        "m-1",
    ];
    arguments.extend_from_slice(extra);
    launch(env, &arguments, None)
}

/// The presets a workspace of [`start_in_workspace`] declares: one that only names the
/// selection's variable as the key, and one whose own provider would use it.
#[cfg(target_os = "macos")]
const PRESETS: &str = "[preset.control976]\napi_key_env = \"COMMANDAGENT_RUN_BUDGET\"\n\n\
    [preset.compat976]\nprovider = \"openai-compatible\"\nbase_url = \"http://127.0.0.1:9/v1\"\n\
    api_key_env = \"COMMANDAGENT_RUN_BUDGET\"\n";

/// The binary through `main`, started in a workspace whose configuration declares [`PRESETS`],
/// with `arguments` after its directories and model.
#[cfg(target_os = "macos")]
fn start_in_workspace(env: &[(&str, &OsStr)], arguments: &[&str]) -> Started {
    let mut all = vec!["--model", "m-1"];
    all.extend_from_slice(arguments);
    launch(env, &all, Some(PRESETS))
}

/// One start of the binary, with `env` its whole environment beside a fake HOME and PATH, and
/// `arguments` after its directories. `<FAKE>`, in an argument or as a whole value in `env`, is
/// the fake provider's address. With a `configuration`, the workspace declares it and the binary
/// starts there.
fn launch(env: &[(&str, &OsStr)], arguments: &[&str], configuration: Option<&str>) -> Started {
    let workspace = configuration.map_or(Workspace::Bare, Workspace::Declares);
    launch_in(env, arguments, workspace).0
}

/// What the workspace's configuration is for one start.
#[derive(Clone, Copy)]
enum Workspace<'a> {
    /// None: the binary starts in this test's own directory.
    Bare,
    /// A file that declares this, and the binary starts there.
    Declares(&'a str),
    /// A named pipe that gives each read of the file the text for its number, counted from 1,
    /// and the binary starts there. See [`ServedReads`].
    #[cfg(target_os = "macos")]
    Serves(fn(usize) -> &'static str),
}

/// The workspace's `.commandagent/config.toml` as a symlink to a named pipe, one pipe for each
/// read. A thread opens the current pipe to write, and that open returns only when the binary
/// opens the file to read it. The thread then makes the next pipe and points the symlink at it,
/// and only after that writes this read's text and closes. The binary reads to the end, which
/// it cannot reach before that close, so its next open is of the next pipe: which text a read
/// gets is decided by its number alone, with no clock in it.
#[cfg(target_os = "macos")]
struct ServedReads {
    stop: Arc<AtomicBool>,
    link: std::path::PathBuf,
    thread: JoinHandle<usize>,
}

#[cfg(target_os = "macos")]
impl ServedReads {
    fn start(directory: &std::path::Path, text: fn(usize) -> &'static str) -> Self {
        let link = directory.join("config.toml");
        let directory = directory.to_path_buf();
        let pipe = move |read: usize| directory.join(format!("read-{read}.fifo"));
        let point = |link: &std::path::Path, target: &std::path::Path| {
            let next = link.with_extension("next");
            std::os::unix::fs::symlink(target, &next).unwrap();
            std::fs::rename(&next, link).unwrap();
        };
        make_pipe(&pipe(1));
        point(&link, &pipe(1));
        let stop = Arc::new(AtomicBool::new(false));
        let (thread_stop, thread_link) = (stop.clone(), link.clone());
        let thread = std::thread::spawn(move || {
            let mut read = 1;
            loop {
                let mut writer = std::fs::OpenOptions::new()
                    .write(true)
                    .open(pipe(read))
                    .unwrap();
                if thread_stop.load(Ordering::SeqCst) {
                    return read - 1;
                }
                make_pipe(&pipe(read + 1));
                point(&thread_link, &pipe(read + 1));
                let _ = writer.write_all(text(read).as_bytes());
                drop(writer);
                read += 1;
            }
        });
        Self { stop, link, thread }
    }

    /// How many reads were served. Opening the current pipe to read, without waiting, is what
    /// lets the thread's open return to see the stop.
    fn finish(self) -> usize {
        use std::os::unix::fs::OpenOptionsExt as _;
        self.stop.store(true, Ordering::SeqCst);
        let _reader = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(&self.link)
            .unwrap();
        self.thread.join().unwrap()
    }
}

#[cfg(target_os = "macos")]
fn make_pipe(path: &std::path::Path) {
    let path = std::ffi::CString::new(path.as_os_str().as_bytes()).unwrap();
    // SAFETY: `path` is a NUL-terminated string that lives through the call.
    assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
}

/// [`launch`] in a `workspace`, with how many reads of its configuration a served one gave.
fn launch_in(env: &[(&str, &OsStr)], arguments: &[&str], workspace: Workspace) -> (Started, usize) {
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
    let arguments = arguments
        .iter()
        .map(|argument| argument.replace("<FAKE>", &fake.url))
        .collect::<Vec<_>>();
    let env = env
        .iter()
        .map(|(variable, value)| match value.to_str() {
            Some("<FAKE>") => (*variable, OsString::from(&fake.url)),
            _ => (*variable, value.to_os_string()),
        })
        .collect::<Vec<_>>();
    // A file, not a pipe: nothing the child prints can make it wait on this test.
    let printed_path = base.join("printed.txt");
    let printed_file = std::fs::File::create(&printed_path).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_commandagent"));
    #[cfg(target_os = "macos")]
    let mut served = None;
    match workspace {
        Workspace::Bare => {}
        Workspace::Declares(configuration) => {
            std::fs::create_dir_all(cwd.join(".commandagent")).unwrap();
            std::fs::write(cwd.join(".commandagent/config.toml"), configuration).unwrap();
            command.current_dir(&cwd);
        }
        #[cfg(target_os = "macos")]
        Workspace::Serves(text) => {
            std::fs::create_dir_all(cwd.join(".commandagent")).unwrap();
            served = Some(ServedReads::start(&cwd.join(".commandagent"), text));
            command.current_dir(&cwd);
        }
    }
    let mut child = command
        .env_clear()
        .env("HOME", &home)
        .env("PATH", "/usr/bin:/bin")
        .envs(env)
        .args(["--cwd", cwd.to_str().unwrap()])
        .args(["--state-dir", state.to_str().unwrap()])
        .args(&arguments)
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
    #[cfg(target_os = "macos")]
    let reads = served.map_or(0, ServedReads::finish);
    #[cfg(not(target_os = "macos"))]
    let reads = 0;
    let started = Started {
        code,
        sent: fake.finish(),
        error,
        printed: std::fs::read_to_string(&printed_path).unwrap(),
    };
    (started, reads)
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

// ---- the name the start takes (F1-R1) ----

/// Every role on the fake Ollama, so a compatible endpoint is in no role and the configuration
/// keeps no key's name.
#[cfg(target_os = "macos")]
const EVERY_ROLE_OLLAMA: [&str; 6] = [
    "--provider",
    "ollama",
    "--planner-provider",
    "ollama",
    "--ollama-host",
    "<FAKE>",
];

#[cfg(target_os = "macos")]
fn joined(parts: &[&[&'static str]]) -> Vec<&'static str> {
    parts.iter().flat_map(|part| part.iter().copied()).collect()
}

/// macOS: the start takes the preset's `api_key_env` out of the environment whether or not a role
/// of the run uses it. With no role on a compatible endpoint the configuration keeps no name, and
/// the selection is gone all the same: the doctor and a run are refused by the name the start took,
/// and nothing is sent. Whether the preset's own provider is compatible and the command line puts
/// every role elsewhere, or the preset names no provider at all, is the same. A key of its own name
/// on the command line wins over the preset's in the start as in the configuration: with no
/// selection, that sends as before.
#[cfg(target_os = "macos")]
#[test]
fn a_preset_name_no_role_uses_is_judged_as_the_start_takes_it() {
    let selected = [(RUN_BUDGET_ENV, OsStr::new("wallet976"))];
    for (label, preset) in [
        ("the preset names no provider", "control976"),
        ("the command line overrides the preset's", "compat976"),
    ] {
        let named = joined(&[&["--preset", preset], &EVERY_ROLE_OLLAMA]);
        let doctor = start_in_workspace(&selected, &joined(&[&named, &["--doctor", "--json"]]));
        assert_eq!(
            (doctor.code, doctor.sent.len()),
            (Some(1), 0),
            "{label}: {doctor:?}"
        );
        assert!(
            doctor.printed.contains(DOCTOR_REFUSED_BY_NAME),
            "{label}: {doctor:?}"
        );
        let run = start_in_workspace(&selected, &joined(&[&named, &["--prompt", "say hello"]]));
        assert_eq!((run.code, run.sent.len()), (Some(2), 0), "{label}: {run:?}");
        assert_eq!(run.error, KEY_IS_CONTROL, "{label}: {run:?}");
    }

    let own = [("GATEWAY_KEY_976", OsStr::new("synthetic-key-976"))];
    let over_the_preset = [
        "--preset",
        "control976",
        "--provider",
        "openai-compatible",
        "--base-url",
        "<FAKE>/v1",
        "--api-key-env",
        "GATEWAY_KEY_976",
    ];
    for action in [&["--doctor", "--json"][..], &["--prompt", "say hello"][..]] {
        let started = start_in_workspace(&own, &joined(&[&over_the_preset, action]));
        assert!(!started.sent.is_empty(), "{action:?}: {started:?}");
    }
}

/// macOS: what the start takes out of the environment and what the run refuses, row by row.
///
/// The first start has the selection and `--sandbox off`. The run's first act after the start is
/// the sandbox's refusal under a selection (`lib.rs`, before the configuration and before any name
/// is judged), which reads only whether the variable is in this process's environment: refused for
/// `SandboxOff` means the selection is still there; anything else means the start took it out.
/// That start must then end refused by the key's name, with nothing sent. The second start has no
/// selection, and says whether the names the run judges are refused. A row where the start takes
/// the selection out and the names pass is the drift this looks for.
#[cfg(target_os = "macos")]
#[test]
fn what_the_start_takes_out_is_what_the_run_refuses() {
    const COMPATIBLE: [&str; 4] = ["--provider", "openai-compatible", "--base-url", "<FAKE>/v1"];
    const PROMPT: [&str; 2] = ["--prompt", "say hello"];
    const SANDBOX_OFF: &str = "error: the run budget refused the entry: SandboxOff";
    // (row, the arguments, whether the start takes the selection out, whether the run refuses)
    let rows: [(&str, Vec<&str>, bool, bool); 10] = [
        (
            "the command line names the selection",
            joined(&[&COMPATIBLE, &["--api-key-env", RUN_BUDGET_ENV], &PROMPT]),
            true,
            true,
        ),
        (
            "the command line names the selection's legacy spelling",
            joined(&[&COMPATIBLE, &["--api-key-env", "ANVIL_RUN_BUDGET"], &PROMPT]),
            false,
            true,
        ),
        (
            "the command line names the operator root",
            joined(&[
                &COMPATIBLE,
                &["--api-key-env", "COMMANDAGENT_ADAPTER_CONFIG_ROOT"],
                &PROMPT,
            ]),
            false,
            true,
        ),
        (
            "a preset names the selection and a role uses it",
            joined(&[
                &["--preset", "compat976", "--base-url", "<FAKE>/v1"],
                &PROMPT,
            ]),
            true,
            true,
        ),
        (
            "a preset names the selection and no role uses it",
            joined(&[&["--preset", "control976"], &EVERY_ROLE_OLLAMA, &PROMPT]),
            true,
            true,
        ),
        (
            "the command line puts the preset's roles elsewhere",
            joined(&[&["--preset", "compat976"], &EVERY_ROLE_OLLAMA, &PROMPT]),
            true,
            true,
        ),
        (
            "the command line's own name over the preset's",
            joined(&[
                &["--preset", "control976"],
                &COMPATIBLE,
                &["--api-key-env", "GATEWAY_KEY_976"],
                &PROMPT,
            ]),
            false,
            false,
        ),
        (
            "a name of its own",
            joined(&[&COMPATIBLE, &["--api-key-env", "GATEWAY_KEY_976"], &PROMPT]),
            false,
            false,
        ),
        ("no name", joined(&[&COMPATIBLE, &PROMPT]), false, false),
        (
            "the name only after `--`, where the shell puts what it completes",
            joined(&[
                &COMPATIBLE,
                &PROMPT,
                &["--", "commandagent", "--api-key-env", RUN_BUDGET_ENV],
            ]),
            false,
            false,
        ),
    ];
    let mut wrong = Vec::new();
    for (row, arguments, takes, refuses) in rows {
        let mut selected_and_off = arguments.clone();
        let at = selected_and_off
            .iter()
            .position(|argument| *argument == "--prompt")
            .unwrap();
        selected_and_off.splice(at..at, ["--sandbox", "off"]);
        let first = start_in_workspace(
            &[
                (RUN_BUDGET_ENV, OsStr::new("wallet976")),
                ("GATEWAY_KEY_976", OsStr::new("synthetic-key-976")),
            ],
            &selected_and_off,
        );
        let taken = first.error != SANDBOX_OFF;
        let first_ok = first.sent.is_empty()
            && first.code == Some(2)
            && (!taken || first.error == KEY_IS_CONTROL);
        let second = start_in_workspace(
            &[("GATEWAY_KEY_976", OsStr::new("synthetic-key-976"))],
            &arguments,
        );
        let refused = second.code == Some(2) && second.error == KEY_IS_CONTROL;
        let refused_ok = if refused {
            second.sent.is_empty()
        } else {
            !second.sent.is_empty()
        };
        if taken != takes || refused != refuses || (taken && !refused) || !first_ok || !refused_ok {
            wrong.push(format!(
                "{row}: taken {taken}, refused {refused}\n  first {first:?}\n  second {second:?}"
            ));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// macOS: the shell's model completion judges the name the start of its command line took. Its
/// model lists go to fixed local addresses, so this child's are sent through a proxy that is the
/// fake (`HTTP_PROXY`, which the client follows for every address): nothing reaches a real server
/// on this machine, and the fake counts what would have gone out. The control, with no selection
/// and a key of its own name, is counted there, so a count of nothing is not a proxy that was not
/// used.
#[cfg(target_os = "macos")]
#[test]
fn the_completion_sends_nothing_when_the_start_takes_a_control() {
    let completed = ["--", "commandagent", "--model", ""];
    let proxy = ("HTTP_PROXY", OsStr::new("<FAKE>"));
    let fish = ("COMPLETE", OsStr::new("fish"));
    let selection = (RUN_BUDGET_ENV, OsStr::new("wallet976"));
    for (label, named) in [
        (
            "the command line names the selection",
            &["--api-key-env", RUN_BUDGET_ENV][..],
        ),
        (
            "a preset names the selection and no role uses it",
            &["--preset", "control976"][..],
        ),
        (
            "the command line puts the preset's roles elsewhere",
            &["--preset", "compat976", "--provider", "ollama"][..],
        ),
    ] {
        let started = start_in_workspace(&[proxy, fish, selection], &joined(&[named, &completed]));
        assert_eq!(
            (started.code, started.sent.len()),
            (Some(0), 0),
            "{label}: {started:?}"
        );
    }
    let control = start_in_workspace(
        &[
            proxy,
            fish,
            ("GATEWAY_KEY_976", OsStr::new("synthetic-key-976")),
        ],
        &joined(&[&["--api-key-env", "GATEWAY_KEY_976"], &completed]),
    );
    assert!(
        control
            .sent
            .iter()
            .any(|request| request.contains("localhost:11434/api/tags")
                || request.contains("localhost:1234/v1/models")),
        "{control:?}"
    );
}

// ---- what the start took, against a configuration that changes after it (F1-R2) ----

#[cfg(target_os = "macos")]
const NAMES_THE_SELECTION: &str =
    "[preset.control976]\napi_key_env = \"COMMANDAGENT_RUN_BUDGET\"\n";
#[cfg(target_os = "macos")]
const NAMES_AN_ALIAS: &str = "[preset.control976]\napi_key_env = \"GATEWAY_KEY_976\"\n";
#[cfg(target_os = "macos")]
const NAMES_THE_ROOT: &str =
    "[preset.control976]\napi_key_env = \"COMMANDAGENT_ADAPTER_CONFIG_ROOT\"\n";

/// Reads 1 and 2 of the configuration are the start's: step 0 before its exec and step 1 after
/// it, in the same pid. Every later read, the run's configuration, the doctor's inspection, and
/// any name found again, gets the alias.
#[cfg(target_os = "macos")]
fn an_alias_after_the_start(read: usize) -> &'static str {
    if read <= 2 {
        NAMES_THE_SELECTION
    } else {
        NAMES_AN_ALIAS
    }
}

/// The start reads an alias; every later read, the operator root.
#[cfg(target_os = "macos")]
fn the_root_after_the_start(read: usize) -> &'static str {
    if read <= 2 {
        NAMES_AN_ALIAS
    } else {
        NAMES_THE_ROOT
    }
}

/// macOS: the start takes the selection out of the environment as the preset's key, and the
/// preset then names an alias for every later read (codex-3's F1-R2, placed by the read's number,
/// see [`ServedReads`]). The doctor, a run, and the shell's model completion are judged by what
/// the start took, and send nothing; read again, the configuration would have let each one send.
/// The controls, with no selection: the same configuration sends as before, and so does a key
/// of the alias's own name.
#[cfg(target_os = "macos")]
#[test]
fn a_configuration_changed_after_the_start_is_judged_by_what_the_start_took() {
    let selected = [(RUN_BUDGET_ENV, OsStr::new("wallet976"))];
    let served = Workspace::Serves(an_alias_after_the_start);
    let named = joined(&[
        &["--model", "m-1", "--preset", "control976"],
        &EVERY_ROLE_OLLAMA,
    ]);
    let doctor_arguments = joined(&[&named, &["--doctor", "--json"]]);
    let run_arguments = joined(&[&named, &["--prompt", "say hello"]]);

    let (doctor, reads) = launch_in(&selected, &doctor_arguments, served);
    assert_eq!((doctor.code, doctor.sent.len()), (Some(1), 0), "{doctor:?}");
    assert!(
        doctor.printed.contains(DOCTOR_REFUSED_BY_NAME),
        "{doctor:?}"
    );
    assert!(reads > 2, "no read after the start: {reads}");
    let (run, reads) = launch_in(&selected, &run_arguments, served);
    assert_eq!((run.code, run.sent.len()), (Some(2), 0), "{run:?}");
    assert_eq!(run.error, KEY_IS_CONTROL, "{run:?}");
    assert!(reads > 2, "no read after the start: {reads}");
    let completion = [
        ("HTTP_PROXY", OsStr::new("<FAKE>")),
        ("COMPLETE", OsStr::new("fish")),
        selected[0],
    ];
    let completing = [
        "--preset",
        "control976",
        "--",
        "commandagent",
        "--model",
        "",
    ];
    let (completed, reads) = launch_in(&completion, &completing, served);
    assert_eq!(
        (completed.code, completed.sent.len()),
        (Some(0), 0),
        "{completed:?}"
    );
    assert!(reads >= 2, "the start did not read: {reads}");

    for arguments in [&doctor_arguments, &run_arguments] {
        let (started, _) = launch_in(&[], arguments, served);
        assert!(!started.sent.is_empty(), "{arguments:?}: {started:?}");
    }
    let own = [("GATEWAY_KEY_976", OsStr::new("synthetic-key-976"))];
    let compatible = [
        "--model",
        "m-1",
        "--preset",
        "control976",
        "--provider",
        "openai-compatible",
        "--base-url",
        "<FAKE>/v1",
    ];
    for action in [&["--doctor", "--json"][..], &["--prompt", "say hello"][..]] {
        let arguments = joined(&[&compatible, action]);
        let (started, _) = launch_in(&own, &arguments, Workspace::Serves(|_| NAMES_AN_ALIAS));
        assert!(!started.sent.is_empty(), "{action:?}: {started:?}");
    }
}

/// macOS: the other way round, the start takes an alias and every later read names the operator
/// root for a role that uses it. What the start took passes; the configuration's name does not,
/// and is refused beside it, by the run and by the doctor, with nothing sent.
#[cfg(target_os = "macos")]
#[test]
fn the_configurations_name_is_judged_beside_what_the_start_took() {
    let own = [("GATEWAY_KEY_976", OsStr::new("synthetic-key-976"))];
    let compatible = [
        "--model",
        "m-1",
        "--preset",
        "control976",
        "--provider",
        "openai-compatible",
        "--base-url",
        "<FAKE>/v1",
    ];
    let served = Workspace::Serves(the_root_after_the_start);
    let (doctor, reads) = launch_in(
        &own,
        &joined(&[&compatible, &["--doctor", "--json"]]),
        served,
    );
    assert_eq!((doctor.code, doctor.sent.len()), (Some(1), 0), "{doctor:?}");
    assert!(
        doctor.printed.contains(DOCTOR_REFUSED_BY_NAME),
        "{doctor:?}"
    );
    assert!(reads > 2, "no read after the start: {reads}");
    let (run, reads) = launch_in(
        &own,
        &joined(&[&compatible, &["--prompt", "say hello"]]),
        served,
    );
    assert_eq!((run.code, run.sent.len()), (Some(2), 0), "{run:?}");
    assert_eq!(run.error, KEY_IS_CONTROL, "{run:?}");
    assert!(reads > 2, "no read after the start: {reads}");
}

/// macOS: the shell's model completion, row for row as the entry test judged it before the start's
/// own answer moved to the real binary (`entry_tests.rs`, 44f5f002, its `completed` rows), with no
/// selection and no key in the environment. A control's name, current or legacy, as the command
/// line's key or as the selected preset's, gives no candidate and sends nothing; another name, a
/// preset of another name, and no name send through the proxy (see the test above).
#[cfg(target_os = "macos")]
#[test]
fn the_completion_judges_each_name_as_the_entry_test_did() {
    const PRESETS_976: &str = "[preset.control976]\napi_key_env = \"COMMANDAGENT_RUN_BUDGET\"\n\n\
        [preset.own976]\napi_key_env = \"GATEWAY_KEY_976\"\n";
    let env = [
        ("HTTP_PROXY", OsStr::new("<FAKE>")),
        ("COMPLETE", OsStr::new("fish")),
    ];
    let completed = ["--", "commandagent", "--model", ""];
    let rows: [(&[&str], bool); 8] = [
        (&["--api-key-env", "COMMANDAGENT_RUN_BUDGET"], true),
        (&["--api-key-env", "ANVIL_RUN_BUDGET"], true),
        (&["--api-key-env", "COMMANDAGENT_ADAPTER_CONFIG_ROOT"], true),
        (&["--api-key-env", "ANVIL_ADAPTER_CONFIG_ROOT"], true),
        (&["--preset", "control976"], true),
        (&["--api-key-env", "GATEWAY_KEY_976"], false),
        (&["--preset", "own976"], false),
        (&[], false),
    ];
    let mut wrong = Vec::new();
    for (named, named_as_a_control) in rows {
        let started = launch(&env, &joined(&[named, &completed]), Some(PRESETS_976));
        // A send stops the child where the fake sees it (see `launch`), so whether it went on to
        // print the candidate is not this test's to see; a refusal ends on its own, with none.
        let candidate = started.printed.lines().any(|line| line.starts_with("m-1"));
        let refused = started.code == Some(0) && started.sent.is_empty() && !candidate;
        let as_before = !started.sent.is_empty();
        if refused != named_as_a_control || as_before == named_as_a_control {
            wrong.push(format!("{named:?}: {started:?}"));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}
