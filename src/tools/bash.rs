use crate::safety::path_guard::PathGuard;
use crate::tools::test_output::CommandOutput;
use crate::tools::{ToolError, ToolResult};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandClass {
    ReadOnly,
    ScriptRun,
    BuildTest,
    DirectoryCreation,
    Network,
    Mutating,
    Dangerous,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyDecision {
    pub class: CommandClass,
    pub allowed: bool,
    pub message: Option<String>,
    pub reclassified_cd_wrapper: bool,
}

pub struct BashTool<'a> {
    guard: &'a PathGuard,
}

impl<'a> BashTool<'a> {
    pub fn new(guard: &'a PathGuard) -> Self {
        Self { guard }
    }

    pub fn run(&self, command: &str) -> ToolResult<CommandOutput> {
        let decision = enforce_offline_policy(command, self.guard.root());
        if !decision.allowed {
            return Err(ToolError::BashBlocked {
                class: decision.class,
                message: decision
                    .message
                    .unwrap_or_else(|| "command blocked".to_string()),
            });
        }

        let output = Command::new("sh")
            .arg("-lc")
            .arg(command)
            .current_dir(self.guard.root())
            .output()
            .map_err(|err| ToolError::Io {
                path: self.guard.root().to_path_buf(),
                message: err.to_string(),
            })?;

        Ok(CommandOutput {
            status: output.status.code().unwrap_or(1),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }
}

pub fn enforce_offline_policy(command: &str, cwd: &Path) -> PolicyDecision {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return blocked(CommandClass::Unknown, "empty bash command");
    }

    if has_extra_chaining(trimmed) {
        return blocked(
            CommandClass::Unknown,
            "compound shell commands are blocked unless they are exactly `cd <workspace> && <local check>`",
        );
    }

    if let Some((dir, tail)) = parse_cd_wrapper(trimmed) {
        let dir = strip_shell_quotes(dir);
        let path = if Path::new(&dir).is_absolute() {
            PathBuf::from(&dir)
        } else {
            cwd.join(&dir)
        };
        let Ok(canonical) = path.canonicalize() else {
            return blocked(CommandClass::Unknown, "cd wrapper directory does not exist");
        };
        let Ok(canonical_cwd) = cwd.canonicalize() else {
            return blocked(CommandClass::Unknown, "workspace directory does not exist");
        };
        if !canonical.starts_with(canonical_cwd) {
            return blocked(CommandClass::Dangerous, "cd wrapper escapes the workspace");
        }

        let tail_decision = classify_simple(tail);
        if matches!(
            tail_decision.class,
            CommandClass::ReadOnly | CommandClass::ScriptRun | CommandClass::BuildTest
        ) && tail_decision.allowed
        {
            return PolicyDecision {
                reclassified_cd_wrapper: true,
                ..tail_decision
            };
        }
        return blocked(
            tail_decision.class,
            "cd wrapper is only allowed for read-only, local script-run, or build-test commands",
        );
    }

    classify_simple(trimmed)
}

fn classify_simple(command: &str) -> PolicyDecision {
    let command = command.trim();
    let lower = command.to_ascii_lowercase();

    if starts_with_any(&lower, &["sudo ", "rm ", "rm -", "chmod ", "chown ", "dd "]) {
        return blocked(CommandClass::Dangerous, "dangerous shell command blocked");
    }
    if starts_with_any(
        &lower,
        &[
            "curl ",
            "wget ",
            "git clone ",
            "npm install",
            "npm ci",
            "pnpm install",
        ],
    ) {
        return blocked(
            CommandClass::Network,
            "network or dependency install command blocked",
        );
    }
    if starts_with_any(&lower, &["mkdir ", "mkdir\t"]) || lower == "mkdir" {
        return blocked(
            CommandClass::DirectoryCreation,
            "directory creation is unnecessary: use Write directly; parent directories are created automatically",
        );
    }
    if starts_with_any(
        &lower,
        &[
            "touch ",
            "mv ",
            "cp ",
            "python -m pip",
            "pip install",
            "cargo add",
            "cargo update",
        ],
    ) {
        return blocked(CommandClass::Mutating, "mutating shell command blocked");
    }
    if starts_with_any(
        &lower,
        &[
            "npm run build",
            "npm test",
            "cargo check",
            "cargo test",
            "cargo build",
        ],
    ) {
        return allowed(CommandClass::BuildTest);
    }
    if starts_with_any(&lower, &["cargo run"]) {
        return allowed(CommandClass::ScriptRun);
    }
    if starts_with_any(
        &lower,
        &["python ", "python3 ", "node ", "deno run ", "ruby "],
    ) {
        return allowed(CommandClass::ScriptRun);
    }
    if starts_with_any(
        &lower,
        &[
            "pwd", "ls", "cat ", "sed -n ", "head ", "tail ", "wc ", "find ", "test -f ",
            "test -d ", "grep -q ",
        ],
    ) {
        return allowed(CommandClass::ReadOnly);
    }

    blocked(
        CommandClass::Unknown,
        "offline policy could not classify command",
    )
}

fn parse_cd_wrapper(command: &str) -> Option<(&str, &str)> {
    let mut parts = command.split("&&");
    let first = parts.next()?.trim();
    let second = parts.next()?.trim();
    if parts.next().is_some() {
        return None;
    }
    let dir = first.strip_prefix("cd ")?.trim();
    Some((dir, second))
}

fn has_extra_chaining(command: &str) -> bool {
    command.contains(';') || command.contains("||") || command.matches("&&").count() > 1
}

fn starts_with_any(value: &str, prefixes: &[&str]) -> bool {
    prefixes
        .iter()
        .any(|prefix| value == prefix.trim() || value.starts_with(prefix))
}

fn strip_shell_quotes(value: &str) -> String {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .or_else(|| {
            value
                .strip_prefix('\'')
                .and_then(|value| value.strip_suffix('\''))
        })
        .unwrap_or(value)
        .to_string()
}

fn allowed(class: CommandClass) -> PolicyDecision {
    PolicyDecision {
        class,
        allowed: true,
        message: None,
        reclassified_cd_wrapper: false,
    }
}

fn blocked(class: CommandClass, message: &str) -> PolicyDecision {
    PolicyDecision {
        class,
        allowed: false,
        message: Some(message.to_string()),
        reclassified_cd_wrapper: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::safety::path_guard::PathGuard;
    use std::fs;

    #[test]
    fn blocks_mkdir_with_write_guidance() {
        let root = temp_workspace("mkdir");
        let decision = enforce_offline_policy("mkdir -p src", &root);

        assert_eq!(decision.class, CommandClass::DirectoryCreation);
        assert!(!decision.allowed);
        assert!(decision.message.unwrap().contains("use Write directly"));
    }

    #[test]
    fn allows_cd_wrapper_for_local_script_run() {
        let root = temp_workspace("script");
        let decision = enforce_offline_policy("cd . && python3 script.py", &root);

        assert_eq!(decision.class, CommandClass::ScriptRun);
        assert!(decision.allowed);
        assert!(decision.reclassified_cd_wrapper);
    }

    #[test]
    fn blocks_cd_wrapper_with_dangerous_tail() {
        let root = temp_workspace("dangerous");
        let decision = enforce_offline_policy("cd . && rm -rf target", &root);

        assert_eq!(decision.class, CommandClass::Dangerous);
        assert!(!decision.allowed);
    }

    #[test]
    fn allows_cargo_check_as_build_test() {
        let root = temp_workspace("cargo-check");
        let decision = enforce_offline_policy("cargo check", &root);

        assert_eq!(decision.class, CommandClass::BuildTest);
        assert!(decision.allowed);
    }

    #[test]
    fn allows_cargo_run_as_local_script_run() {
        let root = temp_workspace("cargo-run");
        let decision = enforce_offline_policy("cargo run", &root);

        assert_eq!(decision.class, CommandClass::ScriptRun);
        assert!(decision.allowed);
    }

    #[test]
    fn blocks_three_part_chaining() {
        let root = temp_workspace("chain");
        let decision = enforce_offline_policy("cd . && python3 script.py && rm file", &root);

        assert!(!decision.allowed);
    }

    #[test]
    fn allows_read_only_shell_checks() {
        let root = temp_workspace("read-only-checks");

        for command in [
            "test -f Cargo.toml",
            "test -d src",
            "grep -q hello src/main.rs",
        ] {
            let decision = enforce_offline_policy(command, &root);
            assert_eq!(decision.class, CommandClass::ReadOnly, "{command}");
            assert!(decision.allowed, "{command}");
        }
    }

    #[test]
    fn runs_allowed_read_only_command() {
        let root = temp_workspace("run");
        let guard = PathGuard::new(&root).unwrap();

        let output = BashTool::new(&guard).run("pwd").unwrap();

        assert!(output.success());
        assert!(
            output
                .stdout
                .contains(root.file_name().unwrap().to_str().unwrap())
        );
    }

    fn temp_workspace(name: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("commandagent-bash-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }
}
