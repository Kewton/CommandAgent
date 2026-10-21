/// Inspections remain useful step checks, but cannot establish final success.
/// Share this boundary with generated completion-contract registration.
pub(crate) fn is_artifact_only_verify_command(command: &str) -> bool {
    let lower = command.trim().to_ascii_lowercase();
    let mut words = lower.split_whitespace();
    match words.next() {
        Some("cat") => words.next().is_some(),
        Some("test") => matches!(words.next(), Some("-f" | "-e")),
        _ => false,
    }
}
