//! Task-contract projection for planning, lint, and eval evidence.
//!
//! This module does not arbitrate jobs or execute recovery. It collects the
//! deterministic facts already known at planning time and gives them stable
//! names so prompts, lint failures, repair packets, and eval reports can refer
//! to the same contract vocabulary.

use crate::agent::step_runner::artifact_graph::{ArtifactLifecycle, ArtifactRole, role_for_path};
use crate::agent::step_runner::deliverable_obligation::{
    DeliverableKind, obligation_kind_for_path,
};
use crate::agent::step_runner::profiles::{ProfileId, ProfileObligation};
use crate::agent::step_runner::{StepPlan, WorkIntent};
use std::collections::BTreeSet;

const MAX_RENDERED_ITEMS: usize = 16;
const MAX_RENDERED_CHARS: usize = 180;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TaskKind {
    New,
    Modify,
    Investigation,
    Documentation,
    Data,
    Unknown,
}

impl TaskKind {
    pub(crate) fn from_intent(intent: WorkIntent) -> Self {
        match intent {
            WorkIntent::New => Self::New,
            WorkIntent::Modify => Self::Modify,
            WorkIntent::Investigate => Self::Investigation,
            WorkIntent::Document => Self::Documentation,
            WorkIntent::Data => Self::Data,
            WorkIntent::Unknown => Self::Unknown,
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::New => "new",
            Self::Modify => "modify",
            Self::Investigation => "investigation",
            Self::Documentation => "documentation",
            Self::Data => "data",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum TaskContractSource {
    Goal,
    RequiredArtifact,
    ProfileObligation,
    ArtifactRole,
}

impl TaskContractSource {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Goal => "goal",
            Self::RequiredArtifact => "required_artifact",
            Self::ProfileObligation => "profile_obligation",
            Self::ArtifactRole => "artifact_role",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum TaskContractAdmissionStatus {
    Admitted,
    Partial,
    Conflict,
    Unknown,
}

impl TaskContractAdmissionStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Admitted => "admitted",
            Self::Partial => "partial",
            Self::Conflict => "conflict",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum BehaviorObligationKind {
    DependencySetup,
    ManifestContract,
    BuildContract,
    DevServerPort,
    RouteIntegration,
    ArtifactCompletion,
    TestArtifact,
    DocsLiteral,
    DataSchema,
    SourceImplementation,
    ProfileContract,
}

impl BehaviorObligationKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::DependencySetup => "dependency_setup",
            Self::ManifestContract => "manifest_contract",
            Self::BuildContract => "build_contract",
            Self::DevServerPort => "dev_server_port",
            Self::RouteIntegration => "route_integration",
            Self::ArtifactCompletion => "artifact_completion",
            Self::TestArtifact => "test_artifact",
            Self::DocsLiteral => "docs_literal",
            Self::DataSchema => "data_schema",
            Self::SourceImplementation => "source_implementation",
            Self::ProfileContract => "profile_contract",
        }
    }

    pub(crate) fn default_owner(self) -> &'static str {
        match self {
            Self::DependencySetup => "setup",
            Self::ManifestContract | Self::BuildContract | Self::DevServerPort => "manifest",
            Self::RouteIntegration => "route_integration",
            Self::TestArtifact => "test",
            Self::DocsLiteral => "docs",
            Self::DataSchema => "data",
            Self::ArtifactCompletion => "scaffold",
            Self::SourceImplementation | Self::ProfileContract => "source",
        }
    }

