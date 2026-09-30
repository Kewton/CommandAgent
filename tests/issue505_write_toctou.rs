//! Issue #505 — Write/Edit must not be redirected outside the workspace when a
//! parent directory is swapped for a symlink between the check and the open.
//!
//! Deterministic hook-driven cases live in `src/tools/{write,edit,dir_fd}` unit
//! tests (the seam is `#[cfg(test)]`). This integration test covers the
//! non-deterministic race (time-bounded), the static rejected shapes, the
//! ordinary behaviour that must not change, and the error classification.
//!
//! Synthetic values only: everything happens inside one tempdir, and every
//! symlink points at a sibling "outside" directory (never a real host path).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use commandagent::tools::registry::tool_error_kind;
use commandagent::tools::{edit, write};

const PAYLOAD: &str = "PAYLOAD_505";
const OUTSIDE_CANARY: &str = "OUTSIDE_CANARY_505";

fn workspace(hold: &Path) -> PathBuf {
    let workspace = hold.join("W");
    fs::create_dir_all(&workspace).unwrap();
    workspace
}

fn outside(hold: &Path) -> PathBuf {
    let outside = hold.join("O");
    fs::create_dir_all(&outside).unwrap();
    outside
}

fn entries(dir: &Path) -> Vec<std::ffi::OsString> {
    match fs::read_dir(dir) {
        Ok(entries) => entries.map(|entry| entry.unwrap().file_name()).collect(),
        Err(_) => Vec::new(),
    }
}

// --- Rejected static shapes (rows 6, 7, 10, 11, 15) ---------------------------

#[test]
fn static_parent_symlink_is_refused_and_classified() {
    let hold = tempfile::tempdir().unwrap();
    let workspace = workspace(hold.path());
    let outside = outside(hold.path());
    std::os::unix::fs::symlink(&outside, workspace.join("d")).unwrap();

    let err = write::write_checked(&workspace, &workspace.join("d/f.txt"), PAYLOAD).unwrap_err();
    assert!(
        err.to_string()
            .contains("path escapes workspace through existing component"),
        "{err}"
    );
    assert_eq!(tool_error_kind(&err), "path_confinement_error");
    assert!(!outside.join("f.txt").exists());
    assert!(entries(&outside).is_empty());
}

#[test]
fn final_leaf_symlink_is_refused_and_classified() {
    let hold = tempfile::tempdir().unwrap();
    let workspace = workspace(hold.path());
    let outside = outside(hold.path());
    std::os::unix::fs::symlink(outside.join("t.txt"), workspace.join("leaf")).unwrap();

    let err = write::write_checked(&workspace, &workspace.join("leaf"), PAYLOAD).unwrap_err();
    assert!(err.to_string().contains("symlink_write_blocked"), "{err}");
    assert_eq!(tool_error_kind(&err), "path_confinement_error");
    assert!(!outside.join("t.txt").exists());
}

#[test]
fn inner_symlink_parent_is_refused_for_write_and_edit() {
    let hold = tempfile::tempdir().unwrap();
    let workspace = workspace(hold.path());
    fs::create_dir_all(workspace.join("real")).unwrap();
    fs::write(workspace.join("real/e.txt"), "anchor_505\n").unwrap();
    std::os::unix::fs::symlink(workspace.join("real"), workspace.join("link")).unwrap();

    let write_err =
        write::write_checked(&workspace, &workspace.join("link/f.txt"), PAYLOAD).unwrap_err();
    assert!(
        write_err
            .to_string()
            .contains("path escapes workspace through existing component"),
        "{write_err}"
    );
    assert_eq!(tool_error_kind(&write_err), "path_confinement_error");
    assert!(!workspace.join("real/f.txt").exists());

    let edit_err = edit::run(
        &workspace,
        &workspace.join("link/e.txt"),
        "anchor_505",
        "replaced",
        false,
    )
    .unwrap_err();
    assert!(
        edit_err
            .to_string()
            .contains("path escapes workspace through existing component"),
        "{edit_err}"
    );
    assert_eq!(
        fs::read_to_string(workspace.join("real/e.txt")).unwrap(),
        "anchor_505\n"
    );
}

