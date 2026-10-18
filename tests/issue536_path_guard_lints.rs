//! Paired accept/reject cases for the path_guard input boundary (#707).
//!
//! The panic-class lints (`unwrap_used`, `expect_used`, `indexing_slicing`) are
//! denied in `src/tools/path_guard.rs`. These tests pin the verdicts the
//! rewrite must keep: empty, out-of-range, multi-byte and brace-limit inputs
//! stay rejected, and their well-formed twins stay accepted. The same file is
//! run against the pre-change `develop` sources to show the verdicts match.

#![cfg(unix)]

use std::path::{Path, PathBuf};

use commandagent::tools::bash::path_confinement_rejection;
use commandagent::tools::path_guard::{
    normalize_absolute_workspace_glob, normalize_absolute_workspace_path,
    validate_workspace_relative,
};

fn render(result: anyhow::Result<Option<String>>) -> String {
    match result {
        Ok(Some(value)) => format!("ok:{value}"),
        Ok(None) => "none".to_string(),
        Err(error) => format!("err:{error}"),
    }
}

fn workspace(name: &str) -> (tempfile::TempDir, PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join(name);
    std::fs::create_dir_all(root.join("src")).unwrap();
    let root = root.canonicalize().unwrap();
    (temp, root)
}

fn components(root: &Path) -> String {
    root.to_string_lossy().trim_start_matches('/').to_string()
}

fn assert_rejected_relative(raw: &str) {
    assert!(
        validate_workspace_relative(raw).is_err(),
        "{raw:?} must stay rejected"
    );
}

fn assert_accepted_relative(raw: &str) {
    assert!(
        validate_workspace_relative(raw).is_ok(),
        "{raw:?} must stay accepted"
    );
}

#[test]
fn empty_and_nul_relative_paths_are_rejected() {
    assert_rejected_relative("");
    assert_rejected_relative("a\u{0}b");
    assert_accepted_relative("a.txt");
    assert_accepted_relative(".");
}

#[test]
fn windows_drive_forms_reject_only_when_complete() {
    // Complete drive-absolute forms are rejected.
    assert_rejected_relative("C:\\x");
    assert_rejected_relative("c:/x");
    assert_rejected_relative("\\\\server\\share");
    // Their truncated or non-ASCII twins are ordinary relative names.
    assert_accepted_relative("C:");
    assert_accepted_relative("C:x");
    assert_accepted_relative("ab:/x");
    assert_accepted_relative("é:/x");
    assert_accepted_relative("日:/x");
    assert_accepted_relative("1:/x");
}

#[test]
fn windows_drive_forms_are_rejected_by_normalizers_too() {
    let (_temp, root) = workspace("proj");
    for raw in ["C:\\x", "c:/x", "\\\\server\\share"] {
        let message = render(normalize_absolute_workspace_path(&root, raw));
        assert!(
            message.contains("absolute path is not allowed"),
            "{raw}: {message}"
        );
        let message = render(normalize_absolute_workspace_glob(&root, raw));
        assert!(
            message.contains("absolute path is not allowed"),
            "{raw}: {message}"
        );
    }
    assert_eq!(
        render(normalize_absolute_workspace_path(&root, "C:")),
        "none"
    );
    assert_eq!(
        render(normalize_absolute_workspace_path(&root, "é:/x")),
        "none"
    );
}

#[test]
fn missing_leading_slash_salvage_keeps_its_verdicts() {
    let (_temp, root) = workspace("proj");
    let anchored = components(&root);

    // The root anchor appears once: salvaged to the tail.
    let raw = format!("Users/{anchored}/src/a.rs");
    assert_eq!(
        render(normalize_absolute_workspace_path(&root, &raw)),
        "ok:src/a.rs"
    );
    // The anchor ends the path: nothing follows it, so the tail is the root.
    let raw = format!("Users/{anchored}");
    assert_eq!(
        render(normalize_absolute_workspace_path(&root, &raw)),
        "ok:."
    );
    // No anchor, a single component, or a duplicated anchor: not salvaged.
    for raw in [
        "Users/someone/else/a.rs".to_string(),
        format!("Users/{anchored}/{anchored}/a.rs"),
    ] {
        let message = render(normalize_absolute_workspace_path(&root, &raw));
        assert!(message.starts_with("err:"), "{raw}: {message}");
    }
    assert_eq!(
        render(normalize_absolute_workspace_path(&root, "Users")),
        "none"
    );
    assert_eq!(
        render(normalize_absolute_workspace_path(&root, "src/a.rs")),
        "none"
    );
}

