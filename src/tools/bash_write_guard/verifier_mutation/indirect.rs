//! Execution forms whose body a Verify or Report step cannot inspect (Issue
//! #871, IC-29b).
//!
//! [`super::find`] names an explicit update command. This module refuses the
//! forms that can run one without naming it: a shell `-c` body, `eval`,
//! `xargs`, `busybox`, a `find` that runs or deletes, an inline interpreter
//! (`node -e/-p`, `python -c`, `perl -e`), and any `sed` outside one closed
//! print grammar. Each is refused without reading its body, so a body that is
//! only `cat a.txt` is refused too. The segment split and prefix peeling are the
//! parent's, so a quoted or absolute program name and an
//! `env`/`command`/`timeout` prefix do not hide the program.
//!
//! | form | refused when |
//! | --- | --- |
//! | `sh`, `bash`, `dash`, `zsh`, `ksh`, ... | an option cluster before the script holds `c` (`-c`, `-lc`, `-ec`), or no script operand is given (`-s`, `-`, a bare shell reading stdin) |
//! | `eval`, `xargs`, `busybox` | always |
//! | `find`, `gfind` | `-exec`, `-execdir`, `-ok`, `-okdir`, `-delete`, `-fprint*`, `-fls` |
//! | `node`, `nodejs` | `-e`, `--eval`, `-p`, `--print`, a cluster holding either, or `-` |
//! | `python`, `python3`, `pypy`, ... | a cluster before `-m` or the script holds `c`, or `-` |
//! | `perl` | a switch cluster holding `e` or `E`, or `-` |
//! | `sed`, `gsed` | anything but `-n <N[,M]p> [file...]` |
//!
//! A known launcher (`flock`, `chrt`, `taskset`, `strace`, `watch`, `npx`,
//! `npm exec`, `pnpm exec`, ...) starts the command in its arguments, so every
//! later word is tried as the program; its own shell-string option (`npx -c`,
//! `flock -c`, `script -qc`) is refused. A `data:` URL given to node's
//! `--import`/`--require`/`-r`/`--loader` and a perl `-M`/`-m` value that is not
//! a module name are code and are refused.
//!
//! An option is told apart from the entrypoint and from `--`, so
//! `node smoke-check.js --label=-e` and `python -m pytest -c cfg` are not
//! refused, and a quoted `sh -c` inside an `echo` or `grep` argument is one
//! word that never becomes a program. When the prefix chain cannot be resolved,
//! every word is tried as the program and the rest of the words as its
//! arguments, so an unresolved prefix does not hide the target either.
//!
//! The guarantee ends at this grammar: a script file, a package script, a
//! compiler hook, an interpreter outside the table (`ruby -e`), and a command
//! the host declared in its own verify are not read here.

use super::command_prefix::{self, Resolution};
use super::{CrMode, ShellToken, VerifierMutation, command_words, lex_shell, shell_lexical};

#[cfg(test)]
mod tests;

const SHELLS: [&str; 15] = [
    "sh", "bash", "dash", "zsh", "ksh", "ksh93", "mksh", "pdksh", "ash", "rbash", "yash", "posh",
    "csh", "tcsh", "fish",
];

/// `node` options that take no separate value word. Any other `--option`
/// without `=` may swallow the next word, which is then not the entrypoint.
const NODE_FLAGS: [&str; 26] = [
    "trace-uncaught",
    "trace-exit",
    "trace-sigint",
    "trace-sync-io",
    "trace-tls",
    "pending-deprecation",
    "abort-on-uncaught-exception",
    "experimental-strip-types",
    "experimental-transform-types",
    "experimental-require-module",
    "experimental-detect-module",
    "use-strict",
    "check",
    "test",
    "watch",
    "no-warnings",
    "trace-warnings",
    "trace-deprecation",
    "no-deprecation",
    "throw-deprecation",
    "enable-source-maps",
    "expose-gc",
    "harmony",
    "preserve-symlinks",
    "experimental-vm-modules",
    "version",
];

