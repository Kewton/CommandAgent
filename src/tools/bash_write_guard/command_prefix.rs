//! Command-prefix peeling for the Bash write-target inspector (Issue #566).
//!
//! The write target of a shell segment belongs to the program that actually
//! runs, not to the first word of the segment. Prefix programs (`env`, `sudo`,
//! `timeout`, `nice`, reserved words such as `{` and `if`, ...) sit in front of
//! that program, so the inspector peels them before it decides which program's
//! write table applies.
//!
//! [`resolve`] applies the table repeatedly until a fixpoint and returns one of
//! three outcomes: a resolved program, "nothing executes", or "the chain cannot
//! be resolved". An unresolvable chain fails closed: [`resolve`] scans the
//! remaining words and reports the chain as unresolvable only when that scan
//! finds a write program or another prefix. An ordinary verification command
//! such as `timeout 600 cargo test` therefore stays allowed.

use std::path::Path;

use super::is_environment_assignment;

/// Upper bound on how many prefixes are peeled before the chain is treated as
/// unresolvable. A deeper chain is a fail-closed case, never an unbounded walk.
const MAX_DEPTH: usize = 16;

/// Operation recorded for a segment whose prefix chain could not be resolved
/// while a write program or another prefix remained in the words.
pub(super) const UNRESOLVED_OPERATION: &str = "unresolved command prefix";

/// Operation recorded for a segment whose `env` option selects `-S` /
/// `--split-string`, whose command string cannot be verified.
pub(super) const UNVERIFIABLE_SPLIT_STRING_OPERATION: &str = "env -S / --split-string";

/// A write destination named by a prefix itself rather than by the program it
/// launches, e.g. `time -o FILE` or `sudo -e FILE`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PrefixWrite {
    pub path: String,
    pub operation: String,
}

/// Outcome of peeling the prefixes of a single segment's words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Resolution {
    /// A program runs; `arguments` are the words after it and `writes` are the
    /// destinations the prefix chain named itself.
    Program {
        program: String,
        arguments: Vec<String>,
        writes: Vec<PrefixWrite>,
    },
    /// The words do not launch a program (e.g. `command -v`, `sudo -l`).
    NoExecution { writes: Vec<PrefixWrite> },
    /// The chain is unresolvable and the remaining words name a write program
    /// or another prefix, so the caller must reject the segment.
    Undecidable { prefix: String },
    /// The segment carries an `env` `-S` / `--split-string` spelling, whose
    /// command string cannot be verified, so the caller must reject it.
    UnverifiableSplitString { word: String },
}

/// Peels the command prefixes of one segment and decides what runs.
pub(super) fn resolve(words: &[&str]) -> Resolution {
    let mut writes: Vec<PrefixWrite> = Vec::new();
    let mut cursor = 0usize;
    let mut depth = 0usize;
    loop {
        let Some(word) = words.get(cursor) else {
            return Resolution::NoExecution { writes };
        };
        let name = basename(word);
        let Some(prefix) = prefix_for(name) else {
            return Resolution::Program {
                program: name.to_string(),
                arguments: words[cursor + 1..]
                    .iter()
                    .map(|word| (*word).to_string())
                    .collect(),
                writes,
            };
        };
        if depth >= MAX_DEPTH {
            return unresolvable(words, cursor, writes);
        }
        match step(prefix, cursor, &words[cursor + 1..], &mut writes) {
            Step::Continue { cursor: next } => {
                cursor = next;
                depth += 1;
            }
            Step::Stop => return Resolution::NoExecution { writes },
            Step::Undecidable => return unresolvable(words, cursor, writes),
            Step::Reject { word } => return Resolution::UnverifiableSplitString { word },
        }
    }
}

/// A prefix program or reserved word that may sit in front of the real program.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Prefix {
    Env,
    Command,
    Builtin,
    Exec,
    Nohup,
    Sudo,
    Doas,
    Timeout,
    Nice,
    Ionice,
    Stdbuf,
    Time,
    Setsid,
    Caffeinate,
    Arch,
    Bang,
    BraceOpen,
    BraceClose,
    If,
    Then,
    Elif,
    Else,
    While,
    Until,
    Do,
    Coproc,
    Function,
}

fn prefix_for(name: &str) -> Option<Prefix> {
    Some(match name {
        "env" => Prefix::Env,
        "command" => Prefix::Command,
        "builtin" => Prefix::Builtin,
        "exec" => Prefix::Exec,
        "nohup" => Prefix::Nohup,
        "sudo" => Prefix::Sudo,
        "doas" => Prefix::Doas,
        "timeout" | "gtimeout" => Prefix::Timeout,
        "nice" => Prefix::Nice,
        "ionice" => Prefix::Ionice,
        "stdbuf" => Prefix::Stdbuf,
        "time" => Prefix::Time,
        "setsid" => Prefix::Setsid,
        "caffeinate" => Prefix::Caffeinate,
        "arch" => Prefix::Arch,
        "!" => Prefix::Bang,
        "{" => Prefix::BraceOpen,
        "}" => Prefix::BraceClose,
        "if" => Prefix::If,
        "then" => Prefix::Then,
        "elif" => Prefix::Elif,
        "else" => Prefix::Else,
        "while" => Prefix::While,
        "until" => Prefix::Until,
        "do" => Prefix::Do,
        "coproc" => Prefix::Coproc,
        "function" => Prefix::Function,
        _ => return None,
    })
}

/// How a prefix consumes the words that follow it.
enum Step {
    /// Peeling may continue at `cursor`.
    Continue { cursor: usize },
    /// The words launch nothing; the segment has no program.
    Stop,
    /// The prefix cannot be peeled without ambiguity.
    Undecidable,
    /// The prefix carries an `env` `-S` / `--split-string` spelling, which
    /// cannot be verified. Carries the offending option word.
    Reject { word: String },
}

