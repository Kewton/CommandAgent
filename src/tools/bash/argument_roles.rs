//! Argument roles of a static simple command (Issue #930).
//!
//! The read-path judge used to send every de-quoted word to the path proof, so
//! a `sed` address, an `awk` program, a `grep` pattern, or the text `echo` and
//! `printf` display was confined as if it named a file. This leaf reads the
//! segments [`super::path_tokens::read_segments`] produced and marks the words
//! whose role is proven to be data rather than a path:
//!
//! * the read-only program of a bare `sed` or `awk` (the grammar lives in
//!   [`read_only_program`]),
//! * the pattern of a bare `grep` (`-e` values and the first operand), the
//!   field separator of `awk -F`,
//! * the operands of a bare `echo`, and the format and operands of a bare
//!   `printf` whose format holds only literal text, `%s`, `%%`, and plain
//!   escapes.
//!
//! The role comes from the command name, the argument position, the options,
//! the redirects, and the quote/expansion metadata — never from what the text
//! looks like. Only a top-level simple command whose name is a bare static word
//! is judged; a prefix assignment, a wrapper, a subshell or process
//! substitution, an input redirect or here-string, an unknown option, a dynamic
//! or glob word, and a pipeline partner that may evaluate or re-read the data
//! all leave every word to the existing path proof. A file operand, an option
//! value that names a file, and a redirect target are never data.
//!
//! Two stricter results come with the judgement and never remove a check: the
//! files a `sed`/`awk` program names for its own I/O (`r`, `getline <` as reads;
//! `w`, `s///w`, awk `>`/`>>` as writes) and the value of a joined `-fFILE`
//! option are returned for the existing proof. A program with I/O or command
//! execution is never data, so its word keeps the reading it had before.

use std::collections::HashSet;

use super::path_tokens::{self, CrMode, Opener, Segment, Word};

mod read_only_program;
#[cfg(test)]
mod tests;

use read_only_program::Verdict;

/// The role judgement of one reading of the command text.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Roles {
    /// Per segment, per word: the word is proven data, not a path.
    data: Vec<Vec<bool>>,
    /// Files to read: a joined `-fFILE` value and the `r`/`getline <` files of a
    /// `sed`/`awk` program.
    pub extra_reads: Vec<String>,
    /// Files a `sed`/`awk` program writes (`w`, `s///w`, awk `>`/`>>`).
    pub extra_writes: Vec<String>,
}

impl Roles {
    /// Whether word `word` of segment `segment` is proven data.
    pub(super) fn is_data(&self, segment: usize, word: usize) -> bool {
        self.data
            .get(segment)
            .and_then(|words| words.get(word))
            .copied()
            .unwrap_or(false)
    }
}

/// Command names that evaluate text as shell code, change what a command name
/// runs, or turn data into file arguments or file lists. A word whose basename
/// is one of them, anywhere in the text (after a wrapper, as an absolute path),
/// closes the data roles for the whole text.
const CLOSING_WORDS: &[&str] = &[
    "alias", "function", "eval", "source", "enable", "hash", "shopt", "declare", "typeset",
    "export", "readonly", "local", "builtin", "command", "exec", "trap", "coproc", "xargs",
    "parallel", "sh", "bash", "zsh", "dash", "ksh", "mksh", "fish", "csh", "tcsh", "busybox",
    "tar", "rsync", "cpio", "zip",
];

/// Interpreters that run inline code (`-c`, `-e`, `-`) or a program read from
/// stdin. A word whose basename is one of them, followed by an option or by
/// nothing, closes the data roles for the whole text.
const INLINE_INTERPRETERS: &[&str] = &[
    "python",
    "python2",
    "python3",
    "node",
    "nodejs",
    "perl",
    "ruby",
    "php",
    "lua",
    "deno",
    "bun",
    "Rscript",
    "tclsh",
    "osascript",
    "pwsh",
];

