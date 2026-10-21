//! Exact interpreter/script parsing; never infer path authority from a substring.
use crate::planner::verify::{normalize_verify_command, shell_words_with_spans};
pub(crate) fn local_script_path(command: &str) -> Option<String> {
    let command = normalize_verify_command(command).ok()?;
    let words = shell_words_with_spans(command.as_str())?;
    let [program, script, ..] = words.as_slice() else {
        return None;
    };
    if !matches!(
        program.value.as_str(),
        "node" | "python" | "python3" | "sh" | "bash"
    ) || script.value.starts_with('-')
    {
        return None;
    }
    let extension = std::path::Path::new(&script.value).extension()?.to_str()?;
    if !matches!(
        (program.value.as_str(), extension),
        ("node", "js" | "cjs" | "mjs") | ("python" | "python3", "py") | ("sh" | "bash", "sh")
    ) {
        return None;
    }
    crate::planner::recovery_contract_authority::verifier_obligations::normalized(&script.value)
        .ok()
}
