#![cfg(unix)]

//! Issue #576: the Bash lexical guards did not understand shell comments (a `#`
//! at the start of a word runs to the end of the line) or heredoc bodies. A `'`
//! or `"` inside either was read as the start of a quote, so the lexer returned
//! `None`, the write-target set became empty, and the command was allowed to
//! write outside the workspace (or to read a secret). The guards now elide
//! comments and data-reader heredoc bodies once, keep every other body as text,
//! and fail closed on text they cannot read.
//!
//! Every test name contains `comment_heredoc` so the mutation-test filter
//! selects these tests together with the unit tests in `shell_lexical.rs`.
//! Rows come from H-02 §1 (blockers B1-B6) and H-02 §3 (the surviving mutants).

use std::path::PathBuf;

use commandagent::tools::allow_policy::bash_verify_command_is_auto_approvable;
use commandagent::tools::bash::path_confinement_rejection;
use commandagent::tools::sensitive_path::command_references_secret;

struct Fixture {
    _dir: tempfile::TempDir,
    root: PathBuf,
}

/// A workspace with an escaping `sub/link` symlink, an inside `a.txt`, an inside
/// credential file, and directories the verify auto-approval cases need.
fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("ws");
    for directory in ["sub", "frontend", "crates/x", "src", "tests"] {
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

fn assert_rejected(commands: &[&str]) {
    let fixture = fixture();
    let root = &fixture.root;
    for command in commands {
        assert!(
            path_confinement_rejection(command, root).is_some(),
            "expected rejection: {command:?}"
        );
    }
}

#[test]
fn comment_heredoc_rejects_b1_unreadable_delimiters() {
    assert_rejected(&[
        "cat <<\"E\\OF\"\nx\nE\\OF\ntee sub/link/f\nEOF",
        "cat <<\"E\\aOF\"\nx\nE\\aOF\ntee sub/link/f\nEaOF",
        "cat <<-\"E\\OF\"\nx\nE\\OF\ntee sub/link/f\nEOF",
        "cat <<E\\\nOF\nx\nEOF\ntee sub/link/f",
        "cat <<\"E\\\nOF\"\nx\nEOF\ntee sub/link/f",
        "cat <<E'O'\\\nF\nx\nEOF\ntee sub/link/f",
        "cat <<\\\nEOF\nx\nEOF\ntee sub/link/f",
        "cat <<EOF\nx\nEO\\\nF\ntee sub/link/f\nEOF",
        // Reads and protected paths through the same forms.
        "cat <<\"E\\OF\"\nx\nE\\OF\ncat sub/link/secret\nEOF",
        "cat <<EOF\nx\nEO\\\nF\ncat sub/link/secret\nEOF",
    ]);
}

#[test]
fn comment_heredoc_rejects_b2_body_executing_commands() {
    assert_rejected(&[
        "ba\"sh\" <<EOF\ntee sub/link/f\nEOF",
        "b\\ash <<EOF\ntee sub/link/f\nEOF",
        "s''h <<EOF\ntee sub/link/f\nEOF",
        "'ba'sh <<EOF\ntee sub/link/f\nEOF",
        "$SHELL <<EOF\ntee sub/link/f\nEOF",
        "${BASH:-bash} <<EOF\ntee sub/link/f\nEOF",
        "mksh <<EOF\ntee sub/link/f\nEOF",
        "fish <<EOF\ntee sub/link/f\nEOF",
        "{\nsh\n} <<EOF\ntee sub/link/f\nEOF",
        "f(){ sh; }\nf <<EOF\ntee sub/link/f\nEOF",
        "exec 3<<EOF\ntee sub/link/f\nEOF\nsh <&3",
        "xargs tee <<EOF\nsub/link/f\nEOF",
        "python3 <<EOF\nopen(\"sub/link/f\",\"w\")\nEOF",
        "node <<EOF\nopen(\"sub/link/f\",\"w\")\nEOF",
        "python3 <<EOF\nopen(\"sub/link/secret\").read()\nEOF",
    ]);
}

#[test]
fn comment_heredoc_rejects_b3_unquoted_body_execution() {
    assert_rejected(&[
        "cat <<EOF\n# $(tee sub/link/f)\nEOF",
        "cat <<EOF\nx # $(tee sub/link/f)\nEOF",
        "sh <<EOF\n# $(tee sub/link/f)\nEOF",
        "cat <<EOF\nit's\n$(tee sub/link/f)\nEOF\n#'",
        "cat <<EOF\n# $(cat sub/link/secret)\nEOF",
        // The accepted false rejection from design 10.
        "cat <<EOF > notes.md\nv $(git rev-parse HEAD)\nEOF",
    ]);
}

#[test]
fn comment_heredoc_rejects_b4_vertical_whitespace_hidden_groups() {
    assert_rejected(&[
        "echo x\r#x; tee sub/link/f",
        "echo x\u{b}#x; tee sub/link/f",
        "echo x\r#x; cat sub/link/secret",
        "echo x\u{c}#x; tee sub/link/f",
    ]);
}

#[test]
fn comment_heredoc_rejects_b5_continuation_hidden_comments() {
    assert_rejected(&[
        "echo a \\\n# \"x\ntee sub/link/f\n#\"y",
        "echo \\\n# \"x\ntee sub/link/f\n#\"y",
        "echo a\\\n#x\ntee sub/link/f",
        "npm test \\\n && tee sub/link/f",
    ]);
}

#[test]
fn comment_heredoc_rejects_n1_control_operators_and_functions() {
    assert_rejected(&[
        "cat; sh <<'EOF'\ntee sub/link/f\nEOF",
        "cat | sh <<EOF\ntee sub/link/f\nEOF",
        "cat <<'EOF' | sh\ntee sub/link/f\nEOF",
        "tee >(sh) <<'EOF'\ntee sub/link/f\nEOF",
        "cat <<'EOF' > >(sh)\ntee sub/link/f\nEOF",
        "cat <<'EOF'; sh <<'END'\nx\nEOF\ntee sub/link/f\nEND",
        "cat && python3 <<'EOF'\nopen(\"sub/link/secret\").read()\nEOF",
        "cat; env sh <<'EOF'\ntee sub/link/f\nEOF",
        // N1': a function of the same name may replace the command.
        "cat(){ sh; }\ncat <<'EOF'\ntee sub/link/f\nEOF",
        "function cat { sh; }\ncat <<'EOF'\ntee sub/link/f\nEOF",
    ]);
}

#[test]
fn comment_heredoc_rejects_n2_git_gh_and_openssl_forms() {
    assert_rejected(&[
        "git hash-object -w --stdin-paths <<'EOF'\nsub/link/secret\nEOF",
        "git -c alias.x='!sh' x <<'EOF'\ntee sub/link/f\nEOF",
        "openssl enc <<'EOF'\ntee sub/link/f\nEOF",
    ]);
}

#[test]
fn comment_heredoc_rejects_n3_carriage_return_delimiters() {
    assert_rejected(&[
        "cat <<EOF\r\nx\nEOF\r\ntee sub/link/f",
        "cat <<EOF\nx\nEOF\r\ntee sub/link/f",
    ]);
}

#[test]
fn comment_heredoc_rejects_n4_kept_body_comments() {
    assert_rejected(&[
        "sh <<'EOF'\n#'\ntee sub/link/f\n#'\nEOF",
        "bash <<'EOF'\necho x # it's\ntee sub/link/f #'\nEOF",
        "python3 <<'EOF'\n#'\nopen(\"sub/link/f\",\"w\")\n#'\nEOF",
    ]);
}

#[test]
fn comment_heredoc_detects_n5_secrets_after_a_quoted_body() {
    for command in [
        "cat <<'EOF'\nit's\nEOF\ncat .env",
        "cat <<EOF\nit's\nEOF\ncat .env #'",
        // The kept body is inspected on its own.
        "cat <<'EOF'\ncat .env\nEOF",
    ] {
        assert!(
            command_references_secret(command).is_some(),
            "expected secret detection: {command:?}"
        );
    }
}

#[test]
fn comment_heredoc_rejects_the_mutation_killer_inputs() {
    assert_rejected(&[
        "echo \"a #\"; tee sub/link/f\n\"",
        "echo `true` # it's\ntee sub/link/f #'",
        "cat <<EOF\nx\nEOF\ntee sub/link/f",
        "cat <<-EOF\n\tx\n\tEOF\ntee sub/link/f",
        "cat <<\"EOF\"\nx\nEOF\ntee sub/link/f",
    ]);
}

#[test]
fn comment_heredoc_rejects_c1_function_and_alias_definitions() {
    assert_rejected(&[
        "cat () { sh; }\ncat <<'EOF'\ntee sub/link/f\nEOF",
        "cat\t() { sh; }\ncat <<'EOF'\ntee sub/link/f\nEOF",
        "cat\\\n(){ sh; }\ncat <<'EOF'\ntee sub/link/f\nEOF",
        "alias cat=sh\ncat <<'EOF'\ntee sub/link/f\nEOF",
        "cat () { sh; }\ncat <<'EOF'\ncat sub/link/secret\nEOF",
    ]);
}

#[test]
fn comment_heredoc_rejects_c2_quoted_git_argv() {
    assert_rejected(&[
        "git '-c' 'al''ias.x=!sh #' x commit -F - <<'EOF'\ntee sub/link/f\nEOF",
        "git '-c' 'al''ias.x=!sh #' x tag -F - <<'EOF'\ntee sub/link/f\nEOF",
    ]);
}

#[test]
fn comment_heredoc_rejects_c3_python_inline_comment_quoting() {
    assert_rejected(&[
        "python3 <<'EOF'\npass#'\nopen(\"sub/link/f\",\"w\")#'\nEOF",
        "python3 <<'EOF'\npass#'\nopen(\"sub/link/secret\").read()#'\nEOF",
    ]);
}

#[test]
fn comment_heredoc_detects_c4_secrets_in_a_body() {
    for command in [
        "cat <<'EOF'\nit's\ncat .env\nEOF",
        "cat <<'EOF'\ncat .e\\\nnv\nEOF",
        "git commit -F - <<'EOF'\nit's\ncat .env\nEOF",
    ] {
        assert!(
            command_references_secret(command).is_some(),
            "expected secret detection: {command:?}"
        );
    }
}

#[test]
fn comment_heredoc_rejects_the_h04_killer_inputs() {
    assert_rejected(&[
        "echo \"a\\\" #x\"; tee sub/link/f\n\"",
        "echo `a\\` #x`; tee sub/link/f\n`",
        "echo \\` #'\ntee sub/link/f #'",
        "tee sub/li\\\nnk/f",
        "cat <'<' # it's\ntee sub/link/f\n'",
        "cat < x\ntee sub/link/f",
        "cat <<'E\\\nOF'\nx\nEOF\ntee sub/link/f",
        "sh <<'EOF'\necho \"\nEOF\ntee sub/link/f\n\"",
        "cat \"a\\\" #x\"; sh <<'EOF'\ntee sub/link/f\nEOF",
        "cat \"a\\;b\" <<'EOF'\ntee sub/link/f\nEOF",
        "cat a\\;b <<'EOF'\ntee sub/link/f\nEOF",
        "cat \\ #x; sh <<'EOF'\ntee sub/link/f\nEOF",
        "cat x#;sh <<'EOF'\ntee sub/link/f\nEOF",
        "cat \\#; sh <<'EOF'\ntee sub/link/f\nEOF",
    ]);
}

#[test]
fn comment_heredoc_removed_readers_are_accepted_false_rejections() {
    // `tee` is no longer a data reader; its unreadable body now rejects.
    assert_rejected(&["tee x <<'EOF'\nit's\nEOF"]);
}

#[test]
fn comment_heredoc_rejects_the_original_problem_table() {
    assert_rejected(&[
        "echo x # it's\ntee sub/link/f",
        "cat <<EOF\nit's\nEOF\ntee /tmp/f",
        "echo x # it's\ntee /tmp/f #'",
        "echo x # it's\n$'tee' /tmp/f #'",
        "echo x # it's\ncd sub && tee link/f #'",
        "echo x # it's\ncat sub/link/secret #'",
        "echo x #\"\ntee sub/link/f",
        "# it's\ntee sub/link/f #'",
        "echo x;# it's\ntee sub/link/f",
        "true &#c\ntee sub/link/f",
        "true |#c\ntee sub/link/f",
        "(#c\n) ; tee sub/link/f",
        "$(echo x # it's\n) ; tee sub/link/f",
        "cat <<EOF\nit's\nEOF\ntee /tmp/f",
        "cat <<EOF #'\nx\nEOF\ntee sub/link/f",
        "cat <<-EOF #'\n\tx\n\tEOF\ntee sub/link/f",
        "cat <<'EOF' #'\nx\nEOF\ntee sub/link/f",
        "cat <<\"EOF\" #'\nx\nEOF\ntee sub/link/f",
        "cat <<\\EOF #'\nx\nEOF\ntee sub/link/f",
        "cat << EOF #'\nx\nEOF\ntee sub/link/f",
        "cat <<A <<B\nx\nA\ny\nB\ntee sub/link/f",
        "cat <<EOF\nEOF \n EOF\nEOF\ntee sub/link/f",
        "echo $(cat <<EOF\nit's\nEOF\n) ; tee sub/link/f",
        "cat <<<\"it's\"\ntee sub/link/f",
        "tee 'x",
        "cat <<EOF\n$(tee sub/link/f)\nEOF",
        "sh <<EOF\ntee /tmp/f\nEOF",
        "bash -s <<EOF\ntee sub/link/f\nEOF",
        "echo $((1<<2))\ntee sub/link/f\n2))",
    ]);
}

#[test]
fn comment_heredoc_rejects_the_killer_table() {
    assert_rejected(&[
        "echo \"a\\\\\"#x\ntee sub/link/f",
        "echo `a\\`b`; tee sub/link/f",
        "echo `x #y`; tee sub/link/f",
        "echo a\\\n#x\ntee sub/link/f",
        "npm test \\\n && tee sub/link/f",
        "echo \\#x; tee sub/link/f",
        "cat <<-EOF\n\tdata\n\tEOF\ntee sub/link/f",
        "cat <<EOF\nEOF \n EOF\nEOF\ntee sub/link/f",
        "cat <<\\EOF\ndata\nEOF\ntee sub/link/f",
        "cat <<E'OF'\ndata\nEOF\ntee sub/link/f",
        "cat <<\"E\\\\OF\"\ndata\nE\\OF\ntee sub/link/f",
    ]);
}

#[test]
fn comment_heredoc_keeps_allowed_forms() {
    let fixture = fixture();
    let root = &fixture.root;
    let cases = [
        // Issue #576 allowed table.
        "cat <<'EOF' > notes.md\nit's\nEOF",
        "cat <<EOF > out.txt\nit's\nEOF",
        "cat > notes.md <<'EOF'\nit's \"quoted\nEOF",
        "git commit -F - <<'EOF'\nFix it's bug\nEOF",
        "gh issue comment 1 --body-file - <<'EOF'\nit's\nEOF",
        "gh pr create --body-file - <<'EOF'\nit's\nEOF",
        "cat <<EOF\nit's ok\nEOF",
        "cat <<EOF\ntee /tmp/f\nEOF",
        "cat <<'EOF'\n$(tee sub/link/f)\nEOF",
        "python3 - <<'EOF'\nprint(\"it's\")\nEOF",
        "cargo test # don't\ncargo build",
        "echo x # tee sub/link/f",
        "cd sub && tee f # CDPATH",
        "echo a#b > out.txt",
        "echo $# ${#x} ${x#y} > out.txt",
        // B1 readable delimiter forms (H-02 §1, "今の判定のまま").
        "cat <<'E\\OF'\nx\nE\\OF",
        "cat <<E\\\\OF\nx\nE\\OF",
        "cat <<\"E\\$OF\"\nx\nE$OF",
        // The severe side: an unterminated data-reader heredoc runs nothing.
        "cat <<EOF\ndata\ntee sub/link/f",
        // Short spellings must not panic and stay as before.
        "cat <",
        "cat <<x",
        "cat << \"E\\\\OF\"\ndata\nEOF\ntee sub/link/f",
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
    for command in [
        "echo x # it's\ncat .env",
        "cat .e\\\nnv",
        "cat <<'EOF'\ncat .env\nEOF",
        "cat <<EOF\ncat .env\nEOF",
        "cat <<EOF\n# $(cat .env)\nEOF",
        "echo \"x #y\";cat .env",
        "echo \"a\\\\\"#x\ncat .env",
        "echo x\r#x; cat .env",
        "echo \\#x; cat .env",
    ] {
        assert!(
            command_references_secret(command).is_some(),
            "expected secret detection: {command:?}"
        );
    }
    for command in ["echo x # cat .env", "cat .env.example"] {
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
        // B6: the verify normalization turns a vertical tab / NBSP into a space
        // and would comment out the mutation.
        "cargo test\u{b}#x;rm -rf src",
        "cargo test\u{b}#x;touch a.txt",
        "cargo test\u{a0}#x;tee tests/spec.rs",
        "echo x\u{b}#x;tee tests/spec.rs",
        // A line continuation also rewrites the text.
        "echo \"a\\\nb\"",
    ] {
        assert!(
            !bash_verify_command_is_auto_approvable(command, root),
            "must not be auto-approved: {command:?}"
        );
    }
    for command in ["cargo test", "cd frontend && npm test", "echo \"\\q\""] {
        assert!(
            bash_verify_command_is_auto_approvable(command, root),
            "must stay auto-approved: {command:?}"
        );
    }
}
