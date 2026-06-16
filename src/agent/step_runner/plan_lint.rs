use crate::agent::step_runner::StepPlan;
use std::path::Path;

pub fn lint_step_plan(plan: &StepPlan) -> Result<(), PlanLintError> {
    for step in &plan.steps {
        for path in &step.expected_paths {
            lint_expected_path(path)?;
        }
        lint_setup_verify_boundary(&step.id, &step.instruction, !step.expected_paths.is_empty())?;
    }
    Ok(())
}

fn lint_expected_path(path: &str) -> Result<(), PlanLintError> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err(PlanLintError::InvalidExpectedPath {
            path: path.to_string(),
            reason: "path is empty".to_string(),
        });
    }
    let path_obj = Path::new(trimmed);
    if path_obj.is_absolute() || trimmed.contains("..") {
        return Err(PlanLintError::InvalidExpectedPath {
            path: path.to_string(),
            reason: "path must be repository-relative and cannot contain parent traversal"
                .to_string(),
        });
    }
    if trimmed.contains(':') || trimmed.starts_with("$.") {
        return Err(PlanLintError::InvalidExpectedPath {
            path: path.to_string(),
            reason: "expected_paths must be file paths, not JSON/property selectors".to_string(),
        });
    }
    if looks_like_version(trimmed) {
        return Err(PlanLintError::InvalidExpectedPath {
            path: path.to_string(),
            reason: "expected_paths must be file paths, not version strings".to_string(),
        });
    }
    if !looks_like_file_path(trimmed) {
        return Err(PlanLintError::InvalidExpectedPath {
            path: path.to_string(),
            reason: "expected_paths must name concrete files".to_string(),
        });
    }
    Ok(())
}

fn lint_setup_verify_boundary(
    step_id: &str,
    instruction: &str,
    has_expected_paths: bool,
) -> Result<(), PlanLintError> {
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
    let verify = contains_any(
        &lower,
        &[
            "verify ",
            "validate ",
            "run build",
            "npm run build",
            "cargo test",
            "cargo build",
            "pytest",
            "test ",
        ],
    );

    if has_expected_paths && setup && verify {
        return Err(PlanLintError::MixedSetupAndVerify {
            step_id: step_id.to_string(),
        });
    }
    Ok(())
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

fn looks_like_file_path(path: &str) -> bool {
    if path.contains('/') {
        return !path.ends_with('/');
    }
    if matches!(
        path,
        "Dockerfile" | "Makefile" | "README" | "LICENSE" | "Cargo.lock"
    ) {
        return true;
    }
    let Some(extension) = Path::new(path).extension().and_then(|ext| ext.to_str()) else {
        return false;
    };
    matches!(
        extension,
        "cjs"
            | "css"
            | "go"
            | "html"
            | "js"
            | "json"
            | "jsx"
            | "lock"
            | "md"
            | "mjs"
            | "py"
            | "rs"
            | "toml"
            | "ts"
            | "tsx"
            | "txt"
            | "yaml"
            | "yml"
    )
}

fn looks_like_version(value: &str) -> bool {
    let mut saw_dot = false;
    let mut saw_digit = false;
    for ch in value.chars() {
        if ch == '.' {
            saw_dot = true;
        } else if ch.is_ascii_digit() {
            saw_digit = true;
        } else {
            return false;
        }
    }
    saw_dot && saw_digit
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanLintError {
    InvalidExpectedPath { path: String, reason: String },
    MixedSetupAndVerify { step_id: String },
}

impl std::fmt::Display for PlanLintError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidExpectedPath { path, reason } => {
                write!(f, "invalid expected path `{path}`: {reason}")
            }
            Self::MixedSetupAndVerify { step_id } => write!(
                f,
                "step `{step_id}` mixes setup/editing work with verification; split it into separate steps"
            ),
        }
    }
}

impl std::error::Error for PlanLintError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::step_runner::{StepPlan, StepPlanStep};

    #[test]
    fn accepts_generic_file_paths() {
        let plan = plan_with_paths("generic", vec!["README.md", "src/main.rs"]);

        lint_step_plan(&plan).unwrap();
    }

    #[test]
    fn accepts_nextjs_verification_step_without_naming_every_file_in_instruction() {
        let plan = StepPlan {
            goal: "verify app".to_string(),
            profile: "nextjs".to_string(),
            style: "default".to_string(),
            steps: vec![StepPlanStep {
                id: "final-build-check".to_string(),
                instruction: "Run npm run build.".to_string(),
                expected_paths: vec![
                    "package.json".to_string(),
                    "app/page.tsx".to_string(),
                    "next.config.js".to_string(),
                ],
                verify: vec!["npm run build".to_string()],
            }],
        };

        lint_step_plan(&plan).unwrap();
    }

    #[test]
    fn rejects_package_identifiers_as_expected_paths() {
        let plan = plan_with_paths("nextjs", vec!["next"]);

        let err = lint_step_plan(&plan).unwrap_err();

        assert!(matches!(err, PlanLintError::InvalidExpectedPath { .. }));
    }

    #[test]
    fn rejects_json_property_paths_as_expected_paths() {
        let plan = plan_with_paths("nextjs", vec!["package.json:scripts.build"]);

        let err = lint_step_plan(&plan).unwrap_err();

        assert!(matches!(err, PlanLintError::InvalidExpectedPath { .. }));
    }

    #[test]
    fn rejects_version_strings_as_expected_paths() {
        let plan = plan_with_paths("nextjs", vec!["18.2.0"]);

        let err = lint_step_plan(&plan).unwrap_err();

        assert!(matches!(err, PlanLintError::InvalidExpectedPath { .. }));
    }

    #[test]
    fn rejects_obvious_setup_and_verify_mix() {
        let plan = StepPlan {
            goal: "build app".to_string(),
            profile: "generic".to_string(),
            style: "default".to_string(),
            steps: vec![StepPlanStep {
                id: "create-and-build".to_string(),
                instruction: "Create app/page.tsx and run build.".to_string(),
                expected_paths: vec!["app/page.tsx".to_string()],
                verify: vec!["npm run build".to_string()],
            }],
        };

        let err = lint_step_plan(&plan).unwrap_err();

        assert_eq!(
            err,
            PlanLintError::MixedSetupAndVerify {
                step_id: "create-and-build".to_string()
            }
        );
    }

    fn plan_with_paths(profile: &str, paths: Vec<&str>) -> StepPlan {
        StepPlan {
            goal: "goal".to_string(),
            profile: profile.to_string(),
            style: "default".to_string(),
            steps: vec![StepPlanStep {
                id: "step".to_string(),
                instruction: "Create files.".to_string(),
                expected_paths: paths.into_iter().map(ToString::to_string).collect(),
                verify: Vec::new(),
            }],
        }
    }
}
