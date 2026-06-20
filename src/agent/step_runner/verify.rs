use crate::safety::path_guard::{PathGuard, PathGuardError};
use crate::tools::ToolError;
use crate::tools::bash::BashTool;
use crate::tools::test_output::CommandOutput;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

const EXCERPT_LIMIT: usize = 4_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationReport {
    pub ok: bool,
    pub failures: Vec<VerificationFailure>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationFailure {
    pub command: String,
    pub reason: String,
    pub stdout_excerpt: String,
    pub stderr_excerpt: String,
    pub diagnostic_excerpt: String,
    pub source_excerpt: Option<SourceExcerpt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceExcerpt {
    pub path: String,
    pub line: usize,
    pub excerpt: String,
}

pub fn run_verifiers(
    cwd: impl AsRef<Path>,
    commands: &[String],
) -> Result<VerificationReport, VerifyError> {
    let guard = PathGuard::new(cwd.as_ref()).map_err(VerifyError::Path)?;
    let bash = BashTool::new(&guard);
    let mut failures = Vec::new();

    for command in commands {
        if let Some(failure) = dependency_missing_failure(&guard, command)? {
            failures.push(failure);
            continue;
        }

        match bash.run(command) {
            Ok(output) if output.success() => {}
            Ok(output) => failures.push(summarize_command_failure(&guard, command, &output)),
            Err(ToolError::BashBlocked { class, message }) => {
                failures.push(VerificationFailure {
                    command: command.clone(),
                    reason: format!("blocked:{class:?}: {message}"),
                    stdout_excerpt: String::new(),
                    stderr_excerpt: String::new(),
                    diagnostic_excerpt: message,
                    source_excerpt: None,
                });
            }
            Err(err) => return Err(VerifyError::Tool(err.to_string())),
        }
    }

    Ok(VerificationReport {
        ok: failures.is_empty(),
        failures,
    })
}

pub fn summarize_command_failure(
    guard: &PathGuard,
    command: &str,
    output: &CommandOutput,
) -> VerificationFailure {
    let combined = format!("{}\n{}", output.stdout, output.stderr);
    if let Some(failure) = dependency_missing_from_command_failure(guard, command, &combined) {
        return failure;
    }
    VerificationFailure {
        command: command.to_string(),
        reason: format!("command_failed:{}", output.status),
        stdout_excerpt: excerpt(&output.stdout),
        stderr_excerpt: excerpt(&output.stderr),
        diagnostic_excerpt: diagnostic_excerpt(&combined),
        source_excerpt: source_excerpt(guard, &combined),
    }
}

fn dependency_missing_failure(
    guard: &PathGuard,
    command: &str,
) -> Result<Option<VerificationFailure>, VerifyError> {
    let lower = command.trim().to_ascii_lowercase();
    if lower != "npm run build" {
        return Ok(None);
    }

    let package_path = match guard.resolve("package.json") {
        Ok(path) if path.exists() => path,
        _ => return Ok(None),
    };
    let package = fs::read_to_string(&package_path).map_err(|err| VerifyError::Io {
        path: package_path.clone(),
        message: err.to_string(),
    })?;
    if !package_uses_next_build(&package) {
        return Ok(None);
    }
    let next_bin = guard
        .resolve("node_modules/.bin/next")
        .map_err(VerifyError::Path)?;
    if next_bin.exists() {
        if let Some(missing_packages) = missing_package_dependencies(guard, &package)? {
            let message = format!(
                "dependency_missing: verifier_unavailable: npm run build requires package.json dependencies to be installed, but node_modules is missing: {}. Run dependency setup when allowed, then rerun npm run build; do not change scripts.build away from next build to fake success.",
                missing_packages.join(", ")
            );
            return Ok(Some(VerificationFailure {
                command: command.to_string(),
                reason: "dependency_missing".to_string(),
                stdout_excerpt: String::new(),
                stderr_excerpt: String::new(),
                diagnostic_excerpt: message,
                source_excerpt: None,
            }));
        }
        return Ok(None);
    }

    let message = "dependency_missing: verifier_unavailable: npm run build requires node_modules/.bin/next, but it is missing. Install dependencies with npm install/npm ci when allowed, or stop as dependency_missing; do not change scripts.build away from next build to fake success.".to_string();
    Ok(Some(VerificationFailure {
        command: command.to_string(),
        reason: "dependency_missing".to_string(),
        stdout_excerpt: String::new(),
        stderr_excerpt: String::new(),
        diagnostic_excerpt: message,
        source_excerpt: None,
    }))
}

fn dependency_missing_from_command_failure(
    guard: &PathGuard,
    command: &str,
    text: &str,
) -> Option<VerificationFailure> {
    if command.trim().to_ascii_lowercase() != "npm run build" {
        return None;
    }
    let package_path = guard.resolve("package.json").ok()?;
    let package = fs::read_to_string(&package_path).ok()?;
    if !package_uses_next_build(&package) {
        return None;
    }
    let missing_module = missing_module_from_diagnostic(text)?;
    let dependencies = package_dependency_keys(&package);
    if !dependencies.iter().any(|dep| dep == &missing_module) {
        return None;
    }
    let module_path = node_module_dependency_path(&missing_module)?;
    let resolved = guard.resolve(&module_path).ok()?;
    if resolved.exists() {
        return None;
    }
    let message = format!(
        "dependency_missing: verifier_unavailable: npm run build requires declared package `{missing_module}`, but `{module_path}` is missing. Run dependency setup when allowed, then rerun npm run build; do not change scripts.build away from next build to fake success."
    );
    Some(VerificationFailure {
        command: command.to_string(),
        reason: "dependency_missing".to_string(),
        stdout_excerpt: String::new(),
        stderr_excerpt: String::new(),
        diagnostic_excerpt: message,
        source_excerpt: None,
    })
}

fn missing_package_dependencies(
    guard: &PathGuard,
    package: &str,
) -> Result<Option<Vec<String>>, VerifyError> {
    let dependencies = package_dependency_keys(package);
    if dependencies.is_empty() {
        return Ok(None);
    }
    let mut missing = Vec::new();
    for dep in dependencies {
        let Some(path) = node_module_dependency_path(&dep) else {
            continue;
        };
        let resolved = guard.resolve(&path).map_err(VerifyError::Path)?;
        if !resolved.exists() {
            missing.push(dep);
        }
    }
    if missing.is_empty() {
        Ok(None)
    } else {
        missing.truncate(8);
        Ok(Some(missing))
    }
}

fn package_dependency_keys(package: &str) -> Vec<String> {
    let Ok(value) = serde_json::from_str::<Value>(package) else {
        return Vec::new();
    };
    let mut keys = Vec::new();
    for section in ["dependencies", "devDependencies"] {
        let Some(object) = value.get(section).and_then(Value::as_object) else {
            continue;
        };
        for name in object.keys() {
            if !keys.iter().any(|existing| existing == name) {
                keys.push(name.clone());
            }
        }
    }
    keys
}

fn missing_module_from_diagnostic(text: &str) -> Option<String> {
    for quote in ['\'', '"'] {
        let needle = format!("Cannot find module {quote}");
        if let Some(start) = text.find(&needle) {
            let rest = &text[start + needle.len()..];
            let value = rest.split(quote).next()?.trim();
            if node_module_dependency_path(value).is_some() {
                return Some(value.to_string());
            }
        }
    }
    None
}

fn node_module_dependency_path(name: &str) -> Option<String> {
    if name.starts_with('@') {
        let mut parts = name.split('/');
        let scope = parts.next()?;
        let package = parts.next()?;
        if parts.next().is_some() || package.is_empty() {
            return None;
        }
        if safe_package_segment(scope) && safe_package_segment(package) {
            return Some(format!("node_modules/{scope}/{package}"));
        }
        return None;
    }
    if safe_package_segment(name) && !name.contains('/') {
        Some(format!("node_modules/{name}"))
    } else {
        None
    }
}

fn safe_package_segment(value: &str) -> bool {
    !value.is_empty()
        && !value.contains("..")
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '@' | '-' | '_' | '.'))
}

