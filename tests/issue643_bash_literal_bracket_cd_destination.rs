#![cfg(unix)]

//! Issue #643: Bash `cd`/`pushd` destinations with quoted or escaped literal
//! brackets (such as Next.js dynamic routes `cd "src/app/[id]"`) were falsely
//! rejected because the working-directory walk treated every bracket as an
//! undetermined glob destination (Issue #635).
//!
//! A destination whose glob/brace characters (`[`, `]`, `*`, `?`, `{`, `}`)
//! are all quoted or escaped is now recognized as a literal candidate working
//! directory. Relative reads and writes within it are allowed; outward symlinks
//! and mixed executed globs remain rejected.

use std::path::PathBuf;

use commandagent::tools::bash::path_confinement_rejection;

/// The fixed operation a read-side refusal carries.
const READ_OPERATION: &str = "path reference";
/// The existing write-side working-directory reason.
const CWD_REASON: &str = "follows a working directory change that cannot be determined";

struct Fixture {
    _dir: tempfile::TempDir,
    root: PathBuf,
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).expect("create tempdir");
    let root = dir.path().join("ws");
    for directory in ["src/app/[id]", "s[2]", "star*", "q?"] {
        std::fs::create_dir_all(root.join(directory)).unwrap();
    }
    std::fs::write(
        root.join("src/app/[id]/page.tsx"),
        "export default function Page() {}",
    )
    .unwrap();
    std::fs::write(root.join("s[2]/item.txt"), "hello").unwrap();
    std::fs::write(root.join("file*"), "regular file").unwrap();

    let outside = dir.path().join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("secret"), "outside-secret").unwrap();

    std::os::unix::fs::symlink(&outside, root.join("src/app/[id]/lf6")).unwrap();
    std::os::unix::fs::symlink(&outside, root.join("star*/lf6")).unwrap();
    std::os::unix::fs::symlink(&outside, root.join("q?/lf6")).unwrap();

    let root = root.canonicalize().unwrap();
    Fixture { _dir: dir, root }
}

/// Acceptance table rows 1 and 2: reads and writes inside a literal bracket
/// destination are allowed (R=0, W=0).
#[test]
fn literal_bracket_cd_destination_allows_internal_reads_and_writes() {
    let fixture = fixture();
    let root = &fixture.root;

    // Row 1: reads inside the literal destination.
    for command in [
        "cd \"src/app/[id]\" && cat page.tsx",
        "cd 'src/app/[id]' && cat page.tsx",
        r"cd src/app/\[id\] && cat page.tsx",
        "pushd \"src/app/[id]\" && cat page.tsx",
        // Issue #637 migration: cd "s[2]" && ls is allowed.
        "cd \"s[2]\" && ls",
    ] {
        assert!(
            path_confinement_rejection(command, root).is_none(),
            "expected allow: {command}"
        );
    }

    // Row 2: writes inside the literal destination.
    for command in [
        "cd \"src/app/[id]\" && echo x > page.tsx",
        "cd 'src/app/[id]' && touch page.tsx",
        "cd \"src/app/[id]\" && tee page.tsx",
        "pushd \"src/app/[id]\" && echo x > page.tsx",
    ] {
        assert!(
            path_confinement_rejection(command, root).is_none(),
            "expected allow: {command}"
        );
    }
}

/// Acceptance table row 3: an outward symlink inside the literal bracket
/// destination remains rejected (R=1, W=1).
#[test]
fn literal_bracket_cd_destination_rejects_outward_symlink() {
    let fixture = fixture();
    let root = &fixture.root;

    // Read through outward symlink: R=1.
    for command in [
        "cd \"src/app/[id]\" && cat lf6",
        "cd 'src/app/[id]' && cat lf6",
        r"cd src/app/\[id\] && cat lf6",
        "pushd \"src/app/[id]\" && cat lf6",
    ] {
        let rejection = path_confinement_rejection(command, root)
            .unwrap_or_else(|| panic!("expected rejection: {command}"));
        assert_eq!(rejection.operation, READ_OPERATION, "command: {command}");
        assert_eq!(rejection.path, "lf6", "command: {command}");
        assert!(
            rejection
                .reason
                .contains("resolves outside current workspace root"),
            "command: {command}: {}",
            rejection.reason
        );
    }

    // Write through outward symlink: R=1, W=1.
    for (command, operation) in [
        ("cd \"src/app/[id]\" && echo x > lf6", "output redirection"),
        ("cd \"src/app/[id]\" && touch lf6", "touch"),
        ("cd \"src/app/[id]\" && tee lf6", "tee"),
        (
            "pushd \"src/app/[id]\" && echo x > lf6",
            "output redirection",
        ),
    ] {
        let rejection = path_confinement_rejection(command, root)
            .unwrap_or_else(|| panic!("expected write rejection: {command}"));
        assert_eq!(rejection.operation, operation, "command: {command}");
        assert_eq!(rejection.path, "src/app/[id]/lf6", "command: {command}");
        assert!(
            rejection.reason.contains(
                "outside the Gate 1 workspace boundary relative to a working directory candidate"
            ),
            "command: {command}: {}",
            rejection.reason
        );
    }
}

