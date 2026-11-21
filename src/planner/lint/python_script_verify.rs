//! A verify command that runs a Python script the same plan creates counts as
//! strong verification in the plan quality report, like `node <script>`.
//!
//! The rule is narrower than `node <anything>`: the command is one simple
//! `python` / `python3` invocation, optionally after short options that take no
//! argument or `-W` / `-X` with their value, whose script operand is a
//! workspace-relative `.py` path listed in the `expected_paths` of an Implement
//! or Setup step of the same plan. Inline code (`-c`), modules (`-m`), `--`,
//! long options, paths outside the workspace, scripts the plan does not create,
//! and compound commands are not strong. Only the quality report uses this;
//! lint, admission, and post-run verification are unchanged.
use crate::planner::step_plan::{StepKind, StepPlan};
use crate::tools::path_guard::validate_workspace_relative;

/// Short options that take no argument and still run the script. `-h`, `-V`,
/// `-?` exit before the script runs and `-i` waits for input, so they are left
/// out.
const NO_ARGUMENT_FLAGS: &str = "bBdEIOPqsSux";

/// Text that makes a command more than one simple command, or substitutes
/// output into it.
const COMPOUND_MARKERS: &[&str] = &["|", "&", ";", "\n", "\r", "`", "$(", "<(", ">("];

/// Whether `command` runs a Python script that an Implement or Setup step of
/// `plan` creates.
pub(super) fn runs_plan_script(plan: &StepPlan, command: &str) -> bool {
    let Some(script) = script_operand(command) else {
        return false;
    };
    plan.steps
        .iter()
        .filter(|step| matches!(step.step_kind(), StepKind::Implement | StepKind::Setup))
        .flat_map(|step| step.expected_paths.iter())
        .any(|path| strip_dot_slash(path.trim()) == script)
}

/// The workspace-relative `.py` script `command` runs with `python` /
/// `python3`, without a leading `./`.
fn script_operand(command: &str) -> Option<&str> {
    if COMPOUND_MARKERS
        .iter()
        .any(|marker| command.contains(marker))
    {
        return None;
    }
    let mut words = command.split_whitespace();
    if !matches!(words.next(), Some("python" | "python3")) {
        return None;
    }
    let script = loop {
        let word = words.next()?;
        let Some(flags) = word.strip_prefix('-') else {
            break word;
        };
        match flags.chars().next() {
            Some('W' | 'X') if flags.len() == 1 => {
                words.next()?;
            }
            Some('W' | 'X') => {}
            Some(_) if flags.chars().all(|flag| NO_ARGUMENT_FLAGS.contains(flag)) => {}
            _ => return None,
        }
    };
    let script = strip_dot_slash(script);
    if !script.ends_with(".py") || script == ".py" {
        return None;
    }
    validate_workspace_relative(script).ok()?;
    Some(script)
}

fn strip_dot_slash(path: &str) -> &str {
    let mut path = path;
    while let Some(rest) = path.strip_prefix("./") {
        path = rest;
    }
    path
}

#[cfg(test)]
mod tests;
