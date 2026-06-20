use crate::agent::step_runner::correction_evidence::PlanCorrectionEvidence;
use crate::agent::step_runner::profile_artifact::{
    ArtifactProvenance, artifact_kind_label, classify_profile_artifact, setup_step_may_own_artifact,
};
use crate::agent::step_runner::profiles::{
    ProfileId, ProfileObligation, lint_profile_plan, lint_profile_step_contract,
};
use crate::agent::step_runner::{StepKind, StepPlan};
use std::collections::BTreeSet;
use std::path::Path;

mod instructions;
mod paths;
mod verifiers;
mod workspace;

use instructions::{
    lint_inspect_verifier_boundary, lint_optional_inspection_paths, lint_setup_verify_boundary,
    lint_step_instruction,
};
use paths::lint_expected_path;
use verifiers::lint_verifier_command;
use workspace::{lint_inspect_expected_paths_exist, paths_exist};

pub fn lint_step_plan(plan: &StepPlan) -> Result<(), PlanLintError> {
    lint_step_plan_with_workspace(plan, None)
}

pub fn lint_step_plan_with_workspace(
    plan: &StepPlan,
    cwd: Option<&Path>,
) -> Result<(), PlanLintError> {
    lint_step_plan_generic(plan, cwd)
}

fn lint_step_plan_generic(plan: &StepPlan, cwd: Option<&Path>) -> Result<(), PlanLintError> {
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
        lint_profile_step_contract(
            plan.profile.as_str(),
            &step.id,
            step.kind,
            &step.instruction,
            &step.expected_paths,
            cwd,
        )?;
        lint_step_artifact_ownership(
            plan.profile.as_str(),
            &step.id,
            step.kind,
            &step.expected_paths,
        )?;
    }
    lint_required_artifact_owners(plan, cwd)?;
    Ok(())
}

fn lint_required_artifact_owners(plan: &StepPlan, cwd: Option<&Path>) -> Result<(), PlanLintError> {
    if plan.required_artifacts.is_empty() {
        return Ok(());
    }

    if !plan
        .steps
        .iter()
        .any(|step| mutation_step_can_own_required_artifact(step.kind))
    {
        return Ok(());
    }

    let owned_paths = plan
        .steps
        .iter()
        .filter(|step| mutation_step_can_own_required_artifact(step.kind))
        .flat_map(|step| {
            step.expected_paths
                .iter()
                .map(|path| normalize_plan_path(path))
        })
        .collect::<BTreeSet<_>>();

    for artifact in &plan.required_artifacts {
        let normalized = normalize_plan_path(artifact);
        if owned_paths.contains(&normalized)
            || cwd.is_some_and(|cwd| cwd.join(&normalized).exists())
        {
            continue;
        }

        let reason = format!(
            "required_artifact `{artifact}` is not owned by any create/edit/setup/repair step expected_paths"
        );
        return Err(PlanLintError::ContractViolation {
            step_id: "plan".to_string(),
            reason: reason.clone(),
            evidence: Box::new(
                PlanCorrectionEvidence::new("plan_lint.step_decomposition")
                    .with_failed_step("plan")
                    .with_violated_contract("required_artifact_step_ownership")
                    .with_reason_code("required_artifact_missing_step_owner")
                    .with_failure_kind("plan_decomposition_failure")
                    .with_target_field("steps.expected_paths")
                    .with_target_path(artifact.clone())
                    .with_required_paths(vec![artifact.clone()])
                    .with_missing_paths(vec![artifact.clone()])
                    .with_repair_target(artifact.clone())
                    .with_active_job("scaffold_materialization")
                    .with_repair_kind("plan_correction")
                    .with_repair_action("add_required_artifact_owner_step")
                    .with_required_action(
                        "add a create/edit/setup/repair step whose expected_paths includes this required_artifact before verify/report steps"
                    )
                    .with_disallowed_actions(vec![
                        "do not remove the required_artifact",
                        "do not satisfy final artifacts only from verify/report steps",
                    ])
                    .with_diagnostic(reason),
            ),
        });
    }

    Ok(())
}

fn mutation_step_can_own_required_artifact(kind: StepKind) -> bool {
    matches!(
        kind,
        StepKind::Create | StepKind::Edit | StepKind::Setup | StepKind::Repair
    )
}

fn normalize_plan_path(path: &str) -> String {
    path.trim().trim_start_matches("./").replace('\\', "/")
}

fn lint_step_artifact_ownership(
    profile: &str,
    step_id: &str,
    kind: StepKind,
    expected_paths: &[String],
) -> Result<(), PlanLintError> {
    if kind != StepKind::Setup {
        return Ok(());
    }
    let Ok(profile) = ProfileId::parse(profile) else {
        return Ok(());
    };
    for path in expected_paths {
        let artifact =
            classify_profile_artifact(profile, path, ArtifactProvenance::StepExpectedPath);
        if setup_step_may_own_artifact(artifact.kind) {
            continue;
        }
        let observed_role = artifact_kind_label(artifact.kind);
        let reason = format!(
            "setup step cannot own `{}` because it is classified as {observed_role}; split setup/config work from source creation or change the step kind to create/edit",
            artifact.path
        );
        return Err(PlanLintError::ContractViolation {
            step_id: step_id.to_string(),
            reason: reason.clone(),
            evidence: Box::new(
                PlanCorrectionEvidence::new("plan_lint.step_decomposition")
                    .with_failed_step(step_id.to_string())
                    .with_violated_contract("step_kind_artifact_role")
                    .with_reason_code("setup_step_owns_non_setup_artifact")
                    .with_target_field("expected_paths")
                    .with_target_path(artifact.path.clone())
                    .with_rejected_value(artifact.path)
                    .with_observed_expected_pairs(vec![format!(
                        "observed_role={observed_role}; expected_role=setup/manifest or setup/config"
                    )])
                    .with_required_literals(vec!["setup/manifest", "setup/config"])
                    .with_required_action(
                        "change this step kind to create/edit, or split setup/config work from source creation"
                    )
                    .with_diagnostic(reason),
            ),
        });
    }
    Ok(())
}