#[test]
fn symlinked_workspace_root_is_still_trusted() {
    let hold = tempfile::tempdir().unwrap();
    let real = hold.path().join("realroot");
    fs::create_dir_all(&real).unwrap();
    let alias = hold.path().join("linkroot");
    std::os::unix::fs::symlink(&real, &alias).unwrap();

    write::write_checked(&alias, &alias.join("f.txt"), PAYLOAD).unwrap();
    assert_eq!(fs::read_to_string(real.join("f.txt")).unwrap(), PAYLOAD);
}

#[test]
fn edit_final_leaf_symlink_is_refused_before_reading() {
    let hold = tempfile::tempdir().unwrap();
    let workspace = workspace(hold.path());
    let outside = outside(hold.path());
    fs::write(outside.join("f.txt"), OUTSIDE_CANARY).unwrap();
    std::os::unix::fs::symlink(outside.join("f.txt"), workspace.join("e")).unwrap();

    let err = edit::run(&workspace, &workspace.join("e"), "old", "new", false).unwrap_err();
    assert!(err.to_string().contains("symlink_write_blocked"), "{err}");
    assert_eq!(
        fs::read_to_string(outside.join("f.txt")).unwrap(),
        OUTSIDE_CANARY
    );
}

// --- Ordinary behaviour that must not change (rows 8, 9, 16, 17) --------------

#[test]
fn ordinary_create_and_in_place_replace_are_unchanged() {
    use std::os::unix::fs::MetadataExt;

    let hold = tempfile::tempdir().unwrap();
    let workspace = workspace(hold.path());

    write::write_checked(&workspace, &workspace.join("ok/new.txt"), PAYLOAD).unwrap();
    assert_eq!(
        fs::read_to_string(workspace.join("ok/new.txt")).unwrap(),
        PAYLOAD
    );

    fs::create_dir_all(workspace.join("d")).unwrap();
    fs::write(workspace.join("d/f.txt"), "before").unwrap();
    let before = fs::metadata(workspace.join("d/f.txt")).unwrap().ino();
    write::write_checked(&workspace, &workspace.join("d/f.txt"), "after").unwrap();
    assert_eq!(
        fs::metadata(workspace.join("d/f.txt")).unwrap().ino(),
        before
    );
    assert_eq!(
        fs::read_to_string(workspace.join("d/f.txt")).unwrap(),
        "after"
    );
}

#[test]
fn ordinary_edit_replace_salvage_and_already_applied_are_unchanged() {
    let hold = tempfile::tempdir().unwrap();
    let workspace = workspace(hold.path());

    fs::write(workspace.join("plain.txt"), "hello world\n").unwrap();
    let out = edit::run(
        &workspace,
        &workspace.join("plain.txt"),
        "hello",
        "hi",
        false,
    )
    .unwrap();
    assert!(out.contains("edited"), "{out}");
    assert_eq!(
        fs::read_to_string(workspace.join("plain.txt")).unwrap(),
        "hi world\n"
    );

    fs::write(workspace.join("salv.txt"), "const  x = 1;\n").unwrap();
    let out = edit::run(
        &workspace,
        &workspace.join("salv.txt"),
        "const x = 1;",
        "const x = 2;",
        false,
    )
    .unwrap();
    assert!(out.contains("edit_anchor_salvaged"), "{out}");
    assert_eq!(
        fs::read_to_string(workspace.join("salv.txt")).unwrap(),
        "const x = 2;\n"
    );

    fs::write(workspace.join("applied.txt"), "new").unwrap();
    let out = edit::run(
        &workspace,
        &workspace.join("applied.txt"),
        "old",
        "new",
        false,
    )
    .unwrap();
    assert!(out.contains("edit_already_applied"), "{out}");
    assert_eq!(
        fs::read_to_string(workspace.join("applied.txt")).unwrap(),
        "new"
    );
}