/// Command names whose program the read-only grammar judges. A word whose
/// basename is one of them closes the data roles for the whole text unless it
/// is a bare `sed`/`awk` whose every program was proven read-only, so a program
/// with file I/O or command execution elsewhere in the text cannot consume the
/// data another command produced.
const PROGRAM_RUNNERS: &[&str] = &["sed", "gsed", "awk", "gawk", "mawk", "nawk"];

/// Substrings of a word that touch the command lookup or shell start-up
/// (`PATH`, `BASH_CMDS`, `BASH_ALIASES`, `BASH_ENV`, `ENV`).
const CLOSING_SUBSTRINGS: &[&str] = &["PATH", "BASH_", "ENV"];

/// Judges the argument roles of the segments of one reading.
pub(super) fn judge(segments: &[Segment]) -> Roles {
    let mut roles = Roles {
        data: segments
            .iter()
            .map(|segment| vec![false; segment.words.len()])
            .collect(),
        extra_reads: Vec::new(),
        extra_writes: Vec::new(),
    };
    let pipelines = pipelines(segments);
    let mut read_only_runners = vec![false; segments.len()];
    let mut members: Vec<Vec<(usize, Member)>> = Vec::new();
    for pipeline in &pipelines {
        let mut judged = Vec::new();
        for command in pipeline {
            let argv = argv(segments, command.segment);
            let analysis = analyze(&argv);
            roles.extra_reads.extend(analysis.extra_reads);
            roles.extra_writes.extend(analysis.extra_writes);
            read_only_runners[command.segment] = analysis.data.is_some()
                && argv
                    .first()
                    .is_some_and(|name| matches!(name.text.as_str(), "sed" | "awk"));
            let member = match analysis.data {
                Some(data) if redirects_allow_data(segments, command) => Member::Table(data),
                _ => partner_kind(&argv),
            };
            judged.push((command.segment, member));
        }
        members.push(judged);
    }
    if !gate_open(segments, &read_only_runners)
        || !every_command_known(segments, &pipelines, &roles.extra_writes)
    {
        return roles;
    }
    for judged in members {
        let alone = judged.len() == 1;
        let partners_allowed = judged
            .iter()
            .all(|(_, member)| !matches!(member, Member::Other));
        if !alone && !partners_allowed {
            continue;
        }
        for (segment, member) in judged {
            if let Member::Table(data) = member {
                for index in data {
                    roles.data[segment][index] = true;
                }
            }
        }
    }
    roles
}

/// The static words that every reading of `text` treats as data and nowhere
/// as anything else (Issue #930). The working-directory walk reads the same
/// text with its own lexer, so it drops a word only when no occurrence of that
/// spelling could be a path, a redirect target, or an option value.
pub(crate) fn data_only_words(text: &str, cr_mode: CrMode) -> HashSet<String> {
    let Some(segments) = path_tokens::read_segments(text, cr_mode) else {
        return HashSet::new();
    };
    let roles = judge(&segments);
    let mut data = HashSet::new();
    let mut other = HashSet::new();
    for (segment_index, segment) in segments.iter().enumerate() {
        for (index, word) in segment.words.iter().enumerate() {
            if roles.is_data(segment_index, index) {
                data.insert(word.text.clone());
            } else {
                other.insert(word.text.clone());
            }
        }
    }
    data.retain(|word| !other.contains(word));
    data
}

/// One simple command: its own segment and the redirect segments after it.
struct Command {
    segment: usize,
    redirects: Vec<usize>,
}

/// Groups the segments into pipelines of commands. A segment that starts
/// with a redirect operator belongs to the command before it.
fn pipelines(segments: &[Segment]) -> Vec<Vec<Command>> {
    let mut pipelines: Vec<Vec<Command>> = Vec::new();
    for (index, segment) in segments.iter().enumerate() {
        match segment.opener {
            Opener::Input | Opener::Output | Opener::Duplicate => {
                let Some(command) = pipelines
                    .last_mut()
                    .and_then(|pipeline| pipeline.last_mut())
                else {
                    continue;
                };
                command.redirects.push(index);
            }
            Opener::Pipe if !pipelines.is_empty() => {
                let Some(pipeline) = pipelines.last_mut() else {
                    continue;
                };
                pipeline.push(Command {
                    segment: index,
                    redirects: Vec::new(),
                });
            }
            _ => pipelines.push(vec![Command {
                segment: index,
                redirects: Vec::new(),
            }]),
        }
    }
    pipelines
}

