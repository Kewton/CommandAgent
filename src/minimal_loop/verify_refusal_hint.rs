//! Names the acceptable next step in a Verify/Report runtime Bash refusal.
//!
//! The accept/reject decision, repeat key and limit are untouched: this only
//! appends guidance to the feedback the model already receives.

use std::path::Path;

use crate::planner::verify::{
    VerifyCommandViolationKind, diagnose_verify_command,
    normalize_runtime_bash_command_for_boundary,
};
use crate::tools::bash_write_guard::verifier_mutation;

use super::tool_feedback;

const REDIRECT_REASON_MARKER: &str = "shell redirects";

pub(crate) fn recoverable_tool_feedback(
    name: &str,
    err: &anyhow::Error,
    command: &str,
    root: &Path,
) -> String {
    let feedback = tool_feedback::recoverable_tool_feedback(name, err, None);
    match hint(&err.to_string(), command, root) {
        // Leading: the history keeps only the first 500 characters of a tool
        // result, and the generic feedback alone nearly fills that.
        Some(hint) => format!("{hint} {feedback}"),
        None => feedback,
    }
}

/// The base is only named when the runtime Bash rules admit it as it stands.
fn base_is_accepted(base: &str, root: &Path) -> bool {
    diagnose_verify_command(base).violation.is_none()
        && normalize_runtime_bash_command_for_boundary(base, root).is_ok()
        && crate::tools::bash::blocked_reason(base, false).is_none()
        && crate::tools::bash::path_confinement_rejection(base, root).is_none()
        && verifier_mutation::rejection(base, true).is_none()
}

fn hint(err_text: &str, command: &str, root: &Path) -> Option<String> {
    if !err_text.contains("verify_command_policy_error") {
        return None;
    }
    let diagnosis = diagnose_verify_command(command);
    match diagnosis.violation? {
        VerifyCommandViolationKind::SuccessFailureEchoStripped
        | VerifyCommandViolationKind::FallbackTrueStripped
        | VerifyCommandViolationKind::ExitCodeEchoStripped => {
            let base = diagnosis.normalized;
            // Normalization collapses runs of whitespace, even inside quotes; a
            // base the model did not type verbatim could mean something else.
            (command.contains(base.as_str()) && base_is_accepted(&base, root)).then(|| {
                    format!(
                        "Run the base command `{base}` as-is, once (no echo or `|| true` branch; the verifier records pass/fail itself)."
                    )
                })
        }
        VerifyCommandViolationKind::ShellControlSyntax
            if diagnosis
                .reason
                .as_deref()
                .is_some_and(|reason| reason.contains(REDIRECT_REASON_MARKER)) =>
        {
            Some("Create fixture files with the Write tool, not a shell redirect.".to_string())
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests;
