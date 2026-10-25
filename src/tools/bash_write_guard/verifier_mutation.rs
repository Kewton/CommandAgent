//! Explicit workspace-update commands that must never count as verifier
//! evidence (Issue #861).
//!
//! A Verify or Report step runs Bash to observe the workspace, not to change
//! it. [`find`] names the first explicit update command in a command text. It
//! reuses the parent's lexer, segment split, and command-prefix peeling, so a
//! quoted program name, an absolute program path, an `env`/`command`/`timeout`
//! prefix, and a writer behind `&&`, `;`, `|`, or a newline are all read the
//! way the write guard reads them.
//!
//! Recognized update grammar (the only forms this classifier proves):
//!
//! | form | recognized as |
//! | --- | --- |
//! | the parent's write table (`rm`, `mv`, `cp`, `install`, `ln`, `tee`, `mkdir`, `touch`, `truncate`, `dd of=`, `chmod`, `chown`, `sudoedit`) and output redirections | the parent's operation name |
//! | `sed` / `gsed` with `-i`, `-i<suffix>`, `-I`, a short cluster holding `i` (`-ni`, `-Ei`), or `--in-place[=<suffix>]` (any unambiguous abbreviation) | `sed -i` |
//! | `sed -l` (BSD flag, no value) does not hide a later `-i`; an unresolvable prefix spelling (`timeout -sKILL 5`, `nice -n5`) before `sed`/`gsed`/`perl` is refused | `sed -i` / `perl -i` (fail closed) |
//! | `perl` with `-i`, `-i<suffix>`, or a switch cluster holding `i` (`-pi`, `-0777pi`) | `perl -i` |
//!
//! Anything else is not proven read-only by this table. In particular the sed
//! `w` and `e` commands, a perl program body, inline interpreters
//! (`node -e`, `python -c`), `sh -c`, `bash -c`, `eval`, `xargs`, `find -exec`, `busybox`, package scripts, and compiler hooks can still
//! write; they belong to a separate design, and a command that avoids the
//! table above is not thereby "known read-only".

use super::command_prefix::{self, Resolution};
use super::{CrMode, ShellToken, command_words, lex_shell, shell_lexical, write_targets};

#[cfg(test)]
mod tests;

/// One explicit update command found in a command text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VerifierMutation {
    pub(crate) operation: String,
    pub(crate) target: Option<String>,
}

impl VerifierMutation {
    /// A short description for a rejection reason.
    pub(crate) fn describe(&self) -> String {
        match &self.target {
            Some(target) => format!("`{}` of `{target}`", self.operation),
            None => format!("`{}`", self.operation),
        }
    }
}

/// Names the first explicit update command in `command`.
///
/// `allow_null_sink` skips a redirection, `tee`, or `dd` whose target is
/// `/dev/null`, the same exemption the workspace-boundary guard grants. The
/// `--allow bash:verify` ceiling passes `false` so it stays as strict as the
/// recognized-mutation table it replaces.
pub(crate) fn find(command: &str, allow_null_sink: bool) -> Option<VerifierMutation> {
    let table_hit = write_targets(command).into_iter().find(|target| {
        target.operation != "working directory"
            && !(allow_null_sink
                && target.path == "/dev/null"
                && matches!(
                    target.operation.as_str(),
                    "output redirection" | "tee" | "dd"
                ))
    });
    if let Some(target) = table_hit {
        return Some(VerifierMutation {
            operation: target.operation,
            target: Some(target.path),
        });
    }
    in_place_edit(command)
}

/// Finds a sed or perl in-place edit in any segment of `command`.
fn in_place_edit(command: &str) -> Option<VerifierMutation> {
    // Anything the lexer cannot read, or that carries ANSI-C quoting, was
    // already named by the write table above.
    let view = shell_lexical::write_guard_view(command)?;
    let mut modes = vec![CrMode::Separator];
    if view.text.contains('\r') {
        modes.push(CrMode::WordChar);
    }
    for mode in modes {
        let Some(lexed) = lex_shell(&view.text, mode) else {
            continue;
        };
        for segment in lexed.tokens.split(|token| *token == ShellToken::SegmentEnd) {
            let words = command_words(segment);
            if words.is_empty() {
                continue;
            }
            let (program, arguments) = match command_prefix::resolve(&words) {
                Resolution::Program {
                    program, arguments, ..
                } => (program, arguments),
                // An unresolvable prefix spelling (`timeout -sKILL 5`,
                // `nice -n5`) hides which program runs, and the parent reports
                // it as `NoExecution` unless a write program follows. A sed or
                // perl among the words therefore fails closed (so does the
                // rare `command -v sed`). The env -S case stays the parent's.
                Resolution::Undecidable { .. } | Resolution::NoExecution { .. } => {
                    if let Some(word) = words.iter().find(|word| is_in_place_program(word)) {
                        return Some(VerifierMutation {
                            operation: "unresolved command prefix before sed/perl".to_string(),
                            target: Some((*word).to_string()),
                        });
                    }
                    continue;
                }
                Resolution::UnverifiableSplitString { .. } => continue,
            };
            if let Some(operation) = in_place_operation(&program, &arguments) {
                return Some(VerifierMutation {
                    operation: operation.to_string(),
                    target: None,
                });
            }
        }
    }
    None
}

