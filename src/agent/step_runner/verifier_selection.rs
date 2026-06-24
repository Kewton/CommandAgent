#![allow(dead_code)]

use crate::agent::step_runner::verifier_diagnostic::command_is_weak_source_grep;
use crate::agent::step_runner::verify::VerificationFailure;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VerifierSelection {
    StructuredRunnable,
    StructuredWeak,
    StructuredMissing,
    LegacyRunnable,
    Missing,
    BlockedByPolicy,
    DependencySetupRequired,
    RuntimeError,
}

impl VerifierSelection {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::StructuredRunnable => "structured_runnable",
            Self::StructuredWeak => "structured_weak",
            Self::StructuredMissing => "structured_missing",
            Self::LegacyRunnable => "legacy_runnable",
            Self::Missing => "missing",
            Self::BlockedByPolicy => "blocked_by_policy",
            Self::DependencySetupRequired => "dependency_setup_required",
            Self::RuntimeError => "runtime_error",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VerifierBinding {
    pub(crate) command: String,
    pub(crate) selection: VerifierSelection,
    pub(crate) runner_kind: Option<String>,
    pub(crate) owned_test_artifact: Option<String>,
    pub(crate) implementation_artifact: Option<String>,
    pub(crate) setup_manifest: Option<String>,
    pub(crate) failure_signature: Option<String>,
    pub(crate) candidate_repair_target: Option<String>,
}

impl VerifierBinding {
    pub(crate) fn from_failure(failure: &VerificationFailure) -> Self {
        let selection = classify_verifier_failure(failure);
        Self {
            command: failure.command.clone(),
            selection,
            runner_kind: runner_kind(&failure.command).map(str::to_string),
            owned_test_artifact: test_artifact_from_command(&failure.command),
            implementation_artifact: failure
                .source_excerpt
                .as_ref()
                .map(|excerpt| excerpt.path.clone()),
            setup_manifest: setup_manifest_for_failure(failure),
            failure_signature: Some(format!(
                "{}|{}",
                selection.as_str(),
                compact_reason(&failure.reason)
            )),
            candidate_repair_target: failure
                .source_excerpt
                .as_ref()
                .map(|excerpt| excerpt.path.clone())
                .or_else(|| test_artifact_from_command(&failure.command))
                .or_else(|| setup_manifest_for_failure(failure)),
        }
    }
}

pub(crate) fn classify_verifier_failure(failure: &VerificationFailure) -> VerifierSelection {
    if failure.reason.starts_with("blocked:") {
        return VerifierSelection::BlockedByPolicy;
    }
    if failure.reason == "dependency_missing"
        || failure.diagnostic_excerpt.contains("dependency_missing")
    {
        return VerifierSelection::DependencySetupRequired;
    }
    if failure.command.trim().is_empty() {
        return VerifierSelection::Missing;
    }
    if command_is_weak_source_grep(&failure.command)
        || command_is_self_referential_generated_test(&failure.command)
    {
        return VerifierSelection::StructuredWeak;
    }
    if failure.command.contains("pytest")
        || failure.command.contains("cargo test")
        || failure.command.contains("npm run build")
        || failure.command.contains("npm test")
    {
        return VerifierSelection::StructuredRunnable;
    }
    if failure.reason.starts_with("command_failed:") {
        VerifierSelection::LegacyRunnable
    } else {
        VerifierSelection::RuntimeError
    }
}

fn command_is_self_referential_generated_test(command: &str) -> bool {
    let lower = command.trim().to_ascii_lowercase();
    (lower.starts_with("test -f ") || lower.starts_with("grep -q "))
        && (lower.contains("tests/") || lower.contains("_test.rs") || lower.contains("test_"))
}

fn runner_kind(command: &str) -> Option<&'static str> {
    let command = command.trim();
    if command.starts_with("cargo ") {
        Some("cargo")
    } else if command.starts_with("python ") || command.starts_with("python3 ") {
        Some("python")
    } else if command.starts_with("pytest") {
        Some("pytest")
    } else if command.starts_with("npm ") {
        Some("npm")
    } else {
        None
    }
}

fn test_artifact_from_command(command: &str) -> Option<String> {
    command
        .split_whitespace()
        .find(|part| part.starts_with("tests/") || part.contains("/tests/"))
        .map(|part| {
            part.trim_matches(|ch: char| matches!(ch, '\'' | '"' | ',' | ';'))
                .to_string()
        })
}

fn setup_manifest_for_failure(failure: &VerificationFailure) -> Option<String> {
    if failure.reason == "dependency_missing" || failure.diagnostic_excerpt.contains("package.json")
    {
        Some("package.json".to_string())
    } else if failure.command.contains("cargo ") {
        Some("Cargo.toml".to_string())
    } else {
        None
    }
}

fn compact_reason(reason: &str) -> String {
    reason
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("_")
        .chars()
        .take(80)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn failure(command: &str, reason: &str, diagnostic: &str) -> VerificationFailure {
        VerificationFailure {
            command: command.to_string(),
            reason: reason.to_string(),
            stdout_excerpt: String::new(),
            stderr_excerpt: String::new(),
            diagnostic_excerpt: diagnostic.to_string(),
            source_excerpt: None,
        }
    }

    #[test]
    fn blocked_command_is_policy_failure() {
        let failure = failure(
            "npm run build && npm test",
            "blocked:Unknown: compound",
            "compound",
        );

        let binding = VerifierBinding::from_failure(&failure);

        assert_eq!(binding.selection, VerifierSelection::BlockedByPolicy);
        assert_eq!(binding.runner_kind.as_deref(), Some("npm"));
    }

    #[test]
    fn dependency_missing_points_to_setup_manifest() {
        let failure = failure(
            "npm run build",
            "dependency_missing",
            "package.json dependencies",
        );

        let binding = VerifierBinding::from_failure(&failure);

        assert_eq!(
            binding.selection,
            VerifierSelection::DependencySetupRequired
        );
        assert_eq!(binding.setup_manifest.as_deref(), Some("package.json"));
        assert_eq!(
            binding.candidate_repair_target.as_deref(),
            Some("package.json")
        );
    }

    #[test]
    fn pytest_command_binds_test_artifact() {
        let failure = failure("pytest tests/test_app.py", "command_failed:1", "failed");

        let binding = VerifierBinding::from_failure(&failure);

        assert_eq!(binding.selection, VerifierSelection::StructuredRunnable);
        assert_eq!(
            binding.owned_test_artifact.as_deref(),
            Some("tests/test_app.py")
        );
    }

    #[test]
    fn source_grep_is_structured_weak() {
        let failure = failure(
            "grep -q CommandAgent src/main.rs",
            "command_failed:1",
            "missing literal",
        );

        let binding = VerifierBinding::from_failure(&failure);

        assert_eq!(binding.selection, VerifierSelection::StructuredWeak);
    }
}
