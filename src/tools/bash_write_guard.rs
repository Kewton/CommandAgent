use std::path::Path;

mod command_prefix;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct BashWriteConfinementRejection {
    pub path: String,
    pub operation: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WriteTarget {
    path: String,
    operation: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ShellToken {
    Word(String),
    OutputRedirect,
    InputRedirect,
    DescriptorRedirect,
    SegmentEnd,
}

pub(super) fn confinement_rejection(
    command: &str,
    root: &Path,
) -> Option<BashWriteConfinementRejection> {
    for target in write_targets(command) {
        if target.operation == command_prefix::UNRESOLVED_OPERATION {
            return Some(BashWriteConfinementRejection {
                reason: format!(
                    "Bash command prefix `{}` cannot be resolved, so the write targets it launches cannot be proven to remain in the Gate 1 workspace boundary",
                    target.path
                ),
                path: target.path,
                operation: target.operation,
            });
        }
        if target.path == "/dev/null"
            && matches!(target.operation.as_str(), "output redirection" | "tee")
        {
            continue;
        }
        if target.operation == "working directory" && target.path == "-" {
            return Some(BashWriteConfinementRejection {
                path: target.path,
                operation: target.operation,
                reason: "Bash working directory target `-` depends on OLDPWD and cannot be proven to remain in the Gate 1 workspace boundary".to_string(),
            });
        }
        if let Err(error) = super::path_guard::ensure_bash_write_target(root, &target.path) {
            return Some(BashWriteConfinementRejection {
                reason: format!(
                    "Bash {} target `{}` is outside the Gate 1 workspace boundary: {error}",
                    target.operation, target.path
                ),
                path: target.path,
                operation: target.operation,
            });
        }
    }
    None
}

pub(super) fn has_recognized_mutation(command: &str) -> bool {
    write_targets(command)
        .iter()
        .any(|target| target.operation != "working directory")
}

pub(crate) fn protected_path_mutation(
    command: &str,
    root: &Path,
    protected_paths: &[String],
) -> Option<String> {
    write_targets(command)
        .into_iter()
        .filter(|target| target.operation != "working directory")
        .find_map(|target| {
            let candidate = Path::new(&target.path);
            let relative = if candidate.is_absolute() {
                match candidate.strip_prefix(root) {
                    Ok(relative) => relative,
                    Err(_) => {
                        return protected_paths
                            .iter()
                            .find(|protected| candidate.ends_with(protected))
                            .cloned();
                    }
                }
            } else {
                candidate
            };
            protected_paths
                .iter()
                .find(|protected| path_matches(relative, Path::new(protected)))
                .cloned()
        })
}

fn path_matches(candidate: &Path, protected: &Path) -> bool {
    candidate == protected || candidate.starts_with(protected) || protected.starts_with(candidate)
}

fn write_targets(command: &str) -> Vec<WriteTarget> {
    let Some(tokens) = shell_tokens(command) else {
        return Vec::new();
    };
    let mut targets = redirect_targets(&tokens);
    for segment in tokens.split(|token| *token == ShellToken::SegmentEnd) {
        let words = command_words(segment);
        if words.is_empty() {
            continue;
        }
        match command_prefix::resolve(&words) {
            command_prefix::Resolution::Program {
                program,
                arguments,
                writes,
            } => {
                push_prefix_writes(&mut targets, writes);
                let arguments: Vec<&str> = arguments.iter().map(String::as_str).collect();
                collect_program_targets(&program, &arguments, &mut targets);
            }
            command_prefix::Resolution::NoExecution { writes } => {
                push_prefix_writes(&mut targets, writes);
            }
            command_prefix::Resolution::Undecidable { prefix } => {
                targets.push(WriteTarget {
                    path: prefix,
                    operation: command_prefix::UNRESOLVED_OPERATION.to_string(),
                });
            }
        }
    }
    targets
}

fn push_prefix_writes(targets: &mut Vec<WriteTarget>, writes: Vec<command_prefix::PrefixWrite>) {
    targets.extend(writes.into_iter().map(|write| WriteTarget {
        path: write.path,
        operation: write.operation,
    }));
}

fn collect_program_targets(program: &str, arguments: &[&str], targets: &mut Vec<WriteTarget>) {
    let operands = positional_operands(arguments);
    match program {
        "ln" => {
            if let Some(target_directory) = target_directory(arguments) {
                targets.push(WriteTarget {
                    path: target_directory,
                    operation: program.to_string(),
                });
                if symbolic_link_requested(arguments) {
                    targets.extend(operands.iter().map(|path| WriteTarget {
                        path: (*path).to_string(),
                        operation: "symlink target".to_string(),
                    }));
                }
            } else if let Some(destination) = operands.get(1..).and_then(|items| items.last()) {
                targets.push(WriteTarget {
                    path: (*destination).to_string(),
                    operation: program.to_string(),
                });
                if symbolic_link_requested(arguments) {
                    targets.extend(
                        operands[..operands.len() - 1]
                            .iter()
                            .map(|path| WriteTarget {
                                path: (*path).to_string(),
                                operation: "symlink target".to_string(),
                            }),
                    );
                }
            }
        }
        "cp" | "mv" | "install" => {
            if let Some(target_directory) = target_directory(arguments) {
                targets.push(WriteTarget {
                    path: target_directory,
                    operation: program.to_string(),
                });
            } else if program == "install"
                && arguments
                    .iter()
                    .any(|argument| matches!(*argument, "-d" | "--directory"))
            {
                targets.extend(operands.into_iter().map(|path| WriteTarget {
                    path: path.to_string(),
                    operation: program.to_string(),
                }));
            } else if let Some(destination) = operands.last() {
                targets.push(WriteTarget {
                    path: (*destination).to_string(),
                    operation: program.to_string(),
                });
            }
        }
        "tee" | "mkdir" | "rm" | "touch" | "truncate" => {
            targets.extend(operands.into_iter().map(|path| WriteTarget {
                path: path.to_string(),
                operation: program.to_string(),
            }));
        }
        "chmod" | "chown" => {
            targets.extend(operands.into_iter().skip(1).map(|path| WriteTarget {
                path: path.to_string(),
                operation: program.to_string(),
            }));
        }
        "cd" => {
            targets.push(WriteTarget {
                path: operands.first().copied().unwrap_or("~").to_string(),
                operation: "working directory".to_string(),
            });
        }
        "sudoedit" => {
            targets.extend(operands.into_iter().map(|path| WriteTarget {
                path: path.to_string(),
                operation: program.to_string(),
            }));
        }
        _ => {}
    }
}

fn redirect_targets(tokens: &[ShellToken]) -> Vec<WriteTarget> {
    tokens
        .windows(2)
        .filter_map(|window| match window {
            [ShellToken::OutputRedirect, ShellToken::Word(path)] => Some(WriteTarget {
                path: path.clone(),
                operation: "output redirection".to_string(),
            }),
            [ShellToken::DescriptorRedirect, ShellToken::Word(path)]
                if !is_descriptor_word(path) =>
            {
                Some(WriteTarget {
                    path: path.clone(),
                    operation: "output redirection".to_string(),
                })
            }
            _ => None,
        })
        .collect()
}

fn is_descriptor_word(word: &str) -> bool {
    if word == "-" {
        return true;
    }
    let digits = word.strip_suffix('-').unwrap_or(word);
    !digits.is_empty() && digits.chars().all(|ch| ch.is_ascii_digit())
}

fn command_words(tokens: &[ShellToken]) -> Vec<&str> {
    let mut words = Vec::new();
    let mut skip_redirect_target = false;
    for token in tokens {
        match token {
            ShellToken::OutputRedirect | ShellToken::InputRedirect => {
                skip_redirect_target = true;
            }
            ShellToken::DescriptorRedirect => skip_redirect_target = true,
            ShellToken::Word(word) if skip_redirect_target => skip_redirect_target = false,
            ShellToken::Word(word) => words.push(word.as_str()),
            ShellToken::SegmentEnd => {}
        }
    }
    let assignment_count = words
        .iter()
        .take_while(|word| is_environment_assignment(word))
        .count();
    words.drain(..assignment_count);
    words
}

fn positional_operands<'a>(arguments: &'a [&'a str]) -> Vec<&'a str> {
    let mut options_ended = false;
    arguments
        .iter()
        .copied()
        .filter(|argument| {
            if options_ended {
                return true;
            }
            if *argument == "--" {
                options_ended = true;
                return false;
            }
            !argument.starts_with('-') || *argument == "-"
        })
        .collect()
}

fn target_directory(arguments: &[&str]) -> Option<String> {
    let mut index = 0usize;
    while index < arguments.len() {
        let argument = arguments[index];
        if let Some(path) = argument.strip_prefix("--target-directory=") {
            return Some(path.to_string());
        }
        if argument == "--target-directory" || argument == "-t" {
            return arguments.get(index + 1).map(|path| (*path).to_string());
        }
        if let Some(path) = argument.strip_prefix("-t")
            && !path.is_empty()
        {
            return Some(path.to_string());
        }
        index += 1;
    }
    None
}

fn symbolic_link_requested(arguments: &[&str]) -> bool {
    arguments
        .iter()
        .take_while(|argument| **argument != "--")
        .any(|argument| {
            *argument == "--symbolic"
                || argument
                    .strip_prefix('-')
                    .is_some_and(|flags| !flags.starts_with('-') && flags.contains('s'))
        })
}

fn is_environment_assignment(word: &str) -> bool {
    let Some((name, _)) = word.split_once('=') else {
        return false;
    };
    !name.is_empty()
        && name
            .chars()
            .all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn shell_tokens(command: &str) -> Option<Vec<ShellToken>> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut word_started = false;
    let mut chars = command.chars().peekable();
    let mut single_quoted = false;
    let mut double_quoted = false;

    while let Some(ch) = chars.next() {
        if single_quoted {
            if ch == '\'' {
                single_quoted = false;
            } else {
                current.push(ch);
            }
            continue;
        }
        if double_quoted {
            match ch {
                '"' => double_quoted = false,
                '\\' => current.push(chars.next()?),
                _ => current.push(ch),
            }
            continue;
        }
        match ch {
            '\'' => {
                single_quoted = true;
                word_started = true;
            }
            '"' => {
                double_quoted = true;
                word_started = true;
            }
            '\\' => {
                current.push(chars.next()?);
                word_started = true;
            }
            '>' => {
                if word_started && current.chars().all(|ch| ch.is_ascii_digit()) {
                    current.clear();
                    word_started = false;
                } else {
                    push_word(&mut tokens, &mut current, &mut word_started);
                }
                if chars.peek() == Some(&'>') {
                    chars.next();
                }
                if chars.peek() == Some(&'&') {
                    chars.next();
                    tokens.push(ShellToken::DescriptorRedirect);
                } else {
                    if chars.peek() == Some(&'|') {
                        chars.next();
                    }
                    tokens.push(ShellToken::OutputRedirect);
                }
            }
            '<' => {
                push_word(&mut tokens, &mut current, &mut word_started);
                if chars.peek() == Some(&'<') {
                    chars.next();
                }
                tokens.push(ShellToken::InputRedirect);
            }
            ';' | '|' | '&' | '(' | ')' | '\n' | '\r' => {
                push_word(&mut tokens, &mut current, &mut word_started);
                if chars.peek() == Some(&ch) {
                    chars.next();
                }
                if tokens.last() != Some(&ShellToken::SegmentEnd) {
                    tokens.push(ShellToken::SegmentEnd);
                }
            }
            ch if ch.is_whitespace() => {
                push_word(&mut tokens, &mut current, &mut word_started);
            }
            _ => {
                current.push(ch);
                word_started = true;
            }
        }
    }
    if single_quoted || double_quoted {
        return None;
    }
    push_word(&mut tokens, &mut current, &mut word_started);
    Some(tokens)
}

fn push_word(tokens: &mut Vec<ShellToken>, current: &mut String, word_started: &mut bool) {
    if *word_started {
        tokens.push(ShellToken::Word(std::mem::take(current)));
        *word_started = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_write_destinations_and_external_symlink_targets() {
        let targets = write_targets(
            "ln -s /usr/bin/python3 /usr/local/bin/python 2>/dev/null || cp input.txt output.txt",
        );

        assert_eq!(
            targets,
            vec![
                WriteTarget {
                    path: "/dev/null".to_string(),
                    operation: "output redirection".to_string(),
                },
                WriteTarget {
                    path: "/usr/local/bin/python".to_string(),
                    operation: "ln".to_string(),
                },
                WriteTarget {
                    path: "/usr/bin/python3".to_string(),
                    operation: "symlink target".to_string(),
                },
                WriteTarget {
                    path: "output.txt".to_string(),
                    operation: "cp".to_string(),
                },
            ]
        );
    }

    #[test]
    fn descriptor_redirection_does_not_hide_the_last_command_destination() {
        let targets = write_targets("ln -s source /usr/local/bin/python 2>&1");

        assert_eq!(
            targets,
            vec![
                WriteTarget {
                    path: "/usr/local/bin/python".to_string(),
                    operation: "ln".to_string(),
                },
                WriteTarget {
                    path: "source".to_string(),
                    operation: "symlink target".to_string(),
                },
            ]
        );
    }

    #[test]
    fn one_operand_ln_treats_the_operand_as_a_source() {
        assert!(write_targets("ln /usr/bin/python3").is_empty());
    }

    #[test]
    fn rejects_outside_and_symlinked_targets_but_allows_workspace_targets() {
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path().join("workspace");
        let outside = fixture.path().join("outside");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&outside).unwrap();

        assert!(
            confinement_rejection(
                &format!("printf forbidden > {}", outside.join("file").display()),
                &root,
            )
            .is_some()
        );
        assert!(confinement_rejection("mkdir -p output", &root).is_none());

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&outside, root.join("linked-outside")).unwrap();
            assert!(
                confinement_rejection("tee linked-outside/file", &root).is_some(),
                "an existing intermediate symlink must be canonicalized"
            );
        }
    }

    #[test]
    fn rejects_dynamic_and_parent_relative_write_targets() {
        let root = tempfile::tempdir().unwrap();
        let outside = root
            .path()
            .parent()
            .unwrap()
            .join("issue-206-symlink-outside");

        assert!(confinement_rejection("tee $HOME/file", root.path()).is_some());
        assert!(confinement_rejection("mkdir ../outside", root.path()).is_some());
        assert!(confinement_rejection("touch ~/outside", root.path()).is_some());
        assert!(confinement_rejection("cd /tmp && printf outside > file", root.path()).is_some());
        assert!(confinement_rejection("cd - && printf outside > file", root.path()).is_some());
        assert!(confinement_rejection("cd && printf outside > file", root.path()).is_some());
        assert!(confinement_rejection("rm /dev/null", root.path()).is_some());
        assert!(
            confinement_rejection(
                &format!("ln -s {} linked-outside", outside.display()),
                root.path(),
            )
            .is_some(),
            "a new workspace symlink must not point outside the workspace"
        );
    }

    fn write_target_pairs(command: &str) -> Vec<(String, String)> {
        write_targets(command)
            .into_iter()
            .map(|target| (target.path, target.operation))
            .collect()
    }

    fn expected_targets(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(path, operation)| ((*path).to_string(), (*operation).to_string()))
            .collect()
    }