fn in_place_operation(program: &str, arguments: &[String]) -> Option<&'static str> {
    if matches!(program, "sed" | "gsed") && sed_in_place(arguments) {
        return Some("sed -i");
    }
    if is_perl(program) && perl_in_place(arguments) {
        return Some("perl -i");
    }
    None
}

fn is_in_place_program(word: &str) -> bool {
    let name = word.rsplit('/').next().unwrap_or(word);
    matches!(name, "sed" | "gsed") || is_perl(name)
}

fn is_perl(program: &str) -> bool {
    program == "perl"
        || program
            .strip_prefix("perl")
            .is_some_and(|rest| rest.starts_with(|ch: char| ch.is_ascii_digit()))
}

/// GNU and BSD sed in-place switches. Options may follow the script, so the
/// whole argument list is read up to `--`. The arguments of `-e`, `-f`, and
/// `--line-length` (and the long forms) are skipped so a script that spells
/// `-i` is not read as a switch. A short `-l` takes no value: BSD sed uses it as
/// a flag, and GNU's `-l 80` leaves `80` as a harmless operand.
fn sed_in_place(arguments: &[String]) -> bool {
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index].as_str();
        index += 1;
        if argument == "--" {
            break;
        }
        if let Some(long) = argument.strip_prefix("--") {
            let name = long.split('=').next().unwrap_or_default();
            if name.is_empty() {
                continue;
            }
            if "in-place".starts_with(name) {
                return true;
            }
            let takes_value = ["expression", "file", "line-length"]
                .iter()
                .any(|option| option.starts_with(name));
            if takes_value && !long.contains('=') {
                index += 1;
            }
            continue;
        }
        let Some(cluster) = argument.strip_prefix('-').filter(|rest| !rest.is_empty()) else {
            continue;
        };
        for (position, ch) in cluster.char_indices() {
            match ch {
                'i' | 'I' => return true,
                'e' | 'f' => {
                    if position + ch.len_utf8() == cluster.len() {
                        index += 1;
                    }
                    break;
                }
                _ => {}
            }
        }
    }
    false
}

/// Perl switch clusters up to the first non-switch word. `-i[ext]` takes the
/// rest of its cluster, so `-pi.bak` and `-0777pi` are both in-place. A switch
/// that takes the rest of the cluster as its value ends the cluster.
fn perl_in_place(arguments: &[String]) -> bool {
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index].as_str();
        index += 1;
        if argument == "--" || argument == "-" {
            break;
        }
        let Some(cluster) = argument.strip_prefix('-') else {
            break;
        };
        if cluster.starts_with('-') {
            continue;
        }
        let chars: Vec<char> = cluster.chars().collect();
        let mut position = 0;
        while position < chars.len() {
            let ch = chars[position];
            position += 1;
            match ch {
                'i' => return true,
                'e' | 'E' | 'I' => {
                    if position == chars.len() {
                        index += 1;
                    }
                    break;
                }
                'C' | 'd' | 'D' | 'F' | 'm' | 'M' | 'V' | 'x' => break,
                '0' | 'l' => {
                    // `-0[octal|xHEX]` and `-l[octal]` take digits, not the
                    // rest of the cluster.
                    let hex = ch == '0' && matches!(chars.get(position), Some('x' | 'X'));
                    if hex {
                        position += 1;
                    }
                    while chars.get(position).is_some_and(|next| {
                        if hex {
                            next.is_ascii_hexdigit()
                        } else {
                            next.is_ascii_digit()
                        }
                    }) {
                        position += 1;
                    }
                }
                _ => {}
            }
        }
    }
    false
}
