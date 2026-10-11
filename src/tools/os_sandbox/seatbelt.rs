//! macOS Seatbelt backend (Issue #655, stage 1a).
//!
//! Applies a fixed write-confinement profile through `sandbox-exec`. The
//! profile is a constant string; the workspace, the run temp directory, and the
//! allowed caches travel only as `-D` parameters, so no location is embedded in
//! the profile text. Reads and network are left as they are (`allow default`
//! then `deny file-write*`), and the only re-allowed writes are the workspace,
//! the run temp directory, the known dependency caches, and device files.

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use super::{SandboxAvailability, SandboxPolicy};

const SANDBOX_EXEC: &str = "/usr/bin/sandbox-exec";
const PROBE_PROFILE: &str = "(version 1)(allow default)";
const PROBE_TIMEOUT: Duration = Duration::from_secs(10);
/// A placeholder path for an absent cache so every profile parameter is always
/// defined. It matches nothing on a real system.
const UNSET_PATH: &str = "/commandagent-sandbox-unset";

/// The fixed profile. `(allow default)` keeps reads and network unchanged;
/// `(deny file-write*)` then confines every write to the parameters below.
const PROFILE: &str = r#"
(version 1)
(allow default)
(deny file-write*)
(allow file-write* (subpath (param "WS")))
(allow file-write* (subpath (param "TMP")))
(allow file-write* (subpath (param "CACHE_CARGO_REGISTRY")))
(allow file-write* (subpath (param "CACHE_CARGO_GIT")))
(allow file-write* (subpath (param "CACHE_NPM")))
(allow file-write* (subpath (param "CACHE_PIP")))
(allow file-write* (subpath (param "CACHE_PLAYWRIGHT")))
(allow file-write* (literal (param "CACHE_CARGO_PACKAGE")))
(allow file-write* (regex #"^/dev/"))
"#;

/// Start `sandbox-exec` with a do-nothing profile. A nested sandbox reports
/// rc=71 (`Operation not permitted`), which is an honest "unavailable".
pub fn probe() -> SandboxAvailability {
    let mut command = Command::new(SANDBOX_EXEC);
    command
        .arg("-p")
        .arg(PROBE_PROFILE)
        .arg("true")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    match crate::bounded_process::run_with_timeout(&mut command, PROBE_TIMEOUT) {
        Ok(output) if output.success() => SandboxAvailability::Available,
        Ok(output) => SandboxAvailability::Unavailable(format!(
            "sandbox probe exited with {}",
            output
                .status
                .map(|status| status.to_string())
                .unwrap_or_else(|| "no status".to_string())
        )),
        Err(err) => {
            SandboxAvailability::Unavailable(format!("sandbox probe could not start: {err}"))
        }
    }
}

/// Rewrite `command` into `sandbox-exec -p <profile> -D ... <program> <args...>`.
///
/// The process's program, arguments, environment changes, and working directory
/// are transferred, and `TMPDIR` is set explicitly to the run temp directory.
/// Standard I/O is intentionally left unset: it does not survive the transfer,
/// so callers set it on the returned command.
pub fn wrap(command: Command, policy: &SandboxPolicy) -> Command {
    let mut wrapped = Command::new(SANDBOX_EXEC);
    wrapped.arg("-p").arg(PROFILE);
    define(&mut wrapped, "WS", Some(&policy.workspace_root));
    define(&mut wrapped, "TMP", Some(&policy.run_tmp_dir));
    define(
        &mut wrapped,
        "CACHE_CARGO_REGISTRY",
        policy.caches.cargo_registry.as_deref(),
    );
    define(
        &mut wrapped,
        "CACHE_CARGO_GIT",
        policy.caches.cargo_git.as_deref(),
    );
    define(&mut wrapped, "CACHE_NPM", policy.caches.npm.as_deref());
    define(&mut wrapped, "CACHE_PIP", policy.caches.pip.as_deref());
    define(
        &mut wrapped,
        "CACHE_PLAYWRIGHT",
        policy.caches.playwright.as_deref(),
    );
    define(
        &mut wrapped,
        "CACHE_CARGO_PACKAGE",
        policy.caches.cargo_package_cache.as_deref(),
    );

    wrapped.arg(command.get_program());
    for arg in command.get_args() {
        wrapped.arg(arg);
    }
    for (key, value) in command.get_envs() {
        match value {
            Some(value) => {
                wrapped.env(key, value);
            }
            None => {
                wrapped.env_remove(key);
            }
        }
    }
    if let Some(dir) = command.get_current_dir() {
        wrapped.current_dir(dir);
    }
    wrapped.env("TMPDIR", &policy.run_tmp_dir);
    wrapped
}

fn define(command: &mut Command, name: &str, path: Option<&Path>) {
    let value = path
        .map(canonical)
        .unwrap_or_else(|| UNSET_PATH.to_string());
    command.arg("-D").arg(format!("{name}={value}"));
}

fn canonical(path: &Path) -> String {
    path.canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .into_owned()
}
