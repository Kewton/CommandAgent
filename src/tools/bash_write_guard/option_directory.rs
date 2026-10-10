//! Programs that name a working directory in an option value (Issue #637).
//!
//! The working-directory walk in [`super::working_directory`] reads the `cd`,
//! `pushd`, and `popd` destinations, but a program can change its own working
//! directory through an option: `tar -C DIR`, `git -C DIR`, `make -C DIR`,
//! `env -C DIR`, `sudo -D DIR`, and others. Those segments resolved to no
//! candidate, so a relative path after them (`tar -C sub -cf - lf2`) was judged
//! against the workspace root alone and an outward symlink inside the option's
//! directory escaped.
//!
//! This leaf module holds only the table and the value extraction; the walk
//! wires each value into the same candidate/target/mark handling a `cd`
//! destination gets. It reads the raw segment words rather than the resolved
//! program, because `env -C` and `sudo -D` never resolve to a program (the
//! prefix peel fails closed on them) and because a table program can sit behind
//! a prefix (`nice env -C DIR`, `sudo nice -C DIR`).
//!
//! # Table
//!
//! Each program is matched by the basename of a word, with its short option
//! character and long option names (without the leading `--`):
//!
//! | program | short | long |
//! |---|---|---|
//! | `tar`, `gtar`, `bsdtar` | `-C` | `--directory` |
//! | `make`, `gmake` | `-C` | `--directory` |
//! | `git` | `-C` | (global options before the subcommand only) |
//! | `env` | `-C` | `--chdir` |
//! | `sudo` | `-D` | `--chdir` |
//! | `ninja`, `go`, `cargo` | `-C` | |
//! | `pnpm` | `-C` | `--dir` |
//! | `poetry` | `-C` | `--directory` |
//! | `yarn`, `bun` | | `--cwd` |
//! | `npm` | | `--prefix` |
//! | `uv` | | `--directory` |
//! | `just` | `-d` | `--working-directory` |
//! | `patch` | `-d` | `--directory` |
//!
//! # Value forms
//!
//! `-C dir`, `-Cdir` (a short-option cluster: the value is the rest of the
//! word when the option is not last, otherwise the next word), `--directory=dir`,
//! and `--directory dir`. Nothing after a `--` word is read. `git -C` is a
//! global option only before the subcommand, so `git commit -C HEAD` names no
//! directory.
//!
//! # Programs not in the table
//!
//! A program that is not listed is read exactly as before: its option values
//! are not treated as working directories. `find -execdir`, whose working
//! directory is a different mechanism, and the options of unlisted programs are
//! out of this issue's scope.

use std::path::Path;

/// One program's option spellings that carry a working-directory value.
struct Entry {
    program: &'static str,
    short: Option<char>,
    long: &'static [&'static str],
    /// When true, only the options before the first operand (the subcommand)
    /// are global (`git -C`); a later `-C` belongs to the subcommand.
    stop_at_subcommand: bool,
}

const fn entry(
    program: &'static str,
    short: Option<char>,
    long: &'static [&'static str],
    stop_at_subcommand: bool,
) -> Entry {
    Entry {
        program,
        short,
        long,
        stop_at_subcommand,
    }
}

/// The table of programs whose option value names a working directory.
const TABLE: &[Entry] = &[
    entry("tar", Some('C'), &["directory"], false),
    entry("gtar", Some('C'), &["directory"], false),
    entry("bsdtar", Some('C'), &["directory"], false),
    entry("make", Some('C'), &["directory"], false),
    entry("gmake", Some('C'), &["directory"], false),
    entry("git", Some('C'), &[], true),
    entry("env", Some('C'), &["chdir"], false),
    entry("sudo", Some('D'), &["chdir"], false),
    entry("ninja", Some('C'), &[], false),
    entry("go", Some('C'), &[], false),
    entry("cargo", Some('C'), &[], false),
    entry("pnpm", Some('C'), &["dir"], false),
    entry("poetry", Some('C'), &["directory"], false),
    entry("yarn", None, &["cwd"], false),
    entry("bun", None, &["cwd"], false),
    entry("npm", None, &["prefix"], false),
    entry("uv", None, &["directory"], false),
    entry("just", Some('d'), &["working-directory"], false),
    entry("patch", Some('d'), &["directory"], false),
];

/// The working-directory values the table programs name in one segment's words,
/// in the order they appear. A word whose basename matches a table program is
/// scanned; a table program behind a prefix (`nice env -C DIR`) is found the
/// same way. Values are returned with their spelling as the shell builds the
/// word (quotes removed, escapes resolved).
pub(super) fn directories(words: &[&str]) -> Vec<String> {
    let mut values = Vec::new();
    for (index, word) in words.iter().enumerate() {
        let name = basename(word);
        let Some(entry) = TABLE.iter().find(|entry| entry.program == name) else {
            continue;
        };
        collect(entry, &words[index + 1..], &mut values);
    }
    values
}

