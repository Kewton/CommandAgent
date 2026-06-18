use crate::agent::step_runner::StepPlan;
use std::path::Path;

mod instructions;
mod paths;
mod profile;
mod verifiers;
mod workspace;

use instructions::{
    lint_inspect_verifier_boundary, lint_optional_inspection_paths, lint_setup_verify_boundary,
    lint_step_instruction,
};
use paths::lint_expected_path;
use profile::lint_profile_scaffolding;
use verifiers::lint_verifier_command;
use workspace::{lint_inspect_expected_paths_exist, paths_exist};

pub fn lint_step_plan(plan: &StepPlan) -> Result<(), PlanLintError> {
    lint_step_plan_with_workspace(plan, None)
}

pub fn lint_step_plan_with_workspace(
    plan: &StepPlan,
    cwd: Option<&Path>,
) -> Result<(), PlanLintError> {
    for step in &plan.steps {
        for path in &plan.required_artifacts {
            lint_expected_path(path)?;
        }
        for path in &step.expected_paths {
            lint_expected_path(path)?;
        }
        for command in &step.verify {
            lint_verifier_command(&step.id, command)?;
        }
        lint_step_instruction(&step.id, step.kind, &step.instruction, &step.expected_paths)?;
        lint_optional_inspection_paths(
            &step.id,
            step.kind,
            &step.instruction,
            &step.expected_paths,
        )?;
        lint_inspect_expected_paths_exist(&step.id, step.kind, &step.expected_paths, cwd)?;
        lint_inspect_verifier_boundary(&step.id, step.kind, &step.expected_paths, &step.verify)?;
        lint_setup_verify_boundary(
            &step.id,
            step.kind,
            &step.instruction,
            &step.verify,
            !step.expected_paths.is_empty(),
            paths_exist(cwd, &step.expected_paths),
        )?;
        lint_profile_scaffolding(
            plan.profile.as_str(),
            &step.id,
            step.kind,
            &step.instruction,
            &step.expected_paths,
            cwd,
        )?;
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanLintError {
    InvalidExpectedPath {
        path: String,
        reason: String,
    },
    MixedSetupAndVerify {
        step_id: String,
    },
    ShellScaffold {
        step_id: String,
        command: String,
        guidance: String,
    },
    InvalidVerifierCommand {
        step_id: String,
        command: String,
        reason: String,
    },
    InvalidStepInstruction {
        step_id: String,
        reason: String,
    },
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
            Self::ShellScaffold {
                step_id,
                command,
                guidance,
            } => write!(
                f,
                "step `{step_id}` uses shell scaffolding `{command}`; {guidance}"
            ),
            Self::InvalidVerifierCommand {
                step_id,
                command,
                reason,
            } => write!(
                f,
                "step `{step_id}` has invalid verifier command `{command}`: {reason}"
            ),
            Self::InvalidStepInstruction { step_id, reason } => {
                write!(f, "step `{step_id}` has invalid instruction: {reason}")
            }
        }
    }
}

