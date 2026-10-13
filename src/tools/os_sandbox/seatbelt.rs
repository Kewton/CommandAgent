//! macOS Seatbelt backend (Issue #655, stage 1a; read isolation in Issue #657,
//! stage 2).
//!
//! Applies a write-confinement profile and, since #657, a read-confinement
//! layer through `sandbox-exec`. The profile is assembled from a constant; every
//! location travels only as a `-D` parameter, so no path is embedded in the
//! profile text. `(allow default)` keeps everything else open, then
//! `(deny file-write*)` confines writes to the workspace, the run temp
//! directory, the known dependency caches, and device files, and `(deny
//! file-read*)` denies the home secret locations and the workspace credential
//! names. Read isolation is a deny list (decision 4 of #502): everything that is
//! not a credential stays readable, so a toolchain, git, and the system files
//! keep working.

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use super::{SandboxAvailability, SandboxPolicy, secret_locations};

const SANDBOX_EXEC: &str = "/usr/bin/sandbox-exec";
const PROBE_PROFILE: &str = "(version 1)(allow default)";
const PROBE_TIMEOUT: Duration = Duration::from_secs(10);
/// A placeholder path for an absent cache so every profile parameter is always
/// defined. It matches nothing on a real system.
const UNSET_PATH: &str = "/commandagent-sandbox-unset";

/// The fixed write profile. `(allow default)` keeps everything else open;
/// `(deny file-write*)` then confines every write to the parameters below. The
/// read-deny rules are appended per run by [`profile`], because the number of
/// home secret locations is fixed but their names travel as parameters.
const WRITE_PROFILE: &str = r#"
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

/// The profile for a run, with `deny_count` home secret locations and the
/// workspace credential rules. Seatbelt applies the last matching rule, so the
/// template re-allow comes after the workspace credential deny and keeps the
/// three strict dotenv templates readable.
fn profile(deny_count: usize) -> String {
    let mut profile = String::from(WRITE_PROFILE);
    for index in 0..deny_count {
        profile.push_str(&format!(
            "(deny file-read* (subpath (param \"RDENY_HOME_{index}\")))\n"
        ));
    }
    profile.push_str("(deny file-read* (regex (param \"RDENY_WS\")))\n");
    profile.push_str("(allow file-read* (regex (param \"RALLOW_WS_TEMPLATE\")))\n");
    // Re-deny a template below a secret-named directory, so the re-allow above
    // cannot open `.ssh/.env.example`.
    profile.push_str("(deny file-read* (regex (param \"RDENY_WS_TEMPLATE\")))\n");
    profile
}

/// Start `sandbox-exec` with a do-nothing profile. A nested sandbox reports
/// rc=71 (`Operation not permitted`), which is an honest "unavailable".
pub fn probe() -> SandboxAvailability {
    match run_probe() {
        Ok(output) => classify_probe_exit(
            output.success(),
            output.status.map(|status| status.to_string()),
        ),
        Err(err) => {
            SandboxAvailability::Unavailable(format!("sandbox probe could not start: {err}"))
        }
    }
}

fn run_probe() -> std::io::Result<crate::bounded_process::BoundedProcessOutput> {
    let mut command = Command::new(SANDBOX_EXEC);
    command
        .arg("-p")
        .arg(PROBE_PROFILE)
        .arg("true")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    crate::bounded_process::run_with_timeout(&mut command, PROBE_TIMEOUT)
}

/// Only a zero exit status means the backend can start. rc=71 (a nested
/// sandbox) and every other non-zero status are unavailable, with the status in
/// the reason; a missing status is still unavailable.
fn classify_probe_exit(success: bool, status: Option<String>) -> SandboxAvailability {
    if success {
        SandboxAvailability::Available
    } else {
        SandboxAvailability::Unavailable(format!(
            "sandbox probe exited with {}",
            status.unwrap_or_else(|| "no status".to_string())
        ))
    }
}

/// Rewrite `command` into `sandbox-exec -p <profile> -D ... <program> <args...>`.
///
/// The process's program, arguments, environment changes, and working directory
/// are transferred, and `TMPDIR` is set explicitly to the run temp directory.
/// Standard I/O is intentionally left unset: it does not survive the transfer,
/// so callers set it on the returned command.
pub fn wrap(command: Command, policy: &SandboxPolicy) -> Command {
    let home_secrets = secret_locations::home_secret_locations(&policy.home);
    let mut wrapped = Command::new(SANDBOX_EXEC);
    wrapped.arg("-p").arg(profile(home_secrets.len()));
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

    for (index, location) in home_secrets.iter().enumerate() {
        define(&mut wrapped, &format!("RDENY_HOME_{index}"), Some(location));
    }
    define_text(
        &mut wrapped,
        "RDENY_WS",
        &secret_locations::workspace_credential_regex(&policy.workspace_root),
    );
    define_text(
        &mut wrapped,
        "RALLOW_WS_TEMPLATE",
        &secret_locations::workspace_template_regex(&policy.workspace_root),
    );
    define_text(
        &mut wrapped,
        "RDENY_WS_TEMPLATE",
        &secret_locations::workspace_template_under_secret_regex(&policy.workspace_root),
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

/// Define a parameter whose value is already a literal string (a generated
/// regex), so it is not canonicalized as a path.
fn define_text(command: &mut Command, name: &str, value: &str) {
    command.arg("-D").arg(format!("{name}={value}"));
}

fn canonical(path: &Path) -> String {
    path.canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_denies_each_home_location_and_re_allows_templates_last() {
        let built = profile(3);
        assert!(built.contains("(deny file-write*)"), "{built}");
        for index in 0..3 {
            assert!(
                built.contains(&format!(
                    "(deny file-read* (subpath (param \"RDENY_HOME_{index}\")))"
                )),
                "{built}"
            );
        }
        assert!(!built.contains("RDENY_HOME_3"), "{built}");
        let deny_ws = built
            .find("(deny file-read* (regex (param \"RDENY_WS\")))")
            .expect("workspace deny rule");
        let allow_template = built
            .find("(allow file-read* (regex (param \"RALLOW_WS_TEMPLATE\")))")
            .expect("template allow rule");
        let deny_template = built
            .find("(deny file-read* (regex (param \"RDENY_WS_TEMPLATE\")))")
            .expect("template re-deny rule");
        assert!(
            deny_ws < allow_template && allow_template < deny_template,
            "the template re-allow must sit between the deny and the re-deny: {built}"
        );
    }

    #[test]
    fn probe_classifies_a_zero_exit_as_available() {
        assert_eq!(
            classify_probe_exit(true, Some("exit status: 0".to_string())),
            SandboxAvailability::Available
        );
    }

    #[test]
    fn probe_classifies_a_nested_exit_as_unavailable_with_the_status() {
        let result = classify_probe_exit(false, Some("exit status: 71".to_string()));
        match result {
            SandboxAvailability::Unavailable(reason) => {
                assert!(reason.contains("71"), "{reason}");
            }
            SandboxAvailability::Available => panic!("rc=71 must be unavailable"),
        }
    }

    #[test]
    fn probe_classifies_a_missing_status_as_unavailable() {
        assert!(matches!(
            classify_probe_exit(false, None),
            SandboxAvailability::Unavailable(_)
        ));
    }
}
