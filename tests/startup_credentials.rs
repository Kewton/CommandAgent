//! The start of the real binary: where a credential is after it (Issue #974).
//!
//! Every secret here is a dummy, every provider is a disposable fake on 127.0.0.1, and every
//! child starts from a cleared environment.

use std::io::{Read as _, Write as _};
use std::net::TcpListener;
use std::path::Path;
use std::process::{Child, Command, Output, Stdio};
use std::sync::mpsc;
use std::time::Duration;

const DUMMY: &str = "fake-startup-credential-974-not-a-real-key";

fn command(workspace: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_commandagent"));
    command
        .current_dir(workspace)
        .env_clear()
        .env("HOME", workspace)
        .env("PATH", "/usr/bin:/bin")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}

/// A fake provider. It records what each request's first lines say, tells the test about the
/// first request, and answers every request with an error. Nothing it hears is printed.
struct FakeProvider {
    url: String,
    first_request: mpsc::Receiver<String>,
    release: mpsc::Sender<()>,
}

impl FakeProvider {
    fn start() -> Self {
        Self::listening(false)
    }

    /// One that keeps its first request unanswered until [`Self::release`], or until it is
    /// dropped, so the process that asked is there for as long as the test looks at it.
    fn holding() -> Self {
        Self::listening(true)
    }

    fn listening(holds: bool) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let (sender, first_request) = mpsc::channel();
        let (release, released) = mpsc::channel();
        std::thread::spawn(move || {
            let mut holds = holds;
            for stream in listener.incoming().flatten() {
                let mut stream = stream;
                let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                let mut heard = [0u8; 8192];
                let read = stream.read(&mut heard).unwrap_or(0);
                let _ = sender.send(String::from_utf8_lossy(&heard[..read]).into_owned());
                if std::mem::take(&mut holds) {
                    let _ = released.recv();
                }
                let _ = stream.write_all(
                    b"HTTP/1.1 500 Internal Server Error\r\ncontent-length: 0\r\nconnection: close\r\n\r\n",
                );
            }
        });
        Self {
            url,
            first_request,
            release,
        }
    }

    /// The test has finished looking: the first request gets its answer.
    fn release(&self) {
        self.release.send(()).expect("the fake is still listening");
    }

    /// The first request, which is also the proof that the start finished, or a failure.
    fn wait_for_request(&self) -> String {
        self.first_request
            .recv_timeout(Duration::from_secs(60))
            .expect("the provider heard nothing")
    }

    /// Every request that arrives within a moment.
    fn drain(&self) -> Vec<String> {
        let mut heard = Vec::new();
        while let Ok(request) = self.first_request.recv_timeout(Duration::from_millis(200)) {
            heard.push(request);
        }
        heard
    }
}

fn stop(mut child: Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn lm_studio_arguments(url: &str) -> Vec<String> {
    [
        "--provider",
        "lm-studio",
        "--lm-studio-host",
        url,
        "--model",
        "m",
        "--chat-retries",
        "0",
        "--prompt",
        "h\u{e9}llo",
    ]
    .map(str::to_string)
    .to_vec()
}

#[cfg(target_os = "macos")]
mod procargs {
    /// One process's exec-time area, by its own structure. A pid that is gone, a refusal, or a
    /// shape that does not parse is a failure, and never "the dummy is absent".
    pub(super) struct Procargs {
        pub(super) raw: Vec<u8>,
        pub(super) argv: Vec<Vec<u8>>,
        pub(super) env: Vec<Vec<u8>>,
    }

    fn take_text<'a>(rest: &mut &'a [u8]) -> &'a [u8] {
        let end = rest
            .iter()
            .position(|byte| *byte == 0)
            .expect("a NUL ends each string");
        let (text, tail) = rest.split_at(end);
        *rest = &tail[1..];
        text
    }

    pub(super) fn read(pid: u32) -> Procargs {
        let mut mib = [libc::CTL_KERN, libc::KERN_PROCARGS2, pid as i32];
        let mut limit = 0i32;
        let mut limit_size = size_of::<i32>();
        let mut limit_mib = [libc::CTL_KERN, libc::KERN_ARGMAX];
        // SAFETY: `limit_mib` names two levels, and `limit` is writable for its own size.
        let limit_answer = unsafe {
            libc::sysctl(
                limit_mib.as_mut_ptr(),
                2,
                (&raw mut limit).cast(),
                &raw mut limit_size,
                std::ptr::null_mut(),
                0,
            )
        };
        assert_eq!(limit_answer, 0, "the area's limit was not read");
        let mut raw = vec![0u8; usize::try_from(limit).unwrap()];
        let mut size = raw.len();
        // SAFETY: `mib` names three levels, and `raw` is writable for `size` bytes.
        let answer = unsafe {
            libc::sysctl(
                mib.as_mut_ptr(),
                3,
                raw.as_mut_ptr().cast(),
                &raw mut size,
                std::ptr::null_mut(),
                0,
            )
        };
        assert_eq!(
            answer,
            0,
            "the exec-time area of {pid} was not read: {}",
            std::io::Error::last_os_error()
        );
        raw.truncate(size);
        let (count, mut rest) = raw.split_at(4);
        let argc = i32::from_ne_bytes(count.try_into().unwrap());
        assert!(argc > 0, "argc is {argc}");
        let _executable = take_text(&mut rest);
        let padding = rest.iter().position(|byte| *byte != 0).unwrap();
        rest = &rest[padding..];
        let argv = (0..argc)
            .map(|_| take_text(&mut rest).to_vec())
            .collect::<Vec<_>>();
        let mut env = Vec::new();
        while !rest.is_empty() && rest[0] != 0 {
            env.push(take_text(&mut rest).to_vec());
        }
        Procargs {
            argv,
            env,
            raw: raw.clone(),
        }
    }

    pub(super) fn holds(bytes: &[u8], needle: &str) -> bool {
        bytes
            .windows(needle.len())
            .any(|window| window == needle.as_bytes())
    }
}