fn step(prefix: Prefix, index: usize, args: &[&str], writes: &mut Vec<PrefixWrite>) -> Step {
    match prefix {
        Prefix::Env => step_env(index, args),
        Prefix::Command => step_command(index, args),
        Prefix::Exec => step_exec(index, args),
        Prefix::Sudo => step_sudo(index, args, writes),
        Prefix::Timeout => step_timeout(index, args),
        Prefix::Nice => step_nice(index, args),
        Prefix::Stdbuf => step_stdbuf(index, args),
        Prefix::Time => step_time(index, args, writes),
        Prefix::Doas => step_options(index, args, &["-n", "-L"], &["-u", "-C"], &[]),
        Prefix::Ionice => step_options(
            index,
            args,
            &["-t", "--ignore"],
            &["-c", "--class", "-n", "--classdata", "-p", "--pid"],
            &["--help", "--version"],
        ),
        Prefix::Setsid => step_options(
            index,
            args,
            &["-c", "--ctty", "-f", "--fork", "-w", "--wait"],
            &[],
            &["--help", "--version"],
        ),
        Prefix::Caffeinate => step_options(
            index,
            args,
            &["-d", "-i", "-m", "-s", "-u"],
            &["-t", "-w"],
            &[],
        ),
        Prefix::Arch => step_options(
            index,
            args,
            &["-arm64", "-i386", "-x86_64"],
            &["-arch"],
            &["-h"],
        ),
        Prefix::Builtin | Prefix::Nohup => step_bare_end_of_options(index, args),
        Prefix::Bang
        | Prefix::BraceOpen
        | Prefix::BraceClose
        | Prefix::If
        | Prefix::Then
        | Prefix::Elif
        | Prefix::Else
        | Prefix::While
        | Prefix::Until
        | Prefix::Do
        | Prefix::Coproc => step_bare(index, args),
        Prefix::Function => step_function(index, args),
    }
}

/// Peels a part-of-speech prefix that consumes no option words of its own.
fn step_bare(index: usize, args: &[&str]) -> Step {
    if args.is_empty() {
        Step::Stop
    } else {
        Step::Continue { cursor: index + 1 }
    }
}

/// Like [`step_bare`], but skips a `--` end-of-options marker so the program
/// that follows it is still resolved (`nohup --`, `builtin --`).
fn step_bare_end_of_options(index: usize, args: &[&str]) -> Step {
    match args.first() {
        None => Step::Stop,
        Some(&"--") => Step::Continue { cursor: index + 2 },
        Some(_) => Step::Continue { cursor: index + 1 },
    }
}

/// `function NAME` consumes the function name before the body.
fn step_function(index: usize, args: &[&str]) -> Step {
    if args.is_empty() {
        Step::Stop
    } else {
        Step::Continue { cursor: index + 2 }
    }
}

/// Peels a prefix whose option grammar is a flag/value/stop table.
fn step_options(
    index: usize,
    args: &[&str],
    flags: &[&str],
    value_options: &[&str],
    stop_options: &[&str],
) -> Step {
    let mut offset = 0;
    while let Some(&word) = args.get(offset) {
        if word == "--" {
            return Step::Continue {
                cursor: index + 1 + offset + 1,
            };
        }
        if stop_options.contains(&word) {
            return Step::Stop;
        }
        if flags.contains(&word) {
            offset += 1;
            continue;
        }
        if value_options.contains(&word) {
            if args.get(offset + 1).is_none() {
                return Step::Undecidable;
            }
            offset += 2;
            continue;
        }
        if let Some((name, _)) = word.split_once('=')
            && value_options.contains(&name)
        {
            offset += 1;
            continue;
        }
        if word.starts_with('-') && word.len() > 1 {
            return Step::Undecidable;
        }
        return Step::Continue {
            cursor: index + 1 + offset,
        };
    }
    Step::Stop
}

fn step_env(index: usize, args: &[&str]) -> Step {
    let mut offset = 0;
    while let Some(&word) = args.get(offset) {
        if word == "--" {
            return Step::Continue {
                cursor: index + 1 + offset + 1,
            };
        }
        // `env -S` / `--split-string` builds the command from a string operand
        // that cannot be re-split the way the shell would, so every spelling is
        // refused before its value is looked at.
        if env_requests_split_string(word) {
            return Step::Reject {
                word: word.to_string(),
            };
        }
        if matches!(
            word,
            "-i" | "-" | "-0" | "-v" | "--ignore-environment" | "--null" | "--debug"
        ) {
            offset += 1;
            continue;
        }
        if matches!(word, "-C" | "--chdir") || word.starts_with("--chdir=") {
            return Step::Undecidable;
        }
        if matches!(word, "-u" | "--unset" | "-P" | "--path") {
            if args.get(offset + 1).is_none() {
                return Step::Undecidable;
            }
            offset += 2;
            continue;
        }
        if word.starts_with("--unset=") || word.starts_with("--path=") {
            offset += 1;
            continue;
        }
        if is_environment_assignment(word) {
            offset += 1;
            continue;
        }
        if word.starts_with('-') && word.len() > 1 {
            return Step::Undecidable;
        }
        return Step::Continue {
            cursor: index + 1 + offset,
        };
    }
    Step::Stop
}

fn step_command(index: usize, args: &[&str]) -> Step {
    let mut offset = 0;
    while let Some(&word) = args.get(offset) {
        match word {
            "--" => {
                return Step::Continue {
                    cursor: index + 1 + offset + 1,
                };
            }
            "-v" | "-V" => return Step::Stop,
            "-p" => offset += 1,
            w if w.starts_with('-') && w.len() > 1 => return Step::Undecidable,
            _ => {
                return Step::Continue {
                    cursor: index + 1 + offset,
                };
            }
        }
    }
    Step::Stop
}

fn step_exec(index: usize, args: &[&str]) -> Step {
    let mut offset = 0;
    while let Some(&word) = args.get(offset) {
        match word {
            "--" => {
                return Step::Continue {
                    cursor: index + 1 + offset + 1,
                };
            }
            "-c" | "-l" => offset += 1,
            "-a" => {
                if args.get(offset + 1).is_none() {
                    return Step::Undecidable;
                }
                offset += 2;
            }
            w if w.starts_with('-') && w.len() > 1 => return Step::Undecidable,
            _ => {
                return Step::Continue {
                    cursor: index + 1 + offset,
                };
            }
        }
    }
    Step::Stop
}