impl std::error::Error for PlanLintError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::step_runner::{ExpectedResult, StepKind, StepPlan, StepPlanStep, WorkIntent};
    use std::fs;

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
            intent: WorkIntent::New,
            required_artifacts: Vec::new(),
            steps: vec![StepPlanStep {
                id: "final-build-check".to_string(),
                kind: StepKind::Verify,
                instruction: "Run npm run build.".to_string(),
                expected_result: ExpectedResult::Pass,
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
    fn rejects_globs_as_expected_paths() {
        let plan = plan_with_paths("python", vec!["app/routes/*.py"]);

        let err = lint_step_plan(&plan).unwrap_err();

        assert!(matches!(err, PlanLintError::InvalidExpectedPath { .. }));
    }

    #[test]
    fn rejects_alternative_paths_as_expected_paths() {
        let plan = plan_with_paths("nextjs", vec!["app/layout.tsx or app/layout.ts"]);

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
    fn rejects_dependency_cache_paths_as_expected_paths() {
        let plan = plan_with_paths("nextjs", vec!["node_modules/.package-lock.json"]);

        let err = lint_step_plan(&plan).unwrap_err();

        assert!(matches!(err, PlanLintError::InvalidExpectedPath { .. }));
    }

    #[test]
    fn rejects_compound_verifier_commands() {
        let mut plan = plan_with_paths("generic", vec!["README.md"]);
        plan.steps[0].verify = vec!["test -f README.md && grep -q usage README.md".to_string()];

        let err = lint_step_plan(&plan).unwrap_err();

        assert!(matches!(err, PlanLintError::InvalidVerifierCommand { .. }));
    }

    #[test]
    fn accepts_quoted_semicolon_in_python_verifier() {
        let mut plan = plan_with_paths("python", vec!["app/main.py"]);
        plan.steps[0].verify =
            vec![r#"python -c "import ast; ast.parse(open('app/main.py').read())""#.to_string()];

        lint_step_plan(&plan).unwrap();
    }

    #[test]
    fn rejects_unquoted_semicolon_in_verifier() {
        let mut plan = plan_with_paths("generic", vec!["README.md"]);
        plan.steps[0].verify = vec!["test -f README.md; cat README.md".to_string()];

        let err = lint_step_plan(&plan).unwrap_err();

        assert!(matches!(err, PlanLintError::InvalidVerifierCommand { .. }));
    }

    #[test]
    fn rejects_unquoted_or_in_verifier() {
        let mut plan = plan_with_paths("generic", vec!["README.md"]);
        plan.steps[0].verify = vec!["test -f README.md || cat README.md".to_string()];

        let err = lint_step_plan(&plan).unwrap_err();

        assert!(matches!(err, PlanLintError::InvalidVerifierCommand { .. }));
    }

    #[test]
    fn rejects_noop_true_verifier() {
        let mut plan = plan_with_paths("generic", vec!["README.md"]);
        plan.steps[0].verify = vec!["true".to_string()];

        let err = lint_step_plan(&plan).unwrap_err();

        assert!(matches!(err, PlanLintError::InvalidVerifierCommand { .. }));
    }

    #[test]
    fn rejects_optional_inspection_expected_paths() {
        let mut plan = plan_with_paths("rust", vec!["src/lib.rs"]);
        plan.steps[0].kind = StepKind::Inspect;
        plan.steps[0].instruction = "Read src/lib.rs if it exists.".to_string();
        plan.steps[0].verify = vec!["test -f src/lib.rs".to_string()];

        let err = lint_step_plan(&plan).unwrap_err();

        assert_eq!(
            err,
            PlanLintError::InvalidStepInstruction {
                step_id: "step".to_string(),
                reason: "optional inspection targets must not be placed in expected_paths; use Read/Glob inspection and an empty expected_paths list".to_string(),
            }
        );
    }

    #[test]
    fn rejects_inspect_test_verifier_without_required_paths() {
        let plan = StepPlan {
            goal: "inspect components".to_string(),
            profile: "nextjs".to_string(),
            style: "default".to_string(),
            intent: WorkIntent::Modify,
            required_artifacts: Vec::new(),
            steps: vec![StepPlanStep {
                id: "inspect-components".to_string(),
                kind: StepKind::Inspect,
                instruction: "Glob components directory to check if AnalyticsPanel already exists."
                    .to_string(),
                expected_result: ExpectedResult::Pass,
                expected_paths: Vec::new(),
                verify: vec!["test -d components".to_string()],
            }],
        };

        let err = lint_step_plan(&plan).unwrap_err();

        assert_eq!(
            err,
            PlanLintError::InvalidStepInstruction {
                step_id: "inspect-components".to_string(),
                reason: "inspect steps must not make optional file or directory discovery a fatal verifier; use verify: [] and observe with Read/Glob"
                    .to_string(),
            }
        );
    }

    #[test]
    fn rejects_inspect_expected_paths_missing_from_workspace() {
        let root = temp_workspace("inspect-missing-path");
        fs::write(root.join("Cargo.toml"), "[package]\nname = \"demo\"\n").unwrap();
        let plan = StepPlan {
            goal: "inspect rust project".to_string(),
            profile: "rust".to_string(),
            style: "default".to_string(),
            intent: WorkIntent::Modify,
            required_artifacts: vec!["Cargo.toml".to_string(), "src/lib.rs".to_string()],
            steps: vec![StepPlanStep {
                id: "inspect-project-structure".to_string(),
                kind: StepKind::Inspect,
                instruction: "Inspect existing Cargo.toml, src/lib.rs, and src/main.rs."
                    .to_string(),
                expected_result: ExpectedResult::Pass,
                expected_paths: vec!["Cargo.toml".to_string(), "src/lib.rs".to_string()],
                verify: vec![
                    "test -f Cargo.toml".to_string(),
                    "test -f src/lib.rs".to_string(),
                ],
            }],
        };

        let err = lint_step_plan_with_workspace(&plan, Some(&root)).unwrap_err();

        assert_eq!(
            err,
            PlanLintError::InvalidStepInstruction {
                step_id: "inspect-project-structure".to_string(),
                reason: "inspect expected_paths must already exist in the workspace; missing src/lib.rs. Use expected_paths: [] and verify: [] for discovery, and enforce final artifacts only at the final boundary".to_string(),
            }
        );
    }

    #[test]
    fn accepts_inspect_expected_paths_that_exist_in_workspace() {
        let root = temp_workspace("inspect-existing-path");
        fs::write(root.join("Cargo.toml"), "[package]\nname = \"demo\"\n").unwrap();
        let plan = StepPlan {
            goal: "inspect rust project".to_string(),
            profile: "rust".to_string(),
            style: "default".to_string(),
            intent: WorkIntent::Modify,
            required_artifacts: Vec::new(),
            steps: vec![StepPlanStep {
                id: "inspect-cargo".to_string(),
                kind: StepKind::Inspect,
                instruction: "Inspect existing Cargo.toml.".to_string(),
                expected_result: ExpectedResult::Pass,
                expected_paths: vec!["Cargo.toml".to_string()],
                verify: vec!["test -f Cargo.toml".to_string()],
            }],
        };

        lint_step_plan_with_workspace(&plan, Some(&root)).unwrap();
    }

    #[test]
    fn rejects_py_compile_for_non_python_source() {
        let mut plan = plan_with_paths("rust", vec!["src/main.rs"]);
        plan.steps[0].verify = vec!["python -m py_compile src/main.rs".to_string()];

        let err = lint_step_plan(&plan).unwrap_err();

        assert!(matches!(err, PlanLintError::InvalidVerifierCommand { .. }));
    }

    #[test]
    fn accepts_py_compile_for_python_source() {
        let mut plan = plan_with_paths("python", vec!["app/main.py"]);
        plan.steps[0].verify = vec!["python -m py_compile app/main.py".to_string()];

        lint_step_plan(&plan).unwrap();
    }

    #[test]
    fn rejects_source_code_grep_verifier() {
        let mut plan = plan_with_paths("rust", vec!["src/main.rs"]);
        plan.steps[0].verify = vec![r##"grep -q "#\[derive(Parser)\]" src/main.rs"##.to_string()];

        let err = lint_step_plan(&plan).unwrap_err();

        assert!(matches!(err, PlanLintError::InvalidVerifierCommand { .. }));
    }

    #[test]
    fn accepts_literal_grep_for_docs() {
        let mut plan = plan_with_paths("docs", vec!["README.md"]);
        plan.steps[0].verify = vec!["grep -q Usage README.md".to_string()];

        lint_step_plan(&plan).unwrap();
    }

    #[test]
    fn rejects_dependency_install_steps() {
        let mut plan = plan_with_paths("nextjs", vec!["package.json"]);
        plan.steps[0].instruction = "Run npm install to download dependencies.".to_string();
        plan.steps[0].expected_paths.clear();

        let err = lint_step_plan(&plan).unwrap_err();

        assert!(matches!(err, PlanLintError::InvalidStepInstruction { .. }));
    }

    #[test]
    fn rejects_dependency_cache_verifiers() {
        let mut plan = plan_with_paths("nextjs", vec!["package.json"]);
        plan.steps[0].kind = StepKind::Verify;
        plan.steps[0].instruction = "Verify local dependencies are available.".to_string();
        plan.steps[0].expected_paths.clear();
        plan.steps[0].verify = vec!["test -f node_modules/.package-lock.json".to_string()];

        let err = lint_step_plan(&plan).unwrap_err();

        assert_eq!(
            err,
            PlanLintError::InvalidVerifierCommand {
                step_id: "step".to_string(),
                command: "test -f node_modules/.package-lock.json".to_string(),
                reason: "verifier commands must not require generated dependency caches; report dependency_missing when local dependencies are unavailable".to_string(),
            }
        );
    }

    #[test]
    fn rejects_directory_only_steps() {
        let mut plan = plan_with_paths("rust", vec!["Cargo.toml"]);
        plan.steps[0].kind = StepKind::Setup;
        plan.steps[0].instruction = "Create the src directory.".to_string();
        plan.steps[0].expected_paths.clear();

        let err = lint_step_plan(&plan).unwrap_err();

        assert!(matches!(err, PlanLintError::InvalidStepInstruction { .. }));
    }

    #[test]
    fn rejects_inspect_step_that_creates_files() {
        let mut plan = plan_with_paths("nextjs", vec!["package.json"]);
        plan.steps[0].kind = StepKind::Inspect;
        plan.steps[0].instruction = "Inspect the workspace and create package.json.".to_string();
        plan.steps[0].expected_paths.clear();

        let err = lint_step_plan(&plan).unwrap_err();

        assert_eq!(
            err,
            PlanLintError::InvalidStepInstruction {
                step_id: "step".to_string(),
                reason: "inspect steps are read-only; move file creation or edits into create/edit/repair steps".to_string(),
            }
        );
    }

    #[test]
    fn rejects_verify_step_that_fixes_files() {
        let mut plan = plan_with_paths("nextjs", vec!["package.json"]);
        plan.steps[0].kind = StepKind::Verify;
        plan.steps[0].instruction = "Run npm run build and fix package.json if needed.".to_string();
        plan.steps[0].expected_paths.clear();
        plan.steps[0].verify = vec!["npm run build".to_string()];

        let err = lint_step_plan(&plan).unwrap_err();

        assert_eq!(
            err,
            PlanLintError::InvalidStepInstruction {
                step_id: "step".to_string(),
                reason:
                    "verify steps must not mutate files; move fixes into create/edit/repair steps"
                        .to_string(),
            }
        );
    }

    #[test]
    fn rejects_obvious_setup_and_verify_mix() {
        let plan = StepPlan {
            goal: "build app".to_string(),
            profile: "generic".to_string(),
            style: "default".to_string(),
            intent: WorkIntent::New,
            required_artifacts: Vec::new(),
            steps: vec![StepPlanStep {
                id: "create-and-build".to_string(),
                kind: StepKind::Create,
                instruction: "Create app/page.tsx and run build.".to_string(),
                expected_result: ExpectedResult::Pass,
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

    #[test]
    fn rejects_new_artifact_create_step_with_build_verifier() {
        let root = temp_workspace("new-artifact-build-verifier");
        let plan = StepPlan {
            goal: "create panel".to_string(),
            profile: "nextjs".to_string(),
            style: "default".to_string(),
            intent: WorkIntent::Modify,
            required_artifacts: Vec::new(),
            steps: vec![StepPlanStep {
                id: "create-panel".to_string(),
                kind: StepKind::Create,
                instruction: "Create components/AnalyticsPanel.tsx.".to_string(),
                expected_result: ExpectedResult::Pass,
                expected_paths: vec!["components/AnalyticsPanel.tsx".to_string()],
                verify: vec![
                    "test -f components/AnalyticsPanel.tsx".to_string(),
                    "npm run build".to_string(),
                ],
            }],
        };

        let err = lint_step_plan_with_workspace(&plan, Some(&root)).unwrap_err();

        assert_eq!(
            err,
            PlanLintError::MixedSetupAndVerify {
                step_id: "create-panel".to_string()
            }
        );
    }

    #[test]
    fn workspace_lint_accepts_existing_paths_in_verify_like_step() {
        let root = temp_workspace("existing-paths");
        fs::create_dir_all(root.join("app")).unwrap();
        fs::write(root.join("package.json"), "{}").unwrap();
        fs::write(
            root.join("app/page.tsx"),
            "export default function Page() {}",
        )
        .unwrap();
        let plan = StepPlan {
            goal: "verify app".to_string(),
            profile: "nextjs".to_string(),
            style: "default".to_string(),
            intent: WorkIntent::Modify,
            required_artifacts: Vec::new(),
            steps: vec![StepPlanStep {
                id: "install-and-build".to_string(),
                kind: StepKind::Setup,
                instruction: "Install dependencies if needed and verify npm run build.".to_string(),
                expected_result: ExpectedResult::Pass,
                expected_paths: vec!["package.json".to_string(), "app/page.tsx".to_string()],
                verify: vec!["npm run build".to_string()],
            }],
        };

        lint_step_plan_with_workspace(&plan, Some(&root)).unwrap();
    }

    #[test]
    fn rejects_rust_cargo_init_scaffolding() {
        let plan = StepPlan {
            goal: "create rust app".to_string(),
            profile: "rust".to_string(),
            style: "default".to_string(),
            intent: WorkIntent::New,
            required_artifacts: Vec::new(),
            steps: vec![StepPlanStep {
                id: "init-project".to_string(),
                kind: StepKind::Create,
                instruction: "Initialize a new Rust project using cargo init.".to_string(),
                expected_result: ExpectedResult::Pass,
                expected_paths: vec!["Cargo.toml".to_string(), "src/main.rs".to_string()],
                verify: vec!["test -f Cargo.toml".to_string()],
            }],
        };

        let err = lint_step_plan(&plan).unwrap_err();

        assert_eq!(
            err,
            PlanLintError::ShellScaffold {
                step_id: "init-project".to_string(),
                command: "cargo init/new".to_string(),
                guidance: "create Cargo.toml and src/main.rs with Write/Edit".to_string()
            }
        );
    }

    #[test]
    fn rejects_nextjs_root_drift_from_src_app_to_app() {
        let root = temp_workspace("nextjs-src-app-drift");
        fs::create_dir_all(root.join("src/app")).unwrap();
        fs::write(
            root.join("src/app/page.tsx"),
            "export default function Page() {}",
        )
        .unwrap();
        let plan = StepPlan {
            goal: "add route".to_string(),
            profile: "nextjs".to_string(),
            style: "default".to_string(),
            intent: WorkIntent::Modify,
            required_artifacts: Vec::new(),
            steps: vec![StepPlanStep {
                id: "create-page".to_string(),
                kind: StepKind::Create,
                instruction: "Create app/page.tsx.".to_string(),
                expected_result: ExpectedResult::Pass,
                expected_paths: vec!["app/page.tsx".to_string()],
                verify: vec!["test -f app/page.tsx".to_string()],
            }],
        };

        let err = lint_step_plan_with_workspace(&plan, Some(&root)).unwrap_err();

        assert_eq!(
            err,
            PlanLintError::InvalidStepInstruction {
                step_id: "create-page".to_string(),
                reason: "Next.js workspace already uses src/app; creating app/page.tsx would split the app root unless this is an explicit migration"
                    .to_string(),
            }
        );
    }

    #[test]
    fn accepts_nextjs_explicit_root_migration() {
        let root = temp_workspace("nextjs-root-migration");
        fs::create_dir_all(root.join("src/app")).unwrap();
        fs::write(
            root.join("src/app/page.tsx"),
            "export default function Page() {}",
        )
        .unwrap();
        let plan = StepPlan {
            goal: "migrate route".to_string(),
            profile: "nextjs".to_string(),
            style: "default".to_string(),
            intent: WorkIntent::Modify,
            required_artifacts: Vec::new(),
            steps: vec![StepPlanStep {
                id: "migrate-page".to_string(),
                kind: StepKind::Create,
                instruction: "Migrate the app root by creating app/page.tsx.".to_string(),
                expected_result: ExpectedResult::Pass,
                expected_paths: vec!["app/page.tsx".to_string()],
                verify: vec!["test -f app/page.tsx".to_string()],
            }],
        };

        lint_step_plan_with_workspace(&plan, Some(&root)).unwrap();
    }

    fn plan_with_paths(profile: &str, paths: Vec<&str>) -> StepPlan {
        StepPlan {
            goal: "goal".to_string(),
            profile: profile.to_string(),
            style: "default".to_string(),
            intent: WorkIntent::Unknown,
            required_artifacts: Vec::new(),
            steps: vec![StepPlanStep {
                id: "step".to_string(),
                kind: StepKind::Create,
                instruction: "Create files.".to_string(),
                expected_result: ExpectedResult::Pass,
                expected_paths: paths.into_iter().map(ToString::to_string).collect(),
                verify: Vec::new(),
            }],
        }
    }

    fn temp_workspace(name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "commandagent-plan-lint-{}-{}",
            name,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }
}
