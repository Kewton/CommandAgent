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

/// Classification of a workspace path for credential blocking and containment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathVerdict {
    /// Inside the workspace root and not a credential.
    Inside,
    /// A credential by lexical name or by canonical target.
    Credential(SensitivePathRefusal),
    /// The canonical target resolves outside the workspace root.
    Outside,
}

/// Reason a walker should exclude a path without leaking its name or bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkippedPath {
    Credential,
    Outside,
}

fn canonical_target(root: &Path, path: &Path) -> Option<(PathBuf, PathBuf)> {
    let canonical_root = root.canonicalize().ok()?;
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    if absolute.exists() {
        return absolute
            .canonicalize()
            .ok()
            .map(|canonical| (canonical, canonical_root));
    }
    let mut tail = Vec::new();
    let mut cursor = absolute.as_path();
    loop {
        match std::fs::symlink_metadata(cursor) {
            Ok(_) => {
                let mut resolved = cursor.canonicalize().ok()?;
                for name in tail.iter().rev() {
                    resolved.push(name);
                }
                return Some((resolved, canonical_root));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let name = cursor.file_name()?.to_os_string();
                tail.push(name);
                cursor = cursor.parent()?;
            }
            Err(_) => return None,
        }
    }
}

/// Classifies a path by lexical name and by canonical target.
///
/// The canonical check does not depend on the final component being a symlink:
/// a symlinked parent directory (for example `keys` -> `.ssh`) still resolves a
/// credential child. A not-yet-existing target is judged by its canonical
/// existing parent plus the created name. `Outside` is distinct from `Inside`
/// so callers never read `None` as proof of containment.
pub fn classify_workspace_path(root: &Path, path: &Path) -> PathVerdict {
    if let Some(relative) = strip_root(root, path)
        && let Some(rule) = relative_path_rule(&relative)
    {
        return PathVerdict::Credential(SensitivePathRefusal::new(display_path(&relative), rule));
    }
    let Some((canonical, canonical_root)) = canonical_target(root, path) else {
        return PathVerdict::Inside;
    };
    match canonical.strip_prefix(&canonical_root) {
        Ok(relative) => match relative_path_rule(relative) {
            Some(rule) => {
                PathVerdict::Credential(SensitivePathRefusal::new(display_path(relative), rule))
            }
            None => PathVerdict::Inside,
        },
        Err(_) => PathVerdict::Outside,
    }
}

/// Credential refusal for a path, ignoring containment. `None` means "not a
/// credential"; it is not a statement that the path is inside the root.
pub fn credential_refusal(root: &Path, path: &Path) -> Option<SensitivePathRefusal> {
    match classify_workspace_path(root, path) {
        PathVerdict::Credential(refusal) => Some(refusal),
        PathVerdict::Inside | PathVerdict::Outside => None,
    }
}

/// Walker exclusion reason: credential or an out-of-root symlink target.
pub fn sensitive_skip(root: &Path, path: &Path) -> Option<SkippedPath> {
    match classify_workspace_path(root, path) {
        PathVerdict::Credential(_) => Some(SkippedPath::Credential),
        PathVerdict::Outside => Some(SkippedPath::Outside),
        PathVerdict::Inside => None,
    }
}

pub fn confinement_error(subject: impl std::fmt::Display) -> anyhow::Error {
    anyhow::anyhow!(
        "path escapes workspace: `{subject}` resolves outside the current workspace root through a symlink; use workspace-relative paths"
    )
}

/// Non-leaking notice that a broad enumeration excluded protected entries, so
/// an empty or shorter result is not mistaken for "nothing exists".
pub fn exclusion_notice(excluded: usize) -> String {
    format!("[commandagent: {excluded} workspace path(s) excluded by workspace policy]")
}

pub fn refusal_for_relative(display: &str, relative: &Path) -> Option<SensitivePathRefusal> {
    relative_path_rule(relative).map(|rule| SensitivePathRefusal::new(display, rule))
}

pub fn path_is_sensitive(root: &Path, path: &Path) -> bool {
    credential_refusal(root, path).is_some()
}

pub fn ensure_not_sensitive_path(root: &Path, path: &Path) -> anyhow::Result<()> {
    match credential_refusal(root, path) {
        Some(refusal) => Err(anyhow::Error::new(refusal)),
        None => Ok(()),
    }
}

/// Rejects an explicit glob that names a credential or resolves outside the
/// root. Broad globs return `None`; their walkers exclude those entries and
/// report the exclusion separately.
pub fn explicit_glob_rejection(root: &Path, pattern: &str) -> Option<anyhow::Error> {
    if let Some(reference) = glob_references_secret(pattern) {
        return Some(anyhow::Error::new(SensitivePathRefusal::glob(reference)));
    }
    if !is_literal_glob(pattern) {
        return None;
    }
    let candidate = if Path::new(pattern).is_absolute() {
        PathBuf::from(pattern)
    } else {
        root.join(pattern)
    };
    match classify_workspace_path(root, &candidate) {
        PathVerdict::Credential(refusal) => Some(anyhow::Error::new(refusal)),
        PathVerdict::Outside => Some(confinement_error(pattern)),
        PathVerdict::Inside => None,
    }
}

fn is_literal_glob(pattern: &str) -> bool {
    !pattern
        .chars()
        .any(|ch| matches!(ch, '*' | '?' | '[' | ']' | '{' | '}' | '!'))
}