    pub(crate) fn active_job(self) -> &'static str {
        match self {
            Self::DependencySetup => "setup_bootstrap",
            Self::ManifestContract | Self::BuildContract | Self::DevServerPort => "manifest_repair",
            Self::RouteIntegration => "route_integration_repair",
            Self::TestArtifact => "test_artifact_completion",
            Self::DocsLiteral => "documentation_repair",
            Self::DataSchema => "data_artifact_completion",
            Self::ArtifactCompletion => "scaffold_materialization",
            Self::SourceImplementation | Self::ProfileContract => "source_implementation_repair",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BehaviorObligation {
    pub(crate) code: String,
    pub(crate) kind: BehaviorObligationKind,
    pub(crate) source: TaskContractSource,
    pub(crate) owner: String,
    pub(crate) paths: Vec<String>,
    pub(crate) required_literals: Vec<String>,
    pub(crate) expected: Option<String>,
}

impl BehaviorObligation {
    fn new(
        code: impl Into<String>,
        kind: BehaviorObligationKind,
        source: TaskContractSource,
    ) -> Self {
        Self {
            code: code.into(),
            kind,
            source,
            owner: kind.default_owner().to_string(),
            paths: Vec::new(),
            required_literals: Vec::new(),
            expected: None,
        }
    }

    fn with_paths<I, S>(mut self, paths: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.paths = dedupe(paths);
        self
    }

    fn with_expected(mut self, expected: Option<String>) -> Self {
        self.expected = expected;
        self
    }

    fn with_required_literals<I, S>(mut self, literals: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.required_literals = dedupe(literals);
        self
    }

    fn render_summary(&self) -> String {
        format!("{}:{}:{}", self.code, self.kind.as_str(), self.owner)
    }

    fn render_line(&self) -> String {
        format!(
            "task.contract.behavior_obligation.{}=kind={} owner={} source={} paths={} literals={} expected={}",
            self.code,
            self.kind.as_str(),
            self.owner,
            self.source.as_str(),
            join_or(&self.paths, "none"),
            join_or(&self.required_literals, "none"),
            self.expected
                .as_deref()
                .map(bounded_value)
                .unwrap_or_else(|| "none".to_string())
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TaskArtifactRoleProjection {
    pub(crate) path: String,
    pub(crate) role: ArtifactRole,
    pub(crate) source: TaskContractSource,
}

impl TaskArtifactRoleProjection {
    fn render_summary(&self) -> String {
        format!(
            "{}:{}:{}",
            self.path,
            self.role.as_str(),
            self.source.as_str()
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TaskContract {
    pub(crate) profile: String,
    pub(crate) task_kind: TaskKind,
    pub(crate) source: TaskContractSource,
    pub(crate) admission_status: TaskContractAdmissionStatus,
    pub(crate) required_artifacts: Vec<String>,
    pub(crate) behavior_obligations: Vec<BehaviorObligation>,
    pub(crate) artifact_roles: Vec<TaskArtifactRoleProjection>,
}

impl TaskContract {
    pub(crate) fn new(
        profile: &str,
        intent: WorkIntent,
        required_artifacts: &[String],
        profile_obligations: &[ProfileObligation],
    ) -> Self {
        Self::from_goal(profile, "", intent, required_artifacts, profile_obligations)
    }

    pub(crate) fn from_goal(
        profile: &str,
        goal: &str,
        intent: WorkIntent,
        required_artifacts: &[String],
        profile_obligations: &[ProfileObligation],
    ) -> Self {
        let mut behavior_obligations = Vec::new();
        for artifact in required_artifacts {
            if let Some(obligation) = deliverable_behavior_obligation(artifact) {
                push_obligation(&mut behavior_obligations, obligation);
            }
        }
        for obligation in profile_obligations {
            push_obligation(
                &mut behavior_obligations,
                behavior_obligation_from_profile_obligation(obligation),
            );
        }

        let artifact_roles =
            artifact_role_projections(profile, required_artifacts, profile_obligations);
        let task_kind = if matches!(intent, WorkIntent::Unknown) {
            infer_task_kind_from_goal(goal)
        } else {
            TaskKind::from_intent(intent)
        };
        Self {
            profile: profile.to_string(),
            task_kind,
            source: TaskContractSource::Goal,
            admission_status: TaskContractAdmissionStatus::Admitted,
            required_artifacts: dedupe(required_artifacts.iter().cloned()),
            behavior_obligations,
            artifact_roles,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn from_plan(plan: &StepPlan, profile_obligations: &[ProfileObligation]) -> Self {
        Self::from_goal(
            &plan.profile,
            &plan.goal,
            plan.intent,
            &plan.required_artifacts,
            profile_obligations,
        )
    }

    pub(crate) fn render_lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!("task.contract.profile={}", bounded_value(&self.profile)),
            format!("task.contract.kind={}", self.task_kind.as_str()),
            format!("task.contract.source={}", self.source.as_str()),
            format!("task.contract.status={}", self.admission_status.as_str()),
            format!(
                "task.contract.required_artifacts={}",
                join_or(&self.required_artifacts, "none")
            ),
            format!(
                "task.contract.behavior_obligations={}",
                join_or(
                    &self
                        .behavior_obligations
                        .iter()
                        .map(BehaviorObligation::render_summary)
                        .collect::<Vec<_>>(),
                    "none",
                )
            ),
            format!(
                "task.contract.artifact_roles={}",
                join_or(
                    &self
                        .artifact_roles
                        .iter()
                        .map(TaskArtifactRoleProjection::render_summary)
                        .collect::<Vec<_>>(),
                    "none",
                )
            ),
        ];
        lines.extend(
            self.behavior_obligations
                .iter()
                .take(MAX_RENDERED_ITEMS)
                .map(BehaviorObligation::render_line),
        );
        lines
    }

    pub(crate) fn render_prompt_section(&self) -> String {
        self.render_lines()
            .into_iter()
            .take(MAX_RENDERED_ITEMS)
            .map(|line| format!("- {}", bounded_value(&line)))
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub(crate) fn eval_report_fields(&self) -> Vec<String> {
        vec![
            format!("task_contract_kind={}", self.task_kind.as_str()),
            format!("task_contract_status={}", self.admission_status.as_str()),
            format!(
                "behavior_obligation_codes={}",
                join_or(
                    &self
                        .behavior_obligations
                        .iter()
                        .map(|obligation| obligation.code.clone())
                        .collect::<Vec<_>>(),
                    "none",
                )
            ),
            format!(
                "behavior_obligation_status={}",
                if self.behavior_obligations.is_empty() {
                    "none"
                } else {
                    "projected"
                }
            ),
            format!(
                "artifact_role_projection_status={}",
                if self.artifact_roles.is_empty() {
                    "none"
                } else {
                    "projected"
                }
            ),
        ]
    }
}

pub(crate) fn render_task_contract_lines(contract: &TaskContract) -> Vec<String> {
    contract.render_lines()
}

fn behavior_obligation_from_profile_obligation(
    obligation: &ProfileObligation,
) -> BehaviorObligation {
    let kind = match obligation.code.as_str() {
        "nextjs_dependencies_required" | "nextjs_tailwind_dependencies_required" => {
            BehaviorObligationKind::DependencySetup
        }
        "nextjs_build_script_required" => BehaviorObligationKind::BuildContract,
        "nextjs_dev_port_required" => BehaviorObligationKind::DevServerPort,
        "nextjs_route_integration_required" => BehaviorObligationKind::RouteIntegration,
        _ => BehaviorObligationKind::ProfileContract,
    };
    BehaviorObligation::new(
        obligation.code.clone(),
        kind,
        TaskContractSource::ProfileObligation,
    )
    .with_paths(obligation.paths.clone())
    .with_expected(obligation.expected.clone())
    .with_required_literals(required_literals_for_profile_obligation(obligation))
}

fn required_literals_for_profile_obligation(obligation: &ProfileObligation) -> Vec<String> {
    match obligation.code.as_str() {
        "nextjs_dependencies_required" => vec!["next", "react", "react-dom"],
        "nextjs_tailwind_dependencies_required" => {
            vec!["tailwindcss", "postcss", "autoprefixer"]
        }
        "nextjs_build_script_required" => vec!["next build"],
        "nextjs_dev_port_required" => vec!["next dev", "3011"],
        "nextjs_route_integration_required" => obligation
            .paths
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        _ => obligation
            .expected
            .as_deref()
            .map(|expected| vec![expected])
            .unwrap_or_default(),
    }
    .into_iter()
    .map(ToString::to_string)
    .collect()
}

fn deliverable_behavior_obligation(path: &str) -> Option<BehaviorObligation> {
    let kind = match obligation_kind_for_path(path) {
        DeliverableKind::Source => BehaviorObligationKind::ArtifactCompletion,
        DeliverableKind::SetupManifest => BehaviorObligationKind::ManifestContract,
        DeliverableKind::Test => BehaviorObligationKind::TestArtifact,
        DeliverableKind::Docs | DeliverableKind::Report => BehaviorObligationKind::DocsLiteral,
        DeliverableKind::StructuredData => BehaviorObligationKind::DataSchema,
    };
    Some(
        BehaviorObligation::new(
            format!("required_artifact:{}", normalize_for_code(path)),
            kind,
            TaskContractSource::RequiredArtifact,
        )
        .with_paths(vec![path.to_string()]),
    )
}

fn artifact_role_projections(
    profile: &str,
    required_artifacts: &[String],
    profile_obligations: &[ProfileObligation],
) -> Vec<TaskArtifactRoleProjection> {
    let profile_id = ProfileId::parse(profile).unwrap_or(ProfileId::Generic);
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    for path in required_artifacts {
        push_role_projection(
            &mut out,
            &mut seen,
            profile_id,
            path,
            TaskContractSource::RequiredArtifact,
        );
    }
    for obligation in profile_obligations {
        for path in &obligation.paths {
            push_role_projection(
                &mut out,
                &mut seen,
                profile_id,
                path,
                TaskContractSource::ProfileObligation,
            );
        }
    }
    out
}

fn push_role_projection(
    out: &mut Vec<TaskArtifactRoleProjection>,
    seen: &mut BTreeSet<String>,
    _profile: ProfileId,
    path: &str,
    source: TaskContractSource,
) {
    let normalized = normalize_path(path);
    if normalized.is_empty() || !seen.insert(normalized.clone()) {
        return;
    }
    let role = role_for_path(&normalized, ArtifactLifecycle::Required);
    out.push(TaskArtifactRoleProjection {
        path: normalized,
        role,
        source,
    });
}

fn infer_task_kind_from_goal(goal: &str) -> TaskKind {
    let lower = goal.to_ascii_lowercase();
    if contains_any(&lower, &["investigate", "debug", "調査", "原因"]) {
        TaskKind::Investigation
    } else if contains_any(&lower, &["document", "docs", "readme", "ドキュメント"]) {
        TaskKind::Documentation
    } else if contains_any(&lower, &["data", "csv", "分析"]) {
        TaskKind::Data
    } else if contains_any(&lower, &["fix", "modify", "update", "修正", "改修"]) {
        TaskKind::Modify
    } else if contains_any(&lower, &["create", "build", "implement", "作成", "開発"]) {
        TaskKind::New
    } else {
        TaskKind::Unknown
    }
}

fn push_obligation(out: &mut Vec<BehaviorObligation>, obligation: BehaviorObligation) {
    if out.iter().any(|existing| existing.code == obligation.code) {
        return;
    }
    out.push(obligation);
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

fn dedupe<I, S>(values: I) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for value in values {
        let value = value.into();
        if value.trim().is_empty() || !seen.insert(value.clone()) {
            continue;
        }
        out.push(value);
    }
    out
}

fn join_or(values: &[String], empty: &str) -> String {
    if values.is_empty() {
        empty.to_string()
    } else {
        values
            .iter()
            .take(MAX_RENDERED_ITEMS)
            .map(|value| bounded_value(value))
            .collect::<Vec<_>>()
            .join("|")
    }
}

fn bounded_value(value: &str) -> String {
    let mut out = value.trim().replace('\n', " ");
    if out.len() > MAX_RENDERED_CHARS {
        out.truncate(MAX_RENDERED_CHARS);
        out.push_str("...");
    }
    out
}

fn normalize_path(path: &str) -> String {
    path.trim().trim_start_matches("./").replace('\\', "/")
}

fn normalize_for_code(path: &str) -> String {
    normalize_path(path)
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projects_nextjs_profile_obligations_into_behavior_contracts() {
        let contract = TaskContract::from_goal(
            "nextjs",
            "Create app on port 3011",
            WorkIntent::New,
            &["app/page.tsx".to_string()],
            &[
                ProfileObligation {
                    code: "nextjs_dependencies_required".to_string(),
                    message: "deps".to_string(),
                    paths: vec!["package.json".to_string()],
                    expected: None,
                },
                ProfileObligation {
                    code: "nextjs_route_integration_required".to_string(),
                    message: "route".to_string(),
                    paths: vec![
                        "app/page.tsx".to_string(),
                        "components/Game.tsx".to_string(),
                    ],
                    expected: Some("route references Game".to_string()),
                },
            ],
        );

        assert_eq!(contract.task_kind, TaskKind::New);
        assert!(contract.behavior_obligations.iter().any(|obligation| {
            obligation.kind == BehaviorObligationKind::DependencySetup
                && obligation
                    .required_literals
                    .contains(&"react-dom".to_string())
        }));
        assert!(contract.behavior_obligations.iter().any(|obligation| {
            obligation.kind == BehaviorObligationKind::RouteIntegration
                && obligation
                    .paths
                    .contains(&"components/Game.tsx".to_string())
        }));
        assert!(
            contract
                .render_lines()
                .iter()
                .any(|line| line.starts_with("task.contract.behavior_obligation."))
        );
    }

    #[test]
    fn projects_required_artifact_roles_across_profiles() {
        let contract = TaskContract::from_goal(
            "rust",
            "Create CLI",
            WorkIntent::New,
            &["Cargo.toml".to_string(), "src/main.rs".to_string()],
            &[],
        );

        assert!(contract.artifact_roles.iter().any(|projection| {
            projection.path == "Cargo.toml" && projection.role == ArtifactRole::SetupManifest
        }));
        assert!(contract.artifact_roles.iter().any(|projection| {
            projection.path == "src/main.rs" && projection.role == ArtifactRole::Entrypoint
        }));
        assert!(
            contract
                .behavior_obligations
                .iter()
                .any(|obligation| { obligation.kind == BehaviorObligationKind::ManifestContract })
        );
    }

    #[test]
    fn renders_eval_fields_with_stable_names() {
        let contract = TaskContract::from_goal(
            "docs",
            "Write README",
            WorkIntent::Document,
            &["README.md".to_string()],
            &[],
        );

        let fields = contract.eval_report_fields();

        assert!(fields.contains(&"task_contract_kind=documentation".to_string()));
        assert!(fields.contains(&"task_contract_status=admitted".to_string()));
        assert!(
            fields
                .iter()
                .any(|field| field.starts_with("behavior_obligation_codes="))
        );
    }
}
