use super::{RepairContext, VerificationReport, bullet_list};

/// Compact retries start a fresh session. Preserve the supplied task and
/// remaining failures without replacing the bounded compile-frame instruction.
pub(super) fn render(report: &VerificationReport, context: &RepairContext) -> String {
    let mut out = String::new();
    if let Some(goal) = &context.overall_goal {
        out.push_str(&format!("Overall goal:\n{goal}\n\n"));
    }
    for (label, values) in [
        ("Relevant repair paths", &context.expected_paths),
        (
            "Verification commands still required after repair",
            &context.verify_commands,
        ),
        ("Unresolved profile failures", &report.profile_failures),
        (
            "Files already changed (not proof of repair)",
            &context.changed_files,
        ),
    ] {
        if !values.is_empty() {
            out.push_str(&format!("{label}:\n{}\n\n", bullet_list(values)));
        }
    }
    out
}