/// Case-insensitive `find` name/path predicates that select every path the
/// credential predicate denies. Kept beside the predicate so the shared rule
/// and the bounded broad-walk prune cannot drift.
///
/// The three strict template *files* stay selectable, but a template-named
/// *directory* is pruned like any other `.env.*` directory: the shared rule
/// treats the template name as a protected parent, so its children must not be
/// read. This is the file/directory parity R-01 requires.
pub fn find_exclusion_expression() -> String {
    let template_file_exclusions = TEMPLATE_NAMES
        .iter()
        .map(|name| format!(" ! -iname '{name}'"))
        .collect::<String>();
    let template_directory_patterns = TEMPLATE_NAMES
        .iter()
        .map(|name| format!("-iname '{name}'"))
        .collect::<Vec<_>>()
        .join(" -o ");
    [
        "-iname '.env'".to_string(),
        format!("\\( -iname '.env.*'{template_file_exclusions} \\)"),
        "-iname '.envrc'".to_string(),
        "-iname '.npmrc'".to_string(),
        "-iname '.pypirc'".to_string(),
        "-iname '.netrc'".to_string(),
        "-iname '.git-credentials'".to_string(),
        "-iname '.ssh'".to_string(),
        "-iname 'service-account*.json'".to_string(),
        "-iname '*.private.key'".to_string(),
        "-iname '*.private.pem'".to_string(),
        "-iname '*.p12'".to_string(),
        "-iname '*.pfx'".to_string(),
        "-ipath '*/.aws/credentials'".to_string(),
        "-ipath '*/.docker/config.json'".to_string(),
        "-ipath '*/.config/gcloud/application_default_credentials.json'".to_string(),
        format!("\\( {template_directory_patterns} \\) -type d"),
    ]
    .join(" -o ")
}

/// Rejects an explicit glob segment or whole compound path that names a
/// credential.
pub fn glob_references_secret(pattern: &str) -> Option<String> {
    if relative_path_rule(Path::new(pattern)).is_some() {
        return Some(pattern.to_string());
    }
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
    // Read the comment- and heredoc-elided text when the lexical guard can elide
    // it; fall back to the raw command when it cannot (Issue #576).
    let elided = super::shell_lexical::strip_comments_and_heredocs(command);
    let text = elided.as_deref().unwrap_or(command);
    shell_tokens(text)
        .iter()
        .find_map(|token| token_reference(token))
}

fn argument_refusal(root: &Path, raw: &str) -> Option<SensitivePathRefusal> {
    credential_refusal(root, Path::new(raw))
}

/// Rejects a direct credential reference in a built-in tool argument before any
/// approval request or IO.
pub fn argument_reference_rejection(
    name: &str,
    arguments: &Value,
    root: &Path,
) -> Option<anyhow::Error> {
    match name {
        "Read" | "Write" | "Edit" => {
            let raw = arguments.get("path").and_then(Value::as_str)?;
            Some(anyhow::Error::new(argument_refusal(root, raw)?))
        }
        "Glob" => {
            let pattern = arguments.get("pattern").and_then(Value::as_str)?;
            explicit_glob_rejection(root, pattern)
        }
        "Grep" => {
            let glob = arguments.get("glob").and_then(Value::as_str)?;
            explicit_glob_rejection(root, glob)
        }
        "Bash" => {
            let command = arguments.get("command").and_then(Value::as_str)?;
            let reference = command_references_secret(command)?;
            Some(anyhow::Error::new(SensitivePathRefusal::new(
                reference,
                "bash_reference",
            )))
        }
        _ => None,
    }
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
        let refusal = credential_refusal(dir.path(), &dir.path().join(".env.example")).unwrap();
        assert_eq!(refusal.rule, "dotenv");
    }

    #[test]
    fn new_secret_name_is_refused_without_reading_anything() {
        let dir = tempfile::tempdir().unwrap();
        let refusal = credential_refusal(dir.path(), &dir.path().join(".env")).unwrap();
        assert_eq!(refusal.rule, "dotenv");
        assert!(!refusal.to_string().contains("CANARY"));
    }

    #[test]
    fn parent_symlink_and_outside_target_are_classified() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("workspace");
        std::fs::create_dir_all(root.join(".ssh")).unwrap();
        std::fs::write(root.join(".ssh/id_ed25519"), "CANARY\n").unwrap();
        std::os::unix::fs::symlink(root.join(".ssh"), root.join("keys")).unwrap();
        assert!(matches!(
            classify_workspace_path(&root, &root.join("keys/id_ed25519")),
            PathVerdict::Credential(_)
        ));
        assert!(matches!(
            classify_workspace_path(&root, &root.join("keys/new_key")),
            PathVerdict::Credential(_)
        ));

        let outside = dir.path().join("outside.env");
        std::fs::write(&outside, "CANARY\n").unwrap();
        std::os::unix::fs::symlink(&outside, root.join("outside.txt")).unwrap();
        assert_eq!(
            classify_workspace_path(&root, &root.join("outside.txt")),
            PathVerdict::Outside
        );

        std::fs::write(root.join("safe.txt"), "ok\n").unwrap();
        std::os::unix::fs::symlink(root.join("safe.txt"), root.join("link.txt")).unwrap();
        assert_eq!(
            classify_workspace_path(&root, &root.join("link.txt")),
            PathVerdict::Inside
        );
    }
}