/// Whether the segment after `index` starts with a redirect operator.
fn closed_by_redirect(segments: &[Segment], index: usize) -> bool {
    segments.get(index + 1).is_some_and(|next| {
        matches!(
            next.opener,
            Opener::Input | Opener::Output | Opener::Duplicate
        )
    })
}

/// The words of a segment without a trailing file-descriptor number glued to
/// the redirect operator after it (`2>`).
fn argv(segments: &[Segment], index: usize) -> Vec<&Word> {
    let segment = &segments[index];
    let mut words: Vec<&Word> = segment.words.iter().collect();
    if segment.glued_close
        && closed_by_redirect(segments, index)
        && words.last().is_some_and(|word| {
            !word.text.is_empty() && word.text.chars().all(|ch| ch.is_ascii_digit())
        })
    {
        words.pop();
    }
    words
}

/// Whether the redirects of a command leave its data roles intact: no input
/// redirect, here-string, or heredoc, and each redirect target is one word, so
/// no argument hides after a target.
fn redirects_allow_data(segments: &[Segment], command: &Command) -> bool {
    command
        .redirects
        .iter()
        .all(|&index| segments[index].opener != Opener::Input && argv(segments, index).len() <= 1)
}

/// How a pipeline member may share a pipe with data.
enum Member {
    /// A table command whose data words were proven.
    Table(Vec<usize>),
    /// A command that neither evaluates its input nor turns it into arguments:
    /// `cat`, or `python3` running a static workspace-relative script file.
    Partner,
    /// Anything else, which closes the data roles of the whole pipeline.
    Other,
}

fn partner_kind(argv: &[&Word]) -> Member {
    let Some(name) = argv.first().filter(|word| is_bare(word)) else {
        return Member::Other;
    };
    match name.text.as_str() {
        "cat" => Member::Partner,
        "python3" if argv.get(1).is_some_and(|script| is_python_script(script)) => Member::Partner,
        _ => Member::Other,
    }
}

/// Whether a word is a static, unquoted command name.
fn is_bare(word: &Word) -> bool {
    !word.quoted && is_static(word)
}

/// A word with no executed expansion: no `$`/backtick, no unquoted glob or
/// brace, and no unquoted tilde.
fn is_static(word: &Word) -> bool {
    !word.dynamic && !word.glob && !word.tilde
}

/// A workspace-relative path spelled without an expansion, an absolute or
/// home prefix, an option dash, or a `.`/`..`/empty component, with a leading
/// `./` removed. Any other spelling returns `None`.
fn clean_relative(word: &Word) -> Option<String> {
    if !is_static(word) {
        return None;
    }
    let mut text = word.text.as_str();
    while let Some(rest) = text.strip_prefix("./") {
        text = rest;
    }
    let clean = !text.is_empty()
        && !text.starts_with(['/', '-', '~'])
        && text
            .split('/')
            .all(|component| !matches!(component, "" | "." | ".."));
    clean.then(|| text.to_string())
}

/// A `python3` script operand: a clean workspace-relative path ending in `.py`.
fn is_python_script(word: &Word) -> bool {
    clean_relative(word).is_some_and(|path| path.ends_with(".py"))
}

/// Commands that leave the data roles open without taking any: their
/// arguments keep the existing path proof. `python3` and `node` are admitted
/// only in the forms [`every_command_known`] reads.
const KNOWN_COMMANDS: &[&str] = &[
    "sed", "awk", "grep", "echo", "printf", "cat", "wc", "head", "tail", "cd", "rm",
];

