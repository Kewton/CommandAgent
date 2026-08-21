use std::path::Path;

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

fn write_targets(command: &str) -> Vec<WriteTarget> {
    let Some(tokens) = shell_tokens(command) else {
        return Vec::new();
    };
    let mut targets = redirect_targets(&tokens);
    for segment in tokens.split(|token| *token == ShellToken::SegmentEnd) {
        let words = command_words(segment);
        let Some((program, arguments)) = words.split_first() else {
            continue;
        };
        let program = Path::new(program)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(program);
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
                        targets.extend(operands[..operands.len() - 1].iter().map(|path| {
                            WriteTarget {
                                path: (*path).to_string(),
                                operation: "symlink target".to_string(),
                            }
                        }));
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
            _ => {}
        }
    }
    targets
}

fn redirect_targets(tokens: &[ShellToken]) -> Vec<WriteTarget> {
    tokens
        .windows(2)
        .filter_map(|window| match window {
            [ShellToken::OutputRedirect, ShellToken::Word(path)] => Some(WriteTarget {
                path: path.clone(),
                operation: "output redirection".to_string(),
            }),
            _ => None,
        })
        .collect()
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
}