fn step_sudo(index: usize, args: &[&str], writes: &mut Vec<PrefixWrite>) -> Step {
    const FLAGS: &[&str] = &[
        "-E",
        "--preserve-env",
        "-H",
        "--set-home",
        "-n",
        "--non-interactive",
        "-A",
        "--askpass",
        "-b",
        "--background",
        "-P",
        "--preserve-groups",
        "-S",
        "--stdin",
        "-k",
        "--reset-timestamp",
    ];
    const VALUE_OPTIONS: &[&str] = &[
        "-u",
        "--user",
        "-g",
        "--group",
        "-h",
        "--host",
        "-p",
        "--prompt",
        "-C",
        "--close-from",
        "-T",
        "--command-timeout",
        "-r",
        "--role",
        "-t",
        "--type",
    ];
    const TERMINAL: &[&str] = &[
        "-l",
        "--list",
        "-v",
        "--validate",
        "-K",
        "--remove-timestamp",
        "-V",
        "--version",
    ];
    let mut offset = 0;
    while let Some(&word) = args.get(offset) {
        if word == "--" {
            return Step::Continue {
                cursor: index + 1 + offset + 1,
            };
        }
        if TERMINAL.contains(&word) {
            return Step::Stop;
        }
        if matches!(word, "-i" | "--login" | "-s" | "--shell")
            || matches!(word, "-D" | "--chdir" | "-R" | "--chroot")
            || word.starts_with("--chdir=")
            || word.starts_with("--chroot=")
        {
            return Step::Undecidable;
        }
        if matches!(word, "-e" | "--edit") {
            let Some(file) = args.get(offset + 1) else {
                return Step::Undecidable;
            };
            writes.push(PrefixWrite {
                path: (*file).to_string(),
                operation: "sudo".to_string(),
            });
            return Step::Stop;
        }
        if FLAGS.contains(&word) {
            offset += 1;
            continue;
        }
        if let Some((name, _)) = word.split_once('=')
            && VALUE_OPTIONS.contains(&name)
        {
            offset += 1;
            continue;
        }
        if VALUE_OPTIONS.contains(&word) {
            if args.get(offset + 1).is_none() {
                return Step::Undecidable;
            }
            offset += 2;
            continue;
        }
        if let Some((option, _)) = split_attached(word)
            && VALUE_OPTIONS.contains(&option.as_str())
        {
            offset += 1;
            continue;
        }
        if word.starts_with('-') && word.len() > 1 {
            return Step::Undecidable;
        }
        return Step::Continue {
            cursor: index + 1 + offset,
        };
    }
    Step::Stop
}

fn step_timeout(index: usize, args: &[&str]) -> Step {
    let mut offset = 0;
    while let Some(&word) = args.get(offset) {
        match word {
            "--" => {
                offset += 1;
                break;
            }
            "--help" | "--version" => return Step::Stop,
            "--preserve-status" | "--foreground" | "--verbose" | "-v" => offset += 1,
            "-s" | "--signal" | "-k" | "--kill-after" => {
                if args.get(offset + 1).is_none() {
                    return Step::Undecidable;
                }
                offset += 2;
            }
            w if w.starts_with("--signal=") || w.starts_with("--kill-after=") => offset += 1,
            w if w.starts_with('-') && w.len() > 1 => return Step::Undecidable,
            _ => break,
        }
    }
    if args.get(offset).is_none() {
        return Step::Stop;
    }
    offset += 1;
    if args.get(offset).is_none() {
        return Step::Stop;
    }
    Step::Continue {
        cursor: index + 1 + offset,
    }
}

fn step_nice(index: usize, args: &[&str]) -> Step {
    let mut offset = 0;
    while let Some(&word) = args.get(offset) {
        if word == "--" {
            return Step::Continue {
                cursor: index + 1 + offset + 1,
            };
        }
        if matches!(word, "--help" | "--version") {
            return Step::Stop;
        }
        if matches!(word, "-n" | "--adjustment") {
            if args.get(offset + 1).is_none() {
                return Step::Undecidable;
            }
            offset += 2;
            continue;
        }
        if word.starts_with("--adjustment=") {
            offset += 1;
            continue;
        }
        if is_numeric_adjustment(word) {
            offset += 1;
            continue;
        }
        if word.starts_with('-') && word.len() > 1 {
            return Step::Undecidable;
        }
        return Step::Continue {
            cursor: index + 1 + offset,
        };
    }
    Step::Stop
}

fn step_stdbuf(index: usize, args: &[&str]) -> Step {
    const VALUE_OPTIONS: &[&str] = &["-i", "-o", "-e", "--input", "--output", "--error"];
    let mut offset = 0;
    while let Some(&word) = args.get(offset) {
        if word == "--" {
            return Step::Continue {
                cursor: index + 1 + offset + 1,
            };
        }
        if matches!(word, "--help" | "--version") {
            return Step::Stop;
        }
        if VALUE_OPTIONS.contains(&word) {
            if args.get(offset + 1).is_none() {
                return Step::Undecidable;
            }
            offset += 2;
            continue;
        }
        if let Some((name, _)) = word.split_once('=')
            && VALUE_OPTIONS.contains(&name)
        {
            offset += 1;
            continue;
        }
        if let Some((option, _)) = split_attached(word)
            && VALUE_OPTIONS.contains(&option.as_str())
        {
            offset += 1;
            continue;
        }
        if word.starts_with('-') && word.len() > 1 {
            return Step::Undecidable;
        }
        return Step::Continue {
            cursor: index + 1 + offset,
        };
    }
    Step::Stop
}

fn step_time(index: usize, args: &[&str], writes: &mut Vec<PrefixWrite>) -> Step {
    const FLAGS: &[&str] = &[
        "-p",
        "--portability",
        "-v",
        "--verbose",
        "-a",
        "--append",
        "-q",
        "--quiet",
    ];
    const FORMAT_OPTIONS: &[&str] = &["-f", "--format"];
    const OUTPUT_OPTIONS: &[&str] = &["-o", "--output"];
    let mut offset = 0;
    while let Some(&word) = args.get(offset) {
        if word == "--" {
            return Step::Continue {
                cursor: index + 1 + offset + 1,
            };
        }
        if matches!(word, "--help" | "--version" | "-V") {
            return Step::Stop;
        }
        if FLAGS.contains(&word) {
            offset += 1;
            continue;
        }
        if FORMAT_OPTIONS.contains(&word) {
            if args.get(offset + 1).is_none() {
                return Step::Undecidable;
            }
            offset += 2;
            continue;
        }
        if OUTPUT_OPTIONS.contains(&word) {
            let Some(path) = args.get(offset + 1) else {
                return Step::Undecidable;
            };
            writes.push(PrefixWrite {
                path: (*path).to_string(),
                operation: "time".to_string(),
            });
            offset += 2;
            continue;
        }
        if let Some((name, value)) = word.split_once('=') {
            if FORMAT_OPTIONS.contains(&name) {
                offset += 1;
                continue;
            }
            if OUTPUT_OPTIONS.contains(&name) {
                writes.push(PrefixWrite {
                    path: value.to_string(),
                    operation: "time".to_string(),
                });
                offset += 1;
                continue;
            }
        }
        if let Some((option, value)) = split_attached(word) {
            if FORMAT_OPTIONS.contains(&option.as_str()) {
                offset += 1;
                continue;
            }
            if OUTPUT_OPTIONS.contains(&option.as_str()) {
                writes.push(PrefixWrite {
                    path: value,
                    operation: "time".to_string(),
                });
                offset += 1;
                continue;
            }
        }
        if word.starts_with('-') && word.len() > 1 {
            return Step::Undecidable;
        }
        return Step::Continue {
            cursor: index + 1 + offset,
        };
    }
    Step::Stop
}