/// Whether the options of an added reader take no file and no list of files:
/// `wc -l/-c/-w/-m`, `head`/`tail` `-n N`/`-c N` with a numeric value, and
/// `rm -f/-r/-R`. `cat` takes none. Every option word before `--` is read
/// wherever it stands, since GNU tools accept options after operands; any
/// other option (`--files0-from`, an unknown flag, a non-numeric value)
/// closes the roles.
fn plain_options(name: &str, arguments: &[&Word]) -> bool {
    let numeric = |value: &str| {
        let digits = value.strip_prefix(['+', '-']).unwrap_or(value);
        !digits.is_empty() && digits.chars().all(|ch| ch.is_ascii_digit())
    };
    let mut index = 0usize;
    while index < arguments.len() {
        let text = arguments[index].text.as_str();
        index += 1;
        if text == "--" {
            return true;
        }
        let Some(flags) = text.strip_prefix('-').filter(|flags| !flags.is_empty()) else {
            continue;
        };
        let allowed = match name {
            "wc" => flags.chars().all(|ch| matches!(ch, 'l' | 'c' | 'w' | 'm')),
            "rm" => flags.chars().all(|ch| matches!(ch, 'f' | 'r' | 'R')),
            "head" | "tail" => match flags.split_at_checked(1) {
                Some(("n" | "c", "")) => {
                    let value = arguments.get(index).is_some_and(|word| numeric(&word.text));
                    index += 1;
                    value
                }
                Some(("n" | "c", value)) => numeric(value),
                _ => numeric(flags),
            },
            _ => false,
        };
        if !allowed {
            return false;
        }
    }
    true
}

/// Whether every command in the text is one this leaf knows (rule A): a
/// read-only table command, `cat`, `wc`, `head`, `tail`, `cd`, `rm`,
/// `python3 <script>.py [arguments]`, or `node --check <file>`, each named by
/// a bare ASCII word (rule C). A redirect that hides words after its target,
/// a `python3`/`node` file the same text writes (an output redirect target or
/// a sed/awk write), or such a file next to a write the text cannot spell as a
/// clean workspace-relative path or after a `cd` (rule B) closes the roles.
fn every_command_known(
    segments: &[Segment],
    pipelines: &[Vec<Command>],
    program_writes: &[String],
) -> bool {
    let mut written: Vec<String> = Vec::new();
    let mut unknown_write = false;
    for segment in segments {
        if !matches!(segment.opener, Opener::Output | Opener::Duplicate) {
            continue;
        }
        let Some(target) = segment.words.first() else {
            continue;
        };
        let descriptor = segment.opener == Opener::Duplicate
            && target
                .text
                .chars()
                .all(|ch| ch.is_ascii_digit() || ch == '-');
        match clean_relative(target) {
            Some(path) => written.push(path),
            None => unknown_write |= !descriptor,
        }
    }
    for path in program_writes {
        let word = Word {
            text: path.clone(),
            ..Word::default()
        };
        match clean_relative(&word) {
            Some(path) => written.push(path),
            None => unknown_write = true,
        }
    }
    let mut scripts: Vec<String> = Vec::new();
    let mut changes_directory = false;
    for command in pipelines.iter().flatten() {
        if command
            .redirects
            .iter()
            .any(|&index| argv(segments, index).len() > 1)
        {
            return false;
        }
        let argv = argv(segments, command.segment);
        let Some(name) = argv.first() else {
            continue;
        };
        if !is_bare(name) || !name.text.is_ascii() {
            return false;
        }
        match name.text.as_str() {
            "cd" => changes_directory = true,
            "python3" => {
                let Some(script) = argv.get(1).filter(|script| is_python_script(script)) else {
                    return false;
                };
                scripts.extend(clean_relative(script));
            }
            "node" => {
                let file = match argv.as_slice() {
                    [_, check, file] if check.text == "--check" => clean_relative(file),
                    _ => None,
                };
                let Some(file) = file else {
                    return false;
                };
                scripts.push(file);
            }
            "cat" | "wc" | "head" | "tail" | "rm" if !plain_options(&name.text, &argv[1..]) => {
                return false;
            }
            other if KNOWN_COMMANDS.contains(&other) => {}
            _ => return false,
        }
    }
    scripts.is_empty()
        || !(unknown_write
            || (changes_directory && !written.is_empty())
            || scripts.iter().any(|script| written.contains(script)))
}