    #[test]
    fn extracts_write_targets_for_the_supported_command_table() {
        let cases: &[(&str, &[(&str, &str)])] = &[
            ("printf x > out.txt", &[("out.txt", "output redirection")]),
            ("printf x >> out.txt", &[("out.txt", "output redirection")]),
            ("printf x 2> err", &[("err", "output redirection")]),
            (
                "printf x 2>&1 > out.txt",
                &[("out.txt", "output redirection")],
            ),
            ("cat < in > out", &[("out", "output redirection")]),
            ("cat << EOF > out", &[("out", "output redirection")]),
            ("echo a>out", &[("out", "output redirection")]),
            ("echo 2>out", &[("out", "output redirection")]),
            ("tee a>out", &[("out", "output redirection"), ("a", "tee")]),
            ("tee a b", &[("a", "tee"), ("b", "tee")]),
            ("tee -a a", &[("a", "tee")]),
            ("FOO=1 tee f", &[("f", "tee")]),
            ("FOO_BAR=1 tee f", &[("f", "tee")]),
            ("A-B=1 tee f", &[]),
            ("=x tee f", &[]),
            ("tee 'a b'", &[("a b", "tee")]),
            ("tee \"a b\"", &[("a b", "tee")]),
            ("tee a\\ b", &[("a b", "tee")]),
            ("tee \"a\\\"b\"", &[("a\"b", "tee")]),
            ("tee 'it''s'", &[("its", "tee")]),
            ("tee 'unterminated", &[]),
            ("cp a b", &[("b", "cp")]),
            ("cp -t dir a", &[("dir", "cp")]),
            ("cp -tdir a", &[("dir", "cp")]),
            ("cp --target-directory=dir a", &[("dir", "cp")]),
            ("cp --target-directory dir a", &[("dir", "cp")]),
            ("cp a -t dir", &[("dir", "cp")]),
            ("cp -d src dst", &[("dst", "cp")]),
            ("install -d d1 d2", &[("d1", "install"), ("d2", "install")]),
            ("ln -s src dst", &[("dst", "ln"), ("src", "symlink target")]),
            (
                "ln --symbolic src dst",
                &[("dst", "ln"), ("src", "symlink target")],
            ),
            ("ln -f src dst", &[("dst", "ln")]),
            ("chmod 600 f", &[("f", "chmod")]),
            ("chown u:g f", &[("f", "chown")]),
            ("true && tee a", &[("a", "tee")]),
            ("false || tee a", &[("a", "tee")]),
            ("(tee a)", &[("a", "tee")]),
            ("tee a; tee b", &[("a", "tee"), ("b", "tee")]),
            ("tee a;tee b", &[("a", "tee"), ("b", "tee")]),
            ("cd src", &[("src", "working directory")]),
            (
                "cd src && touch a",
                &[("src", "working directory"), ("a", "touch")],
            ),
        ];
        for (command, expected) in cases {
            assert_eq!(
                write_target_pairs(command),
                expected_targets(expected),
                "command: {command}"
            );
        }
    }

