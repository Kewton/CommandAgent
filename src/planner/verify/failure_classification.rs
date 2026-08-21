use std::path::Path;

use crate::eval_events;
use crate::tools::bash::{BashOutcome, BashOutcomeKind};

use super::{
    NormalizedVerifyCommand, VerifyCommandRunResult, handle_verify_command_timeout,
    outcome_exit_code,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn early(
    command: &NormalizedVerifyCommand,
    root: &Path,
    profile: Option<&str>,
    offline: bool,
    eval_events_path: Option<&Path>,
    formatted: &str,
    outcome: &BashOutcome,
) -> Option<VerifyCommandRunResult> {
    if outcome.kind == BashOutcomeKind::Timeout {
        return Some(handle_verify_command_timeout(
            command,
            root,
            profile,
            offline,
            eval_events_path,
            formatted,
            outcome.elapsed_ms,
        ));
    }
    let kind = environment_failure_kind(outcome)?;
    Some(VerifyCommandRunResult::FalseNegative {
        command: command.as_str().to_string(),
        reason: format!(
            "verify_command_false_negative: deterministic_environment_error:{kind}: the verify command cannot execute in the current environment; command=`{command}`; tool_error={}",
            eval_events::body_snippet(formatted)
        ),
    })
}

fn environment_failure_kind(outcome: &BashOutcome) -> Option<&'static str> {
    match outcome_exit_code(outcome) {
        Some(127) => return Some("exit_127"),
        Some(126) => return Some("command_not_executable"),
        _ => {}
    }
    let stderr = outcome.stderr.to_ascii_lowercase();
    if stderr.contains("permission denied")
        || stderr.contains("operation not permitted")
        || stderr.contains("access is denied")
    {
        return Some("permission_denied");
    }
    if stderr.contains("bad interpreter")
        || stderr.contains("interpreter not found")
        || (stderr.contains("/usr/bin/env:")
            && (stderr.contains("no such file or directory") || stderr.contains("not found")))
    {
        return Some("interpreter_unavailable");
    }
    None
}

#[cfg(test)]
mod tests {
    use crate::tools::bash::BashOutcomeKind;

    use super::*;

    #[test]
    fn detects_environment_failures_without_claiming_syntax_errors() {
        let outcome = |code, stderr: &str| BashOutcome {
            kind: BashOutcomeKind::CommandFailed,
            status: Some(format!("exit status: {code}")),
            stdout: String::new(),
            stderr: stderr.to_string(),
            elapsed_ms: 1,
            summary: String::new(),
        };
        assert_eq!(
            environment_failure_kind(&outcome(127, "")),
            Some("exit_127")
        );
        assert_eq!(
            environment_failure_kind(&outcome(126, "")),
            Some("command_not_executable")
        );
        assert_eq!(
            environment_failure_kind(&outcome(1, "Permission denied")),
            Some("permission_denied")
        );
        assert_eq!(
            environment_failure_kind(&outcome(1, "bad interpreter")),
            Some("interpreter_unavailable")
        );
        assert_eq!(
            environment_failure_kind(&outcome(1, "SyntaxError: invalid syntax")),
            None
        );
    }
}