/// `find` primaries that run a command or change the file system.
const FIND_UPDATES: [&str; 9] = [
    "-exec", "-execdir", "-ok", "-okdir", "-delete", "-fprint", "-fprint0", "-fprintf", "-fls",
];

/// `find` primaries whose value word is a pattern, not a primary.
const FIND_PATTERNS: [&str; 10] = [
    "-name",
    "-iname",
    "-path",
    "-ipath",
    "-wholename",
    "-iwholename",
    "-regex",
    "-iregex",
    "-lname",
    "-ilname",
];

/// `node` options whose value, when it is a `data:` URL, is code.
const NODE_CODE_LOADERS: [&str; 4] = ["import", "require", "loader", "experimental-loader"];

/// Programs that start the command given in their own arguments. Their options
/// are not parsed: every later word is tried as the program instead.
const LAUNCHERS: [&str; 14] = [
    "flock", "chrt", "taskset", "ionice", "setsid", "unshare", "nsenter", "strace", "ltrace",
    "watch", "script", "npx", "bunx", "pnpx",
];

/// Package managers whose first argument turns them into a launcher.
const EXEC_SUBCOMMANDS: [(&str, &[&str]); 3] = [
    ("npm", &["exec", "x"]),
    ("pnpm", &["exec", "dlx"]),
    ("yarn", &["exec", "dlx"]),
];

/// Names the first execution form in `command` whose body cannot be inspected.
pub(super) fn find(command: &str) -> Option<VerifierMutation> {
    // Anything the lexer cannot read, or that carries ANSI-C quoting, was
    // already named by the write table.
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
            if let Some(operation) = segment_operation(&words) {
                return Some(VerifierMutation {
                    operation,
                    target: None,
                    opaque: true,
                });
            }
        }
    }
    None
}

fn segment_operation(words: &[&str]) -> Option<String> {
    match command_prefix::resolve(words) {
        Resolution::Program {
            program, arguments, ..
        } => classify(&program, &arguments, true).or_else(|| launched(&program, &arguments)),
        // An unresolvable prefix hides which program runs, so every word is
        // tried as the program with the words after it as its arguments.
        Resolution::Undecidable { .. } | Resolution::NoExecution { .. } => {
            let mut under_launcher = false;
            for index in 0..words.len() {
                let arguments: Vec<String> = words[index + 1..]
                    .iter()
                    .map(|word| (*word).to_string())
                    .collect();
                let name = base_name(words[index]);
                // A shell under a launcher that was reached is certain enough for
                // the stdin rule; a bare guess is not.
                let found = classify(name, &arguments, under_launcher)
                    .or_else(|| launcher_string(name, &arguments));
                if found.is_some() {
                    return found;
                }
                under_launcher |= is_launcher(name, &arguments);
            }
            None
        }
        // The parent's write table already refuses `env -S`.
        Resolution::UnverifiableSplitString { .. } => None,
    }
}

fn is_launcher(program: &str, arguments: &[String]) -> bool {
    LAUNCHERS.contains(&program)
        || EXEC_SUBCOMMANDS.iter().any(|(name, subcommands)| {
            *name == program
                && arguments
                    .first()
                    .is_some_and(|first| subcommands.contains(&first.as_str()))
        })
}

/// The first refused form among the words a launcher starts, or a launcher's
/// own shell-string option. Every word is a suffix start of `arguments`, so a
/// launcher started by a launcher, at any depth, is read by this one pass.
fn launched(program: &str, arguments: &[String]) -> Option<String> {
    if !is_launcher(program, arguments) {
        return None;
    }
    (0..arguments.len())
        .find_map(|index| {
            let name = base_name(&arguments[index]);
            let rest = &arguments[index + 1..];
            classify(name, rest, true).or_else(|| launcher_string(name, rest))
        })
        .or_else(|| launcher_string(program, arguments))
}

/// `program`'s own shell-string option, when `program` is a launcher.
fn launcher_string(program: &str, arguments: &[String]) -> Option<String> {
    (is_launcher(program, arguments) && launcher_runs_string(program, arguments))
        .then(|| format!("{program} -c"))
}

