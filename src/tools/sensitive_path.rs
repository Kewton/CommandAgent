//! Shared credential-file predicate, direct-reference inspection, and typed
//! refusal for workspace secret paths.
//!
//! Issue #501 owns this leaf. #504 and later work must import these predicates
//! instead of re-declaring a private secret-filename list. Every
//! `WorkspacePolicy` blocks these paths; metadata exceptions, `--yes`, and
//! `--allow` do not lift the block, and there is deliberately no secret-read
//! opt-in in this change.

use std::fmt;
use std::path::{Component, Path, PathBuf};

use serde_json::Value;

/// The only basenames that stay readable/writable: strict dotenv templates.
const TEMPLATE_NAMES: [&str; 3] = [".env.example", ".env.sample", ".env.template"];

/// A denied credential path. The value carries names only: it never contains
/// the credential bytes, an expansion value, or a provider token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SensitivePathRefusal {
    pub path: String,
    pub rule: &'static str,
    pub template: Option<String>,
}

impl SensitivePathRefusal {
    pub fn new(path: impl Into<String>, rule: &'static str) -> Self {
        let path = path.into();
        let template = template_suggestion(&path, rule);
        Self {
            template,
            path,
            rule,
        }
    }

    pub fn glob(segment: impl Into<String>) -> Self {
        Self {
            path: segment.into(),
            rule: "glob_secret",
            template: None,
        }
    }
}

impl fmt::Display for SensitivePathRefusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "workspace_policy_blocked: `{}` is a protected credential path ({})",
            self.path, self.rule
        )?;
        if let Some(template) = &self.template {
            write!(formatter, "; edit the template `{template}` instead")?;
        }
        write!(
            formatter,
            "; CommandAgent does not read or write workspace credentials. Keep real secrets outside the agent workspace and edit templates; #502 tracks dynamic/indirect Bash exfiltration isolation"
        )
    }
}

impl std::error::Error for SensitivePathRefusal {}

pub fn refusal_from_error(error: &anyhow::Error) -> Option<&SensitivePathRefusal> {
    error.downcast_ref::<SensitivePathRefusal>()
}

fn template_suggestion(path: &str, rule: &str) -> Option<String> {
    if rule != "dotenv" {
        return None;
    }
    const TEMPLATE: &str = ".env.example";
    match Path::new(path).parent() {
        Some(parent) if !parent.as_os_str().is_empty() && parent != Path::new(".") => {
            Some(parent.join(TEMPLATE).to_string_lossy().replace('\\', "/"))
        }
        _ => Some(TEMPLATE.to_string()),
    }
}

/// True only for the three strict dotenv template basenames. `.env.example.local`
/// and `.env.production.example` are not templates and remain denied.
pub fn is_template_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    TEMPLATE_NAMES.contains(&lower.as_str())
}

fn sensitive_name_rule(name: &str) -> Option<&'static str> {
    let lower = name.to_ascii_lowercase();
    if lower == ".env" || lower.starts_with(".env.") {
        return Some("dotenv");
    }
    match lower.as_str() {
        ".envrc" => return Some("dotenvrc"),
        ".npmrc" => return Some("npm_config"),
        ".pypirc" => return Some("pypi_config"),
        ".netrc" => return Some("netrc"),
        ".git-credentials" => return Some("git_credentials"),
        ".ssh" => return Some("ssh_directory"),
        _ => {}
    }
    if lower.starts_with("service-account") && lower.ends_with(".json") {
        return Some("service_account_json");
    }
    if lower.ends_with(".private.key") || lower.ends_with(".private.pem") {
        return Some("private_key");
    }
    if lower.ends_with(".p12") || lower.ends_with(".pfx") {
        return Some("pkcs12");
    }
    None
}

fn basename_rule(name: &str) -> Option<&'static str> {
    if is_template_name(name) {
        return None;
    }
    sensitive_name_rule(name)
}

fn components_lower(path: &Path) -> Vec<String> {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy().to_ascii_lowercase()),
            _ => None,
        })
        .collect()
}

fn contains_sequence(haystack: &[String], needle: &[&str]) -> bool {
    !needle.is_empty()
        && haystack.len() >= needle.len()
        && haystack.windows(needle.len()).any(|window| {
            window
                .iter()
                .zip(needle)
                .all(|(part, expected)| part == expected)
        })
}

fn path_rule(components: &[String]) -> Option<&'static str> {
    let (last, parents) = components.split_last()?;
    if contains_sequence(components, &[".aws", "credentials"]) {
        return Some("aws_credentials");
    }
    if contains_sequence(components, &[".docker", "config.json"]) {
        return Some("docker_config");
    }
    if contains_sequence(
        components,
        &[".config", "gcloud", "application_default_credentials.json"],
    ) {
        return Some("gcloud_credentials");
    }
    for parent in parents {
        if let Some(rule) = sensitive_name_rule(parent) {
            return Some(rule);
        }
    }
    basename_rule(last)
}