    #[test]
    fn shell_tokens_collapses_separators_and_recognizes_redirection() {
        assert_eq!(
            shell_tokens("a && b"),
            Some(vec![
                ShellToken::Word("a".to_string()),
                ShellToken::SegmentEnd,
                ShellToken::Word("b".to_string()),
            ])
        );
        assert_eq!(
            shell_tokens("a ;; b"),
            Some(vec![
                ShellToken::Word("a".to_string()),
                ShellToken::SegmentEnd,
                ShellToken::Word("b".to_string()),
            ])
        );
        assert_eq!(
            shell_tokens("cat < in"),
            Some(vec![
                ShellToken::Word("cat".to_string()),
                ShellToken::InputRedirect,
                ShellToken::Word("in".to_string()),
            ])
        );
        assert_eq!(
            shell_tokens("cat << EOF"),
            Some(vec![
                ShellToken::Word("cat".to_string()),
                ShellToken::InputRedirect,
                ShellToken::Word("EOF".to_string()),
            ])
        );
        assert_eq!(shell_tokens("tee 'unterminated"), None);
    }

    #[test]
    fn has_recognized_mutation_ignores_working_directory_targets() {
        assert!(!has_recognized_mutation("cd src"));
        assert!(has_recognized_mutation("touch a"));
    }