/// Whether the text leaves room for data roles at all: no subshell or process
/// substitution, no compound redirect the segments cannot hold (`>|`), no
/// evaluator, definition, list consumer, or inline interpreter, no `sed`/`awk`
/// whose program is not proven read-only, and no lookup variable.
/// `read_only_runners[i]` marks a command segment that is a bare `sed`/`awk`
/// whose programs were all proven read-only.
fn gate_open(segments: &[Segment], read_only_runners: &[bool]) -> bool {
    segments.iter().enumerate().all(|(index, segment)| {
        segment.opener != Opener::Group
            && !unreadable_redirect(segments, index)
            && !segment.words.iter().any(is_closing_word)
            && !runs_inline_code(&segment.words)
            && segment.words.iter().enumerate().all(|(at, word)| {
                !names_in(PROGRAM_RUNNERS, &folded_name(&word.text))
                    || (at == 0 && read_only_runners[index])
            })
            && !sources_at_command_position(&segment.words)
    })
}

/// Whether a redirect operator at segment `index` holds no target and is not
/// continued by another redirect operator (`>>`, `>&`, `&>`, `<>`, `<<<`).
/// Such an operator is a compound the segments split apart, such as the `|`
/// of `>|`, after which a real argument would read as another command.
fn unreadable_redirect(segments: &[Segment], index: usize) -> bool {
    let is_redirect = |segment: &Segment| {
        matches!(
            segment.opener,
            Opener::Input | Opener::Output | Opener::Duplicate
        )
    };
    is_redirect(&segments[index])
        && segments[index].words.is_empty()
        && !segments.get(index + 1).is_some_and(is_redirect)
}

/// The final `/`-separated component of a word.
fn basename(word: &str) -> &str {
    word.rsplit('/').next().unwrap_or(word)
}

/// The lowercase basename a closing-name table is compared with. Names are
/// folded on both sides because a case-insensitive filesystem resolves
/// `/bin/SH` to `/bin/sh`.
fn folded_name(word: &str) -> String {
    basename(word).to_lowercase()
}

/// Whether a folded name appears in a table, folding the table too.
fn names_in(table: &[&str], name: &str) -> bool {
    table.iter().any(|entry| entry.to_lowercase() == name)
}

/// Whether a word names a closing command or holds a closing substring.
fn is_closing_word(word: &Word) -> bool {
    names_in(CLOSING_WORDS, &folded_name(&word.text))
        || CLOSING_SUBSTRINGS
            .iter()
            .any(|needle| word.text.contains(needle))
}

/// Whether a word names an interpreter that can run inline code or a program
/// read from stdin.
fn is_interpreter(word: &Word) -> bool {
    let name = folded_name(&word.text);
    names_in(INLINE_INTERPRETERS, &name)
        || name
            .strip_prefix("python")
            .is_some_and(|version| version.chars().all(|ch| ch.is_ascii_digit() || ch == '.'))
}

/// Whether the words run an interpreter in a form other than the few this leaf
/// reads: a static script file as the first argument (`python3 app.py`),
/// `python3 -m module`, or `node --check file`. Any option list before them,
/// `-`, inline code, or no argument at all closes the roles, because the
/// option values and the place an inline-code option takes effect are not
/// read here.
fn runs_inline_code(words: &[Word]) -> bool {
    words.iter().enumerate().any(|(at, word)| {
        if !is_interpreter(word) {
            return false;
        }
        let name = folded_name(&word.text);
        let first = words.get(at + 1).map(|next| next.text.as_str());
        let second = words.get(at + 2).map(|next| next.text.as_str());
        let script = |argument: Option<&str>| argument.is_some_and(|text| !text.starts_with('-'));
        let understood = script(first)
            || (name.starts_with("python") && first == Some("-m") && script(second))
            || (name.starts_with("node") && first == Some("--check") && script(second));
        !understood
    })
}

