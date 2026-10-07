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

use super::path_tokens::{self, Word};

/// The reason recorded for a read word whose expanded value cannot be proven to
/// remain in the workspace.
pub(super) const UNVERIFIABLE_REASON: &str =
    "is a read argument whose expanded value cannot be proven to remain in the workspace";

/// The read-path candidates and the first unverifiable word for one command.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct ReadVerdict {
    /// A word whose value cannot be determined statically. The caller refuses.
    pub unverifiable: Option<String>,
    /// Literal path spellings and supported glob/brace words to confine with the
    /// existing write-target proof and the working-directory candidate union.
    pub candidates: Vec<String>,
}

/// Reads the read-path judgement for a command the write guard allowed.
pub(super) fn inspect(command: &str) -> ReadVerdict {
    // Read the elided text exactly as the write guard does; a command it allowed
    // has readable text, so the raw fallback is only a belt-and-braces guard.
    let elided = super::super::shell_lexical::strip_comments_and_heredocs(command);
    let text = elided.as_deref().unwrap_or(command);
    let Some(segments) = path_tokens::read_words(text) else {
        return ReadVerdict {
            unverifiable: Some("<shell text>".to_string()),
            candidates: Vec::new(),
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
                // A shell glob option this guard does not model (`shopt -s
                // dotglob`, `GLOBIGNORE`, `set -f`) changes which names the same
                // spelling matches, so the word cannot be proven by assuming the
                // default expansion.
                if glob_shell_option && verdict.unverifiable.is_none() {
                    verdict.unverifiable = Some(word.text.clone());
                }
                verdict.candidates.push(word.text.clone());
                continue;
            }
            push_static(&mut verdict.candidates, word);
        }
    }
    verdict
}

fn push_static(candidates: &mut Vec<String>, word: &Word) {
    let mut produced = Vec::new();
    if word.text.starts_with('/') {
        // A quoted operator or bracket is part of the actual filename. Inspect
        // the complete absolute word before any embedded fallback.
        produced.push(word.text.clone());
    } else if path_tokens::is_literal_path(&word.text) {
        produced.push(word.text.clone());
    } else {
        produced.extend(super::absolute_path_candidates(&word.text).map(str::to_owned));
    }
    produced.dedup();
    candidates.extend(produced);
}

fn extend_embedded(candidates: &mut Vec<String>, text: &str) {
    let mut produced: Vec<String> = super::absolute_path_candidates(text)
        .map(str::to_owned)
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

    fn fixture() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("ws");
        for directory in [
            "sub",
            "src/app/[id]",
            "node_modules/pkg",
            "frontend",
            "dir1",
            "dir2",
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
        std::os::unix::fs::symlink(&outside, root.join("linked-outside")).unwrap();
        std::os::unix::fs::symlink(&outside, root.join(".hidden-out")).unwrap();
        std::os::unix::fs::symlink(dir.path().join("missing-target"), root.join("dangling"))
            .unwrap();
        std::os::unix::fs::symlink(root.join("sub"), root.join("sub/loop")).unwrap();
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

    #[test]
    fn read_candidates_keeps_static_word_metadata() {
        let parsed = super::path_tokens::read_words("echo $HOME && head sub/link/secret").unwrap();
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
        assert!(verdict.candidates.iter().any(|c| c == "sub/link/secret"));
        assert!(
            super::super::path_confinement_rejection(
                "echo $HOME && head sub/li\"nk/secret\"",
                root
            )
            .is_some()
        );
    }
}