fn package_uses_next_build(package: &str) -> bool {
    serde_json::from_str::<Value>(package)
        .ok()
        .and_then(|value| {
            value
                .get("scripts")
                .and_then(|scripts| scripts.get("build"))
                .and_then(Value::as_str)
                .map(|script| script.contains("next build"))
        })
        .unwrap_or_else(|| package.contains("next build"))
}

fn diagnostic_excerpt(text: &str) -> String {
    let mut lines = Vec::new();
    for line in text.lines() {
        let lower = line.to_ascii_lowercase();
        if lower.contains("error")
            || lower.contains("failed")
            || lower.contains("cannot find")
            || lower.contains("not assignable")
            || lower.contains("type '")
        {
            lines.push(line.to_string());
        }
    }

    if lines.is_empty() {
        excerpt(text)
    } else {
        excerpt(&lines.join("\n"))
    }
}

fn source_excerpt(guard: &PathGuard, text: &str) -> Option<SourceExcerpt> {
    let (path, line) = first_source_reference(text)?;
    let resolved = guard.resolve(&path).ok()?;
    let display_path = resolved
        .strip_prefix(guard.root())
        .ok()
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .unwrap_or(path);
    if ignored_source_excerpt_path(&display_path) {
        return None;
    }
    let body = fs::read_to_string(&resolved).ok()?;
    let excerpt = line_window(&body, line, 2);
    Some(SourceExcerpt {
        path: display_path,
        line,
        excerpt,
    })
}

