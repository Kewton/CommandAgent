use crate::config::Action;

const HEADLESS_PROMPT_WARNING: &str = "warning: --prompt is non-interactive because stdin is not a TTY; mutating tools require an explicit --allow policy or --yes. Use --yes only in a trusted workspace.";

pub(crate) fn startup_warning(
    action: &Action,
    auto_approve: bool,
    stdin_is_terminal: bool,
) -> Option<&'static str> {
    (matches!(action, Action::Prompt(_))
        && !auto_approve
        && !super::allow_policy::current_has_mutating_authority()
        && !stdin_is_terminal)
        .then_some(HEADLESS_PROMPT_WARNING)
}

pub(crate) fn require_tool_approval(
    name: &str,
    auto_approve: bool,
    interactive_approval: bool,
) -> anyhow::Result<()> {
    if auto_approve || interactive_approval {
        return Ok(());
    }
    anyhow::bail!("approval required for {name}; rerun with --yes only in a trusted workspace")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headless_prompt_without_yes_warns_with_an_executable_choice() {
        let warning = startup_warning(&Action::Prompt("inspect".to_string()), false, false)
            .expect("headless prompt warning");

        assert!(warning.contains("--yes"), "{warning}");
        assert!(warning.contains("trusted workspace"), "{warning}");
        assert!(!warning.contains("interactive approval"), "{warning}");
    }

    #[test]
    fn startup_warning_does_not_change_other_cli_presentations() {
        assert!(startup_warning(&Action::Prompt("inspect".to_string()), true, false).is_none());
        assert!(startup_warning(&Action::Prompt("inspect".to_string()), false, true).is_none());
        assert!(startup_warning(&Action::Runs, false, false).is_none());
    }

    #[test]
    fn unavailable_approval_lists_only_the_executable_headless_rerun() {
        let error = require_tool_approval("Bash", false, false)
            .unwrap_err()
            .to_string();

        assert_eq!(
            error,
            "approval required for Bash; rerun with --yes only in a trusted workspace"
        );
        assert!(!error.contains("interactive approval"), "{error}");
        assert!(require_tool_approval("Bash", true, false).is_ok());
        assert!(require_tool_approval("Bash", false, true).is_ok());
    }
}