/// `npx` options that take no value word.
const NPX_FLAGS: [&str; 12] = [
    "--yes",
    "-y",
    "--no",
    "--quiet",
    "-q",
    "--no-install",
    "--ignore-existing",
    "--verbose",
    "--silent",
    "--version",
    "-v",
    "--help",
];

/// `npx`/`npm exec` options read before the package: the closed list. Any other
/// dash word there (an abbreviation, a short alias, a cluster, an unknown
/// option) is refused, so no spelling of a call option can hide in it.
const NPX_VALUED: [&str; 4] = ["--package", "-p", "--workspace", "-w"];

/// `flock --command`, `script -qc`, `npx -c`, `npm exec --call`: the launcher
/// hands one string to a shell. `flock` and `script` are scanned whole. `npx`
/// reads options up to its package and `npm exec` up to `--` (all words when
/// there is none); only [`NPX_FLAGS`] and [`NPX_VALUED`] are understood there.
fn launcher_runs_string(program: &str, arguments: &[String]) -> bool {
    match program {
        "flock" | "script" => arguments
            .iter()
            .take_while(|word| *word != "--")
            .any(|word| {
                matches!(word.as_str(), "--call" | "--command")
                    || word.starts_with("--call=")
                    || word.starts_with("--command=")
                    || word
                        .strip_prefix('-')
                        .is_some_and(|cluster| !cluster.starts_with('-') && cluster.contains('c'))
            }),
        "npx" => package_launcher_options_unreadable(arguments, true),
        // `npm exec <options>`: the subcommand word is not an option.
        "npm" => package_launcher_options_unreadable(arguments.get(1..).unwrap_or_default(), false),
        _ => false,
    }
}

/// True when an option before the package is outside the closed list, which
/// includes every spelling of `-c`/`--call`.
fn package_launcher_options_unreadable(arguments: &[String], ends_at_package: bool) -> bool {
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index].as_str();
        index += 1;
        if argument == "--" {
            return false;
        }
        if !argument.starts_with('-') {
            if ends_at_package {
                return false;
            }
            continue;
        }
        if NPX_FLAGS.contains(&argument) {
            // A boolean option takes an explicit `true`/`false` as its value, so
            // that word is not the package.
            if arguments
                .get(index)
                .is_some_and(|next| matches!(next.as_str(), "true" | "false"))
            {
                index += 1;
            }
            continue;
        }
        if NPX_VALUED.contains(&argument) {
            if arguments
                .get(index)
                .is_some_and(|next| !next.starts_with('-'))
            {
                index += 1;
            }
            continue;
        }
        let inline_value = argument
            .strip_prefix("--package=")
            .or_else(|| argument.strip_prefix("--workspace="));
        if inline_value.is_none() {
            return true;
        }
    }
    false
}

fn base_name(word: &str) -> &str {
    word.rsplit('/').next().unwrap_or(word)
}

/// `resolved` is false when the program was guessed from an unresolvable
/// chain; the stdin-script rule needs a certain program.
fn classify(program: &str, arguments: &[String], resolved: bool) -> Option<String> {
    if SHELLS.contains(&program) {
        return match shell_form(arguments) {
            ShellForm::Command => Some(format!("{program} -c")),
            ShellForm::Stdin if resolved => Some(format!("{program} reading a script from stdin")),
            ShellForm::Stdin | ShellForm::Script => None,
        };
    }
    match program {
        "eval" | "xargs" | "gxargs" | "busybox" => Some(program.to_string()),
        "find" | "gfind" => find_update(arguments).map(|option| format!("{program} {option}")),
        "sed" | "gsed" => {
            (!sed_prints_numeric_range(arguments)).then(|| format!("{program} outside `-n N,Mp`"))
        }
        "node" | "nodejs" => node_inline(arguments).then(|| format!("{program} -e/-p")),
        name if is_python(name) => python_inline(arguments).then(|| format!("{name} -c")),
        name if is_perl(name) => perl_inline(arguments).then(|| format!("{name} -e")),
        _ => None,
    }
}

