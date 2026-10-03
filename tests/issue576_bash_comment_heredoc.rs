#![cfg(unix)]

//! Issue #576: the Bash lexical guards did not understand shell comments (a `#`
//! at the start of a word runs to the end of the line) or heredoc bodies. A `'`
//! or `"` inside either was read as the start of a quote, so the lexer returned
//! `None`, the write-target set became empty, and the command was allowed to
//! write outside the workspace (or to read a secret). The guards now elide
//! comments and heredoc bodies once, and fail closed when the text cannot be
//! read.
//!
//! Every test name contains `comment_heredoc` so the mutation-test filter
//! selects these tests together with the unit tests in `shell_lexical.rs`.

use std::path::PathBuf;

use commandagent::tools::allow_policy::bash_verify_command_is_auto_approvable;
use commandagent::tools::bash::path_confinement_rejection;
use commandagent::tools::sensitive_path::command_references_secret;

struct Fixture {
    _dir: tempfile::TempDir,
    root: PathBuf,
}

/// A workspace with an escaping `sub/link` symlink, an inside `a.txt`, and an
/// inside credential file; `frontend` and `crates/x` keep the verify
/// auto-approval cases exercisable.
fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("ws");
    for directory in ["sub", "frontend", "crates/x", "src"] {
        std::fs::create_dir_all(root.join(directory)).unwrap();
    }
    std::fs::write(root.join("a.txt"), "x").unwrap();
    std::fs::write(root.join(".env"), "workspace-secret").unwrap();
    let outside = dir.path().join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("secret"), "outside-secret").unwrap();
    std::os::unix::fs::symlink(&outside, root.join("sub/link")).unwrap();
    let root = root.canonicalize().unwrap();
    Fixture { _dir: dir, root }
}

#[test]
fn comment_heredoc_rejects_comments_that_hide_writes_and_reads() {
    let fixture = fixture();
    let root = &fixture.root;
    let cases = [
        // The problem table from the issue body.
        "echo x # it's\ntee sub/link/f",
        "cat <<EOF\nit's\nEOF\ntee /tmp/f",
        "echo x # it's\ntee /tmp/f #'",
        "echo x # it's\n$'tee' /tmp/f #'",
        "echo x # it's\ncd sub && tee link/f #'",
        "echo x # it's\ncat sub/link/secret #'",
        // Comment introducers in every operator position.
        "echo x #\"\ntee sub/link/f",
        "# it's\ntee sub/link/f #'",
        "echo x;# it's\ntee sub/link/f",
        "true &#c\ntee sub/link/f",
        "true |#c\ntee sub/link/f",
        "(#c\n) ; tee sub/link/f",
        "$(echo x # it's\n) ; tee sub/link/f",
        // Heredoc delimiter spellings whose bodies must be dropped, then a
        // command after the terminator caught.
        "cat <<EOF\nx\nEOF\ntee sub/link/f",
        "cat <<-EOF\n\tx\n\tEOF\ntee sub/link/f",
        "cat <<'EOF'\nx\nEOF\ntee sub/link/f",
        "cat <<\"EOF\"\nx\nEOF\ntee sub/link/f",
        "cat <<\\EOF\nx\nEOF\ntee sub/link/f",
        "cat << EOF\nx\nEOF\ntee sub/link/f",
        "cat <<A <<B\nx\nA\ny\nB\ntee sub/link/f",
        // A near-delimiter line must not terminate the body early.
        "cat <<EOF\nEOF \nEOF\ntee sub/link/f",
        "cat <<EOF\n EOF\nEOF\ntee sub/link/f",
        // A body that the shell executes must stay inspected.
        "echo $(cat <<EOF\nit's\nEOF\n) ; tee sub/link/f",
        "cat <<<\"it's\"\ntee sub/link/f",
        "tee 'x",
        "cat <<EOF\n$(tee sub/link/f)\nEOF",
        "sh <<EOF\ntee /tmp/f\nEOF",
        "bash -s <<EOF\ntee sub/link/f\nEOF",
        "echo $((1<<2))\ntee sub/link/f\n2))",
    ];
    for command in cases {
        assert!(
            path_confinement_rejection(command, root).is_some(),
            "expected rejection: {command:?}"
        );
    }
}

#[test]
fn comment_heredoc_keeps_allowed_forms_and_stops_false_rejections() {
    let fixture = fixture();
    let root = &fixture.root;
    let cases = [
        // The allowed table from the issue body.
        "cat <<'EOF' > notes.md\nit's\nEOF",
        "cat <<EOF > out.txt\nit's\nEOF",
        "cat > notes.md <<'EOF'\nit's \"quoted\nEOF",
        "git commit -F - <<'EOF'\nFix it's bug\nEOF",
        "cat <<EOF\ntee /tmp/f\nEOF",
        "cat <<'EOF'\n$(tee sub/link/f)\nEOF",
        "python3 - <<'EOF'\nprint(\"it's\")\nEOF",
        "cargo test # don't\ncargo build",
        "echo x # tee sub/link/f",
        "cd sub && tee f # CDPATH",
        "echo a#b > out.txt",
        "echo $# ${#x} ${x#y} > out.txt",
    ];
    for command in cases {
        assert!(
            path_confinement_rejection(command, root).is_none(),
            "expected allow: {command:?}"
        );
    }
}

#[test]
fn comment_heredoc_detects_and_ignores_secrets_in_the_elided_text() {
    for command in ["echo x # it's\ncat .env", "cat .e\\\nnv"] {
        assert!(
            command_references_secret(command).is_some(),
            "expected secret detection: {command:?}"
        );
    }
    for command in ["echo x # cat .env", "cat <<'EOF'\ncat .env\nEOF"] {
        assert!(
            command_references_secret(command).is_none(),
            "expected no secret: {command:?}"
        );
    }
}

#[test]
fn comment_heredoc_gates_bash_verify_auto_approval() {
    let fixture = fixture();
    let root = &fixture.root;
    for command in [
        "echo x # cd sub\ntee link/f",
        "cargo test # it's\nrm -rf src #'",
    ] {
        assert!(
            !bash_verify_command_is_auto_approvable(command, root),
            "must not be auto-approved: {command:?}"
        );
    }
    for command in ["cargo test", "cd frontend && npm test"] {
        assert!(
            bash_verify_command_is_auto_approvable(command, root),
            "must stay auto-approved: {command:?}"
        );
    }
}
