//! Keep provider credentials out of this process's exec-time environment (Issue #974).
//!
//! On macOS, any process of the same user can read another's exec-time environment and argv
//! through `KERN_PROCARGS2`. That area is fixed by the exec: removing a variable later changes
//! nothing in it. So the host's first act, when a protected credential is in its environment,
//! is to exec itself without that credential and hand the value over a pipe.
//!
//! - Step 0, first in `main`: write the values to a pipe that takes them without blocking, then
//!   exec the same binary with the same argv, without those variables, with the pipe's number
//!   in [`BOOT_FD_ENV`].
//! - Step 1, the same pid after that exec: read the pipe within a bound, close it, and keep the
//!   values in a read-only store in this process. They are not put back into the environment,
//!   so no child can inherit them by accident.
//!
//! The marker is not proof of anything. It names an input from outside this process, judged
//! like one, and a marker beside a credential that is still in the environment is refused.
//!
//! A library caller and every other operating system never reach this: [`var`] is then the
//! plain environment read it always was. A failure stops the start. Nothing here continues
//! with a credential still in the exec-time environment.

use std::collections::BTreeMap;
use std::env::VarError;
use std::fmt;
use std::sync::OnceLock;

/// The provider keys a run always treats as credentials, selected or not.
pub const BUILT_IN_CREDENTIAL_NAMES: [&str; 4] = [
    "OPENAI_API_KEY",
    "GEMINI_API_KEY",
    "LM_STUDIO_API_TOKEN",
    "JEV_API_KEY",
];

/// The read-only store. It says how many values it holds and nothing about them.
struct Held(BTreeMap<String, String>);

impl fmt::Debug for Held {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Held")
            .field("count", &self.0.len())
            .finish_non_exhaustive()
    }
}

static HELD: OnceLock<Held> = OnceLock::new();

fn held(name: &str) -> Option<&'static str> {
    HELD.get()?.0.get(name).map(String::as_str)
}

/// A credential by name: what step 1 received, then the process environment. This is the only
/// way to read a received value by name. The caller uses it for its own request and must not
/// copy it to a child's environment or arguments.
pub fn var(name: &str) -> Result<String, VarError> {
    match held(name) {
        Some(value) => Ok(value.to_string()),
        None => std::env::var(name),
    }
}

/// The same for the GUI's delegation, the one place a received value goes to a child's
/// environment: that child's own start removes it again. With nothing received, this is the
/// plain environment read it was.
pub fn var_os(name: &str) -> Option<std::ffi::OsString> {
    match held(name) {
        Some(value) => Some(value.into()),
        None => std::env::var_os(name),
    }
}

/// Every received value, for what must recognize or scrub one. Empty when step 1 did not run.
pub(crate) fn held_values() -> Vec<&'static str> {
    HELD.get()
        .map(|held| held.0.values().map(String::as_str).collect())
        .unwrap_or_default()
}

/// Step 0 or 1 for the command-line binary. Call this first in `main`, while one thread runs.
pub fn bootstrap_cli() {
    #[cfg(target_os = "macos")]
    boot::run(cli_api_key_env);
}

/// The same for the GUI server, which has no provider arguments. Call this before its async
/// runtime starts.
pub fn bootstrap_gui() {
    #[cfg(target_os = "macos")]
    boot::run(|| None);
}

/// The compatible provider's key name, by the same resolution a run uses. A command line that
/// does not parse ends the process right after this with no tool started, so it adds no name.
#[cfg(target_os = "macos")]
fn cli_api_key_env() -> Option<String> {
    let parsed = crate::provider_cli::parse_from(std::env::args_os()).ok()?;
    let root = parsed
        .cli
        .cwd
        .clone()
        .or_else(|| std::env::current_dir().ok())
        .and_then(|root| root.canonicalize().ok());
    match parsed.provider_options.api_key_env {
        Some(name) => frame::is_name(name.trim()).then(|| name.trim().to_string()),
        None => crate::config::preset_api_key_env(&root?, parsed.cli.preset.as_deref()),
    }
}

/// The bytes on the pipe. Every length is checked against what is left, so nothing is read past
/// the end and no sum can overflow.
#[cfg(any(target_os = "macos", test))]
mod frame {
    use std::collections::BTreeMap;

