//! Working-directory candidates for the Bash write-target inspector (Issue #568).
//!
//! The inspector judges a relative path against the workspace root, but a
//! command may `cd`/`pushd` first and the shell then resolves that relative path
//! against the changed directory. When the destination is a symlink that leaves
//! the workspace, a root-relative judgement misses it and the write (or read)
//! escapes.
//!
//! [`inspect`] walks the lexed tokens and builds the set of working directories
//! the command could run in. It starts at the workspace root and *adds* each
//! determinable `cd`/`pushd` destination instead of replacing, so a subshell, a
//! pipeline, or a `||` branch that keeps the old directory cannot hide the root
//! candidate. A destination whose value cannot be read statically (`$`,
//! backquotes, `~`, a glob, `..`, `-`, or an empty `cd`) adds nothing. A command
//! that mixes a `cd` with `CDPATH`, a loop, or a function definition is flagged
//! `undecidable`, and the caller refuses its relative writes.
//!
//! The lexical analysis (`shell_tokens`, `ShellToken`) and the prefix
//! `Resolution` are not changed here: this module only reads their output.

use std::collections::HashSet;
use std::path::Path;

use super::command_prefix::{self, Resolution};
use super::{ShellToken, command_words, positional_operands};

/// Upper bound on the working-directory candidates. Consecutive distinct `cd`
/// destinations can double the set at each step, so past this bound the
/// destination is treated as undeterminable instead of expanded further.
const MAX_CANDIDATES: usize = 64;

/// The working-directory candidates and derived checks for one command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Inspection {
    /// Candidate working directories. `""` is the workspace root; every other
    /// entry is a workspace-relative path, or an absolute path for an absolute
    /// `cd`/`pushd` destination.
    pub(crate) bases: Vec<String>,
    /// `cd`/`pushd` destinations joined onto the candidates that preceded them,
    /// to be confined exactly like a write target.
    pub(crate) targets: Vec<String>,
    /// A `cd`/`pushd`/`popd` destination cannot be determined: `CDPATH` or
    /// `DIRSTACK` can redirect it, a loop or function definition can repeat or
    /// defer it, or its candidate set passed [`MAX_CANDIDATES`]. Relative paths
    /// after it must be refused.
    pub(crate) undecidable: bool,
    /// Literal relative read words (from segments that are not a working
    /// directory change). The second stage of the inspector joins these onto
    /// every candidate.
    pub(crate) relative_words: Vec<String>,
}

impl Inspection {
    /// Whether a `cd`/`pushd` added a candidate beyond the workspace root.
    pub(crate) fn has_working_directory(&self) -> bool {
        self.bases.iter().any(|base| !base.is_empty())
    }

    /// The base-joined forms of a relative path, excluding the root form that
    /// the caller already judged on its own.
    pub(crate) fn relative_variants(&self, path: &str) -> Vec<String> {
        self.bases
            .iter()
            .filter(|base| !base.is_empty())
            .map(|base| join(base, path))
            .collect()
    }

    /// The base-joined forms of a relative path, including the root form.
    pub(crate) fn all_variants(&self, path: &str) -> Vec<String> {
        self.bases.iter().map(|base| join(base, path)).collect()
    }
}

