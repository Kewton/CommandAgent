//! Issue #673: a machine-checked inventory of the dangerous commands in this
//! repository, paired with a grep of the same tree.
//!
//! The inventory is detection, not isolation. It fails when a signal call or a
//! recursive delete appears that is not declared here (and therefore not in
//! `docs/dev/dangerous-commands.md`), or when a declared count changes. That
//! is what makes an added `kill` visible in review instead of silent.
//!
//! Limits (see the doc for the full list): this matches direct API names, so it
//! misses aliases, wrappers, and code inside `sh -c` / Python. It covers the
//! two categories the Issue requires in full — signal (`libc::kill`,
//! `libc::killpg`, a `nix` signal wrapper, and the external `kill`/`killall`/
//! `pkill` commands) and recursive delete (`remove_dir_all`) — and leaves
//! single-file delete, truncate/rename, permission changes, terminal ioctls,
//! and `sh -c` out of the automated match (documented as out of scope).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// The token that marks a signal site. `libc::kill(` is matched literally, so
/// `libc::killpg(` is counted with its own token.
const SIGNAL_TOKENS: &[&str] = &[
    "libc::kill(",
    "libc::killpg(",
    "nix::sys::signal::kill(",
    "Command::new(\"kill\")",
    "Command::new(\"killall\")",
    "Command::new(\"pkill\")",
];

/// The token that marks a recursive delete.
const RECURSIVE_DELETE_TOKENS: &[&str] = &["remove_dir_all("];

/// Every signal / recursive-delete site declared in the inventory, with the
/// number of token occurrences. `(path, token, count)`.
///
/// This file is excluded from the scan, so its own token literals do not count.
const DECLARED: &[(&str, &str, usize)] = &[
    // Signal: negative process-group targets and one positive pid.
    // `src/bounded_process.rs` carries 4 in the wrappers plus 1 in the
    // `dangerous_sentinel_` test's `group_exists` probe.
    ("src/bounded_process.rs", "libc::kill(", 5),
    ("src/bin/gui_server/trial_process.rs", "libc::kill(", 2),
    ("src/planner/runner/acceptance.rs", "libc::kill(", 2),
    ("src/preflight.rs", "libc::kill(", 1),
    (
        "src/planner/runner/tests/support/process.rs",
        "libc::kill(",
        1,
    ),
    ("tests/gui_server.rs", "libc::kill(", 1),
    ("tests/headless_summary.rs", "libc::kill(", 1),
    // `tests/gui_read_only_guard.rs` holds these as string literals only.
    ("tests/gui_read_only_guard.rs", "libc::kill(", 3),
    // Recursive delete.
    ("src/model_probe.rs", "remove_dir_all(", 1),
    ("src/bin/gui_server/session_paths.rs", "remove_dir_all(", 1),
    ("src/tools/dir_fd.rs", "remove_dir_all(", 2),
    ("src/tools/edit.rs", "remove_dir_all(", 1),
    ("src/tools/os_sandbox.rs", "remove_dir_all(", 1),
    ("src/planner/pack/supply.rs", "remove_dir_all(", 2),
    ("src/tools/write.rs", "remove_dir_all(", 1),
    ("tests/gui_server.rs", "remove_dir_all(", 4),
    ("tests/issue505_write_toctou.rs", "remove_dir_all(", 3),
];

fn all_tokens() -> impl Iterator<Item = &'static str> {
    SIGNAL_TOKENS.iter().chain(RECURSIVE_DELETE_TOKENS).copied()
}