#[test]
fn multibyte_root_names_salvage_without_splitting_characters() {
    let (_temp, root) = workspace("プロジェクト");
    let anchored = components(&root);
    let raw = format!("Users/{anchored}/src/日本語.rs");
    assert_eq!(
        render(normalize_absolute_workspace_path(&root, &raw)),
        "ok:src/日本語.rs"
    );
    // A same-length sibling that differs in a non-digit character is rejected.
    let sibling = format!("Users/{}/src/a.rs", anchored.replace("プロ", "ブロ"));
    let message = render(normalize_absolute_workspace_path(&root, &sibling));
    assert!(message.starts_with("err:"), "{message}");
}

#[test]
fn absolute_paths_inside_and_outside_the_root_keep_their_verdicts() {
    let (_temp, root) = workspace("proj");
    let inside = root.join("src/a.rs");
    assert_eq!(
        render(normalize_absolute_workspace_path(
            &root,
            &inside.to_string_lossy()
        )),
        "ok:src/a.rs"
    );
    assert_eq!(
        render(normalize_absolute_workspace_path(
            &root,
            &root.to_string_lossy()
        )),
        "ok:."
    );
    let message = render(normalize_absolute_workspace_path(&root, "/outside/x.rs"));
    assert!(message.starts_with("err:"), "{message}");
    let message = render(normalize_absolute_workspace_path(&root, "/"));
    assert!(message.starts_with("err:"), "{message}");
}

#[test]
fn digit_variance_of_the_root_is_rejected_as_corruption() {
    let (temp, root) = workspace("proj1");
    let sibling = temp.path().canonicalize().unwrap().join("proj2/src/a.rs");
    let message = render(normalize_absolute_workspace_path(
        &root,
        &sibling.to_string_lossy(),
    ));
    assert!(message.contains("near_root_corruption"), "{message}");
    // A non-digit difference is an ordinary escape, not corruption.
    let other = temp.path().canonicalize().unwrap().join("projA/src/a.rs");
    let message = render(normalize_absolute_workspace_path(
        &root,
        &other.to_string_lossy(),
    ));
    assert!(message.starts_with("err:"), "{message}");
    assert!(!message.contains("near_root_corruption"), "{message}");
}

#[test]
fn absolute_glob_split_keeps_its_verdicts() {
    let (_temp, root) = workspace("proj");
    let base = root.to_string_lossy().to_string();
    for (raw, expected) in [
        (format!("{base}/src/*.rs"), "ok:src/*.rs"),
        (format!("{base}/*.rs"), "ok:*.rs"),
        (format!("{base}/src/日本*.rs"), "ok:src/日本*.rs"),
        (format!("{base}/src/{{a,b}}.rs"), "ok:src/{a,b}.rs"),
    ] {
        assert_eq!(
            render(normalize_absolute_workspace_glob(&root, &raw)),
            expected,
            "{raw}"
        );
    }
    // A glob whose first metacharacter has no slash before it, or whose base
    // is outside the root, is not accepted.
    for raw in ["/*.rs", "/outside/*.rs", "/outside/日本*.rs"] {
        let message = render(normalize_absolute_workspace_glob(&root, raw));
        assert!(message.starts_with("err:"), "{raw}: {message}");
    }
    assert_eq!(
        render(normalize_absolute_workspace_glob(&root, "*.rs")),
        "none"
    );
    assert_eq!(render(normalize_absolute_workspace_glob(&root, "")), "none");
}

fn bash_rejects(root: &Path, command: &str) -> bool {
    path_confinement_rejection(command, root).is_some()
}