/// Rule name for a workspace-relative path, or `None` when the path is clean.
pub fn relative_path_rule(relative: &Path) -> Option<&'static str> {
    path_rule(&components_lower(relative))
}

pub fn relative_path_is_sensitive(relative: &Path) -> bool {
    relative_path_rule(relative).is_some()
}

fn display_path(path: &Path) -> String {
    let display = path.to_string_lossy().replace('\\', "/");
    if display.is_empty() {
        ".".to_string()
    } else {
        display
    }
}

fn strip_root(root: &Path, path: &Path) -> Option<PathBuf> {
    if let Ok(relative) = path.strip_prefix(root) {
        return Some(relative.to_path_buf());
    }
    let canonical_root = root.canonicalize().ok()?;
    path.strip_prefix(&canonical_root)
        .ok()
        .map(Path::to_path_buf)
}

/// True when the lexical name or the canonical target is a credential path.
///
/// The lexical check catches new (not yet existing) names; the canonical check
/// catches a template-name symlink whose target is a secret.
pub fn refusal_for_path(root: &Path, path: &Path) -> Option<SensitivePathRefusal> {
    if let Some(relative) = strip_root(root, path)
        && let Some(rule) = relative_path_rule(&relative)
    {
        return Some(SensitivePathRefusal::new(display_path(&relative), rule));
    }
    if !std::fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        return None;
    }
    let canonical = path.canonicalize().ok()?;
    let relative = strip_root(root, &canonical)?;
    relative_path_rule(&relative)
        .map(|rule| SensitivePathRefusal::new(display_path(&relative), rule))
}

pub fn refusal_for_relative(display: &str, relative: &Path) -> Option<SensitivePathRefusal> {
    relative_path_rule(relative).map(|rule| SensitivePathRefusal::new(display, rule))
}

pub fn path_is_sensitive(root: &Path, path: &Path) -> bool {
    refusal_for_path(root, path).is_some()
}

pub fn ensure_not_sensitive_path(root: &Path, path: &Path) -> anyhow::Result<()> {
    match refusal_for_path(root, path) {
        Some(refusal) => Err(anyhow::Error::new(refusal)),
        None => Ok(()),
    }
}

/// Rejects an explicit glob segment that names a credential.
///
/// Broad globs such as `**/*` return `None`; the walkers exclude secret entries
/// separately. A template segment (`.env.example`) is allowed.
pub fn glob_references_secret(pattern: &str) -> Option<String> {
    for raw_segment in pattern.split(['/', '\\']) {
        let segment = raw_segment.trim();
        if segment.is_empty() || matches!(segment, "." | "..") {
            continue;
        }
        let stripped =
            segment.trim_matches(|ch: char| matches!(ch, '*' | '?' | '[' | ']' | '{' | '}' | '!'));
        if stripped.is_empty() {
            continue;
        }
        if basename_rule(stripped).is_some() {
            return Some(stripped.to_string());
        }
    }
    None
}

fn push_token(tokens: &mut Vec<String>, word: &mut String) {
    if !word.is_empty() {
        tokens.push(std::mem::take(word));
    }
}

/// Splits a shell command into literal words without executing expansions.
fn shell_tokens(command: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut word = String::new();
    let mut quote: Option<char> = None;
    let mut chars = command.chars().peekable();
    while let Some(ch) = chars.next() {
        match (quote, ch) {
            (Some('\''), '\'') | (Some('"'), '"') => quote = None,
            (Some('\''), _) => word.push(ch),
            (Some('"'), '\\') => {
                if let Some(next) = chars.next() {
                    word.push(next);
                }
            }
            (Some(_), _) => word.push(ch),
            (None, '\\') => {
                if let Some(next) = chars.next() {
                    word.push(next);
                }
            }
            (None, '\'' | '"') => quote = Some(ch),
            (None, '`') => push_token(&mut tokens, &mut word),
            (None, ch)
                if ch.is_whitespace() || matches!(ch, ';' | '|' | '&' | '<' | '>' | '(' | ')') =>
            {
                push_token(&mut tokens, &mut word);
            }
            (None, _) => word.push(ch),
        }
    }
    push_token(&mut tokens, &mut word);
    tokens
}

fn token_reference(token: &str) -> Option<String> {
    let mut candidates = vec![token];
    if let Some((_, value)) = token.split_once('=') {
        candidates.push(value);
    }
    for candidate in candidates {
        let candidate = candidate.trim().trim_end_matches('/');
        if candidate.is_empty() {
            continue;
        }
        if relative_path_rule(Path::new(candidate)).is_some() {
            return Some(candidate.to_string());
        }
    }
    None
}

/// First direct credential reference in one literal token, if any.
pub fn token_references_secret(token: &str) -> Option<String> {
    token_reference(token)
}

pub fn tokens_reference_secret<'a, I>(tokens: I) -> Option<String>
where
    I: IntoIterator<Item = &'a str>,
{
    tokens.into_iter().find_map(token_reference)
}

/// First direct credential reference in a Bash command string, if any.
pub fn command_references_secret(command: &str) -> Option<String> {
    shell_tokens(command)
        .iter()
        .find_map(|token| token_reference(token))
}

