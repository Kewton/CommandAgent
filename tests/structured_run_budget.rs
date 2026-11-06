//! Issue #976, stage 3b: the GUI under a run budget. The GUI server is a binary of its own, so it
//! is run as one, with an empty environment and a fake HOME. Its model discovery and its delegated
//! runs are sends no wallet counts, and under a selection neither goes out; with none, both go out
//! as before. Every send goes to a fake on 127.0.0.1, which counts what reaches it. The GUI server
//! is built only with the `gui` feature, so these run with it, as `tests/gui_server.rs` does.

#![cfg(all(unix, feature = "gui"))]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

const RUN_BUDGET_ENV: &str = "COMMANDAGENT_RUN_BUDGET";
const TOKEN: &str = "commandagent-gui-test-token-000000000976";
const REFUSED: &str = "the run budget refused the entry: Unmetered(GuiSend)";

/// A fake on 127.0.0.1 that answers every request with a model list and counts them.
struct Fake {
    url: String,
    stop: Arc<AtomicBool>,
    thread: JoinHandle<()>,
    seen: Arc<AtomicUsize>,
}

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

struct Server {
    child: Child,
    port: u16,
}

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

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Model discovery: under a selection the list is empty and nothing reaches the fake; with none,
/// the fake answers and its model is listed.
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