/// Stage 1 is done when the provider hears from this same pid: the area is then what stays.
#[cfg(target_os = "macos")]
#[test]
fn a_credential_is_not_in_the_exec_time_area_after_the_start() {
    let workspace = tempfile::tempdir().unwrap();
    let provider = FakeProvider::holding();
    let arguments = lm_studio_arguments(&provider.url);
    let child = command(workspace.path())
        .args(&arguments)
        .env("OPENAI_API_KEY", DUMMY)
        .env("KEPT_VARIABLE", "kept-value")
        .spawn()
        .unwrap();
    let pid = child.id();
    provider.wait_for_request();
    let area = procargs::read(pid);
    stop(child);
    assert!(!procargs::holds(&area.raw, DUMMY));
    assert!(!procargs::holds(&area.raw, "OPENAI_API_KEY"));
    assert_eq!(area.argv.len(), arguments.len() + 1);
    assert_eq!(
        area.argv[1..],
        arguments
            .iter()
            .map(|argument| argument.as_bytes().to_vec())
            .collect::<Vec<_>>()
    );
    assert!(area.env.contains(&b"KEPT_VARIABLE=kept-value".to_vec()));
    assert!(
        area.env
            .iter()
            .any(|row| row.starts_with(b"COMMANDAGENT_BOOT_FD="))
    );
}

/// Stays until stdin closes. This binary is not a platform binary, so it is what the area of an
/// ordinary program looks like with the dummy in its exec-time environment.
#[test]
fn stays_until_stdin_closes() {
    if std::env::var_os("G3_STAY_UNTIL_STDIN_CLOSES").is_some() {
        println!("ready");
        std::io::stdout().flush().unwrap();
        let _ = std::io::stdin().read(&mut [0u8; 1]);
    }
}

/// The witness for the test above: the same read, on a program that has the dummy, sees it.
#[cfg(target_os = "macos")]
#[test]
fn the_same_read_sees_the_dummy_in_an_ordinary_programs_area() {
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "stays_until_stdin_closes", "--nocapture"])
        .env_clear()
        .env("G3_STAY_UNTIL_STDIN_CLOSES", "1")
        .env("OPENAI_API_KEY", DUMMY)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let mut heard = Vec::new();
    let mut byte = [0u8; 1];
    while !procargs::holds(&heard, "ready") {
        assert_eq!(stdout.read(&mut byte).unwrap(), 1, "the helper ended");
        heard.push(byte[0]);
    }
    let area = procargs::read(child.id());
    drop(child.stdin.take());
    stop(child);
    assert!(procargs::holds(&area.raw, DUMMY));
    assert!(
        area.env
            .iter()
            .any(|row| row.starts_with(b"OPENAI_API_KEY="))
    );
}

fn authorization_of(heard: &[String]) -> Vec<String> {
    heard
        .iter()
        .flat_map(|request| request.lines())
        .filter_map(|line| line.split_once(':'))
        .filter(|(name, _)| name.eq_ignore_ascii_case("authorization"))
        .map(|(_, value)| value.trim().to_string())
        .collect()
}

/// Run to its end, and what a holding fake heard. The run is alive for the read because the fake
/// answers only after it, and only that answer lets the run end.
fn heard_by_a_run(
    workspace: &Path,
    provider: &FakeProvider,
    build: impl Fn(&mut Command),
) -> Vec<String> {
    let mut command = command(workspace);
    build(&mut command);
    let child = command.spawn().unwrap();
    let pid = child.id();
    let first = provider.wait_for_request();
    #[cfg(target_os = "macos")]
    {
        let area = procargs::read(pid);
        assert!(!procargs::holds(&area.raw, DUMMY), "in the exec-time area");
    }
    let _ = pid;
    provider.release();
    let output = child.wait_with_output().unwrap();
    let shown = [output.stdout, output.stderr].concat();
    assert!(
        !String::from_utf8_lossy(&shown).contains(DUMMY),
        "the output shows the dummy"
    );
    let mut heard = vec![first];
    heard.extend(provider.drain());
    heard
}

/// A received credential authenticates: the exact value reaches the fake endpoint.
#[test]
fn the_lm_studio_token_reaches_the_endpoint() {
    let workspace = tempfile::tempdir().unwrap();
    let provider = FakeProvider::holding();
    let heard = heard_by_a_run(workspace.path(), &provider, |command| {
        command
            .args(lm_studio_arguments(&provider.url))
            .env("LM_STUDIO_API_TOKEN", DUMMY);
    });
    let expected = format!("Bearer {DUMMY}");
    assert!(authorization_of(&heard).contains(&expected));
}

fn compatible_arguments(url: &str) -> Vec<String> {
    [
        "--provider",
        "openai-compatible",
        "--base-url",
        url,
        "--model",
        "m",
        "--chat-retries",
        "0",
        "--prompt",
        "hello",
    ]
    .map(str::to_string)
    .to_vec()
}

/// Any name the command line chooses, including one the credential predicate does not match.
#[test]
fn a_key_name_from_the_command_line_reaches_the_endpoint() {
    let workspace = tempfile::tempdir().unwrap();
    let provider = FakeProvider::holding();
    let heard = heard_by_a_run(workspace.path(), &provider, |command| {
        command
            .args(compatible_arguments(&provider.url))
            .args(["--api-key-env", "AUTH"])
            .env("AUTH", DUMMY);
    });
    let expected = format!("Bearer {DUMMY}");
    assert!(authorization_of(&heard).contains(&expected));
}