/// Fails closed for an unresolvable chain: if any remaining word selects
/// `env -S` / `--split-string`, or names a write program or another prefix,
/// the chain is reported unresolvable.
fn unresolvable(words: &[&str], stuck: usize, writes: Vec<PrefixWrite>) -> Resolution {
    if unresolvable_scan_rejects(&words[stuck + 1..]) {
        return Resolution::Undecidable {
            prefix: words[stuck].to_string(),
        };
    }
    Resolution::NoExecution { writes }
}

/// Scans the words after an unresolvable prefix with the same per-word rules:
/// an `env` `-S` / `--split-string` option is refused in any spelling, and a
/// write program or another prefix is refused.
fn unresolvable_scan_rejects(words: &[&str]) -> bool {
    words
        .iter()
        .any(|word| env_requests_split_string(word) || contains_write_or_prefix(word))
}

/// Whether a text names a write program or another prefix when split the way a
/// shell splits `env -S`'s string operand.
fn contains_write_or_prefix(text: &str) -> bool {
    text.split_whitespace().any(|piece| {
        let name = basename(piece);
        is_write_program(name) || prefix_for(name).is_some()
    })
}

/// Whether an `env` option word selects `-S` / `--split-string`, in any
/// spelling, so its command string cannot be verified.
///
/// Long form: any non-empty prefix of `--split-string` (`--s` … `--split-string`),
/// with or without an attached `=`. Short form: a single-dash option cluster
/// that contains an `S` anywhere, unless its first letter is a value-taking
/// option (`-u`, `-C`, `-P`), whose remainder is that option's value.
fn env_requests_split_string(word: &str) -> bool {
    if let Some(rest) = word.strip_prefix("--") {
        let name = rest.split_once('=').map_or(rest, |(name, _)| name);
        let name = format!("--{name}");
        return name.len() > 2 && "--split-string".starts_with(&name);
    }
    let Some(rest) = word.strip_prefix('-') else {
        return false;
    };
    if rest.is_empty() || matches!(rest.chars().next(), Some('u' | 'C' | 'P')) {
        return false;
    }
    rest.contains('S')
}

fn is_write_program(name: &str) -> bool {
    matches!(
        name,
        "tee"
            | "cp"
            | "mv"
            | "install"
            | "ln"
            | "mkdir"
            | "rm"
            | "touch"
            | "truncate"
            | "chmod"
            | "chown"
            | "cd"
            | "pushd"
            | "popd"
            | "sudoedit"
    )
}

fn is_numeric_adjustment(word: &str) -> bool {
    let Some(digits) = word.strip_prefix('-') else {
        return false;
    };
    !digits.is_empty() && digits.chars().all(|ch| ch.is_ascii_digit())
}