#[test]
fn brace_targets_inside_the_workspace_are_not_rejected() {
    let (_temp, root) = workspace("proj");
    for command in [
        "touch src/{a,b}.txt",
        "touch src/{é,日本}.txt",
        "touch src/{1..3}.txt",
        "touch src/{{a,b},c}.txt",
        "touch src/{}.txt",
        "touch src/{a}.txt",
    ] {
        assert!(!bash_rejects(&root, command), "{command} must pass");
    }
}

#[test]
fn brace_targets_outside_the_workspace_stay_rejected() {
    let (_temp, root) = workspace("proj");
    for command in [
        "touch /outside/{a,b}.txt",
        "touch /outside/{é,日本}.txt",
        "touch /outside/{1..3}.txt",
        "touch {a,../../escape}.txt",
        "touch src/{a,../../../escape}",
    ] {
        assert!(bash_rejects(&root, command), "{command} must be rejected");
    }
}

#[test]
fn unterminated_and_stray_braces_never_panic() {
    let (_temp, root) = workspace("proj");
    // Literal when unbalanced: proven as written, so inside targets pass.
    for command in [
        "touch src/{",
        "touch src/{a,b",
        "touch src/a}",
        "touch src/}{",
        "touch src/{{a,b",
        "touch src/é{",
        "touch src/日{本",
    ] {
        assert!(!bash_rejects(&root, command), "{command} must pass");
    }
    // The same shapes aimed outside the workspace remain rejected.
    for command in [
        "touch /outside/{",
        "touch /outside/{a,b",
        "touch /outside/}{",
    ] {
        assert!(bash_rejects(&root, command), "{command} must be rejected");
    }
}

#[test]
fn brace_expansion_limit_rejects_one_past_the_cap() {
    let (_temp, root) = workspace("proj");
    // The cap is 256 expansions: the cap passes, one more cannot be proven.
    assert!(!bash_rejects(&root, "touch src/{1..256}.txt"));
    assert!(bash_rejects(&root, "touch src/{1..257}.txt"));
    assert!(!bash_rejects(&root, "touch src/{1..16}{1..16}.txt"));
    assert!(bash_rejects(&root, "touch src/{1..16}{1..17}.txt"));
}

#[test]
fn missing_leading_slash_salvage_rejects_parent_components() {
    let (_temp, root) = workspace("proj");
    let anchored = components(&root);

    for raw in [
        format!("Users/{anchored}/../a.rs"),
        format!("Users/{anchored}/src/.."),
        format!("Users/{anchored}//../a.rs"),
        format!("Users/{anchored}/./../a.rs"),
        format!("Users/{anchored}/.././/a.rs"),
        format!("Users/../{anchored}/a.rs"),
        format!("Users/{anchored}/src/../a.rs"),
    ] {
        let message = render(normalize_absolute_workspace_path(&root, &raw));
        assert!(
            message.starts_with("err:path may not contain ..;"),
            "{raw}: {message}"
        );
    }
}

#[test]
fn missing_leading_slash_salvage_without_parent_components_is_unchanged() {
    let (_temp, root) = workspace("proj");
    let anchored = components(&root);

    for (raw, expected) in [
        (format!("Users/{anchored}/./src/a.rs"), "ok:src/a.rs"),
        (format!("Users/{anchored}//src//a.rs"), "ok:src/a.rs"),
        (format!("Users/{anchored}/a..b"), "ok:a..b"),
        (format!("Users/{anchored}/..foo/a.rs"), "ok:..foo/a.rs"),
    ] {
        assert_eq!(
            render(normalize_absolute_workspace_path(&root, &raw)),
            expected,
            "{raw}"
        );
    }
}

#[test]
fn missing_leading_slash_parent_rejection_matches_the_absolute_form() {
    let (_temp, root) = workspace("proj");
    let anchored = components(&root);

    let missing = render(normalize_absolute_workspace_path(
        &root,
        &format!("Users/{anchored}/src/../a.rs"),
    ));
    let absolute = render(normalize_absolute_workspace_path(
        &root,
        &format!("/{anchored}/src/../a.rs"),
    ));
    assert!(
        absolute.starts_with("err:path may not contain ..;"),
        "{absolute}"
    );
    assert_eq!(missing, absolute);
}
