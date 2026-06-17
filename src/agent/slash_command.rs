use crate::safety::path_guard::{PathGuard, PathGuardError};
use std::fs;
use std::path::{Path, PathBuf};

const MAX_FILE_REF_BYTES: usize = 32 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlashCommand {
    pub kind: SlashCommandKind,
    pub profile: Option<String>,
    pub style: Option<String>,
    pub intent: Option<String>,
    pub artifacts: Vec<String>,
    pub argument: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlashCommandKind {
    PlanSteps,
    PlanRun,
    RunPlan,
    UltraPlan,
    UltraPlanRun,
    RunUltraPlan,
}

pub fn parse_slash_command(
    input: &str,
    cwd: impl AsRef<Path>,
) -> Result<Option<SlashCommand>, SlashCommandError> {
    let input = input.trim();
    if !input.starts_with('/') {
        return Ok(None);
    }

    let guard = PathGuard::new(cwd.as_ref()).map_err(SlashCommandError::Path)?;
    let expanded = expand_cat_references(input, &guard)?;
    let (command, rest) = split_command(&expanded);
    let kind = parse_kind(command)?;
    let tokens = tokenize_with_spans(rest)?;
    let mut profile = None;
    let mut style = None;
    let mut intent = None;
    let mut artifacts = Vec::new();
    let mut idx = 0usize;

    while idx < tokens.len() {
        let token = &tokens[idx].text;
        if token == "--profile" {
            idx += 1;
            let Some(value) = tokens.get(idx) else {
                return Err(SlashCommandError::MissingOptionValue(
                    "--profile".to_string(),
                ));
            };
            profile = Some(value.text.clone());
            idx += 1;
        } else if let Some(value) = token.strip_prefix("--profile=") {
            profile = Some(value.to_string());
            idx += 1;
        } else if token == "--style" {
            idx += 1;
            let Some(value) = tokens.get(idx) else {
                return Err(SlashCommandError::MissingOptionValue("--style".to_string()));
            };
            style = Some(value.text.clone());
            idx += 1;
        } else if let Some(value) = token.strip_prefix("--style=") {
            style = Some(value.to_string());
            idx += 1;
        } else if token == "--intent" {
            idx += 1;
            let Some(value) = tokens.get(idx) else {
                return Err(SlashCommandError::MissingOptionValue(
                    "--intent".to_string(),
                ));
            };
            intent = Some(value.text.clone());
            idx += 1;
        } else if let Some(value) = token.strip_prefix("--intent=") {
            intent = Some(value.to_string());
            idx += 1;
        } else if token == "--artifact" {
            idx += 1;
            let Some(value) = tokens.get(idx) else {
                return Err(SlashCommandError::MissingOptionValue(
                    "--artifact".to_string(),
                ));
            };
            artifacts.push(value.text.clone());
            idx += 1;
        } else if let Some(value) = token.strip_prefix("--artifact=") {
            artifacts.push(value.to_string());
            idx += 1;
        } else {
            break;
        }
    }

    let argument = tokens[idx..]
        .iter()
        .map(|token| token.text.as_str())
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string();

    Ok(Some(SlashCommand {
        kind,
        profile,
        style,
        intent,
        artifacts,
        argument,
    }))
}

fn parse_kind(command: &str) -> Result<SlashCommandKind, SlashCommandError> {
    match command {
        "/plan-steps" => Ok(SlashCommandKind::PlanSteps),
        "/plan-run" => Ok(SlashCommandKind::PlanRun),
        "/run-plan" => Ok(SlashCommandKind::RunPlan),
        "/ultra-plan" => Ok(SlashCommandKind::UltraPlan),
        "/ultra-plan-run" => Ok(SlashCommandKind::UltraPlanRun),
        "/run-ultra-plan" => Ok(SlashCommandKind::RunUltraPlan),
        other => Err(SlashCommandError::UnknownCommand(other.to_string())),
    }
}

fn split_command(input: &str) -> (&str, &str) {
    input
        .split_once(char::is_whitespace)
        .map(|(command, rest)| (command, rest.trim_start()))
        .unwrap_or((input, ""))
}

fn expand_cat_references(input: &str, guard: &PathGuard) -> Result<String, SlashCommandError> {
    let mut output = String::new();
    let mut rest = input;

    while let Some(start) = rest.find("$(cat ") {
        output.push_str(&rest[..start]);
        let after_start = &rest[start + "$(cat ".len()..];
        let Some(end) = after_start.find(')') else {
            return Err(SlashCommandError::UnclosedFileReference);
        };
        let raw_path = after_start[..end].trim();
        let raw_path = raw_path
            .strip_prefix('"')
            .and_then(|path| path.strip_suffix('"'))
            .or_else(|| {
                raw_path
                    .strip_prefix('\'')
                    .and_then(|path| path.strip_suffix('\''))
            })
            .unwrap_or(raw_path);
        let path = guard.resolve(raw_path).map_err(SlashCommandError::Path)?;
        let contents = read_bounded(&path)?;
        output.push_str(&contents);
        rest = &after_start[end + 1..];
    }

    output.push_str(rest);
    Ok(output)
}

fn read_bounded(path: &Path) -> Result<String, SlashCommandError> {
    let metadata = fs::metadata(path).map_err(|err| SlashCommandError::Io {
        path: path.to_path_buf(),
        message: err.to_string(),
    })?;
    if metadata.len() as usize > MAX_FILE_REF_BYTES {
        return Err(SlashCommandError::FileReferenceTooLarge {
            path: path.to_path_buf(),
            bytes: metadata.len(),
            max: MAX_FILE_REF_BYTES,
        });
    }
    fs::read_to_string(path).map_err(|err| SlashCommandError::Io {
        path: path.to_path_buf(),
        message: err.to_string(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Token {
    text: String,
}

fn tokenize_with_spans(input: &str) -> Result<Vec<Token>, SlashCommandError> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut chars = input.chars().peekable();
    let mut quote = None;

    while let Some(ch) = chars.next() {
        match quote {
            Some(active) if ch == active => quote = None,
            Some(_) => current.push(ch),
            None if ch == '"' || ch == '\'' => quote = Some(ch),
            None if ch.is_whitespace() => {
                if !current.is_empty() {
                    tokens.push(Token {
                        text: std::mem::take(&mut current),
                    });
                }
            }
            None if ch == '\\' => {
                if let Some(next) = chars.next() {
                    current.push(next);
                }
            }
            None => current.push(ch),
        }
    }

    if quote.is_some() {
        return Err(SlashCommandError::UnclosedQuote);
    }
    if !current.is_empty() {
        tokens.push(Token { text: current });
    }
    Ok(tokens)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlashCommandError {
    UnknownCommand(String),
    MissingOptionValue(String),
    UnclosedQuote,
    UnclosedFileReference,
    Path(PathGuardError),
    Io {
        path: PathBuf,
        message: String,
    },
    FileReferenceTooLarge {
        path: PathBuf,
        bytes: u64,
        max: usize,
    },
}

impl std::fmt::Display for SlashCommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownCommand(command) => write!(f, "unknown slash command: {command}"),
            Self::MissingOptionValue(option) => write!(f, "missing value for {option}"),
            Self::UnclosedQuote => write!(f, "unclosed quote in slash command"),
            Self::UnclosedFileReference => write!(f, "unclosed $(cat ...) file reference"),
            Self::Path(err) => write!(f, "{}", err),
            Self::Io { path, message } => write!(f, "{}: {}", path.display(), message),
            Self::FileReferenceTooLarge { path, bytes, max } => write!(
                f,
                "file reference is too large: {} has {} bytes; max is {}",
                path.display(),
                bytes,
                max
            ),
        }
    }
}

impl std::error::Error for SlashCommandError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_none_for_plain_prompt() {
        let root = temp_workspace("plain");

        let parsed = parse_slash_command("create README", &root).unwrap();

        assert_eq!(parsed, None);
    }

    #[test]
    fn parses_ultra_plan_run_options_and_goal() {
        let root = temp_workspace("ultra");

        let parsed = parse_slash_command(
            r#"/ultra-plan-run --profile nextjs --style tdd --intent modify --artifact app/page.tsx "build a game""#,
            &root,
        )
        .unwrap()
        .unwrap();

        assert_eq!(parsed.kind, SlashCommandKind::UltraPlanRun);
        assert_eq!(parsed.profile.as_deref(), Some("nextjs"));
        assert_eq!(parsed.style.as_deref(), Some("tdd"));
        assert_eq!(parsed.intent.as_deref(), Some("modify"));
        assert_eq!(parsed.artifacts, vec!["app/page.tsx"]);
        assert_eq!(parsed.argument, "build a game");
    }

    #[test]
    fn expands_commandagent_cat_file_reference() {
        let root = temp_workspace("cat-ref");
        fs::create_dir_all(root.join(".commandagent/repairs")).unwrap();
        fs::write(root.join(".commandagent/repairs/repair.md"), "fix build").unwrap();

        let parsed = parse_slash_command(
            r#"/ultra-plan-run --profile nextjs "$(cat .commandagent/repairs/repair.md)""#,
            &root,
        )
        .unwrap()
        .unwrap();

        assert_eq!(parsed.argument, "fix build");
    }

    #[test]
    fn rejects_file_reference_path_escape() {
        let root = temp_workspace("cat-escape");

        let err =
            parse_slash_command(r#"/ultra-plan-run "$(cat ../repair.md)""#, &root).unwrap_err();

        assert!(matches!(err, SlashCommandError::Path(_)));
    }

    #[test]
    fn rejects_unknown_slash_command() {
        let root = temp_workspace("unknown");

        let err = parse_slash_command("/unknown do thing", &root).unwrap_err();

        assert_eq!(
            err,
            SlashCommandError::UnknownCommand("/unknown".to_string())
        );
    }

    fn temp_workspace(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "commandagent-slash-{}-{}",
            name,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }
}