/// Whether `.` runs a file as shell text: it stands at the command position,
/// after any assignments and reserved words.
fn sources_at_command_position(words: &[Word]) -> bool {
    const RESERVED: &[&str] = &[
        "!", "{", "}", "if", "then", "else", "elif", "fi", "do", "done", "while", "until", "time",
    ];
    words
        .iter()
        .find(|word| !is_assignment(&word.text) && !RESERVED.contains(&word.text.as_str()))
        .is_some_and(|word| word.text == ".")
}

fn is_assignment(word: &str) -> bool {
    let Some((name, _)) = word.split_once('=') else {
        return false;
    };
    let name = name.strip_suffix('+').unwrap_or(name);
    !name.is_empty()
        && name
            .chars()
            .all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

/// What one command's arguments prove.
#[derive(Default)]
struct Analysis {
    /// The argv indices proven data, when the whole command is understood.
    data: Option<Vec<usize>>,
    extra_reads: Vec<String>,
    extra_writes: Vec<String>,
}

/// Judges one command's argv (the program name first).
fn analyze(argv: &[&Word]) -> Analysis {
    let Some(name) = argv.first().filter(|word| is_bare(word)) else {
        return Analysis::default();
    };
    let mut analysis = match name.text.as_str() {
        "sed" => program_command(argv, sed_arguments(argv), read_only_program::sed),
        "awk" => program_command(argv, awk_arguments(argv), read_only_program::awk),
        "grep" => grep_command(argv),
        "echo" => Analysis {
            data: Some((1..argv.len()).collect()),
            ..Analysis::default()
        },
        "printf" => Analysis {
            data: printf_data(argv),
            ..Analysis::default()
        },
        _ => return Analysis::default(),
    };
    if !argv.iter().all(|word| is_static(word)) {
        analysis.data = None;
    }
    analysis
}

/// The option reading of a `sed`, `awk`, or `grep` argv.
#[derive(Default)]
struct Arguments {
    /// Argv indices of inline programs or patterns, with the program text
    /// (the remainder of a joined `-eTEXT`).
    programs: Vec<(usize, String)>,
    /// Argv indices of other data words (`awk -F` values).
    data: Vec<usize>,
    /// Joined option values that name a file to read.
    extra_reads: Vec<String>,
    /// Every option is one the data roles support.
    supported: bool,
}

fn program_command(
    argv: &[&Word],
    arguments: Option<Arguments>,
    grammar: fn(&str) -> (Verdict, read_only_program::Targets),
) -> Analysis {
    let Some(arguments) = arguments else {
        return Analysis::default();
    };
    let mut analysis = Analysis {
        extra_reads: arguments.extra_reads,
        ..Analysis::default()
    };
    let mut read_only = arguments.supported && !arguments.programs.is_empty();
    for (index, program) in &arguments.programs {
        if !is_static(argv[*index]) {
            read_only = false;
            continue;
        }
        let (verdict, targets) = grammar(program);
        analysis.extra_reads.extend(targets.reads);
        analysis.extra_writes.extend(targets.writes);
        if verdict != Verdict::ReadOnly {
            read_only = false;
        }
    }
    if read_only {
        let mut data: Vec<usize> = arguments.programs.iter().map(|(index, _)| *index).collect();
        data.extend(arguments.data);
        analysis.data = Some(data);
    }
    analysis
}

/// The value of an option that takes one: the rest of the word, or the next
/// word. Returns the value and whether it was joined, or `None` when the next
/// word is missing.
fn option_value(argv: &[&Word], index: usize, rest: &str) -> Option<(String, bool)> {
    if rest.is_empty() {
        argv.get(index + 1).map(|word| (word.text.clone(), false))
    } else {
        Some((rest.to_string(), true))
    }
}

/// Reads a `sed` argv. Options must all come before the first operand, so a
/// later `-i` or `-e` cannot be read differently by a permuting and a
/// non-permuting implementation.
fn sed_arguments(argv: &[&Word]) -> Option<Arguments> {
    let mut arguments = Arguments {
        supported: true,
        ..Arguments::default()
    };
    let mut operands = Vec::new();
    let mut script_option = false;
    let mut options_ended = false;
    let mut index = 1;
    while index < argv.len() {
        let text = argv[index].text.as_str();
        if options_ended || !text.starts_with('-') || text == "-" {
            operands.push(index);
            index += 1;
            continue;
        }
        if !operands.is_empty() {
            return None;
        }
        let mut consumed_next = false;
        if text == "--" {
            options_ended = true;
        } else if let Some(long) = text.strip_prefix("--") {
            let (name, value) = match long.split_once('=') {
                Some((name, value)) => (name, Some(value)),
                None => (long, None),
            };
            match name {
                "expression" => {
                    script_option = true;
                    match value {
                        Some(value) => arguments.programs.push((index, value.to_string())),
                        None => {
                            let script = argv.get(index + 1)?;
                            arguments.programs.push((index + 1, script.text.clone()));
                            consumed_next = true;
                        }
                    }
                }
                "file" => {
                    script_option = true;
                    arguments.supported = false;
                    consumed_next = value.is_none();
                }
                "quiet" | "silent" | "regexp-extended" | "posix" | "debug" | "sandbox"
                | "separate" | "unbuffered" | "null-data" | "in-place" | "follow-symlinks" => {
                    arguments.supported = false;
                }
                _ => return None,
            }
        } else {
            let letters: Vec<char> = text.chars().skip(1).collect();
            for (at, letter) in letters.iter().enumerate() {
                let rest: String = letters[at + 1..].iter().collect();
                match letter {
                    'n' | 'E' | 'r' => {}
                    's' | 'u' | 'z' => arguments.supported = false,
                    // The rest of the word is the in-place suffix.
                    'i' => {
                        arguments.supported = false;
                        break;
                    }
                    'e' => {
                        let (script, joined) = option_value(argv, index, &rest)?;
                        script_option = true;
                        let at_index = if joined { index } else { index + 1 };
                        arguments.programs.push((at_index, script));
                        consumed_next = !joined;
                        break;
                    }
                    'f' => {
                        let (file, joined) = option_value(argv, index, &rest)?;
                        script_option = true;
                        arguments.supported = false;
                        if joined {
                            arguments.extra_reads.push(file);
                        }
                        consumed_next = !joined;
                        break;
                    }
                    'l' => {
                        let (_, joined) = option_value(argv, index, &rest)?;
                        arguments.supported = false;
                        consumed_next = !joined;
                        break;
                    }
                    _ => return None,
                }
            }
        }
        index += if consumed_next { 2 } else { 1 };
    }
    if !script_option {
        let script = *operands.first()?;
        arguments.programs.push((script, argv[script].text.clone()));
    }
    Some(arguments)
}

/// Reads an `awk` argv: `-F` and `--` before the program, then the program,
/// then file operands. An option word after the program is not read.
fn awk_arguments(argv: &[&Word]) -> Option<Arguments> {
    let mut arguments = Arguments {
        supported: true,
        ..Arguments::default()
    };
    let mut script_file = false;
    let mut program_seen = false;
    let mut options_ended = false;
    let mut index = 1;
    while index < argv.len() {
        let text = argv[index].text.as_str();
        let option = !options_ended && text.starts_with('-') && text != "-";
        if program_seen || (script_file && !option) {
            if text.starts_with('-') && text != "-" {
                return None;
            }
            index += 1;
            continue;
        }
        if !option {
            arguments.programs.push((index, text.to_string()));
            program_seen = true;
            index += 1;
            continue;
        }
        if text == "--" {
            options_ended = true;
            index += 1;
            continue;
        }
        let mut letters = text.chars().skip(1);
        let letter = letters.next();
        let rest: String = letters.collect();
        let rest = rest.as_str();
        let joined = !rest.is_empty();
        match letter {
            Some('F') => {
                option_value(argv, index, rest)?;
                arguments.data.push(if joined { index } else { index + 1 });
            }
            Some('v') => {
                option_value(argv, index, rest)?;
                arguments.supported = false;
            }
            Some('f') => {
                let (file, _) = option_value(argv, index, rest)?;
                script_file = true;
                arguments.supported = false;
                if joined {
                    arguments.extra_reads.push(file);
                }
            }
            _ => return None,
        }
        index += if joined { 1 } else { 2 };
    }
    if !script_file && !program_seen {
        return None;
    }
    Some(arguments)
}

/// Reads a `grep` argv: `-e`/`--regexp` values and, without them, the first
/// operand are patterns (data); `-f`/`--file` values and the other operands
/// are files.
fn grep_command(argv: &[&Word]) -> Analysis {
    let mut analysis = Analysis::default();
    let mut patterns = Vec::new();
    let mut pattern_option = false;
    let mut operands = Vec::new();
    let mut options_ended = false;
    let mut index = 1;
    while index < argv.len() {
        let text = argv[index].text.as_str();
        if options_ended || !text.starts_with('-') || text == "-" {
            operands.push(index);
            index += 1;
            continue;
        }
        if !operands.is_empty() {
            return analysis;
        }
        let mut consumed_next = false;
        if text == "--" {
            options_ended = true;
        } else if let Some(long) = text.strip_prefix("--") {
            let (name, value) = match long.split_once('=') {
                Some((name, value)) => (name, Some(value)),
                None => (long, None),
            };
            match name {
                "regexp" => {
                    pattern_option = true;
                    if value.is_some() {
                        patterns.push(index);
                    } else if argv.get(index + 1).is_some() {
                        patterns.push(index + 1);
                        consumed_next = true;
                    } else {
                        return analysis;
                    }
                }
                "file" => {
                    pattern_option = true;
                    if value.is_none() {
                        if argv.get(index + 1).is_none() {
                            return analysis;
                        }
                        consumed_next = true;
                    }
                }
                _ => return analysis,
            }
        } else {
            let letters: Vec<char> = text.chars().skip(1).collect();
            for (at, letter) in letters.iter().enumerate() {
                let rest: String = letters[at + 1..].iter().collect();
                match letter {
                    'n' | 'E' | 'F' => {}
                    'e' => {
                        let Some((_, joined)) = option_value(argv, index, &rest) else {
                            return analysis;
                        };
                        pattern_option = true;
                        patterns.push(if joined { index } else { index + 1 });
                        consumed_next = !joined;
                        break;
                    }
                    'f' => {
                        let Some((file, joined)) = option_value(argv, index, &rest) else {
                            return analysis;
                        };
                        pattern_option = true;
                        if joined {
                            analysis.extra_reads.push(file);
                        }
                        consumed_next = !joined;
                        break;
                    }
                    _ => return analysis,
                }
            }
        }
        index += if consumed_next { 2 } else { 1 };
    }
    if !pattern_option {
        let Some(&pattern) = operands.first() else {
            return analysis;
        };
        patterns.push(pattern);
    }
    analysis.data = Some(patterns);
    analysis
}

/// The data words of a `printf` argv: every word after the name when the
/// format has no side effect, or `None`.
fn printf_data(argv: &[&Word]) -> Option<Vec<usize>> {
    let format_index = if argv.get(1).is_some_and(|word| word.text == "--") {
        2
    } else {
        1
    };
    let format = argv.get(format_index)?;
    if format.text.starts_with('-') || !is_plain_format(&format.text) {
        return None;
    }
    Some((1..argv.len()).collect())
}

/// Whether a `printf` format holds only literal text, `%s`, `%%`, and the plain
/// escapes `\n \t \r \\ \" \'`.
fn is_plain_format(format: &str) -> bool {
    let mut chars = format.chars();
    while let Some(ch) = chars.next() {
        match ch {
            '%' if !matches!(chars.next(), Some('s' | '%')) => return false,
            '\\' if !matches!(chars.next(), Some('n' | 't' | 'r' | '\\' | '"' | '\'')) => {
                return false;
            }
            _ => {}
        }
    }
    true
}