pub fn lint_step_plan_with_workspace_and_obligations(
    plan: &StepPlan,
    cwd: Option<&Path>,
    obligations: &[ProfileObligation],
) -> Result<(), PlanLintError> {
    lint_step_plan_generic(plan, cwd)?;
    lint_profile_plan(plan, cwd, obligations)
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
    ContractViolation {
        step_id: String,
        reason: String,
        evidence: Box<PlanCorrectionEvidence>,
    },
}

impl PlanLintError {
    pub fn correction_evidence(&self) -> Option<&PlanCorrectionEvidence> {
        match self {
            Self::ContractViolation { evidence, .. } => Some(evidence.as_ref()),
            _ => None,
        }
    }
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
            Self::ContractViolation {
                step_id, reason, ..
            } => {
                write!(f, "step `{step_id}` has invalid instruction: {reason}")
            }
        }
    }
}

impl std::error::Error for PlanLintError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::step_runner::profiles::ProfileObligation;
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
    fn nextjs_profile_lint_rejects_npx_verifier() {
        let plan = StepPlan {
            goal: "verify app".to_string(),
            profile: "nextjs".to_string(),
            style: "default".to_string(),
            intent: WorkIntent::New,
            required_artifacts: Vec::new(),
            steps: vec![StepPlanStep {
                id: "verify-compilation".to_string(),
                kind: StepKind::Verify,
                instruction: "Verify TypeScript compilation.".to_string(),
                expected_result: ExpectedResult::Pass,
                expected_paths: Vec::new(),
                verify: vec!["npx tsc --noEmit".to_string()],
            }],
        };

        let err = lint_step_plan_with_workspace_and_obligations(&plan, None, &[]).unwrap_err();

        match err {
            PlanLintError::ContractViolation {
                step_id,
                reason,
                evidence,
            } => {
                assert_eq!(step_id, "verify-compilation");
                assert!(reason.contains("uses npx"));
                assert_eq!(
                    evidence.violated_contract.as_deref(),
                    Some("nextjs_verifier_command_required")
                );
                assert_eq!(evidence.rejected_value.as_deref(), Some("npx tsc --noEmit"));
                assert_eq!(
                    evidence.required_literals,
                    vec!["npm run build".to_string()]
                );
            }
            other => panic!("expected nextjs verifier contract violation, got {other:?}"),
        }
    }

    #[test]
    fn nextjs_profile_lint_accepts_npm_run_build_verifier() {
        let plan = StepPlan {
            goal: "verify app".to_string(),
            profile: "nextjs".to_string(),
            style: "default".to_string(),
            intent: WorkIntent::New,
            required_artifacts: Vec::new(),
            steps: vec![StepPlanStep {
                id: "verify-build".to_string(),
                kind: StepKind::Verify,
                instruction: "Run the Next.js build.".to_string(),
                expected_result: ExpectedResult::Pass,
                expected_paths: Vec::new(),
                verify: vec!["npm run build".to_string()],
            }],
        };

        lint_step_plan_with_workspace_and_obligations(&plan, None, &[]).unwrap();
    }

    #[test]
    fn rejects_setup_step_owning_nextjs_global_css() {
        let mut plan = plan_with_paths("nextjs", vec!["package.json", "app/globals.css"]);
        plan.steps[0].kind = StepKind::Setup;
        plan.steps[0].instruction =
            "Create package.json and app/globals.css for Tailwind styling.".to_string();

        let err = lint_step_plan(&plan).unwrap_err();

        assert_setup_ownership_violation(err, "step", "app/globals.css", "source/style");
    }

    #[test]
    fn rejects_setup_step_owning_nextjs_route_source() {
        let mut plan = plan_with_paths("nextjs", vec!["src/app/page.tsx"]);
        plan.steps[0].kind = StepKind::Setup;
        plan.steps[0].instruction = "Prepare src/app/page.tsx.".to_string();

        let err = lint_step_plan(&plan).unwrap_err();

        assert_setup_ownership_violation(err, "step", "src/app/page.tsx", "route_entry");
    }

    #[test]
    fn accepts_setup_step_owning_nextjs_manifest_and_config() {
        let mut plan = plan_with_paths("nextjs", vec!["package.json", "tailwind.config.js"]);
        plan.steps[0].kind = StepKind::Setup;
        plan.steps[0].instruction = "Create package.json and tailwind.config.js.".to_string();

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
    fn generic_lint_error_has_no_contract_evidence() {
        let plan = plan_with_paths("nextjs", vec!["app/layout.tsx or app/layout.ts"]);

        let err = lint_step_plan(&plan).unwrap_err();

        assert!(err.correction_evidence().is_none(), "{err:?}");
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

        match err {
            PlanLintError::ContractViolation {
                step_id,
                reason,
                evidence,
            } => {
                assert_eq!(step_id, "inspect-project-structure");
                assert!(reason.contains("missing src/lib.rs"));
                assert_eq!(
                    evidence.violated_contract.as_deref(),
                    Some("inspect_future_artifact")
                );
                assert_eq!(evidence.missing_paths, vec!["src/lib.rs"]);
                assert_eq!(evidence.active_job.as_deref(), Some("explicit_stop"));
                assert_eq!(
                    evidence.explicit_stop_reason.as_deref(),
                    Some("inspect_future_artifact")
                );
                assert!(
                    evidence
                        .artifact_graph_summary
                        .iter()
                        .any(|line| line.contains("src/lib.rs"))
                );
            }
            other => panic!("expected artifact graph contract violation, got {other:?}"),
        }
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
    fn rejects_required_artifact_without_mutation_step_owner() {
        let root = temp_workspace("required-artifact-unowned");
        let plan = StepPlan {
            goal: "create rust app".to_string(),
            profile: "rust".to_string(),
            style: "default".to_string(),
            intent: WorkIntent::New,
            required_artifacts: vec!["Cargo.toml".to_string(), "src/main.rs".to_string()],
            steps: vec![StepPlanStep {
                id: "write-main".to_string(),
                kind: StepKind::Create,
                instruction: "Create src/main.rs.".to_string(),
                expected_result: ExpectedResult::Pass,
                expected_paths: vec!["src/main.rs".to_string()],
                verify: vec!["test -f src/main.rs".to_string()],
            }],
        };

        let err = lint_step_plan_with_workspace(&plan, Some(&root)).unwrap_err();

        match err {
            PlanLintError::ContractViolation {
                step_id,
                reason,
                evidence,
            } => {
                assert_eq!(step_id, "plan");
                assert!(
                    reason.contains("required_artifact `Cargo.toml` is not owned"),
                    "{reason}"
                );
                assert_eq!(
                    evidence.violated_contract.as_deref(),
                    Some("required_artifact_step_ownership")
                );
                assert_eq!(
                    evidence.reason_code.as_deref(),
                    Some("required_artifact_missing_step_owner")
                );
                assert_eq!(
                    evidence.target_field.as_deref(),
                    Some("steps.expected_paths")
                );
                assert_eq!(evidence.target_path.as_deref(), Some("Cargo.toml"));
                assert_eq!(evidence.repair_target.as_deref(), Some("Cargo.toml"));
                assert_eq!(
                    evidence.repair_action.as_deref(),
                    Some("add_required_artifact_owner_step")
                );
            }
            other => panic!("expected required artifact ownership violation, got {other:?}"),
        }
    }

    #[test]
    fn accepts_required_artifact_that_already_exists_without_owner_step() {
        let root = temp_workspace("required-artifact-existing");
        fs::write(root.join("Cargo.toml"), "[package]\nname = \"demo\"\n").unwrap();
        let plan = StepPlan {
            goal: "update rust app".to_string(),
            profile: "rust".to_string(),
            style: "default".to_string(),
            intent: WorkIntent::Modify,
            required_artifacts: vec!["Cargo.toml".to_string(), "src/main.rs".to_string()],
            steps: vec![StepPlanStep {
                id: "write-main".to_string(),
                kind: StepKind::Create,
                instruction: "Create src/main.rs.".to_string(),
                expected_result: ExpectedResult::Pass,
                expected_paths: vec!["src/main.rs".to_string()],
                verify: vec!["test -f src/main.rs".to_string()],
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
    fn obligation_lint_rejects_package_step_missing_requested_port() {
        let mut plan = plan_with_paths("nextjs", vec!["package.json"]);
        plan.steps[0].instruction =
            "Create package.json with next build and next/react/react-dom dependencies."
                .to_string();

        let err = lint_step_plan_with_workspace_and_obligations(
            &plan,
            None,
            &[nextjs_obligation("nextjs_dev_port_required")],
        )
        .unwrap_err();

        assert_contract_violation(
            err,
            "step",
            "profile obligations require package.json work to mention nextjs_dev_port_required: requested port 3011",
            "nextjs_dev_port_required",
            &["3011"],
            &["3011"],
        );
    }

    #[test]
    fn obligation_lint_rejects_package_step_missing_tailwind_dependencies() {
        let mut plan = plan_with_paths("nextjs", vec!["package.json"]);
        plan.steps[0].instruction =
            "Create package.json with next build and next/react/react-dom dependencies."
                .to_string();

        let err = lint_step_plan_with_workspace_and_obligations(
            &plan,
            None,
            &[nextjs_obligation("nextjs_tailwind_dependencies_required")],
        )
        .unwrap_err();

        assert_contract_violation(
            err,
            "step",
            "profile obligations require package.json work to mention nextjs_tailwind_dependencies_required: tailwindcss, postcss, and autoprefixer",
            "nextjs_tailwind_dependencies_required",
            &["tailwindcss", "postcss", "autoprefixer"],
            &["tailwindcss", "postcss", "autoprefixer"],
        );
    }

    #[test]
    fn obligation_lint_rejects_tailwind_source_intent_without_setup_contract() {
        let plan = StepPlan {
            goal: "Create a Next.js game.".to_string(),
            profile: "nextjs".to_string(),
            style: "default".to_string(),
            intent: WorkIntent::New,
            required_artifacts: Vec::new(),
            steps: vec![
                StepPlanStep {
                    id: "setup-package-json".to_string(),
                    kind: StepKind::Setup,
                    instruction:
                        "Create package.json with next, react, and react-dom dependencies."
                            .to_string(),
                    expected_result: ExpectedResult::Pass,
                    expected_paths: vec!["package.json".to_string()],
                    verify: vec!["test -f package.json".to_string()],
                },
                StepPlanStep {
                    id: "create-global-styles".to_string(),
                    kind: StepKind::Create,
                    instruction:
                        "Create app/globals.css with base CSS styles or Tailwind CSS directives."
                            .to_string(),
                    expected_result: ExpectedResult::Pass,
                    expected_paths: vec!["app/globals.css".to_string()],
                    verify: vec!["test -f app/globals.css".to_string()],
                },
            ],
        };

        let err = lint_step_plan_with_workspace_and_obligations(&plan, None, &[]).unwrap_err();

        match err {
            PlanLintError::ContractViolation {
                step_id,
                reason,
                evidence,
            } => {
                assert_eq!(step_id, "create-global-styles");
                assert!(reason.contains("mentions Tailwind"));
                assert_eq!(
                    evidence.violated_contract.as_deref(),
                    Some("nextjs_tailwind_plan_contract")
                );
                assert_eq!(evidence.active_job.as_deref(), Some("manifest_repair"));
                assert_eq!(evidence.artifact_role.as_deref(), Some("manifest"));
                assert_eq!(
                    evidence.repair_kind.as_deref(),
                    Some("tailwind_contract_repair")
                );
                assert_eq!(
                    evidence.setup_implication.as_deref(),
                    Some("setup_after_manifest_repair_required")
                );
                assert_eq!(evidence.target_field.as_deref(), Some("steps"));
                assert_eq!(evidence.target_path.as_deref(), Some("package.json"));
                assert_eq!(
                    evidence.repair_target.as_deref(),
                    Some("step:setup-package-json:instruction")
                );
                assert_eq!(
                    evidence.missing_literals,
                    vec![
                        "tailwindcss".to_string(),
                        "postcss".to_string(),
                        "autoprefixer".to_string(),
                        "tailwind.config".to_string(),
                        "postcss.config".to_string(),
                    ]
                );
                assert!(
                    evidence
                        .required_action
                        .as_deref()
                        .unwrap()
                        .contains("manifest repair")
                );
                assert!(
                    evidence
                        .disallowed_actions
                        .iter()
                        .any(|action| action.contains("Do not rewrite source/gameplay"))
                );
            }
            other => panic!("expected tailwind plan contract violation, got {other:?}"),
        }
    }

    #[test]
    fn obligation_lint_accepts_tailwind_source_intent_with_setup_contract() {
        let plan = StepPlan {
            goal: "Create a Next.js game.".to_string(),
            profile: "nextjs".to_string(),
            style: "default".to_string(),
            intent: WorkIntent::New,
            required_artifacts: Vec::new(),
            steps: vec![
                StepPlanStep {
                    id: "setup-package-json".to_string(),
                    kind: StepKind::Setup,
                    instruction:
                        "Create package.json with next, react, react-dom, tailwindcss, postcss, and autoprefixer dependencies."
                            .to_string(),
                    expected_result: ExpectedResult::Pass,
                    expected_paths: vec!["package.json".to_string()],
                    verify: vec!["test -f package.json".to_string()],
                },
                StepPlanStep {
                    id: "setup-tailwind-config".to_string(),
                    kind: StepKind::Setup,
                    instruction: "Create tailwind.config.js and postcss.config.js.".to_string(),
                    expected_result: ExpectedResult::Pass,
                    expected_paths: vec![
                        "tailwind.config.js".to_string(),
                        "postcss.config.js".to_string(),
                    ],
                    verify: vec![
                        "test -f tailwind.config.js".to_string(),
                        "test -f postcss.config.js".to_string(),
                    ],
                },
                StepPlanStep {
                    id: "create-global-styles".to_string(),
                    kind: StepKind::Create,
                    instruction: "Create app/globals.css with Tailwind CSS directives.".to_string(),
                    expected_result: ExpectedResult::Pass,
                    expected_paths: vec!["app/globals.css".to_string()],
                    verify: vec!["test -f app/globals.css".to_string()],
                },
            ],
        };

        lint_step_plan_with_workspace_and_obligations(&plan, None, &[]).unwrap();
    }

    #[test]
    fn obligation_lint_rejects_typescript_plan_without_toolchain_contract() {
        let plan = StepPlan {
            goal: "Create a Next.js game.".to_string(),
            profile: "nextjs".to_string(),
            style: "default".to_string(),
            intent: WorkIntent::New,
            required_artifacts: Vec::new(),
            steps: vec![
                StepPlanStep {
                    id: "setup-package-json".to_string(),
                    kind: StepKind::Setup,
                    instruction:
                        "Create package.json with next, react, and react-dom dependencies."
                            .to_string(),
                    expected_result: ExpectedResult::Pass,
                    expected_paths: vec!["package.json".to_string()],
                    verify: vec!["test -f package.json".to_string()],
                },
                StepPlanStep {
                    id: "create-tsconfig".to_string(),
                    kind: StepKind::Setup,
                    instruction: "Create tsconfig.json for TypeScript.".to_string(),
                    expected_result: ExpectedResult::Pass,
                    expected_paths: vec!["tsconfig.json".to_string()],
                    verify: vec!["test -f tsconfig.json".to_string()],
                },
            ],
        };

        let err = lint_step_plan_with_workspace_and_obligations(&plan, None, &[]).unwrap_err();

        match err {
            PlanLintError::ContractViolation {
                step_id,
                reason,
                evidence,
            } => {
                assert_eq!(step_id, "create-tsconfig");
                assert!(reason.contains("TypeScript toolchain"));
                assert_eq!(
                    evidence.violated_contract.as_deref(),
                    Some("nextjs_typescript_toolchain_plan_contract")
                );
                assert_eq!(
                    evidence.missing_literals,
                    vec![
                        "typescript".to_string(),
                        "@types/react".to_string(),
                        "18".to_string(),
                        "5.x or ^5.".to_string(),
                    ]
                );
            }
            other => panic!("expected typescript plan contract violation, got {other:?}"),
        }
    }

    #[test]
    fn obligation_lint_accepts_typescript_plan_with_toolchain_contract() {
        let plan = StepPlan {
            goal: "Create a Next.js game.".to_string(),
            profile: "nextjs".to_string(),
            style: "default".to_string(),
            intent: WorkIntent::New,
            required_artifacts: Vec::new(),
            steps: vec![
                StepPlanStep {
                    id: "setup-package-json".to_string(),
                    kind: StepKind::Setup,
                    instruction: "Create package.json with next, react, react-dom, typescript ^5.4.0 compatibility, and @types/react 18.x compatibility."
                        .to_string(),
                    expected_result: ExpectedResult::Pass,
                    expected_paths: vec!["package.json".to_string()],
                    verify: vec!["test -f package.json".to_string()],
                },
                StepPlanStep {
                    id: "create-page".to_string(),
                    kind: StepKind::Create,
                    instruction: "Create app/page.tsx.".to_string(),
                    expected_result: ExpectedResult::Pass,
                    expected_paths: vec!["app/page.tsx".to_string()],
                    verify: vec!["test -f app/page.tsx".to_string()],
                },
            ],
        };

        lint_step_plan_with_workspace_and_obligations(&plan, None, &[]).unwrap();
    }

    #[test]
    fn obligation_lint_rejects_ambiguous_typescript_major_without_5x_contract() {
        let plan = StepPlan {
            goal: "Create a Next.js game.".to_string(),
            profile: "nextjs".to_string(),
            style: "default".to_string(),
            intent: WorkIntent::New,
            required_artifacts: Vec::new(),
            steps: vec![
                StepPlanStep {
                    id: "setup-package-json".to_string(),
                    kind: StepKind::Setup,
                    instruction: "Create package.json with next, react, react-dom, typescript, 5, @types/react, and 18."
                        .to_string(),
                    expected_result: ExpectedResult::Pass,
                    expected_paths: vec!["package.json".to_string()],
                    verify: vec!["test -f package.json".to_string()],
                },
                StepPlanStep {
                    id: "create-page".to_string(),
                    kind: StepKind::Create,
                    instruction: "Create app/page.tsx.".to_string(),
                    expected_result: ExpectedResult::Pass,
                    expected_paths: vec!["app/page.tsx".to_string()],
                    verify: vec!["test -f app/page.tsx".to_string()],
                },
            ],
        };

        let err = lint_step_plan_with_workspace_and_obligations(&plan, None, &[]).unwrap_err();

        match err {
            PlanLintError::ContractViolation {
                step_id, evidence, ..
            } => {
                assert_eq!(step_id, "setup-package-json");
                assert_eq!(
                    evidence.violated_contract.as_deref(),
                    Some("nextjs_typescript_toolchain_plan_contract")
                );
                assert_eq!(evidence.missing_literals, vec!["5.x or ^5.".to_string()]);
            }
            other => panic!("expected typescript plan contract violation, got {other:?}"),
        }
    }

    #[test]
    fn nextjs_dependency_obligation_reports_missing_react_dom() {
        let mut plan = plan_with_paths("nextjs", vec!["package.json"]);
        plan.steps[0].id = "create-package-json".to_string();
        plan.steps[0].instruction =
            "Create package.json with scripts.build as next build and dependencies next and react."
                .to_string();

        let err = lint_step_plan_with_workspace_and_obligations(
            &plan,
            None,
            &[nextjs_obligation("nextjs_dependencies_required")],
        )
        .unwrap_err();

        assert_contract_violation(
            err,
            "create-package-json",
            "profile obligations require package.json work to mention nextjs_dependencies_required: next, react, react-dom, and React 18.2+ compatibility",
            "nextjs_dependencies_required",
            &["next", "react", "react-dom", "18.2"],
            &["react-dom", "18.2"],
        );
    }

    #[test]
    fn obligation_lint_accepts_package_step_with_profile_contract_literals() {
        let mut plan = plan_with_paths("nextjs", vec!["package.json"]);
        plan.steps[0].instruction =
            "Create package.json with scripts.dev as next dev -p 3011, scripts.build as next build, and dependencies next, react, react-dom with React 18.2 compatibility, tailwindcss, postcss, and autoprefixer."
                .to_string();

        lint_step_plan_with_workspace_and_obligations(
            &plan,
            None,
            &[
                nextjs_obligation("nextjs_dev_port_required"),
                nextjs_obligation("nextjs_build_script_required"),
                nextjs_obligation("nextjs_dependencies_required"),
                nextjs_obligation("nextjs_tailwind_dependencies_required"),
            ],
        )
        .unwrap();
    }

    #[test]
    fn obligation_lint_accepts_contract_literals_split_across_package_steps() {
        let plan = StepPlan {
            goal: "Create Next.js app".to_string(),
            profile: "nextjs".to_string(),
            style: "default".to_string(),
            intent: WorkIntent::New,
            required_artifacts: Vec::new(),
            steps: vec![
                StepPlanStep {
                    id: "setup-project".to_string(),
                    kind: StepKind::Setup,
                    instruction: "Create package.json with scripts.build as next build and dependencies next, react, and react-dom with React 18.2 compatibility."
                        .to_string(),
                    expected_result: ExpectedResult::Pass,
                    expected_paths: vec!["package.json".to_string()],
                    verify: vec!["test -f package.json".to_string()],
                },
                StepPlanStep {
                    id: "configure-port".to_string(),
                    kind: StepKind::Edit,
                    instruction: "Update package.json scripts.dev to next dev -p 3011."
                        .to_string(),
                    expected_result: ExpectedResult::Pass,
                    expected_paths: vec!["package.json".to_string()],
                    verify: vec!["test -f package.json".to_string()],
                },
            ],
        };

        lint_step_plan_with_workspace_and_obligations(
            &plan,
            None,
            &[
                nextjs_obligation("nextjs_dev_port_required"),
                nextjs_obligation("nextjs_build_script_required"),
                nextjs_obligation("nextjs_dependencies_required"),
            ],
        )
        .unwrap();
    }

    #[test]
    fn obligation_lint_accepts_existing_manifest_facts_that_satisfy_obligations() {
        let root = temp_workspace("nextjs-existing-package-obligation-satisfied");
        fs::write(
            root.join("package.json"),
            r#"{"scripts":{"dev":"next dev -p 3011","build":"next build"},"dependencies":{"next":"14.0.0","react":"^18.2.0","react-dom":"^18.2.0"}}"#,
        )
        .unwrap();
        let plan = StepPlan {
            goal: "Update package metadata".to_string(),
            profile: "nextjs".to_string(),
            style: "default".to_string(),
            intent: WorkIntent::New,
            required_artifacts: Vec::new(),
            steps: vec![StepPlanStep {
                id: "update-package-metadata".to_string(),
                kind: StepKind::Edit,
                instruction: "Update package.json metadata and keep the existing scripts and dependencies unchanged.".to_string(),
                expected_result: ExpectedResult::Pass,
                expected_paths: vec!["package.json".to_string()],
                verify: Vec::new(),
            }],
        };

        lint_step_plan_with_workspace_and_obligations(
            &plan,
            Some(&root),
            &[
                nextjs_obligation("nextjs_dev_port_required"),
                nextjs_obligation("nextjs_build_script_required"),
                nextjs_obligation("nextjs_dependencies_required"),
            ],
        )
        .unwrap();
    }

    #[test]
    fn obligation_lint_requires_dev_port_for_unsatisfied_existing_package_edits() {
        let root = temp_workspace("nextjs-existing-package-obligation-missing-port");
        fs::write(
            root.join("package.json"),
            r#"{"scripts":{"dev":"next dev","build":"next build"},"dependencies":{"next":"14.0.0","react":"^18.2.0","react-dom":"^18.2.0"}}"#,
        )
        .unwrap();
        let plan = StepPlan {
            goal: "Update package metadata".to_string(),
            profile: "nextjs".to_string(),
            style: "default".to_string(),
            intent: WorkIntent::New,
            required_artifacts: Vec::new(),
            steps: vec![StepPlanStep {
                id: "update-package-metadata".to_string(),
                kind: StepKind::Edit,
                instruction: "Update package.json metadata and keep the existing scripts and dependencies unchanged.".to_string(),
                expected_result: ExpectedResult::Pass,
                expected_paths: vec!["package.json".to_string()],
                verify: Vec::new(),
            }],
        };

        let err = lint_step_plan_with_workspace_and_obligations(
            &plan,
            Some(&root),
            &[
                nextjs_obligation("nextjs_dev_port_required"),
                nextjs_obligation("nextjs_build_script_required"),
                nextjs_obligation("nextjs_dependencies_required"),
            ],
        )
        .unwrap_err();

        assert_contract_violation(
            err,
            "update-package-metadata",
            "profile obligations require package.json work to mention nextjs_dev_port_required: requested port 3011",
            "nextjs_dev_port_required",
            &["3011"],
            &["3011"],
        );
    }

    #[test]
    fn obligation_lint_rejects_nextjs_source_step_missing_selected_route() {
        let mut plan = plan_with_paths("nextjs", vec!["app/hooks/useGame.ts"]);
        plan.steps[0].instruction = "Create app/hooks/useGame.ts.".to_string();

        let err = lint_step_plan_with_workspace_and_obligations(
            &plan,
            None,
            &[nextjs_route_obligation(
                "app/page.tsx",
                "app/hooks/useGame.ts",
            )],
        )
        .unwrap_err();

        assert_route_contract_violation(err, "step", "app/page.tsx", "app/hooks/useGame.ts");
    }

    #[test]
    fn nextjs_route_obligation_reports_missing_selected_route() {
        let mut plan = plan_with_paths("nextjs", vec!["app/hooks/useGame.ts"]);
        plan.steps[0].id = "create-game-hook".to_string();
        plan.steps[0].instruction = "Create app/hooks/useGame.ts.".to_string();

        let err = lint_step_plan_with_workspace_and_obligations(
            &plan,
            None,
            &[nextjs_route_obligation(
                "app/page.tsx",
                "app/hooks/useGame.ts",
            )],
        )
        .unwrap_err();

        assert_route_contract_violation(
            err,
            "create-game-hook",
            "app/page.tsx",
            "app/hooks/useGame.ts",
        );
    }

    #[test]
    fn obligation_lint_accepts_nextjs_source_step_with_selected_route_expected_path() {
        let mut plan = plan_with_paths("nextjs", vec!["app/hooks/useGame.ts", "app/page.tsx"]);
        plan.steps[0].instruction = "Create app/hooks/useGame.ts.".to_string();

        lint_step_plan_with_workspace_and_obligations(
            &plan,
            None,
            &[nextjs_route_obligation(
                "app/page.tsx",
                "app/hooks/useGame.ts",
            )],
        )
        .unwrap();
    }

    #[test]
    fn obligation_lint_accepts_nextjs_source_step_with_selected_route_instruction() {
        let mut plan = plan_with_paths("nextjs", vec!["app/hooks/useGame.ts"]);
        plan.steps[0].instruction =
            "Create app/hooks/useGame.ts and wire it from app/page.tsx.".to_string();

        lint_step_plan_with_workspace_and_obligations(
            &plan,
            None,
            &[nextjs_route_obligation(
                "app/page.tsx",
                "app/hooks/useGame.ts",
            )],
        )
        .unwrap();
    }

    #[test]
    fn obligation_lint_accepts_later_route_integration_step() {
        let mut plan = plan_with_paths("nextjs", vec!["app/components/GameCanvas.tsx"]);
        plan.steps[0].id = "create-game-canvas-component".to_string();
        plan.steps[0].instruction = "Create the GameCanvas component.".to_string();
        plan.steps.push(StepPlanStep {
            id: "update-page-entry".to_string(),
            kind: StepKind::Edit,
            instruction: "Edit app/page.tsx to render the GameCanvas component.".to_string(),
            expected_result: ExpectedResult::Pass,
            expected_paths: vec!["app/page.tsx".to_string()],
            verify: Vec::new(),
        });

        lint_step_plan_with_workspace_and_obligations(
            &plan,
            None,
            &[nextjs_route_obligation(
                "app/page.tsx",
                "app/components/GameCanvas.tsx",
            )],
        )
        .unwrap();
    }

    #[test]
    fn obligation_lint_uses_workspace_selected_route_when_obligation_is_absent() {
        let root = temp_workspace("nextjs-route-workspace-obligation");
        fs::create_dir_all(root.join("app")).unwrap();
        fs::write(
            root.join("app/page.tsx"),
            "export default function Page() {}",
        )
        .unwrap();
        let mut plan = plan_with_paths("nextjs", vec!["app/hooks/useGame.ts"]);
        plan.steps[0].instruction = "Create app/hooks/useGame.ts.".to_string();

        let err =
            lint_step_plan_with_workspace_and_obligations(&plan, Some(&root), &[]).unwrap_err();

        assert_route_contract_violation(err, "step", "app/page.tsx", "app/hooks/useGame.ts");
    }

    #[test]
    fn obligation_lint_does_not_treat_nextjs_config_as_route_integration_source() {
        let root = temp_workspace("nextjs-route-config");
        fs::create_dir_all(root.join("app")).unwrap();
        fs::write(
            root.join("app/page.tsx"),
            "export default function Page() {}",
        )
        .unwrap();
        let mut plan = plan_with_paths("nextjs", vec!["tailwind.config.js"]);
        plan.steps[0].instruction = "Create tailwind.config.js.".to_string();

        lint_step_plan_with_workspace_and_obligations(&plan, Some(&root), &[]).unwrap();
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
    fn workspace_lint_accepts_existing_paths_in_verify_step() {
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
                id: "verify-build".to_string(),
                kind: StepKind::Verify,
                instruction: "Verify npm run build.".to_string(),
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
    fn rejects_nextjs_create_next_app_scaffolding() {
        let plan = StepPlan {
            goal: "create next app".to_string(),
            profile: "nextjs".to_string(),
            style: "default".to_string(),
            intent: WorkIntent::New,
            required_artifacts: Vec::new(),
            steps: vec![StepPlanStep {
                id: "setup-project".to_string(),
                kind: StepKind::Setup,
                instruction:
                    "Initialize a new Next.js project using create-next-app with TypeScript."
                        .to_string(),
                expected_result: ExpectedResult::Pass,
                expected_paths: vec!["package.json".to_string(), "app/page.tsx".to_string()],
                verify: vec!["test -f package.json".to_string()],
            }],
        };

        let err = lint_step_plan(&plan).unwrap_err();

        assert_eq!(
            err,
            PlanLintError::ShellScaffold {
                step_id: "setup-project".to_string(),
                command: "create-next-app".to_string(),
                guidance: "create package.json and app/page.tsx with Write/Edit".to_string()
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
    fn rejects_nextjs_root_drift_from_app_to_src_app() {
        let root = temp_workspace("nextjs-app-drift");
        fs::create_dir_all(root.join("app")).unwrap();
        fs::write(
            root.join("app/page.tsx"),
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
                instruction: "Create src/app/page.tsx.".to_string(),
                expected_result: ExpectedResult::Pass,
                expected_paths: vec!["src/app/page.tsx".to_string()],
                verify: vec!["test -f src/app/page.tsx".to_string()],
            }],
        };

        let err = lint_step_plan_with_workspace(&plan, Some(&root)).unwrap_err();

        assert_eq!(
            err,
            PlanLintError::InvalidStepInstruction {
                step_id: "create-page".to_string(),
                reason: "Next.js workspace already uses app; creating src/app/page.tsx would split the app root unless this is an explicit migration"
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

    fn nextjs_obligation(code: &str) -> ProfileObligation {
        ProfileObligation {
            code: code.to_string(),
            message: code.to_string(),
            paths: vec!["package.json".to_string()],
            expected: None,
        }
    }

    fn nextjs_route_obligation(route: &str, artifact: &str) -> ProfileObligation {
        ProfileObligation {
            code: "nextjs_route_integration_required".to_string(),
            message:
                "selected Next.js route must import or reference explicit UI/game source artifacts"
                    .to_string(),
            paths: vec![route.to_string(), artifact.to_string()],
            expected: Some(format!(
                "selected route `{route}` references `{artifact}` or its module name"
            )),
        }
    }

    fn assert_contract_violation(
        err: PlanLintError,
        expected_step: &str,
        expected_reason: &str,
        expected_contract: &str,
        expected_required_literals: &[&str],
        expected_missing_literals: &[&str],
    ) {
        match err {
            PlanLintError::ContractViolation {
                step_id,
                reason,
                evidence,
            } => {
                assert_eq!(step_id, expected_step);
                assert_eq!(reason, expected_reason);
                assert_eq!(evidence.failed_step.as_deref(), Some(expected_step));
                assert_eq!(
                    evidence.violated_contract.as_deref(),
                    Some(expected_contract)
                );
                assert_eq!(evidence.target_field.as_deref(), Some("instruction"));
                assert_eq!(
                    evidence.required_literals,
                    expected_required_literals
                        .iter()
                        .map(|value| value.to_string())
                        .collect::<Vec<_>>()
                );
                assert_eq!(
                    evidence.missing_literals,
                    expected_missing_literals
                        .iter()
                        .map(|value| value.to_string())
                        .collect::<Vec<_>>()
                );
                let rendered = evidence.render().unwrap();
                assert!(rendered.contains(expected_contract), "{rendered}");
            }
            other => panic!("expected contract violation, got {other:?}"),
        }
    }

    fn assert_route_contract_violation(
        err: PlanLintError,
        expected_step: &str,
        expected_route: &str,
        expected_artifact: &str,
    ) {
        match err {
            PlanLintError::ContractViolation {
                step_id,
                reason,
                evidence,
            } => {
                assert_eq!(step_id, expected_step);
                assert_eq!(
                    reason,
                    format!(
                        "profile obligations require Next.js route integration: step creates or edits {expected_artifact} but does not mention selected route {expected_route} in instruction or expected_paths"
                    )
                );
                assert_eq!(
                    evidence.violated_contract.as_deref(),
                    Some("nextjs_route_integration_required")
                );
                assert_eq!(
                    evidence.target_field.as_deref(),
                    Some("instruction_or_expected_paths")
                );
                assert_eq!(evidence.required_paths, vec![expected_route.to_string()]);
                assert_eq!(evidence.missing_paths, vec![expected_route.to_string()]);
                assert_eq!(evidence.rejected_value.as_deref(), Some(expected_artifact));
            }
            other => panic!("expected route contract violation, got {other:?}"),
        }
    }

    fn assert_setup_ownership_violation(
        err: PlanLintError,
        expected_step: &str,
        expected_path: &str,
        expected_role: &str,
    ) {
        match err {
            PlanLintError::ContractViolation {
                step_id,
                reason,
                evidence,
            } => {
                assert_eq!(step_id, expected_step);
                assert!(reason.contains("setup step cannot own"), "{reason}");
                assert!(reason.contains(expected_path), "{reason}");
                assert_eq!(evidence.failed_step.as_deref(), Some(expected_step));
                assert_eq!(
                    evidence.violated_contract.as_deref(),
                    Some("step_kind_artifact_role")
                );
                assert_eq!(
                    evidence.reason_code.as_deref(),
                    Some("setup_step_owns_non_setup_artifact")
                );
                assert_eq!(evidence.target_field.as_deref(), Some("expected_paths"));
                assert_eq!(evidence.target_path.as_deref(), Some(expected_path));
                assert_eq!(evidence.rejected_value.as_deref(), Some(expected_path));
                assert!(
                    evidence
                        .observed_expected_pairs
                        .iter()
                        .any(|value| value.contains(expected_role)),
                    "{evidence:?}"
                );
                assert_eq!(
                    evidence.required_literals,
                    vec!["setup/manifest".to_string(), "setup/config".to_string()]
                );
                let rendered = evidence.render().unwrap();
                assert!(rendered.contains(expected_path), "{rendered}");
                assert!(rendered.contains(expected_role), "{rendered}");
                assert!(
                    rendered.contains("change this step kind to create/edit"),
                    "{rendered}"
                );
            }
            other => panic!("expected step ownership contract violation, got {other:?}"),
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
