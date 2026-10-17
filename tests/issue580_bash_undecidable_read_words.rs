#![cfg(unix)]

//! Issue #580: when a `cd`/`pushd` destination cannot be determined (`CDPATH`,
//! `DIRSTACK`, a loop or function, more than 64 candidates), the second stage
//! refused any command with a relative read word. It counted the program name,
//! reserved words, and options as read words, so ordinary commands were
//! refused, and a word that only contained `DIRSTACK`/`CDPATH` as a substring
//! raised the mark. Only words that can name a file are counted now, and the
//! variable names count only where the shell reads them as variables.

use std::path::PathBuf;

use commandagent::tools::bash::path_confinement_rejection;

struct Fixture {
    _dir: tempfile::TempDir,
    root: PathBuf,
}

/// `sub` is an ordinary directory; `link` is a symlink to a directory outside
/// the workspace that holds `secret`.
fn fixture() -> Fixture {
    let dir = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).expect("create tempdir");
    let root = dir.path().join("ws");
    std::fs::create_dir_all(root.join("sub")).unwrap();
    std::fs::write(root.join("secret"), "x").unwrap();
    let outside = dir.path().join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("secret"), "outside-secret").unwrap();
    std::os::unix::fs::symlink(&outside, root.join("link")).unwrap();
    std::os::unix::fs::symlink(&outside, root.join("linked-outside")).unwrap();
    std::os::unix::fs::symlink(&outside, root.join("sub/link")).unwrap();
    let root = root.canonicalize().unwrap();
    Fixture { _dir: dir, root }
}

fn assert_allowed(root: &std::path::Path, commands: &[&str]) {
    for command in commands {
        assert!(
            path_confinement_rejection(command, root).is_none(),
            "expected allow: {command}"
        );
    }
}

fn assert_rejected(root: &std::path::Path, commands: &[&str]) {
    for command in commands {
        assert!(
            path_confinement_rejection(command, root).is_some(),
            "expected rejection: {command}"
        );
    }
}

#[test]
fn undecidable_cd_allows_commands_that_read_no_relative_file() {
    let fixture = fixture();
    let nested = format!("{}cargo build", "cd sub; ".repeat(7));
    assert_allowed(
        &fixture.root,
        &[
            "CDPATH= cd sub && cargo test",
            "CDPATH= cd sub && cargo build",
            "for d in a b; do (cd sub && cargo build); done",
            "CDPATH= cd sub && cargo --locked test",
            &nested,
        ],
    );
}

#[test]
fn undecidable_cd_allows_absolute_paths_inside_the_root() {
    let fixture = fixture();
    let root = fixture.root.display();
    let many = "cd sub; ".repeat(7);
    let command = format!("{many}tee {root}/a.txt");
    assert_allowed(&fixture.root, &[&command]);
}

#[test]
fn word_containing_a_variable_name_is_not_a_cd_mark() {
    let fixture = fixture();
    assert_allowed(
        &fixture.root,
        &[
            "cd sub && echo MYDIRSTACK",
            "cd sub && echo MYCDPATH",
            "cd sub && echo CDPATH_NOTE",
        ],
    );
}

#[test]
fn undecidable_cd_keeps_rejecting_relative_file_words() {
    let fixture = fixture();
    assert_rejected(
        &fixture.root,
        &[
            "CDPATH=sub cd link && cat secret",
            "CDPATH= cd sub && cat secret",
            "for d in a b; do (cd sub && cat secret); done",
            // Only what is left after the prefix is peeled is the program.
            "FOO=1 CDPATH= cd sub && FOO=1 cat secret",
            "CDPATH= cd sub && env cat secret",
            "CDPATH= cd sub && command cat secret",
            // `do`/`then` are reserved words only in command position.
            "CDPATH= cd sub && cat do",
            "CDPATH= cd sub && cat then",
            // A name that looks like an option is a file after `--`.
            "CDPATH= cd sub && cat -- -f",
            "CDPATH= cd sub && cat -- -",
            // Only the cargo subcommand is dropped, and only one word.
            "CDPATH=sub cd link && cargo test -- secret",
            "CDPATH=sub cd link && cargo test secret",
            "CDPATH=sub cd link && cargo ./x",
            "CDPATH=sub cd link && cargo -C secret test",
            "CDPATH=sub cd link && cargo Secret",
            // An option's value stays a read word.
            "CDPATH=sub cd link && cat --file=secret",
            "CDPATH=sub cd link && cat -f./x",
            "CDPATH=sub cd link && cat --file=./x",
        ],
    );
}

