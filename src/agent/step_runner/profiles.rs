use crate::agent::step_runner::correction_evidence::PlanCorrectionEvidence;
use crate::agent::step_runner::mechanical_repair::mechanical_adapter_family_specs;
use crate::agent::step_runner::plan_lint::PlanLintError;
use crate::agent::step_runner::profile_artifact::{
    ArtifactKind, ArtifactProvenance, artifact_kind_label, classify_profile_artifact,
};
use crate::agent::step_runner::{StepKind, StepPlan, StepPlanStep};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

const NEXTJS_ROUTE_GRAPH_MAX_DEPTH: usize = 3;
const NEXTJS_ROUTE_GRAPH_MAX_FILES: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileId {
    Generic,
    NextJs,
    Python,
    Rust,
    Investigation,
    Docs,
    DataAnalysis,
    DataPipeline,
}

impl ProfileId {
    pub fn parse(value: &str) -> Result<Self, ProfileError> {
        match value.trim().to_ascii_lowercase().as_str() {
            "generic" => Ok(Self::Generic),
            "nextjs" | "next.js" => Ok(Self::NextJs),
            "python" => Ok(Self::Python),
            "rust" => Ok(Self::Rust),
            "investigation" => Ok(Self::Investigation),
            "docs" | "documentation" => Ok(Self::Docs),
            "data-analysis" | "data_analysis" => Ok(Self::DataAnalysis),
            "data-pipeline" | "data_pipeline" => Ok(Self::DataPipeline),
            other => Err(ProfileError::UnknownProfile(other.to_string())),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Generic => "generic",
            Self::NextJs => "nextjs",
            Self::Python => "python",
            Self::Rust => "rust",
            Self::Investigation => "investigation",
            Self::Docs => "docs",
            Self::DataAnalysis => "data-analysis",
            Self::DataPipeline => "data-pipeline",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileContract {
    pub id: ProfileId,
    pub text: String,
    pub verifier_commands: Vec<String>,
    pub protected_path_prefixes: Vec<String>,
}

pub fn profile_contract(id: ProfileId) -> ProfileContract {
    match id {
        ProfileId::Generic => ProfileContract {
            id,
            text: "Keep changes scoped. Prefer Read/Bash inspection before editing. Use Write/Edit for file changes. End with deterministic checks when practical.".to_string(),
            verifier_commands: Vec::new(),
            protected_path_prefixes: Vec::new(),
        },
        ProfileId::NextJs => ProfileContract {
            id,
            text: "For Next.js work, preserve honest build scripts. New apps need package.json with next/react/react-dom dependencies, app/page.tsx or pages/index.tsx, and a build script that remains `next build`. If TypeScript or .tsx files are used, use a stable TypeScript 5.x range such as ^5.4.0 with @types/react 18.x. If source imports use @/*, create tsconfig.json with compilerOptions.paths for @/*; otherwise use relative imports. If node_modules/.bin/next is missing, install dependencies when allowed or report dependency_missing; never fake build success.".to_string(),
            verifier_commands: vec!["npm run build".to_string()],
            protected_path_prefixes: Vec::new(),
        },
        ProfileId::Python => ProfileContract {
            id,
            text: "For Python work, keep modules importable, prefer small functions, and verify with pytest or direct local script execution when tests are not present.".to_string(),
            verifier_commands: vec!["python -m pytest".to_string()],
            protected_path_prefixes: Vec::new(),
        },
        ProfileId::Rust => ProfileContract {
            id,
            text: "For Rust work, keep Cargo.toml honest, use idiomatic modules, and verify with cargo test, cargo build, or cargo run when requested. For new minimal projects, create Cargo.toml and src/main.rs with Write/Edit instead of cargo init or cargo new. If integration tests use CARGO_BIN_EXE_<name>, ensure <name> matches the Cargo binary name defined by Cargo.toml; tests must reference actual package, binary, module, and public item names defined in the project.".to_string(),
            verifier_commands: vec!["cargo test".to_string()],
            protected_path_prefixes: Vec::new(),
        },
        ProfileId::Investigation => ProfileContract {
            id,
            text: "For investigation work, inspect first, preserve evidence, and avoid code changes unless the requested task explicitly asks for a fix.".to_string(),
            verifier_commands: Vec::new(),
            protected_path_prefixes: Vec::new(),
        },
        ProfileId::Docs => ProfileContract {
            id,
            text: "For documentation work, keep claims tied to repository facts, update indexes when present, and avoid changing behavior code.".to_string(),
            verifier_commands: Vec::new(),
            protected_path_prefixes: Vec::new(),
        },
        ProfileId::DataAnalysis => ProfileContract {
            id,
            text: "For data analysis work, keep raw inputs immutable, write derived artifacts separately, and record assumptions and reproducible commands.".to_string(),
            verifier_commands: Vec::new(),
            protected_path_prefixes: data_protected_prefixes(),
        },
        ProfileId::DataPipeline => ProfileContract {
            id,
            text: "For data pipeline work, keep raw inputs immutable, separate extraction, transformation, and output steps, and make reruns deterministic.".to_string(),
            verifier_commands: Vec::new(),
            protected_path_prefixes: data_protected_prefixes(),
        },
    }
}

pub fn profile_contract_text(profile: &str) -> Result<String, ProfileError> {
    Ok(profile_contract(ProfileId::parse(profile)?).text)
}

pub fn profile_verifier_commands(profile: &str) -> Result<Vec<String>, ProfileError> {
    Ok(profile_contract(ProfileId::parse(profile)?).verifier_commands)
}

pub fn protected_by_profile(profile: &str, path: &str) -> Result<bool, ProfileError> {
    let contract = profile_contract(ProfileId::parse(profile)?);
    Ok(contract
        .protected_path_prefixes
        .iter()
        .any(|prefix| path == prefix || path.starts_with(&format!("{prefix}/"))))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileFactSummary {
    pub lines: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProfileCapabilityFamily {
    ProjectKind,
    RootHints,
    ManifestContract,
    EntrypointContract,
    IntegrationContract,
    SetupContract,
    VerifierContract,
    CompletionEvidenceContract,
    ProtectedInputContract,
    ScaffoldContract,
    ProfileFailureMapping,
    LanguageAdapterFamily,
}

impl ProfileCapabilityFamily {
    fn eval_key(self) -> &'static str {
        match self {
            Self::ProjectKind => "project",
            Self::RootHints => "roots",
            Self::ManifestContract => "manifest",
            Self::EntrypointContract => "entrypoint",
            Self::IntegrationContract => "integration",
            Self::SetupContract => "setup",
            Self::VerifierContract => "verifier",
            Self::CompletionEvidenceContract => "evidence",
            Self::ProtectedInputContract => "protected",
            Self::ScaffoldContract => "scaffold",
            Self::ProfileFailureMapping => "failure",
            Self::LanguageAdapterFamily => "adapter",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProfileCapabilityStatus {
    Supported,
    Partial,
    NotApplicable,
}

impl ProfileCapabilityStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Supported => "supported",
            Self::Partial => "partial",
            Self::NotApplicable => "not_applicable",
        }
    }

    fn summary_str(self) -> &'static str {
        match self {
            Self::Supported => "ok",
            Self::Partial => "partial",
            Self::NotApplicable => "na",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProfileCapability {
    pub(crate) family: ProfileCapabilityFamily,
    pub(crate) status: ProfileCapabilityStatus,
    pub(crate) source_of_truth: String,
    pub(crate) artifacts: Vec<String>,
    pub(crate) recovery_owner_hint: Option<String>,
    pub(crate) authority: Option<String>,
    pub(crate) reason: Option<String>,
}

impl ProfileCapability {
    fn render_line(&self) -> String {
        format!(
            "profile.output.capability.{}=status:{} artifacts:{} owner:{} authority:{} reason:{}",
            self.family.eval_key(),
            self.status.as_str(),
            join_profile_values_limited(&self.artifacts, 1),
            bounded_value(self.recovery_owner_hint.as_deref().unwrap_or("none")),
            bounded_value(self.authority.as_deref().unwrap_or("none")),
            bounded_value(self.reason.as_deref().unwrap_or("none"))
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProfileOutput {
    pub(crate) id: ProfileId,
    pub(crate) project_kind: String,
    pub(crate) project_root_hints: Vec<String>,
    pub(crate) manifest_artifacts: Vec<String>,
    pub(crate) entrypoints: Vec<String>,
    pub(crate) integration_artifacts: Vec<String>,
    pub(crate) completion_evidence_requirements: Vec<String>,
    pub(crate) failure_mappings: Vec<String>,
    pub(crate) adapter_families: Vec<String>,
    pub(crate) capabilities: Vec<ProfileCapability>,
    pub(crate) artifact_classifications: Vec<String>,
    pub(crate) setup_artifacts: Vec<String>,
    pub(crate) scaffold_artifacts: Vec<String>,
    pub(crate) route_integration_artifacts: Vec<String>,
    pub(crate) verifier_commands: Vec<String>,
    pub(crate) protected_paths: Vec<String>,
    pub(crate) behavior_obligations: Vec<String>,
    pub(crate) verification_failures: Vec<String>,
    pub(crate) recovery_candidate_hints: Vec<String>,
}

impl ProfileOutput {
    pub(crate) fn render_lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!("profile.output.id={}", self.id.as_str()),
            format!("profile.output.project_kind={}", self.project_kind),
            format!(
                "profile.output.project_roots={}",
                join_profile_values(&self.project_root_hints)
            ),
            format!(
                "profile.output.manifests={}",
                join_profile_values(&self.manifest_artifacts)
            ),
            format!(
                "profile.output.entrypoints={}",
                join_profile_values(&self.entrypoints)
            ),
            format!(
                "profile.output.integration_artifacts={}",
                join_profile_values(&self.integration_artifacts)
            ),
            format!(
                "profile.output.artifacts={}",
                join_profile_values(&self.artifact_classifications)
            ),
            format!(
                "profile.output.setup_artifacts={}",
                join_profile_values(&self.setup_artifacts)
            ),
            format!(
                "profile.output.scaffold_artifacts={}",
                join_profile_values(&self.scaffold_artifacts)
            ),
            format!(
                "profile.output.route_artifacts={}",
                join_profile_values(&self.route_integration_artifacts)
            ),
            format!(
                "profile.output.verifiers={}",
                join_profile_values(&self.verifier_commands)
            ),
            format!(
                "profile.output.protected_paths={}",
                join_profile_values(&self.protected_paths)
            ),
            format!(
                "profile.output.behavior_obligations={}",
                join_profile_values(&self.behavior_obligations)
            ),
            format!(
                "profile.output.completion_evidence={}",
                join_profile_values(&self.completion_evidence_requirements)
            ),
            format!(
                "profile.output.verification_failures={}",
                join_profile_values(&self.verification_failures)
            ),
            format!(
                "profile.output.failure_mappings={}",
                join_profile_values(&self.failure_mappings)
            ),
            format!(
                "profile.output.adapter_families={}",
                join_profile_values(&self.adapter_families)
            ),
            format!(
                "profile.output.recovery_candidate_hints={}",
                join_profile_values(&self.recovery_candidate_hints)
            ),
            format!("profile_project_kind={}", self.project_kind),
            format!(
                "profile_manifest_artifacts={}",
                join_profile_values(&self.manifest_artifacts)
            ),
            format!(
                "profile_entrypoints={}",
                join_profile_values(&self.entrypoints)
            ),
            format!(
                "profile_integration_artifacts={}",
                join_profile_values(&self.integration_artifacts)
            ),
            format!(
                "profile_completion_evidence={}",
                join_profile_values(&self.completion_evidence_requirements)
            ),
            format!(
                "profile_failure_mapping={}",
                join_profile_values(&self.failure_mappings)
            ),
            format!(
                "profile_adapter_families={}",
                join_profile_values(&self.adapter_families)
            ),
            format!(
                "profile_capability_status={}",
                profile_capability_status_summary(&self.capabilities)
            ),
        ];
        lines.extend(self.capabilities.iter().map(ProfileCapability::render_line));
        lines.retain(|line| line.len() <= 240);
        lines
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileVerificationContext {
    pub goal_excerpt: String,
    pub required_artifacts: Vec<String>,
    pub expected_paths: Vec<String>,
    pub phase_contract_facts: Vec<String>,
    pub profile_facts: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileVerificationFailure {
    pub code: String,
    pub message: String,
    pub paths: Vec<String>,
}

impl ProfileVerificationFailure {
    fn new(code: &str, message: impl Into<String>, paths: Vec<String>) -> Self {
        Self {
            code: code.to_string(),
            message: message.into(),
            paths,
        }
    }

    pub fn render(&self) -> String {
        if self.paths.is_empty() {
            format!("{}: {}", self.code, self.message)
        } else {
            format!(
                "{}: {} ({})",
                self.code,
                self.message,
                self.paths.join(", ")
            )
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileObligationContext {
    pub goal_excerpt: String,
    pub required_artifacts: Vec<String>,
    pub phase_contract_facts: Vec<String>,
    pub profile_facts: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileObligation {
    pub code: String,
    pub message: String,
    pub paths: Vec<String>,
    pub expected: Option<String>,
}

impl ProfileObligation {
    fn new(
        code: &str,
        message: impl Into<String>,
        paths: Vec<String>,
        expected: Option<String>,
    ) -> Self {
        Self {
            code: code.to_string(),
            message: message.into(),
            paths,
            expected,
        }
    }

    pub fn render(&self) -> String {
        let mut out = if self.paths.is_empty() {
            format!("{}: {}", self.code, self.message)
        } else {
            format!(
                "{}: {} ({})",
                self.code,
                self.message,
                self.paths.join(", ")
            )
        };
        if let Some(expected) = &self.expected {
            out.push_str(&format!(" expected={expected}"));
        }
        out
    }
}

pub fn profile_fact_summary(profile: &str, cwd: &Path) -> Result<ProfileFactSummary, ProfileError> {
    let id = ProfileId::parse(profile)?;
    let mut lines = profile_output_summary(id, cwd).render_lines();
    match id {
        ProfileId::NextJs => {
            lines.extend(nextjs_fact_summary(cwd).lines);
            Ok(ProfileFactSummary { lines })
        }
        _ => Ok(ProfileFactSummary { lines }),
    }
}

pub fn verify_profile(
    profile: &str,
    cwd: &Path,
    context: &ProfileVerificationContext,
) -> Result<Vec<ProfileVerificationFailure>, ProfileError> {
    match ProfileId::parse(profile)? {
        ProfileId::NextJs => Ok(verify_nextjs_profile(cwd, context)),
        _ => Ok(Vec::new()),
    }
}

pub fn profile_obligations(
    profile: &str,
    context: &ProfileObligationContext,
) -> Result<Vec<ProfileObligation>, ProfileError> {
    match ProfileId::parse(profile)? {
        ProfileId::NextJs => Ok(nextjs_profile_obligations(context)),
        _ => Ok(Vec::new()),
    }
}

pub fn profile_plan_guidance(profile: &str) -> &'static str {
    match ProfileId::parse(profile).unwrap_or(ProfileId::Generic) {
        ProfileId::NextJs => {
            "For Next.js apps, generated package.json steps must instruct a compatible dependency family: next plus react/react-dom with React 18.2 or newer compatibility. If the plan creates tsconfig.json, .ts, .tsx, or TypeScript code, the package.json step must also literally include typescript 5.x compatibility, a stable TypeScript 5.x range such as ^5.4.0, and @types/react 18.x compatibility. Do not use exact React pins below 18.2 with Next.js 14, TypeScript 6 with Next.js 14, @types/react 19 with React 18, exact TypeScript pins such as 5.0.0, or latest as the compatibility strategy. If source imports use @/*, the same plan must create or edit tsconfig.json with compilerOptions.paths mapping @/* to the selected source root; otherwise use relative imports. Use plain CSS unless the goal or phase explicitly requires Tailwind. If any source/style step mentions Tailwind, @tailwind, or Tailwind directives, the same step plan must also include exact package.json dependency literals tailwindcss, postcss, and autoprefixer, plus setup/config outputs tailwind.config.js and postcss.config.js. Do not write only Tailwind CSS dependencies as a substitute for the exact package names. For Next.js source verification, use npm run build in a separate verify step; do not use npx tsc --noEmit or other npx verifiers because npx may perform dependency setup and is blocked. Do not plan npm install; verifier-owned setup handles dependency installation when approved."
        }
        ProfileId::Rust => {
            "For new Rust projects, plan explicit file creation for Cargo.toml and src/main.rs. Do not plan cargo init or cargo new shell scaffolding."
        }
        _ => "No additional profile-specific plan guidance.",
    }
}

pub fn lint_profile_plan(
    plan: &StepPlan,
    cwd: Option<&Path>,
    obligations: &[ProfileObligation],
) -> Result<(), PlanLintError> {
    match ProfileId::parse(plan.profile.as_str()).unwrap_or(ProfileId::Generic) {
        ProfileId::NextJs => lint_nextjs_plan(plan, cwd, obligations),
        ProfileId::Rust => Ok(()),
        _ => Ok(()),
    }
}

pub fn lint_profile_step_contract(
    profile: &str,
    step_id: &str,
    kind: StepKind,
    instruction: &str,
    expected_paths: &[String],
    cwd: Option<&Path>,
) -> Result<(), PlanLintError> {
    match ProfileId::parse(profile).unwrap_or(ProfileId::Generic) {
        ProfileId::NextJs => {
            lint_nextjs_scaffolding_and_root_drift(step_id, kind, instruction, expected_paths, cwd)
        }
        ProfileId::Rust => lint_rust_step_contract(step_id, instruction),
        _ => Ok(()),
    }
}

pub fn render_profile_obligations(obligations: &[ProfileObligation]) -> Vec<String> {
    obligations
        .iter()
        .map(|obligation| {
            let paths = if obligation.paths.is_empty() {
                "none".to_string()
            } else {
                obligation
                    .paths
                    .iter()
                    .map(|path| bounded_value(path))
                    .collect::<Vec<_>>()
                    .join(",")
            };
            let expected = obligation
                .expected
                .as_deref()
                .map(bounded_value)
                .unwrap_or_else(|| "none".to_string());
            format!(
                "profile.obligation.{}={}; paths={}; expected={}",
                obligation.code,
                bounded_value(&obligation.message),
                paths,
                expected
            )
        })
        .collect()
}

fn profile_output_summary(id: ProfileId, cwd: &Path) -> ProfileOutput {
    let contract = profile_contract(id);
    let project_kind = profile_project_kind(id).to_string();
    let project_root_hints = profile_project_root_hints(id, cwd);
    let observed_paths = profile_observed_paths(id, cwd);
    let artifact_classifications = observed_paths
        .iter()
        .map(|path| {
            let classified =
                classify_profile_artifact(id, path, ArtifactProvenance::WorkspaceObservation);
            format!(
                "{}:{}",
                classified.path,
                artifact_kind_label(classified.kind)
            )
        })
        .collect::<Vec<_>>();
    let setup_artifacts = observed_paths
        .iter()
        .filter(|path| {
            let kind =
                classify_profile_artifact(id, path, ArtifactProvenance::WorkspaceObservation).kind;
            matches!(kind, ArtifactKind::Manifest | ArtifactKind::Config)
        })
        .cloned()
        .collect::<Vec<_>>();
    let scaffold_artifacts = profile_scaffold_artifacts(id, cwd);
    let route_integration_artifacts = observed_paths
        .iter()
        .filter(|path| {
            let classified =
                classify_profile_artifact(id, path, ArtifactProvenance::WorkspaceObservation);
            classified.eligibility.route_integration
                || matches!(classified.kind, ArtifactKind::RouteEntry)
        })
        .cloned()
        .collect::<Vec<_>>();
    let manifest_artifacts = profile_manifest_artifacts(id, cwd, &setup_artifacts);
    let entrypoints = profile_entrypoints(id, cwd);
    let integration_artifacts =
        profile_integration_artifacts(id, cwd, &route_integration_artifacts);
    let completion_evidence_requirements = profile_completion_evidence_requirements(id);
    let failure_mappings = profile_failure_mappings(id);
    let adapter_families = profile_adapter_families(id);
    let behavior_obligations = profile_behavior_obligations(id, cwd);
    let recovery_candidate_hints = profile_recovery_candidate_hints(
        id,
        &setup_artifacts,
        &scaffold_artifacts,
        &route_integration_artifacts,
    );
    let verifier_commands = contract.verifier_commands;
    let protected_paths = contract.protected_path_prefixes;
    let mut output = ProfileOutput {
        id,
        project_kind,
        project_root_hints,
        manifest_artifacts,
        entrypoints,
        integration_artifacts,
        completion_evidence_requirements,
        failure_mappings,
        adapter_families,
        capabilities: Vec::new(),
        artifact_classifications,
        setup_artifacts,
        scaffold_artifacts,
        route_integration_artifacts,
        verifier_commands,
        protected_paths,
        behavior_obligations,
        verification_failures: Vec::new(),
        recovery_candidate_hints,
    };
    output.capabilities = profile_capabilities(&output);
    output
}

fn profile_project_kind(id: ProfileId) -> &'static str {
    match id {
        ProfileId::Generic => "generic_workspace",
        ProfileId::NextJs => "nextjs_app",
        ProfileId::Python => "python_app",
        ProfileId::Rust => "rust_crate",
        ProfileId::Investigation => "investigation",
        ProfileId::Docs => "documentation",
        ProfileId::DataAnalysis => "data_analysis",
        ProfileId::DataPipeline => "data_pipeline",
    }
}

fn profile_project_root_hints(id: ProfileId, cwd: &Path) -> Vec<String> {
    match id {
        ProfileId::NextJs => {
            let mut roots = Vec::new();
            if cwd.join("src/app").exists() {
                roots.push("src/app".to_string());
            }
            if cwd.join("app").exists() {
                roots.push("app".to_string());
            }
            if roots.is_empty() {
                roots.push("app".to_string());
            }
            roots
        }
        ProfileId::Rust => vec![".".to_string()],
        ProfileId::Python => {
            if cwd.join("app").exists() {
                vec!["app".to_string()]
            } else {
                vec![".".to_string()]
            }
        }
        ProfileId::Docs => vec!["docs".to_string(), ".".to_string()],
        ProfileId::DataAnalysis | ProfileId::DataPipeline => {
            vec!["data".to_string(), "reports".to_string()]
        }
        ProfileId::Generic | ProfileId::Investigation => Vec::new(),
    }
}

fn profile_observed_paths(id: ProfileId, cwd: &Path) -> Vec<String> {
    let candidates: &[&str] = match id {
        ProfileId::NextJs => &[
            "package.json",
            "package-lock.json",
            "pnpm-lock.yaml",
            "next.config.js",
            "next.config.mjs",
            "tsconfig.json",
            "tailwind.config.js",
            "postcss.config.js",
            "app/page.tsx",
            "app/layout.tsx",
            "app/globals.css",
            "src/app/page.tsx",
            "src/app/layout.tsx",
            "src/app/globals.css",
            "components/Game.tsx",
        ],
        ProfileId::Rust => &["Cargo.toml", "Cargo.lock", "src/main.rs", "src/lib.rs"],
        ProfileId::Python => &[
            "pyproject.toml",
            "requirements.txt",
            "app/__init__.py",
            "app/main.py",
            "main.py",
            "tests/test_app.py",
        ],
        ProfileId::Docs => &["README.md", "docs/README.md", "docs/index.md"],
        ProfileId::DataAnalysis | ProfileId::DataPipeline => &[
            "data/raw",
            "data/processed",
            "output/report.csv",
            "reports/report.md",
        ],
        ProfileId::Generic | ProfileId::Investigation => &[],
    };
    candidates
        .iter()
        .filter(|path| cwd.join(path).exists())
        .map(|path| (*path).to_string())
        .collect()
}

fn profile_manifest_artifacts(
    id: ProfileId,
    cwd: &Path,
    setup_artifacts: &[String],
) -> Vec<String> {
    let expected = match id {
        ProfileId::NextJs => &["package.json"][..],
        ProfileId::Rust => &["Cargo.toml"][..],
        ProfileId::Python => &["pyproject.toml", "requirements.txt", "setup.py"][..],
        ProfileId::DataAnalysis | ProfileId::DataPipeline => &["pipeline.yaml", "pipeline.yml"][..],
        ProfileId::Generic | ProfileId::Investigation | ProfileId::Docs => &[][..],
    };
    let existing = expected
        .iter()
        .filter(|path| cwd.join(path).exists())
        .map(|path| (*path).to_string())
        .collect::<Vec<_>>();
    if !existing.is_empty() {
        existing
    } else if !setup_artifacts.is_empty() {
        setup_artifacts.to_vec()
    } else {
        expected.iter().map(|path| (*path).to_string()).collect()
    }
}

fn profile_entrypoints(id: ProfileId, cwd: &Path) -> Vec<String> {
    let candidates = match id {
        ProfileId::NextJs => vec!["src/app/page.tsx", "app/page.tsx", "pages/index.tsx"],
        ProfileId::Rust => vec!["src/main.rs", "src/lib.rs"],
        ProfileId::Python => vec!["app/main.py", "main.py", "app/__init__.py"],
        ProfileId::Docs => vec!["README.md", "docs/README.md", "docs/index.md"],
        ProfileId::DataAnalysis | ProfileId::DataPipeline => {
            vec![
                "scripts/analyze.py",
                "scripts/pipeline.py",
                "notebooks/analysis.ipynb",
            ]
        }
        ProfileId::Generic | ProfileId::Investigation => Vec::new(),
    };
    existing_or_expected(cwd, &candidates)
}

fn profile_integration_artifacts(
    id: ProfileId,
    cwd: &Path,
    route_artifacts: &[String],
) -> Vec<String> {
    match id {
        ProfileId::NextJs if !route_artifacts.is_empty() => route_artifacts.to_vec(),
        ProfileId::NextJs => existing_or_expected(
            cwd,
            &["src/app/page.tsx", "app/page.tsx", "components/Game.tsx"],
        ),
        ProfileId::Rust => {
            existing_or_expected(cwd, &["tests/integration.rs", "src/main.rs", "src/lib.rs"])
        }
        ProfileId::Python => {
            existing_or_expected(cwd, &["tests/test_app.py", "app/main.py", "main.py"])
        }
        ProfileId::Docs => existing_or_expected(cwd, &["README.md", "docs/README.md"]),
        ProfileId::DataAnalysis | ProfileId::DataPipeline => existing_or_expected(
            cwd,
            &["output/report.csv", "reports/report.md", "data/processed"],
        ),
        ProfileId::Generic | ProfileId::Investigation => Vec::new(),
    }
}

fn profile_completion_evidence_requirements(id: ProfileId) -> Vec<String> {
    match id {
        ProfileId::NextJs => vec![
            "npm_run_build".to_string(),
            "selected_route_binding".to_string(),
            "dev_port_smoke_when_requested".to_string(),
        ],
        ProfileId::Rust => vec![
            "cargo_test_or_build".to_string(),
            "cargo_manifest_binding".to_string(),
            "binary_or_library_binding".to_string(),
        ],
        ProfileId::Python => vec![
            "pytest_or_script_verifier".to_string(),
            "import_binding".to_string(),
        ],
        ProfileId::Docs => vec!["requested_doc_literal_or_section".to_string()],
        ProfileId::DataAnalysis | ProfileId::DataPipeline => vec![
            "derived_output_present".to_string(),
            "raw_input_unchanged".to_string(),
        ],
        ProfileId::Generic | ProfileId::Investigation => Vec::new(),
    }
}

fn profile_failure_mappings(id: ProfileId) -> Vec<String> {
    match id {
        ProfileId::NextJs => vec![
            "nextjs_missing_dependency->manifest_repair".to_string(),
            "nextjs_route_not_integrated->route_integration_repair".to_string(),
            "nextjs_app_root_ambiguous->explicit_stop".to_string(),
            "nextjs_dev_port_drift->manifest_repair".to_string(),
        ],
        ProfileId::Rust => vec![
            "cargo_manifest_missing->manifest_repair".to_string(),
            "rust_compile_error->source_implementation_repair".to_string(),
            "rust_test_failure->test_or_source_repair".to_string(),
        ],
        ProfileId::Python => vec![
            "python_manifest_missing->manifest_repair".to_string(),
            "python_import_missing->source_implementation_repair".to_string(),
            "pytest_failure->test_or_source_repair".to_string(),
        ],
        ProfileId::Docs => vec!["docs_literal_missing->documentation_repair".to_string()],
        ProfileId::DataAnalysis | ProfileId::DataPipeline => vec![
            "raw_input_mutation->explicit_stop".to_string(),
            "derived_output_missing->data_artifact_completion".to_string(),
        ],
        ProfileId::Generic | ProfileId::Investigation => Vec::new(),
    }
}

fn profile_adapter_families(id: ProfileId) -> Vec<String> {
    let ids = match id {
        ProfileId::NextJs => &[
            "node_next_type",
            "nextjs_route_integration",
            "manifest_dependency",
        ][..],
        ProfileId::Rust => &["rust_compile", "rust_cargo_manifest", "rust_assertion"][..],
        ProfileId::Python => &["python_import", "python_assertion", "fastapi_response"][..],
        ProfileId::Docs => &["docs_literal"][..],
        ProfileId::DataAnalysis | ProfileId::DataPipeline => &["data_schema"][..],
        ProfileId::Generic | ProfileId::Investigation => &[][..],
    };
    let registered = mechanical_adapter_family_specs()
        .iter()
        .map(|spec| spec.id)
        .collect::<BTreeSet<_>>();
    ids.iter()
        .filter(|id| registered.contains(**id))
        .map(|id| (*id).to_string())
        .collect()
}

fn existing_or_expected(cwd: &Path, candidates: &[&str]) -> Vec<String> {
    let existing = candidates
        .iter()
        .filter(|path| cwd.join(path).exists())
        .map(|path| (*path).to_string())
        .collect::<Vec<_>>();
    if existing.is_empty() {
        candidates.iter().map(|path| (*path).to_string()).collect()
    } else {
        existing
    }
}

fn profile_scaffold_artifacts(id: ProfileId, cwd: &Path) -> Vec<String> {
    match id {
        ProfileId::NextJs => {
            let root = if cwd.join("src/app").exists() {
                "src/app"
            } else {
                "app"
            };
            vec![
                "package.json".to_string(),
                format!("{root}/page.tsx"),
                format!("{root}/layout.tsx"),
            ]
        }
        ProfileId::Rust => vec!["Cargo.toml".to_string(), "src/main.rs".to_string()],
        ProfileId::Python => vec!["pyproject.toml".to_string(), "app/main.py".to_string()],
        ProfileId::Docs => vec!["README.md".to_string()],
        ProfileId::DataAnalysis | ProfileId::DataPipeline => vec!["output/report.csv".to_string()],
        ProfileId::Generic | ProfileId::Investigation => Vec::new(),
    }
}

fn profile_behavior_obligations(id: ProfileId, cwd: &Path) -> Vec<String> {
    match id {
        ProfileId::NextJs => {
            let mut obligations = vec![
                "manifest_contract".to_string(),
                "route_integration".to_string(),
                "build_verifier".to_string(),
            ];
            if cwd.join("package.json").exists() {
                obligations.push("dependency_setup_readiness".to_string());
            }
            obligations
        }
        ProfileId::Rust => vec!["cargo_manifest".to_string(), "cargo_verifier".to_string()],
        ProfileId::Python => vec!["python_imports".to_string(), "pytest_verifier".to_string()],
        ProfileId::Docs => vec!["documentation_literal".to_string()],
        ProfileId::DataAnalysis | ProfileId::DataPipeline => vec!["derived_output".to_string()],
        ProfileId::Generic | ProfileId::Investigation => Vec::new(),
    }
}

fn profile_recovery_candidate_hints(
    id: ProfileId,
    setup_artifacts: &[String],
    scaffold_artifacts: &[String],
    route_artifacts: &[String],
) -> Vec<String> {
    let mut hints = Vec::new();
    if !setup_artifacts.is_empty() {
        hints.push(format!("manifest_repair:{}", setup_artifacts.join("|")));
    }
    if !scaffold_artifacts.is_empty() {
        hints.push(format!(
            "scaffold_materialization:{}",
            scaffold_artifacts.join("|")
        ));
    }
    if !route_artifacts.is_empty() {
        hints.push(format!(
            "route_integration_repair:{}",
            route_artifacts.join("|")
        ));
    }
    match id {
        ProfileId::NextJs => hints.push("dev_server_smoke:requested_port".to_string()),
        ProfileId::Rust | ProfileId::Python => {
            hints.push("source_implementation_repair:verifier_diagnostic".to_string())
        }
        _ => {}
    }
    hints
}

fn profile_capabilities(output: &ProfileOutput) -> Vec<ProfileCapability> {
    use ProfileCapabilityFamily as Family;
    use ProfileCapabilityStatus as Status;

    let id = output.id;
    let manifest_status = if matches!(
        id,
        ProfileId::Generic | ProfileId::Investigation | ProfileId::Docs
    ) {
        Status::NotApplicable
    } else if output.manifest_artifacts.is_empty() {
        Status::Partial
    } else {
        Status::Supported
    };
    let integration_status = if matches!(id, ProfileId::Generic | ProfileId::Investigation) {
        Status::NotApplicable
    } else if output.integration_artifacts.is_empty() {
        Status::Partial
    } else {
        Status::Supported
    };
    let setup_status = if matches!(
        id,
        ProfileId::NextJs | ProfileId::Rust | ProfileId::Python | ProfileId::DataPipeline
    ) {
        Status::Supported
    } else {
        Status::NotApplicable
    };
    let verifier_status = if output.verifier_commands.is_empty() {
        match id {
            ProfileId::Docs | ProfileId::DataAnalysis | ProfileId::DataPipeline => {
                Status::NotApplicable
            }
            ProfileId::Generic | ProfileId::Investigation => Status::NotApplicable,
            _ => Status::Partial,
        }
    } else {
        Status::Supported
    };
    let protected_status = if output.protected_paths.is_empty() {
        Status::NotApplicable
    } else {
        Status::Supported
    };

    vec![
        ProfileCapability {
            family: Family::ProjectKind,
            status: Status::Supported,
            source_of_truth: "profile_id".to_string(),
            artifacts: vec![output.project_kind.clone()],
            recovery_owner_hint: None,
            authority: Some("profile_contract".to_string()),
            reason: None,
        },
        ProfileCapability {
            family: Family::RootHints,
            status: if output.project_root_hints.is_empty() {
                Status::NotApplicable
            } else {
                Status::Supported
            },
            source_of_truth: "workspace_observation".to_string(),
            artifacts: output.project_root_hints.clone(),
            recovery_owner_hint: None,
            authority: Some("profile_contract".to_string()),
            reason: None,
        },
        ProfileCapability {
            family: Family::ManifestContract,
            status: manifest_status,
            source_of_truth: "profile_manifest_artifacts".to_string(),
            artifacts: output.manifest_artifacts.clone(),
            recovery_owner_hint: Some("manifest_repair".to_string()),
            authority: Some("profile_contract".to_string()),
            reason: Some("manifest_or_setup_boundary".to_string()),
        },
        ProfileCapability {
            family: Family::EntrypointContract,
            status: if output.entrypoints.is_empty() {
                Status::NotApplicable
            } else {
                Status::Supported
            },
            source_of_truth: "profile_entrypoints".to_string(),
            artifacts: output.entrypoints.clone(),
            recovery_owner_hint: Some("artifact_completion".to_string()),
            authority: Some("profile_contract".to_string()),
            reason: None,
        },
        ProfileCapability {
            family: Family::IntegrationContract,
            status: integration_status,
            source_of_truth: "profile_integration_artifacts".to_string(),
            artifacts: output.integration_artifacts.clone(),
            recovery_owner_hint: Some("route_or_entrypoint_integration_repair".to_string()),
            authority: Some("profile_verification".to_string()),
            reason: None,
        },
        ProfileCapability {
            family: Family::SetupContract,
            status: setup_status,
            source_of_truth: "profile_setup_artifacts".to_string(),
            artifacts: output.setup_artifacts.clone(),
            recovery_owner_hint: Some("setup_recovery".to_string()),
            authority: Some("verifier_owned_setup".to_string()),
            reason: None,
        },
        ProfileCapability {
            family: Family::VerifierContract,
            status: verifier_status,
            source_of_truth: "profile_verifier_commands".to_string(),
            artifacts: output.verifier_commands.clone(),
            recovery_owner_hint: Some("verifier_repair".to_string()),
            authority: Some("step_runner_verification".to_string()),
            reason: None,
        },
        ProfileCapability {
            family: Family::CompletionEvidenceContract,
            status: if output.completion_evidence_requirements.is_empty() {
                Status::Partial
            } else {
                Status::Supported
            },
            source_of_truth: "profile_completion_evidence".to_string(),
            artifacts: output.completion_evidence_requirements.clone(),
            recovery_owner_hint: Some("completion_evidence_binding".to_string()),
            authority: Some("profile_contract".to_string()),
            reason: None,
        },
        ProfileCapability {
            family: Family::ProtectedInputContract,
            status: protected_status,
            source_of_truth: "profile_protected_paths".to_string(),
            artifacts: output.protected_paths.clone(),
            recovery_owner_hint: Some("explicit_stop".to_string()),
            authority: Some("safety_guard".to_string()),
            reason: Some("raw_or_protected_inputs".to_string()),
        },
        ProfileCapability {
            family: Family::ScaffoldContract,
            status: if output.scaffold_artifacts.is_empty() {
                Status::NotApplicable
            } else {
                Status::Supported
            },
            source_of_truth: "profile_scaffold_artifacts".to_string(),
            artifacts: output.scaffold_artifacts.clone(),
            recovery_owner_hint: Some("scaffold_materialization".to_string()),
            authority: Some("profile_contract".to_string()),
            reason: None,
        },
        ProfileCapability {
            family: Family::ProfileFailureMapping,
            status: if output.failure_mappings.is_empty() {
                Status::Partial
            } else {
                Status::Supported
            },
            source_of_truth: "profile_failure_mapping".to_string(),
            artifacts: output.failure_mappings.clone(),
            recovery_owner_hint: Some("recovery_task_contract".to_string()),
            authority: Some("profile_failure_mapping".to_string()),
            reason: None,
        },
        ProfileCapability {
            family: Family::LanguageAdapterFamily,
            status: if output.adapter_families.is_empty() {
                Status::Partial
            } else {
                Status::Supported
            },
            source_of_truth: "profile_adapter_families".to_string(),
            artifacts: output.adapter_families.clone(),
            recovery_owner_hint: Some("mechanical_repair_adapter".to_string()),
            authority: Some("diagnostic_adapter_registry".to_string()),
            reason: None,
        },
    ]
}

fn profile_capability_status_summary(capabilities: &[ProfileCapability]) -> String {
    if capabilities.is_empty() {
        return "none".to_string();
    }
    capabilities
        .iter()
        .map(|capability| {
            format!(
                "{}:{}",
                capability.family.eval_key(),
                capability.status.summary_str()
            )
        })
        .collect::<Vec<_>>()
        .join("|")
}

fn join_profile_values(values: &[String]) -> String {
    join_profile_values_limited(values, 8)
}

fn join_profile_values_limited(values: &[String], limit: usize) -> String {
    if values.is_empty() {
        "none".to_string()
    } else {
        values
            .iter()
            .take(limit)
            .map(|value| bounded_value(value))
            .collect::<Vec<_>>()
            .join("|")
    }
}

fn data_protected_prefixes() -> Vec<String> {
    vec![
        "raw".to_string(),
        "data/raw".to_string(),
        "input".to_string(),
        "inputs".to_string(),
    ]
}

fn lint_rust_step_contract(step_id: &str, instruction: &str) -> Result<(), PlanLintError> {
    let lower = instruction.to_ascii_lowercase();
    if contains_any(&lower, &["cargo init", "cargo new"]) {
        return Err(PlanLintError::ShellScaffold {
            step_id: step_id.to_string(),
            command: "cargo init/new".to_string(),
            guidance: "create Cargo.toml and src/main.rs with Write/Edit".to_string(),
        });
    }
    Ok(())
}

fn lint_nextjs_plan(
    plan: &StepPlan,
    cwd: Option<&Path>,
    obligations: &[ProfileObligation],
) -> Result<(), PlanLintError> {
    lint_nextjs_verifier_contract(plan)?;
    lint_nextjs_typescript_plan_contract(plan)?;
    lint_nextjs_alias_plan_contract(plan)?;
    lint_nextjs_app_layout_plan_contract(plan, cwd)?;
    lint_nextjs_tailwind_plan_contract(plan)?;
    lint_package_profile_obligations(plan, cwd, obligations)?;
    lint_nextjs_route_integration_obligations(plan, cwd, obligations)
}

fn lint_nextjs_scaffolding_and_root_drift(
    step_id: &str,
    kind: StepKind,
    instruction: &str,
    expected_paths: &[String],
    cwd: Option<&Path>,
) -> Result<(), PlanLintError> {
    let lower = instruction.to_ascii_lowercase();
    if mentions_nextjs_shell_scaffold(&lower) {
        return Err(PlanLintError::ShellScaffold {
            step_id: step_id.to_string(),
            command: "create-next-app".to_string(),
            guidance: "create package.json and app/page.tsx with Write/Edit".to_string(),
        });
    }
    lint_nextjs_root_drift(step_id, kind, &lower, expected_paths, cwd)?;
    if mentions_noop_nextjs_build_script(&lower) {
        return Err(PlanLintError::InvalidStepInstruction {
            step_id: step_id.to_string(),
            reason:
                "Next.js build script must remain honest; do not replace it with no-op commands"
                    .to_string(),
        });
    }
    Ok(())
}

fn mentions_noop_nextjs_build_script(lower_instruction: &str) -> bool {
    contains_any(
        lower_instruction,
        &[
            "\"build\":\"true\"",
            "\"build\": \"true\"",
            "'build':'true'",
            "'build': 'true'",
            "scripts.build=true",
            "scripts.build = true",
            "scripts.build as true",
            "scripts.build to true",
            "build script as true",
            "build script to true",
            "build script is true",
            "build script runs true",
            "build script with true",
            "\"build\":\"echo ok\"",
            "\"build\": \"echo ok\"",
            "'build':'echo ok'",
            "'build': 'echo ok'",
            "scripts.build=echo ok",
            "scripts.build = echo ok",
            "scripts.build as echo ok",
            "scripts.build to echo ok",
            "build script as echo ok",
            "build script to echo ok",
            "build script is echo ok",
            "build script runs echo ok",
            "build script with echo ok",
        ],
    )
}

fn mentions_nextjs_shell_scaffold(lower_instruction: &str) -> bool {
    contains_any(
        lower_instruction,
        &[
            "npm create next-app",
            "pnpm create next-app",
            "yarn create next-app",
        ],
    ) || contains_any(
        lower_instruction,
        &[
            "using create-next-app",
            "use create-next-app",
            "run create-next-app",
            "execute create-next-app",
            "with create-next-app",
            "via create-next-app",
        ],
    )
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

fn lint_nextjs_verifier_contract(plan: &StepPlan) -> Result<(), PlanLintError> {
    for step in &plan.steps {
        for command in &step.verify {
            let lower = command.trim().to_ascii_lowercase();
            if !lower.starts_with("npx ") {
                continue;
            }
            let reason = format!(
                "Next.js verifier `{}` uses npx, which may perform dependency setup and is blocked by the execution policy; use npm run build so verifier-owned setup recovery can classify dependency_missing",
                command.trim()
            );
            return Err(PlanLintError::ContractViolation {
                step_id: step.id.clone(),
                reason: reason.clone(),
                evidence: Box::new(
                    PlanCorrectionEvidence::new("plan_lint.nextjs_verifier_contract")
                        .with_failed_step(step.id.clone())
                        .with_violated_contract("nextjs_verifier_command_required")
                        .with_target_field("verify")
                        .with_rejected_value(command.clone())
                        .with_required_literals(vec!["npm run build"])
                        .with_missing_literals(vec!["npm run build"])
                        .with_required_action(
                            "replace the npx verifier with npm run build in a separate verify step; do not add npm install, npm ci, node_modules checks, or npx commands",
                        )
                        .with_diagnostic(reason),
                ),
            });
        }
    }
    Ok(())
}

fn lint_nextjs_typescript_plan_contract(plan: &StepPlan) -> Result<(), PlanLintError> {
    if !plan.steps.iter().any(step_mentions_package_json) {
        return Ok(());
    }
    let Some(source_step) = plan_typescript_source_step(plan) else {
        return Ok(());
    };
    let package_steps = plan
        .steps
        .iter()
        .filter(|step| step_mentions_package_json(step))
        .collect::<Vec<_>>();
    let package_step_id = match package_steps.as_slice() {
        [step] => Some(step.id.clone()),
        _ => None,
    };
    let package_plan_text = package_steps
        .iter()
        .map(|step| step_contract_text(step))
        .collect::<Vec<_>>()
        .join("\n");
    let mut missing = Vec::new();
    let mut required_literals = Vec::new();
    let mut missing_literals = Vec::new();
    for literal in ["typescript", "@types/react", "18"] {
        push_unique(&mut required_literals, literal);
        if !package_plan_text.contains(literal) {
            push_unique(&mut missing_literals, literal);
            missing.push(literal.to_string());
        }
    }
    push_unique(&mut required_literals, "5.x or ^5.");
    if !stable_typescript_5_plan_literal(&package_plan_text) {
        push_unique(&mut missing_literals, "5.x or ^5.");
        missing.push("5.x or ^5.".to_string());
    }
    if missing.is_empty() {
        return Ok(());
    }
    let reason = format!(
        "Next.js TypeScript step `{}` uses tsconfig, .ts, .tsx, or TypeScript, but the package.json plan does not make the TypeScript toolchain contract complete: missing {}",
        source_step.id,
        missing.join(", ")
    );
    let mut evidence = PlanCorrectionEvidence::new("plan_lint.nextjs_typescript_plan_contract")
        .with_failed_step(source_step.id.clone())
        .with_violated_contract("nextjs_typescript_toolchain_plan_contract")
        .with_target_field("steps")
        .with_target_path("package.json")
        .with_active_job("manifest_repair")
        .with_artifact_role("manifest")
        .with_repair_kind("typescript_toolchain_contract_repair")
        .with_repair_action("add_manifest_dependency")
        .with_setup_implication("setup_after_manifest_repair_required")
        .with_rerun_authority(vec!["plan lint", "profile verification", "npm run build"])
        .with_required_literals(required_literals)
        .with_missing_literals(missing_literals)
        .with_required_action(
            "make the Next.js TypeScript toolchain explicit in the package.json step: include exact literals typescript, @types/react, and 18, plus a stable TypeScript 5.x range such as ^5.4.0; do not use TypeScript 6, exact TypeScript pins such as 5.0.0, or @types/react 19 with Next.js 14"
        )
        .with_disallowed_actions(vec![
            "Do not rewrite source/gameplay behavior while repairing the manifest plan contract.",
            "Do not add npm install, npm ci, pnpm install, yarn install, node_modules, or lockfile checks as required plan work.",
            "Do not replace npm run build with a weaker verifier.",
        ])
        .with_diagnostic(reason.clone());
    if let Some(package_step_id) = package_step_id {
        evidence = evidence
            .with_repair_target(format!("step:{package_step_id}:instruction"))
            .with_candidate_artifacts(vec![
                format!("step:{package_step_id}"),
                "package.json".to_string(),
            ]);
    } else {
        evidence = evidence.with_repair_attempt(
            "active job arbitration could not select one package.json step for deterministic plan materialization",
        );
    }
    Err(PlanLintError::ContractViolation {
        step_id: source_step.id.clone(),
        reason: reason.clone(),
        evidence: Box::new(evidence),
    })
}

fn stable_typescript_5_plan_literal(text: &str) -> bool {
    text.contains("5.x") || text.contains("^5.") || text.contains("~5.")
}

fn plan_typescript_source_step(plan: &StepPlan) -> Option<&StepPlanStep> {
    plan.steps.iter().find(|step| {
        let text = step_contract_text(step);
        text.contains("typescript")
            || text.contains("tsconfig")
            || step.expected_paths.iter().any(|path| {
                matches!(
                    Path::new(path).extension().and_then(|ext| ext.to_str()),
                    Some("ts" | "tsx")
                )
            })
    })
}

fn lint_nextjs_alias_plan_contract(plan: &StepPlan) -> Result<(), PlanLintError> {
    let Some(source_step) = plan.steps.iter().find(|step| {
        let text = step_contract_text(step);
        text.contains("\"@/")
            || text.contains("'@/")
            || text.contains("from \"@")
            || text.contains("from '@")
    }) else {
        return Ok(());
    };
    let plan_text = plan
        .steps
        .iter()
        .map(step_contract_text)
        .collect::<Vec<_>>()
        .join("\n");
    let required_alias_literals = nextjs_required_alias_literals(&plan_text);
    let has_tsconfig_path = plan.steps.iter().any(|step| {
        step.expected_paths
            .iter()
            .any(|path| path == "tsconfig.json")
    }) || plan_text.contains("tsconfig.json");
    let has_alias_contract = plan_text.contains("compileroptions.paths")
        || plan_text.contains("compileroptions paths")
        || plan_text.contains("paths mapping")
        || plan_text.contains("\"paths\"");
    let missing_alias_literals = required_alias_literals
        .iter()
        .filter(|literal| !plan_text.contains(literal.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    if has_tsconfig_path && has_alias_contract && missing_alias_literals.is_empty() {
        return Ok(());
    }
    let reason = format!(
        "Next.js source step `{}` uses @/* imports, but the plan does not create or edit tsconfig.json with compilerOptions.paths for @/*",
        source_step.id
    );
    Err(PlanLintError::ContractViolation {
        step_id: source_step.id.clone(),
        reason: reason.clone(),
        evidence: Box::new(
            PlanCorrectionEvidence::new("plan_lint.nextjs_alias_plan_contract")
                .with_failed_step(source_step.id.clone())
                .with_violated_contract("nextjs_alias_plan_contract")
                .with_target_field("steps")
                .with_target_path("tsconfig.json")
                .with_repair_target("tsconfig.json")
                .with_active_job("manifest_repair")
                .with_artifact_role("setup_config")
                .with_repair_action("add_missing_manifest_dependency")
                .with_required_paths(vec!["tsconfig.json"])
                .with_missing_paths(vec!["tsconfig.json"])
                .with_required_literals(
                    std::iter::once("compilerOptions.paths".to_string())
                        .chain(required_alias_literals)
                        .collect::<Vec<_>>(),
                )
                .with_missing_literals(
                    std::iter::once("compilerOptions.paths".to_string())
                        .chain(missing_alias_literals)
                        .collect::<Vec<_>>(),
                )
                .with_required_action(
                    "either replace @/* imports with relative imports, or add a tsconfig.json step with compilerOptions.paths mapping @/* to the selected source root",
                )
                .with_disallowed_actions(vec![
                    "Do not leave @/* imports without tsconfig compilerOptions.paths.",
                    "Do not replace npm run build with a weaker verifier.",
                ])
                .with_rerun_authority(vec!["plan lint", "profile verification", "npm run build"])
                .with_diagnostic(reason),
        ),
    })
}

fn nextjs_required_alias_literals(plan_text: &str) -> Vec<String> {
    let mut aliases = Vec::new();
    if plan_text.contains("\"@/") || plan_text.contains("'@/") {
        push_unique(&mut aliases, "@/*");
    }
    if plan_text.contains("\"@components/") || plan_text.contains("'@components/") {
        push_unique(&mut aliases, "@components/*");
    }
    aliases
}

fn lint_nextjs_app_layout_plan_contract(
    plan: &StepPlan,
    cwd: Option<&Path>,
) -> Result<(), PlanLintError> {
    let Some(page_path) = plan
        .steps
        .iter()
        .flat_map(|step| step.expected_paths.iter())
        .find(|path| nextjs_app_route_page_path(path))
        .cloned()
    else {
        return Ok(());
    };
    let layout_path = nextjs_layout_path_for_page(&page_path);
    if cwd
        .map(collect_nextjs_facts)
        .is_some_and(|facts| facts.layouts.iter().any(|path| path == &layout_path))
        || plan.steps.iter().any(|step| {
            step.expected_paths.iter().any(|path| path == &layout_path)
                || step.instruction.contains(&layout_path)
        })
    {
        return Ok(());
    }
    let reason = format!(
        "Next.js app route `{page_path}` requires root layout `{layout_path}` in the same app root"
    );
    Err(PlanLintError::ContractViolation {
        step_id: "nextjs-app-layout".to_string(),
        reason: reason.clone(),
        evidence: Box::new(
            PlanCorrectionEvidence::new("plan_lint.nextjs_app_layout_plan_contract")
                .with_failed_step("nextjs-app-layout")
                .with_violated_contract("nextjs_app_layout_plan_contract")
                .with_target_field("steps")
                .with_target_path(layout_path.clone())
                .with_repair_target(layout_path.clone())
                .with_active_job("route_integration_repair")
                .with_artifact_role("route_layout")
                .with_repair_kind("route_layout_repair")
                .with_repair_action("add_missing_route_layout")
                .with_required_paths(vec![layout_path.clone()])
                .with_missing_paths(vec![layout_path])
                .with_required_action(
                    "add a minimal root layout file for the selected Next.js app root before running npm run build",
                )
                .with_disallowed_actions(vec![
                    "Do not weaken npm run build.",
                    "Do not switch to pages router to avoid the layout contract.",
                ])
                .with_rerun_authority(vec!["plan lint", "profile verification", "npm run build"])
                .with_diagnostic(reason),
        ),
    })
}

fn nextjs_app_route_page_path(path: &str) -> bool {
    matches!(
        path,
        "app/page.tsx" | "app/page.ts" | "src/app/page.tsx" | "src/app/page.ts"
    )
}

fn nextjs_layout_path_for_page(page_path: &str) -> String {
    if page_path.starts_with("src/app/") {
        "src/app/layout.tsx".to_string()
    } else {
        "app/layout.tsx".to_string()
    }
}

fn lint_nextjs_tailwind_plan_contract(plan: &StepPlan) -> Result<(), PlanLintError> {
    let Some(source_step) = plan_tailwind_source_step(plan) else {
        return Ok(());
    };
    let package_steps = plan
        .steps
        .iter()
        .filter(|step| step_mentions_package_json(step))
        .collect::<Vec<_>>();
    let package_step_id = match package_steps.as_slice() {
        [step] => Some(step.id.clone()),
        _ => None,
    };
    let package_plan_text = plan
        .steps
        .iter()
        .filter(|step| step_mentions_package_json(step))
        .map(step_contract_text)
        .collect::<Vec<_>>()
        .join("\n");
    let plan_text = plan
        .steps
        .iter()
        .map(step_contract_text)
        .collect::<Vec<_>>()
        .join("\n");
    let mut missing = Vec::new();
    let mut required_literals = Vec::new();
    let mut missing_literals = Vec::new();
    for literal in ["tailwindcss", "postcss", "autoprefixer"] {
        push_unique(&mut required_literals, literal);
        if !package_plan_text.contains(literal) {
            push_unique(&mut missing_literals, literal);
            missing.push(literal.to_string());
        }
    }
    for literal in ["tailwind.config", "postcss.config"] {
        push_unique(&mut required_literals, literal);
        if !plan_text.contains(literal) {
            push_unique(&mut missing_literals, literal);
            missing.push(literal.to_string());
        }
    }
    if missing.is_empty() {
        return Ok(());
    }
    let reason = format!(
        "Next.js source/style step `{}` mentions Tailwind, but the plan does not make the Tailwind package/config setup contract complete: missing {}",
        source_step.id,
        missing.join(", ")
    );
    let mut evidence = PlanCorrectionEvidence::new("plan_lint.nextjs_tailwind_plan_contract")
        .with_failed_step(source_step.id.clone())
        .with_violated_contract("nextjs_tailwind_plan_contract")
        .with_target_field("steps")
        .with_target_path("package.json")
        .with_active_job("manifest_repair")
        .with_artifact_role("manifest")
        .with_repair_kind("tailwind_contract_repair")
        .with_repair_action("repair_tailwind_contract")
        .with_setup_implication("setup_after_manifest_repair_required")
        .with_rerun_authority(vec!["plan lint", "profile verification", "npm run build"])
        .with_required_literals(required_literals)
        .with_missing_literals(missing_literals)
        .with_required_action(
            "manifest repair: either remove Tailwind from source/style steps, or update the package.json plan step to literally include tailwindcss, postcss, and autoprefixer plus setup/config outputs tailwind.config.js and postcss.config.js; the phrase Tailwind CSS dependencies is not sufficient",
        )
        .with_disallowed_actions(vec![
            "Do not rewrite source/gameplay behavior while repairing the manifest plan contract.",
            "Do not add npm install, npm ci, pnpm install, yarn install, node_modules, or lockfile checks as required plan work.",
            "Do not replace npm run build with a weaker verifier.",
        ])
        .with_diagnostic(reason.clone());
    if let Some(package_step_id) = package_step_id {
        evidence = evidence
            .with_repair_target(format!("step:{package_step_id}:instruction"))
            .with_candidate_artifacts(vec![
                format!("step:{package_step_id}"),
                "package.json".to_string(),
            ]);
    } else {
        evidence = evidence.with_repair_attempt(
            "active job arbitration could not select one package.json step for deterministic plan materialization",
        );
    }
    Err(PlanLintError::ContractViolation {
        step_id: source_step.id.clone(),
        reason: reason.clone(),
        evidence: Box::new(evidence),
    })
}

fn plan_tailwind_source_step(plan: &StepPlan) -> Option<&StepPlanStep> {
    plan.steps.iter().find(|step| {
        let text = step_contract_text(step);
        if !(text.contains("tailwind") || text.contains("@tailwind")) {
            return false;
        }
        step.expected_paths
            .iter()
            .any(|path| nextjs_style_source_path(path))
            || text.contains("globals.css")
    })
}

fn nextjs_style_source_path(path: &str) -> bool {
    matches!(
        path,
        "app/globals.css" | "src/app/globals.css" | "styles/globals.css" | "src/styles/globals.css"
    )
}

fn lint_package_profile_obligations(
    plan: &StepPlan,
    cwd: Option<&Path>,
    obligations: &[ProfileObligation],
) -> Result<(), PlanLintError> {
    let package_steps = plan
        .steps
        .iter()
        .filter(|step| {
            matches!(
                step.kind,
                StepKind::Create | StepKind::Edit | StepKind::Setup | StepKind::Repair
            ) && step_mentions_package_json(step)
        })
        .collect::<Vec<_>>();
    if package_steps.is_empty() {
        return Ok(());
    }
    let first_step_id = package_steps[0].id.clone();
    let package_plan_text = package_steps
        .iter()
        .map(|step| step_contract_text(step))
        .collect::<Vec<_>>()
        .join("\n");
    let facts = cwd.map(collect_nextjs_facts);
    let mut missing = Vec::new();
    let mut violated_contracts = Vec::new();
    let mut required_literals = Vec::new();
    let mut missing_literals = Vec::new();
    for obligation in obligations {
        match obligation.code.as_str() {
            "nextjs_dev_port_required"
                if !nextjs_dev_port_satisfied(facts.as_ref())
                    && !package_plan_text.contains("3011") =>
            {
                missing.push("nextjs_dev_port_required: requested port 3011");
                violated_contracts.push("nextjs_dev_port_required".to_string());
                collect_missing_literals(
                    &package_plan_text,
                    &["3011"],
                    &mut required_literals,
                    &mut missing_literals,
                );
            }
            "nextjs_build_script_required"
                if !nextjs_build_script_satisfied(facts.as_ref())
                    && !package_plan_text.contains("next build") =>
            {
                missing.push("nextjs_build_script_required: scripts.build as next build");
                violated_contracts.push("nextjs_build_script_required".to_string());
                collect_missing_literals(
                    &package_plan_text,
                    &["next build"],
                    &mut required_literals,
                    &mut missing_literals,
                );
            }
            "nextjs_dependencies_required"
                if !nextjs_runtime_dependencies_satisfied(facts.as_ref())
                    && !["next", "react", "react-dom", "18.2"]
                        .iter()
                        .all(|dep| package_plan_text.contains(dep)) =>
            {
                missing.push(
                    "nextjs_dependencies_required: next, react, react-dom, and React 18.2+ compatibility",
                );
                violated_contracts.push("nextjs_dependencies_required".to_string());
                collect_missing_literals(
                    &package_plan_text,
                    &["next", "react", "react-dom", "18.2"],
                    &mut required_literals,
                    &mut missing_literals,
                );
            }
            "nextjs_tailwind_dependencies_required"
                if !nextjs_tailwind_dependencies_satisfied(facts.as_ref())
                    && !["tailwindcss", "postcss", "autoprefixer"]
                        .iter()
                        .all(|dep| package_plan_text.contains(dep)) =>
            {
                missing.push(
                    "nextjs_tailwind_dependencies_required: tailwindcss, postcss, and autoprefixer",
                );
                violated_contracts.push("nextjs_tailwind_dependencies_required".to_string());
                collect_missing_literals(
                    &package_plan_text,
                    &["tailwindcss", "postcss", "autoprefixer"],
                    &mut required_literals,
                    &mut missing_literals,
                );
            }
            _ => {}
        }
    }
    if !missing.is_empty() {
        let reason = format!(
            "profile obligations require package.json work to mention {}",
            missing.join("; ")
        );
        let mut evidence = PlanCorrectionEvidence::new("plan_lint.profile_obligations")
            .with_failed_step(package_steps[0].id.clone())
            .with_violated_contract(violated_contracts.join(", "))
            .with_target_field("instruction")
            .with_target_path("package.json")
            .with_active_job("manifest_repair")
            .with_artifact_role("manifest")
            .with_repair_kind("manifest_dependency_repair")
            .with_repair_action("add_manifest_dependency")
            .with_setup_implication("setup_after_manifest_repair_required")
            .with_rerun_authority(vec!["plan lint", "profile verification", "npm run build"])
            .with_required_literals(required_literals)
            .with_missing_literals(missing_literals)
            .with_required_action(
                "manifest repair: include these exact package/profile literals in the corrected package.json step instruction",
            )
            .with_disallowed_actions(vec![
                "Do not rewrite source/gameplay behavior while repairing the manifest plan contract.",
                "Do not add npm install, npm ci, pnpm install, yarn install, node_modules, or lockfile checks as required plan work.",
                "Do not replace npm run build with a weaker verifier.",
            ])
            .with_diagnostic(reason.clone());
        if package_steps.len() == 1 {
            evidence = evidence
                .with_repair_target(format!("step:{}:instruction", package_steps[0].id))
                .with_candidate_artifacts(vec![
                    format!("step:{}", package_steps[0].id),
                    "package.json".to_string(),
                ]);
        } else {
            evidence = evidence.with_repair_attempt(
                "active job arbitration could not select one package.json step for deterministic plan materialization",
            );
        }
        return Err(PlanLintError::ContractViolation {
            step_id: first_step_id,
            reason: reason.clone(),
            evidence: Box::new(evidence),
        });
    }
    Ok(())
}

fn nextjs_dev_port_satisfied(facts: Option<&NextJsFacts>) -> bool {
    facts
        .and_then(|facts| facts.scripts_dev.as_deref())
        .is_some_and(|script| script.contains("next dev") && script.contains("3011"))
}

fn nextjs_build_script_satisfied(facts: Option<&NextJsFacts>) -> bool {
    facts
        .and_then(|facts| facts.scripts_build.as_deref())
        .is_some_and(|script| script.trim() == "next build")
}

fn nextjs_runtime_dependencies_satisfied(facts: Option<&NextJsFacts>) -> bool {
    facts.is_some_and(|facts| {
        ["next", "react", "react-dom"]
            .iter()
            .all(|dep| facts.dependencies.contains(*dep))
            && facts
                .dependency_versions
                .get("react")
                .is_some_and(|version| version_major_at_least(version, 18))
    })
}

fn nextjs_tailwind_dependencies_satisfied(facts: Option<&NextJsFacts>) -> bool {
    facts.is_some_and(|facts| {
        ["tailwindcss", "postcss", "autoprefixer"]
            .iter()
            .all(|dep| facts.dependencies.contains(*dep))
    })
}

fn lint_nextjs_route_integration_obligations(
    plan: &StepPlan,
    cwd: Option<&Path>,
    obligations: &[ProfileObligation],
) -> Result<(), PlanLintError> {
    let selected_route = selected_route_from_route_obligations(obligations)
        .or_else(|| cwd.and_then(nextjs_selected_route_from_workspace));
    let Some(selected_route) = selected_route else {
        return Ok(());
    };
    for step in plan.steps.iter().filter(|step| {
        matches!(
            step.kind,
            StepKind::Create | StepKind::Edit | StepKind::Repair
        )
    }) {
        let Some(candidate) = step
            .expected_paths
            .iter()
            .find(|path| nextjs_route_integration_candidate(path))
        else {
            continue;
        };
        if step
            .expected_paths
            .iter()
            .any(|path| path == &selected_route)
            || step.instruction.contains(&selected_route)
        {
            continue;
        }
        if route_integration_planned_in_step_plan(plan, candidate, &selected_route) {
            continue;
        }
        let reason = format!(
            "profile obligations require Next.js route integration: step creates or edits {candidate} but does not mention selected route {selected_route} in instruction or expected_paths"
        );
        return Err(PlanLintError::ContractViolation {
            step_id: step.id.clone(),
            reason: reason.clone(),
            evidence: Box::new(PlanCorrectionEvidence::new("plan_lint.profile_obligations")
                .with_failed_step(step.id.clone())
                .with_violated_contract("nextjs_route_integration_required")
                .with_target_field("instruction_or_expected_paths")
                .with_active_job("route_integration_repair")
                .with_artifact_role("route_integration")
                .with_repair_kind("route_integration_repair")
                .with_repair_action("connect_artifact_to_selected_route")
                .with_rerun_authority(vec!["plan lint", "profile verification", "npm run build"])
                .with_required_paths(vec![selected_route.clone()])
                .with_missing_paths(vec![selected_route])
                .with_rejected_value(candidate.clone())
                .with_required_action(
                    "include the selected route in expected_paths or explicitly mention updating it"
                )
                .with_diagnostic(reason)),
        });
    }
    Ok(())
}

fn route_integration_planned_in_step_plan(
    plan: &StepPlan,
    candidate: &str,
    selected_route: &str,
) -> bool {
    let Some(candidate_stem) = path_stem(candidate) else {
        return false;
    };
    plan.steps
        .iter()
        .filter(|step| {
            matches!(
                step.kind,
                StepKind::Create | StepKind::Edit | StepKind::Repair
            )
        })
        .any(|step| {
            let touches_selected_route = step
                .expected_paths
                .iter()
                .any(|path| path == selected_route)
                || step.instruction.contains(selected_route);
            let mentions_artifact =
                step.instruction.contains(candidate) || step.instruction.contains(candidate_stem);
            touches_selected_route && mentions_artifact
        })
}

fn selected_route_from_route_obligations(obligations: &[ProfileObligation]) -> Option<String> {
    obligations
        .iter()
        .find(|obligation| obligation.code == "nextjs_route_integration_required")
        .and_then(|obligation| obligation.paths.first())
        .cloned()
}

fn step_contract_text(step: &StepPlanStep) -> String {
    format!(
        "{}\n{}\n{}",
        step.instruction.to_ascii_lowercase(),
        step.expected_paths
            .iter()
            .map(|path| path.to_ascii_lowercase())
            .collect::<Vec<_>>()
            .join("\n"),
        step.verify.join("\n").to_ascii_lowercase()
    )
}

fn step_mentions_package_json(step: &StepPlanStep) -> bool {
    step.expected_paths
        .iter()
        .any(|path| path == "package.json")
        || step
            .instruction
            .to_ascii_lowercase()
            .contains("package.json")
}

fn collect_missing_literals(
    text: &str,
    required: &[&str],
    required_literals: &mut Vec<String>,
    missing_literals: &mut Vec<String>,
) {
    for literal in required {
        push_unique(required_literals, literal);
        if !text.contains(literal) {
            push_unique(missing_literals, literal);
        }
    }
}

fn push_unique(values: &mut Vec<String>, value: &str) {
    if !values.iter().any(|existing| existing == value) {
        values.push(value.to_string());
    }
}

fn path_stem(path: &str) -> Option<&str> {
    let file_name = path.rsplit('/').next().unwrap_or(path);
    file_name.rsplit_once('.').map(|(stem, _)| stem)
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

#[derive(Debug, Default)]
struct NextJsFacts {
    package_json: bool,
    scripts_dev: Option<String>,
    scripts_build: Option<String>,
    dependencies: BTreeSet<String>,
    dependency_versions: BTreeMap<String, String>,
    routes: Vec<String>,
    layouts: Vec<String>,
    config_files: Vec<String>,
    tailwind_css_files: Vec<String>,
    tsconfig_root_dir: Option<String>,
    tsconfig_has_alias: bool,
}

fn nextjs_fact_summary(cwd: &Path) -> ProfileFactSummary {
    let facts = collect_nextjs_facts(cwd);
    let mut lines = Vec::new();
    lines.push(format!(
        "nextjs.package_json={}",
        if facts.package_json {
            "present"
        } else {
            "missing"
        }
    ));
    push_optional_fact(
        &mut lines,
        "nextjs.scripts.dev",
        facts.scripts_dev.as_deref(),
    );
    push_optional_fact(
        &mut lines,
        "nextjs.scripts.build",
        facts.scripts_build.as_deref(),
    );
    lines.push(format!(
        "nextjs.dependencies={}",
        if facts.dependencies.is_empty() {
            "none".to_string()
        } else {
            facts
                .dependencies
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(",")
        }
    ));
    lines.push(format!(
        "nextjs.routes={}",
        bounded_join(&facts.routes, "none")
    ));
    lines.push(format!(
        "nextjs.layouts={}",
        bounded_join(&facts.layouts, "none")
    ));
    lines.push(format!(
        "nextjs.app_root={}",
        selected_nextjs_root(&facts).unwrap_or_else(|| "unknown".to_string())
    ));
    lines.push(format!(
        "nextjs.configs={}",
        bounded_join(&facts.config_files, "none")
    ));
    lines.push(format!(
        "nextjs.tailwind_css={}",
        bounded_join(&facts.tailwind_css_files, "none")
    ));
    push_optional_fact(
        &mut lines,
        "nextjs.tsconfig.rootDir",
        facts.tsconfig_root_dir.as_deref(),
    );
    lines.push(format!(
        "nextjs.tsconfig.alias=@/*:{}",
        if facts.tsconfig_has_alias {
            "present"
        } else {
            "missing"
        }
    ));
    ProfileFactSummary {
        lines: lines.into_iter().take(16).collect(),
    }
}

fn verify_nextjs_profile(
    cwd: &Path,
    context: &ProfileVerificationContext,
) -> Vec<ProfileVerificationFailure> {
    let facts = collect_nextjs_facts(cwd);
    let mut failures = Vec::new();

    let has_app = !facts.routes.is_empty() || !facts.layouts.is_empty();
    let root = selected_nextjs_root(&facts);
    if root.as_deref() == Some("mixed") {
        failures.push(ProfileVerificationFailure::new(
            "nextjs_app_root_ambiguous",
            "both root app and src/app routes are present without an explicit migration boundary",
            facts.routes.clone(),
        ));
    }

    if let Some(build) = &facts.scripts_build {
        if build.trim() != "next build" {
            failures.push(ProfileVerificationFailure::new(
                "nextjs_build_script_drift",
                format!("scripts.build must remain `next build`, got `{build}`"),
                vec!["package.json".to_string()],
            ));
        }
    } else if facts.package_json && has_app {
        failures.push(ProfileVerificationFailure::new(
            "nextjs_build_script_drift",
            "package.json is missing scripts.build for a Next.js app",
            vec!["package.json".to_string()],
        ));
    }

    if context_requires_literal(context, "3011") {
        match &facts.scripts_dev {
            Some(dev) if dev.contains("next dev") && dev.contains("3011") => {}
            Some(dev) => failures.push(ProfileVerificationFailure::new(
                "nextjs_dev_port_drift",
                format!("scripts.dev must preserve requested port 3011, got `{dev}`"),
                vec!["package.json".to_string()],
            )),
            None if facts.package_json => failures.push(ProfileVerificationFailure::new(
                "nextjs_dev_port_drift",
                "package.json is missing scripts.dev for requested port 3011",
                vec!["package.json".to_string()],
            )),
            None => {}
        }
    }

    if facts.package_json && has_app {
        for dep in ["next", "react", "react-dom"] {
            if !facts.dependencies.contains(dep) {
                failures.push(ProfileVerificationFailure::new(
                    "nextjs_missing_dependency",
                    format!("package.json is missing `{dep}`"),
                    vec!["package.json".to_string()],
                ));
            }
        }
        if let Some(message) = nextjs_dependency_version_conflict(&facts) {
            failures.push(ProfileVerificationFailure::new(
                "nextjs_dependency_version_conflict",
                message,
                vec!["package.json".to_string()],
            ));
        }
        if let Some(message) = nextjs_tailwind_dependency_version_conflict(&facts) {
            failures.push(ProfileVerificationFailure::new(
                "nextjs_dependency_version_conflict",
                message,
                vec!["package.json".to_string()],
            ));
        }
        if let Some(message) = nextjs_typescript_toolchain_version_conflict(&facts) {
            failures.push(ProfileVerificationFailure::new(
                "nextjs_dependency_version_conflict",
                message,
                vec!["package.json".to_string()],
            ));
        }
    }

    if !facts.tailwind_css_files.is_empty() {
        for dep in ["tailwindcss", "postcss", "autoprefixer"] {
            if !facts.dependencies.contains(dep) {
                failures.push(ProfileVerificationFailure::new(
                    "nextjs_tailwind_contract",
                    format!("Tailwind directives require `{dep}` in package.json"),
                    vec!["package.json".to_string()],
                ));
            }
        }
        if !facts
            .config_files
            .iter()
            .any(|path| path.starts_with("tailwind.config."))
        {
            failures.push(ProfileVerificationFailure::new(
                "nextjs_tailwind_contract",
                "Tailwind directives require tailwind.config.*",
                facts.tailwind_css_files.clone(),
            ));
        }
        if !facts
            .config_files
            .iter()
            .any(|path| path.starts_with("postcss.config."))
        {
            failures.push(ProfileVerificationFailure::new(
                "nextjs_tailwind_contract",
                "Tailwind directives require postcss.config.*",
                facts.tailwind_css_files.clone(),
            ));
        }
        for path in facts
            .config_files
            .iter()
            .filter(|path| path.starts_with("postcss.config."))
        {
            let text = fs::read_to_string(cwd.join(path)).unwrap_or_default();
            if text.contains("@tailwindcss/postcss")
                && !facts.dependencies.contains("@tailwindcss/postcss")
            {
                failures.push(ProfileVerificationFailure::new(
                    "nextjs_tailwind_contract",
                    format!(
                        "{path} uses @tailwindcss/postcss, so package.json must include `@tailwindcss/postcss`"
                    ),
                    vec!["package.json".to_string(), path.clone()],
                ));
            }
            if facts.dependencies.contains("@tailwindcss/postcss")
                && postcss_config_uses_tailwindcss_directly(&text)
            {
                failures.push(ProfileVerificationFailure::new(
                    "nextjs_tailwind_contract",
                    format!(
                        "{path} must use @tailwindcss/postcss instead of tailwindcss directly as a PostCSS plugin"
                    ),
                    vec![path.clone()],
                ));
            }
        }
    }

    if root.as_deref() == Some("app")
        && facts
            .tsconfig_root_dir
            .as_deref()
            .is_some_and(|root_dir| root_dir == "src" || root_dir == "./src")
    {
        failures.push(ProfileVerificationFailure::new(
            "nextjs_tsconfig_excludes_route",
            "tsconfig compilerOptions.rootDir points at src while the selected route root is app",
            vec!["tsconfig.json".to_string(), "app/page.tsx".to_string()],
        ));
    }

    let selected_route = selected_route_file(&facts);
    if let Some(route) = selected_route.as_deref() {
        let route_text = fs::read_to_string(cwd.join(route)).unwrap_or_default();
        let route_graph = nextjs_route_graph(cwd, route);
        if (route_text.contains("\"@/") || route_text.contains("'@/")) && !facts.tsconfig_has_alias
        {
            failures.push(ProfileVerificationFailure::new(
                "nextjs_alias_missing",
                "route uses @/* imports but tsconfig compilerOptions.paths does not define @/*",
                vec![route.to_string(), "tsconfig.json".to_string()],
            ));
        }

        for explicit_path in explicit_integration_paths(context) {
            if explicit_path == route {
                continue;
            }
            if !cwd.join(&explicit_path).exists() {
                failures.push(ProfileVerificationFailure::new(
                    "nextjs_integration_artifact_missing",
                    format!(
                        "explicit artifact `{explicit_path}` does not exist before route integration with selected route `{route}`"
                    ),
                    vec![explicit_path, route.to_string()],
                ));
                continue;
            }
            if !route_graph_integrates_artifact(cwd, &route_graph, &explicit_path) {
                let repair_target = route_graph_repair_target(&route_graph, &explicit_path)
                    .unwrap_or_else(|| route.to_string());
                let mut paths = vec![route.to_string(), explicit_path.clone()];
                if repair_target != route {
                    paths.push(repair_target.clone());
                }
                failures.push(ProfileVerificationFailure::new(
                    "nextjs_route_not_integrated",
                    format!(
                        "explicit artifact `{explicit_path}` is not referenced from selected route graph rooted at `{route}`; repair target `{repair_target}`"
                    ),
                    paths,
                ));
            }
        }
    }

    failures
}

fn nextjs_profile_obligations(context: &ProfileObligationContext) -> Vec<ProfileObligation> {
    let mut obligations = Vec::new();
    if nextjs_app_requested(context) {
        obligations.push(ProfileObligation::new(
            "nextjs_build_script_required",
            "package.json scripts.build must remain an honest Next.js build",
            vec!["package.json".to_string()],
            Some("scripts.build == next build".to_string()),
        ));
        obligations.push(ProfileObligation::new(
            "nextjs_dependencies_required",
            "package.json dependencies must include compatible runtime packages for a Next.js app",
            vec!["package.json".to_string()],
            Some("dependencies include next, react, react-dom with React 18.2 or newer compatibility".to_string()),
        ));
    }
    if obligation_context_requires_literal(context, "3011") {
        obligations.push(ProfileObligation::new(
            "nextjs_dev_port_required",
            "package.json scripts.dev must preserve the requested development port",
            vec!["package.json".to_string()],
            Some("scripts.dev contains next dev and 3011".to_string()),
        ));
    }
    if nextjs_tailwind_requested(context) {
        obligations.push(ProfileObligation::new(
            "nextjs_tailwind_dependencies_required",
            "package.json dependencies must include Tailwind runtime tooling when Tailwind directives or config are requested",
            vec!["package.json".to_string()],
            Some("dependencies include compatible tailwindcss, postcss, autoprefixer versions".to_string()),
        ));
    }
    if let Some((route, artifact)) = nextjs_route_integration_obligation_target(context) {
        obligations.push(ProfileObligation::new(
            "nextjs_route_integration_required",
            "selected Next.js route must import or reference explicit UI/game source artifacts",
            vec![route.clone(), artifact.clone()],
            Some(format!(
                "selected route `{route}` references `{artifact}` or its module name"
            )),
        ));
    }
    obligations
}

fn nextjs_app_requested(context: &ProfileObligationContext) -> bool {
    let goal = context.goal_excerpt.to_ascii_lowercase();
    if goal.contains("next.js") || goal.contains("nextjs") || goal.contains("next js") {
        return true;
    }
    context
        .required_artifacts
        .iter()
        .chain(context.phase_contract_facts.iter())
        .chain(context.profile_facts.iter())
        .any(|value| {
            value.contains("app/page.")
                || value.contains("src/app/page.")
                || value.contains("pages/index.")
                || (value.contains("nextjs.routes=") && !value.contains("nextjs.routes=none"))
                || (value.contains("nextjs.app_root=")
                    && !value.contains("nextjs.app_root=unknown"))
        })
}

fn nextjs_tailwind_requested(context: &ProfileObligationContext) -> bool {
    let goal = context.goal_excerpt.to_ascii_lowercase();
    if goal.contains("tailwind") {
        return true;
    }
    context
        .required_artifacts
        .iter()
        .chain(context.phase_contract_facts.iter())
        .chain(context.profile_facts.iter())
        .any(|value| {
            let lower = value.to_ascii_lowercase();
            lower.contains("tailwind.config")
                || lower.contains("postcss.config")
                || (lower.contains("nextjs.tailwind_css=")
                    && !lower.contains("nextjs.tailwind_css=none"))
                || lower.contains("@tailwind")
        })
}

fn obligation_context_requires_literal(context: &ProfileObligationContext, literal: &str) -> bool {
    context.goal_excerpt.contains(literal)
        || context
            .required_artifacts
            .iter()
            .chain(context.phase_contract_facts.iter())
            .chain(context.profile_facts.iter())
            .any(|value| value.contains(literal))
}

fn nextjs_route_integration_obligation_target(
    context: &ProfileObligationContext,
) -> Option<(String, String)> {
    let route = selected_nextjs_route_from_obligation_context(context)?;
    let artifact = explicit_obligation_integration_paths(context)
        .into_iter()
        .find(|path| path != &route)?;
    Some((route, artifact))
}

fn selected_nextjs_route_from_obligation_context(
    context: &ProfileObligationContext,
) -> Option<String> {
    let mut root = None;
    let mut routes = Vec::new();
    for value in context
        .required_artifacts
        .iter()
        .chain(context.phase_contract_facts.iter())
        .chain(context.profile_facts.iter())
    {
        if let Some(value) = value.strip_prefix("nextjs.app_root=")
            && !matches!(value, "unknown" | "mixed")
        {
            root = Some(value.to_string());
        }
        if let Some(value) = value.strip_prefix("nextjs.routes=") {
            for route in value.split(',') {
                if route != "none" && is_nextjs_route_infra_path(route) {
                    routes.push(route.to_string());
                }
            }
        }
        for token in value.split([',', ' ', '=']) {
            if is_nextjs_route_infra_path(token) && !routes.iter().any(|route| route == token) {
                routes.push(token.to_string());
            }
        }
    }
    if let Some(root) = root {
        routes
            .into_iter()
            .find(|route| nextjs_route_matches_root(route, &root))
    } else {
        routes.into_iter().next()
    }
}

fn explicit_obligation_integration_paths(context: &ProfileObligationContext) -> Vec<String> {
    let mut paths = context
        .required_artifacts
        .iter()
        .filter(|path| {
            nextjs_route_integration_candidate_with_provenance(
                path,
                ArtifactProvenance::PhaseRequiredArtifact,
            )
        })
        .cloned()
        .collect::<Vec<_>>();
    paths.sort();
    paths.dedup();
    paths
}

fn collect_nextjs_facts(cwd: &Path) -> NextJsFacts {
    let mut facts = NextJsFacts::default();
    let package_path = cwd.join("package.json");
    facts.package_json = package_path.exists();
    if let Ok(text) = fs::read_to_string(&package_path)
        && let Ok(json) = serde_json::from_str::<Value>(&text)
    {
        facts.scripts_dev = json_string_at(&json, &["scripts", "dev"]);
        facts.scripts_build = json_string_at(&json, &["scripts", "build"]);
        collect_dependency_keys(
            &mut facts.dependencies,
            &mut facts.dependency_versions,
            &json,
            "dependencies",
        );
        collect_dependency_keys(
            &mut facts.dependencies,
            &mut facts.dependency_versions,
            &json,
            "devDependencies",
        );
    }

    for path in [
        "app/page.tsx",
        "app/page.jsx",
        "src/app/page.tsx",
        "src/app/page.jsx",
        "pages/index.tsx",
        "pages/index.jsx",
        "src/pages/index.tsx",
        "src/pages/index.jsx",
    ] {
        if cwd.join(path).exists() {
            facts.routes.push(path.to_string());
        }
    }
    for path in [
        "app/layout.tsx",
        "app/layout.jsx",
        "src/app/layout.tsx",
        "src/app/layout.jsx",
    ] {
        if cwd.join(path).exists() {
            facts.layouts.push(path.to_string());
        }
    }
    for path in [
        "next.config.js",
        "next.config.mjs",
        "next.config.ts",
        "tailwind.config.js",
        "tailwind.config.cjs",
        "tailwind.config.mjs",
        "tailwind.config.ts",
        "postcss.config.js",
        "postcss.config.cjs",
        "postcss.config.mjs",
        "tsconfig.json",
    ] {
        if cwd.join(path).exists() {
            facts.config_files.push(path.to_string());
        }
    }
    for path in [
        "app/globals.css",
        "src/app/globals.css",
        "styles/globals.css",
        "src/styles/globals.css",
    ] {
        if fs::read_to_string(cwd.join(path))
            .map(|text| text.contains("@tailwind "))
            .unwrap_or(false)
        {
            facts.tailwind_css_files.push(path.to_string());
        }
    }
    if let Ok(text) = fs::read_to_string(cwd.join("tsconfig.json"))
        && let Ok(json) = serde_json::from_str::<Value>(&text)
    {
        facts.tsconfig_root_dir = json_string_at(&json, &["compilerOptions", "rootDir"]);
        facts.tsconfig_has_alias = json
            .pointer("/compilerOptions/paths/@~1*")
            .or_else(|| json.pointer("/compilerOptions/paths/@/*"))
            .is_some();
    }

    facts
}

fn json_string_at(json: &Value, path: &[&str]) -> Option<String> {
    let mut current = json;
    for part in path {
        current = current.get(*part)?;
    }
    current.as_str().map(ToString::to_string)
}

fn collect_dependency_keys(
    out: &mut BTreeSet<String>,
    versions: &mut BTreeMap<String, String>,
    json: &Value,
    key: &str,
) {
    let Some(object) = json.get(key).and_then(Value::as_object) else {
        return;
    };
    for (name, value) in object {
        out.insert(name.clone());
        if let Some(version) = value.as_str() {
            versions.insert(name.clone(), version.to_string());
        }
    }
}

fn nextjs_dependency_version_conflict(facts: &NextJsFacts) -> Option<String> {
    let next = facts.dependency_versions.get("next")?;
    if !version_major_at_least(next, 14) {
        return None;
    }
    let mut conflicts = Vec::new();
    for dep in ["react", "react-dom"] {
        if facts
            .dependency_versions
            .get(dep)
            .and_then(|version| exact_version_tuple(version))
            .is_some_and(|version| version < (18, 2, 0))
        {
            conflicts.push(format!(
                "{}@{}",
                dep,
                facts.dependency_versions.get(dep).unwrap()
            ));
        }
    }
    if conflicts.is_empty() {
        None
    } else {
        Some(format!(
            "Next.js 14 requires React peer versions compatible with 18.2 or newer; incompatible exact pins: {}",
            conflicts.join(", ")
        ))
    }
}

fn nextjs_tailwind_dependency_version_conflict(facts: &NextJsFacts) -> Option<String> {
    let autoprefixer = facts.dependency_versions.get("autoprefixer")?;
    let postcss = facts.dependency_versions.get("postcss")?;
    let autoprefixer_exact = exact_version_tuple(autoprefixer)?;
    let postcss_exact = exact_version_tuple(postcss)?;
    if autoprefixer_exact.0 >= 10 && postcss_exact < (8, 0, 2) {
        Some(format!(
            "autoprefixer@{autoprefixer} requires postcss peer range ^8.0.2 or newer; incompatible exact pin: postcss@{postcss}"
        ))
    } else {
        None
    }
}

fn nextjs_typescript_toolchain_version_conflict(facts: &NextJsFacts) -> Option<String> {
    let next = facts.dependency_versions.get("next")?;
    if !version_major_at_least(next, 14) {
        return None;
    }

    let mut conflicts = Vec::new();
    if facts
        .dependency_versions
        .get("typescript")
        .and_then(|version| version_major(version))
        .is_some_and(|major| major >= 6)
    {
        conflicts.push(format!(
            "typescript@{}",
            facts.dependency_versions.get("typescript").unwrap()
        ));
    }
    if facts
        .dependency_versions
        .get("typescript")
        .is_some_and(|version| version.trim() == "5.0.0")
    {
        conflicts.push("typescript@5.0.0".to_string());
    }

    let react_major = facts
        .dependency_versions
        .get("react")
        .and_then(|version| version_major(version));
    let react_types_major = facts
        .dependency_versions
        .get("@types/react")
        .and_then(|version| version_major(version));
    if react_major == Some(18) && react_types_major.is_some_and(|major| major >= 19) {
        conflicts.push(format!(
            "@types/react@{}",
            facts.dependency_versions.get("@types/react").unwrap()
        ));
    }

    if conflicts.is_empty() {
        None
    } else {
        Some(format!(
            "Next.js 14 generated TypeScript apps should use a stable TypeScript 5.x and React 18 type family; incompatible generated toolchain pins: {}",
            conflicts.join(", ")
        ))
    }
}

fn postcss_config_uses_tailwindcss_directly(text: &str) -> bool {
    text.contains("tailwindcss") && !text.contains("@tailwindcss/postcss")
}

fn version_major(value: &str) -> Option<u64> {
    value
        .trim_start_matches(['^', '~', 'v'])
        .split('.')
        .next()
        .and_then(|part| part.parse::<u64>().ok())
}

fn version_major_at_least(value: &str, minimum: u64) -> bool {
    version_major(value).is_some_and(|major| major >= minimum)
}

fn exact_version_tuple(value: &str) -> Option<(u64, u64, u64)> {
    let first = value.chars().next()?;
    if !first.is_ascii_digit() {
        return None;
    }
    let mut parts = value.split('.');
    let major = parts.next()?.parse::<u64>().ok()?;
    let minor = parts.next().unwrap_or("0").parse::<u64>().ok()?;
    let patch_part = parts.next().unwrap_or("0");
    let patch = patch_part
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>()
        .parse::<u64>()
        .ok()?;
    Some((major, minor, patch))
}

fn selected_nextjs_root(facts: &NextJsFacts) -> Option<String> {
    let has_app = facts
        .routes
        .iter()
        .chain(facts.layouts.iter())
        .any(|path| path.starts_with("app/"));
    let has_src_app = facts
        .routes
        .iter()
        .chain(facts.layouts.iter())
        .any(|path| path.starts_with("src/app/"));
    let has_pages = facts.routes.iter().any(|path| path.contains("pages/"));
    match (has_app, has_src_app, has_pages) {
        (true, false, false) => Some("app".to_string()),
        (false, true, false) => Some("src/app".to_string()),
        (false, false, true) => Some("pages".to_string()),
        (false, false, false) => None,
        _ => Some("mixed".to_string()),
    }
}

fn selected_route_file(facts: &NextJsFacts) -> Option<String> {
    let root = selected_nextjs_root(facts)?;
    if root == "mixed" {
        return None;
    }
    facts
        .routes
        .iter()
        .find(|path| match root.as_str() {
            "app" => path.starts_with("app/"),
            "src/app" => path.starts_with("src/app/"),
            "pages" => path.contains("pages/"),
            _ => false,
        })
        .cloned()
}

pub(crate) fn nextjs_selected_route_from_workspace(cwd: &Path) -> Option<String> {
    selected_route_file(&collect_nextjs_facts(cwd))
}

fn nextjs_route_matches_root(route: &str, root: &str) -> bool {
    match root {
        "app" => route.starts_with("app/"),
        "src/app" => route.starts_with("src/app/"),
        "pages" => route.contains("pages/"),
        _ => false,
    }
}

fn context_requires_literal(context: &ProfileVerificationContext, literal: &str) -> bool {
    context.goal_excerpt.contains(literal)
        || context
            .required_artifacts
            .iter()
            .chain(context.expected_paths.iter())
            .chain(context.phase_contract_facts.iter())
            .chain(context.profile_facts.iter())
            .any(|value| value.contains(literal))
}

fn explicit_integration_paths(context: &ProfileVerificationContext) -> Vec<String> {
    let mut paths = context
        .required_artifacts
        .iter()
        .filter(|path| {
            nextjs_route_integration_candidate_with_provenance(
                path,
                ArtifactProvenance::PhaseRequiredArtifact,
            )
        })
        .chain(context.expected_paths.iter().filter(|path| {
            nextjs_route_integration_candidate_with_provenance(
                path,
                ArtifactProvenance::StepExpectedPath,
            )
        }))
        .cloned()
        .collect::<Vec<_>>();
    paths.sort();
    paths.dedup();
    paths
}

fn is_nextjs_route_infra_path(path: &str) -> bool {
    matches!(
        path,
        "app/page.tsx"
            | "app/page.jsx"
            | "app/layout.tsx"
            | "app/layout.jsx"
            | "src/app/page.tsx"
            | "src/app/page.jsx"
            | "src/app/layout.tsx"
            | "src/app/layout.jsx"
            | "pages/index.tsx"
            | "pages/index.jsx"
            | "src/pages/index.tsx"
            | "src/pages/index.jsx"
    )
}

pub(crate) fn nextjs_route_integration_candidate(path: &str) -> bool {
    nextjs_route_integration_candidate_with_provenance(path, ArtifactProvenance::StepExpectedPath)
}

fn nextjs_route_integration_candidate_with_provenance(
    path: &str,
    provenance: ArtifactProvenance,
) -> bool {
    classify_profile_artifact(ProfileId::NextJs, path, provenance)
        .eligibility
        .route_integration
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NextJsRouteGraph {
    selected_route: String,
    files: Vec<String>,
}

fn nextjs_route_graph(cwd: &Path, selected_route: &str) -> NextJsRouteGraph {
    let mut files = Vec::new();
    let mut seen = BTreeSet::new();
    let mut queue = vec![(selected_route.to_string(), 0usize)];
    while let Some((path, depth)) = queue.pop() {
        if files.len() >= NEXTJS_ROUTE_GRAPH_MAX_FILES || depth > NEXTJS_ROUTE_GRAPH_MAX_DEPTH {
            continue;
        }
        if !seen.insert(path.clone()) || ignored_route_graph_path(&path) {
            continue;
        }
        let absolute = cwd.join(&path);
        let Ok(text) = fs::read_to_string(&absolute) else {
            continue;
        };
        files.push(path.clone());
        if depth == NEXTJS_ROUTE_GRAPH_MAX_DEPTH {
            continue;
        }
        for import_path in static_relative_imports(&text) {
            if let Some(resolved) = resolve_route_import(cwd, &path, &import_path)
                && !seen.contains(&resolved)
            {
                queue.push((resolved, depth + 1));
            }
        }
    }
    NextJsRouteGraph {
        selected_route: selected_route.to_string(),
        files,
    }
}

fn route_graph_integrates_artifact(cwd: &Path, graph: &NextJsRouteGraph, artifact: &str) -> bool {
    if graph.files.iter().any(|file| file == artifact) {
        return true;
    }
    let stem = Path::new(artifact)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default();
    if stem.is_empty() {
        return false;
    }
    let path_without_ext = artifact
        .strip_suffix(".tsx")
        .or_else(|| artifact.strip_suffix(".jsx"))
        .or_else(|| artifact.strip_suffix(".ts"))
        .or_else(|| artifact.strip_suffix(".js"))
        .unwrap_or(artifact);
    graph.files.iter().any(|file| {
        fs::read_to_string(cwd.join(file))
            .map(|text| text.contains(stem) || text.contains(path_without_ext))
            .unwrap_or(false)
    })
}

fn route_graph_repair_target(graph: &NextJsRouteGraph, artifact: &str) -> Option<String> {
    let artifact_dir = Path::new(artifact)
        .parent()
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default();
    let component_target = graph
        .files
        .iter()
        .filter(|file| file.as_str() != graph.selected_route)
        .find(|file| {
            let lower = file.to_ascii_lowercase();
            lower.contains("/components/") || lower.starts_with("components/")
        })
        .cloned();
    if !artifact_dir.is_empty()
        && artifact_dir.contains("/hooks")
        && let Some(target) = component_target
    {
        return Some(target);
    }
    graph.files.first().cloned()
}

fn static_relative_imports(text: &str) -> Vec<String> {
    let mut imports = Vec::new();
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if !line.starts_with("import ") {
            continue;
        }
        for quote in ['\'', '"'] {
            for value in quoted_values(line, quote) {
                if value.starts_with('.') && !imports.iter().any(|existing| existing == &value) {
                    imports.push(value);
                }
            }
        }
    }
    imports
}

fn quoted_values(line: &str, quote: char) -> Vec<String> {
    let mut values = Vec::new();
    let mut rest = line;
    while let Some(start) = rest.find(quote) {
        let after_start = &rest[start + quote.len_utf8()..];
        let Some(end) = after_start.find(quote) else {
            break;
        };
        values.push(after_start[..end].to_string());
        rest = &after_start[end + quote.len_utf8()..];
    }
    values
}

fn resolve_route_import(cwd: &Path, from_file: &str, import_path: &str) -> Option<String> {
    if !import_path.starts_with('.') {
        return None;
    }
    let base_dir = Path::new(from_file)
        .parent()
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default();
    let base = normalize_route_import_path(&base_dir, import_path)?;
    for candidate in route_import_candidates(&base) {
        if ignored_route_graph_path(&candidate) {
            continue;
        }
        if cwd.join(&candidate).is_file() {
            return Some(candidate);
        }
    }
    None
}

fn normalize_route_import_path(base_dir: &str, import_path: &str) -> Option<String> {
    let combined = if base_dir.is_empty() {
        import_path.to_string()
    } else {
        format!("{base_dir}/{import_path}")
    };
    let mut parts = Vec::new();
    for part in combined.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            value => parts.push(value),
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("/"))
    }
}

fn route_import_candidates(base: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    if Path::new(base).extension().is_some() {
        candidates.push(base.to_string());
    } else {
        candidates.push(base.to_string());
        for ext in ["tsx", "ts", "jsx", "js"] {
            candidates.push(format!("{base}.{ext}"));
        }
        for ext in ["tsx", "ts", "jsx", "js"] {
            candidates.push(format!("{base}/index.{ext}"));
        }
    }
    candidates
}

fn ignored_route_graph_path(path: &str) -> bool {
    let classified = classify_profile_artifact(
        ProfileId::NextJs,
        path,
        ArtifactProvenance::WorkspaceObservation,
    );
    matches!(
        classified.kind,
        crate::agent::step_runner::profile_artifact::ArtifactKind::DependencyCache
            | crate::agent::step_runner::profile_artifact::ArtifactKind::BuildOutput
            | crate::agent::step_runner::profile_artifact::ArtifactKind::GeneratedDeclaration
            | crate::agent::step_runner::profile_artifact::ArtifactKind::Manifest
            | crate::agent::step_runner::profile_artifact::ArtifactKind::Config
    )
}

fn push_optional_fact(lines: &mut Vec<String>, key: &str, value: Option<&str>) {
    if let Some(value) = value {
        lines.push(format!("{key}={}", bounded_value(value)));
    }
}

fn bounded_join(values: &[String], empty: &str) -> String {
    if values.is_empty() {
        return empty.to_string();
    }
    values
        .iter()
        .take(8)
        .map(|value| bounded_value(value))
        .collect::<Vec<_>>()
        .join(",")
}

fn bounded_value(value: &str) -> String {
    const MAX: usize = 120;
    let mut out = value.chars().take(MAX).collect::<String>();
    if value.chars().count() > MAX {
        out.push_str("...");
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileError {
    UnknownProfile(String),
}

impl std::fmt::Display for ProfileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownProfile(profile) => write!(f, "unknown profile: {profile}"),
        }
    }
}

impl std::error::Error for ProfileError {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};

    #[test]
    fn parses_all_mvp_profiles() {
        for profile in [
            "generic",
            "nextjs",
            "python",
            "rust",
            "investigation",
            "docs",
            "data-analysis",
            "data-pipeline",
        ] {
            assert!(ProfileId::parse(profile).is_ok(), "{profile}");
        }
    }

    #[test]
    fn nextjs_profile_preserves_honest_build_contract() {
        let contract = profile_contract(ProfileId::NextJs);

        assert!(contract.text.contains("next/react/react-dom"));
        assert!(contract.text.contains("never fake build success"));
        assert_eq!(contract.verifier_commands, vec!["npm run build"]);
    }

    #[test]
    fn rust_profile_keeps_scaffolding_in_file_tools() {
        let contract = profile_contract(ProfileId::Rust);

        assert!(contract.text.contains("Cargo.toml"));
        assert!(contract.text.contains("src/main.rs"));
        assert!(contract.text.contains("instead of cargo init or cargo new"));
        assert!(contract.text.contains("CARGO_BIN_EXE_<name>"));
        assert!(contract.text.contains("matches the Cargo binary name"));
    }

    #[test]
    fn data_profiles_protect_raw_inputs() {
        assert!(protected_by_profile("data-analysis", "data/raw/source.csv").unwrap());
        assert!(protected_by_profile("data-pipeline", "raw/source.csv").unwrap());
        assert!(!protected_by_profile("data-analysis", "derived/report.csv").unwrap());
    }

    #[test]
    fn unknown_profile_is_error() {
        let err = ProfileId::parse("legacy").unwrap_err();

        assert_eq!(err, ProfileError::UnknownProfile("legacy".to_string()));
    }

    #[test]
    fn nextjs_summary_reports_root_app() {
        let root = temp_workspace("summary-root-app");
        fs::create_dir_all(root.join("app")).unwrap();
        fs::write(
            root.join("app/page.tsx"),
            "export default function Page() {}",
        )
        .unwrap();
        fs::write(
            root.join("package.json"),
            r#"{"scripts":{"dev":"next dev -p 3011","build":"next build"},"dependencies":{"next":"latest","react":"latest","react-dom":"latest"}}"#,
        )
        .unwrap();

        let summary = profile_fact_summary("nextjs", &root).unwrap();

        assert!(summary.lines.contains(&"nextjs.app_root=app".to_string()));
        assert!(
            summary
                .lines
                .contains(&"nextjs.scripts.dev=next dev -p 3011".to_string())
        );
    }

    #[test]
    fn nextjs_summary_reports_mixed_roots() {
        let root = temp_workspace("summary-mixed");
        fs::create_dir_all(root.join("app")).unwrap();
        fs::create_dir_all(root.join("src/app")).unwrap();
        fs::write(
            root.join("app/page.tsx"),
            "export default function Page() {}",
        )
        .unwrap();
        fs::write(
            root.join("src/app/page.tsx"),
            "export default function Page() {}",
        )
        .unwrap();

        let summary = profile_fact_summary("nextjs", &root).unwrap();

        assert!(summary.lines.contains(&"nextjs.app_root=mixed".to_string()));
    }

    #[test]
    fn nextjs_obligations_include_requested_dev_port() {
        let obligations = profile_obligations(
            "nextjs",
            &obligation_context_with_goal("Create a Next.js app on port 3011"),
        )
        .unwrap();

        assert!(
            obligations
                .iter()
                .any(|obligation| obligation.code == "nextjs_dev_port_required")
        );
    }

    #[test]
    fn nextjs_obligations_include_build_and_dependencies_for_app_artifact() {
        let mut context = obligation_context_with_goal("Create app");
        context.required_artifacts = vec!["app/page.tsx".to_string()];

        let obligations = profile_obligations("nextjs", &context).unwrap();

        assert!(
            obligations
                .iter()
                .any(|obligation| obligation.code == "nextjs_build_script_required")
        );
        assert!(
            obligations
                .iter()
                .any(|obligation| obligation.code == "nextjs_dependencies_required")
        );
    }

    #[test]
    fn nextjs_obligations_include_tailwind_dependencies_when_requested() {
        let obligations = profile_obligations(
            "nextjs",
            &obligation_context_with_goal("Create a Next.js app with Tailwind CSS"),
        )
        .unwrap();

        let obligation = obligations
            .iter()
            .find(|obligation| obligation.code == "nextjs_tailwind_dependencies_required")
            .expect("tailwind dependency obligation");
        assert_eq!(
            obligation.expected.as_deref(),
            Some("dependencies include compatible tailwindcss, postcss, autoprefixer versions")
        );
    }

    #[test]
    fn nextjs_obligations_include_route_integration_for_explicit_source_artifact() {
        let mut context = obligation_context_with_goal("Create a Next.js game");
        context.required_artifacts = vec!["app/hooks/useGame.ts".to_string()];
        context.profile_facts = vec![
            "nextjs.routes=app/page.tsx".to_string(),
            "nextjs.app_root=app".to_string(),
        ];

        let obligations = profile_obligations("nextjs", &context).unwrap();

        let obligation = obligations
            .iter()
            .find(|obligation| obligation.code == "nextjs_route_integration_required")
            .expect("route integration obligation");
        assert_eq!(
            obligation.paths,
            vec![
                "app/page.tsx".to_string(),
                "app/hooks/useGame.ts".to_string()
            ]
        );
        assert_eq!(
            obligation.expected.as_deref(),
            Some(
                "selected route `app/page.tsx` references `app/hooks/useGame.ts` or its module name"
            )
        );
    }

    #[test]
    fn nextjs_obligations_do_not_require_route_integration_for_setup_paths_only() {
        let mut context = obligation_context_with_goal("Create a Next.js app");
        context.required_artifacts = vec![
            "package.json".to_string(),
            "app/page.tsx".to_string(),
            "tailwind.config.js".to_string(),
        ];
        context.profile_facts = vec![
            "nextjs.routes=app/page.tsx".to_string(),
            "nextjs.app_root=app".to_string(),
        ];

        let obligations = profile_obligations("nextjs", &context).unwrap();

        assert!(
            obligations
                .iter()
                .all(|obligation| obligation.code != "nextjs_route_integration_required")
        );
    }

    #[test]
    fn nextjs_obligations_do_not_use_workspace_entries_for_route_integration() {
        let mut context = obligation_context_with_goal("Create a Next.js app");
        context.phase_contract_facts = vec![
            "workspace.entries=app/,components/,next-env.d.ts".to_string(),
            "workspace.entries=components/Game.tsx".to_string(),
        ];
        context.profile_facts = vec![
            "nextjs.routes=app/page.tsx".to_string(),
            "nextjs.app_root=app".to_string(),
        ];

        let obligations = profile_obligations("nextjs", &context).unwrap();

        assert!(
            obligations
                .iter()
                .all(|obligation| obligation.code != "nextjs_route_integration_required"),
            "{obligations:?}"
        );
    }

    #[test]
    fn generic_profile_has_no_profile_obligations() {
        let obligations = profile_obligations(
            "generic",
            &obligation_context_with_goal("Create a Next.js app on port 3011"),
        )
        .unwrap();

        assert!(obligations.is_empty());
    }

    #[test]
    fn renders_profile_obligations_as_bounded_fact_lines() {
        let obligations = vec![ProfileObligation {
            code: "nextjs_dev_port_required".to_string(),
            message: "x".repeat(200),
            paths: vec!["package.json".to_string()],
            expected: Some("scripts.dev contains next dev and 3011".to_string()),
        }];

        let rendered = render_profile_obligations(&obligations);

        assert_eq!(rendered.len(), 1);
        assert!(rendered[0].starts_with("profile.obligation.nextjs_dev_port_required="));
        assert!(rendered[0].contains("paths=package.json"));
        assert!(rendered[0].contains("expected=scripts.dev contains next dev and 3011"));
        assert!(rendered[0].contains("..."));
    }

    #[test]
    fn nextjs_verification_enforces_requested_port() {
        let root = temp_workspace("verify-port");
        write_minimal_next_app(&root, r#"{"dev":"next dev","build":"next build"}"#);

        let failures =
            verify_profile("nextjs", &root, &context_with_goal("run on port 3011")).unwrap();

        assert!(
            failures
                .iter()
                .any(|failure| failure.code == "nextjs_dev_port_drift")
        );
    }

    #[test]
    fn nextjs_verification_rejects_build_script_drift() {
        let root = temp_workspace("verify-build-script");
        write_minimal_next_app(&root, r#"{"dev":"next dev -p 3011","build":"echo ok"}"#);

        let failures =
            verify_profile("nextjs", &root, &context_with_goal("run on port 3011")).unwrap();

        assert!(
            failures
                .iter()
                .any(|failure| failure.code == "nextjs_build_script_drift")
        );
    }

    #[test]
    fn nextjs_verification_rejects_next14_react18_0_exact_pin() {
        let root = temp_workspace("verify-peer-version");
        fs::create_dir_all(root.join("app")).unwrap();
        fs::write(
            root.join("app/page.tsx"),
            "export default function Page() {}",
        )
        .unwrap();
        fs::write(
            root.join("package.json"),
            r#"{"scripts":{"dev":"next dev -p 3011","build":"next build"},"dependencies":{"next":"14.0.0","react":"18.0.0","react-dom":"18.0.0"}}"#,
        )
        .unwrap();

        let failures =
            verify_profile("nextjs", &root, &context_with_goal("run on port 3011")).unwrap();

        assert!(
            failures
                .iter()
                .any(|failure| failure.code == "nextjs_dependency_version_conflict")
        );
    }

    #[test]
    fn nextjs_verification_allows_react_version_range() {
        let root = temp_workspace("verify-peer-range");
        fs::create_dir_all(root.join("app")).unwrap();
        fs::write(
            root.join("app/page.tsx"),
            "export default function Page() {}",
        )
        .unwrap();
        fs::write(
            root.join("package.json"),
            r#"{"scripts":{"dev":"next dev -p 3011","build":"next build"},"dependencies":{"next":"14.0.0","react":"^18.0.0","react-dom":"^18.0.0"}}"#,
        )
        .unwrap();

        let failures =
            verify_profile("nextjs", &root, &context_with_goal("run on port 3011")).unwrap();

        assert!(
            failures
                .iter()
                .all(|failure| failure.code != "nextjs_dependency_version_conflict")
        );
    }

    #[test]
    fn nextjs_verification_rejects_postcss_autoprefixer_peer_conflict() {
        let root = temp_workspace("verify-postcss-autoprefixer-conflict");
        write_next_app_with_dependencies(
            &root,
            r#"{"next":"14.0.0","react":"18.2.0","react-dom":"18.2.0","tailwindcss":"3.3.0","postcss":"8.0.0","autoprefixer":"10.0.0"}"#,
        );

        let failures =
            verify_profile("nextjs", &root, &context_with_goal("run on port 3011")).unwrap();

        let failure = failures
            .iter()
            .find(|failure| failure.code == "nextjs_dependency_version_conflict")
            .expect("dependency conflict failure");
        assert!(failure.message.contains("autoprefixer@10.0.0"));
        assert!(failure.message.contains("postcss peer range ^8.0.2"));
        assert!(failure.message.contains("postcss@8.0.0"));
        assert_eq!(failure.paths, vec!["package.json".to_string()]);
    }

    #[test]
    fn nextjs_verification_accepts_compatible_postcss_autoprefixer_versions() {
        let root = temp_workspace("verify-postcss-autoprefixer-compatible");
        write_next_app_with_dependencies(
            &root,
            r#"{"next":"14.0.0","react":"18.2.0","react-dom":"18.2.0","tailwindcss":"3.3.0","postcss":"8.4.31","autoprefixer":"10.4.16"}"#,
        );

        let failures =
            verify_profile("nextjs", &root, &context_with_goal("run on port 3011")).unwrap();

        assert!(
            failures
                .iter()
                .all(|failure| failure.code != "nextjs_dependency_version_conflict"),
            "{failures:?}"
        );
    }

    #[test]
    fn nextjs_verification_accepts_postcss_version_range_for_tailwind_stack() {
        let root = temp_workspace("verify-postcss-autoprefixer-range");
        write_next_app_with_dependencies(
            &root,
            r#"{"next":"14.0.0","react":"18.2.0","react-dom":"18.2.0","tailwindcss":"^3.3.0","postcss":"^8.4.0","autoprefixer":"^10.4.0"}"#,
        );

        let failures =
            verify_profile("nextjs", &root, &context_with_goal("run on port 3011")).unwrap();

        assert!(
            failures
                .iter()
                .all(|failure| failure.code != "nextjs_dependency_version_conflict"),
            "{failures:?}"
        );
    }

    #[test]
    fn nextjs_verification_rejects_typescript_6_for_next14_generated_app() {
        let root = temp_workspace("verify-typescript-major");
        write_next_app_with_dependencies(
            &root,
            r#"{"next":"14.0.0","react":"^18.2.0","react-dom":"^18.2.0","typescript":"6.0.3","@types/react":"18.2.79"}"#,
        );

        let failures =
            verify_profile("nextjs", &root, &context_with_goal("run on port 3011")).unwrap();

        let failure = failures
            .iter()
            .find(|failure| failure.code == "nextjs_dependency_version_conflict")
            .expect("dependency conflict failure");
        assert!(failure.message.contains("typescript@6.0.3"));
        assert!(failure.message.contains("TypeScript 5.x"));
        assert_eq!(failure.paths, vec!["package.json".to_string()]);
    }

    #[test]
    fn nextjs_verification_rejects_unstable_typescript_5_0_0_pin() {
        let root = temp_workspace("verify-typescript-5-0-0");
        write_next_app_with_dependencies(
            &root,
            r#"{"next":"14.0.0","react":"^18.2.0","react-dom":"^18.2.0","typescript":"5.0.0","@types/react":"18.2.79"}"#,
        );

        let failures =
            verify_profile("nextjs", &root, &context_with_goal("run on port 3011")).unwrap();

        let failure = failures
            .iter()
            .find(|failure| failure.code == "nextjs_dependency_version_conflict")
            .expect("dependency conflict failure");
        assert!(failure.message.contains("typescript@5.0.0"));
        assert!(failure.message.contains("stable TypeScript 5.x"));
        assert_eq!(failure.paths, vec!["package.json".to_string()]);
    }

    #[test]
    fn nextjs_verification_rejects_react19_types_for_react18_generated_app() {
        let root = temp_workspace("verify-react-types-major");
        write_next_app_with_dependencies(
            &root,
            r#"{"next":"14.0.0","react":"^18.2.0","react-dom":"^18.2.0","typescript":"^5.4.0","@types/react":"19.2.17"}"#,
        );

        let failures =
            verify_profile("nextjs", &root, &context_with_goal("run on port 3011")).unwrap();

        let failure = failures
            .iter()
            .find(|failure| failure.code == "nextjs_dependency_version_conflict")
            .expect("dependency conflict failure");
        assert!(failure.message.contains("@types/react@19.2.17"));
        assert!(failure.message.contains("React 18 type family"));
        assert_eq!(failure.paths, vec!["package.json".to_string()]);
    }

    #[test]
    fn nextjs_verification_accepts_stable_typescript_toolchain() {
        let root = temp_workspace("verify-stable-typescript-toolchain");
        write_next_app_with_dependencies(
            &root,
            r#"{"next":"14.0.0","react":"^18.2.0","react-dom":"^18.2.0","typescript":"^5.4.0","@types/react":"^18.2.79"}"#,
        );

        let failures =
            verify_profile("nextjs", &root, &context_with_goal("run on port 3011")).unwrap();

        assert!(
            failures
                .iter()
                .all(|failure| failure.code != "nextjs_dependency_version_conflict"),
            "{failures:?}"
        );
    }

    #[test]
    fn nextjs_verification_rejects_tailwind_contract_drift() {
        let root = temp_workspace("verify-tailwind");
        write_minimal_next_app(&root, r#"{"dev":"next dev -p 3011","build":"next build"}"#);
        fs::write(root.join("app/globals.css"), "@tailwind base;").unwrap();
        fs::write(root.join("tailwind.config.js"), "module.exports = {}").unwrap();
        fs::write(root.join("postcss.config.js"), "module.exports = {}").unwrap();

        let failures =
            verify_profile("nextjs", &root, &context_with_goal("run on port 3011")).unwrap();

        assert!(
            failures
                .iter()
                .any(|failure| failure.code == "nextjs_tailwind_contract")
        );
    }

    #[test]
    fn nextjs_verification_rejects_postcss_plugin_without_declared_package() {
        let root = temp_workspace("verify-postcss-plugin-dependency");
        write_minimal_next_app(&root, r#"{"dev":"next dev -p 3011","build":"next build"}"#);
        fs::write(root.join("app/globals.css"), "@tailwind base;").unwrap();
        fs::write(root.join("tailwind.config.js"), "module.exports = {}").unwrap();
        fs::write(
            root.join("postcss.config.js"),
            "module.exports = { plugins: { '@tailwindcss/postcss': {} } }",
        )
        .unwrap();

        let failures =
            verify_profile("nextjs", &root, &context_with_goal("run on port 3011")).unwrap();

        let failure = failures
            .iter()
            .find(|failure| {
                failure.code == "nextjs_tailwind_contract"
                    && failure.message.contains("@tailwindcss/postcss")
            })
            .expect("postcss plugin dependency failure");
        assert_eq!(
            failure.paths,
            vec!["package.json".to_string(), "postcss.config.js".to_string()]
        );
    }

    #[test]
    fn nextjs_verification_rejects_direct_tailwindcss_postcss_plugin_when_v4_plugin_declared() {
        let root = temp_workspace("verify-postcss-direct-plugin");
        fs::create_dir_all(root.join("app")).unwrap();
        fs::write(
            root.join("app/page.tsx"),
            "export default function Page() {}",
        )
        .unwrap();
        fs::write(root.join("app/globals.css"), "@tailwind base;").unwrap();
        fs::write(root.join("tailwind.config.js"), "module.exports = {}").unwrap();
        fs::write(
            root.join("postcss.config.js"),
            "module.exports = { plugins: { tailwindcss: {}, autoprefixer: {} } }",
        )
        .unwrap();
        fs::write(
            root.join("package.json"),
            r#"{"scripts":{"dev":"next dev -p 3011","build":"next build"},"dependencies":{"next":"14.2.0","react":"18.2.0","react-dom":"18.2.0","tailwindcss":"latest","@tailwindcss/postcss":"latest","postcss":"latest","autoprefixer":"latest"}}"#,
        )
        .unwrap();

        let failures =
            verify_profile("nextjs", &root, &context_with_goal("run on port 3011")).unwrap();

        let failure = failures
            .iter()
            .find(|failure| {
                failure.code == "nextjs_tailwind_contract"
                    && failure.message.contains("tailwindcss directly")
            })
            .expect("direct postcss plugin failure");
        assert_eq!(failure.paths, vec!["postcss.config.js".to_string()]);
    }

    #[test]
    fn nextjs_verification_rejects_mixed_roots() {
        let root = temp_workspace("verify-mixed");
        write_minimal_next_app(&root, r#"{"dev":"next dev -p 3011","build":"next build"}"#);
        fs::create_dir_all(root.join("src/app")).unwrap();
        fs::write(
            root.join("src/app/page.tsx"),
            "export default function Page() {}",
        )
        .unwrap();

        let failures =
            verify_profile("nextjs", &root, &context_with_goal("run on port 3011")).unwrap();

        assert!(
            failures
                .iter()
                .any(|failure| failure.code == "nextjs_app_root_ambiguous")
        );
    }

    #[test]
    fn nextjs_verification_rejects_disconnected_source_artifact() {
        let root = temp_workspace("verify-route-disconnected");
        write_minimal_next_app(&root, r#"{"dev":"next dev -p 3011","build":"next build"}"#);
        fs::create_dir_all(root.join("app/hooks")).unwrap();
        fs::write(
            root.join("app/hooks/useGame.ts"),
            "export function useGame() {}",
        )
        .unwrap();
        let mut context = context_with_goal("run on port 3011");
        context.expected_paths = vec!["app/hooks/useGame.ts".to_string()];

        let failures = verify_profile("nextjs", &root, &context).unwrap();

        assert!(
            failures
                .iter()
                .any(|failure| failure.code == "nextjs_route_not_integrated"),
            "{failures:?}"
        );
    }

    #[test]
    fn nextjs_verification_reports_missing_integration_artifact_before_route_drift() {
        let root = temp_workspace("verify-missing-integration-artifact");
        write_minimal_next_app(&root, r#"{"dev":"next dev -p 3011","build":"next build"}"#);
        let mut context = context_with_goal("run on port 3011");
        context.expected_paths = vec!["components/SpaceInvaders.tsx".to_string()];

        let failures = verify_profile("nextjs", &root, &context).unwrap();

        let missing = failures
            .iter()
            .find(|failure| failure.code == "nextjs_integration_artifact_missing")
            .expect("missing integration artifact failure");
        assert_eq!(
            missing.paths,
            vec![
                "components/SpaceInvaders.tsx".to_string(),
                "app/page.tsx".to_string()
            ]
        );
    }

    #[test]
    fn nextjs_missing_integration_artifact_does_not_emit_route_not_integrated() {
        let root = temp_workspace("verify-missing-not-route-drift");
        write_minimal_next_app(&root, r#"{"dev":"next dev -p 3011","build":"next build"}"#);
        let mut context = context_with_goal("run on port 3011");
        context.required_artifacts = vec!["components/SpaceInvaders.tsx".to_string()];

        let failures = verify_profile("nextjs", &root, &context).unwrap();

        assert!(
            failures
                .iter()
                .any(|failure| failure.code == "nextjs_integration_artifact_missing"),
            "{failures:?}"
        );
        assert!(
            failures
                .iter()
                .all(|failure| failure.code != "nextjs_route_not_integrated"),
            "{failures:?}"
        );
    }

    #[test]
    fn nextjs_verification_accepts_source_artifact_stem_reference_from_route() {
        let root = temp_workspace("verify-route-stem-reference");
        write_minimal_next_app(&root, r#"{"dev":"next dev -p 3011","build":"next build"}"#);
        fs::write(
            root.join("app/page.tsx"),
            "import { useGame } from './hooks/useGame'; export default function Page() { useGame(); return null }",
        )
        .unwrap();
        fs::create_dir_all(root.join("app/hooks")).unwrap();
        fs::write(
            root.join("app/hooks/useGame.ts"),
            "export function useGame() {}",
        )
        .unwrap();
        let mut context = context_with_goal("run on port 3011");
        context.expected_paths = vec!["app/hooks/useGame.ts".to_string()];

        let failures = verify_profile("nextjs", &root, &context).unwrap();

        assert!(
            failures
                .iter()
                .all(|failure| failure.code != "nextjs_route_not_integrated"),
            "{failures:?}"
        );
    }

    #[test]
    fn nextjs_verification_accepts_source_artifact_path_reference_from_route() {
        let root = temp_workspace("verify-route-path-reference");
        write_minimal_next_app(&root, r#"{"dev":"next dev -p 3011","build":"next build"}"#);
        fs::write(
            root.join("app/page.tsx"),
            "const modulePath = 'app/hooks/useGame'; export default function Page() { return modulePath }",
        )
        .unwrap();
        fs::create_dir_all(root.join("app/hooks")).unwrap();
        fs::write(
            root.join("app/hooks/useGame.ts"),
            "export function useGame() {}",
        )
        .unwrap();
        let mut context = context_with_goal("run on port 3011");
        context.expected_paths = vec!["app/hooks/useGame.ts".to_string()];

        let failures = verify_profile("nextjs", &root, &context).unwrap();

        assert!(
            failures
                .iter()
                .all(|failure| failure.code != "nextjs_route_not_integrated"),
            "{failures:?}"
        );
    }

    #[test]
    fn nextjs_verification_accepts_transitive_route_component_integration() {
        let root = temp_workspace("verify-route-graph-transitive");
        write_minimal_next_app(&root, r#"{"dev":"next dev -p 3011","build":"next build"}"#);
        fs::create_dir_all(root.join("app/components")).unwrap();
        fs::create_dir_all(root.join("app/hooks")).unwrap();
        fs::write(
            root.join("app/page.tsx"),
            "import GameBoard from './components/GameBoard'; export default function Page() { return <GameBoard /> }",
        )
        .unwrap();
        fs::write(
            root.join("app/components/GameBoard.tsx"),
            "import { useGame } from '../hooks/useGame'; export default function GameBoard() { useGame(); return null }",
        )
        .unwrap();
        fs::write(
            root.join("app/hooks/useGame.ts"),
            "export function useGame() {}",
        )
        .unwrap();
        let mut context = context_with_goal("run on port 3011");
        context.expected_paths = vec!["app/hooks/useGame.ts".to_string()];

        let failures = verify_profile("nextjs", &root, &context).unwrap();

        assert!(
            failures
                .iter()
                .all(|failure| failure.code != "nextjs_route_not_integrated"),
            "{failures:?}"
        );
    }

    #[test]
    fn nextjs_verification_route_graph_failure_can_target_component() {
        let root = temp_workspace("verify-route-graph-target");
        write_minimal_next_app(&root, r#"{"dev":"next dev -p 3011","build":"next build"}"#);
        fs::create_dir_all(root.join("app/components")).unwrap();
        fs::create_dir_all(root.join("app/hooks")).unwrap();
        fs::write(
            root.join("app/page.tsx"),
            "import GameBoard from './components/GameBoard'; export default function Page() { return <GameBoard /> }",
        )
        .unwrap();
        fs::write(
            root.join("app/components/GameBoard.tsx"),
            "export default function GameBoard() { return null }",
        )
        .unwrap();
        fs::write(
            root.join("app/hooks/useGame.ts"),
            "export function useGame() {}",
        )
        .unwrap();
        let mut context = context_with_goal("run on port 3011");
        context.expected_paths = vec!["app/hooks/useGame.ts".to_string()];

        let failures = verify_profile("nextjs", &root, &context).unwrap();

        let failure = failures
            .iter()
            .find(|failure| failure.code == "nextjs_route_not_integrated")
            .expect("route integration failure");
        assert_eq!(
            failure.paths,
            vec![
                "app/page.tsx".to_string(),
                "app/hooks/useGame.ts".to_string(),
                "app/components/GameBoard.tsx".to_string(),
            ]
        );
        assert!(
            failure
                .message
                .contains("repair target `app/components/GameBoard.tsx`")
        );
    }

    #[test]
    fn nextjs_verification_does_not_require_layout_import_from_page() {
        let root = temp_workspace("verify-layout-infra");
        write_minimal_next_app(&root, r#"{"dev":"next dev -p 3011","build":"next build"}"#);
        fs::write(
            root.join("app/layout.tsx"),
            "export default function Layout({ children }: { children: React.ReactNode }) { return children }",
        )
        .unwrap();
        let mut context = context_with_goal("run on port 3011");
        context.expected_paths = vec!["app/page.tsx".to_string(), "app/layout.tsx".to_string()];

        let failures = verify_profile("nextjs", &root, &context).unwrap();

        assert!(
            failures
                .iter()
                .all(|failure| failure.code != "nextjs_route_not_integrated"),
            "{failures:?}"
        );
    }

    #[test]
    fn nextjs_verification_ignores_workspace_observed_next_env_declaration() {
        let root = temp_workspace("verify-next-env-workspace-entry");
        write_minimal_next_app(&root, r#"{"dev":"next dev -p 3011","build":"next build"}"#);
        fs::write(
            root.join("next-env.d.ts"),
            "/// <reference types=\"next\" />",
        )
        .unwrap();
        let mut context = context_with_goal("run on port 3011");
        context.phase_contract_facts =
            vec!["workspace.entries=app/,next-env.d.ts,package.json,tsconfig.json".to_string()];

        let failures = verify_profile("nextjs", &root, &context).unwrap();

        assert!(
            failures
                .iter()
                .all(|failure| failure.code != "nextjs_route_not_integrated"),
            "{failures:?}"
        );
    }

    #[test]
    fn nextjs_verification_ignores_required_generated_declarations() {
        let root = temp_workspace("verify-generated-declaration");
        write_minimal_next_app(&root, r#"{"dev":"next dev -p 3011","build":"next build"}"#);
        let mut context = context_with_goal("run on port 3011");
        context.required_artifacts =
            vec!["next-env.d.ts".to_string(), "types/routes.d.ts".to_string()];

        let failures = verify_profile("nextjs", &root, &context).unwrap();

        assert!(
            failures
                .iter()
                .all(|failure| failure.code != "nextjs_route_not_integrated"),
            "{failures:?}"
        );
    }

    #[test]
    fn nextjs_verification_ignores_profile_obligation_text_as_integration_path() {
        let root = temp_workspace("verify-obligation-text");
        write_minimal_next_app(&root, r#"{"dev":"next dev -p 3011","build":"next build"}"#);
        let mut context = context_with_goal("run on port 3011");
        context.phase_contract_facts = vec![
            "profile.obligation.nextjs_dependencies_required=package.json dependencies must include compatible runtime packages for a Next.js app; paths=package.json; expected=dependencies include next, react, react-dom with React 18.2 or newer compatibility".to_string(),
        ];

        let failures = verify_profile("nextjs", &root, &context).unwrap();

        assert!(
            failures
                .iter()
                .all(|failure| failure.code != "nextjs_route_not_integrated"),
            "{failures:?}"
        );
    }

    #[test]
    fn profile_fact_summary_renders_common_nextjs_output_schema() {
        let root = temp_workspace("profile-output-nextjs");
        write_minimal_next_app(&root, r#"{"dev":"next dev -p 3011","build":"next build"}"#);

        let summary = profile_fact_summary("nextjs", &root).unwrap();

        assert!(
            summary
                .lines
                .iter()
                .any(|line| line == "profile.output.id=nextjs")
        );
        assert!(
            summary
                .lines
                .iter()
                .any(|line| line.contains("profile.output.setup_artifacts=package.json"))
        );
        assert!(
            summary
                .lines
                .iter()
                .any(|line| line.contains("scaffold_materialization:"))
        );
        assert!(
            summary
                .lines
                .iter()
                .any(|line| line.contains("dev_server_smoke:requested_port"))
        );
    }

    #[test]
    fn profile_fact_summary_renders_common_rust_and_python_schema() {
        let rust_root = temp_workspace("profile-output-rust");
        fs::create_dir_all(rust_root.join("src")).unwrap();
        fs::write(rust_root.join("Cargo.toml"), "[package]\nname = \"x\"").unwrap();
        fs::write(rust_root.join("src/main.rs"), "fn main() {}").unwrap();

        let rust = profile_fact_summary("rust", &rust_root).unwrap();
        assert!(
            rust.lines
                .iter()
                .any(|line| line == "profile.output.id=rust")
        );
        assert!(
            rust.lines
                .iter()
                .any(|line| line.contains("profile.output.setup_artifacts=Cargo.toml"))
        );

        let python_root = temp_workspace("profile-output-python");
        fs::create_dir_all(python_root.join("app")).unwrap();
        fs::write(python_root.join("requirements.txt"), "pytest\n").unwrap();
        fs::write(python_root.join("app/main.py"), "def app(): pass").unwrap();

        let python = profile_fact_summary("python", &python_root).unwrap();
        assert!(
            python
                .lines
                .iter()
                .any(|line| line == "profile.output.id=python")
        );
        assert!(
            python
                .lines
                .iter()
                .any(|line| line.contains("profile.output.setup_artifacts=requirements.txt"))
        );
    }

    #[test]
    fn profile_fact_summary_renders_phase13_parity_fields_for_all_profiles() {
        let root = temp_workspace("profile-output-parity");
        fs::create_dir_all(root.join("app")).unwrap();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::create_dir_all(root.join("docs")).unwrap();
        fs::write(root.join("package.json"), "{}").unwrap();
        fs::write(
            root.join("app/page.tsx"),
            "export default function Page() { return null; }",
        )
        .unwrap();
        fs::write(root.join("Cargo.toml"), "[package]\nname = \"x\"").unwrap();
        fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();
        fs::write(root.join("requirements.txt"), "pytest\n").unwrap();
        fs::write(root.join("app/main.py"), "def app(): pass").unwrap();
        fs::write(root.join("README.md"), "# Docs").unwrap();

        for profile in [
            "nextjs",
            "rust",
            "python",
            "docs",
            "data-analysis",
            "data-pipeline",
        ] {
            let summary = profile_fact_summary(profile, &root).unwrap();
            for prefix in [
                "profile_project_kind=",
                "profile_manifest_artifacts=",
                "profile_entrypoints=",
                "profile_integration_artifacts=",
                "profile_completion_evidence=",
                "profile_failure_mapping=",
                "profile_adapter_families=",
                "profile_capability_status=",
                "profile.output.capability.project=",
                "profile.output.capability.failure=",
                "profile.output.capability.adapter=",
            ] {
                assert!(
                    summary.lines.iter().any(|line| line.starts_with(prefix)),
                    "missing {prefix} for {profile}: {:?}",
                    summary.lines
                );
            }
        }
    }

    #[test]
    fn phase26_nextjs_profile_output_renders_scaffold_failure_and_capability_facts() {
        let root = temp_workspace("profile-output-phase26-nextjs");
        fs::create_dir_all(root.join("app")).unwrap();
        fs::create_dir_all(root.join("components")).unwrap();
        fs::write(root.join("package.json"), "{}").unwrap();
        fs::write(
            root.join("app/page.tsx"),
            "import Game from '../components/Game'; export default function Page() { return <Game />; }",
        )
        .unwrap();
        fs::write(
            root.join("components/Game.tsx"),
            "export default function Game() { return null; }",
        )
        .unwrap();

        let summary = profile_fact_summary("nextjs", &root).unwrap();
        let lines = summary.lines.join("\n");

        assert!(lines.contains("profile.output.project_kind=nextjs"));
        assert!(lines.contains("profile.output.manifests=package.json"));
        assert!(lines.contains("profile.output.entrypoints=app/page.tsx"));
        assert!(lines.contains("profile.output.integration_artifacts="));
        assert!(lines.contains("profile.output.scaffold_artifacts="));
        assert!(lines.contains("profile.output.failure_mappings="));
        assert!(lines.contains("profile_failure_mapping="));
        assert!(lines.contains("profile_capability_status="));
        assert!(lines.contains("scaffold:"));
        assert!(lines.contains("failure:"));
    }

    fn context_with_goal(goal: &str) -> ProfileVerificationContext {
        ProfileVerificationContext {
            goal_excerpt: goal.to_string(),
            required_artifacts: Vec::new(),
            expected_paths: Vec::new(),
            phase_contract_facts: Vec::new(),
            profile_facts: Vec::new(),
        }
    }

    fn obligation_context_with_goal(goal: &str) -> ProfileObligationContext {
        ProfileObligationContext {
            goal_excerpt: goal.to_string(),
            required_artifacts: Vec::new(),
            phase_contract_facts: Vec::new(),
            profile_facts: Vec::new(),
        }
    }

    fn write_minimal_next_app(root: &Path, scripts: &str) {
        fs::create_dir_all(root.join("app")).unwrap();
        fs::write(
            root.join("app/page.tsx"),
            "export default function Page() {}",
        )
        .unwrap();
        fs::write(
            root.join("package.json"),
            format!(
                r#"{{"scripts":{scripts},"dependencies":{{"next":"latest","react":"latest","react-dom":"latest"}}}}"#
            ),
        )
        .unwrap();
    }

    fn write_next_app_with_dependencies(root: &Path, dependencies: &str) {
        fs::create_dir_all(root.join("app")).unwrap();
        fs::write(
            root.join("app/page.tsx"),
            "export default function Page() {}",
        )
        .unwrap();
        fs::write(
            root.join("package.json"),
            format!(
                r#"{{"scripts":{{"dev":"next dev -p 3011","build":"next build"}},"dependencies":{dependencies}}}"#
            ),
        )
        .unwrap();
    }

    fn temp_workspace(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "commandagent-profiles-{}-{}",
            name,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }
}