    const MAGIC: &[u8] = b"CACRED1\n";
    /// Everything on the pipe: the header, the count, each length, each name, each value.
    pub(super) const MAX_BYTES: usize = 16 * 1024;

    pub(super) fn is_name(name: &str) -> bool {
        let mut characters = name.chars();
        characters
            .next()
            .is_some_and(|first| first == '_' || first.is_ascii_alphabetic())
            && characters.all(|character| character == '_' || character.is_ascii_alphanumeric())
    }

    pub(super) fn encode(pairs: &[(String, String)]) -> Option<Vec<u8>> {
        let mut bytes = MAGIC.to_vec();
        bytes.extend_from_slice(&u32::try_from(pairs.len()).ok()?.to_be_bytes());
        for (name, value) in pairs {
            for part in [name, value] {
                bytes.extend_from_slice(&u32::try_from(part.len()).ok()?.to_be_bytes());
                bytes.extend_from_slice(part.as_bytes());
            }
            if bytes.len() > MAX_BYTES {
                return None;
            }
        }
        Some(bytes)
    }

    fn take<'a>(bytes: &mut &'a [u8], length: usize) -> Option<&'a [u8]> {
        if bytes.len() < length {
            return None;
        }
        let (head, tail) = bytes.split_at(length);
        *bytes = tail;
        Some(head)
    }

    fn take_length(bytes: &mut &[u8]) -> Option<usize> {
        let raw: [u8; 4] = take(bytes, 4)?.try_into().ok()?;
        usize::try_from(u32::from_be_bytes(raw)).ok()
    }

    fn take_text(bytes: &mut &[u8]) -> Option<String> {
        let length = take_length(bytes)?;
        let text = std::str::from_utf8(take(bytes, length)?).ok()?;
        (!text.is_empty() && !text.contains('\0')).then(|| text.to_string())
    }

    /// `None` for anything but exactly what [`encode`] writes, within the bound, with one value
    /// for each well-formed name.
    pub(super) fn decode(mut bytes: &[u8]) -> Option<BTreeMap<String, String>> {
        if bytes.len() > MAX_BYTES || take(&mut bytes, MAGIC.len())? != MAGIC {
            return None;
        }
        let count = take_length(&mut bytes)?;
        let mut pairs = BTreeMap::new();
        for _ in 0..count {
            let name = take_text(&mut bytes)?;
            let value = take_text(&mut bytes)?;
            if !is_name(&name) || pairs.insert(name, value).is_some() {
                return None;
            }
        }
        bytes.is_empty().then_some(pairs)
    }
}

#[cfg(target_os = "macos")]
mod boot {
    use std::collections::BTreeSet;
    use std::ffi::{OsStr, OsString};
    use std::io::Write as _;
    use std::os::unix::ffi::OsStrExt as _;
    use std::os::unix::process::CommandExt as _;

    use super::{Held, frame};

    /// Names an input from outside this process. It does not match the credential predicate.
    const BOOT_FD_ENV: &str = "COMMANDAGENT_BOOT_FD";

    /// A fixed reason, and never a name's value: this ends the start.
    fn stop(reason: &str) -> ! {
        let _ = writeln!(
            std::io::stderr(),
            "error: the startup credential handoff failed: {reason}"
        );
        std::process::exit(1)
    }

    pub(super) fn run(extra_name: fn() -> Option<String>) {
        match std::env::var_os(BOOT_FD_ENV) {
            Some(marker) => receive(&marker, extra_name),
            None => hand_off(extra_name),
        }
    }

    fn protected_names(extra_name: fn() -> Option<String>) -> BTreeSet<String> {
        super::BUILT_IN_CREDENTIAL_NAMES
            .iter()
            .map(ToString::to_string)
            .chain(extra_name())
            .collect()
    }

    fn present(name: &str) -> Option<OsString> {
        std::env::var_os(name).filter(|value| !value.is_empty())
    }

