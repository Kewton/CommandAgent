//! Issue #673: a machine-checked inventory of the dangerous commands in this
//! repository, paired with a grep of the same tree.
//!
//! The inventory is detection, not isolation. It fails when a signal call or a
//! recursive delete appears that is not declared here (and therefore not in
//! `docs/dev/dangerous-commands.md`), or when a declared count changes. That
//! is what makes an added `kill` visible in review instead of silent.
//!
//! Limits (see the doc for the full list): this matches direct API names and
//! shell commands in tracked `.rs`, `.sh`, and `.py` files, so it misses
//! aliases, wrappers, code inside `sh -c` / a Python string, and other
//! languages (for example the Node `child.kill()` in `scripts/*.mjs`). It
//! covers in full the categories the Issue requires — signal (Rust
//! `libc::kill`/`libc::killpg`, a `nix` signal wrapper, the external
//! `kill`/`pkill`/`killall`, Python `os.kill(`/`os.killpg(`) and recursive
//! delete (`remove_dir_all`, `rm -rf`/`rm -r `, `shutil.rmtree(`) — and leaves
//! single-file delete, truncate/rename, permission changes, terminal ioctls,
//! and interpreter string bodies out of the automated match (documented).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Tokens that mark a dangerous site in a Rust source file.
const RUST_TOKENS: &[&str] = &[
    "libc::kill(",
    "libc::killpg(",
    "nix::sys::signal::kill(",
    "Command::new(\"kill\")",
    "Command::new(\"killall\")",
    "Command::new(\"pkill\")",
    "remove_dir_all(",
];

/// Tokens that mark a dangerous site in a shell script: the external `kill`,
/// `pkill`, `killall`, and the recursive `rm`.
const SHELL_TOKENS: &[&str] = &["kill ", "pkill", "killall", "rm -rf", "rm -r "];

/// Tokens that mark a dangerous site in a Python file: `os.kill`, `os.killpg`,
/// and the recursive `shutil.rmtree`.
const PYTHON_TOKENS: &[&str] = &["os.kill(", "os.killpg(", "shutil.rmtree("];

fn tokens_for(path: &str) -> &'static [&'static str] {
    if path.ends_with(".rs") {
        RUST_TOKENS
    } else if path.ends_with(".sh") {
        SHELL_TOKENS
    } else if path.ends_with(".py") {
        PYTHON_TOKENS
    } else {
        &[]
    }
}

/// Every signal / recursive-delete site declared in the inventory, with the
/// number of token occurrences. `(path, token, count)`.
///
/// This file is excluded from the scan, so its own token literals do not count.
const DECLARED: &[(&str, &str, usize)] = &[
    // Rust: negative process-group targets, one positive pid, probes.
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
    // Rust recursive delete.
    ("src/model_probe.rs", "remove_dir_all(", 1),
    ("src/bin/gui_server/session_paths.rs", "remove_dir_all(", 1),
    ("src/tools/dir_fd.rs", "remove_dir_all(", 2),
    ("src/tools/edit.rs", "remove_dir_all(", 1),
    ("src/tools/os_sandbox.rs", "remove_dir_all(", 1),
    ("src/planner/pack/supply.rs", "remove_dir_all(", 2),
    ("src/tools/write.rs", "remove_dir_all(", 1),
    ("tests/gui_server.rs", "remove_dir_all(", 4),
    ("tests/issue505_write_toctou.rs", "remove_dir_all(", 3),
    // Shell recursive delete. `scripts/cmate-pm-watch.sh` also matches the
    // `rm -rf` inside its stop-pattern string and its own grep of that string.
    ("scripts/build-release.sh", "rm -rf", 3),
    ("scripts/install.sh", "rm -rf", 1),
    ("scripts/cmate-pm-watch.sh", "rm -rf", 3),
    // Python signal and recursive delete.
    ("scripts/demo/record_cli_demo.py", "os.kill(", 1),
    ("scripts/eval_lib/heavy_lock.py", "os.kill(", 2),
    ("scripts/eval_lib/heavy_lock.py", "shutil.rmtree(", 2),
    (
        "scripts/eval_lib/goal_verify_recovery_deterministic_pair.py",
        "os.killpg(",
        2,
    ),
    (
        "scripts/eval_lib/goal_verify_recovery_exposure_corpus_pilot.py",
        "os.killpg(",
        2,
    ),
    (
        "scripts/eval_lib/goal_verify_workspaces_v3.py",
        "shutil.rmtree(",
        1,
    ),
    (
        "scripts/eval_lib/goal_verify_live_v4.py",
        "shutil.rmtree(",
        2,
    ),
    ("scripts/nextjs_domain_oracle.py", "shutil.rmtree(", 1),
    ("tests/eval/test_heavy_lock.py", "os.kill(", 1),
];

/// Report every mismatch between the declared inventory and `corpus`:
/// undeclared sites, changed counts, and declared sites that disappeared.
fn inventory_violations(corpus: &[(String, String)]) -> Vec<String> {
    let mut found: BTreeMap<(String, String), usize> = BTreeMap::new();
    for (path, text) in corpus {
        for token in tokens_for(path) {
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
    let corpus = scan_tree();
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

/// The script surface is measured too: an unlisted external `kill` in a shell
/// script is a violation.
#[test]
fn dangerous_command_inventory_rejects_an_unlisted_shell_kill() {
    let corpus = vec![(
        "scripts/new_run.sh".to_string(),
        "kill -TERM \"$pid\"\n".to_string(),
    )];
    let violations = inventory_violations(&corpus);
    assert!(
        violations
            .iter()
            .any(|violation| violation.contains("scripts/new_run.sh")),
        "{violations:?}"
    );
}

fn scan_tree() -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut corpus = Vec::new();
    for (dir, extensions) in [
        ("src", ["rs"].as_slice()),
        ("tests", ["rs", "py"].as_slice()),
        ("scripts", ["sh", "py"].as_slice()),
    ] {
        for path in files_with_extensions(&root.join(dir), extensions) {
            push_relative(root, &path, &mut corpus);
        }
    }
    push_relative(root, &root.join("build.rs"), &mut corpus);
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

fn files_with_extensions(dir: &Path, extensions: &[&str]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    collect_files(dir, extensions, &mut out);
    out
}

fn collect_files(dir: &Path, extensions: &[&str], out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap_or_else(|error| panic!("failed to read {dir:?}: {error}"))
    {
        let entry = entry.expect("directory entry");
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, extensions, out);
        } else if let Some(extension) = path.extension().and_then(|ext| ext.to_str())
            && extensions.contains(&extension)
        {
            out.push(path);
        }
    }
}
