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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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
    Intent,
    RequiredArtifact,
    ProfileObligation,
    ArtifactRole,
    Profile,
}

impl TaskContractSource {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Goal => "goal",
            Self::Intent => "intent",
            Self::RequiredArtifact => "required_artifact",
            Self::ProfileObligation => "profile_obligation",
            Self::ArtifactRole => "artifact_role",
            Self::Profile => "profile",
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TaskContractLifecycleState {
    Created,
    Admitted,
    Projected,
    Rejected,
}

impl TaskContractLifecycleState {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Admitted => "admitted",
            Self::Projected => "projected",
            Self::Rejected => "rejected",
        }
    }
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TaskRequestSignal {
    pub(crate) code: String,
    pub(crate) kind: TaskKind,
    pub(crate) source: TaskContractSource,
    pub(crate) value: String,
}

impl TaskRequestSignal {
    fn new(
        code: impl Into<String>,
        kind: TaskKind,
        source: TaskContractSource,
        value: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            kind,
            source,
            value: value.into(),
        }
    }

    fn render_summary(&self) -> String {
        format!(
            "{}:{}:{}",
            self.code,
            self.kind.as_str(),
            self.source.as_str()
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TaskContractConstraint {
    pub(crate) code: String,
    pub(crate) source: TaskContractSource,
    pub(crate) value: String,
}

impl TaskContractConstraint {
    fn new(code: impl Into<String>, source: TaskContractSource, value: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            source,
            value: value.into(),
        }
    }

    fn render_summary(&self) -> String {
        format!(
            "{}:{}:{}",
            self.code,
            self.source.as_str(),
            bounded_value(&self.value)
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExpectedCompletionEvidence {
    pub(crate) code: String,
    pub(crate) source: TaskContractSource,
    pub(crate) target: String,
}

impl ExpectedCompletionEvidence {
    fn new(code: impl Into<String>, source: TaskContractSource, target: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            source,
            target: target.into(),
        }
    }

    fn render_summary(&self) -> String {
        format!(
            "{}:{}:{}",
            self.code,
            self.source.as_str(),
            bounded_value(&self.target)
        )
    }
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
    pub(crate) lifecycle_state: TaskContractLifecycleState,
    pub(crate) request_signals: Vec<TaskRequestSignal>,
    pub(crate) constraints: Vec<TaskContractConstraint>,
    pub(crate) expected_completion_evidence: Vec<ExpectedCompletionEvidence>,
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
        let request_signals =
            request_signals(profile, goal, intent, required_artifacts, &artifact_roles);
        let (task_kind, source, admission_status) =
            admission_from_signals(intent, &request_signals);
        let lifecycle_state =
            lifecycle_for_admission(admission_status, !behavior_obligations.is_empty());
        let constraints = task_constraints(
            profile,
            intent,
            required_artifacts,
            profile_obligations,
            &request_signals,
        );
        let expected_completion_evidence = expected_completion_evidence(
            required_artifacts,
            &behavior_obligations,
            &artifact_roles,
        );
        Self {
            profile: profile.to_string(),
            task_kind,
            source,
            admission_status,
            lifecycle_state,
            request_signals,
            constraints,
            expected_completion_evidence,
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
            format!("task.contract.lifecycle={}", self.lifecycle_state.as_str()),
            format!(
                "task.contract.request_signals={}",
                join_or(
                    &self
                        .request_signals
                        .iter()
                        .map(TaskRequestSignal::render_summary)
                        .collect::<Vec<_>>(),
                    "none",
                )
            ),
            format!(
                "task.contract.constraints={}",
                join_or(
                    &self
                        .constraints
                        .iter()
                        .map(TaskContractConstraint::render_summary)
                        .collect::<Vec<_>>(),
                    "none",
                )
            ),
            format!(
                "task.contract.expected_completion_evidence={}",
                join_or(
                    &self
                        .expected_completion_evidence
                        .iter()
                        .map(ExpectedCompletionEvidence::render_summary)
                        .collect::<Vec<_>>(),
                    "none",
                )
            ),
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
            format!("task_contract_lifecycle={}", self.lifecycle_state.as_str()),
            format!(
                "task_contract_request_signals={}",
                join_or(
                    &self
                        .request_signals
                        .iter()
                        .map(TaskRequestSignal::render_summary)
                        .collect::<Vec<_>>(),
                    "none",
                )
            ),
            format!(
                "task_contract_constraints={}",
                join_or(
                    &self
                        .constraints
                        .iter()
                        .map(TaskContractConstraint::render_summary)
                        .collect::<Vec<_>>(),
                    "none",
                )
            ),
            format!(
                "task_contract_completion_evidence={}",
                join_or(
                    &self
                        .expected_completion_evidence
                        .iter()
                        .map(ExpectedCompletionEvidence::render_summary)
                        .collect::<Vec<_>>(),
                    "none",
                )
            ),
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
                "behavior_obligation_owners={}",
                join_or(
                    &self
                        .behavior_obligations
                        .iter()
                        .map(|obligation| format!("{}:{}", obligation.code, obligation.owner))
                        .collect::<Vec<_>>(),
                    "none",
                )
            ),
            format!(
                "behavior_obligation_paths={}",
                join_or(
                    &self
                        .behavior_obligations
                        .iter()
                        .map(|obligation| format!(
                            "{}:{}",
                            obligation.code,
                            join_or(&obligation.paths, "none")
                        ))
                        .collect::<Vec<_>>(),
                    "none",
                )
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

fn request_signals(
    profile: &str,
    goal: &str,
    intent: WorkIntent,
    required_artifacts: &[String],
    artifact_roles: &[TaskArtifactRoleProjection],
) -> Vec<TaskRequestSignal> {
    let mut out = Vec::new();
    if !matches!(intent, WorkIntent::Unknown) {
        out.push(TaskRequestSignal::new(
            "explicit_intent",
            TaskKind::from_intent(intent),
            TaskContractSource::Intent,
            intent.as_str(),
        ));
    }
    let goal_kind = infer_task_kind_from_goal(goal);
    if goal_kind != TaskKind::Unknown {
        out.push(TaskRequestSignal::new(
            "goal_keyword",
            goal_kind,
            TaskContractSource::Goal,
            goal,
        ));
    }
    if let Some(artifact_kind) = infer_task_kind_from_artifacts(required_artifacts, artifact_roles)
    {
        out.push(TaskRequestSignal::new(
            "artifact_roles",
            artifact_kind,
            TaskContractSource::ArtifactRole,
            artifact_roles
                .iter()
                .map(TaskArtifactRoleProjection::render_summary)
                .collect::<Vec<_>>()
                .join("|"),
        ));
    }
    if let Some(profile_kind) = infer_task_kind_from_profile(profile, goal, required_artifacts) {
        out.push(TaskRequestSignal::new(
            "profile_goal",
            profile_kind,
            TaskContractSource::Profile,
            profile,
        ));
    }
    out
}

fn admission_from_signals(
    intent: WorkIntent,
    signals: &[TaskRequestSignal],
) -> (TaskKind, TaskContractSource, TaskContractAdmissionStatus) {
    if !matches!(intent, WorkIntent::Unknown) {
        return (
            TaskKind::from_intent(intent),
            TaskContractSource::Intent,
            TaskContractAdmissionStatus::Admitted,
        );
    }

    let kinds = signals
        .iter()
        .filter_map(|signal| {
            (signal.kind != TaskKind::Unknown).then_some((signal.kind, signal.source))
        })
        .collect::<Vec<_>>();
    let unique_kinds = kinds.iter().map(|(kind, _)| *kind).collect::<BTreeSet<_>>();
    match unique_kinds.len() {
        0 => (
            TaskKind::Unknown,
            TaskContractSource::Goal,
            TaskContractAdmissionStatus::Partial,
        ),
        1 => {
            let (kind, source) = kinds
                .first()
                .copied()
                .unwrap_or((TaskKind::Unknown, TaskContractSource::Goal));
            (kind, source, TaskContractAdmissionStatus::Admitted)
        }
        _ => (
            TaskKind::Unknown,
            TaskContractSource::Goal,
            TaskContractAdmissionStatus::Conflict,
        ),
    }
}

fn lifecycle_for_admission(
    status: TaskContractAdmissionStatus,
    has_behavior_projection: bool,
) -> TaskContractLifecycleState {
    match status {
        TaskContractAdmissionStatus::Admitted if has_behavior_projection => {
            TaskContractLifecycleState::Projected
        }
        TaskContractAdmissionStatus::Admitted => TaskContractLifecycleState::Admitted,
        TaskContractAdmissionStatus::Conflict => TaskContractLifecycleState::Rejected,
        TaskContractAdmissionStatus::Partial | TaskContractAdmissionStatus::Unknown => {
            TaskContractLifecycleState::Created
        }
    }
}

fn task_constraints(
    profile: &str,
    intent: WorkIntent,
    required_artifacts: &[String],
    profile_obligations: &[ProfileObligation],
    request_signals: &[TaskRequestSignal],
) -> Vec<TaskContractConstraint> {
    let mut out = vec![
        TaskContractConstraint::new("profile", TaskContractSource::Profile, profile),
        TaskContractConstraint::new("intent", TaskContractSource::Intent, intent.as_str()),
        TaskContractConstraint::new(
            "source_authority",
            TaskContractSource::Goal,
            if matches!(intent, WorkIntent::Unknown) {
                "inferred"
            } else {
                "explicit_intent"
            },
        ),
    ];
    if !required_artifacts.is_empty() {
        out.push(TaskContractConstraint::new(
            "required_artifacts",
            TaskContractSource::RequiredArtifact,
            join_or(required_artifacts, "none"),
        ));
    }
    if !profile_obligations.is_empty() {
        out.push(TaskContractConstraint::new(
            "profile_obligations",
            TaskContractSource::ProfileObligation,
            profile_obligations
                .iter()
                .map(|obligation| obligation.code.clone())
                .collect::<Vec<_>>()
                .join("|"),
        ));
    }
    if !request_signals.is_empty() {
        out.push(TaskContractConstraint::new(
            "request_signal_count",
            TaskContractSource::Goal,
            request_signals.len().to_string(),
        ));
    }
    out
}

fn expected_completion_evidence(
    required_artifacts: &[String],
    behavior_obligations: &[BehaviorObligation],
    artifact_roles: &[TaskArtifactRoleProjection],
) -> Vec<ExpectedCompletionEvidence> {
    let mut out = Vec::new();
    for artifact in required_artifacts {
        out.push(ExpectedCompletionEvidence::new(
            "artifact_exists",
            TaskContractSource::RequiredArtifact,
            artifact.clone(),
        ));
    }
    for obligation in behavior_obligations {
        out.push(ExpectedCompletionEvidence::new(
            format!("behavior:{}", obligation.code),
            obligation.source,
            obligation
                .expected
                .clone()
                .or_else(|| obligation.paths.first().cloned())
                .unwrap_or_else(|| obligation.kind.as_str().to_string()),
        ));
    }
    for role in artifact_roles {
        out.push(ExpectedCompletionEvidence::new(
            format!("artifact_role:{}", role.role.as_str()),
            role.source,
            role.path.clone(),
        ));
    }
    out
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
    } else if contains_any(&lower, &["data", "csv", "json", "schema", "分析"]) {
        TaskKind::Data
    } else if contains_any(&lower, &["fix", "modify", "update", "修正", "改修"]) {
        TaskKind::Modify
    } else if contains_any(&lower, &["create", "build", "implement", "作成", "開発"]) {
        TaskKind::New
    } else {
        TaskKind::Unknown
    }
}

fn infer_task_kind_from_artifacts(
    required_artifacts: &[String],
    artifact_roles: &[TaskArtifactRoleProjection],
) -> Option<TaskKind> {
    if required_artifacts.is_empty() {
        return None;
    }
    let roles = artifact_roles
        .iter()
        .map(|projection| projection.role)
        .collect::<BTreeSet<_>>();
    if roles.is_empty() {
        return None;
    }
    if roles.iter().all(|role| *role == ArtifactRole::Docs) {
        return Some(TaskKind::Documentation);
    }
    if required_artifacts.iter().all(|path| {
        matches!(
            obligation_kind_for_path(path),
            DeliverableKind::StructuredData | DeliverableKind::Report
        )
    }) {
        return Some(TaskKind::Data);
    }
    if roles.iter().any(|role| {
        matches!(
            role,
            ArtifactRole::SetupManifest
                | ArtifactRole::SetupConfig
                | ArtifactRole::Entrypoint
                | ArtifactRole::IntegrationTarget
                | ArtifactRole::Implementation
                | ArtifactRole::Test
        )
    }) {
        return Some(TaskKind::New);
    }
    None
}

fn infer_task_kind_from_profile(
    profile: &str,
    goal: &str,
    required_artifacts: &[String],
) -> Option<TaskKind> {
    if !matches!(
        ProfileId::parse(profile).unwrap_or(ProfileId::Generic),
        ProfileId::NextJs | ProfileId::Rust | ProfileId::Python
    ) {
        return None;
    }
    let goal_kind = infer_task_kind_from_goal(goal);
    if goal_kind == TaskKind::New
        || required_artifacts.iter().any(|path| {
            matches!(
                obligation_kind_for_path(path),
                DeliverableKind::Source | DeliverableKind::SetupManifest | DeliverableKind::Test
            )
        })
    {
        Some(TaskKind::New)
    } else {
        None
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
        assert!(fields.contains(&"task_contract_lifecycle=projected".to_string()));
        assert!(
            fields
                .iter()
                .any(|field| field.starts_with("task_contract_constraints="))
        );
        assert!(
            fields
                .iter()
                .any(|field| field.starts_with("task_contract_completion_evidence="))
        );
        assert!(
            fields
                .iter()
                .any(|field| field.starts_with("behavior_obligation_codes="))
        );
    }

    #[test]
    fn projects_lifecycle_constraints_and_completion_evidence() {
        let contract = TaskContract::from_goal(
            "nextjs",
            "Create a Next.js app on port 3011",
            WorkIntent::New,
            &["package.json".to_string(), "app/page.tsx".to_string()],
            &[ProfileObligation {
                code: "nextjs_dev_port_required".to_string(),
                message: "dev port".to_string(),
                paths: vec!["package.json".to_string()],
                expected: Some("next dev -p 3011".to_string()),
            }],
        );

        assert_eq!(
            contract.lifecycle_state,
            TaskContractLifecycleState::Projected
        );
        assert!(
            contract
                .constraints
                .iter()
                .any(|constraint| { constraint.code == "profile" && constraint.value == "nextjs" })
        );
        assert!(
            contract
                .expected_completion_evidence
                .iter()
                .any(|evidence| {
                    evidence.code == "artifact_exists" && evidence.target == "app/page.tsx"
                })
        );
        assert!(
            contract
                .expected_completion_evidence
                .iter()
                .any(|evidence| {
                    evidence.code == "behavior:nextjs_dev_port_required"
                        && evidence.target.contains("3011")
                })
        );
        assert!(
            contract
                .render_lines()
                .iter()
                .any(|line| line.starts_with("task.contract.expected_completion_evidence="))
        );
    }

    #[test]
    fn explicit_intent_is_admitted_even_when_other_signals_exist() {
        let contract = TaskContract::from_goal(
            "generic",
            "Investigate README docs",
            WorkIntent::Modify,
            &["README.md".to_string()],
            &[],
        );

        assert_eq!(contract.task_kind, TaskKind::Modify);
        assert_eq!(contract.source, TaskContractSource::Intent);
        assert_eq!(
            contract.admission_status,
            TaskContractAdmissionStatus::Admitted
        );
        assert!(
            contract.request_signals.iter().any(|signal| {
                signal.code == "explicit_intent" && signal.kind == TaskKind::Modify
            })
        );
    }

    #[test]
    fn ambiguous_inferred_request_signals_are_conflict() {
        let contract = TaskContract::from_goal(
            "rust",
            "Investigate why the CLI fails",
            WorkIntent::Unknown,
            &["src/main.rs".to_string()],
            &[],
        );

        assert_eq!(contract.task_kind, TaskKind::Unknown);
        assert_eq!(
            contract.admission_status,
            TaskContractAdmissionStatus::Conflict
        );
        assert_eq!(
            contract.lifecycle_state,
            TaskContractLifecycleState::Rejected
        );
        assert!(contract.request_signals.iter().any(|signal| {
            signal.code == "goal_keyword" && signal.kind == TaskKind::Investigation
        }));
        assert!(
            contract
                .request_signals
                .iter()
                .any(|signal| { signal.code == "artifact_roles" && signal.kind == TaskKind::New })
        );
    }

    #[test]
    fn missing_request_signals_are_partial() {
        let contract =
            TaskContract::from_goal("generic", "Please handle it", WorkIntent::Unknown, &[], &[]);

        assert_eq!(
            contract.admission_status,
            TaskContractAdmissionStatus::Partial
        );
        assert_eq!(
            contract.lifecycle_state,
            TaskContractLifecycleState::Created
        );
    }

    #[test]
    fn docs_and_data_artifacts_project_request_signals() {
        let docs = TaskContract::from_goal(
            "docs",
            "Write README",
            WorkIntent::Unknown,
            &["README.md".to_string()],
            &[],
        );
        let data = TaskContract::from_goal(
            "data",
            "Create output schema",
            WorkIntent::Unknown,
            &["schema/output.json".to_string()],
            &[],
        );

        assert_eq!(docs.task_kind, TaskKind::Documentation);
        assert_eq!(data.task_kind, TaskKind::Data);
    }
}
