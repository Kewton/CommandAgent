use super::PlanLintError;
use super::verifiers::verifier_runs_build_test;
use crate::agent::step_runner::StepKind;

pub(super) fn lint_step_instruction(
    step_id: &str,
    kind: StepKind,
    instruction: &str,
    expected_paths: &[String],
) -> Result<(), PlanLintError> {
    let lower = instruction.to_ascii_lowercase();
    if contains_any(
        &lower,
        &[
            "npm install",
            "npm ci",
            "pnpm install",
            "pip install",
            "python -m pip",
        ],
    ) {
        return Err(PlanLintError::InvalidStepInstruction {
            step_id: step_id.to_string(),
            reason: "dependency installation must not be a required success step; report dependency_missing when offline".to_string(),
        });
    }
    if matches!(kind, StepKind::Setup | StepKind::Create)
        && expected_paths.is_empty()
        && contains_any(&lower, &["create", "make", "initialize", "init", "ensure"])
        && contains_any(
            &lower,
            &[" directory", " directories", " folder", " folders"],
        )
        && (contains_any(&lower, &["create", "make", "initialize", "init"])
            || contains_any(&lower, &["exists", "exist"]))
    {
        return Err(PlanLintError::InvalidStepInstruction {
            step_id: step_id.to_string(),
            reason: "directory-only steps are unnecessary because Write creates parent directories automatically".to_string(),
        });
    }
    if matches!(kind, StepKind::Inspect)
        && contains_any(
            &lower,
            &[
                "create ", "write ", "edit ", "modify ", "update ", "fix ", "add ",
            ],
        )
    {
        return Err(PlanLintError::InvalidStepInstruction {
            step_id: step_id.to_string(),
            reason: "inspect steps are read-only; move file creation or edits into create/edit/repair steps"
                .to_string(),
        });
    }
    if matches!(kind, StepKind::Verify)
        && contains_any(
            &lower,
            &["write ", "edit ", "modify ", "update ", "fix ", "repair "],
        )
    {
        return Err(PlanLintError::InvalidStepInstruction {
            step_id: step_id.to_string(),
            reason: "verify steps must not mutate files; move fixes into create/edit/repair steps"
                .to_string(),
        });
    }
    Ok(())
}

pub(super) fn lint_optional_inspection_paths(
    step_id: &str,
    kind: StepKind,
    instruction: &str,
    expected_paths: &[String],
) -> Result<(), PlanLintError> {
    if !matches!(kind, StepKind::Inspect) || expected_paths.is_empty() {
        return Ok(());
    }
    let lower = instruction.to_ascii_lowercase();
    if contains_any(
        &lower,
        &[
            "if it exists",
            "if exists",
            "if present",
            "if available",
            "when present",
            "if any",
            "if there is",
            "if there are",
        ],
    ) {
        return Err(PlanLintError::InvalidStepInstruction {
            step_id: step_id.to_string(),
            reason: "optional inspection targets must not be placed in expected_paths; use Read/Glob inspection and an empty expected_paths list"
                .to_string(),
        });
    }
    Ok(())
}

pub(super) fn lint_inspect_verifier_boundary(
    step_id: &str,
    kind: StepKind,
    expected_paths: &[String],
    verify: &[String],
) -> Result<(), PlanLintError> {
    if !matches!(kind, StepKind::Inspect) || !expected_paths.is_empty() || verify.is_empty() {
        return Ok(());
    }
    if verify.iter().any(|command| {
        let lower = command.trim().to_ascii_lowercase();
        lower.starts_with("test -f ") || lower.starts_with("test -d ")
    }) {
        return Err(PlanLintError::InvalidStepInstruction {
            step_id: step_id.to_string(),
            reason: "inspect steps must not make optional file or directory discovery a fatal verifier; use verify: [] and observe with Read/Glob"
                .to_string(),
        });
    }
    Ok(())
}

pub(super) fn lint_setup_verify_boundary(
    step_id: &str,
    kind: StepKind,
    instruction: &str,
    verify_commands: &[String],
    has_expected_paths: bool,
    expected_paths_already_exist: bool,
) -> Result<(), PlanLintError> {
    if matches!(kind, StepKind::Verify | StepKind::Report) {
        return Ok(());
    }
    let lower = instruction.to_ascii_lowercase();
    let setup = contains_any(
        &lower,
        &[
            "create ",
            "write ",
            "edit ",
            "add ",
            "install ",
            "scaffold ",
            "configure ",
            "implement ",
        ],
    );
    let verifier_has_build_test = verifier_runs_build_test(verify_commands);
    let verify = contains_any(
        &lower,
        &[
            "verify ",
            "validate ",
            "run build",
            "npm run build",
            "cargo check",
            "cargo test",
            "cargo build",
            "pytest",
            "test ",
        ],
    ) || verifier_has_build_test;

    let explicit_sequence = contains_any(
        &lower,
        &[
            " and run ",
            " then run ",
            " and verify ",
            " then verify ",
            " and validate ",
            " then validate ",
            " and test ",
            " then test ",
            " and build ",
            " then build ",
        ],
    );

    if has_expected_paths
        && !expected_paths_already_exist
        && setup
        && verify
        && (explicit_sequence || verifier_has_build_test)
    {
        return Err(PlanLintError::MixedSetupAndVerify {
            step_id: step_id.to_string(),
        });
    }
    Ok(())
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}
