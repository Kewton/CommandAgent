//! The Linux leaf against the real kernel (Issue #982).
//!
//! These tests read `COMMANDAGENT_TEST_LINUX_STRICT`, which only tests read. With `available`
//! the launch must work and every attempt below must get the kernel's refusal, each beside a
//! positive control run outside the sandbox. With `unavailable` every launch must be refused
//! before its exec, with the step named. Unset, they fail: nothing here is skipped.
//!
//! The tool in most of them is this test binary started again as [`raw_child`], which makes
//! each system call itself and prints the kernel's answer, so the errno is the call's own.
//!
//! The body is a `#[cfg(test)] mod tests` of its own so that the protection audit, which reads
//! each source file alone, sees the processes it starts as test code.

pub(super) use tests::planted_fault;

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::collections::BTreeMap;
    use std::os::fd::{AsRawFd as _, FromRawFd as _, OwnedFd};
    use std::os::linux::net::SocketAddrExt as _;
    use std::os::unix::net::SocketAddr as UnixAddress;
    use std::os::unix::process::CommandExt as _;
    use std::path::PathBuf;
    use std::time::Duration;

    use super::super::super::super::process::{self, Ended, Observation, RunLimits};
    use super::super::super::{
        StrictLaunch, final_command, probe_area, probe_strict, strict_command, test_hook,
    };
    use super::super::{
        ProbeSeen, STEPS, Step, confine, describe_refusal, judge_probe, probe, refused_step,
        step_error,
    };

    thread_local! {
        static FAULT: Cell<Option<Step>> = const { Cell::new(None) };
    }

    /// The step a launch prepared on this thread fails at, planted by a test.
    pub(in super::super) fn planted_fault() -> Option<Step> {
        FAULT.with(Cell::get)
    }

    struct Planted;

    impl Drop for Planted {
        fn drop(&mut self) {
            FAULT.with(|fault| fault.set(None));
        }
    }

    fn plant(step: Step) -> Planted {
        FAULT.with(|fault| fault.set(Some(step)));
        Planted
    }

    const EXPECTATION: &str = "COMMANDAGENT_TEST_LINUX_STRICT";

    /// What this host is expected to answer. Unset or anything else fails the test that asks.
    fn available() -> bool {
        match std::env::var(EXPECTATION).as_deref() {
            Ok("available") => true,
            Ok("unavailable") => false,
            other => panic!(
                "set {EXPECTATION}=available or {EXPECTATION}=unavailable to run the Linux strict \
             isolation tests (it is {other:?}); see docs/dev/profile-tools.md"
            ),
        }
    }

    const LIMITS: RunLimits = RunLimits {
        timeout: Duration::from_secs(20),
        max_stream_bytes: 256 * 1024,
    };

    // ---------------------------------------------------------------------------
    // Without a kernel: the probe's judgement and the refusal's number.
    // ---------------------------------------------------------------------------

    const REFUSED: &str = concat!(
        "/bin/bash: line 1: /t/bound/probe-write: Read-only file system\n",
        "/bin/bash: line 1: /t/hidden: No such file or directory\n",
        "/bin/bash: socket: Operation not permitted\n",
        "/bin/bash: line 1: /dev/tcp/127.0.0.1/9: Operation not permitted\n",
        "/bin/bash: socket: Operation not permitted\n",
        "/bin/bash: line 1: /dev/udp/127.0.0.1/9: Operation not permitted\n",
        "/bin/bash: fork: Operation not permitted\n",
    );

    fn seen<'a>(stdout: &'a [u8], stderr: &'a [u8], ended: Ended) -> ProbeSeen<'a> {
        ProbeSeen {
            stdout,
            stderr,
            ended,
            clean: true,
            wrote: false,
            connected: false,
        }
    }

    #[test]
    fn the_probe_judges_only_the_kernels_refusals() {
        let refused = REFUSED.as_bytes();
        let stopped = Ended::Exited(Some(254));
        assert_eq!(judge_probe(&seen(b"n\n", refused, stopped), "n"), Ok(()));
        assert!(judge_probe(&seen(b"x\n", refused, stopped), "n").is_err());
        for ended in [
            Ended::Exited(Some(0)),
            Ended::Exited(None),
            Ended::TimedOut,
            Ended::Cancelled,
            Ended::OutputExceeded,
            Ended::HeldOutput,
        ] {
            assert!(
                judge_probe(&seen(b"n\n", refused, ended), "n").is_err(),
                "{ended:?}"
            );
        }
        let mut wrote = seen(b"n\n", refused, stopped);
        wrote.wrote = true;
        assert!(judge_probe(&wrote, "n").is_err());
        let mut connected = seen(b"n\n", refused, stopped);
        connected.connected = true;
        assert!(judge_probe(&connected, "n").is_err());
        let mut left = seen(b"n\n", refused, stopped);
        left.clean = false;
        assert!(judge_probe(&left, "n").is_err());
        // Another errno is not the refusal: a network namespace alone answers this way.
        for (from, to) in [
            (
                "socket: Operation not permitted",
                "connect: Network is unreachable",
            ),
            ("Read-only file system", "Permission denied"),
            (
                "fork: Operation not permitted",
                "fork: Resource temporarily unavailable",
            ),
            (
                "hidden: No such file or directory",
                "hidden: Permission denied",
            ),
        ] {
            let other = REFUSED.replacen(from, to, 1);
            assert!(
                judge_probe(&seen(b"n\n", other.as_bytes(), stopped), "n").is_err(),
                "{to}"
            );
        }
    }

    #[test]
    fn a_refused_step_reaches_the_parent_with_its_errno() {
        for step in STEPS {
            for errno in [libc::EPERM, libc::EACCES, libc::EINVAL] {
                let error = step_error(step, errno);
                assert_eq!(refused_step(&error), Some((step, errno)));
                assert!(describe_refusal(step, errno).contains(step.name()));
            }
        }
        for plain in [
            libc::EPERM,
            libc::EACCES,
            libc::ENOENT,
            libc::ENOTRECOVERABLE,
        ] {
            assert_eq!(
                refused_step(&std::io::Error::from_raw_os_error(plain)),
                None
            );
        }
    }

    // ---------------------------------------------------------------------------
    // The probe and the launch, as this host is expected to answer.
    // ---------------------------------------------------------------------------

    #[test]
    fn the_probe_answers_as_this_host_is_expected_to() {
        let answer = probe();
        if available() {
            answer.unwrap();
            probe_strict().unwrap();
        } else {
            let reason = answer.unwrap_err();
            assert!(
                reason.contains("was refused at `") || reason.contains("cannot be prepared"),
                "{reason}"
            );
            let refusal = probe_strict().unwrap_err();
            assert_eq!(refusal.kind.as_str(), "unavailable");
            assert!(
                refusal.reason.contains("the strict isolation probe"),
                "{refusal:?}"
            );
        }
    }

    /// Issue #1013 on Linux: under watch the probe's area is removed and looked at again, refused or
    /// not.
    #[test]
    fn a_watched_probe_removes_its_area_and_says_so() {
        let (verdict, seen) = probe_area::watching(probe);
        assert_eq!(verdict.is_ok(), available(), "{verdict:?}");
        assert_eq!(seen.made, 1);
        assert!(seen.clean(), "{seen:?}");
    }

    /// A tool directory with a script, a file outside every pin, and a mark the script would leave.
    struct Stage {
        _temp: tempfile::TempDir,
        bound: PathBuf,
        links: PathBuf,
        outside: PathBuf,
    }

    fn stage() -> Stage {
        let temp = tempfile::tempdir().unwrap();
        let base = std::fs::canonicalize(temp.path()).unwrap();
        let (bound, links, outside) =
            (base.join("bound"), base.join("links"), base.join("outside"));
        for directory in [&bound, &links, &outside] {
            std::fs::create_dir(directory).unwrap();
        }
        Stage {
            _temp: temp,
            bound,
            links,
            outside,
        }
    }

    fn shell() -> PathBuf {
        std::fs::canonicalize("/bin/bash").unwrap()
    }

    /// No launch falls back to an unconfined one: the same script that leaves its mark when run
    /// without the sandbox (the witness) never leaves it through `strict_command`, which either runs
    /// it confined, where the write is refused, or refuses it before its exec.
    #[test]
    fn no_launch_falls_back_to_an_unconfined_one() {
        let stage = stage();
        let mark = stage.outside.join("mark");
        let argv = [
            "-c".to_string(),
            format!(
                "IFS= read -r line; printf '%s\\n' \"$line\"; : > '{}'",
                mark.display()
            ),
        ];
        let env = BTreeMap::new();
        let shell = shell();
        let launch = StrictLaunch {
            executable: &shell,
            argv: &argv,
            cwd: &stage.bound,
            env: &env,
            read_directories: vec![stage.bound.as_path()],
            link_directories: Vec::new(),
        };
        {
            let _unconfined = test_hook::unconfined();
            let witness = process::run(strict_command(&launch).unwrap(), b"w\n", LIMITS, &|| false);
            assert_eq!(witness.unwrap().stdout, b"w\n");
            assert!(mark.exists(), "the witness leaves its mark");
            std::fs::remove_file(&mark).unwrap();
        }
        let command = strict_command(&launch);
        if let Ok(command) = &command {
            let program = command.command.get_program();
            assert_eq!(program, shell.as_os_str());
        }
        match command {
            Err(refusal) => {
                assert!(!available(), "{refusal:?}");
                assert_eq!(refusal.kind.as_str(), "unavailable");
            }
            Ok(command) => match process::run(command, b"n\n", LIMITS, &|| false) {
                Ok(observation) => {
                    assert!(available(), "it ran: {observation:?}");
                    assert_eq!(observation.stdout, b"n\n");
                    let stderr = String::from_utf8_lossy(&observation.stderr);
                    assert!(stderr.contains("No such file or directory"), "{stderr}");
                }
                Err(error) => {
                    assert!(!available(), "{error}");
                    assert!(refused_step(&error).is_some(), "{error}");
                }
            },
        }
        assert!(!mark.exists(), "the sandboxed launch left the mark");
    }

    /// Each step, made to fail, stops the launch before its exec and is named. Where the host lacks
    /// the capability, the real refusal comes first, and is named the same way.
    #[test]
    fn a_step_that_fails_stops_the_launch_before_the_exec() {
        let stage = stage();
        let argv = ["-c".to_string(), "exit 0".to_string()];
        let env = BTreeMap::new();
        let shell = shell();
        // A pinned path through a link, so that the root holds a link to make as well.
        let via = stage.outside.join("via");
        std::os::unix::fs::symlink(&stage.bound, &via).unwrap();
        let launch = StrictLaunch {
            executable: &shell,
            argv: &argv,
            cwd: &stage.bound,
            env: &env,
            read_directories: vec![via.as_path()],
            link_directories: Vec::new(),
        };
        for step in STEPS {
            let _planted = plant(step);
            let mut strict = final_command(shell.as_os_str(), &[], &launch, false);
            if let Err(reason) = confine(&mut strict.command, &launch) {
                assert!(!available(), "{reason}");
                continue;
            }
            let error = process::run(strict, b"", LIMITS, &|| false).unwrap_err();
            let (refused, errno) =
                refused_step(&error).unwrap_or_else(|| panic!("{step:?}: {error}"));
            if available() {
                assert_eq!((refused, errno), (step, libc::EPERM));
            }
        }
    }

    // ---------------------------------------------------------------------------
    // The raw child: this test binary started again as the tool.
    // ---------------------------------------------------------------------------

    const RAW_CHILD: &str = "COMMANDAGENT_TEST_982_RAW_CHILD";
    const RAW_MODE: &str = "COMMANDAGENT_TEST_982_RAW_MODE";
    const RAW_PREFIX: &str = "RAW ";
    const DUMMY_ARGUMENT: &str = "dummy-982-argument";
    const DUMMY_ENV: &str = "DUMMY_982_ENV";
    const DUMMY_VALUE: &str = "dummy-982-not-a-secret";

    fn say(name: &str, answer: &str) {
        println!("{RAW_PREFIX}{name}={answer}");
    }

    fn errno_name(errno: i32) -> String {
        match errno {
            libc::EPERM => "EPERM".to_string(),
            libc::ENOENT => "ENOENT".to_string(),
            libc::ESRCH => "ESRCH".to_string(),
            libc::EBADF => "EBADF".to_string(),
            libc::EACCES => "EACCES".to_string(),
            libc::EFAULT => "EFAULT".to_string(),
            libc::EINVAL => "EINVAL".to_string(),
            libc::EROFS => "EROFS".to_string(),
            libc::ENOSYS => "ENOSYS".to_string(),
            libc::ENETUNREACH => "ENETUNREACH".to_string(),
            libc::ECONNREFUSED => "ECONNREFUSED".to_string(),
            libc::ETIMEDOUT => "ETIMEDOUT".to_string(),
            other => format!("errno {other}"),
        }
    }

    /// `ok` for an answer that is not negative, or the errno's name.
    fn said(answer: i64) -> String {
        if answer >= 0 {
            "ok".to_string()
        } else {
            errno_name(std::io::Error::last_os_error().raw_os_error().unwrap_or(0))
        }
    }

    fn said_io<T>(result: std::io::Result<T>) -> String {
        match result {
            Ok(_) => "ok".to_string(),
            Err(error) => error
                .raw_os_error()
                .map_or_else(|| error.kind().to_string(), errno_name),
        }
    }

    fn var(name: &str) -> String {
        std::env::var(name).unwrap_or_default()
    }

    fn number(name: &str) -> i32 {
        var(name).parse().unwrap_or(-1)
    }

    fn path_of(name: &str) -> std::ffi::CString {
        std::ffi::CString::new(var(name)).unwrap()
    }

    fn raw_network() {
        for (name, domain, kind, protocol) in [
            ("tcp4", libc::AF_INET, libc::SOCK_STREAM, 0),
            ("udp4", libc::AF_INET, libc::SOCK_DGRAM, 0),
            ("tcp6", libc::AF_INET6, libc::SOCK_STREAM, 0),
            ("udp6", libc::AF_INET6, libc::SOCK_DGRAM, 0),
            ("unix-stream", libc::AF_UNIX, libc::SOCK_STREAM, 0),
            ("unix-datagram", libc::AF_UNIX, libc::SOCK_DGRAM, 0),
            ("raw", libc::AF_INET, libc::SOCK_RAW, libc::IPPROTO_ICMP),
            ("packet", libc::AF_PACKET, libc::SOCK_RAW, 0),
            ("netlink", libc::AF_NETLINK, libc::SOCK_RAW, 0),
        ] {
            // SAFETY: `socket` takes no pointer.
            let descriptor = unsafe { libc::socket(domain, kind | libc::SOCK_CLOEXEC, protocol) };
            let answer = said(i64::from(descriptor));
            say(&format!("socket-{name}"), &answer);
            if descriptor >= 0 {
                // SAFETY: this process has just opened `descriptor`.
                unsafe { libc::close(descriptor) };
            }
        }
        let mut pair = [-1; 2];
        // SAFETY: `pair` is writable for two descriptors.
        let answer =
            unsafe { libc::socketpair(libc::AF_UNIX, libc::SOCK_STREAM, 0, pair.as_mut_ptr()) };
        say("socketpair", &said(i64::from(answer)));
        let port = |name: &str| var(name).parse::<u16>().unwrap_or(0);
        say(
            "reach-tcp4",
            &said_io(std::net::TcpStream::connect(("127.0.0.1", port("TCP4")))),
        );
        say(
            "reach-tcp6",
            &said_io(std::net::TcpStream::connect(("::1", port("TCP6")))),
        );
        let datagram = |local: &str, to: (&str, u16)| {
            std::net::UdpSocket::bind(local).and_then(|socket| socket.send_to(b"x", to))
        };
        say(
            "reach-udp4",
            &said_io(datagram("127.0.0.1:0", ("127.0.0.1", port("UDP4")))),
        );
        say(
            "reach-udp6",
            &said_io(datagram("[::1]:0", ("::1", port("UDP6")))),
        );
        say(
            "reach-unix",
            &said_io(std::os::unix::net::UnixStream::connect(var("UNIX_PATH"))),
        );
        let abstract_name = UnixAddress::from_abstract_name(var("ABSTRACT"));
        say(
            "reach-abstract",
            &said_io(
                abstract_name
                    .and_then(|address| std::os::unix::net::UnixStream::connect_addr(&address)),
            ),
        );
    }

    fn raw_files() {
        let open = |name: &str, flags: i32| {
            let path = path_of(name);
            // SAFETY: `path` is NUL-terminated and outlives the call.
            let descriptor = unsafe { libc::open(path.as_ptr(), flags | libc::O_CLOEXEC, 0o644) };
            let answer = said(i64::from(descriptor));
            if descriptor >= 0 {
                // SAFETY: this process has just opened `descriptor`.
                unsafe { libc::close(descriptor) };
            }
            answer
        };
        let stat = |name: &str| {
            let path = path_of(name);
            // SAFETY: an all-zero `stat` is a valid value of this plain struct.
            let mut answer: libc::stat = unsafe { std::mem::zeroed() };
            // SAFETY: `path` is NUL-terminated and `answer` writable; both outlive the call.
            said(i64::from(unsafe {
                libc::stat(path.as_ptr(), &raw mut answer)
            }))
        };
        say("read-secret", &open("SECRET", libc::O_RDONLY));
        say("stat-secret", &stat("SECRET"));
        say("read-visible", &open("VISIBLE", libc::O_RDONLY));
        say("stat-visible", &stat("VISIBLE"));
        say("read-bound", &open("BOUND_EXISTING", libc::O_RDONLY));
        say(
            "append-bound",
            &open("BOUND_EXISTING", libc::O_WRONLY | libc::O_APPEND),
        );
        say(
            "create-bound",
            &open("BOUND_NEW", libc::O_WRONLY | libc::O_CREAT),
        );
        say(
            "create-outside",
            &open("OUTSIDE_NEW", libc::O_WRONLY | libc::O_CREAT),
        );
        say("write-null", &open("NULL", libc::O_WRONLY));
        say("read-passwd", &open("PASSWD", libc::O_RDONLY));
        say("list-root", &said_io(std::fs::read_dir("/")));
    }

    fn raw_processes() {
        let dummy = number("DUMMY");
        // SAFETY: the child of the fork only ends itself, which is async-signal-safe.
        let forked = unsafe { libc::fork() };
        if forked == 0 {
            // SAFETY: as above.
            unsafe { libc::_exit(0) };
        }
        say("fork", &said(i64::from(forked)));
        if forked > 0 {
            let mut status = 0;
            // SAFETY: `forked` is this process's own child, and `status` is writable.
            unsafe { libc::waitpid(forked, &raw mut status, 0) };
        }
        say(
            "spawn",
            &said_io(std::process::Command::new(var("UNPINNED")).status()),
        );
        // SAFETY: a null argument with a zero size asks only for the kernel's answer.
        let clone3 = unsafe { libc::syscall(libc::SYS_clone3, std::ptr::null::<u8>(), 0usize) };
        say("clone3", &said(clone3));
        say(
            "thread",
            &said_io(
                std::thread::spawn(|| 7)
                    .join()
                    .map_err(|_| std::io::Error::other("panicked")),
            ),
        );
        // SAFETY: `getpid` has no preconditions.
        let own = unsafe { libc::getpid() };
        let signal = |target: i32| {
            // SAFETY: signal 0 only asks whether `target` could be signalled; it takes no pointer.
            said(unsafe { libc::syscall(libc::SYS_kill, target, 0) })
        };
        say("signal-dummy", &signal(dummy));
        say("signal-own-group", &signal(0));
        say("signal-itself", &signal(own));
        // SAFETY: `gettid` has no preconditions, and signal 0 to this thread takes no pointer.
        let thread = unsafe { libc::syscall(libc::SYS_tgkill, own, libc::gettid(), 0) };
        say("signal-own-thread", &said(thread));
        let mut name: libc::utsname =
        // SAFETY: an all-zero `utsname` is a valid value of this plain struct.
        unsafe { std::mem::zeroed() };
        // SAFETY: `name` is writable and outlives the call.
        unsafe { libc::uname(&raw mut name) };
        // SAFETY: `uname` wrote a NUL-terminated name.
        let host = unsafe { std::ffi::CStr::from_ptr(name.nodename.as_ptr()) };
        say("hostname", &host.to_string_lossy());
    }

    /// The answer of a call, read at once, before anything else can change the errno.
    fn done(answer: i64) -> (i64, String) {
        (answer, said(answer))
    }

    fn raw_escapes() {
        let dummy = libc::c_long::from(number("DUMMY"));
        let none = std::ptr::null::<libc::c_void>();
        let (zero, minus_one): (libc::c_long, libc::c_long) = (0, -1);
        // SAFETY: each call takes no pointer, or a null one, or a NUL-terminated literal; each asks
        // only for the kernel's answer and changes nothing that answer does not allow. The elements
        // are evaluated in order, each answer read right after its own call.
        let answers = unsafe {
            [
                (
                    "unshare",
                    done(libc::syscall(
                        libc::SYS_unshare,
                        libc::c_long::from(libc::CLONE_NEWUSER),
                    )),
                ),
                (
                    "setns",
                    done(libc::syscall(libc::SYS_setns, minus_one, zero)),
                ),
                (
                    "mount",
                    done(libc::syscall(
                        libc::SYS_mount,
                        c"none".as_ptr(),
                        c"/l216-absent".as_ptr(),
                        c"tmpfs".as_ptr(),
                        zero,
                        none,
                    )),
                ),
                (
                    "chroot",
                    done(libc::syscall(libc::SYS_chroot, c"/".as_ptr())),
                ),
                (
                    "pivot_root",
                    done(libc::syscall(
                        libc::SYS_pivot_root,
                        c"/l216-absent".as_ptr(),
                        c"/l216-absent".as_ptr(),
                    )),
                ),
                (
                    "ptrace",
                    done(libc::syscall(
                        libc::SYS_ptrace,
                        libc::c_long::from(libc::PTRACE_PEEKDATA),
                        dummy,
                        zero,
                        zero,
                    )),
                ),
                (
                    "process_vm_readv",
                    done(libc::syscall(
                        libc::SYS_process_vm_readv,
                        dummy,
                        none,
                        zero,
                        none,
                        zero,
                        zero,
                    )),
                ),
                (
                    "pidfd_open",
                    done(libc::syscall(libc::SYS_pidfd_open, dummy, zero)),
                ),
                (
                    "io_uring_setup",
                    done(libc::syscall(
                        libc::SYS_io_uring_setup,
                        libc::c_long::from(1u8),
                        none,
                    )),
                ),
                (
                    "bpf",
                    done(libc::syscall(
                        libc::SYS_bpf,
                        libc::c_long::from(0x7fffu16),
                        none,
                        zero,
                    )),
                ),
                (
                    "memfd_create",
                    done(libc::syscall(
                        libc::SYS_memfd_create,
                        c"l216".as_ptr(),
                        zero,
                    )),
                ),
                (
                    "userfaultfd",
                    done(libc::syscall(libc::SYS_userfaultfd, zero)),
                ),
                (
                    "keyctl",
                    done(libc::syscall(
                        libc::SYS_keyctl,
                        libc::c_long::from(0x7fffu16),
                        zero,
                        zero,
                        zero,
                        zero,
                    )),
                ),
                (
                    "perf_event_open",
                    done(libc::syscall(
                        libc::SYS_perf_event_open,
                        none,
                        zero,
                        minus_one,
                        minus_one,
                        zero,
                    )),
                ),
            ]
        };
        for (name, (answer, shown)) in answers {
            say(name, &shown);
            if answer > 0 && ["pidfd_open", "memfd_create", "userfaultfd"].contains(&name) {
                // SAFETY: the call just opened this descriptor, and nothing else owns it.
                unsafe { libc::close(i32::try_from(answer).unwrap_or(-1)) };
            }
        }
    }

    fn raw_proc() {
        let dummy = number("DUMMY");
        for part in ["cmdline", "environ"] {
            let answer = std::fs::read(format!("/proc/{dummy}/{part}"));
            let holds = answer.as_ref().is_ok_and(|bytes| {
                let text = String::from_utf8_lossy(bytes);
                text.contains(DUMMY_ARGUMENT) || text.contains(DUMMY_VALUE)
            });
            say(&format!("proc-{part}"), &said_io(answer));
            say(&format!("proc-{part}-holds-dummy"), &holds.to_string());
        }
        say("proc", &said_io(std::fs::metadata("/proc")));
    }

    fn raw_descriptors() {
        for name in ["FD_FILE", "FD_SOCKET"] {
            // SAFETY: an all-zero `stat` is a valid value of this plain struct.
            let mut answer: libc::stat = unsafe { std::mem::zeroed() };
            // SAFETY: `answer` is writable; `fstat` of any number is safe to ask.
            let result = unsafe { libc::fstat(number(name), &raw mut answer) };
            let shown = if result == 0 {
                format!("ok {}:{}", answer.st_dev, answer.st_ino)
            } else {
                said(i64::from(result))
            };
            say(name, &shown);
        }
    }

    fn raw_environment() {
        let mut keys = std::env::vars_os()
            .map(|(key, _)| key.to_string_lossy().to_string())
            .collect::<Vec<_>>();
        keys.sort();
        say("env", &keys.join(","));
    }

    /// Inside only: an exec of a program that is there but not pinned, of one that is not there,
    /// then of the pinned program itself, which must still be inside afterwards.
    fn raw_exec() {
        let refused = std::process::Command::new(var("UNPINNED")).exec();
        say("exec-unpinned", &said_io(Err::<(), _>(refused)));
        let absent = std::process::Command::new("/bin/sh").exec();
        say("exec-absent", &said_io(Err::<(), _>(absent)));
        // No `/proc/self/exe` inside: the path comes from the parent.
        let again = std::process::Command::new(var("EXECUTABLE"))
            .args(child_arguments())
            .env_clear()
            .envs(std::env::vars_os().filter(|(key, _)| key != RAW_MODE))
            .env(RAW_MODE, "after-exec")
            .exec();
        say("exec-pinned", &said_io(Err::<(), _>(again)));
    }

    fn raw_after_exec() {
        // SAFETY: `socket` takes no pointer.
        let descriptor = unsafe { libc::socket(libc::AF_INET, libc::SOCK_STREAM, 0) };
        say("after-exec-socket", &said(i64::from(descriptor)));
        // SAFETY: as in `raw_processes`.
        let forked = unsafe { libc::fork() };
        if forked == 0 {
            // SAFETY: as above.
            unsafe { libc::_exit(0) };
        }
        say("after-exec-fork", &said(i64::from(forked)));
        let path = path_of("BOUND_NEW");
        // SAFETY: `path` is NUL-terminated and outlives the call.
        let created = unsafe {
            libc::open(
                path.as_ptr(),
                libc::O_WRONLY | libc::O_CREAT | libc::O_CLOEXEC,
                0o644,
            )
        };
        say("after-exec-create", &said(i64::from(created)));
    }

    /// The tool. It runs only when started as one, and then only what its mode names.
    #[test]
    fn raw_child() {
        if std::env::var_os(RAW_CHILD).is_none() {
            return;
        }
        match var(RAW_MODE).as_str() {
            "network" => raw_network(),
            "files" => raw_files(),
            "processes" => raw_processes(),
            "escapes" => raw_escapes(),
            "proc" => raw_proc(),
            "descriptors" => raw_descriptors(),
            "environment" => raw_environment(),
            "exec" => raw_exec(),
            "after-exec" => raw_after_exec(),
            other => panic!("no mode {other}"),
        }
    }

    fn child_arguments() -> Vec<String> {
        let (_, module) = module_path!().split_once("::").unwrap();
        vec![
            "--exact".to_string(),
            format!("{module}::raw_child"),
            "--nocapture".to_string(),
            "--test-threads=1".to_string(),
        ]
    }

    fn answers(stdout: &[u8]) -> BTreeMap<String, String> {
        String::from_utf8_lossy(stdout)
            .lines()
            // The harness's own `test … ` has no line end, so the first answer follows it.
            .filter_map(|line| line.split_once(RAW_PREFIX).map(|(_, answer)| answer))
            .filter_map(|line| line.split_once('='))
            .map(|(name, answer)| (name.to_string(), answer.to_string()))
            .collect()
    }

    /// Everything a raw child is pointed at, with the witnesses that count what reaches them.
    struct Scene {
        stage: Stage,
        executable: PathBuf,
        env: BTreeMap<String, String>,
        tcp4: std::net::TcpListener,
        tcp6: std::net::TcpListener,
        udp4: std::net::UdpSocket,
        udp6: std::net::UdpSocket,
        unix: std::os::unix::net::UnixListener,
        abstract_socket: std::os::unix::net::UnixListener,
        dummy: std::process::Child,
    }

    impl Drop for Scene {
        fn drop(&mut self) {
            let _ = self.dummy.kill();
            let _ = self.dummy.wait();
        }
    }

    impl Scene {
        fn new(mode: &str) -> Self {
            let stage = stage();
            let executable = std::fs::canonicalize(std::env::current_exe().unwrap()).unwrap();
            let tcp4 = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let tcp6 = std::net::TcpListener::bind("[::1]:0").unwrap();
            let udp4 = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
            let udp6 = std::net::UdpSocket::bind("[::1]:0").unwrap();
            tcp4.set_nonblocking(true).unwrap();
            tcp6.set_nonblocking(true).unwrap();
            udp4.set_nonblocking(true).unwrap();
            udp6.set_nonblocking(true).unwrap();
            let unix_path = stage.outside.join("witness.sock");
            let unix = std::os::unix::net::UnixListener::bind(&unix_path).unwrap();
            unix.set_nonblocking(true).unwrap();
            let abstract_name = format!("commandagent-982-{}", uuid::Uuid::now_v7());
            let address = UnixAddress::from_abstract_name(abstract_name.as_bytes()).unwrap();
            let abstract_socket = std::os::unix::net::UnixListener::bind_addr(&address).unwrap();
            abstract_socket.set_nonblocking(true).unwrap();
            let dummy = std::process::Command::new("/bin/bash")
                .args(["-c", "IFS= read -r _", DUMMY_ARGUMENT])
                .env_clear()
                .env(DUMMY_ENV, DUMMY_VALUE)
                .stdin(std::process::Stdio::piped())
                .spawn()
                .unwrap();
            let (secret, visible) = (stage.outside.join("secret"), stage.links.join("visible"));
            let existing = stage.bound.join("existing");
            for (path, text) in [
                (&secret, "dummy-982-secret\n"),
                (&visible, "dummy-982-visible\n"),
                (&existing, "unchanged"),
            ] {
                std::fs::write(path, text).unwrap();
            }
            let unpinned = stage.bound.join("unpinned");
            std::fs::copy(std::fs::canonicalize("/bin/true").unwrap(), &unpinned).unwrap();
            let port = |address: std::io::Result<std::net::SocketAddr>| {
                address.unwrap().port().to_string()
            };
            let env = BTreeMap::from([
                (RAW_CHILD.to_string(), "1".to_string()),
                (RAW_MODE.to_string(), mode.to_string()),
                ("TCP4".to_string(), port(tcp4.local_addr())),
                ("TCP6".to_string(), port(tcp6.local_addr())),
                ("UDP4".to_string(), port(udp4.local_addr())),
                ("UDP6".to_string(), port(udp6.local_addr())),
                ("UNIX_PATH".to_string(), unix_path.display().to_string()),
                ("ABSTRACT".to_string(), abstract_name),
                ("DUMMY".to_string(), dummy.id().to_string()),
                ("SECRET".to_string(), secret.display().to_string()),
                ("VISIBLE".to_string(), visible.display().to_string()),
                ("BOUND_EXISTING".to_string(), existing.display().to_string()),
                (
                    "BOUND_NEW".to_string(),
                    stage.bound.join("new").display().to_string(),
                ),
                (
                    "OUTSIDE_NEW".to_string(),
                    stage.outside.join("new").display().to_string(),
                ),
                ("UNPINNED".to_string(), unpinned.display().to_string()),
                ("EXECUTABLE".to_string(), executable.display().to_string()),
                ("NULL".to_string(), "/dev/null".to_string()),
                ("PASSWD".to_string(), "/etc/passwd".to_string()),
            ]);
            Self {
                stage,
                executable,
                env,
                tcp4,
                tcp6,
                udp4,
                udp6,
                unix,
                abstract_socket,
                dummy,
            }
        }

        fn with(mut self, name: &str, value: String) -> Self {
            self.env.insert(name.to_string(), value);
            self
        }

        /// What reached the witnesses since the last call.
        fn reached(&self) -> usize {
            let mut buffer = [0u8; 16];
            let mut count = 0;
            while self.tcp4.accept().is_ok() {
                count += 1;
            }
            while self.tcp6.accept().is_ok() {
                count += 1;
            }
            while self.udp4.recv(&mut buffer).is_ok() {
                count += 1;
            }
            while self.udp6.recv(&mut buffer).is_ok() {
                count += 1;
            }
            while self.unix.accept().is_ok() {
                count += 1;
            }
            while self.abstract_socket.accept().is_ok() {
                count += 1;
            }
            count
        }

        /// The positive control: the same program, arguments, and variables, as a plain child with
        /// no sandbox and no descriptor marking.
        fn outside(&self) -> BTreeMap<String, String> {
            let output = std::process::Command::new(&self.executable)
                .args(child_arguments())
                .env_clear()
                .envs(&self.env)
                .current_dir(&self.stage.bound)
                .stdin(std::process::Stdio::null())
                .output()
                .unwrap();
            assert!(output.status.success(), "{output:?}");
            answers(&output.stdout)
        }

        /// The same child under the strict launch, or `None` after checking that it was refused
        /// before its exec where the host is expected to lack the capability.
        fn inside(&self) -> Option<BTreeMap<String, String>> {
            let argv = child_arguments();
            let launch = StrictLaunch {
                executable: &self.executable,
                argv: &argv,
                cwd: &self.stage.bound,
                env: &self.env,
                read_directories: vec![self.stage.bound.as_path()],
                link_directories: vec![self.stage.links.as_path()],
            };
            let mut strict = final_command(self.executable.as_os_str(), &[], &launch, false);
            if let Err(reason) = confine(&mut strict.command, &launch) {
                assert!(!available(), "{reason}");
                return None;
            }
            match process::run(strict, b"", LIMITS, &|| false) {
                Ok(observation) => {
                    assert!(available(), "it ran: {observation:?}");
                    assert_eq!(observation.failure_code(), None, "{}", shown(&observation));
                    Some(answers(&observation.stdout))
                }
                Err(error) => {
                    assert!(!available(), "{error}");
                    assert!(refused_step(&error).is_some(), "{error}");
                    None
                }
            }
        }
    }

    fn shown(observation: &Observation) -> String {
        format!(
            "{:?}\n--- stdout\n{}\n--- stderr\n{}",
            observation.ended,
            String::from_utf8_lossy(&observation.stdout),
            String::from_utf8_lossy(&observation.stderr)
        )
    }

    fn answer<'a>(answers: &'a BTreeMap<String, String>, name: &str) -> &'a str {
        answers
            .get(name)
            .unwrap_or_else(|| panic!("no answer for {name}: {answers:?}"))
    }

    /// Invariant 3: every kind of socket is refused at `socket()` with `EPERM`, and nothing reaches
    /// a witness. Outside, the same calls make sockets and reach every witness. A timeout or an
    /// unreachable address never counts as the refusal.
    #[test]
    fn every_network_path_is_refused_at_its_socket_and_reaches_no_witness() {
        let scene = Scene::new("network");
        let outside = scene.outside();
        for name in [
            "tcp4",
            "udp4",
            "tcp6",
            "udp6",
            "unix-stream",
            "unix-datagram",
            "netlink",
        ] {
            assert_eq!(
                answer(&outside, &format!("socket-{name}")),
                "ok",
                "{outside:?}"
            );
        }
        assert_eq!(answer(&outside, "socketpair"), "ok");
        for name in [
            "reach-tcp4",
            "reach-tcp6",
            "reach-udp4",
            "reach-udp6",
            "reach-unix",
            "reach-abstract",
        ] {
            assert_eq!(answer(&outside, name), "ok", "{name}: {outside:?}");
        }
        assert_eq!(
            scene.reached(),
            6,
            "the outside run reaches each witness once"
        );
        let Some(inside) = scene.inside() else {
            return;
        };
        for name in [
            "socket-tcp4",
            "socket-udp4",
            "socket-tcp6",
            "socket-udp6",
            "socket-unix-stream",
            "socket-unix-datagram",
            "socket-raw",
            "socket-packet",
            "socket-netlink",
            "socketpair",
            "reach-tcp4",
            "reach-tcp6",
            "reach-udp4",
            "reach-udp6",
            "reach-unix",
            "reach-abstract",
        ] {
            assert_eq!(answer(&inside, name), "EPERM", "{name}: {inside:?}");
        }
        assert_eq!(scene.reached(), 0, "a witness was reached from inside");
    }

    /// Invariants 1 and 2: no write lands anywhere, a host file outside the root is not there
    /// (`ENOENT`, Q2), a visible but unallowed file is refused by Landlock (`EACCES`), and the
    /// dummy secret is never read. Outside, each of these succeeds.
    #[test]
    fn writes_and_reads_outside_the_pins_are_refused_by_the_kernel() {
        let outside_scene = Scene::new("files");
        let outside = outside_scene.outside();
        for name in [
            "read-secret",
            "stat-secret",
            "read-visible",
            "stat-visible",
            "read-bound",
            "append-bound",
            "create-bound",
            "create-outside",
            "write-null",
        ] {
            assert_eq!(answer(&outside, name), "ok", "{name}: {outside:?}");
        }
        let scene = Scene::new("files");
        let Some(inside) = scene.inside() else {
            return;
        };
        for (name, expected) in [
            ("read-secret", "ENOENT"),
            ("stat-secret", "ENOENT"),
            ("read-visible", "EACCES"),
            ("stat-visible", "ok"),
            ("read-bound", "ok"),
            ("append-bound", "EROFS"),
            ("create-bound", "EROFS"),
            ("create-outside", "ENOENT"),
            ("write-null", "ok"),
            ("read-passwd", "ENOENT"),
            ("list-root", "EACCES"),
        ] {
            assert_eq!(answer(&inside, name), expected, "{name}: {inside:?}");
        }
        let bound = &scene.stage.bound;
        assert_eq!(
            std::fs::read_to_string(bound.join("existing")).unwrap(),
            "unchanged"
        );
        assert!(!bound.join("new").exists());
        assert!(!scene.stage.outside.join("new").exists());
    }

    /// Invariant 4 and the signals: no process can be made, a thread can, and a signal reaches only
    /// the tool itself. The host name is the fixed one. Outside, each refusal here succeeds.
    #[test]
    fn a_new_process_and_a_signal_to_another_are_refused_by_the_kernel() {
        let scene = Scene::new("processes");
        let outside = scene.outside();
        for name in [
            "fork",
            "spawn",
            "thread",
            "signal-dummy",
            "signal-own-group",
            "signal-itself",
            "signal-own-thread",
        ] {
            assert_eq!(answer(&outside, name), "ok", "{name}: {outside:?}");
        }
        let Some(inside) = scene.inside() else {
            return;
        };
        // Where the backend works the host adds no filter of its own, so these answer otherwise
        // outside. (Docker's default profile, where it is unavailable, answers `clone3` the same.)
        assert_ne!(answer(&outside, "clone3"), "ENOSYS");
        assert_ne!(answer(&outside, "hostname"), "commandagent-tool");
        for (name, expected) in [
            ("fork", "EPERM"),
            ("spawn", "EPERM"),
            ("clone3", "ENOSYS"),
            ("thread", "ok"),
            ("signal-dummy", "EPERM"),
            ("signal-own-group", "EPERM"),
            ("signal-itself", "ok"),
            ("signal-own-thread", "ok"),
            ("hostname", "commandagent-tool"),
        ] {
            assert_eq!(answer(&inside, name), expected, "{name}: {inside:?}");
        }
    }

    /// Invariant 5: an exec of a program that is there but not pinned is refused by Landlock, one
    /// that is not there is not found, and an exec of the pinned program stays inside. Outside, the
    /// unpinned program runs.
    #[test]
    fn an_exec_of_anything_but_the_pinned_program_is_refused_and_a_re_exec_stays_inside() {
        let scene = Scene::new("exec");
        let control = std::process::Command::new(&scene.env["UNPINNED"])
            .status()
            .unwrap();
        assert!(control.success(), "the unpinned program runs outside");
        let Some(inside) = scene.inside() else {
            return;
        };
        for (name, expected) in [
            ("exec-unpinned", "EACCES"),
            ("exec-absent", "ENOENT"),
            ("after-exec-socket", "EPERM"),
            ("after-exec-fork", "EPERM"),
            ("after-exec-create", "EROFS"),
        ] {
            assert_eq!(answer(&inside, name), expected, "{name}: {inside:?}");
        }
        assert!(
            !inside.contains_key("exec-pinned"),
            "the re-exec failed: {inside:?}"
        );
    }

    /// The calls that would leave the sandbox or reach another process are refused. `setns` and
    /// `pidfd_open` answer otherwise outside, so the refusal is the filter's.
    #[test]
    fn every_way_out_is_refused_by_the_filter() {
        let scene = Scene::new("escapes");
        let outside = scene.outside();
        let Some(inside) = scene.inside() else {
            return;
        };
        // Where the backend works the host adds no filter of its own (Docker's default profile,
        // where it is unavailable, refuses `setns` itself).
        assert_eq!(answer(&outside, "setns"), "EBADF");
        assert_eq!(answer(&outside, "pidfd_open"), "ok");
        assert_eq!(answer(&outside, "memfd_create"), "ok");
        for name in [
            "unshare",
            "setns",
            "mount",
            "chroot",
            "pivot_root",
            "ptrace",
            "process_vm_readv",
            "pidfd_open",
            "io_uring_setup",
            "bpf",
            "memfd_create",
            "userfaultfd",
            "keyctl",
            "perf_event_open",
        ] {
            assert_eq!(answer(&inside, name), "EPERM", "{name}: {inside:?}");
        }
    }

    /// Invariant 8 and #981's requirement 5 on Linux (Q1): a dummy process's arguments and
    /// environment are readable outside and not there inside, because no `/proc` is.
    #[test]
    fn another_processs_arguments_and_environment_are_not_there() {
        let scene = Scene::new("proc");
        let outside = scene.outside();
        for part in ["cmdline", "environ"] {
            assert_eq!(
                answer(&outside, &format!("proc-{part}")),
                "ok",
                "{outside:?}"
            );
            assert_eq!(
                answer(&outside, &format!("proc-{part}-holds-dummy")),
                "true"
            );
        }
        let Some(inside) = scene.inside() else {
            return;
        };
        for part in ["cmdline", "environ"] {
            assert_eq!(
                answer(&inside, &format!("proc-{part}")),
                "ENOENT",
                "{inside:?}"
            );
            assert_eq!(
                answer(&inside, &format!("proc-{part}-holds-dummy")),
                "false"
            );
        }
        assert_eq!(answer(&inside, "proc"), "ENOENT");
    }

    /// A descriptor left open without close-on-exec, at a number high enough that no runtime takes
    /// it for itself (F7), with its identity.
    fn inheritable(file: OwnedFd) -> (OwnedFd, i32, String) {
        // SAFETY: `file` is open; this makes a new descriptor, which has no close-on-exec flag.
        let moved = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_DUPFD, 600) };
        assert!(moved >= 600, "{moved}");
        // SAFETY: `moved` was just opened, and nothing else owns it.
        let held = unsafe { OwnedFd::from_raw_fd(moved) };
        // SAFETY: an all-zero `stat` is a valid value of this plain struct.
        let mut answer: libc::stat = unsafe { std::mem::zeroed() };
        // SAFETY: `moved` is open and `answer` writable.
        assert_eq!(unsafe { libc::fstat(moved, &raw mut answer) }, 0);
        (
            held,
            moved,
            format!("ok {}:{}", answer.st_dev, answer.st_ino),
        )
    }

    /// Invariant 6: a file and a socket the caller left open reach a plain child, the same file and
    /// socket by identity, and are a bad descriptor inside.
    #[test]
    fn a_descriptor_the_caller_left_open_does_not_reach_the_tool() {
        let stage = stage();
        let file = std::fs::File::open(stage.outside.as_path()).unwrap();
        let (_file, file_number, file_identity) = inheritable(file.into());
        let (left, _right) = std::os::unix::net::UnixStream::pair().unwrap();
        let (_socket, socket_number, socket_identity) = inheritable(left.into());
        let scene = Scene::new("descriptors")
            .with("FD_FILE", file_number.to_string())
            .with("FD_SOCKET", socket_number.to_string());
        let outside = scene.outside();
        assert_eq!(answer(&outside, "FD_FILE"), file_identity);
        assert_eq!(answer(&outside, "FD_SOCKET"), socket_identity);
        let Some(inside) = scene.inside() else {
            return;
        };
        assert_eq!(answer(&inside, "FD_FILE"), "EBADF", "{inside:?}");
        assert_eq!(answer(&inside, "FD_SOCKET"), "EBADF", "{inside:?}");
    }

    const OWN_PROCESS: &str = "COMMANDAGENT_TEST_982_OWN_PROCESS";
    const HOST_DUMMY: &str = "COMMANDAGENT_TEST_982_HOST_DUMMY";

    /// Invariant 7: in a process of its own that holds a dummy variable from its start, a plain
    /// child sees the dummy and the tool sees only the declared keys.
    #[test]
    fn only_the_declared_keys_reach_the_tool() {
        if std::env::var_os(OWN_PROCESS).is_none() {
            let (_, module) = module_path!().split_once("::").unwrap();
            let mut command = std::process::Command::new(std::env::current_exe().unwrap());
            command
                .args([
                    "--exact",
                    &format!("{module}::only_the_declared_keys_reach_the_tool"),
                ])
                .arg("--nocapture")
                .env_clear()
                .env(OWN_PROCESS, "1")
                .env(HOST_DUMMY, "dummy-982-host-value");
            for name in [EXPECTATION, "TMPDIR", "PATH"] {
                if let Some(value) = std::env::var_os(name) {
                    command.env(name, value);
                }
            }
            let output = command.output().unwrap();
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(
                output.status.success() && stdout.contains("1 passed"),
                "{stdout}\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }
        assert!(std::env::var_os(HOST_DUMMY).is_some());
        let scene = Scene::new("environment");
        let mut leaking = std::process::Command::new(&scene.executable);
        leaking.args(child_arguments()).envs(&scene.env);
        let leaked = answers(&leaking.output().unwrap().stdout);
        assert!(answer(&leaked, "env").contains(HOST_DUMMY), "{leaked:?}");
        let Some(inside) = scene.inside() else {
            return;
        };
        let declared = scene.env.keys().cloned().collect::<Vec<_>>().join(",");
        assert_eq!(answer(&inside, "env"), declared);
    }

    /// The fork test of the macOS policy, in bash: the subshell is refused and bash stops there,
    /// with no descendant left.
    #[test]
    fn a_fork_in_bash_is_refused_with_no_descendant() {
        let stage = stage();
        let argv = ["-c".to_string(), "( : )\nexit 0\n".to_string()];
        let env = BTreeMap::new();
        let shell = shell();
        let launch = StrictLaunch {
            executable: &shell,
            argv: &argv,
            cwd: &stage.bound,
            env: &env,
            read_directories: vec![stage.bound.as_path()],
            link_directories: Vec::new(),
        };
        let mut strict = final_command(shell.as_os_str(), &[], &launch, false);
        if confine(&mut strict.command, &launch).is_err() {
            assert!(!available());
            return;
        }
        match process::run(strict, b"", LIMITS, &|| false) {
            Err(error) => assert!(!available() && refused_step(&error).is_some(), "{error}"),
            Ok(observation) => {
                assert!(available());
                let stderr = String::from_utf8_lossy(&observation.stderr);
                assert!(stderr.contains("fork: Operation not permitted"), "{stderr}");
                assert_eq!(observation.descendants_left, 0);
                assert_ne!(observation.ended, Ended::Exited(Some(0)));
            }
        }
    }

    /// F1: the working directory is entered inside the root, after the pivot.
    #[test]
    fn the_working_directory_is_entered_inside_the_root() {
        let stage = stage();
        let argv = [
            "-c".to_string(),
            "printf '%s\\n' \"$PWD\"; pwd -P".to_string(),
        ];
        let env = BTreeMap::new();
        let shell = shell();
        let launch = StrictLaunch {
            executable: &shell,
            argv: &argv,
            cwd: &stage.bound,
            env: &env,
            read_directories: vec![stage.bound.as_path()],
            link_directories: Vec::new(),
        };
        let mut strict = final_command(shell.as_os_str(), &[], &launch, false);
        if confine(&mut strict.command, &launch).is_err() {
            assert!(!available());
            return;
        }
        let Ok(observation) = process::run(strict, b"", LIMITS, &|| false) else {
            assert!(!available());
            return;
        };
        let expected = format!("{0}\n{0}\n", stage.bound.display());
        assert_eq!(
            String::from_utf8_lossy(&observation.stdout),
            expected,
            "{}",
            shown(&observation)
        );
    }
}
