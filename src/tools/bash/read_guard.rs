//! Read-path judgement for the second-stage Bash inspection (Issue #582).
//!
//! The write guard (`bash_write_guard`) already refuses every write that leaves
//! the workspace, but the read side only inspected static path words: a command
//! whose text carried `$`, a backtick, or a glob made the old word splitter bail
//! out entirely, so an unrelated static relative path (`sub/link/secret`) next to
//! it was never confined, and a relative/absolute glob read was never expanded.
//!
//! This leaf module consumes [`super::path_tokens::read_words`] and classifies
//! each word the shell would see:
//!
//! * a static word becomes a candidate the existing write-target proof confines;
//! * a supported glob or brace is passed through so that proof expands it;
//! * a word whose value cannot be determined (a command substitution, an
//!   arithmetic expansion, a parameter operator, a special parameter, or a
//!   backtick) is refused, except for the two explicit display exceptions — a
//!   leading simple variable assignment and a simple `echo` segment showing a
//!   plain `$NAME` / `${NAME}`.
//!
//! The judgement is deliberately conservative: it never reads a variable value
//! or runs a substitution to resolve a word, and it is never used to prove a
//! later variable reference safe.

use std::path::{Path, PathBuf};

use super::path_tokens::{self, CrMode, Word};

/// The reason recorded for a read word whose expanded value cannot be proven to
/// remain in the workspace.
pub(super) const UNVERIFIABLE_REASON: &str =
    "is a read argument whose expanded value cannot be proven to remain in the workspace";

/// A read-path candidate together with how it must be confined.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ReadCandidate {
    /// A literal path spelling. It is confined by its literal form only, so a
    /// quoted bracket or brace is never treated as a glob (Issue #582 design 3).
    Literal(String),
    /// A word the shell may expand: the existing write-target proof expands
    /// every glob/brace result and confines it.
    Expand(String),
}

impl ReadCandidate {
    pub(super) fn path(&self) -> &str {
        match self {
            Self::Literal(path) | Self::Expand(path) => path,
        }
    }

    pub(super) fn expands(&self) -> bool {
        matches!(self, Self::Expand(_))
    }
}

/// The read-path candidates and the first unverifiable word for one command.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct ReadVerdict {
    /// A word whose value cannot be determined statically. The caller refuses.
    pub unverifiable: Option<String>,
    /// Path spellings to confine with the write-target proof and the
    /// working-directory candidate union.
    pub candidates: Vec<ReadCandidate>,
    /// `/`-less static words the caller re-checks against the working-directory
    /// candidates when no directory separator names them (Issue #613 design 2).
    pub relative_words: Vec<String>,
}

/// Reads the read-path judgement for a command the write guard allowed.
pub(super) fn inspect(command: &str) -> ReadVerdict {
    // Read the elided text exactly as the write guard does; a command it allowed
    // has readable text, so the raw fallback is only a belt-and-braces guard.
    let elided = super::super::shell_lexical::strip_comments_and_heredocs(command);
    let text = elided.as_deref().unwrap_or(command);
    let mut verdict = inspect_text(text, CrMode::Separator);
    // Issue #613: Bash keeps a carriage return inside the word. Read the text
    // that way too and union the verdicts, so an outward path whose spelling
    // holds a `\r` is refused when either reading sees it. The two readings
    // coincide when the text has no `\r`.
    if text.contains('\r') {
        merge(&mut verdict, inspect_text(text, CrMode::WordChar));
    }
    verdict
}

