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
}

impl FakeProvider {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let (sender, first_request) = mpsc::channel();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut stream = stream;
                let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                let mut heard = [0u8; 8192];
                let read = stream.read(&mut heard).unwrap_or(0);
                let _ = sender.send(String::from_utf8_lossy(&heard[..read]).into_owned());
                let _ = stream.write_all(
                    b"HTTP/1.1 500 Internal Server Error\r\ncontent-length: 0\r\nconnection: close\r\n\r\n",
                );
            }
        });
        Self { url, first_request }
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
    let provider = FakeProvider::start();
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

/// Run to its end, and what the fake heard.
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
    let provider = FakeProvider::start();
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
    let provider = FakeProvider::start();
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
    let provider = FakeProvider::start();
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
    let provider = FakeProvider::start();
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
    use std::os::fd::AsRawFd as _;
    use std::os::unix::process::CommandExt as _;
    use std::time::Instant;

    use super::*;

    const MARKER: &str = "COMMANDAGENT_BOOT_FD";
    /// A number the handoff reaches the child at, free in the child.
    const AT: i32 = 40;

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

    /// Both ends, close-on-exec, in this process.
    fn pipe() -> (std::fs::File, std::fs::File) {
        use std::os::fd::FromRawFd as _;
        let mut ends = [-1i32; 2];
        // SAFETY: `ends` is writable for the two descriptors.
        assert_eq!(unsafe { libc::pipe(ends.as_mut_ptr()) }, 0);
        for end in ends {
            // SAFETY: `end` is open, and this only sets a flag.
            let answer = unsafe { libc::fcntl(end, libc::F_SETFD, libc::FD_CLOEXEC) };
            assert_eq!(answer, 0);
        }
        // SAFETY: both are open, and nothing else owns them.
        unsafe {
            (
                std::fs::File::from_raw_fd(ends[0]),
                std::fs::File::from_raw_fd(ends[1]),
            )
        }
    }

    /// A pipe holding these bytes, with its writer closed.
    fn holding(bytes: &[u8]) -> std::fs::File {
        let (reader, mut writer) = pipe();
        writer.write_all(bytes).unwrap();
        reader
    }

    /// The handoff at [`AT`] in the child, for the child alone.
    fn hand_at(command: &mut Command, source: &std::fs::File) {
        let source = source.as_raw_fd();
        // SAFETY: `dup2` is async-signal-safe, and `AT` is free in a new child.
        unsafe {
            command.pre_exec(move || {
                if libc::dup2(source, AT) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }

    /// A refusal: not zero, soon, in a fixed sentence with no secret, and no provider call.
    fn assert_refused(label: &str, build: impl Fn(&mut Command)) {
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
        assert!(
            stderr.contains("the startup credential handoff failed"),
            "{label}: {stderr}"
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
        for marker in ["", "x", "-1", "03", "0", "1", "2", "99999999999", "77"] {
            assert_refused(marker, |command| {
                command.env(MARKER, marker);
            });
        }
    }

    #[test]
    fn a_descriptor_that_is_not_a_pipe_stops_the_start() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let file = file.reopen().unwrap();
        assert_refused("a file", |command| {
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
            let reader = holding(&bytes);
            assert_refused(label, |command| {
                hand_at(command, &reader);
                command.env(MARKER, AT.to_string());
            });
        }
    }

    /// More than the bound, however much the pipe took: what it took already exceeds it.
    #[test]
    fn a_handoff_over_the_bound_stops_the_start() {
        let (reader, mut writer) = pipe();
        let filler = std::thread::spawn(move || {
            let _ = writer.write_all(&vec![b'x'; 64 * 1024]);
        });
        assert_refused("over the bound", |command| {
            hand_at(command, &reader);
            command.env(MARKER, AT.to_string());
        });
        drop(reader);
        filler.join().unwrap();
    }

    /// A writer that never closes: the start ends on it instead of waiting for it.
    #[test]
    fn a_handoff_whose_writer_stays_open_stops_the_start_without_waiting() {
        for bytes in [Vec::new(), frame(&[("OPENAI_API_KEY", DUMMY)])] {
            let (reader, mut writer) = pipe();
            writer.write_all(&bytes).unwrap();
            assert_refused("an open writer", |command| {
                hand_at(command, &reader);
                command.env(MARKER, AT.to_string());
            });
            drop(writer);
        }
    }

    /// The marker is not proof of a clean environment.
    #[test]
    fn a_marker_beside_a_credential_in_the_environment_stops_the_start() {
        for bytes in [
            frame(&[]),
            frame(&[("OPENAI_API_KEY", "another-dummy-value")]),
        ] {
            let reader = holding(&bytes);
            assert_refused("a credential beside the marker", |command| {
                hand_at(command, &reader);
                command
                    .env(MARKER, AT.to_string())
                    .env("OPENAI_API_KEY", DUMMY);
            });
        }
    }

    #[test]
    fn a_handoff_holding_a_name_this_run_does_not_use_stops_the_start() {
        let reader = holding(&frame(&[("PATH", DUMMY)]));
        assert_refused("another name", |command| {
            hand_at(command, &reader);
            command.env(MARKER, AT.to_string());
        });
    }

    /// Too much for the handoff is a refusal, and the credential did not stay in the environment
    /// because the process ended with it there and started nothing.
    #[test]
    fn a_credential_over_the_bound_stops_the_start() {
        assert_refused("a large value", |command| {
            command.env("OPENAI_API_KEY", format!("{DUMMY}{}", "x".repeat(20_000)));
        });
    }

    /// The witness for all of these: a good handoff, by hand, starts and reaches the provider.
    #[test]
    fn a_good_handoff_by_hand_starts() {
        let workspace = tempfile::tempdir().unwrap();
        let provider = FakeProvider::start();
        let reader = holding(&frame(&[("LM_STUDIO_API_TOKEN", DUMMY)]));
        let mut command = super::command(workspace.path());
        command
            .args(lm_studio_arguments(&provider.url))
            .env(MARKER, AT.to_string());
        hand_at(&mut command, &reader);
        let child = command.spawn().unwrap();
        let first = provider.wait_for_request();
        stop(child);
        let expected = format!("Bearer {DUMMY}");
        assert!(authorization_of(&[first]).contains(&expected));
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