/// Any name the selected preset chooses, with a reference to a received value in the same
/// preset, and an unselected preset whose reference to an unset variable stops nothing.
#[test]
fn a_key_name_from_the_selected_preset_reaches_the_endpoint() {
    let workspace = tempfile::tempdir().unwrap();
    let provider = FakeProvider::holding();
    let config = workspace.path().join(".commandagent");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(
        config.join("config.toml"),
        format!(
            "[preset.selected]\nprovider = \"openai-compatible\"\nbase_url = \"{}\"\napi_key_env = \"AUTH\"\nmodel = \"m\"\n\n[preset.other]\nmodel = \"${{NOT_SET_ANYWHERE}}\"\n",
            provider.url
        ),
    )
    .unwrap();
    let heard = heard_by_a_run(workspace.path(), &provider, |command| {
        command
            .args([
                "--preset",
                "selected",
                "--chat-retries",
                "0",
                "--prompt",
                "hello",
            ])
            .env("AUTH", DUMMY)
            .env("OPENAI_API_KEY", format!("{DUMMY}-second"));
    });
    let expected = format!("Bearer {DUMMY}");
    assert!(authorization_of(&heard).contains(&expected));
}

fn run_to_end(workspace: &Path, build: impl Fn(&mut Command)) -> Output {
    let mut command = command(workspace);
    build(&mut command);
    command.output().unwrap()
}

/// PM condition 1. With and without a credential, what a caller can see is byte for byte the
/// same: stdout, stderr, and the exit, on each path that ends without a provider.
#[test]
fn a_run_shows_the_same_bytes_with_and_without_a_credential_in_the_environment() {
    use std::os::unix::ffi::OsStrExt as _;
    let workspace = tempfile::tempdir().unwrap();
    let not_text = std::ffi::OsStr::from_bytes(b"not-\xff-text");
    let cases: Vec<(&str, Vec<&std::ffi::OsStr>)> = vec![
        ("version", vec!["--version".as_ref()]),
        ("help", vec!["--help".as_ref()]),
        ("an unknown flag", vec!["--no-such-flag".as_ref()]),
        ("no terminal", Vec::new()),
        ("not text", vec!["--cwd".as_ref(), not_text]),
        (
            "a completion",
            vec!["--".as_ref(), "commandagent".as_ref(), "--ver".as_ref()],
        ),
    ];
    for (label, arguments) in cases {
        let plain = |command: &mut Command| {
            command.args(&arguments).env("KEPT_VARIABLE", "kept-value");
            if label == "a completion" {
                command.env("COMPLETE", "bash");
            }
        };
        let without = run_to_end(workspace.path(), plain);
        let with = run_to_end(workspace.path(), |command| {
            plain(command);
            command.env("OPENAI_API_KEY", DUMMY);
        });
        assert_eq!(with.status.code(), without.status.code(), "{label}");
        assert_eq!(with.stdout, without.stdout, "{label}");
        assert_eq!(with.stderr, without.stderr, "{label}");
    }
}

/// With no credential there is no second exec at all: the area is exactly what was given.
#[cfg(target_os = "macos")]
#[test]
fn with_no_credential_the_start_does_not_exec_again() {
    let workspace = tempfile::tempdir().unwrap();
    let provider = FakeProvider::holding();
    let child = command(workspace.path())
        .args(lm_studio_arguments(&provider.url))
        .env("KEPT_VARIABLE", "kept-value")
        .spawn()
        .unwrap();
    let pid = child.id();
    provider.wait_for_request();
    let area = procargs::read(pid);
    stop(child);
    assert!(
        !area
            .env
            .iter()
            .any(|row| row.starts_with(b"COMMANDAGENT_BOOT_FD"))
    );
    assert!(area.env.contains(&b"KEPT_VARIABLE=kept-value".to_vec()));
}

/// Elsewhere the start does nothing: the variable stays and the marker means nothing.
#[cfg(target_os = "linux")]
#[test]
fn off_macos_the_start_changes_nothing() {
    let workspace = tempfile::tempdir().unwrap();
    let provider = FakeProvider::start();
    let child = command(workspace.path())
        .args(lm_studio_arguments(&provider.url))
        .env("OPENAI_API_KEY", DUMMY)
        .env("COMMANDAGENT_BOOT_FD", "not-a-number")
        .spawn()
        .unwrap();
    let environ = format!("/proc/{}/environ", child.id());
    provider.wait_for_request();
    let area = std::fs::read(environ).unwrap();
    stop(child);
    assert!(
        area.windows(DUMMY.len())
            .any(|window| window == DUMMY.as_bytes())
    );
}

#[cfg(target_os = "macos")]
mod refused_starts {
    use std::collections::BTreeMap;
    use std::os::fd::AsRawFd as _;
    use std::os::unix::process::CommandExt as _;
    use std::time::Instant;

    use super::*;

    const MARKER: &str = "COMMANDAGENT_BOOT_FD";
    /// A number the handoff reaches the child at, free in the child.
    const AT: i32 = 40;
    /// The most a handoff may hold.
    const BOUND: usize = 16 * 1024;
    const NOT_A_PIPE: &str = "the handoff descriptor is not a pipe";
    const THE_WRITER_STAYED: &str = "the handoff did not end";

    fn frame(rows: &[(&str, &str)]) -> Vec<u8> {
        let mut bytes = b"CACRED1\n".to_vec();
        bytes.extend_from_slice(&(rows.len() as u32).to_be_bytes());
        for (name, value) in rows {
            for part in [name, value] {
                bytes.extend_from_slice(&(part.len() as u32).to_be_bytes());
                bytes.extend_from_slice(part.as_bytes());
            }
        }
        bytes
    }

    /// What the handoff's writer is, once the child has written to it.
    #[derive(Clone, Copy, PartialEq)]
    enum Writer {
        /// Everything was written and it is closed.
        Closed,
        /// Everything was written, and the child keeps it across the exec.
        LeftOpen,
        /// It took more than the bound, as much as the pipe would, and it is closed.
        Filled,
    }