fn argument_refusal(root: &Path, raw: &str) -> Option<SensitivePathRefusal> {
    if let Some(refusal) = refusal_for_relative(&display_argument(root, raw), Path::new(raw)) {
        return Some(refusal);
    }
    Path::new(raw)
        .is_absolute()
        .then(|| refusal_for_path(root, Path::new(raw)))
        .flatten()
}

fn display_argument(root: &Path, raw: &str) -> String {
    match strip_root(root, Path::new(raw)) {
        Some(relative) => display_path(&relative),
        None => display_path(Path::new(raw)),
    }
}

/// Rejects a direct credential reference in a built-in tool argument before any
/// approval request or IO.
pub fn argument_reference_rejection(
    name: &str,
    arguments: &Value,
    root: &Path,
) -> Option<anyhow::Error> {
    let refusal = match name {
        "Read" | "Write" | "Edit" => {
            let raw = arguments.get("path").and_then(Value::as_str)?;
            argument_refusal(root, raw)?
        }
        "Glob" => {
            let pattern = arguments.get("pattern").and_then(Value::as_str)?;
            SensitivePathRefusal::glob(glob_references_secret(pattern)?)
        }
        "Grep" => {
            let glob = arguments.get("glob").and_then(Value::as_str)?;
            SensitivePathRefusal::glob(glob_references_secret(glob)?)
        }
        "Bash" => {
            let command = arguments.get("command").and_then(Value::as_str)?;
            SensitivePathRefusal::new(command_references_secret(command)?, "bash_reference")
        }
        _ => return None,
    };
    Some(anyhow::Error::new(refusal))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn denies_direct_credentials_and_denies_non_template_env_variants() {
        for name in [
            ".env",
            ".env.local",
            ".env.production",
            ".env.example.local",
            ".env.production.example",
            ".envrc",
            ".npmrc",
            ".pypirc",
            ".netrc",
            ".git-credentials",
            ".ssh",
            "service-account.json",
            "my.private.key",
            "my.private.pem",
            "bundle.p12",
            "bundle.pfx",
        ] {
            assert!(
                relative_path_is_sensitive(Path::new(name)),
                "{name} must be denied"
            );
        }
    }

    #[test]
    fn allows_only_the_three_strict_templates_and_ordinary_files() {
        for name in [
            ".env.example",
            ".env.sample",
            ".env.template",
            ".environment",
            "notes.key",
            "config.pem",
            "README.md",
        ] {
            assert!(
                !relative_path_is_sensitive(Path::new(name)),
                "{name} must be allowed"
            );
        }
    }

    #[test]
    fn detects_nested_and_composite_credential_paths() {
        for (path, rule) in [
            ("sub/.env", "dotenv"),
            ("config/.aws/credentials", "aws_credentials"),
            (".docker/config.json", "docker_config"),
            (
                ".config/gcloud/application_default_credentials.json",
                "gcloud_credentials",
            ),
            ("keys/bundle.p12", "pkcs12"),
        ] {
            assert_eq!(relative_path_rule(Path::new(path)), Some(rule), "{path}");
        }
    }

    #[test]
    fn glob_detection_rejects_explicit_secrets_and_allows_broad_patterns() {
        for pattern in [".env", "**/.env", ".env*", "*.private.key"] {
            assert!(glob_references_secret(pattern).is_some(), "{pattern}");
        }
        for pattern in ["**/*", "src/**/*.ts", "**/*.md", ".env.example", "**/*.key"] {
            assert!(glob_references_secret(pattern).is_none(), "{pattern}");
        }
    }

    #[test]
    fn command_scan_handles_quoting_flags_redirection_and_templates() {
        for command in [
            "cat .env",
            "head -n 1 .env",
            "cat '.env'",
            "cat ./.env",
            "cat --file=.env",
            "cat < .env",
            "grep -r KEY .env",
            "$(cat .env)",
            "cat `cat .env`",
            "cat sub/.env.local",
        ] {
            assert!(
                command_references_secret(command).is_some(),
                "{command} must be refused"
            );
        }
        for command in ["cat .env.example", "npm run build", "cat README.md"] {
            assert!(
                command_references_secret(command).is_none(),
                "{command} must be allowed"
            );
        }
    }

    #[test]
    fn template_symlink_to_secret_is_refused_by_canonical_target() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".env"), "CANARY\n").unwrap();
        std::os::unix::fs::symlink(dir.path().join(".env"), dir.path().join(".env.example"))
            .unwrap();
        let refusal = refusal_for_path(dir.path(), &dir.path().join(".env.example")).unwrap();
        assert_eq!(refusal.rule, "dotenv");
    }

    #[test]
    fn new_secret_name_is_refused_without_reading_anything() {
        let dir = tempfile::tempdir().unwrap();
        let refusal = refusal_for_path(dir.path(), &dir.path().join(".env")).unwrap();
        assert_eq!(refusal.rule, "dotenv");
        assert!(!refusal.to_string().contains("CANARY"));
    }
}
