//! Runtime Bash segments that the host decides itself (Issue #879).
//!
//! A Verify step may repeat a declared `anvil-nextjs-check:` token. The runtime
//! never hands it to a shell: it returns the host's Rust judgement. Only the
//! namespace is recognized here. An altered token still becomes a segment, and
//! the judgement then fails it, so no altered text reaches a shell or is
//! admitted as a different command. Inline interpreters stay refused by the
//! shared verify rules; nothing here exempts a text that merely looks declared.
use std::path::Path;

use super::{NormalizedVerifyCommand, VerifyInstallCommandFamily};
use crate::planner::profiles::nextjs::package_script_check;
use crate::tools::bash::{BashOutcome, BashOutcomeKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostCheckSegment {
    command: String,
}

impl HostCheckSegment {
    pub fn as_str(&self) -> &str {
        &self.command
    }

    /// The runtime executes this segment itself, like an install substitution, so
    /// the caller's `install_family().is_some()` routes even a lone token to the
    /// segment executor instead of the Bash tool's shell.
    pub(crate) fn runtime_owned_family(&self) -> Option<VerifyInstallCommandFamily> {
        Some(VerifyInstallCommandFamily::Node)
    }

    /// Judge the token for the segment executor: the formatted segment text, in the
    /// shape of a Bash outcome so progress and evidence readers see the same
    /// `outcome:` line a shell run would give, and whether it passed. Nothing runs.
    pub(crate) fn execute(&self, root: &Path, profile: &str, number: usize) -> (String, bool) {
        let verdict = package_script_check::evaluate_in_profile(root, Some(profile), &self.command);
        let report = match &verdict {
            Ok(()) => {
                "outcome: Success\nstatus: host check passed (not run in a shell)".to_string()
            }
            Err(reason) => format!(
                "outcome: CommandFailed\nstatus: host check failed (not run in a shell)\nsummary: {reason}"
            ),
        };
        (
            format!("segment {number} host check: {}\n{report}", self.command),
            verdict.is_ok(),
        )
    }
}

/// Runs one normalized verify command. A host check token (for instance the repair of
/// a grep on package.json) is judged by the host and never reaches a shell; any other
/// command runs as before. Like a declared token, it is bound to the nextjs profile.
pub(super) fn run_for_verify(
    command: &NormalizedVerifyCommand,
    root: &Path,
    profile: Option<&str>,
    offline: bool,
) -> anyhow::Result<BashOutcome> {
    if !package_script_check::is_check_command(command.as_str()) {
        return crate::minimal_loop::verifier_env::run_structured_for_verify_with_profile(
            command, root, profile, offline,
        );
    }
    let verdict = package_script_check::evaluate_in_profile(root, profile, command.as_str());
    let (kind, status, reason) = match &verdict {
        Ok(()) => (BashOutcomeKind::Success, "exit status: 0", String::new()),
        Err(reason) => (
            BashOutcomeKind::CommandFailed,
            "exit status: 1",
            reason.clone(),
        ),
    };
    Ok(BashOutcome {
        kind,
        status: Some(status.to_string()),
        stdout: String::new(),
        stderr: reason.clone(),
        elapsed_ms: 0,
        summary: reason,
    })
}

/// Whether any segment of `command` is in the host's check namespace. The shared
/// single-command normalization admits any word as a program, so such a command
/// must take the segment route instead of reaching a shell.
pub(super) fn names_check(command: &str) -> bool {
    super::split_runtime_shell_segments(command).is_ok_and(|segments| {
        segments
            .iter()
            .any(|(_, segment)| package_script_check::is_check_command(segment))
    })
}

pub(super) fn recognize(segment: &str) -> Option<HostCheckSegment> {
    package_script_check::is_check_command(segment).then(|| HostCheckSegment {
        command: segment.trim().to_string(),
    })
}

// The runtime Bash tests live beside the segment executor they exercise but reach the
// loop only through its crate-visible entry points, so the chokepoint files stay as is.
#[cfg(test)]
#[path = "../../minimal_loop/loop_run/runtime_bash_effects_host_check_tests.rs"]
mod runtime_tests;