    /// Step 0. A value that is not text was never readable as a credential, so it stays
    /// unreadable: it leaves the environment and does not travel.
    fn hand_off(extra_name: fn() -> Option<String>) {
        let names = protected_names(extra_name);
        let found = names
            .iter()
            .filter_map(|name| Some((name.clone(), present(name)?)))
            .collect::<Vec<_>>();
        if found.is_empty() {
            return;
        }
        let pairs = found
            .into_iter()
            .filter_map(|(name, value)| Some((name, value.into_string().ok()?)))
            .collect::<Vec<_>>();
        let Some(bytes) = frame::encode(&pairs) else {
            stop("the credentials exceed the bound");
        };
        let reader = write_to_pipe(&bytes);
        exec_self(&names, reader);
    }

    /// The reader, inheritable, at 3 or above. Nothing reads until after the exec, so a write
    /// that would wait can never finish: the writer does not block, and any shortfall stops.
    fn write_to_pipe(bytes: &[u8]) -> i32 {
        let mut ends = [-1i32; 2];
        // SAFETY: `ends` is writable for the two descriptors `pipe` returns.
        if unsafe { libc::pipe(ends.as_mut_ptr()) } != 0 {
            stop("a pipe could not be made");
        }
        let [reader, writer] = ends;
        set_flag(writer, libc::F_SETFD, libc::FD_CLOEXEC);
        set_flag(writer, libc::F_SETFL, libc::O_NONBLOCK);
        let mut written = 0;
        while written < bytes.len() {
            let pending = &bytes[written..];
            // SAFETY: `pending` is readable for this many bytes, and `writer` is open.
            let answer = unsafe { libc::write(writer, pending.as_ptr().cast(), pending.len()) };
            match usize::try_from(answer) {
                Ok(count) if count > 0 => written += count,
                _ if std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted => {}
                _ => stop("the pipe did not take the whole handoff"),
            }
        }
        // SAFETY: this closes the writer this function made, once.
        unsafe { libc::close(writer) };
        if reader > 2 {
            return reader;
        }
        // A caller that closed a standard stream gave the pipe its number. Those are not used
        // for this, so the reader moves up.
        // SAFETY: `reader` is open, and this makes a new descriptor at 3 or above.
        let moved = unsafe { libc::fcntl(reader, libc::F_DUPFD, 3) };
        if moved < 0 {
            stop("the pipe could not be moved");
        }
        // SAFETY: this closes the old number of the reader, once.
        unsafe { libc::close(reader) };
        moved
    }

    fn set_flag(descriptor: i32, command: i32, flag: i32) {
        // SAFETY: this sets one flag on a descriptor, and takes no pointer.
        if unsafe { libc::fcntl(descriptor, command, flag) } != 0 {
            stop("a pipe flag could not be set");
        }
    }

    /// The same binary, argv, working directory, and streams, and the same pid. This returns
    /// only by not returning.
    fn exec_self(names: &BTreeSet<String>, reader: i32) -> ! {
        let Ok(executable) = std::env::current_exe() else {
            stop("this executable could not be found");
        };
        let mut arguments = std::env::args_os();
        let first = arguments.next();
        let mut command = std::process::Command::new(executable);
        if let Some(first) = first {
            command.arg0(first);
        }
        command.args(arguments).env_clear();
        for (name, value) in std::env::vars_os() {
            if !names.iter().any(|protected| OsStr::new(protected) == name) {
                command.env(name, value);
            }
        }
        command.env(BOOT_FD_ENV, reader.to_string());
        set_flag(reader, libc::F_SETFD, 0);
        let _failure = command.exec();
        stop("this executable could not be started again")
    }

    /// `3` or above, in plain digits with no sign or leading zero.
    fn parse_descriptor(marker: &OsStr) -> Option<i32> {
        let text = std::str::from_utf8(marker.as_bytes()).ok()?;
        if text.is_empty()
            || text.starts_with('0')
            || !text.bytes().all(|byte| byte.is_ascii_digit())
        {
            return None;
        }
        text.parse().ok().filter(|descriptor| *descriptor > 2)
    }

