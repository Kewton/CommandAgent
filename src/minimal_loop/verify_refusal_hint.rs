//! Names the acceptable next step in a Verify/Report runtime Bash refusal.
//!
//! The accept/reject decision, repeat key and limit are untouched: this only
//! appends guidance to the feedback the model already receives.

use std::path::Path;

use serde_json::json;

use crate::eval_events;
use crate::planner::verify::{
    VerifyCommandViolationKind, diagnose_verify_command,
    normalize_runtime_bash_command_for_boundary,
};
use crate::state::tool_history::history_content;
use crate::tools::bash_write_guard::verifier_mutation;

use super::tool_feedback;

const REDIRECT_REASON_MARKER: &str = "shell redirects";
const OPAQUE_REASON_MARKER: &str = "body cannot be inspected";

/// The repeat whose feedback is the last one the model receives: the loop
/// stops on the next refusal (`RECOVERABLE_TOOL_ERROR_REPEAT_LIMIT`).
const LAST_FEEDBACK_REPEAT: usize = 2;

const REPEAT_LEAD: &str = "Repeated rejection (2/2).";
const STOPS: &str = "a third refusal stops this step.";

pub(crate) fn recoverable_tool_feedback(
    name: &str,
    err: &anyhow::Error,
    command: &str,
    root: &Path,
) -> String {
    let feedback = tool_feedback::recoverable_tool_feedback(name, err, None);
    match hint(&shape(&err.to_string(), command, root)) {
        // Leading: the history keeps only the first 500 characters of a tool
        // result, and the generic feedback alone nearly fills that.
        Some(hint) => format!("{hint} {feedback}"),
        None => feedback,
    }
}

/// [`recoverable_tool_feedback`], led by a concrete next call on the last
/// refusal before the loop stops (Issue #895). The text queued for the model is
/// recorded as it will be sent; that is not proof the model read it.
pub(crate) fn recoverable_tool_feedback_for_repeat(
    name: &str,
    err: &anyhow::Error,
    command: &str,
    root: &Path,
    repeats: usize,
    events_path: Option<&Path>,
) -> String {
    let feedback = recoverable_tool_feedback(name, err, command, root);
    let err_text = err.to_string();
    if !err_text.contains("verify_command_policy_error") {
        return feedback;
    }
    let shape = shape(&err_text, command, root);
    let (guidance_kind, text) = if repeats == LAST_FEEDBACK_REPEAT
        && let Some((kind, lead)) = repeat_lead(name, &shape, &feedback)
    {
        (kind, format!("{lead} {feedback}"))
    } else {
        (first_kind(&shape), feedback)
    };
    eval_events::emit(
        events_path,
        json!({
            "event": "verify_refusal_feedback_queued",
            "name": name,
            "repeat_count": repeats,
            "guidance_kind": guidance_kind,
            "queued_feedback": history_content(Some(name), &text),
        }),
    );
    text
}

enum Shape {
    /// An echo, `|| true` or exit-code wrapper whose base the runtime admits.
    Base(String),
    /// The same wrappers around a base that cannot be named as-is.
    Echo,
    Redirect,
    Opaque,
    Other,
}

/// The base is only named when the runtime Bash rules admit it as it stands.
fn base_is_accepted(base: &str, root: &Path) -> bool {
    diagnose_verify_command(base).violation.is_none()
        && normalize_runtime_bash_command_for_boundary(base, root).is_ok()
        && crate::tools::bash::blocked_reason(base, false).is_none()
        && crate::tools::bash::path_confinement_rejection(base, root).is_none()
        && verifier_mutation::rejection(base, true).is_none()
}

fn shape(err_text: &str, command: &str, root: &Path) -> Shape {
    if !err_text.contains("verify_command_policy_error") {
        return Shape::Other;
    }
    let diagnosis = diagnose_verify_command(command);
    match diagnosis.violation {
        Some(
            VerifyCommandViolationKind::SuccessFailureEchoStripped
            | VerifyCommandViolationKind::FallbackTrueStripped
            | VerifyCommandViolationKind::ExitCodeEchoStripped,
        ) => {
            let base = diagnosis.normalized;
            // Normalization collapses runs of whitespace, even inside quotes; a
            // base the model did not type verbatim could mean something else.
            if command.contains(base.as_str()) && base_is_accepted(&base, root) {
                Shape::Base(base)
            } else {
                Shape::Echo
            }
        }
        Some(VerifyCommandViolationKind::ShellControlSyntax)
            if diagnosis
                .reason
                .as_deref()
                .is_some_and(|reason| reason.contains(REDIRECT_REASON_MARKER)) =>
        {
            Shape::Redirect
        }
        _ if err_text.contains(OPAQUE_REASON_MARKER) => Shape::Opaque,
        _ => Shape::Other,
    }
}

fn hint(shape: &Shape) -> Option<String> {
    match shape {
        Shape::Base(base) => Some(format!(
            "Run the base command `{base}` as-is, once (no echo or `|| true` branch; the verifier records pass/fail itself)."
        )),
        Shape::Redirect => {
            Some("Create fixture files with the Write tool, not a shell redirect.".to_string())
        }
        _ => None,
    }
}

fn first_kind(shape: &Shape) -> &'static str {
    match shape {
        Shape::Base(_) => "base_command",
        Shape::Redirect => "write_tool",
        _ => "none",
    }
}

/// The lead for the last refusal, or `None` when the shape has no concrete
/// next call. A named command is only offered when the whole lead survives the
/// history bound and scrub unchanged, so a cut command is never shown as one to
/// run.
fn repeat_lead(name: &str, shape: &Shape, feedback: &str) -> Option<(&'static str, String)> {
    let verifier_alone = || {
        (
            "repeat_verifier_alone",
            format!(
                "{REPEAT_LEAD} Do not resend this command or a reworded echo of it; {STOPS} Next call: Bash with the step's declared verifier command alone (no echo, `|| true` or exit-code branch)."
            ),
        )
    };
    match shape {
        Shape::Base(base) => {
            let call = serde_json::to_string(&json!({ "command": base })).ok()?;
            let lead = format!("{REPEAT_LEAD} Next call: Bash {call}. Run it alone; {STOPS}");
            let sent = history_content(Some(name), &format!("{lead} {feedback}"));
            Some(if sent.starts_with(&lead) {
                ("repeat_base_call", lead)
            } else {
                verifier_alone()
            })
        }
        Shape::Echo => Some(verifier_alone()),
        // The host does not read, run or rewrite the refused body, so the Write
        // arguments are placeholders, never values taken from that body.
        Shape::Redirect | Shape::Opaque => Some((
            "repeat_write_then_verifier",
            format!(
                "{REPEAT_LEAD} Do not resend this command; {STOPS} Next calls: Write {{\"path\":\"PATH\",\"content\":\"CONTENT\"}} for each input file (syntax example: fill in PATH and CONTENT; do not overwrite host-managed fixtures), then Bash with the step's declared verifier alone, not inside `python3 -c` or a redirect."
            ),
        )),
        Shape::Other => None,
    }
}

#[cfg(test)]
mod escalation_tests;
#[cfg(test)]
mod tests;