/// Reads the working-directory candidates out of a command.
pub(crate) fn inspect(command: &str) -> Inspection {
    let mut inspection = Inspection {
        bases: vec![String::new()],
        targets: Vec::new(),
        undecidable: false,
        relative_words: Vec::new(),
    };
    let Some(text) = super::super::shell_lexical::strip_comments_and_heredocs(command) else {
        // The shell text cannot be read, so no working-directory change can be
        // modelled; relative writes after it must be refused (Issue #576).
        inspection.undecidable = true;
        return inspection;
    };
    let Some(tokens) = super::shell_tokens(&text) else {
        return inspection;
    };

    let mut has_cd = false;
    let mut cdpath = false;
    let mut dirstack = false;
    let mut loop_keyword = false;
    let mut function_keyword = false;
    for word in tokens.iter().filter_map(word_token) {
        if word.contains("CDPATH") {
            cdpath = true;
        }
        if word.contains("DIRSTACK") {
            dirstack = true;
        }
        if matches!(word, "for" | "while" | "until" | "select") {
            loop_keyword = true;
        }
        if word == "function" {
            function_keyword = true;
        }
    }
    if text.contains("()") {
        function_keyword = true;
    }

    let mut overflow = false;
    for segment in tokens.split(|token| *token == ShellToken::SegmentEnd) {
        let words = command_words(segment);
        if words.is_empty() {
            continue;
        }
        let resolution = command_prefix::resolve(&words);
        // A working-directory change is consumed by the candidate walk below;
        // its program and operands are not reads, so they do not feed the
        // second-stage read check.
        let cwd_program = matches!(
            &resolution,
            Resolution::Program { program, .. }
                if matches!(program.as_str(), "cd" | "pushd" | "popd")
        );
        if !cwd_program {
            for word in &words {
                if is_relative_word(word) {
                    inspection.relative_words.push((*word).to_string());
                }
            }
        }
        let Resolution::Program {
            program, arguments, ..
        } = resolution
        else {
            continue;
        };
        let arguments: Vec<&str> = arguments.iter().map(String::as_str).collect();
        match program.as_str() {
            "cd" => {
                has_cd = true;
                if let Some(target) = positional_operands(&arguments).first().copied()
                    && !overflow
                    && let Some(joined) = extend(&mut inspection.bases, target)
                {
                    inspection.targets.extend(joined);
                    overflow = inspection.bases.len() > MAX_CANDIDATES;
                }
            }
            "pushd" => {
                has_cd = true;
                let Some(target) = positional_operands(&arguments).first().copied() else {
                    continue;
                };
                // `pushd` with no operand and `pushd ±N` only rotate the
                // directory stack, which holds nothing beyond the candidates,
                // so they add no candidate.
                if target.starts_with('+') || target.starts_with('-') {
                    continue;
                }
                if !overflow && let Some(joined) = extend(&mut inspection.bases, target) {
                    inspection.targets.extend(joined);
                    overflow = inspection.bases.len() > MAX_CANDIDATES;
                }
            }
            "popd" => {
                has_cd = true;
            }
            _ => {}
        }
    }

    inspection.undecidable =
        cdpath || dirstack || overflow || (has_cd && (loop_keyword || function_keyword));
    inspection
}

fn word_token(token: &ShellToken) -> Option<&str> {
    match token {
        ShellToken::Word(word) => Some(word.as_str()),
        _ => None,
    }
}

/// Whether a `cd`/`pushd` destination is a literal path the inspector can join
/// onto the current candidates.
fn is_determinable(target: &str) -> bool {
    !target.is_empty()
        && target != "-"
        && !target.contains(['$', '`', '~', '*', '?', '[', ']', '{', '}'])
        && !target.split('/').any(|component| component == "..")
}

/// Adds the candidates a destination produces and returns them for the caller
/// to confine. A destination that cannot be determined adds nothing.
fn extend(bases: &mut Vec<String>, target: &str) -> Option<Vec<String>> {
    if !is_determinable(target) {
        return None;
    }
    let joined: Vec<String> = if Path::new(target).is_absolute() {
        vec![target.to_string()]
    } else {
        bases.iter().map(|base| join(base, target)).collect()
    };
    let mut seen: HashSet<String> = bases.iter().cloned().collect();
    for path in &joined {
        if seen.insert(path.clone()) {
            bases.push(path.clone());
        }
    }
    Some(joined)
}

fn join(base: &str, path: &str) -> String {
    if base.is_empty() {
        path.to_string()
    } else {
        format!("{base}/{path}")
    }
}