enum ShellForm {
    /// `-c` (alone or in a cluster) before the script operand.
    Command,
    /// No script operand: the shell reads its program from stdin.
    Stdin,
    /// A script operand (or `--version`/`--help`).
    Script,
}

fn shell_form(arguments: &[String]) -> ShellForm {
    // `-s` reads the script from stdin, but a later `-c` still wins and the
    // words after `-s` are positional parameters, not a script operand.
    let mut reads_stdin = false;
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index].as_str();
        index += 1;
        if argument == "--" {
            return if index < arguments.len() && !reads_stdin {
                ShellForm::Script
            } else {
                ShellForm::Stdin
            };
        }
        if let Some(long) = argument.strip_prefix("--") {
            match long {
                "version" | "help" => {
                    return if reads_stdin {
                        ShellForm::Stdin
                    } else {
                        ShellForm::Script
                    };
                }
                "rcfile" | "init-file" => index += 1,
                _ => {}
            }
            continue;
        }
        if argument == "-" {
            return ShellForm::Stdin;
        }
        let mut chars = argument.chars();
        let sign = chars.next();
        if !matches!(sign, Some('-' | '+')) || argument.len() == 1 {
            return if reads_stdin {
                ShellForm::Stdin
            } else {
                ShellForm::Script
            };
        }
        for ch in chars {
            match ch {
                'c' if sign == Some('-') => return ShellForm::Command,
                's' if sign == Some('-') => reads_stdin = true,
                // `-o name` and `-O name` take the next word.
                // The cluster goes on after the value is taken: `-oc name` is `-c`.
                'o' | 'O' => index += 1,
                _ => {}
            }
        }
    }
    ShellForm::Stdin
}

fn find_update(arguments: &[String]) -> Option<&str> {
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index].as_str();
        index += 1;
        if FIND_UPDATES.contains(&argument) {
            return Some(argument);
        }
        if FIND_PATTERNS.contains(&argument) {
            index += 1;
        }
    }
    None
}

/// The one sed form read here: `-n`, one numeric address or range ending in
/// `p`, and operand files. Anything else is refused without being read.
fn sed_prints_numeric_range(arguments: &[String]) -> bool {
    let [quiet, script, files @ ..] = arguments else {
        return false;
    };
    quiet == "-n" && is_numeric_print(script) && files.iter().all(|file| !file.starts_with('-'))
}

fn is_numeric_print(script: &str) -> bool {
    let Some(range) = script.strip_suffix('p') else {
        return false;
    };
    let mut parts = range.split(',');
    let numeric = |part: Option<&str>| {
        part.is_some_and(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
    };
    let first = numeric(parts.next());
    let second = match parts.next() {
        None => true,
        Some(part) => numeric(Some(part)),
    };
    first && second && parts.next().is_none()
}

/// `node` options up to the entrypoint. `-e`, `--eval`, `-p`, `--print`, a
/// cluster holding `e` or `p`, and a bare `-` (script on stdin) are inline.
fn node_inline(arguments: &[String]) -> bool {
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index].as_str();
        index += 1;
        if argument == "--" {
            return false;
        }
        if argument == "-" {
            return true;
        }
        if let Some(long) = argument.strip_prefix("--") {
            let (name, value) = match long.split_once('=') {
                Some((name, value)) => (name, Some(value)),
                None => (long, None),
            };
            let has_value = value.is_some();
            if matches!(name, "eval" | "print") {
                return true;
            }
            if NODE_CODE_LOADERS.contains(&name) {
                let value = value.or_else(|| arguments.get(index).map(String::as_str));
                if value.is_some_and(is_data_url) {
                    return true;
                }
            }
            // An unknown option may take the next word as its value; that word
            // is then not the entrypoint. Skipping only widens the scan.
            let next_is_plain = arguments
                .get(index)
                .is_some_and(|next| !next.starts_with('-'));
            if !has_value && !NODE_FLAGS.contains(&name) && next_is_plain {
                index += 1;
            }
            continue;
        }
        let Some(cluster) = argument.strip_prefix('-') else {
            return false;
        };
        for (position, ch) in cluster.char_indices() {
            match ch {
                'e' | 'p' => return true,
                // `-r module` and `-C condition` take a value.
                'r' | 'C' => {
                    let rest = &cluster[position + ch.len_utf8()..];
                    let value = if rest.is_empty() {
                        index += 1;
                        arguments.get(index - 1).map(String::as_str)
                    } else {
                        Some(rest)
                    };
                    if ch == 'r' && value.is_some_and(is_data_url) {
                        return true;
                    }
                    break;
                }
                _ => {}
            }
        }
    }
    false
}