    #[test]
    fn protected_path_mutation_matches_relative_absolute_and_prefix_targets() {
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path().join("workspace");
        let other = fixture.path().join("elsewhere");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&other).unwrap();
        let protected = vec!["tests/spec.ts".to_string()];

        for command in ["rm tests/spec.ts", "rm tests/spec.ts/extra", "rm -rf tests"] {
            assert_eq!(
                protected_path_mutation(command, &root, &protected),
                Some("tests/spec.ts".to_string()),
                "command: {command}"
            );
        }

        assert_eq!(
            protected_path_mutation(
                &format!("rm {}/tests/spec.ts", root.display()),
                &root,
                &protected,
            ),
            Some("tests/spec.ts".to_string())
        );
        assert_eq!(
            protected_path_mutation(
                &format!("rm {}/tests/spec.ts", other.display()),
                &root,
                &protected,
            ),
            Some("tests/spec.ts".to_string())
        );

        assert!(protected_path_mutation("rm src/a.ts", &root, &protected).is_none());
        assert!(protected_path_mutation("cd tests", &root, &protected).is_none());
    }

    #[test]
    fn confinement_rejection_skips_dev_null_and_rejects_dash_working_directory() {
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path().join("workspace");
        std::fs::create_dir_all(&root).unwrap();

        assert!(confinement_rejection("printf x > /dev/null", &root).is_none());
        assert!(confinement_rejection("tee /dev/null", &root).is_none());
        assert!(confinement_rejection("cd src", &root).is_none());
        assert!(confinement_rejection("cd -", &root).is_some());
    }

    #[test]
    fn extracts_redirect_targets_for_noclobber_and_descriptor_duplication_forms() {
        let cases: &[(&str, &[(&str, &str)])] = &[
            ("printf x >| out", &[("out", "output redirection")]),
            ("printf x >|out", &[("out", "output redirection")]),
            ("printf x >>| out", &[("out", "output redirection")]),
            ("printf x >& out", &[("out", "output redirection")]),
            ("printf x >&out", &[("out", "output redirection")]),
            ("printf x 1>&out", &[("out", "output redirection")]),
            ("printf x 2>&out", &[("out", "output redirection")]),
            ("printf x >>& out", &[("out", "output redirection")]),
            ("printf x >&$fd", &[("$fd", "output redirection")]),
            ("printf x >& out 2>&1", &[("out", "output redirection")]),
            ("printf x 2>&1", &[]),
            ("printf x >&2", &[]),
            ("printf x 1>&2", &[]),
            ("printf x >&-", &[]),
            ("printf x 2>&-", &[]),
            ("printf x >&1-", &[]),
            ("printf x >&'2'", &[]),
            ("exec 3>&-", &[]),
        ];
        for (command, expected) in cases {
            assert_eq!(
                write_target_pairs(command),
                expected_targets(expected),
                "command: {command}"
            );
        }
    }

    #[test]
    fn shell_tokens_consumes_noclobber_bar_after_redirect() {
        assert_eq!(
            shell_tokens("a >| b"),
            Some(vec![
                ShellToken::Word("a".to_string()),
                ShellToken::OutputRedirect,
                ShellToken::Word("b".to_string()),
            ])
        );
        assert_eq!(
            shell_tokens("a >>| b"),
            Some(vec![
                ShellToken::Word("a".to_string()),
                ShellToken::OutputRedirect,
                ShellToken::Word("b".to_string()),
            ])
        );
    }

    #[test]
    fn has_recognized_mutation_recognizes_noclobber_and_descriptor_write_forms() {
        assert!(!has_recognized_mutation("cd src"));
        assert!(has_recognized_mutation("touch a"));
        assert!(has_recognized_mutation("printf x >| out"));
        assert!(has_recognized_mutation("printf x >& out"));
        assert!(!has_recognized_mutation("cargo test 2>&1"));
    }

    #[test]
    fn confinement_rejection_covers_noclobber_and_descriptor_write_forms() {
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path().join("workspace");
        let outside = fixture.path().join("outside");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&outside).unwrap();

        let temp_outside = outside.join("file").display().to_string();
        let tmp_unique = format!("/tmp/commandagent-issue-565-{}.txt", std::process::id());
        for template in [
            "printf x >| {}",
            "printf x >>| {}",
            "printf x >& {}",
            "printf x >&{}",
            "printf x 1>&{}",
        ] {
            for outside_path in [temp_outside.as_str(), tmp_unique.as_str()] {
                let command = template.replace("{}", outside_path);
                assert!(
                    confinement_rejection(&command, &root).is_some(),
                    "command must be rejected: {command}"
                );
            }
        }

        assert!(
            confinement_rejection("printf x >&$fd", &root).is_some(),
            "a dynamic descriptor-duplication word must be rejected"
        );

        for command in [
            "printf x >| out.txt",
            "printf x >& out.txt",
            "printf x >& /dev/null",
            "printf x 2>&1",
            "printf x >&2",
            "printf x >&-",
        ] {
            assert!(
                confinement_rejection(command, &root).is_none(),
                "command must stay allowed: {command}"
            );
        }
    }

    #[test]
    fn command_prefix_write_targets_table() {
        let cases: &[(&str, &[(&str, &str)])] = &[
            ("env tee /tmp/f", &[("/tmp/f", "tee")]),
            ("env FOO=1 cp a.txt /tmp/f", &[("/tmp/f", "cp")]),
            ("sudo cp a.txt /tmp/f", &[("/tmp/f", "cp")]),
            ("command cp a.txt /tmp/f", &[("/tmp/f", "cp")]),
            ("nohup tee /tmp/f", &[("/tmp/f", "tee")]),
            ("timeout 5 cp a.txt /tmp/f", &[("/tmp/f", "cp")]),
            ("{ tee /tmp/f; }", &[("/tmp/f", "tee")]),
            ("! tee /tmp/f", &[("/tmp/f", "tee")]),
            ("if true; then tee /tmp/f; fi", &[("/tmp/f", "tee")]),
            ("while tee /tmp/f; do :; done", &[("/tmp/f", "tee")]),
            ("env -i tee /tmp/f", &[("/tmp/f", "tee")]),
            ("env -u HOME tee /tmp/f", &[("/tmp/f", "tee")]),
            ("env -u HOME FOO=1 tee /tmp/f", &[("/tmp/f", "tee")]),
            ("env -- tee /tmp/f", &[("/tmp/f", "tee")]),
            ("/usr/bin/env tee /tmp/f", &[("/tmp/f", "tee")]),
            ("command -p tee /tmp/f", &[("/tmp/f", "tee")]),
            ("exec tee /tmp/f", &[("/tmp/f", "tee")]),
            ("exec -a x tee /tmp/f", &[("/tmp/f", "tee")]),
            ("sudo -u root tee /tmp/f", &[("/tmp/f", "tee")]),
            ("sudo -E tee /tmp/f", &[("/tmp/f", "tee")]),
            ("sudo -- tee /tmp/f", &[("/tmp/f", "tee")]),
            ("doas -u root tee /tmp/f", &[("/tmp/f", "tee")]),
            ("timeout -s KILL 5 tee /tmp/f", &[("/tmp/f", "tee")]),
            ("timeout -k 1 5 tee /tmp/f", &[("/tmp/f", "tee")]),
            (
                "timeout --preserve-status 5s tee /tmp/f",
                &[("/tmp/f", "tee")],
            ),
            ("nice -n 5 tee /tmp/f", &[("/tmp/f", "tee")]),
            ("nice -5 tee /tmp/f", &[("/tmp/f", "tee")]),
            ("ionice -c 3 tee /tmp/f", &[("/tmp/f", "tee")]),
            ("stdbuf -oL tee /tmp/f", &[("/tmp/f", "tee")]),
            ("time -p tee /tmp/f", &[("/tmp/f", "tee")]),
            ("/usr/bin/time -o /tmp/f true", &[("/tmp/f", "time")]),
            ("sudo -e /tmp/f", &[("/tmp/f", "sudo")]),
            ("for x in a; do tee /tmp/f; done", &[("/tmp/f", "tee")]),
            ("true && { tee /tmp/f; }", &[("/tmp/f", "tee")]),
            ("f() { tee /tmp/f; }", &[("/tmp/f", "tee")]),
            ("coproc tee /tmp/f", &[("/tmp/f", "tee")]),
            ("env sudo timeout 5 tee /tmp/f", &[("/tmp/f", "tee")]),
            (
                "sudo -X u tee /tmp/f",
                &[("sudo", command_prefix::UNRESOLVED_OPERATION)],
            ),
            (
                "sudo -s tee /tmp/f",
                &[("sudo", command_prefix::UNRESOLVED_OPERATION)],
            ),
            (
                "env -S \"tee /tmp/f\"",
                &[("env", command_prefix::UNRESOLVED_OPERATION)],
            ),
        ];
        for (command, expected) in cases {
            assert_eq!(
                write_target_pairs(command),
                expected_targets(expected),
                "command: {command}"
            );
        }
    }

    #[test]
    fn command_prefix_confinement_rejection_table() {
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path().join("workspace");
        std::fs::create_dir_all(&root).unwrap();

        for command in [
            "env tee /tmp/f",
            "sudo cp a.txt /tmp/f",
            "command cp a.txt /tmp/f",
            "nohup tee /tmp/f",
            "timeout 5 cp a.txt /tmp/f",
            "{ tee /tmp/f; }",
            "! tee /tmp/f",
            "if true; then tee /tmp/f; fi",
            "env -S \"tee /tmp/f\"",
            "sudo -s tee /tmp/f",
            "sudo -X u tee /tmp/f",
            "sudo -e /tmp/f",
            "doas -u root tee /tmp/f",
            "nice -5 tee /tmp/f",
            "time -p tee /tmp/f",
            "/usr/bin/time -o /tmp/f true",
        ] {
            assert!(
                confinement_rejection(command, &root).is_some(),
                "command must be rejected: {command}"
            );
        }

        for command in [
            "env FOO=1 tee out.txt",
            "printf x | env tee out.txt",
            "timeout 5 cp a.txt b.txt",
            "env tee /dev/null",
            "env FOO=1 cargo test",
            "timeout 600 cargo test",
            "timeout --foreground 600 cargo test",
            "timeout --unknown 600 cargo test",
            "nice cargo build",
            "nohup cargo build",
            "time cargo test",
            "{ cargo test; }",
            "command -v cargo",
            "command -v tee /tmp/f",
            "sudo -l tee /tmp/f",
        ] {
            assert!(
                confinement_rejection(command, &root).is_none(),
                "command must stay allowed: {command}"
            );
        }
    }

    #[test]
    fn command_prefix_has_recognized_mutation_table() {
        assert!(has_recognized_mutation("env tee out.txt"));
        assert!(has_recognized_mutation("timeout 5 tee out.txt"));
        assert!(!has_recognized_mutation("timeout 600 cargo test"));
        assert!(!has_recognized_mutation("env FOO=1 cargo test"));
        assert!(!has_recognized_mutation("command -v tee /tmp/f"));
        assert!(has_recognized_mutation("sudo -X u tee /tmp/f"));
    }

    #[test]
    fn command_prefix_protected_path_mutation_table() {
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path().join("workspace");
        std::fs::create_dir_all(&root).unwrap();
        let protected = vec!["build/spec.ts".to_string()];

        assert_eq!(
            protected_path_mutation("env FOO=1 tee build/spec.ts", &root, &protected),
            Some("build/spec.ts".to_string())
        );
        assert_eq!(
            protected_path_mutation("sudo -u root rm build/spec.ts", &root, &protected),
            Some("build/spec.ts".to_string())
        );
        assert!(protected_path_mutation("env FOO=1 cargo test", &root, &protected).is_none());
    }
}