/// Reads the values `entry`'s options name from its argument words.
fn collect(entry: &Entry, arguments: &[&str], values: &mut Vec<String>) {
    let mut index = 0usize;
    while index < arguments.len() {
        let word = arguments[index];
        if word == "--" {
            break;
        }
        if let Some(rest) = word.strip_prefix("--") {
            let (name, inline) = split_long(rest);
            if !entry.long.contains(&name.as_str()) {
                index += 1;
                continue;
            }
            match inline {
                Some(value) => {
                    values.push(value);
                    index += 1;
                }
                None => {
                    let Some(value) = arguments.get(index + 1) else {
                        break;
                    };
                    values.push((*value).to_string());
                    index += 2;
                }
            }
            continue;
        }
        if word.len() > 1 && word.starts_with('-') {
            if let Some(option) = entry.short
                && let Some(position) = word[1..].find(option)
            {
                let remainder = &word[1 + position + option.len_utf8()..];
                if !remainder.is_empty() {
                    values.push(remainder.to_string());
                    index += 1;
                    continue;
                }
                let Some(value) = arguments.get(index + 1) else {
                    break;
                };
                values.push((*value).to_string());
                index += 2;
                continue;
            }
            index += 1;
            continue;
        }
        // A non-option word. Only `git`'s global options end at the subcommand;
        // other table programs keep scanning their operands for an option.
        if entry.stop_at_subcommand {
            break;
        }
        index += 1;
    }
}

/// Splits a long option body into its name and an attached `=value`.
fn split_long(rest: &str) -> (String, Option<String>) {
    match rest.split_once('=') {
        Some((name, value)) => (name.to_string(), Some(value.to_string())),
        None => (rest.to_string(), None),
    }
}

fn basename(word: &str) -> &str {
    Path::new(word)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(word)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values(words: &[&str]) -> Vec<String> {
        directories(words)
    }

    #[test]
    fn reads_short_values_in_both_forms() {
        assert_eq!(values(&["tar", "-C", "sub"]), vec!["sub"]);
        assert_eq!(values(&["tar", "-Csub"]), vec!["sub"]);
        // The option is not first in the cluster: the remainder is the value.
        assert_eq!(values(&["tar", "-xCsub"]), vec!["sub"]);
    }

    #[test]
    fn reads_long_values_in_both_forms() {
        assert_eq!(values(&["tar", "--directory=sub"]), vec!["sub"]);
        assert_eq!(values(&["tar", "--directory", "sub"]), vec!["sub"]);
        assert_eq!(values(&["pnpm", "--dir", "sub"]), vec!["sub"]);
        assert_eq!(values(&["yarn", "--cwd", "sub"]), vec!["sub"]);
        assert_eq!(values(&["npm", "--prefix", "sub"]), vec!["sub"]);
    }

    #[test]
    fn ignores_unknown_options_and_end_of_options() {
        // A cluster with no `C` names no directory.
        assert_eq!(values(&["tar", "-cf", "-", "f"]), Vec::<String>::new());
        // Nothing after `--` is read.
        assert_eq!(values(&["tar", "--", "-C", "sub"]), Vec::<String>::new());
        assert_eq!(values(&["tar", "-C"]), Vec::<String>::new());
    }

    #[test]
    fn git_reads_only_global_options_before_the_subcommand() {
        assert_eq!(values(&["git", "-C", "sub", "status"]), vec!["sub"]);
        assert_eq!(
            values(&["git", "-C", "sub", "-C", "deep", "log"]),
            vec!["sub".to_string(), "deep".to_string()]
        );
        // `-C` after the subcommand belongs to the subcommand.
        assert_eq!(
            values(&["git", "commit", "-C", "HEAD"]),
            Vec::<String>::new()
        );
    }

    #[test]
    fn finds_a_table_program_behind_a_prefix() {
        assert_eq!(values(&["nice", "env", "-C", "sub", "cat"]), vec!["sub"]);
        assert_eq!(values(&["sudo", "make", "-C", "sub"]), vec!["sub"]);
        // `sudo -D` is read even though the prefix peel fails closed on it.
        assert_eq!(values(&["sudo", "-D", "sub", "cat"]), vec!["sub"]);
    }

    #[test]
    fn ignores_programs_not_in_the_table() {
        assert_eq!(values(&["fltr", "-C", "sub"]), Vec::<String>::new());
        assert_eq!(values(&["python3", "-C", "sub"]), Vec::<String>::new());
    }

    #[test]
    fn matches_a_program_by_its_basename() {
        assert_eq!(values(&["/usr/bin/tar", "-C", "sub"]), vec!["sub"]);
    }
}