/// The read-path judgement of one carriage-return reading of the elided text.
fn inspect_text(text: &str, cr_mode: CrMode) -> ReadVerdict {
    let Some(segments) = path_tokens::read_words(text, cr_mode) else {
        return ReadVerdict {
            unverifiable: Some("<shell text>".to_string()),
            candidates: Vec::new(),
            relative_words: Vec::new(),
        };
    };
    let glob_shell_option = uses_glob_shell_option(text);
    let mut verdict = ReadVerdict::default();
    for segment in &segments {
        let leading = segment
            .iter()
            .take_while(|word| is_assignment(&word.text))
            .count();
        let echo = segment
            .get(leading)
            .is_some_and(|word| word.text == "echo" && !word.dynamic && !word.glob);
        for (index, word) in segment.iter().enumerate() {
            if word.dynamic {
                let allowed = word.simple_variable && (index < leading || echo);
                if !allowed && verdict.unverifiable.is_none() {
                    verdict.unverifiable = Some(word.text.clone());
                }
                extend_embedded(&mut verdict.candidates, &word.text);
                continue;
            }
            if word.glob {
                // A quoted or escaped glob/brace metacharacter in the same word
                // as an executed one cannot be told apart once `text` drops the
                // quoting, so the expansion proof would confine a different set
                // than the shell. Refuse rather than expand (Issue #582 review
                // blocker, design 1).
                if word.quoted_glob && verdict.unverifiable.is_none() {
                    verdict.unverifiable = Some(word.text.clone());
                }
                // A shell glob option this guard does not model (`shopt -s
                // dotglob`, `GLOBIGNORE`, `set -f`) changes which names the same
                // spelling matches, so the word cannot be proven by assuming the
                // default expansion.
                if glob_shell_option && verdict.unverifiable.is_none() {
                    verdict.unverifiable = Some(word.text.clone());
                }
                verdict
                    .candidates
                    .push(ReadCandidate::Expand(word.text.clone()));
                continue;
            }
            push_static(
                &mut verdict.candidates,
                word,
                index >= leading,
                index >= leading && !echo,
            );
            collect_relative_targets(&mut verdict.relative_words, word, index >= leading);
        }
    }
    verdict
}

/// Unions another carriage-return reading into `into`. An unverifiable word in
/// either reading is kept, and the candidates and `/`-less words are combined,
/// so the union never admits more than one reading alone.
fn merge(into: &mut ReadVerdict, other: ReadVerdict) {
    if into.unverifiable.is_none() {
        into.unverifiable = other.unverifiable;
    }
    into.candidates.extend(other.candidates);
    into.relative_words.extend(other.relative_words);
}

/// Records the `/`-less static words the caller re-checks against the
/// working-directory candidates (Issue #613 design 2). A leading assignment
/// (`NAME=value`) is a variable, not a path; `.`, `..`, an empty word, and a
/// `~`-prefixed word are excluded. The command name and options are not
/// excluded, matching the existing `cd`-relative word check; a word that is not
/// an actual outward symlink costs only one `lstat` and stays allowed.
fn collect_relative_targets(targets: &mut Vec<String>, word: &Word, is_value_word: bool) {
    if !is_value_word {
        return;
    }
    if is_relative_target(&word.text) {
        targets.push(word.text.clone());
    }
    // The right-hand side of a non-assignment `NAME=value` word names a read
    // path (`dd if=lf`, `grep --file=lf`, `make IN=lf`) even without a `/`.
    if let Some(value) = relative_equals_right_hand_side(&word.text) {
        targets.push(value.to_string());
    }
}

/// Whether a static word can name a `/`-less relative path the caller re-checks.
fn is_relative_target(word: &str) -> bool {
    !word.is_empty() && word != "." && word != ".." && !word.starts_with('~') && !word.contains('/')
}

/// The right-hand side of a `NAME=value` word when it is a non-empty, `/`-less
/// spelling. An empty or `/`-containing right side is left to the existing
/// handling.
fn relative_equals_right_hand_side(word: &str) -> Option<&str> {
    let (_, value) = word.split_once('=')?;
    (!value.is_empty() && !value.contains('/')).then_some(value)
}

/// The first `/`-less static read word that escapes the workspace when joined
/// onto a working-directory candidate (Issue #613 design 2). The literal read
/// proof runs only when the joined path is an existing symlink (or cannot be
/// `lstat`ed for a reason other than a missing name or a non-directory parent),
/// so a word that is not a symlink — an option, a display string, or an
/// `awk -F= '{print $1}'`-style quoted fragment — keeps its existing handling.
///
/// A working directory that cannot be determined is left to the existing
/// relative-candidate logic in `bash.rs`: that path already refuses the words a
/// `cd` could redirect, while this check deliberately walks only the workspace
/// root and the destinations that *can* be determined. Rejecting every
/// `/`-less word of an undecidable walk here would refuse a bare `cd` chain that
/// names no path (Issue #568 `working_directory_caps_candidate_growth`).
pub(super) fn relative_word_rejection(
    words: &[String],
    root: &Path,
    bases: &[String],
) -> Option<String> {
    for word in words {
        for base in bases {
            let joined = join_base(base, word);
            if !joined_is_symlink(root, &joined) {
                continue;
            }
            if super::super::path_guard::ensure_bash_read_target(root, &joined).is_err() {
                return Some(word.clone());
            }
        }
    }
    None
}