    /// A new pipe at [`AT`] holding these bytes. All of it is made in the child, after the fork:
    /// the child has one thread and forks nothing before its exec, so no other process can ever
    /// hold the writer, whatever this test binary's other threads are starting. A pipe made in
    /// this process can reach a sibling that forks while it is open, close-on-exec or not.
    fn hand_pipe_at(command: &mut Command, bytes: Vec<u8>, kept: Writer) {
        let at_least = if kept == Writer::Filled {
            BOUND + 1
        } else {
            bytes.len()
        };
        // SAFETY: only `pipe`, `fcntl`, `write`, `close` and `dup2` run in the child, which are
        // async-signal-safe, and the bytes were made before the fork. Nothing here allocates.
        unsafe {
            command.pre_exec(move || {
                let mut ends = [-1i32; 2];
                if libc::pipe(ends.as_mut_ptr()) != 0
                    || libc::fcntl(ends[1], libc::F_SETFL, libc::O_NONBLOCK) != 0
                {
                    return Err(std::io::Error::last_os_error());
                }
                let [reader, writer] = ends;
                let mut written = 0;
                while written < bytes.len() {
                    let pending = &bytes[written..];
                    let answer = libc::write(writer, pending.as_ptr().cast(), pending.len());
                    match usize::try_from(answer) {
                        Ok(count) if count > 0 => written += count,
                        _ => break,
                    }
                }
                if written < at_least {
                    return Err(std::io::Error::from_raw_os_error(libc::EMSGSIZE));
                }
                if kept != Writer::LeftOpen {
                    libc::close(writer);
                } else if writer == AT && libc::fcntl(writer, libc::F_DUPFD, AT + 1) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                if reader != AT {
                    if libc::dup2(reader, AT) < 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    libc::close(reader);
                }
                inherit_at()
            });
        }
    }

    /// `dup2` onto the same number changes nothing, so the flag is cleared here on every path.
    ///
    /// # Safety
    /// For the new child: `fcntl` is async-signal-safe.
    unsafe fn inherit_at() -> std::io::Result<()> {
        // SAFETY: this sets one flag, and takes no pointer.
        if unsafe { libc::fcntl(AT, libc::F_SETFD, 0) } != 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(())
    }

    /// This process's own descriptor, at [`AT`] in the child alone.
    fn hand_at(command: &mut Command, source: &std::fs::File) {
        hand_number_at(command, source.as_raw_fd());
    }

    /// A file from `reopen` is close-on-exec from its open, so no sibling keeps it past its exec,
    /// and nothing waits for this one to close.
    fn hand_number_at(command: &mut Command, source: i32) {
        // SAFETY: `dup2` and `fcntl` are async-signal-safe, and [`AT`] is replaced in the child.
        unsafe {
            command.pre_exec(move || {
                if libc::dup2(source, AT) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                inherit_at()
            });
        }
    }

