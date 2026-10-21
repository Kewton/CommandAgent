//! Classification of verify command failures that mean the command's
//! executable is not installed (Issue #776, parent #697).

/// Whether `reason` reports that the executable `command` starts with is
/// missing, as opposed to a test or build failure that happens to print
/// "not found".
pub(super) fn is_dependency_missing_error(command: &str, reason: &str) -> bool {
    let lower = reason.to_ascii_lowercase();
    let first_word = command.split_whitespace().next().unwrap_or_default();
    let executable_markers = [
        "command not found",
        "not found:",
        "no such file or directory",
        "failed to spawn command",
    ];
    let marker_found = executable_markers
        .iter()
        .any(|marker| lower.contains(marker));
    if !marker_found && !dash_reports_missing_executable(first_word, reason) {
        return false;
    }
    if lower.contains("assertionerror") || lower.contains("not found in") {
        return false;
    }
    first_word.is_empty() || lower.contains(&first_word.to_ascii_lowercase())
}

/// dash prints `sh: 1: npm: not found` (no trailing colon) for a missing
/// executable. Only a whole line of that shape naming `executable` counts.
fn dash_reports_missing_executable(executable: &str, reason: &str) -> bool {
    !executable.is_empty()
        && reason.lines().any(|line| {
            let parts: Vec<&str> = line.trim().split(": ").collect();
            let [shell, line_number, missing, "not found"] = parts.as_slice() else {
                return false;
            };
            let shell_name = shell.rsplit('/').next().unwrap_or_default();
            matches!(shell_name, "sh" | "dash")
                && !line_number.is_empty()
                && line_number.bytes().all(|byte| byte.is_ascii_digit())
                && *missing == executable
        })
}