/// Whether a word can be a literal relative path operand: no directory
/// separator, no expansion, no option, no assignment, and no whitespace.
fn is_relative_word(word: &str) -> bool {
    !word.is_empty()
        && word != "."
        && word != ".."
        && !word.starts_with('-')
        && !word.contains('/')
        && !word.contains(['$', '`', '~', '*', '?', '[', ']', '{', '}', '=', '\\'])
        && !word.chars().any(char::is_whitespace)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bases(command: &str) -> Vec<String> {
        inspect(command).bases
    }

    #[test]
    fn working_directory_candidates_start_at_the_root() {
        let inspection = inspect("tee out.txt");
        assert_eq!(inspection.bases, vec![String::new()]);
        assert!(!inspection.has_working_directory());
        assert!(!inspection.undecidable);
        assert!(inspection.targets.is_empty());
    }

    #[test]
    fn working_directory_adds_determinable_destinations() {
        assert_eq!(bases("cd sub"), vec!["", "sub"]);
        assert_eq!(bases("cd -P sub"), vec!["", "sub"]);
        assert_eq!(bases("cd -- sub"), vec!["", "sub"]);
        assert_eq!(bases("cd ./sub"), vec!["", "./sub"]);
        assert_eq!(bases("cd sub/"), vec!["", "sub/"]);
        assert_eq!(bases("builtin cd sub"), vec!["", "sub"]);
        assert_eq!(bases("env cd sub"), vec!["", "sub"]);
        assert_eq!(bases("pushd sub"), vec!["", "sub"]);
        assert_eq!(bases("pushd -n sub"), vec!["", "sub"]);
    }

    #[test]
    fn working_directory_appends_instead_of_replacing() {
        assert_eq!(bases("cd a && cd b"), vec!["", "a", "b", "a/b"]);
        assert_eq!(bases("cd sub; cd sub"), vec!["", "sub", "sub/sub"]);
    }

    #[test]
    fn working_directory_records_targets_to_confine() {
        let inspection = inspect("cd sub && cd link");
        assert_eq!(inspection.targets, vec!["sub", "link", "sub/link"]);
    }

    #[test]
    fn working_directory_skips_undeterminable_destinations() {
        for command in [
            "cd", "cd ~", "cd ~/x", "cd ..", "cd ../x", "cd $VAR", "cd -",
        ] {
            let inspection = inspect(command);
            assert!(
                inspection.bases.iter().all(|base| base.is_empty()),
                "no candidate expected for `{command}`: {:?}",
                inspection.bases
            );
        }
        // An absolute destination is kept whole, not joined onto the root.
        assert_eq!(inspect("cd /tmp").bases, vec!["", "/tmp"]);
    }

    #[test]
    fn working_directory_pushd_rotations_do_not_add_candidates() {
        assert_eq!(bases("pushd +1"), vec![""]);
        assert_eq!(bases("pushd"), vec![""]);
        assert_eq!(bases("popd"), vec![""]);
    }

    #[test]
    fn working_directory_marks_cdpath_loops_and_functions_undecidable() {
        assert!(inspect("CDPATH=sub cd link && tee f").undecidable);
        assert!(inspect("export CDPATH=sub; cd link; tee f").undecidable);
        assert!(inspect("for i in 1; do cd sub; done; tee f").undecidable);
        assert!(inspect("f() { cd sub; }; f; tee f").undecidable);
        assert!(inspect("function f { cd sub; }").undecidable);
        assert!(!inspect("cd sub && tee link/f").undecidable);
        assert!(!inspect("for i in 1; do tee f; done").undecidable);
        assert!(!inspect("tee f").undecidable);
    }

    #[test]
    fn working_directory_collects_literal_relative_words() {
        let inspection = inspect("cd sub && grep -r x link");
        assert!(inspection.relative_words.contains(&"link".to_string()));
        assert!(!inspection.relative_words.contains(&"$VAR".to_string()));
        assert!(!inspection.relative_words.contains(&"-r".to_string()));
        assert!(!inspection.relative_words.contains(&"a/b".to_string()));
    }

    #[test]
    fn working_directory_marks_dirstack_undecidable() {
        assert!(inspect("pushd . ; DIRSTACK[1]=sub/link; popd; tee f").undecidable);
        assert!(inspect("DIRSTACK[1]=linked-outside; pushd +1").undecidable);
        assert!(!inspect("tee f").undecidable);
    }

    #[test]
    fn working_directory_marks_popd_loop_undecidable() {
        assert!(inspect("for i in 1; do popd; done; tee f").undecidable);
    }

    #[test]
    fn working_directory_caps_candidate_growth() {
        let command = (0..40)
            .map(|index| format!("cd d{index}"))
            .collect::<Vec<_>>()
            .join("; ");
        let inspection = inspect(&command);
        assert!(
            inspection.undecidable,
            "past the cap the destination is not determinable"
        );
        assert!(
            inspection.bases.len() <= MAX_CANDIDATES * 2,
            "candidate growth must stop at the cap: {}",
            inspection.bases.len()
        );
    }

    #[test]
    fn working_directory_keeps_cwd_words_out_of_reads() {
        let inspection = inspect("cd sub && cat secret");
        assert!(!inspection.relative_words.contains(&"cd".to_string()));
        assert!(!inspection.relative_words.contains(&"sub".to_string()));
        assert!(inspection.relative_words.contains(&"secret".to_string()));
        assert!(inspect("cd d0; cd d1").relative_words.is_empty());
    }

    #[test]
    fn working_directory_variants_join_every_candidate() {
        let inspection = inspect("cd sub");
        assert_eq!(
            inspection.relative_variants("link/secret"),
            vec!["sub/link/secret"]
        );
        assert_eq!(
            inspection.all_variants("link"),
            vec!["link".to_string(), "sub/link".to_string()]
        );
    }
}