/// Acceptance table row 4: destinations that mix executed glob metacharacters
/// with quoted ones, or leave glob metacharacters unquoted, remain rejected (R=1).
#[test]
fn mixed_or_executed_glob_destinations_remain_rejected() {
    let fixture = fixture();
    let root = &fixture.root;

    for command in [
        "cd \"src/app/\"[id] && cat page.tsx",
        "cd \"src/app/\"[id] && ls",
        "cd \"src/app/\"[id]",
        "cd \"src/app/\"[i]d && cat page.tsx",
        "cd \"src/app/\"* && cat page.tsx",
        "cd src/app/[id] && cat page.tsx",
        "cd \"s\"[2] && ls",
    ] {
        let rejection = path_confinement_rejection(command, root)
            .unwrap_or_else(|| panic!("expected rejection: {command}"));
        assert!(
            rejection.reason.contains(CWD_REASON)
                || rejection
                    .reason
                    .contains("resolves outside current workspace root"),
            "command: {command}: {}",
            rejection.reason
        );
    }
}

/// Variable expansions, command substitutions, `..`, and `cd -` remain rejected.
#[test]
fn dynamic_and_escaping_cd_destinations_remain_rejected() {
    let fixture = fixture();
    let root = &fixture.root;

    for command in [
        "cd \"$x\" && cat page.tsx",
        "cd \"$VAR\" && cat page.tsx",
        "cd \"$(pwd)\" && cat page.tsx",
        "cd `pwd` && cat page.tsx",
        "cd .. && cat page.tsx",
        "cd - && cat page.tsx",
    ] {
        assert!(
            path_confinement_rejection(command, root).is_some(),
            "expected rejection: {command}"
        );
    }
}

/// Quoted `*` and `?` destinations:
/// - When the directory does NOT exist on disk: rejected as undetermined cwd.
/// - When the directory DOES exist on disk, but accesses an outward symlink: rejected as outside workspace.
/// - When the path exists as a regular file (not directory): rejected as undetermined cwd.
#[test]
fn quoted_wildcard_destinations_require_existing_directory() {
    let fixture = fixture();
    let root = &fixture.root;

    // Non-existent directory with quoted wildcards: rejected.
    for command in [
        "cd \"nonexistent*\" && cat page.tsx",
        "cd \"nonexistent?\" && cat page.tsx",
        "cd 'no_such_{dir}' && cat page.tsx",
    ] {
        let rejection = path_confinement_rejection(command, root)
            .unwrap_or_else(|| panic!("expected rejection: {command}"));
        assert!(
            rejection.reason.contains(CWD_REASON),
            "command: {command}: {}",
            rejection.reason
        );
    }

    // Existing directory with quoted wildcards, accessing outward symlink: rejected.
    for command in ["cd \"star*\" && cat lf6", "cd \"q?\" && cat lf6"] {
        let rejection = path_confinement_rejection(command, root)
            .unwrap_or_else(|| panic!("expected rejection: {command}"));
        assert_eq!(rejection.path, "lf6", "command: {command}");
        assert!(
            rejection
                .reason
                .contains("resolves outside current workspace root"),
            "command: {command}: {}",
            rejection.reason
        );
    }

    // Path exists as a regular file, not a directory: rejected.
    let rejection = path_confinement_rejection("cd \"file*\" && cat page.tsx", root)
        .unwrap_or_else(|| panic!("expected rejection: cd \"file*\""));
    assert!(
        rejection.reason.contains(CWD_REASON),
        "command: cd \"file*\": {}",
        rejection.reason
    );
}