    /// A refusal for this reason: not zero, soon, in a fixed sentence with no secret, and no
    /// provider call. The reason matters: a handoff that never reached the child is refused too.
    fn assert_refused(label: &str, reason: &str, build: impl Fn(&mut Command)) {
        let workspace = tempfile::tempdir().unwrap();
        let provider = FakeProvider::start();
        let mut command = super::command(workspace.path());
        command.args(lm_studio_arguments(&provider.url));
        build(&mut command);
        let mut child = command.spawn().unwrap();
        let started = Instant::now();
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            assert!(
                started.elapsed() < Duration::from_secs(20),
                "{label}: it did not end"
            );
            std::thread::sleep(Duration::from_millis(5));
        };
        let output = child.wait_with_output().unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!status.success(), "{label}: {stderr}");
        assert_eq!(
            stderr,
            format!("error: the startup credential handoff failed: {reason}\n"),
            "{label}"
        );
        assert!(!stderr.contains(DUMMY), "{label}: {stderr}");
        assert!(output.stdout.is_empty(), "{label}");
        assert!(
            provider.drain().is_empty(),
            "{label}: a provider was called"
        );
    }

    #[test]
    fn a_marker_that_names_no_usable_pipe_stops_the_start() {
        for marker in ["", "x", "-1", "03", "0", "1", "2", "99999999999"] {
            assert_refused(marker, "the handoff descriptor is not valid", |command| {
                command.env(MARKER, marker);
            });
        }
        assert_refused("77", "the handoff descriptor is not open", |command| {
            command.env(MARKER, "77");
        });
    }

    #[test]
    fn a_descriptor_that_is_not_a_pipe_stops_the_start() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let file = file.reopen().unwrap();
        assert_refused("a file", NOT_A_PIPE, |command| {
            hand_at(command, &file);
            command.env(MARKER, AT.to_string());
        });
    }

    /// The right bytes on the wrong kind of descriptor: only the kind can refuse this one.
    #[test]
    fn a_file_holding_a_good_handoff_stops_the_start() {
        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(file.path(), frame(&[("OPENAI_API_KEY", DUMMY)])).unwrap();
        let file = file.reopen().unwrap();
        assert_refused("a file holding a good handoff", NOT_A_PIPE, |command| {
            hand_at(command, &file);
            command.env(MARKER, AT.to_string());
        });
    }

    #[test]
    fn a_handoff_that_is_not_well_formed_stops_the_start() {
        let good = frame(&[("OPENAI_API_KEY", DUMMY)]);
        let mut trailing = good.clone();
        trailing.push(0);
        for (label, bytes) in [
            ("empty", Vec::new()),
            ("words", b"not a frame".to_vec()),
            ("cut", good[..good.len() - 1].to_vec()),
            ("trailing", trailing),
        ] {
            assert_refused(label, "the handoff is not well formed", |command| {
                hand_pipe_at(command, bytes.clone(), Writer::Closed);
                command.env(MARKER, AT.to_string());
            });
        }
    }

    /// More than the bound, however much the pipe took: what it took already exceeds it.
    #[test]
    fn a_handoff_over_the_bound_stops_the_start() {
        assert_refused(
            "over the bound",
            "the handoff exceeds the bound",
            |command| {
                hand_pipe_at(command, vec![b'x'; 64 * 1024], Writer::Filled);
                command.env(MARKER, AT.to_string());
            },
        );
    }

    /// A writer that never closes: the start ends on it instead of waiting for it. The child
    /// itself holds it, so it stays open for exactly as long as the start could wait.
    #[test]
    fn a_handoff_whose_writer_stays_open_stops_the_start_without_waiting() {
        for bytes in [Vec::new(), frame(&[("OPENAI_API_KEY", DUMMY)])] {
            assert_refused("an open writer", THE_WRITER_STAYED, |command| {
                hand_pipe_at(command, bytes.clone(), Writer::LeftOpen);
                command.env(MARKER, AT.to_string());
            });
        }
    }

    /// The marker is not proof of a clean environment.
    #[test]
    fn a_marker_beside_a_credential_in_the_environment_stops_the_start() {
        for bytes in [
            frame(&[]),
            frame(&[("OPENAI_API_KEY", "another-dummy-value")]),
        ] {
            assert_refused(
                "a credential beside the marker",
                "a credential is still in the environment",
                |command| {
                    hand_pipe_at(command, bytes.clone(), Writer::Closed);
                    command
                        .env(MARKER, AT.to_string())
                        .env("OPENAI_API_KEY", DUMMY);
                },
            );
        }
    }

    #[test]
    fn a_handoff_holding_a_name_this_run_does_not_use_stops_the_start() {
        assert_refused(
            "another name",
            "the handoff holds a name that is not a credential of this run",
            |command| {
                hand_pipe_at(command, frame(&[("PATH", DUMMY)]), Writer::Closed);
                command.env(MARKER, AT.to_string());
            },
        );
    }

    /// Too much for the handoff is a refusal, and the credential did not stay in the environment
    /// because the process ended with it there and started nothing.
    #[test]
    fn a_credential_over_the_bound_stops_the_start() {
        assert_refused(
            "a large value",
            "the credentials exceed the bound",
            |command| {
                command.env("OPENAI_API_KEY", format!("{DUMMY}{}", "x".repeat(20_000)));
            },
        );
    }

    /// The witness for all of these: a good handoff, by hand, starts and reaches the provider.
    #[test]
    fn a_good_handoff_by_hand_starts() {
        let workspace = tempfile::tempdir().unwrap();
        let provider = FakeProvider::start();
        let mut command = super::command(workspace.path());
        command
            .args(lm_studio_arguments(&provider.url))
            .env(MARKER, AT.to_string());
        hand_pipe_at(
            &mut command,
            frame(&[("LM_STUDIO_API_TOKEN", DUMMY)]),
            Writer::Closed,
        );
        let child = command.spawn().unwrap();
        let first = provider.wait_for_request();
        stop(child);
        let expected = format!("Bearer {DUMMY}");
        assert!(authorization_of(&[first]).contains(&expected));
    }

    /// The numbers below [`AT`] and just above it, filled or freed in the child so that its next
    /// `pipe()` lands where a test wants it. The filling closes at the exec.
    fn free_only(command: &mut Command, free: [i32; 2]) {
        // SAFETY: `close`, `dup2` and `fcntl` are async-signal-safe, and this is the new child.
        unsafe {
            command.pre_exec(move || {
                for number in 3..=free[1] {
                    if free.contains(&number) {
                        libc::close(number);
                    } else if libc::dup2(0, number) < 0
                        || libc::fcntl(number, libc::F_SETFD, libc::FD_CLOEXEC) != 0
                    {
                        return Err(std::io::Error::last_os_error());
                    }
                }
                Ok(())
            });
        }
    }

    /// The first request of a start handed its pipe by `build`, or a failure.
    fn assert_starts(build: impl Fn(&mut Command)) {
        let workspace = tempfile::tempdir().unwrap();
        let provider = FakeProvider::start();
        let mut command = super::command(workspace.path());
        command
            .args(lm_studio_arguments(&provider.url))
            .env(MARKER, AT.to_string());
        build(&mut command);
        let child = command.spawn().unwrap();
        let first = provider.wait_for_request();
        stop(child);
        let expected = format!("Bearer {DUMMY}");
        assert!(authorization_of(&[first]).contains(&expected));
    }

    /// `pipe()` may give either end the number the handoff goes to.
    #[test]
    fn a_pipe_end_already_at_the_number_still_hands_off() {
        let good = || frame(&[("LM_STUDIO_API_TOKEN", DUMMY)]);
        assert_starts(|command| {
            free_only(command, [AT, AT + 1]);
            hand_pipe_at(command, good(), Writer::Closed);
        });
        assert_starts(|command| {
            free_only(command, [AT - 1, AT]);
            hand_pipe_at(command, good(), Writer::Closed);
        });
        assert_refused("a writer at the number", THE_WRITER_STAYED, |command| {
            free_only(command, [AT - 1, AT]);
            hand_pipe_at(command, good(), Writer::LeftOpen);
            command.env(MARKER, AT.to_string());
        });
    }

    /// A file that already has the number is still there after the exec, and is refused for what
    /// it is. Refused as not open, it would say nothing about its kind.
    #[test]
    fn a_source_already_at_the_number_reaches_the_child() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let file = file.reopen().unwrap();
        let source = file.as_raw_fd();
        assert_refused("already at the number", NOT_A_PIPE, |command| {
            // SAFETY: `dup2` and `fcntl` are async-signal-safe, and this is the new child.
            unsafe {
                command.pre_exec(move || {
                    if libc::dup2(source, AT) < 0
                        || libc::fcntl(AT, libc::F_SETFD, libc::FD_CLOEXEC) != 0
                    {
                        return Err(std::io::Error::last_os_error());
                    }
                    Ok(())
                });
            }
            hand_number_at(command, AT);
            command.env(MARKER, AT.to_string());
        });
    }

    /// Each numbered descriptor of `lsof -Fft`'s answer and its type, or why the answer says
    /// nothing. Named ones such as `cwd` have the same shape and are not numbers.
    fn read_listing(
        pid: u32,
        succeeded: bool,
        listing: &str,
    ) -> Result<BTreeMap<i32, String>, String> {
        if !succeeded {
            return Err("the listing failed".to_string());
        }
        let mut lines = listing.lines();
        if lines.next() != Some(format!("p{pid}").as_str()) {
            return Err("the listing is not of this process".to_string());
        }
        let mut types = BTreeMap::new();
        while let Some(line) = lines.next() {
            let name = line
                .strip_prefix('f')
                .ok_or_else(|| format!("not a descriptor: {line}"))?;
            let kind = lines
                .next()
                .and_then(|line| line.strip_prefix('t'))
                .ok_or_else(|| format!("no type for {name}"))?;
            if let Ok(number) = name.parse::<i32>()
                && types.insert(number, kind.to_string()).is_some()
            {
                return Err(format!("{number} is listed twice"));
            }
        }
        if types.is_empty() {
            return Err("no descriptor is listed".to_string());
        }
        Ok(types)
    }

    /// One look at every descriptor of a live process. Not being able to look is a failure, and
    /// never "that number is not open".
    fn descriptors_of(pid: u32) -> BTreeMap<i32, String> {
        let listing = Command::new("/usr/sbin/lsof")
            .args(["-nP", "-a", "-p", &pid.to_string(), "-Fft"])
            .env_clear()
            .env("PATH", "/usr/bin:/bin:/usr/sbin")
            .output()
            .unwrap();
        read_listing(
            pid,
            listing.status.success(),
            &String::from_utf8_lossy(&listing.stdout),
        )
        .unwrap_or_else(|reason| panic!("the descriptors of {pid} were not read: {reason}"))
    }

    /// Whether the handoff's reader is still at its number. A reader left open keeps the number,
    /// so a later open can only have it once the reader is gone: another kind there means gone.
    /// A later pipe there reads as the reader, which fails the test and hides nothing.
    fn holds_the_handoff(descriptors: &BTreeMap<i32, String>) -> bool {
        descriptors.get(&AT).is_some_and(|kind| kind == "PIPE")
    }

    #[test]
    fn a_listing_is_read_only_when_it_says_what_it_should() {
        let at = format!("f{AT}");
        let read = |lines: &[&str]| read_listing(7, true, &lines.join("\n"));
        for bad in [
            &[][..],
            &["p8", "f1", "tPIPE"],
            &["p7"],
            &["p7", "f1"],
            &["p7", "tPIPE"],
            &["p7", "f1", "nsomething"],
            &["p7", "f1", "tPIPE", "f1", "tPIPE"],
        ] {
            assert!(read(bad).is_err(), "{bad:?}");
        }
        assert!(read_listing(7, false, "p7\nf1\ntPIPE").is_err());

        let closed = read(&["p7", "fcwd", "tDIR", "f1", "tPIPE"]).unwrap();
        assert_eq!(closed[&1], "PIPE");
        assert!(!holds_the_handoff(&closed));
        let reused = read(&["p7", &at, "tREG"]).unwrap();
        assert!(!holds_the_handoff(&reused));
        let open = read(&["p7", &at, "tPIPE"]).unwrap();
        assert!(holds_the_handoff(&open));
    }

    /// Once the start is done, the handoff's reader is gone from this real process. Stdout is a
    /// pipe here too, and it is in the same listing: this look can see a pipe.
    #[test]
    fn the_handoff_descriptor_is_closed_after_the_start() {
        let workspace = tempfile::tempdir().unwrap();
        let provider = FakeProvider::holding();
        let mut command = super::command(workspace.path());
        command
            .args(lm_studio_arguments(&provider.url))
            .env(MARKER, AT.to_string());
        hand_pipe_at(
            &mut command,
            frame(&[("LM_STUDIO_API_TOKEN", DUMMY)]),
            Writer::Closed,
        );
        let child = command.spawn().unwrap();
        let pid = child.id();
        provider.wait_for_request();
        let descriptors = descriptors_of(pid);
        stop(child);
        assert_eq!(descriptors[&1], "PIPE");
        assert!(!holds_the_handoff(&descriptors));
    }

    /// A caller that closed stdin gave the pipe a low number. The start moves it up.
    #[test]
    fn a_closed_standard_stream_does_not_stop_the_start() {
        let workspace = tempfile::tempdir().unwrap();
        let provider = FakeProvider::start();
        let mut command = super::command(workspace.path());
        command
            .args(lm_studio_arguments(&provider.url))
            .env("OPENAI_API_KEY", DUMMY);
        // SAFETY: `close` is async-signal-safe, and this runs in the new child.
        unsafe {
            command.pre_exec(|| {
                libc::close(0);
                Ok(())
            });
        }
        let child = command.spawn().unwrap();
        provider.wait_for_request();
        stop(child);
    }

    /// A credential large enough to need several writes still fits under the bound.
    #[test]
    fn a_large_credential_within_the_bound_starts() {
        let workspace = tempfile::tempdir().unwrap();
        let provider = FakeProvider::start();
        let value = format!("{DUMMY}{}", "x".repeat(8 * 1024));
        let child = super::command(workspace.path())
            .args(lm_studio_arguments(&provider.url))
            .env("LM_STUDIO_API_TOKEN", &value)
            .spawn()
            .unwrap();
        provider.wait_for_request();
        stop(child);
    }
}