/// Splits a short option with an attached value (`-uroot`, `-oL`, `-oFILE`).
fn split_attached(word: &str) -> Option<(String, String)> {
    let mut chars = word.chars();
    if chars.next()? != '-' {
        return None;
    }
    let option = chars.next()?;
    if option == '-' {
        return None;
    }
    Some((format!("-{option}"), chars.as_str().to_string()))
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

    fn program(program: &str, arguments: &[&str]) -> Resolution {
        Resolution::Program {
            program: program.to_string(),
            arguments: arguments.iter().map(|item| (*item).to_string()).collect(),
            writes: Vec::new(),
        }
    }

    fn no_execution() -> Resolution {
        Resolution::NoExecution { writes: Vec::new() }
    }

    fn write(path: &str, operation: &str) -> PrefixWrite {
        PrefixWrite {
            path: path.to_string(),
            operation: operation.to_string(),
        }
    }

    #[test]
    fn command_prefix_resolves_known_prefix_chains() {
        assert_eq!(resolve(&["tee", "/tmp/f"]), program("tee", &["/tmp/f"]));
        assert_eq!(
            resolve(&["env", "tee", "/tmp/f"]),
            program("tee", &["/tmp/f"])
        );
        assert_eq!(
            resolve(&["env", "FOO=1", "tee", "out.txt"]),
            program("tee", &["out.txt"])
        );
        assert_eq!(
            resolve(&["/usr/bin/env", "-u", "HOME", "cp", "a", "b"]),
            program("cp", &["a", "b"])
        );
        assert_eq!(
            resolve(&["sudo", "-u", "root", "tee", "/tmp/f"]),
            program("tee", &["/tmp/f"])
        );
        assert_eq!(
            resolve(&["timeout", "-k", "1", "5", "tee", "/tmp/f"]),
            program("tee", &["/tmp/f"])
        );
        assert_eq!(
            resolve(&["nice", "-5", "tee", "/tmp/f"]),
            program("tee", &["/tmp/f"])
        );
        assert_eq!(
            resolve(&["stdbuf", "-oL", "tee", "/tmp/f"]),
            program("tee", &["/tmp/f"])
        );
        assert_eq!(
            resolve(&["{", "tee", "/tmp/f", "}"]),
            program("tee", &["/tmp/f", "}"])
        );
        assert_eq!(
            resolve(&["command", "env", "nohup", "tee", "/tmp/f"]),
            program("tee", &["/tmp/f"])
        );
    }

    #[test]
    fn command_prefix_records_prefix_owned_write_targets() {
        assert_eq!(
            resolve(&["/usr/bin/time", "-o", "/tmp/f", "true"]),
            Resolution::Program {
                program: "true".to_string(),
                arguments: Vec::new(),
                writes: vec![write("/tmp/f", "time")],
            }
        );
        assert_eq!(
            resolve(&["sudo", "-e", "/tmp/f"]),
            Resolution::NoExecution {
                writes: vec![write("/tmp/f", "sudo")],
            }
        );
    }

    #[test]
    fn command_prefix_marks_non_executing_modes() {
        assert_eq!(resolve(&["command", "-v", "cargo"]), no_execution());
        assert_eq!(resolve(&["sudo", "-l", "tee", "/tmp/f"]), no_execution());
    }

    #[test]
    fn command_prefix_marks_unresolvable_chains() {
        assert_eq!(
            resolve(&["sudo", "-X", "u", "tee", "/tmp/f"]),
            Resolution::Undecidable {
                prefix: "sudo".to_string(),
            }
        );
        assert_eq!(
            resolve(&["env", "-S", "tee /tmp/f"]),
            Resolution::UnverifiableSplitString {
                word: "-S".to_string(),
            }
        );
        assert_eq!(
            resolve(&["env", "-C", "sub", "tee", "f"]),
            Resolution::Undecidable {
                prefix: "env".to_string(),
            }
        );
        assert_eq!(
            resolve(&["sudo", "-s", "tee", "/tmp/f"]),
            Resolution::Undecidable {
                prefix: "sudo".to_string(),
            }
        );
    }

    #[test]
    fn command_prefix_keeps_unidentified_verification_chains_allowed() {
        assert_eq!(
            resolve(&["timeout", "--unknown", "600", "cargo", "test"]),
            no_execution()
        );
        assert_eq!(
            resolve(&["env", "FOO=1", "cargo", "test"]),
            program("cargo", &["test"])
        );
    }

    #[test]
    fn command_prefix_rejects_chains_past_the_depth_limit() {
        let mut words: Vec<&str> = std::iter::repeat_n("env", MAX_DEPTH).collect();
        words.extend(["tee", "/tmp/f"]);
        assert_eq!(
            resolve(&words),
            program("tee", &["/tmp/f"]),
            "exactly the depth limit still resolves"
        );

        words.insert(0, "env");
        assert_eq!(
            resolve(&words),
            Resolution::Undecidable {
                prefix: "env".to_string(),
            },
            "one prefix past the depth limit fails closed"
        );

        // The same boundary with a non-writing program: the scan finds nothing
        // to reject, so the chain stays allowed.
        let mut cargo_words: Vec<&str> = std::iter::repeat_n("env", MAX_DEPTH).collect();
        cargo_words.extend(["cargo", "test"]);
        assert_eq!(
            resolve(&cargo_words),
            program("cargo", &["test"]),
            "exactly the depth limit still resolves a verification command"
        );
        cargo_words.insert(0, "env");
        assert_eq!(
            resolve(&cargo_words),
            no_execution(),
            "past the depth limit a verification command stays allowed"
        );
    }

    #[test]
    fn command_prefix_splits_attached_short_options() {
        assert_eq!(
            split_attached("-uroot"),
            Some(("-u".to_string(), "root".to_string()))
        );
        assert_eq!(
            split_attached("-oL"),
            Some(("-o".to_string(), "L".to_string()))
        );
        assert_eq!(split_attached("--user"), None);
        assert_eq!(split_attached("-"), None);
    }

    #[test]
    fn command_prefix_recognises_every_table_entry() {
        for name in [
            "env",
            "command",
            "builtin",
            "exec",
            "nohup",
            "sudo",
            "doas",
            "timeout",
            "gtimeout",
            "nice",
            "ionice",
            "stdbuf",
            "time",
            "setsid",
            "caffeinate",
            "arch",
            "!",
            "{",
            "}",
            "if",
            "then",
            "elif",
            "else",
            "while",
            "until",
            "do",
            "coproc",
            "function",
        ] {
            assert!(prefix_for(name).is_some(), "table must include `{name}`");
        }
        for name in ["tee", "cargo", "sh", "xargs", "watch", "sudoedit", ""] {
            assert!(prefix_for(name).is_none(), "`{name}` is not a prefix");
        }
    }

    #[test]
    fn command_prefix_resolution_table() {
        let undecidable = |prefix: &str| Resolution::Undecidable {
            prefix: prefix.to_string(),
        };
        let split_string = |word: &str| Resolution::UnverifiableSplitString {
            word: word.to_string(),
        };
        let cases: &[(&[&str], Resolution)] = &[
            // env flags, value options, assignments and `--`.
            (&["env", "-i", "tee", "f"], program("tee", &["f"])),
            (&["env", "-", "tee", "f"], program("tee", &["f"])),
            (&["env", "-0", "tee", "f"], program("tee", &["f"])),
            (&["env", "-v", "tee", "f"], program("tee", &["f"])),
            (
                &["env", "--ignore-environment", "tee", "f"],
                program("tee", &["f"]),
            ),
            (&["env", "--null", "tee", "f"], program("tee", &["f"])),
            (&["env", "--debug", "tee", "f"], program("tee", &["f"])),
            (&["env", "-u", "HOME", "tee", "f"], program("tee", &["f"])),
            (
                &["env", "--unset", "HOME", "tee", "f"],
                program("tee", &["f"]),
            ),
            (
                &["env", "-P", "/usr/bin", "tee", "f"],
                program("tee", &["f"]),
            ),
            (
                &["env", "--path", "/usr/bin", "tee", "f"],
                program("tee", &["f"]),
            ),
            (&["env", "--unset=HOME", "tee", "f"], program("tee", &["f"])),
            (&["env", "--path=/x", "tee", "f"], program("tee", &["f"])),
            (&["env", "--", "tee", "f"], program("tee", &["f"])),
            (
                &["env", "A=1", "-i", "B=2", "tee", "f"],
                program("tee", &["f"]),
            ),
            // command.
            (&["command", "-p", "tee", "f"], program("tee", &["f"])),
            (&["command", "--", "tee", "f"], program("tee", &["f"])),
            (&["command", "-v", "cargo"], no_execution()),
            (&["command", "-V", "cargo"], no_execution()),
            // builtin / exec / nohup.
            (&["builtin", "cd", "/tmp"], program("cd", &["/tmp"])),
            (
                &["exec", "-c", "-l", "-a", "x", "tee", "f"],
                program("tee", &["f"]),
            ),
            (&["exec", "--", "tee", "f"], program("tee", &["f"])),
            (&["nohup", "tee", "f"], program("tee", &["f"])),
            // sudo.
            (&["sudo", "-E", "tee", "f"], program("tee", &["f"])),
            (
                &["sudo", "-H", "-n", "-A", "-b", "-P", "-S", "tee", "f"],
                program("tee", &["f"]),
            ),
            (&["sudo", "-u", "root", "tee", "f"], program("tee", &["f"])),
            (&["sudo", "-uroot", "tee", "f"], program("tee", &["f"])),
            (&["sudo", "--user=root", "tee", "f"], program("tee", &["f"])),
            (
                &["sudo", "-g", "g", "-h", "host", "tee", "f"],
                program("tee", &["f"]),
            ),
            (
                &["sudo", "-p", "p", "-C", "9", "-T", "5", "tee", "f"],
                program("tee", &["f"]),
            ),
            (
                &["sudo", "-r", "r", "-t", "t", "tee", "f"],
                program("tee", &["f"]),
            ),
            (&["sudo", "--", "tee", "f"], program("tee", &["f"])),
            (&["sudo", "-l", "tee", "f"], no_execution()),
            (&["sudo", "--version", "tee", "f"], no_execution()),
            // doas.
            (&["doas", "-n", "-L", "tee", "f"], program("tee", &["f"])),
            (&["doas", "-u", "root", "tee", "f"], program("tee", &["f"])),
            (
                &["doas", "-C", "/etc/doas.conf", "tee", "f"],
                program("tee", &["f"]),
            ),
            (&["doas", "--", "tee", "f"], program("tee", &["f"])),
            // timeout / gtimeout.
            (&["timeout", "5", "tee", "f"], program("tee", &["f"])),
            (&["gtimeout", "5", "tee", "f"], program("tee", &["f"])),
            (
                &["timeout", "--preserve-status", "5", "tee", "f"],
                program("tee", &["f"]),
            ),
            (
                &["timeout", "--foreground", "5", "tee", "f"],
                program("tee", &["f"]),
            ),
            (
                &["timeout", "--verbose", "5", "tee", "f"],
                program("tee", &["f"]),
            ),
            (&["timeout", "-v", "5", "tee", "f"], program("tee", &["f"])),
            (
                &["timeout", "-s", "KILL", "5", "tee", "f"],
                program("tee", &["f"]),
            ),
            (
                &["timeout", "--signal", "KILL", "5", "tee", "f"],
                program("tee", &["f"]),
            ),
            (
                &["timeout", "--signal=KILL", "5", "tee", "f"],
                program("tee", &["f"]),
            ),
            (
                &["timeout", "-k", "1", "5", "tee", "f"],
                program("tee", &["f"]),
            ),
            (
                &["timeout", "--kill-after=1", "5", "tee", "f"],
                program("tee", &["f"]),
            ),
            (&["timeout", "--", "5", "tee", "f"], program("tee", &["f"])),
            (&["timeout", "--help"], no_execution()),
            // nice.
            (&["nice", "-n", "5", "tee", "f"], program("tee", &["f"])),
            (
                &["nice", "--adjustment", "5", "tee", "f"],
                program("tee", &["f"]),
            ),
            (
                &["nice", "--adjustment=5", "tee", "f"],
                program("tee", &["f"]),
            ),
            (&["nice", "-5", "tee", "f"], program("tee", &["f"])),
            (&["nice", "--", "tee", "f"], program("tee", &["f"])),
            (&["nice", "--version"], no_execution()),
            // ionice.
            (&["ionice", "-t", "tee", "f"], program("tee", &["f"])),
            (&["ionice", "--ignore", "tee", "f"], program("tee", &["f"])),
            (&["ionice", "-c", "3", "tee", "f"], program("tee", &["f"])),
            (
                &["ionice", "--class", "3", "tee", "f"],
                program("tee", &["f"]),
            ),
            (&["ionice", "--class=3", "tee", "f"], program("tee", &["f"])),
            (&["ionice", "-n", "7", "tee", "f"], program("tee", &["f"])),
            (
                &["ionice", "--classdata", "7", "tee", "f"],
                program("tee", &["f"]),
            ),
            (&["ionice", "-p", "123", "tee", "f"], program("tee", &["f"])),
            (
                &["ionice", "--pid", "123", "tee", "f"],
                program("tee", &["f"]),
            ),
            (&["ionice", "--", "tee", "f"], program("tee", &["f"])),
            (&["ionice", "--help"], no_execution()),
            // stdbuf.
            (&["stdbuf", "-i", "L", "tee", "f"], program("tee", &["f"])),
            (&["stdbuf", "-o", "L", "tee", "f"], program("tee", &["f"])),
            (&["stdbuf", "-e", "L", "tee", "f"], program("tee", &["f"])),
            (&["stdbuf", "-oL", "tee", "f"], program("tee", &["f"])),
            (&["stdbuf", "--input=L", "tee", "f"], program("tee", &["f"])),
            (
                &["stdbuf", "--output=L", "tee", "f"],
                program("tee", &["f"]),
            ),
            (&["stdbuf", "--error=L", "tee", "f"], program("tee", &["f"])),
            (&["stdbuf", "--", "tee", "f"], program("tee", &["f"])),
            (&["stdbuf", "--help"], no_execution()),
            // time.
            (&["time", "-p", "tee", "f"], program("tee", &["f"])),
            (&["time", "-v", "tee", "f"], program("tee", &["f"])),
            (&["time", "-a", "tee", "f"], program("tee", &["f"])),
            (&["time", "-q", "tee", "f"], program("tee", &["f"])),
            (
                &["time", "--portability", "tee", "f"],
                program("tee", &["f"]),
            ),
            (&["time", "--verbose", "tee", "f"], program("tee", &["f"])),
            (&["time", "--append", "tee", "f"], program("tee", &["f"])),
            (&["time", "--quiet", "tee", "f"], program("tee", &["f"])),
            (&["time", "-f", "%e", "tee", "f"], program("tee", &["f"])),
            (&["time", "--format=%e", "tee", "f"], program("tee", &["f"])),
            (&["time", "--", "tee", "f"], program("tee", &["f"])),
            (&["time", "--version"], no_execution()),
            // setsid / caffeinate / arch.
            (&["setsid", "-f", "-w", "tee", "f"], program("tee", &["f"])),
            (&["setsid", "--fork", "tee", "f"], program("tee", &["f"])),
            (&["setsid", "--wait", "tee", "f"], program("tee", &["f"])),
            (
                &["caffeinate", "-d", "-i", "-m", "-s", "-u", "tee", "f"],
                program("tee", &["f"]),
            ),
            (
                &["caffeinate", "-t", "5", "tee", "f"],
                program("tee", &["f"]),
            ),
            (
                &["caffeinate", "-w", "9", "tee", "f"],
                program("tee", &["f"]),
            ),
            (&["arch", "-arm64", "tee", "f"], program("tee", &["f"])),
            (&["arch", "-i386", "tee", "f"], program("tee", &["f"])),
            (&["arch", "-x86_64", "tee", "f"], program("tee", &["f"])),
            (
                &["arch", "-arch", "arm64", "tee", "f"],
                program("tee", &["f"]),
            ),
            (&["arch", "-h"], no_execution()),
            // flag/value option-table edges: unknown options fail closed,
            // values that follow flags still resolve, and nested prefixes use
            // an offset that is not zero.
            (&["doas", "-X", "tee", "f"], undecidable("doas")),
            (&["doas", "-u"], no_execution()),
            (&["ionice", "-X", "tee", "f"], undecidable("ionice")),
            (&["ionice", "-c"], no_execution()),
            (
                &["ionice", "-t", "-c", "3", "tee", "f"],
                program("tee", &["f"]),
            ),
            (&["setsid", "-X", "tee", "f"], undecidable("setsid")),
            (&["caffeinate", "-X", "tee", "f"], undecidable("caffeinate")),
            (&["arch", "-bogus", "tee", "f"], undecidable("arch")),
            (&["env", "ionice", "--", "tee", "f"], program("tee", &["f"])),
            (
                &["env", "doas", "-u", "root", "tee", "f"],
                program("tee", &["f"]),
            ),
            (
                &["env", "arch", "-arm64", "tee", "f"],
                program("tee", &["f"]),
            ),
            // env -S / --split-string is refused in every spelling.
            (&["env", "-Stee /tmp/f"], split_string("-Stee /tmp/f")),
            (&["env", "-iStee /tmp/f"], split_string("-iStee /tmp/f")),
            (
                &["env", "--split-string=tee /tmp/f"],
                split_string("--split-string=tee /tmp/f"),
            ),
            (
                &["env", "-Scp a.txt /tmp/f"],
                split_string("-Scp a.txt /tmp/f"),
            ),
            (&["env", "-S", "tee /tmp/f"], split_string("-S")),
            (&["env", "-Scargo test"], split_string("-Scargo test")),
            (&["env", "-S"], split_string("-S")),
            (&["env", "--split-string"], split_string("--split-string")),
            (
                &["env", "-S\"tee\" /tmp/f"],
                split_string("-S\"tee\" /tmp/f"),
            ),
            (&["env", "-S'tee' /tmp/f"], split_string("-S'tee' /tmp/f")),
            (
                &["env", "-Ste\"\"e /tmp/f"],
                split_string("-Ste\"\"e /tmp/f"),
            ),
            (&["env", "-Stee\\_/tmp/f"], split_string("-Stee\\_/tmp/f")),
            (&["env", "-S${X} /tmp/f"], split_string("-S${X} /tmp/f")),
            // Behind an undecidable option the scan applies the same refusal.
            (&["env", "-uX", "-Stee /tmp/f"], undecidable("env")),
            (&["env", "-iuX", "-Scp a.txt /tmp/f"], undecidable("env")),
            (&["env", "-C", "sub", "-Stee /tmp/f"], undecidable("env")),
            (&["env", "--chdir=sub", "-Stee link/f"], undecidable("env")),
            (&["env", "--unknown", "-Stee /tmp/f"], undecidable("env")),
            // B2/B3: nohup and builtin skip a leading `--`.
            (&["nohup", "--", "tee", "f"], program("tee", &["f"])),
            (&["builtin", "--", "cd", "x"], program("cd", &["x"])),
            // B4: sudo -k still runs the command.
            (&["sudo", "-k", "tee", "f"], program("tee", &["f"])),
            (&["sudo", "-n", "-k", "tee", "f"], program("tee", &["f"])),
            (&["sudo", "-k"], no_execution()),
            (&["sudo", "-K", "tee", "f"], no_execution()),
            // sudoedit is a write program for the unresolvable scan.
            (&["env", "-C", "sub", "sudoedit", "f"], undecidable("env")),
            (&["sudo", "-X", "u", "sudoedit", "f"], undecidable("sudo")),
            // `command -` runs nothing in sh, so `-` is not treated as the program.
            (&["command", "-", "tee", "f"], program("-", &["tee", "f"])),
            // shell prefixes and reserved words.
            (&["!", "tee", "f"], program("tee", &["f"])),
            (&["{", "tee", "f"], program("tee", &["f"])),
            (&["}", "tee", "f"], program("tee", &["f"])),
            (&["if", "tee", "f"], program("tee", &["f"])),
            (&["then", "tee", "f"], program("tee", &["f"])),
            (&["elif", "tee", "f"], program("tee", &["f"])),
            (&["else", "tee", "f"], program("tee", &["f"])),
            (&["while", "tee", "f"], program("tee", &["f"])),
            (&["until", "tee", "f"], program("tee", &["f"])),
            (&["do", "tee", "f"], program("tee", &["f"])),
            (&["coproc", "tee", "f"], program("tee", &["f"])),
            (&["function", "f", "tee", "x"], program("tee", &["x"])),
            // prefix only, without a command.
            (&["env"], no_execution()),
            (&["timeout", "5"], no_execution()),
            (&["command"], no_execution()),
            (&["sudo", "-l"], no_execution()),
            // unresolvable forms, including nested prefixes.
            (&["env", "-C", "sub", "tee", "f"], undecidable("env")),
            (&["env", "--chdir=sub", "tee", "f"], undecidable("env")),
            (&["env", "-S", "tee f"], split_string("-S")),
            (&["env", "-X", "tee", "f"], undecidable("env")),
            (&["sudo", "-i", "tee", "f"], undecidable("sudo")),
            (&["sudo", "-s", "tee", "f"], undecidable("sudo")),
            (&["sudo", "-D", "/tmp", "tee", "f"], undecidable("sudo")),
            (&["sudo", "-R", "/tmp", "tee", "f"], undecidable("sudo")),
            (&["sudo", "--chdir=/tmp", "tee", "f"], undecidable("sudo")),
            (&["sudo", "--chroot=/tmp", "tee", "f"], undecidable("sudo")),
            (&["env", "sudo", "-s", "tee", "f"], undecidable("sudo")),
        ];
        for (words, expected) in cases {
            assert_eq!(resolve(words), *expected, "words: {words:?}");
        }
    }

    fn command_prefix_inside_fixture() -> tempfile::TempDir {
        let fixture = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(fixture.path().join("ws")).unwrap();
        fixture
    }

    /// An inside write must stay allowed *and* stay recognized. A cursor that
    /// drops the write program (or picks an option value as the program) makes
    /// either the mutation flag or the protected-path lookup disappear, so the
    /// three assertions below pin the offset arithmetic.
    fn assert_command_prefix_inside_write(command: &str, target: &str) {
        let fixture = command_prefix_inside_fixture();
        let root = fixture.path().join("ws");
        assert!(
            super::super::confinement_rejection(command, &root).is_none(),
            "must stay allowed: {command}"
        );
        assert!(
            super::super::has_recognized_mutation(command),
            "must stay a recognized mutation: {command}"
        );
        assert_eq!(
            super::super::protected_path_mutation(command, &root, &[target.to_string()]),
            Some(target.to_string()),
            "must protect the path: {command}"
        );
    }

    fn assert_command_prefix_no_write(command: &str) {
        let fixture = command_prefix_inside_fixture();
        let root = fixture.path().join("ws");
        assert!(
            super::super::confinement_rejection(command, &root).is_none(),
            "must stay allowed: {command}"
        );
        assert!(
            !super::super::has_recognized_mutation(command),
            "must not be a recognized mutation: {command}"
        );
        assert_eq!(
            super::super::protected_path_mutation(command, &root, &["out.txt".to_string()]),
            None,
            "must not protect a path: {command}"
        );
    }

    fn assert_command_prefix_rejected(command: &str) {
        let fixture = command_prefix_inside_fixture();
        let root = fixture.path().join("ws");
        assert!(
            super::super::confinement_rejection(command, &root).is_some(),
            "must be rejected: {command}"
        );
    }

    #[test]
    fn command_prefix_option_values_keep_inside_writes_detected() {
        let cases: &[(&str, &str)] = &[
            ("env -i -- tee out.txt", "out.txt"),
            ("env -u V tee out.txt", "out.txt"),
            ("env - tee out.txt", "out.txt"),
            ("command -p tee out.txt", "out.txt"),
            ("exec -a n tee out.txt", "out.txt"),
            ("exec -c tee out.txt", "out.txt"),
            ("exec -l tee out.txt", "out.txt"),
            ("exec -- tee out.txt", "out.txt"),
            ("exec -c -- tee out.txt", "out.txt"),
            ("sudo -u root tee out.txt", "out.txt"),
            ("sudo -g g tee out.txt", "out.txt"),
            ("sudo -n tee out.txt", "out.txt"),
            ("sudo -- tee out.txt", "out.txt"),
            ("timeout 5 tee out.txt", "out.txt"),
            ("timeout -s TERM 5 tee out.txt", "out.txt"),
            ("timeout -k 1 5 tee out.txt", "out.txt"),
            ("timeout --signal=TERM 5 tee out.txt", "out.txt"),
            ("timeout - tee out.txt", "out.txt"),
            ("nice -n 5 tee out.txt", "out.txt"),
            ("nice -5 tee out.txt", "out.txt"),
            ("nice --adjustment=5 tee out.txt", "out.txt"),
            ("nice -n -5 tee out.txt", "out.txt"),
            ("nice -n 5 -- tee out.txt", "out.txt"),
            ("stdbuf -o L tee out.txt", "out.txt"),
            ("stdbuf -oL tee out.txt", "out.txt"),
            ("stdbuf -i0 -o0 tee out.txt", "out.txt"),
            ("stdbuf --output=L tee out.txt", "out.txt"),
            ("time -p tee out.txt", "out.txt"),
            ("/usr/bin/time -f F tee out.txt", "out.txt"),
            ("/usr/bin/time -o log tee out.txt", "out.txt"),
            ("/usr/bin/time -v tee out.txt", "out.txt"),
            ("/usr/bin/time -a -o log tee out.txt", "out.txt"),
            ("/usr/bin/time --output=log tee out.txt", "out.txt"),
            ("/usr/bin/time -fF tee out.txt", "out.txt"),
            ("/usr/bin/time -olog tee out.txt", "out.txt"),
        ];
        for (command, target) in cases {
            assert_command_prefix_inside_write(command, target);
        }
    }

    #[test]
    fn command_prefix_unknown_options_and_bare_dash_boundaries() {
        for command in [
            "command - tee out.txt",
            "command -v tee out.txt",
            "exec - tee out.txt",
            "nice - tee out.txt",
            "stdbuf - tee out.txt",
            "/usr/bin/time - tee out.txt",
        ] {
            assert_command_prefix_no_write(command);
        }
        for command in [
            "command -X tee /tmp/f",
            "exec -X tee /tmp/f",
            "nice -X tee /tmp/f",
            "stdbuf -X tee /tmp/f",
            "/usr/bin/time -X tee /tmp/f",
            "sudo -i tee /tmp/f",
            "sudo -s tee /tmp/f",
            "sudo -D sub tee /tmp/f",
            "sudo -R sub tee /tmp/f",
        ] {
            assert_command_prefix_rejected(command);
        }
    }

    #[test]
    fn command_prefix_env_requests_split_string_table() {
        // Short clusters with an S anywhere are refused.
        for word in [
            "-S",
            "-Sx",
            "-iS",
            "-vS",
            "-0S",
            "-iuXS",
            "-SCARGO",
            "-iStee /tmp/f",
        ] {
            assert!(
                env_requests_split_string(word),
                "must request split-string: {word}"
            );
        }
        // A value-taking option first owns the rest of the word.
        for word in [
            "-u",
            "-uroot",
            "-uSOMETHING",
            "-P/usr/bin",
            "-PS",
            "-Csub",
            "-CS",
            "-i",
            "-",
            "-0",
            "-x",
            "-X",
            "S",
            "tee",
            "",
        ] {
            assert!(
                !env_requests_split_string(word),
                "must not request split-string: {word}"
            );
        }
        // Every prefix of --split-string, with or without `=`.
        for word in [
            "--s",
            "--sp",
            "--split",
            "--split-str",
            "--split-string",
            "--split-string=tee /tmp/f",
            "--s='tee /tmp/f'",
        ] {
            assert!(
                env_requests_split_string(word),
                "must request split-string: {word}"
            );
        }
        for word in [
            "--",
            "--ignore-environment",
            "--null",
            "--debug",
            "--chdir",
            "--unset=x",
            "--split-stringx",
        ] {
            assert!(
                !env_requests_split_string(word),
                "must not request split-string: {word}"
            );
        }
    }
}