    /// Step 1. It holds the values before it judges their names, because the names can depend
    /// on a config that refers to them, and every refusal ends the process right there.
    fn receive(marker: &OsStr, extra_name: fn() -> Option<String>) {
        let descriptor = parse_descriptor(marker);
        // SAFETY: this is the first act of `main`, while one thread runs.
        unsafe { std::env::remove_var(BOOT_FD_ENV) };
        let Some(descriptor) = descriptor else {
            stop("the handoff descriptor is not valid");
        };
        let received = read_pipe(descriptor);
        let Some(pairs) = frame::decode(&received) else {
            stop("the handoff is not well formed");
        };
        let keys = pairs.keys().cloned().collect::<Vec<_>>();
        if super::HELD.set(Held(pairs)).is_err() {
            stop("the handoff arrived twice");
        }
        let names = protected_names(extra_name);
        if !keys.iter().all(|key| names.contains(key)) {
            stop("the handoff holds a name that is not a credential of this run");
        }
        if names.iter().any(|name| present(name).is_some()) {
            stop("a credential is still in the environment");
        }
    }

    /// One bound, one pass: the writer finished before the exec, so what would have to wait is
    /// not the handoff.
    fn read_pipe(descriptor: i32) -> Vec<u8> {
        // SAFETY: an all-zero `stat` is a valid value of this plain struct.
        let mut status: libc::stat = unsafe { std::mem::zeroed() };
        // SAFETY: `status` is writable, and an unused number only returns an error.
        if unsafe { libc::fstat(descriptor, &raw mut status) } != 0 {
            stop("the handoff descriptor is not open");
        }
        set_flag(descriptor, libc::F_SETFD, libc::FD_CLOEXEC);
        let result = read_bounded(descriptor, status.st_mode);
        // SAFETY: this closes the descriptor named for this handoff, once, on every way out.
        unsafe { libc::close(descriptor) };
        result.unwrap_or_else(|reason| stop(reason))
    }

    fn read_bounded(descriptor: i32, mode: libc::mode_t) -> Result<Vec<u8>, &'static str> {
        if mode & libc::S_IFMT != libc::S_IFIFO {
            return Err("the handoff descriptor is not a pipe");
        }
        // SAFETY: this sets one flag on an open descriptor, and takes no pointer.
        if unsafe { libc::fcntl(descriptor, libc::F_SETFL, libc::O_NONBLOCK) } != 0 {
            return Err("the handoff descriptor cannot be read");
        }
        let mut received = vec![0u8; frame::MAX_BYTES + 1];
        let mut filled = 0;
        loop {
            let room = &mut received[filled..];
            if room.is_empty() {
                return Err("the handoff exceeds the bound");
            }
            // SAFETY: `room` is writable for this many bytes, and `descriptor` is open.
            let answer = unsafe { libc::read(descriptor, room.as_mut_ptr().cast(), room.len()) };
            match usize::try_from(answer) {
                Ok(0) => {
                    received.truncate(filled);
                    return Ok(received);
                }
                Ok(count) => filled += count,
                _ if std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted => {}
                _ => return Err("the handoff did not end"),
            }
        }
    }

    #[cfg(test)]
    pub(super) fn parse_descriptor_for_test(marker: &str) -> Option<i32> {
        parse_descriptor(OsStr::new(marker))
    }

    #[cfg(test)]
    pub(super) const BOOT_FD_ENV_FOR_TEST: &str = BOOT_FD_ENV;
}