/// The real server delegating to the real CLI, with a dummy key and a fake on 127.0.0.1.
#[cfg(feature = "gui")]
mod gui_delegation {
    use std::io::BufRead as _;
    use std::net::TcpStream;
    use std::time::Instant;

    use super::*;

    const ACCESS: &str = "startup-credential-gui-test-token-0000001";

    /// A provider that answers a model list, and holds the first request that carries a key
    /// without answering it, so the process that sent it stays as it is while it is read.
    struct HoldingProvider {
        url: String,
        carrying_a_key: mpsc::Receiver<(u16, String, TcpStream)>,
    }

    impl HoldingProvider {
        fn start() -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let url = format!("http://{}", listener.local_addr().unwrap());
            let (sender, carrying_a_key) = mpsc::channel();
            std::thread::spawn(move || {
                for mut stream in listener.incoming().flatten() {
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                    let mut heard = [0u8; 8192];
                    let read = stream.read(&mut heard).unwrap_or(0);
                    let request = String::from_utf8_lossy(&heard[..read]).into_owned();
                    let peer = stream.peer_addr().map_or(0, |address| address.port());
                    if !authorization_of(std::slice::from_ref(&request)).is_empty() {
                        let _ = sender.send((peer, request, stream));
                        continue;
                    }
                    let body = r#"{"data":[{"id":"m"}]}"#;
                    let _ = write!(
                        stream,
                        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                        body.len()
                    );
                }
            });
            Self {
                url,
                carrying_a_key,
            }
        }
    }

    /// The direct children of a process, for the end of a test.
    fn children_of(parent: u32) -> Vec<u32> {
        let Ok(listing) = Command::new("ps")
            .args(["-A", "-o", "pid=,ppid="])
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .output()
        else {
            return Vec::new();
        };
        String::from_utf8_lossy(&listing.stdout)
            .lines()
            .filter_map(|line| {
                let mut columns = line.split_whitespace();
                let pid = columns.next()?.parse().ok()?;
                let ppid: u32 = columns.next()?.parse().ok()?;
                (ppid == parent).then_some(pid)
            })
            .collect()
    }

    /// The server, stopped with its delegated children however the test ends.
    struct Gui {
        _directory: tempfile::TempDir,
        server: Child,
        port: u16,
        /// The created session's stop request, for the server to stop what it started.
        stop: std::cell::RefCell<Option<(String, serde_json::Value)>>,
    }

    impl Drop for Gui {
        fn drop(&mut self) {
            if let Some((path, body)) = self.stop.take() {
                let _ = self.send(&path, &body);
                let deadline = Instant::now() + Duration::from_secs(20);
                while !children_of(self.server.id()).is_empty() && Instant::now() < deadline {
                    std::thread::sleep(Duration::from_millis(50));
                }
            }
            let _ = self.server.kill();
            let _ = self.server.wait();
        }
    }

    impl Gui {
        fn start(provider_url: &str) -> Self {
            let directory = tempfile::tempdir().unwrap();
            let root = directory.path().canonicalize().unwrap();
            let (workspace, statics) = (root.join("workspace"), root.join("out"));
            for path in [&workspace, &statics] {
                std::fs::create_dir_all(path).unwrap();
            }
            std::fs::write(statics.join("index.html"), "<html></html>").unwrap();
            std::fs::write(
                statics.join("commandagent-gui-contract.json"),
                include_str!("../gui/public/commandagent-gui-contract.json"),
            )
            .unwrap();
            let mut server = Command::new(env!("CARGO_BIN_EXE_gui_server"))
                .args([
                    "--port",
                    "0",
                    "--base-path",
                    "/",
                    "--trial-token-auth",
                    "on",
                ])
                .arg("--repository-root")
                .arg(env!("CARGO_MANIFEST_DIR"))
                .arg("--static-dir")
                .arg(&statics)
                .arg("--execution-root")
                .arg(&workspace)
                .arg("--commandagent-bin")
                .arg(env!("CARGO_BIN_EXE_commandagent"))
                .args(["--ollama-host", provider_url])
                .args(["--lm-studio-host", provider_url])
                .env_clear()
                .env("HOME", &root)
                .env("PATH", "/usr/bin:/bin")
                .env("GUI_TRIAL_TOKEN", ACCESS)
                .env("LM_STUDIO_API_TOKEN", DUMMY)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .unwrap();
            let mut stdout = std::io::BufReader::new(server.stdout.take().unwrap());
            let mut line = String::new();
            let port = loop {
                line.clear();
                let read = stdout.read_line(&mut line).unwrap();
                assert_ne!(read, 0, "the server ended before it listened");
                if let Some(port) = line
                    .split("127.0.0.1:")
                    .nth(1)
                    .and_then(|tail| tail.split('/').next())
                    .and_then(|port| port.trim().parse().ok())
                {
                    break port;
                }
            };
            std::thread::spawn(move || {
                let mut sink = String::new();
                while matches!(stdout.read_line(&mut sink), Ok(read) if read > 0) {
                    sink.clear();
                }
            });
            Self {
                _directory: directory,
                server,
                port,
                stop: std::cell::RefCell::new(None),
            }
        }

        fn post(&self, path: &str, body: &serde_json::Value) -> serde_json::Value {
            let response = self.send(path, body).unwrap();
            let (head, payload) = response.split_once("\r\n\r\n").unwrap();
            assert!(head.starts_with("HTTP/1.1 2"), "{head}\n{payload}");
            serde_json::from_str(payload).unwrap()
        }

        fn send(&self, path: &str, body: &serde_json::Value) -> std::io::Result<String> {
            let mut stream = TcpStream::connect(("127.0.0.1", self.port))?;
            let body = body.to_string();
            write!(
                stream,
                "POST {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nOrigin: http://127.0.0.1:{port}\r\nAuthorization: Bearer {ACCESS}\r\nContent-Type: application/json\r\nContent-Length: {length}\r\nConnection: close\r\n\r\n{body}",
                port = self.port,
                length = body.len(),
            )?;
            let mut response = String::new();
            stream.read_to_string(&mut response)?;
            Ok(response)
        }

        /// Create one session, which starts the delegated CLI.
        fn delegate(&self) {
            let mut spec = serde_json::json!({
                "goal": "Create a CLI --pattern filter command",
                "profile": "python-cli",
                "provider": "lm-studio",
                "model": "m",
                "planner_provider": "lm-studio",
                "planner_model": "m",
            });
            let proposal = self.post("/api/session-proposals", &spec);
            spec["confirmation_hash"] = proposal["card_hash"].clone();
            let created = self.post("/api/sessions", &spec);
            let id = created["id"].as_str().unwrap();
            *self.stop.borrow_mut() = Some((
                format!("/api/sessions/{id}/stop"),
                serde_json::json!({ "generation": created["process_generation"] }),
            ));
        }
    }

    /// The key the server received reaches the provider from the CLI it delegated to.
    #[test]
    fn a_delegated_cli_authenticates_with_the_key_the_server_received() {
        let provider = HoldingProvider::start();
        let gui = Gui::start(&provider.url);
        gui.delegate();
        let (_, request, _held) = provider
            .carrying_a_key
            .recv_timeout(Duration::from_secs(60))
            .expect("no request carried a key");
        let expected = format!("Bearer {DUMMY}");
        assert!(authorization_of(&[request]).contains(&expected));
    }

    /// The server's own request for the model list carries the key it received, for this one
    /// header. It comes from the server itself: nothing was delegated here.
    #[test]
    fn the_model_list_request_carries_the_token_the_server_received() {
        let provider = HoldingProvider::start();
        let gui = Gui::start(&provider.url);
        let port = gui.port;
        std::thread::spawn(move || {
            let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
            let _ = write!(
                stream,
                "GET /api/provider-models?provider=lm-studio HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"
            );
            let _ = stream.read_to_end(&mut Vec::new());
        });
        let (peer, request, _held) = provider
            .carrying_a_key
            .recv_timeout(Duration::from_secs(60))
            .expect("the model list request carried no key");
        assert!(request.starts_with("GET /v1/models "), "the path");
        let expected = format!("Bearer {DUMMY}");
        assert!(authorization_of(&[request]).contains(&expected));
        #[cfg(target_os = "macos")]
        assert_eq!(owner_of_connection(peer), gui.server.id());
        let _ = peer;
        assert!(children_of(gui.server.id()).is_empty());
    }

    /// The process that holds this connection, other than this test.
    #[cfg(target_os = "macos")]
    fn owner_of_connection(peer: u16) -> u32 {
        let listing = Command::new("/usr/sbin/lsof")
            .args(["-nP", &format!("-iTCP:{peer}"), "-sTCP:ESTABLISHED", "-Fp"])
            .env_clear()
            .env("PATH", "/usr/bin:/bin:/usr/sbin")
            .output()
            .unwrap();
        let owners = String::from_utf8_lossy(&listing.stdout)
            .lines()
            .filter_map(|line| line.strip_prefix('p')?.parse::<u32>().ok())
            .filter(|pid| *pid != std::process::id())
            .collect::<Vec<_>>();
        assert_eq!(owners.len(), 1, "{owners:?}");
        owners[0]
    }

    /// Once the delegated CLI has reached the provider, the key is not in its exec-time area,
    /// and it was never left in the server's. It is the process that sent the request, a child
    /// of this server, running what the server delegates.
    #[cfg(target_os = "macos")]
    #[test]
    fn neither_the_delegated_cli_nor_the_server_keeps_the_key_in_its_exec_time_area() {
        let provider = HoldingProvider::start();
        let gui = Gui::start(&provider.url);
        gui.delegate();
        let (peer, _, _held) = provider
            .carrying_a_key
            .recv_timeout(Duration::from_secs(60))
            .expect("no request carried a key");
        let child = owner_of_connection(peer);
        assert!(children_of(gui.server.id()).contains(&child));
        let child_area = procargs::read(child);
        let server_area = procargs::read(gui.server.id());
        assert!(!procargs::holds(&child_area.raw, DUMMY), "the child's area");
        assert!(
            !procargs::holds(&server_area.raw, DUMMY),
            "the server's area"
        );
        assert!(
            child_area
                .argv
                .iter()
                .any(|argument| argument == b"--ultra-plan-run"),
            "it is not the delegated CLI"
        );
        assert!(
            server_area
                .env
                .iter()
                .any(|row| row.starts_with(b"COMMANDAGENT_BOOT_FD=")),
            "the server did not go through its start"
        );
    }

    /// The version check runs a program that records the environment it got. It got the
    /// ordinary names and no credential.
    #[test]
    fn a_version_check_gets_no_credential() {
        use std::os::unix::fs::PermissionsExt as _;
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let recorded = root.join("recorded-env");
        let program = root.join("recording-commandagent");
        std::fs::write(
            &program,
            format!(
                "#!/bin/sh\nenv > '{}'\nprintf 'commandagent 0.1.0 test\\n'\n",
                recorded.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        let statics = root.join("out");
        std::fs::create_dir_all(&statics).unwrap();
        let _output = Command::new(env!("CARGO_BIN_EXE_gui_server"))
            .args(["--check", "--json"])
            .arg("--repository-root")
            .arg(&root)
            .arg("--static-dir")
            .arg(&statics)
            .arg("--commandagent-bin")
            .arg(&program)
            .env_clear()
            .env("HOME", &root)
            .env("PATH", "/usr/bin:/bin")
            .env("OPENAI_API_KEY", DUMMY)
            .env("GEMINI_API_KEY", DUMMY)
            .env("LM_STUDIO_API_TOKEN", DUMMY)
            .output()
            .unwrap();
        let environment = std::fs::read_to_string(&recorded).expect("the check ran the program");
        assert!(environment.contains("PATH="));
        assert!(!environment.contains(DUMMY));
        assert!(!environment.contains("_API_KEY"));
        assert!(!environment.contains("LM_STUDIO_API_TOKEN"));
    }
}

#[test]
fn a_command_line_that_does_not_parse_still_says_only_what_it_said() {
    let workspace = tempfile::tempdir().unwrap();
    let output = run_to_end(workspace.path(), |command| {
        command
            .args(["--api-key-env", "AUTH", "--no-such-flag"])
            .env("AUTH", DUMMY);
    });
    assert!(!output.status.success());
    assert!(!String::from_utf8_lossy(&output.stderr).contains(DUMMY));
}