#[test]
fn variable_names_keep_raising_the_mark() {
    let fixture = fixture();
    assert_rejected(
        &fixture.root,
        &[
            "cd sub && echo $CDPATH && cat secret",
            "cd sub && echo ${CDPATH} && cat secret",
            "cd sub && echo ${DIRSTACK[1]} && cat secret",
            "cd sub && CDPATH=x; cat secret",
            "cd sub && DIRSTACK[1]=x; cat secret",
            "cd sub && DIRSTACK+=x; cat secret",
            "cd sub && declare CDPATH; cat secret",
            "cd sub && export CDPATH; cat secret",
            "cd sub && read DIRSTACK; cat secret",
            "cd sub && unset CDPATH; cat secret",
            "cd sub && \"DIR\"STACK[1]=x; cat secret",
            "cd sub && DIR\\STACK[1]=x; cat secret",
            "cd sub && \"CD\"PATH=x; cat secret",
        ],
    );
}

#[test]
fn input_redirect_targets_stay_read_words() {
    let fixture = fixture();
    assert_rejected(
        &fixture.root,
        &[
            "CDPATH=sub cd link && cat < secret",
            "CDPATH=sub cd link && cat <secret",
            "CDPATH=sub cd link && cat 0<secret",
            "CDPATH= cd sub && cat < secret",
        ],
    );
    assert_allowed(&fixture.root, &["CDPATH= cd sub && cargo test 2>&1"]);
}

#[test]
fn short_option_values_and_scripts_stay_read_words() {
    let fixture = fixture();
    let root = fixture.root.display();
    let absolute = format!("CDPATH=sub cd link && sed -fsecret {root}/secret");
    assert_rejected(
        &fixture.root,
        &[
            "CDPATH=sub cd link && grep -fsecret /dev/null",
            &absolute,
            "CDPATH=sub cd link && tar -xfsecret",
            "CDPATH= cd sub && grep -fsecret /dev/null",
            "CDPATH=sub cd link && sh -c 'cat secret'",
            "CDPATH= cd sub && sh -c 'cat secret'",
            "CDPATH= cd sub && grep --file=secret /dev/null",
        ],
    );
    assert_allowed(
        &fixture.root,
        &["CDPATH= cd sub && ls -a", "cd sub && tee -"],
    );
}

#[test]
fn undecidable_cd_that_may_leave_the_root_stays_rejected() {
    let fixture = fixture();
    assert_rejected(
        &fixture.root,
        &[
            "CDPATH=sub cd link && ls",
            "CDPATH=sub cd link && ls -la",
            "CDPATH=sub cd link && cargo test",
            "CDPATH=sub cd link && cargo run",
            "CDPATH=sub cd link && true",
            "export CDPATH=sub; cd link; true",
            "cd sub && DIRSTACK[1]=x; true",
            "cd sub && echo $CDPATH",
            "for d in a b; do (cd linked-outside && cargo test); done",
            "f() { cd linked-outside; }; f; ls",
            "for d in a b; do (cd sub && cd link && true); done",
            "for d in a b; do (cd $d && true); done",
            "for d in a b; do (cd .. && true); done",
            "for d in a b; do (cd && true); done",
        ],
    );
}