/// Whether the joined path is an existing symlink, or failed to `lstat` for a
/// reason other than a missing name or a non-directory ancestor.
fn joined_is_symlink(root: &Path, joined: &str) -> bool {
    let candidate = if Path::new(joined).is_absolute() {
        PathBuf::from(joined)
    } else {
        root.join(joined)
    };
    match std::fs::symlink_metadata(&candidate) {
        Ok(metadata) => metadata.file_type().is_symlink(),
        Err(error) => {
            error.kind() != std::io::ErrorKind::NotFound
                && error.raw_os_error() != Some(libc::ENOTDIR)
        }
    }
}

fn join_base(base: &str, path: &str) -> String {
    if base.is_empty() {
        path.to_string()
    } else {
        format!("{base}/{path}")
    }
}

fn push_static(
    candidates: &mut Vec<ReadCandidate>,
    word: &Word,
    equals_rhs_is_candidate: bool,
    parent_home_is_candidate: bool,
) {
    let mut produced = Vec::new();
    if word.text.starts_with('/') {
        // A quoted operator or bracket is part of the actual filename. Inspect
        // the complete absolute word before any embedded fallback.
        produced.push(ReadCandidate::Literal(word.text.clone()));
    } else if path_tokens::is_literal_path(&word.text) {
        // A literal spelling (which may contain a quoted bracket) is confined by
        // its literal form, never expanded as a glob (Issue #582 design 3).
        produced.push(ReadCandidate::Literal(word.text.clone()));
    } else {
        // A quoted or escaped single word that carries whitespace still names a
        // literal path; the embedded absolute-path scan below is kept beside it so
        // neither replaces the other (Issue #603 design 1).
        if path_tokens::is_literal_path_allowing_whitespace(&word.text) {
            produced.push(ReadCandidate::Literal(word.text.clone()));
        }
        // The right-hand side of a non-assignment `NAME=value` word names a read
        // path (`dd if=...`, `grep --file=...`, `make IN=...`). A leading
        // assignment is excluded by the caller; an absolute right side is left to
        // the embedded scan (Issue #603 design 3).
        if equals_rhs_is_candidate && let Some(value) = equals_right_hand_side(&word.text) {
            produced.push(ReadCandidate::Literal(value.to_string()));
        }
        // Issue #628: a bare `..` or a `~`-prefixed `/`-less word names the
        // parent or home directory, which the existing literal read proof
        // rejects. It carries no `/`, so it was never a literal-path candidate
        // before. The right-hand side of a non-assignment `NAME=value`
        // (`make PREFIX=~`, `cat --file=..`) is the same. A leading assignment
        // and an `echo` display segment are excluded by the caller.
        if parent_home_is_candidate && is_parent_or_home_literal(&word.text) {
            produced.push(ReadCandidate::Literal(word.text.clone()));
        }
        if parent_home_is_candidate && let Some(value) = parent_home_right_hand_side(&word.text) {
            produced.push(ReadCandidate::Literal(value.to_string()));
        }
        produced.extend(
            super::absolute_path_candidates(&word.text)
                .map(|path| ReadCandidate::Expand(path.to_owned())),
        );
    }
    produced.dedup();
    candidates.extend(produced);
}

/// Whether a static word names the parent directory (`..` exactly) or a
/// `~`-prefixed home spelling without a directory separator (Issue #628). A
/// word with a `/` keeps its existing literal-path handling.
fn is_parent_or_home_literal(word: &str) -> bool {
    word == ".." || (word.starts_with('~') && !word.contains('/'))
}

/// The right-hand side of a `NAME=value` word when it is a parent/home literal
/// (`make PREFIX=~`, `cat --file=..`); see [`is_parent_or_home_literal`]
/// (Issue #628).
fn parent_home_right_hand_side(word: &str) -> Option<&str> {
    let (_, value) = word.split_once('=')?;
    is_parent_or_home_literal(value).then_some(value)
}

/// The right-hand side of a `NAME=value` word when it is a static path spelling.
/// An empty or absolute right side yields `None`: the embedded absolute-path scan
/// already inspects an absolute one (Issue #603 design 3).
fn equals_right_hand_side(word: &str) -> Option<&str> {
    let (_, value) = word.split_once('=')?;
    if value.is_empty() || value.starts_with('/') {
        return None;
    }
    path_tokens::is_literal_path_allowing_whitespace(value).then_some(value)
}