/// Report every mismatch between the declared inventory and `corpus`:
/// undeclared sites, changed counts, and declared sites that disappeared.
fn inventory_violations(corpus: &[(String, String)]) -> Vec<String> {
    let mut found: BTreeMap<(String, String), usize> = BTreeMap::new();
    for (path, text) in corpus {
        for token in all_tokens() {
            let count = text.matches(token).count();
            if count > 0 {
                found.insert((path.clone(), token.to_string()), count);
            }
        }
    }

    let mut violations = Vec::new();
    for ((path, token), count) in &found {
        match DECLARED.iter().find(|(declared_path, declared_token, _)| {
            declared_path == path && declared_token == token
        }) {
            None => violations.push(format!("{path}: undeclared `{token}` site ({count})")),
            Some((_, _, declared_count)) if declared_count != count => violations.push(format!(
                "{path}: `{token}` count {count} does not match declared {declared_count}"
            )),
            Some(_) => {}
        }
    }
    for (path, token, declared_count) in DECLARED {
        if found.get(&(path.to_string(), token.to_string())) != Some(declared_count) {
            violations.push(format!(
                "{path}: declared `{token}` ({declared_count}) is no longer present"
            ));
        }
    }
    violations
}

#[test]
fn dangerous_command_inventory_matches_the_tree() {
    let corpus = scan_rust_tree();
    let violations = inventory_violations(&corpus);
    assert!(
        violations.is_empty(),
        "dangerous-command inventory is out of date:\n{}",
        violations.join("\n")
    );
}

/// The two-point measurement for the detector itself: an unlisted `kill` is a
/// violation. The synthetic site is never compiled or executed — it is a
/// string handed to the audit.
#[test]
fn dangerous_command_inventory_rejects_an_unlisted_signal() {
    let corpus = vec![(
        "src/new_signal.rs".to_string(),
        "fn f(pid: i32) { unsafe { libc::kill(-pid, libc::SIGTERM); } }".to_string(),
    )];
    let violations = inventory_violations(&corpus);
    assert!(
        violations
            .iter()
            .any(|violation| violation.contains("src/new_signal.rs")
                && violation.contains("libc::kill(")),
        "{violations:?}"
    );
}

/// The two-point measurement for the recursive-delete detector.
#[test]
fn dangerous_command_inventory_rejects_an_unlisted_recursive_delete() {
    let corpus = vec![(
        "src/new_delete.rs".to_string(),
        "fn f(p: &std::path::Path) { let _ = std::fs::remove_dir_all(p); }".to_string(),
    )];
    let violations = inventory_violations(&corpus);
    assert!(
        violations
            .iter()
            .any(|violation| violation.contains("src/new_delete.rs")
                && violation.contains("remove_dir_all(")),
        "{violations:?}"
    );
}

/// A change to a declared file (here a second signal in a declared file) is
/// caught by the count, not only by the file list.
#[test]
fn dangerous_command_inventory_rejects_an_extra_signal_in_a_declared_file() {
    let corpus = vec![(
        "src/preflight.rs".to_string(),
        "fn a() { unsafe { libc::kill(1, 1); } } fn b() { unsafe { libc::kill(2, 2); } }"
            .to_string(),
    )];
    let violations = inventory_violations(&corpus);
    assert!(
        violations
            .iter()
            .any(|violation| violation.contains("src/preflight.rs")
                && violation.contains("does not match declared")),
        "{violations:?}"
    );
}

fn scan_rust_tree() -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut corpus = Vec::new();
    for dir in ["src", "tests"] {
        for path in rust_files(&root.join(dir)) {
            push_relative(root, &path, &mut corpus);
        }
    }
    let build_rs = root.join("build.rs");
    push_relative(root, &build_rs, &mut corpus);
    // This file declares the tokens as string literals; it is not a site.
    corpus.retain(|(path, _)| path != "tests/issue673_dangerous_command_inventory.rs");
    corpus
}

fn push_relative(root: &Path, path: &Path, corpus: &mut Vec<(String, String)>) {
    let relative = path
        .strip_prefix(root)
        .expect("relative source path")
        .to_string_lossy()
        .replace('\\', "/");
    let text = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    corpus.push((relative, text));
}

fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    collect_rust_files(dir, &mut out);
    out
}

fn collect_rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap_or_else(|error| panic!("failed to read {dir:?}: {error}"))
    {
        let entry = entry.expect("directory entry");
        let path = entry.path();
        if path.is_dir() {
            collect_rust_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}
