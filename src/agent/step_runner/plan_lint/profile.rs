use super::PlanLintError;

pub(super) fn lint_profile_scaffolding(
    profile: &str,
    step_id: &str,
    instruction: &str,
) -> Result<(), PlanLintError> {
    let lower = instruction.to_ascii_lowercase();
    if profile == "rust" && contains_any(&lower, &["cargo init", "cargo new"]) {
        return Err(PlanLintError::ShellScaffold {
            step_id: step_id.to_string(),
            command: "cargo init/new".to_string(),
            guidance: "create Cargo.toml and src/main.rs with Write/Edit".to_string(),
        });
    }
    Ok(())
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}