/// For a test that runs in a process of its own: fills the store once, as step 1 would.
#[cfg(test)]
pub(crate) fn hold_for_test(pairs: BTreeMap<String, String>) {
    assert!(HELD.set(Held(pairs)).is_ok(), "the store is filled once");
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{Held, frame};

    const DUMMY: &str = "fake-startup-credential-974-not-a-real-key";
    const OWN_PROCESS: &str = "G3_STARTUP_CREDENTIAL_TEST_OWN_PROCESS";

    /// The store is filled once per process and the environment is the whole process's, so this
    /// runs in the same test binary started again for this test alone.
    fn in_own_process(test: &str, scenario: fn()) {
        if std::env::var_os(OWN_PROCESS).is_some() {
            scenario();
            return;
        }
        let (_, module) = module_path!().split_once("::").unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &format!("{module}::{test}"), "--nocapture"])
            .env_clear()
            .env(OWN_PROCESS, "1")
            .env("TMPDIR", std::env::temp_dir())
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success() && stdout.contains("1 passed"),
            "{stdout}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    const NAMES: [&str; 5] = [
        "OPENAI_API_KEY",
        "GEMINI_API_KEY",
        "LM_STUDIO_API_TOKEN",
        "JEV_API_KEY",
        "DUMMY_GATEWAY_NAME",
    ];

    fn value_of(name: &str) -> String {
        format!("{DUMMY}-{name}")
    }

    /// What each reader resolves now. Values are compared and never shown.
    fn resolved(root: &std::path::Path) -> Vec<(String, Option<String>)> {
        use crate::minimal_loop::request_pattern::JevClassifier;
        let mut rows = Vec::new();
        for name in NAMES {
            rows.push((
                format!("load_api_key {name}"),
                crate::config::load_api_key(root, name).ok(),
            ));
            rows.push((
                format!("load_process_api_key {name}"),
                crate::config::load_process_api_key(name).ok(),
            ));
            rows.push((
                format!("expansion {name}"),
                crate::config::expand_env_references(&format!("${{{name}}}")).ok(),
            ));
            rows.push((
                format!("doctor {name}"),
                crate::doctor::process_credential(name),
            ));
        }
        rows.push((
            "lm studio".to_string(),
            crate::providers::lm_studio::process_api_token(),
        ));
        rows.push((
            "jev".to_string(),
            JevClassifier::from_env().map(|jev| jev.api_key_for_test().to_string()),
        ));
        rows
    }

    /// PM condition 2. Every reader, through the store and through the environment.
    #[test]
    fn each_reader_resolves_a_received_value_as_it_resolved_the_environment() {
        in_own_process(
            "each_reader_resolves_a_received_value_as_it_resolved_the_environment",
            || {
                let root = tempfile::tempdir().unwrap();
                for name in NAMES {
                    // SAFETY: this process runs this test alone, on one thread at this point.
                    unsafe { std::env::set_var(name, value_of(name)) };
                }
                let through_environment = resolved(root.path());
                for name in NAMES {
                    // SAFETY: as above.
                    unsafe { std::env::remove_var(name) };
                }
                assert!(
                    resolved(root.path())
                        .iter()
                        .all(|(_, value)| value.is_none())
                );
                super::hold_for_test(
                    NAMES
                        .iter()
                        .map(|name| (name.to_string(), value_of(name)))
                        .collect(),
                );
                let through_store = resolved(root.path());
                for (from_store, from_environment) in through_store.iter().zip(&through_environment)
                {
                    assert!(from_store.1.is_some(), "{} is unset", from_store.0);
                    assert!(from_store == from_environment, "{} differs", from_store.0);
                }
                let catalog = crate::config::build_secret_sources(root.path(), None).unwrap();
                for name in NAMES {
                    let value = value_of(name);
                    assert!(
                        catalog.values().contains(&value),
                        "{name} is not in the catalog"
                    );
                }
                assert_eq!(super::held_values().len(), NAMES.len());
            },
        );
    }

    /// The order: what was received, then the environment, and each reader's own dotenv rule.
    #[test]
    fn a_received_value_comes_first_and_a_reader_keeps_its_dotenv_rule() {
        in_own_process(
            "a_received_value_comes_first_and_a_reader_keeps_its_dotenv_rule",
            || {
                let root = tempfile::tempdir().unwrap();
                std::fs::write(
                    root.path().join(".env"),
                    format!("DOTENV_ONLY_NAME={DUMMY}-dotenv\nOPENAI_API_KEY={DUMMY}-dotenv\n"),
                )
                .unwrap();
                // SAFETY: this process runs this test alone, on one thread at this point.
                unsafe { std::env::set_var("ENVIRONMENT_ONLY_NAME", DUMMY) };
                // SAFETY: as above.
                unsafe { std::env::set_var("JEV_API_KEY", format!("{DUMMY}-environment")) };
                super::hold_for_test(BTreeMap::from([
                    ("OPENAI_API_KEY".to_string(), value_of("OPENAI_API_KEY")),
                    ("JEV_API_KEY".to_string(), value_of("JEV_API_KEY")),
                ]));
                let load = |name| crate::config::load_api_key(root.path(), name).ok();
                let process = |name| crate::config::load_process_api_key(name).ok();
                assert!(load("OPENAI_API_KEY") == Some(value_of("OPENAI_API_KEY")));
                assert!(process("JEV_API_KEY") == Some(value_of("JEV_API_KEY")));
                assert!(process("ENVIRONMENT_ONLY_NAME") == Some(DUMMY.to_string()));
                assert!(load("DOTENV_ONLY_NAME") == Some(format!("{DUMMY}-dotenv")));
                assert!(process("DOTENV_ONLY_NAME").is_none());
            },
        );
    }

    fn pairs(rows: &[(&str, &str)]) -> Vec<(String, String)> {
        rows.iter()
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect()
    }

    fn encoded(rows: &[(&str, &str)]) -> Vec<u8> {
        frame::encode(&pairs(rows)).unwrap()
    }

    #[test]
    fn a_frame_round_trips_and_nothing_else_decodes() {
        let good = encoded(&[("OPENAI_API_KEY", DUMMY), ("AUTH", "another-dummy-value")]);
        let decoded = frame::decode(&good).unwrap();
        assert_eq!(decoded["OPENAI_API_KEY"], DUMMY);
        assert_eq!(decoded["AUTH"], "another-dummy-value");
        assert!(frame::decode(&encoded(&[])).unwrap().is_empty());

        let mut trailing = good.clone();
        trailing.push(0);
        let mut wrong_magic = good.clone();
        wrong_magic[0] ^= 1;
        let mut short_count = encoded(&[("A_NAME", "value")]);
        short_count[11] = 2;
        let mut long_count = encoded(&[]);
        long_count[8..12].copy_from_slice(&u32::MAX.to_be_bytes());
        let mut long_length = encoded(&[("A_NAME", "value")]);
        long_length[12..16].copy_from_slice(&u32::MAX.to_be_bytes());
        for (label, bad) in [
            ("empty", Vec::new()),
            ("truncated", good[..good.len() - 1].to_vec()),
            ("trailing", trailing),
            ("magic", wrong_magic),
            ("count", short_count),
            ("huge count", long_count),
            ("huge length", long_length),
            ("empty name", encoded(&[("", "value")])),
            ("bad name", encoded(&[("A-NAME", "value")])),
            ("digit first", encoded(&[("1NAME", "value")])),
            ("empty value", encoded(&[("A_NAME", "")])),
            ("NUL value", encoded(&[("A_NAME", "va\0lue")])),
            (
                "repeated",
                encoded(&[("A_NAME", "value"), ("A_NAME", "other")]),
            ),
        ] {
            assert_eq!(frame::decode(&bad), None, "{label}");
        }
        let mut not_text = encoded(&[("A_NAME", "value")]);
        let last = not_text.len() - 1;
        not_text[last] = 0xff;
        assert_eq!(frame::decode(&not_text), None);
    }

    #[test]
    fn the_bound_covers_the_whole_frame() {
        let value = "x".repeat(frame::MAX_BYTES);
        assert_eq!(frame::encode(&pairs(&[("A_NAME", &value)])), None);
        let fits = "x".repeat(frame::MAX_BYTES - 8 - 4 - 4 - 6 - 4);
        let at_bound = encoded(&[("A_NAME", &fits)]);
        assert_eq!(at_bound.len(), frame::MAX_BYTES);
        assert!(frame::decode(&at_bound).is_some());
        let mut over = at_bound;
        over.push(b'x');
        assert_eq!(frame::decode(&over), None);
    }

    #[test]
    fn the_store_never_shows_a_value() {
        let held = Held(BTreeMap::from([(
            "OPENAI_API_KEY".to_string(),
            DUMMY.to_string(),
        )]));
        let shown = format!("{held:?} {held:#?}");
        assert!(!shown.contains(DUMMY), "{shown}");
        assert!(!shown.contains("OPENAI_API_KEY"), "{shown}");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn the_marker_is_a_plain_number_above_the_standard_streams_and_no_credential_name() {
        use super::boot::{BOOT_FD_ENV_FOR_TEST, parse_descriptor_for_test as parse};
        assert!(!crate::sensitive_data::is_credential_name(
            BOOT_FD_ENV_FOR_TEST
        ));
        assert_eq!(parse("3"), Some(3));
        assert_eq!(parse("1024"), Some(1024));
        for bad in [
            "",
            "0",
            "1",
            "2",
            "-3",
            "+3",
            "03",
            " 3",
            "3 ",
            "3x",
            "x",
            "99999999999",
        ] {
            assert_eq!(parse(bad), None, "{bad:?}");
        }
    }
}