fn ignored_source_excerpt_path(path: &str) -> bool {
    path.starts_with("node_modules/")
        || path.contains("/node_modules/")
        || path.starts_with(".next/")
        || path.contains("/.next/")
}

fn first_source_reference(text: &str) -> Option<(String, usize)> {
    for raw in text.lines() {
        let line = raw.trim();
        for marker in [".tsx:", ".ts:", ".jsx:", ".js:", ".rs:", ".py:"] {
            if let Some(idx) = line.find(marker) {
                let end = idx + marker.len() - 1;
                let start = line[..end]
                    .rfind(|ch: char| ch.is_whitespace() || ch == '(')
                    .map(|idx| idx + 1)
                    .unwrap_or(0);
                let mut path = line[start..end].trim().trim_start_matches("./").to_string();
                path = path.trim_start_matches("-->").trim().to_string();
                let rest = &line[end + 1..];
                let line_no = rest
                    .split(':')
                    .next()
                    .and_then(|value| value.parse::<usize>().ok())?;
                return Some((path, line_no));
            }
        }
    }
    None
}

fn line_window(body: &str, target: usize, radius: usize) -> String {
    let start = target.saturating_sub(radius).max(1);
    let end = target + radius;
    body.lines()
        .enumerate()
        .filter_map(|(idx, line)| {
            let line_no = idx + 1;
            if (start..=end).contains(&line_no) {
                let marker = if line_no == target { ">" } else { " " };
                Some(format!("{marker}{line_no}: {line}"))
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn excerpt(text: &str) -> String {
    if text.len() <= EXCERPT_LIMIT {
        text.trim().to_string()
    } else {
        format!("{}…", text[..EXCERPT_LIMIT].trim())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyError {
    Path(PathGuardError),
    Io { path: PathBuf, message: String },
    Tool(String),
}

impl std::fmt::Display for VerifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Path(err) => write!(f, "{}", err),
            Self::Io { path, message } => write!(f, "{}: {}", path.display(), message),
            Self::Tool(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for VerifyError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_dependency_missing_for_next_build_without_node_modules() {
        let root = temp_workspace("dependency-missing");
        fs::write(
            root.join("package.json"),
            r#"{"scripts":{"build":"next build"},"dependencies":{"next":"14.2.0"}}"#,
        )
        .unwrap();

        let report = run_verifiers(&root, &["npm run build".to_string()]).unwrap();

        assert!(!report.ok);
        assert_eq!(report.failures[0].reason, "dependency_missing");
        assert!(
            report.failures[0]
                .diagnostic_excerpt
                .contains("node_modules/.bin/next")
        );
    }

    #[test]
    fn reports_dependency_missing_for_manifest_dependency_not_installed() {
        let root = temp_workspace("dependency-missing-stale-manifest");
        fs::create_dir_all(root.join("node_modules/.bin")).unwrap();
        fs::create_dir_all(root.join("node_modules/next")).unwrap();
        fs::create_dir_all(root.join("node_modules/react")).unwrap();
        fs::create_dir_all(root.join("node_modules/react-dom")).unwrap();
        fs::write(root.join("node_modules/.bin/next"), "").unwrap();
        fs::write(
            root.join("package.json"),
            r#"{"scripts":{"build":"next build"},"dependencies":{"next":"14.2.0","react":"18.2.0","react-dom":"18.2.0","@tailwindcss/postcss":"latest"}}"#,
        )
        .unwrap();

        let report = run_verifiers(&root, &["npm run build".to_string()]).unwrap();

        assert!(!report.ok);
        assert_eq!(report.failures[0].reason, "dependency_missing");
        assert!(
            report.failures[0]
                .diagnostic_excerpt
                .contains("@tailwindcss/postcss")
        );
        assert!(
            report.failures[0]
                .diagnostic_excerpt
                .contains("package.json dependencies")
        );
    }

    #[test]
    fn classifies_declared_missing_module_diagnostic_as_dependency_missing() {
        let root = temp_workspace("dependency-missing-diagnostic");
        fs::create_dir_all(root.join("node_modules/.bin")).unwrap();
        fs::create_dir_all(root.join("node_modules/next")).unwrap();
        fs::create_dir_all(root.join("node_modules/react")).unwrap();
        fs::create_dir_all(root.join("node_modules/react-dom")).unwrap();
        fs::write(
            root.join("package.json"),
            r#"{"scripts":{"build":"next build"},"dependencies":{"next":"14.2.0","react":"18.2.0","react-dom":"18.2.0","@tailwindcss/postcss":"latest"}}"#,
        )
        .unwrap();
        let guard = PathGuard::new(&root).unwrap();
        let output = CommandOutput {
            status: 1,
            stdout: String::new(),
            stderr: "Error: Cannot find module '@tailwindcss/postcss'\nRequire stack:\n- postcss.config.js\n".to_string(),
        };

        let failure = summarize_command_failure(&guard, "npm run build", &output);

        assert_eq!(failure.reason, "dependency_missing");
        assert!(failure.diagnostic_excerpt.contains("@tailwindcss/postcss"));
        assert!(
            failure
                .diagnostic_excerpt
                .contains("node_modules/@tailwindcss/postcss")
        );
    }

    #[test]
    fn does_not_classify_undeclared_missing_module_as_dependency_missing() {
        let root = temp_workspace("undeclared-missing-module");
        fs::write(
            root.join("package.json"),
            r#"{"scripts":{"build":"next build"},"dependencies":{"next":"14.2.0"}}"#,
        )
        .unwrap();
        let guard = PathGuard::new(&root).unwrap();
        let output = CommandOutput {
            status: 1,
            stdout: String::new(),
            stderr: "Error: Cannot find module '@tailwindcss/postcss'".to_string(),
        };

        let failure = summarize_command_failure(&guard, "npm run build", &output);

        assert_eq!(failure.reason, "command_failed:1");
    }

    #[test]
    fn does_not_classify_relative_missing_module_as_dependency_missing() {
        let root = temp_workspace("relative-missing-module");
        fs::write(
            root.join("package.json"),
            r#"{"scripts":{"build":"next build"},"dependencies":{"next":"14.2.0"}}"#,
        )
        .unwrap();
        let guard = PathGuard::new(&root).unwrap();
        let output = CommandOutput {
            status: 1,
            stdout: String::new(),
            stderr: "Error: Cannot find module './GameBoard'".to_string(),
        };

        let failure = summarize_command_failure(&guard, "npm run build", &output);

        assert_eq!(failure.reason, "command_failed:1");
    }

    #[test]
    fn captures_typescript_source_excerpt() {
        let root = temp_workspace("ts-excerpt");
        fs::create_dir_all(root.join("app")).unwrap();
        fs::write(
            root.join("app/page.tsx"),
            "line1\nline2\nconst value: string = 1;\nline4\nline5\n",
        )
        .unwrap();
        let guard = PathGuard::new(&root).unwrap();
        let output = CommandOutput {
            status: 1,
            stdout: String::new(),
            stderr: "Failed to compile.\n./app/page.tsx:3:7\nType error: Type 'number' is not assignable to type 'string'.\n".to_string(),
        };

        let failure = summarize_command_failure(&guard, "npm run build", &output);

        assert_eq!(failure.reason, "command_failed:1");
        assert!(
            failure
                .diagnostic_excerpt
                .contains("not assignable to type")
        );
        let excerpt = failure.source_excerpt.unwrap();
        assert_eq!(excerpt.path, "app/page.tsx");
        assert_eq!(excerpt.line, 3);
        assert!(excerpt.excerpt.contains(">3: const value"));
    }

    #[test]
    fn skips_dependency_source_excerpt() {
        let root = temp_workspace("dependency-excerpt");
        fs::create_dir_all(root.join("node_modules/tailwindcss/dist")).unwrap();
        fs::write(
            root.join("node_modules/tailwindcss/dist/lib.js"),
            "line1\nline2\nthrow new Error('plugin')\nline4\n",
        )
        .unwrap();
        let guard = PathGuard::new(&root).unwrap();
        let output = CommandOutput {
            status: 1,
            stdout: String::new(),
            stderr: format!(
                "Error evaluating Node.js code\n{}:3:1\nError: plugin failure\n",
                root.join("node_modules/tailwindcss/dist/lib.js").display()
            ),
        };

        let failure = summarize_command_failure(&guard, "npm run build", &output);

        assert!(failure.source_excerpt.is_none());
        assert!(failure.diagnostic_excerpt.contains("plugin failure"));
    }

    #[test]
    fn blocked_commands_are_reported_as_verification_failures() {
        let root = temp_workspace("blocked");

        let report = run_verifiers(&root, &["mkdir -p src".to_string()]).unwrap();

        assert!(!report.ok);
        assert!(report.failures[0].reason.starts_with("blocked:"));
        assert!(
            report.failures[0]
                .diagnostic_excerpt
                .contains("use Write directly")
        );
    }

    fn temp_workspace(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "commandagent-verify-{}-{}",
            name,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }
}