/// A `data:` URL handed to a module option is code the command line carries.
fn is_data_url(value: &str) -> bool {
    value.trim_start().to_ascii_lowercase().starts_with("data:")
}

fn is_python(program: &str) -> bool {
    matches!(program, "pypy" | "pypy3")
        || program.strip_prefix("python").is_some_and(|rest| {
            rest.bytes()
                .all(|byte| byte.is_ascii_digit() || byte == b'.')
        })
}

/// `python` options up to `-m` or the script. A cluster holding `c` is inline;
/// `-m` hands the rest of the words to the module.
fn python_inline(arguments: &[String]) -> bool {
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index].as_str();
        index += 1;
        if argument == "--" {
            return false;
        }
        if argument == "-" {
            return true;
        }
        if let Some(long) = argument.strip_prefix("--") {
            if long == "check-hash-based-pycs" {
                index += 1;
            }
            continue;
        }
        let Some(cluster) = argument.strip_prefix('-') else {
            return false;
        };
        for (position, ch) in cluster.char_indices() {
            match ch {
                'c' => return true,
                'm' => return false,
                // `-W arg` and `-X opt` take a value.
                'W' | 'X' => {
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

fn is_perl(program: &str) -> bool {
    program == "perl"
        || program
            .strip_prefix("perl")
            .is_some_and(|rest| rest.starts_with(|ch: char| ch.is_ascii_digit()))
}

/// `perl` switch clusters up to the script. A switch that takes the rest of
/// its cluster as a value ends the cluster, as the parent reads them.
fn perl_inline(arguments: &[String]) -> bool {
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index].as_str();
        index += 1;
        if argument == "--" {
            return false;
        }
        if argument == "-" {
            return true;
        }
        let Some(cluster) = argument.strip_prefix('-') else {
            return false;
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
                'e' | 'E' => return true,
                'I' => {
                    if position == chars.len()
                        && arguments
                            .get(index)
                            .is_some_and(|next| !next.starts_with('-'))
                    {
                        index += 1;
                    }
                    break;
                }
                'm' | 'M' => {
                    let rest: String = chars[position..].iter().collect();
                    let value = if rest.is_empty() {
                        arguments
                            .get(index)
                            .filter(|next| !next.starts_with('-'))
                            .inspect(|_| index += 1)
                            .cloned()
                            .unwrap_or_default()
                    } else {
                        rest
                    };
                    if !is_module_spec(&value) {
                        return true;
                    }
                    break;
                }
                'C' | 'd' | 'D' | 'F' | 'V' | 'x' => break,
                '0' | 'l' => {
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

/// `-M`/`-m` value: `Foo::Bar`, `-Foo`, or `Foo=a,b`. Anything else (a `;`, a
/// space, a call) is perl code the switch would run.
fn is_module_spec(value: &str) -> bool {
    if value.is_empty() {
        return true;
    }
    let value = value.strip_prefix('-').unwrap_or(value);
    let (name, imports) = match value.split_once('=') {
        Some((name, imports)) => (name, imports),
        None => (value, ""),
    };
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b':'))
        && imports.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b':' | b',' | b'.' | b'/' | b'-')
        })
}