#[test]
fn edit_anchor_excerpt_still_reports_inside_content() {
    let hold = tempfile::tempdir().unwrap();
    let workspace = workspace(hold.path());
    fs::write(workspace.join("anch.txt"), "alpha middle omega").unwrap();

    let err = edit::run(
        &workspace,
        &workspace.join("anch.txt"),
        "alpha changed omega",
        "done",
        false,
    )
    .unwrap_err();
    let message = err.to_string();
    assert!(message.contains("edit_anchor_not_found"), "{message}");
    assert!(message.contains("1 | alpha middle omega"), "{message}");
}

// --- Time-bounded race (rows 5, 14) ------------------------------------------

#[test]
fn write_race_never_creates_files_outside() {
    let hold = tempfile::tempdir().unwrap();
    let workspace = workspace(hold.path());
    let outside = outside(hold.path());
    let dir = workspace.join("d");
    fs::create_dir_all(&dir).unwrap();

    let stop = Arc::new(AtomicBool::new(false));
    let attacker = {
        let dir = dir.clone();
        let outside = outside.clone();
        let stop = Arc::clone(&stop);
        thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                let _ = fs::remove_dir_all(&dir);
                let _ = fs::remove_file(&dir);
                if std::os::unix::fs::symlink(&outside, &dir).is_err() {
                    let _ = fs::create_dir_all(&dir);
                    continue;
                }
                thread::yield_now();
            }
        })
    };

    let deadline = Instant::now() + Duration::from_millis(1200);
    while Instant::now() < deadline {
        let _ = write::write_checked(&workspace, &workspace.join("d/f.txt"), PAYLOAD);
        let _ = write::write_checked(&workspace, &workspace.join("d/sub/f.txt"), PAYLOAD);
    }
    stop.store(true, Ordering::Relaxed);
    attacker.join().unwrap();

    assert!(
        entries(&outside).is_empty(),
        "outside must stay empty during a parent swap race: {:?}",
        entries(&outside)
    );
}

#[test]
fn edit_race_never_touches_outside_and_never_leaks_canary() {
    let hold = tempfile::tempdir().unwrap();
    let workspace = workspace(hold.path());
    let outside = outside(hold.path());
    fs::write(outside.join("f.txt"), format!("{OUTSIDE_CANARY}\n")).unwrap();
    let dir = workspace.join("d");

    let stop = Arc::new(AtomicBool::new(false));
    let attacker = {
        let dir = dir.clone();
        let outside = outside.clone();
        let stop = Arc::clone(&stop);
        thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                let _ = fs::remove_dir_all(&dir);
                let _ = fs::remove_file(&dir);
                let _ = fs::create_dir_all(&dir);
                let _ = fs::write(dir.join("f.txt"), "anchor_505\n");
                let _ = fs::remove_dir_all(&dir);
                let _ = fs::remove_file(&dir);
                let _ = std::os::unix::fs::symlink(&outside, &dir);
                thread::yield_now();
            }
        })
    };

    let deadline = Instant::now() + Duration::from_millis(1200);
    while Instant::now() < deadline {
        if let Err(err) = edit::run(
            &workspace,
            &workspace.join("d/f.txt"),
            "anchor_505",
            "replaced",
            false,
        ) {
            let display = err.to_string();
            let debug = format!("{err:?}");
            assert!(!display.contains(OUTSIDE_CANARY), "{display}");
            assert!(!debug.contains(OUTSIDE_CANARY), "{debug}");
        }
    }
    stop.store(true, Ordering::Relaxed);
    attacker.join().unwrap();

    assert_eq!(
        fs::read_to_string(outside.join("f.txt")).unwrap(),
        format!("{OUTSIDE_CANARY}\n")
    );
}

// --- Failure shape on non-unix (row 21) --------------------------------------

#[cfg(not(unix))]
#[test]
fn non_unix_write_fails_closed_without_touching_the_filesystem() {
    let hold = tempfile::tempdir().unwrap();
    let err = write::write_checked(hold.path(), &hold.path().join("f.txt"), "x").unwrap_err();
    assert!(!err.to_string().is_empty());
    assert!(!hold.path().join("f.txt").exists());
}
