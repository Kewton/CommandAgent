use super::PlanLintError;
use crate::agent::step_runner::StepKind;
use std::path::Path;

pub(super) fn lint_profile_scaffolding(
    profile: &str,
    step_id: &str,
    kind: StepKind,
    instruction: &str,
    expected_paths: &[String],
    cwd: Option<&Path>,
) -> Result<(), PlanLintError> {
    let lower = instruction.to_ascii_lowercase();
    if profile == "rust" && contains_any(&lower, &["cargo init", "cargo new"]) {
        return Err(PlanLintError::ShellScaffold {
            step_id: step_id.to_string(),
            command: "cargo init/new".to_string(),
            guidance: "create Cargo.toml and src/main.rs with Write/Edit".to_string(),
        });
    }
    if profile == "nextjs" {
        if contains_any(
            &lower,
            &[
                "create-next-app",
                "npm create next-app",
                "pnpm create next-app",
                "yarn create next-app",
            ],
        ) {
            return Err(PlanLintError::ShellScaffold {
                step_id: step_id.to_string(),
                command: "create-next-app".to_string(),
                guidance: "create package.json and app/page.tsx with Write/Edit".to_string(),
            });
        }
        lint_nextjs_root_drift(step_id, kind, &lower, expected_paths, cwd)?;
        if contains_any(&lower, &["build script"]) && contains_any(&lower, &["echo ok", "true"]) {
            return Err(PlanLintError::InvalidStepInstruction {
                step_id: step_id.to_string(),
                reason:
                    "Next.js build script must remain honest; do not replace it with no-op commands"
                        .to_string(),
            });
        }
    }
    Ok(())
}

fn lint_nextjs_root_drift(
    step_id: &str,
    kind: StepKind,
    lower_instruction: &str,
    expected_paths: &[String],
    cwd: Option<&Path>,
) -> Result<(), PlanLintError> {
    if !matches!(kind, StepKind::Create | StepKind::Edit | StepKind::Repair) {
        return Ok(());
    }
    if contains_any(
        lower_instruction,
        &["migrate", "migration", "move app root", "move route root"],
    ) {
        return Ok(());
    }
    let Some(cwd) = cwd else {
        return Ok(());
    };
    let has_src_app =
        cwd.join("src/app/page.tsx").exists() || cwd.join("src/app/layout.tsx").exists();
    let has_root_app = cwd.join("app/page.tsx").exists() || cwd.join("app/layout.tsx").exists();
    if has_src_app && !has_root_app && expected_paths.iter().any(|path| path == "app/page.tsx") {
        return Err(PlanLintError::InvalidStepInstruction {
            step_id: step_id.to_string(),
            reason: "Next.js workspace already uses src/app; creating app/page.tsx would split the app root unless this is an explicit migration"
                .to_string(),
        });
    }
    if has_root_app && !has_src_app && expected_paths.iter().any(|path| path == "src/app/page.tsx")
    {
        return Err(PlanLintError::InvalidStepInstruction {
            step_id: step_id.to_string(),
            reason: "Next.js workspace already uses app; creating src/app/page.tsx would split the app root unless this is an explicit migration"
                .to_string(),
        });
    }
    Ok(())
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}