fn extend_embedded(candidates: &mut Vec<ReadCandidate>, text: &str) {
    let mut produced: Vec<ReadCandidate> = super::absolute_path_candidates(text)
        .map(|path| ReadCandidate::Expand(path.to_owned()))
        .collect();
    produced.dedup();
    candidates.extend(produced);
}

/// Whether a word is a leading environment assignment (`NAME=value`).
fn is_assignment(word: &str) -> bool {
    let Some((name, _)) = word.split_once('=') else {
        return false;
    };
    !name.is_empty()
        && name
            .chars()
            .all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

/// Whether the command text appears to change the shell's glob behaviour. A glob
/// word under such a setting is refused rather than assumed to expand by the
/// default rules.
fn uses_glob_shell_option(text: &str) -> bool {
    text.contains("shopt")
        || text.contains("GLOBIGNORE")
        || text.contains("noglob")
        || text.contains("set -f")
        || text.contains("set +f")
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use serde::Deserialize;

    /// One saved decision row. `R` is the public second-stage predicate, `A` the
    /// recognized-mutation predicate, `W` the write-side confinement, `P` a
    /// protected-path mutation, `S` the credential scan, and `V` the verify
    /// auto-approval allowance. The same file drives the public test in
    /// `tests/issue582_bash_read_candidates.rs`.
    #[derive(Deserialize)]
    struct DecisionCase {
        id: String,
        command: String,
        #[serde(rename = "R")]
        r: bool,
        #[serde(rename = "A")]
        a: bool,
        #[serde(rename = "W")]
        w: bool,
        #[serde(rename = "P")]
        p: bool,
        #[serde(rename = "S")]
        s: bool,
        #[serde(rename = "V")]
        v: bool,
    }

    struct Fixture {
        _dir: tempfile::TempDir,
        root: PathBuf,
    }

    /// A fixture tempdir under the Cargo target directory, never under a
    /// read-allowed system prefix (`/tmp` on Linux would let an escaping symlink
    /// target be admitted as a system path; Issue #604). Cargo sets
    /// `CARGO_TARGET_TMPDIR` only for integration tests, so a unit test derives
    /// the same location from the test binary path.
    fn fixture_tempdir() -> tempfile::TempDir {
        let executable = std::env::current_exe().expect("test executable path");
        let target = executable
            .parent()
            .and_then(Path::parent)
            .expect("target profile directory");
        let base = target.join("tmp");
        std::fs::create_dir_all(&base).expect("fixture base directory");
        tempfile::Builder::new()
            .prefix("read-guard-")
            .tempdir_in(base)
            .expect("fixture tempdir")
    }

    fn fixture() -> Fixture {
        let dir = fixture_tempdir();
        let root = dir.path().join("ws");
        for directory in [
            "sub",
            "src/app/[id]",
            "node_modules/pkg",
            "frontend",
            "dir1",
            "dir2",
            // Issue #613: an inside directory whose name holds a carriage return.
            // Only the WordChar reading reaches it, and a nested outward symlink
            // below it is invisible to every other candidate.
            "cr\rsub",
        ] {
            std::fs::create_dir_all(root.join(directory)).unwrap();
        }
        for file in [
            "a.txt",
            "sub/f",
            "src/main.rs",
            "src/app/[id]/route.ts",
            "node_modules/pkg/package.json",
        ] {
            std::fs::write(root.join(file), "x").unwrap();
        }
        let outside = dir.path().join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("secret"), "x").unwrap();
        std::os::unix::fs::symlink(&outside, root.join("sub/link")).unwrap();
        // A quoted or escaped single word, and a full-width space (U+3000) or a
        // no-break space (U+00A0) inside an unquoted word, must still be judged
        // as one path (Issue #603).
        std::os::unix::fs::symlink(&outside, root.join("sub/space link")).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("sub/wide\u{3000}link")).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("sub/nb\u{a0}link")).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("linked-outside")).unwrap();
        std::os::unix::fs::symlink(&outside, root.join(".hidden-out")).unwrap();
        std::os::unix::fs::symlink(dir.path().join("missing-target"), root.join("dangling"))
            .unwrap();
        std::os::unix::fs::symlink(root.join("sub"), root.join("sub/loop")).unwrap();
        std::os::unix::fs::symlink(root.join("sub"), root.join("inlink")).unwrap();
        // Issue #613 problem 1: outward symlinks whose name holds a carriage
        // return. Bash keeps the `\r` inside the word; the historical reading
        // splits on it and misses the symlink.
        std::os::unix::fs::symlink(&outside, root.join("sub/cr\rlink")).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("cr\rdir")).unwrap();
        std::os::unix::fs::symlink(outside.join("secret"), root.join("lfcr\r")).unwrap();
        // Issue #613 problem 2: a `/`-less name can itself be an outward symlink.
        std::os::unix::fs::symlink(&outside, root.join("lf")).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("space lf")).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("wide\u{3000}lf")).unwrap();
        // Issue #613: an outward symlink inside the CR-named directory above, so
        // only the merged WordChar candidate reaches it (`cd cr␍sub && cat esc`).
        std::os::unix::fs::symlink(&outside, root.join("cr\rsub/esc")).unwrap();
        let root = root.canonicalize().unwrap();
        Fixture { _dir: dir, root }
    }

    fn protected() -> Vec<String> {
        vec!["tests/spec.rs".to_string()]
    }

    /// Issue #582 acceptance: R (confinement), A (recognized mutation), W
    /// (write-side rejection), P (protected mutation), S (secret reference), and
    /// V (verify auto-approval) are distinguished. A new read refusal has `W=0`
    /// and `A=0` (it is not a write), preserves a protected-path mutation's
    /// `P=1`, and never turns a `V=false` into `V=true`.
    #[test]
    fn read_candidates_distinguish_r_a_w_p_s_v_on_the_corpus() {
        let fixture = fixture();
        let root = &fixture.root;
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/corpus/apps/issue582-bash-read-candidates/fixtures/decision-cases.jsonl");
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        let mut seen = 0usize;
        for line in text.lines().filter(|line| !line.trim().is_empty()) {
            let case: DecisionCase = serde_json::from_str(line)
                .unwrap_or_else(|error| panic!("invalid decision case {line:?}: {error}"));
            seen += 1;
            let r = super::super::path_confinement_rejection(&case.command, root).is_some();
            let a = crate::tools::bash_write_guard::has_recognized_mutation(&case.command);
            let w = crate::tools::bash_write_guard::confinement_rejection(&case.command, root)
                .is_some();
            let p = crate::tools::bash_write_guard::protected_path_mutation(
                &case.command,
                root,
                &protected(),
            )
            .is_some();
            let s =
                crate::tools::sensitive_path::command_references_secret(&case.command).is_some();
            let v = crate::tools::allow_policy::bash_verify_command_is_auto_approvable(
                &case.command,
                root,
            );
            assert_eq!(r, case.r, "{}: R mismatch for {:?}", case.id, case.command);
            assert_eq!(a, case.a, "{}: A mismatch for {:?}", case.id, case.command);
            assert_eq!(w, case.w, "{}: W mismatch for {:?}", case.id, case.command);
            assert_eq!(p, case.p, "{}: P mismatch for {:?}", case.id, case.command);
            assert_eq!(s, case.s, "{}: S mismatch for {:?}", case.id, case.command);
            assert_eq!(v, case.v, "{}: V mismatch for {:?}", case.id, case.command);
        }
        assert!(seen >= 20, "the decision table is too small: {seen}");
    }

    /// Issue #613 design 2 "存在しない・ディレクトリでない以外の理由で lstat に
    /// 失敗したら、確かめる": an `lstat` that fails for any reason other than a
    /// missing name or a non-directory ancestor still needs the literal proof. A
    /// component longer than `NAME_MAX` fails with `ENAMETOOLONG`, and narrowing
    /// the check to `NotFound` would drop that path.
    #[test]
    fn joined_is_symlink_treats_other_lstat_failures_as_needing_proof() {
        let fixture = fixture();
        let root = &fixture.root;
        let over_long = "a".repeat(300);
        assert!(
            super::joined_is_symlink(root, &over_long),
            "an over-long name (ENAMETOOLONG) must reach the literal proof"
        );
        assert!(
            !super::joined_is_symlink(root, "no-such-name-f613"),
            "a missing name is not a symlink"
        );
        assert!(
            !super::joined_is_symlink(root, "a.txt"),
            "a regular file is not a symlink"
        );
        assert!(
            super::joined_is_symlink(root, "lf"),
            "an existing symlink needs the proof"
        );
    }

    #[test]
    fn read_candidates_keeps_static_word_metadata() {
        let parsed = super::path_tokens::read_words(
            "echo $HOME && head sub/link/secret",
            super::CrMode::Separator,
        )
        .unwrap();
        // The dynamic word and the static relative word are both present: the
        // static word is not dropped because a sibling word is dynamic.
        let words = parsed.into_iter().flatten().collect::<Vec<_>>();
        assert!(
            words
                .iter()
                .any(|word| word.dynamic && word.text == "$HOME")
        );
        assert!(words.iter().any(|word| word.text == "sub/link/secret"));
    }

    #[test]
    fn read_candidates_refuse_dynamic_but_allow_echo_and_assignment() {
        let fixture = fixture();
        let root = &fixture.root;
        assert!(super::inspect("cat $FILE").unverifiable.is_some());
        assert!(super::inspect("cat $HOME/secret").unverifiable.is_some());
        assert!(super::inspect("echo $HOME").unverifiable.is_none());
        assert!(super::inspect("echo ${HOME}").unverifiable.is_none());
        assert!(super::inspect("echo ${x:-y}").unverifiable.is_some());
        assert!(
            super::inspect("PATH=$PWD/bin:$PATH cargo test")
                .unverifiable
                .is_none()
        );
        // The static outward path beside the dynamic word is still a candidate.
        let verdict = super::inspect("echo $HOME && head sub/li\"nk/secret\"");
        assert!(verdict.unverifiable.is_none());
        assert!(
            verdict
                .candidates
                .iter()
                .any(|candidate| candidate.path() == "sub/link/secret")
        );
        assert!(
            super::super::path_confinement_rejection(
                "echo $HOME && head sub/li\"nk/secret\"",
                root
            )
            .is_some()
        );
    }

    /// The `$?` branch must copy `$?` verbatim into `Word.text`. Copying a
    /// shorter range loses the exit status, which changes the candidate the
    /// embedded absolute-path scan extracts (`/abs/x` or `/abs/x$` instead of
    /// `/abs/x$?`) and can change the decision. This is a read-only check.
    #[test]
    fn read_candidates_keep_exit_status_in_the_word_text() {
        let parsed =
            super::path_tokens::read_words("echo /abs/x$?", super::CrMode::Separator).unwrap();
        let word = &parsed[0][1];
        assert_eq!(word.text, "/abs/x$?");
        assert!(word.dynamic && word.simple_variable);

        for command in ["echo /abs/x$?", r#"echo "/abs/x$?""#] {
            let verdict = super::inspect(command);
            assert!(verdict.unverifiable.is_none(), "{command}");
            let paths: Vec<&str> = verdict
                .candidates
                .iter()
                .map(|candidate| candidate.path())
                .collect();
            assert_eq!(paths, ["/abs/x$?"], "{command}");
        }
    }

    /// Issue #603: a quoted or escaped single word that carries a space, an
    /// unquoted full-width or no-break space, and the right-hand side of a
    /// non-assignment `=` become read candidates. A leading assignment's right
    /// side does not, and a `=` option without a slash is not a path.
    #[test]
    fn read_candidates_extract_space_and_equals_candidates() {
        let paths = |command: &str| {
            super::inspect(command)
                .candidates
                .iter()
                .map(|candidate| candidate.path().to_string())
                .collect::<Vec<_>>()
        };
        assert!(
            paths(r#"cat "sub/space link/secret""#).contains(&"sub/space link/secret".to_string())
        );
        assert!(
            paths("cat sub/wide\u{3000}link/secret")
                .contains(&"sub/wide\u{3000}link/secret".to_string())
        );
        assert!(
            paths("cat sub/nb\u{a0}link/secret").contains(&"sub/nb\u{a0}link/secret".to_string())
        );
        assert!(paths("dd if=sub/link/secret").contains(&"sub/link/secret".to_string()));
        assert!(paths("grep --file=sub/link/secret x").contains(&"sub/link/secret".to_string()));
        assert!(paths("make IN=sub/link/secret").contains(&"sub/link/secret".to_string()));
        // A leading assignment is a variable, not a path; the reference side is
        // judged elsewhere, so its right side is not a read candidate here.
        assert!(paths("OUT=out/x cargo test").is_empty());
        assert!(paths("RUST_LOG=debug cargo test").is_empty());
        // An option right side without a slash is not a path.
        assert!(paths("cargo test --features=gui").is_empty());
    }
}
