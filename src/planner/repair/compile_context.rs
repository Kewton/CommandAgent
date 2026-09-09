use super::{RepairContext, VerificationReport, bullet_list};
mod shared_imports;

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
    out.push_str(&contract_context(report, context));
    out
}

pub(super) fn contract_context(report: &VerificationReport, context: &RepairContext) -> String {
    use std::collections::BTreeMap;
    let mut out = String::new();
    let errors = crate::minimal_loop::compile_repair_scope::repairable(
        context.workspace_root.as_deref(),
        &report.compile_errors,
    );
    let mut files = BTreeMap::<&str, usize>::new();
    for error in &errors {
        *files.entry(&error.path).or_default() += 1;
    }
    if files.len() > 1 {
        out.push_str(&format!("\n\nCross-file contract inspection: {} retained diagnostics across {} files (diagnostic counts, not independent defects; bounded tool output may be incomplete).\n", errors.len(), files.len()));
        for (path, count) in files {
            out.push_str(&format!("- {path}: {count} diagnostics\n"));
        }
        if let Some(root) = context.workspace_root.as_deref() {
            out.push_str(&shared_imports::render(root, &errors));
        }
        out.push_str("Align shared declarations, callers and returned values together. Fixing one frame does not establish overall success: rerun the complete type check and every registered build/functional check. Keep required APIs and features; do not disable checking or hide errors with any/ignore.\n");
    }
    if context
        .profile
        .as_deref()
        .is_some_and(crate::planner::profile::is_nextjs_profile)
        && let Some(root) = context.workspace_root.as_deref()
        && let Some(hint) = crate::planner::profiles::nextjs::contract_repair_advisory(root)
    {
        out.push_str(&format!("\n\n{hint}\n"));
    }
    out
}
