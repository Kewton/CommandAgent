//! Deterministic shell command classification for setup/verifier evidence.
//!
//! This module does not execute commands and does not grant setup authority by
//! itself. It names already proposed shell commands so setup and recovery
//! contracts can decide whether the command is verifier-owned setup,
//! inspection, mutation, or blocked dependency/network work.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CommandClass {
    Verifier,
    SetupCheck,
    SetupExecution,
    Inspection,
    Mutation,
    NetworkOrDependency,
    Blocked,
    Unknown,
}

impl CommandClass {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Verifier => "verifier",
            Self::SetupCheck => "setup_check",
            Self::SetupExecution => "setup_execution",
            Self::Inspection => "inspection",
            Self::Mutation => "mutation",
            Self::NetworkOrDependency => "network_or_dependency",
            Self::Blocked => "blocked",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CommandAuthority {
    OriginalVerifier,
    VerifierOwnedSetup,
    ReadOnlyInspection,
    UserVisibleMutation,
    ExplicitSetupPolicyRequired,
    BlockedByPolicy,
    Unknown,
}

impl CommandAuthority {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::OriginalVerifier => "original_verifier",
            Self::VerifierOwnedSetup => "verifier_owned_setup",
            Self::ReadOnlyInspection => "read_only_inspection",
            Self::UserVisibleMutation => "user_visible_mutation",
            Self::ExplicitSetupPolicyRequired => "explicit_setup_policy_required",
            Self::BlockedByPolicy => "blocked_by_policy",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ShellCommandClassification {
    pub(crate) command_class: CommandClass,
    pub(crate) authority: CommandAuthority,
    pub(crate) reason: String,
}

impl ShellCommandClassification {
    pub(crate) fn eval_report_fields(&self) -> Vec<String> {
        vec![
            format!(
                "setup_command_classification={}",
                self.command_class.as_str()
            ),
            format!("command_authority={}", self.authority.as_str()),
            format!("command_classification_reason={}", compact(&self.reason)),
        ]
    }
}

pub(crate) fn classify_shell_command(command: &str) -> ShellCommandClassification {
    let normalized = normalize_command(command);
    let first = normalized.split_whitespace().next().unwrap_or("");
    let tokens = normalized.split_whitespace().collect::<Vec<_>>();

    if normalized.is_empty() {
        return classify(
            CommandClass::Unknown,
            CommandAuthority::Unknown,
            "empty command",
        );
    }

    if contains_shell_control(&normalized) {
        return classify(
            CommandClass::Blocked,
            CommandAuthority::BlockedByPolicy,
            "compound shell control requires an explicit verifier or setup contract",
        );
    }

    if is_dependency_install(&tokens) {
        return classify(
            CommandClass::NetworkOrDependency,
            CommandAuthority::ExplicitSetupPolicyRequired,
            "dependency setup requires a visible setup job and explicit policy",
        );
    }

    if is_setup_execution(&tokens) {
        return classify(
            CommandClass::SetupExecution,
            CommandAuthority::VerifierOwnedSetup,
            "local setup execution is verifier-owned setup when admitted",
        );
    }

    if is_setup_check(&tokens) {
        return classify(
            CommandClass::SetupCheck,
            CommandAuthority::ReadOnlyInspection,
            "setup check inspects toolchain or manifest state",
        );
    }

    if is_verifier(&tokens) {
        return classify(
            CommandClass::Verifier,
            CommandAuthority::OriginalVerifier,
            "command is an original verifier or test runner",
        );
    }

    if is_inspection(first) {
        return classify(
            CommandClass::Inspection,
            CommandAuthority::ReadOnlyInspection,
            "command is read-only workspace inspection",
        );
    }

    if is_mutation(first) {
        return classify(
            CommandClass::Mutation,
            CommandAuthority::UserVisibleMutation,
            "command mutates local workspace state and needs visible policy",
        );
    }

    classify(
        CommandClass::Unknown,
        CommandAuthority::Unknown,
        "command has no deterministic class",
    )
}

fn classify(
    command_class: CommandClass,
    authority: CommandAuthority,
    reason: impl Into<String>,
) -> ShellCommandClassification {
    ShellCommandClassification {
        command_class,
        authority,
        reason: reason.into(),
    }
}

fn normalize_command(command: &str) -> String {
    command.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn contains_shell_control(command: &str) -> bool {
    ["&&", "||", ";", "|", "$(", "`"].iter().any(|token| {
        command
            .split_whitespace()
            .any(|part| part == *token || part.contains(token))
    })
}

fn is_dependency_install(tokens: &[&str]) -> bool {
    matches!(
        tokens,
        ["npm", "install", ..]
            | ["npm", "i", ..]
            | ["npm", "ci", ..]
            | ["pnpm", "install", ..]
            | ["yarn", "install", ..]
            | ["pip", "install", ..]
            | ["pip3", "install", ..]
            | ["cargo", "install", ..]
            | ["cargo", "fetch", ..]
    )
}

fn is_setup_execution(tokens: &[&str]) -> bool {
    matches!(
        tokens,
        ["npm", "rebuild", ..]
            | ["pnpm", "rebuild", ..]
            | ["yarn", "rebuild", ..]
            | ["cargo", "generate-lockfile", ..]
    )
}

fn is_setup_check(tokens: &[&str]) -> bool {
    matches!(
        tokens,
        ["node", "--version", ..]
            | ["node", "-v", ..]
            | ["npm", "--version", ..]
            | ["npm", "-v", ..]
            | ["cargo", "--version", ..]
            | ["rustc", "--version", ..]
            | ["python", "--version", ..]
            | ["python3", "--version", ..]
            | ["pip", "--version", ..]
            | ["pip3", "--version", ..]
    )
}

fn is_verifier(tokens: &[&str]) -> bool {
    matches!(
        tokens,
        ["cargo", "test", ..]
            | ["cargo", "check", ..]
            | ["cargo", "clippy", ..]
            | ["cargo", "build", ..]
            | ["npm", "run", "build", ..]
            | ["npm", "run", "test", ..]
            | ["npm", "test", ..]
            | ["pnpm", "test", ..]
            | ["pnpm", "build", ..]
            | ["pytest", ..]
            | ["python", "-m", "pytest", ..]
            | ["python3", "-m", "pytest", ..]
    )
}

fn is_inspection(command: &str) -> bool {
    matches!(
        command,
        "ls" | "pwd" | "cat" | "sed" | "rg" | "grep" | "find" | "wc" | "head" | "tail"
    )
}

fn is_mutation(command: &str) -> bool {
    matches!(
        command,
        "mkdir" | "touch" | "cp" | "mv" | "chmod" | "git" | "apply_patch"
    )
}

fn compact(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join("_")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_verifier_commands() {
        let cases = ["cargo test", "npm run build", "python -m pytest tests"];
        for case in cases {
            let classified = classify_shell_command(case);
            assert_eq!(classified.command_class, CommandClass::Verifier);
            assert_eq!(classified.authority, CommandAuthority::OriginalVerifier);
        }
    }

    #[test]
    fn classifies_dependency_install_as_policy_required() {
        let classified = classify_shell_command("npm install");

        assert_eq!(classified.command_class, CommandClass::NetworkOrDependency);
        assert_eq!(
            classified.authority,
            CommandAuthority::ExplicitSetupPolicyRequired
        );
    }

    #[test]
    fn classifies_setup_checks_and_workspace_inspection() {
        assert_eq!(
            classify_shell_command("node --version").command_class,
            CommandClass::SetupCheck
        );
        assert_eq!(
            classify_shell_command("rg TODO src").authority,
            CommandAuthority::ReadOnlyInspection
        );
    }

    #[test]
    fn blocks_compound_shell_control() {
        let classified = classify_shell_command("npm install && npm run build");

        assert_eq!(classified.command_class, CommandClass::Blocked);
        assert_eq!(classified.authority, CommandAuthority::BlockedByPolicy);
    }

    #[test]
    fn exposes_eval_fields() {
        let fields = classify_shell_command("cargo test").eval_report_fields();

        assert!(fields.contains(&"setup_command_classification=verifier".to_string()));
        assert!(fields.contains(&"command_authority=original_verifier".to_string()));
    }
}
