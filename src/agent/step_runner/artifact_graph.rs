#![allow(dead_code)]

use crate::agent::step_runner::correction_evidence::ContractEvidence;
use crate::agent::step_runner::profile_artifact::{
    is_build_output_path, is_config_path, is_dependency_cache_path, is_manifest_path,
};
use crate::agent::step_runner::{StepKind, StepPlan};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ArtifactRole {
    SetupManifest,
    SetupConfig,
    Entrypoint,
    IntegrationTarget,
    Implementation,
    Test,
    Docs,
    GeneratedOutput,
    DependencyCache,
    Unknown,
}

impl ArtifactRole {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::SetupManifest => "setup_manifest",
            Self::SetupConfig => "setup_config",
            Self::Entrypoint => "entrypoint",
            Self::IntegrationTarget => "integration_target",
            Self::Implementation => "implementation",
            Self::Test => "test",
            Self::Docs => "docs",
            Self::GeneratedOutput => "generated_output",
            Self::DependencyCache => "dependency_cache",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArtifactLifecycle {
    Existing,
    Required,
    ToBeCreated,
    SetupManifest,
    IntegrationTarget,
    GeneratedOutput,
}

impl ArtifactLifecycle {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Existing => "existing",
            Self::Required => "required",
            Self::ToBeCreated => "to_be_created",
            Self::SetupManifest => "setup_manifest",
            Self::IntegrationTarget => "integration_target",
            Self::GeneratedOutput => "generated_output",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArtifactRelationKind {
    Produces,
    Consumes,
    Integrates,
    Verifies,
    RequiresSetupFrom,
}

impl ArtifactRelationKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Produces => "produces",
            Self::Consumes => "consumes",
            Self::Integrates => "integrates",
            Self::Verifies => "verifies",
            Self::RequiresSetupFrom => "requires_setup_from",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ArtifactNode {
    pub(crate) path: String,
    pub(crate) role: ArtifactRole,
    pub(crate) lifecycle: ArtifactLifecycle,
    pub(crate) source: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ArtifactRelation {
    pub(crate) from: String,
    pub(crate) to: String,
    pub(crate) kind: ArtifactRelationKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct ArtifactGraph {
    nodes: Vec<ArtifactNode>,
    relations: Vec<ArtifactRelation>,
}

impl ArtifactGraph {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn from_contract_evidence(evidence: &ContractEvidence) -> Self {
        let mut graph = Self::new();
        for path in &evidence.required_paths {
            graph.add_path(path, ArtifactLifecycle::Required, "contract.required_paths");
        }
        for path in &evidence.missing_paths {
            graph.add_path(
                path,
                ArtifactLifecycle::ToBeCreated,
                "contract.missing_paths",
            );
        }
        for path in &evidence.candidate_artifacts {
            graph.add_path(
                path,
                ArtifactLifecycle::Required,
                "contract.candidate_artifacts",
            );
        }
        if let Some(path) = evidence.target_path.as_deref() {
            graph.add_path(path, target_lifecycle(evidence), "contract.target_path");
        }
        if let Some(path) = evidence.repair_target.as_deref() {
            graph.add_path(path, target_lifecycle(evidence), "contract.repair_target");
        }
        if let Some(path) = setup_manifest_hint(evidence) {
            graph.add_path(
                path,
                ArtifactLifecycle::SetupManifest,
                "contract.setup_manifest",
            );
        }
        if evidence.violated_contract.as_deref() == Some("nextjs_route_not_integrated")
            || evidence.reason_code.as_deref() == Some("nextjs_route_not_integrated")
        {
            let route = evidence
                .required_paths
                .first()
                .or(evidence.candidate_artifacts.first());
            let artifact = evidence.candidate_artifacts.get(1);
            if let (Some(route), Some(artifact)) = (route, artifact) {
                graph.add_relation(artifact, route, ArtifactRelationKind::Integrates);
            }
        }
        graph
    }

    pub(crate) fn from_step_plan(plan: &StepPlan, cwd: Option<&Path>) -> Self {
        let mut graph = Self::new();
        for path in &plan.required_artifacts {
            graph.add_path(
                path,
                lifecycle_for_path(cwd, path, StepKind::Report),
                "plan.required_artifacts",
            );
        }
        for step in &plan.steps {
            for path in &step.expected_paths {
                graph.add_path(path, lifecycle_for_path(cwd, path, step.kind), &step.id);
            }
            for command in &step.verify {
                for path in source_like_paths(command) {
                    graph.add_path(
                        &path,
                        lifecycle_for_path(cwd, &path, StepKind::Verify),
                        &step.id,
                    );
                }
            }
        }
        graph
    }

    pub(crate) fn add_path(
        &mut self,
        path: &str,
        lifecycle: ArtifactLifecycle,
        source: impl Into<String>,
    ) {
        let path = normalize_path(path);
        if path.trim().is_empty() {
            return;
        }
        let role = role_for_path(&path, lifecycle);
        if let Some(existing) = self.nodes.iter_mut().find(|node| node.path == path) {
            existing.lifecycle = merge_lifecycle(existing.lifecycle, lifecycle);
            return;
        }
        self.nodes.push(ArtifactNode {
            path,
            role,
            lifecycle,
            source: source.into(),
        });
    }

    pub(crate) fn add_relation(&mut self, from: &str, to: &str, kind: ArtifactRelationKind) {
        let relation = ArtifactRelation {
            from: normalize_path(from),
            to: normalize_path(to),
            kind,
        };
        if !self.relations.contains(&relation) {
            self.relations.push(relation);
        }
    }

    pub(crate) fn node(&self, path: &str) -> Option<&ArtifactNode> {
        let path = normalize_path(path);
        self.nodes.iter().find(|node| node.path == path)
    }

    pub(crate) fn nodes(&self) -> &[ArtifactNode] {
        &self.nodes
    }

    pub(crate) fn is_future_required(&self, path: &str) -> bool {
        self.node(path).is_some_and(|node| {
            matches!(
                node.lifecycle,
                ArtifactLifecycle::Required | ArtifactLifecycle::ToBeCreated
            ) && !matches!(
                node.role,
                ArtifactRole::GeneratedOutput | ArtifactRole::DependencyCache
            )
        })
    }

    pub(crate) fn integration_targets_for(&self, path: &str) -> Vec<String> {
        let path = normalize_path(path);
        self.relations
            .iter()
            .filter(|relation| {
                relation.kind == ArtifactRelationKind::Integrates && relation.from == path
            })
            .map(|relation| relation.to.clone())
            .collect()
    }

    pub(crate) fn setup_manifest_for(&self, path: &str) -> Option<String> {
        let path = normalize_path(path);
        self.relations
            .iter()
            .find(|relation| {
                relation.kind == ArtifactRelationKind::RequiresSetupFrom && relation.from == path
            })
            .map(|relation| relation.to.clone())
            .or_else(|| {
                self.nodes
                    .iter()
                    .find(|node| node.role == ArtifactRole::SetupManifest)
                    .map(|node| node.path.clone())
            })
    }

    pub(crate) fn summary(&self) -> Vec<String> {
        self.nodes
            .iter()
            .take(8)
            .map(|node| {
                format!(
                    "{} role={} lifecycle={} source={}",
                    node.path,
                    node.role.as_str(),
                    node.lifecycle.as_str(),
                    node.source
                )
            })
            .collect()
    }
}

pub(crate) fn role_for_path(path: &str, lifecycle: ArtifactLifecycle) -> ArtifactRole {
    let path = normalize_path(path);
    if path.starts_with("step:") {
        return ArtifactRole::Unknown;
    }
    if is_dependency_cache_path(&path) {
        return ArtifactRole::DependencyCache;
    }
    if is_build_output_path(&path) || matches!(lifecycle, ArtifactLifecycle::GeneratedOutput) {
        return ArtifactRole::GeneratedOutput;
    }
    if is_manifest_path(&path) {
        return ArtifactRole::SetupManifest;
    }
    if is_config_path(&path) {
        return ArtifactRole::SetupConfig;
    }
    if is_entrypoint_path(&path) {
        return ArtifactRole::Entrypoint;
    }
    if is_test_path(&path) {
        return ArtifactRole::Test;
    }
    if is_docs_path(&path) {
        return ArtifactRole::Docs;
    }
    if is_source_path(&path) {
        if matches!(lifecycle, ArtifactLifecycle::IntegrationTarget) {
            ArtifactRole::IntegrationTarget
        } else {
            ArtifactRole::Implementation
        }
    } else {
        ArtifactRole::Unknown
    }
}

pub(crate) fn recovery_target_admissible(role: ArtifactRole) -> bool {
    !matches!(
        role,
        ArtifactRole::GeneratedOutput | ArtifactRole::DependencyCache
    )
}

fn target_lifecycle(evidence: &ContractEvidence) -> ArtifactLifecycle {
    match evidence.active_job.as_deref() {
        Some("route_integration_repair") => ArtifactLifecycle::IntegrationTarget,
        Some("manifest_repair") | Some("setup_bootstrap") => ArtifactLifecycle::SetupManifest,
        Some("integration_artifact_creation") | Some("scaffold_materialization") => {
            ArtifactLifecycle::ToBeCreated
        }
        _ => ArtifactLifecycle::Required,
    }
}

fn setup_manifest_hint(evidence: &ContractEvidence) -> Option<&str> {
    if evidence.active_job.as_deref() == Some("manifest_repair")
        || evidence.setup_implication.as_deref() == Some("setup_after_manifest_repair_required")
        || evidence.reason_code.as_deref().is_some_and(|code| {
            code.contains("dependency") || code.contains("manifest") || code.contains("package")
        })
    {
        Some("package.json")
    } else {
        None
    }
}

fn lifecycle_for_path(cwd: Option<&Path>, path: &str, kind: StepKind) -> ArtifactLifecycle {
    if matches!(
        role_for_path(path, ArtifactLifecycle::Required),
        ArtifactRole::GeneratedOutput
    ) {
        return ArtifactLifecycle::GeneratedOutput;
    }
    if is_manifest_path(path) {
        return ArtifactLifecycle::SetupManifest;
    }
    if cwd.is_some_and(|cwd| cwd.join(path).exists()) {
        return ArtifactLifecycle::Existing;
    }
    match kind {
        StepKind::Create | StepKind::Edit | StepKind::Setup | StepKind::Repair => {
            ArtifactLifecycle::ToBeCreated
        }
        StepKind::Inspect | StepKind::Report | StepKind::Verify => ArtifactLifecycle::Required,
    }
}

fn merge_lifecycle(existing: ArtifactLifecycle, incoming: ArtifactLifecycle) -> ArtifactLifecycle {
    use ArtifactLifecycle as L;
    match (existing, incoming) {
        (L::Existing, _) | (_, L::Existing) => L::Existing,
        (L::SetupManifest, _) | (_, L::SetupManifest) => L::SetupManifest,
        (L::IntegrationTarget, _) | (_, L::IntegrationTarget) => L::IntegrationTarget,
        (L::ToBeCreated, _) | (_, L::ToBeCreated) => L::ToBeCreated,
        (L::GeneratedOutput, _) | (_, L::GeneratedOutput) => L::GeneratedOutput,
        _ => L::Required,
    }
}

fn source_like_paths(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(|raw| {
            raw.trim_matches(|ch: char| {
                matches!(
                    ch,
                    '.' | ',' | ';' | ':' | '(' | ')' | '[' | ']' | '{' | '}' | '\'' | '"' | '`'
                )
            })
            .trim_start_matches("./")
            .to_string()
        })
        .filter(|value| is_source_path(value))
        .collect()
}

fn is_entrypoint_path(path: &str) -> bool {
    matches!(
        path,
        "app/page.tsx"
            | "app/page.jsx"
            | "src/app/page.tsx"
            | "src/app/page.jsx"
            | "pages/index.tsx"
            | "pages/index.jsx"
            | "src/pages/index.tsx"
            | "src/pages/index.jsx"
            | "src/main.rs"
            | "main.py"
            | "app/main.py"
    )
}

fn is_test_path(path: &str) -> bool {
    (path.starts_with("tests/") || path.contains("/tests/")) && extension_is(path, &["rs", "py"])
}

fn is_docs_path(path: &str) -> bool {
    path == "README.md" || path.starts_with("docs/") && extension_is(path, &["md"])
}

fn is_source_path(path: &str) -> bool {
    extension_is(path, &["ts", "tsx", "js", "jsx", "rs", "py", "css"])
}

fn extension_is(path: &str, extensions: &[&str]) -> bool {
    let Some(extension) = path.rsplit_once('.').map(|(_, ext)| ext) else {
        return false;
    };
    extensions.contains(&extension)
}

fn normalize_path(path: &str) -> String {
    path.trim().trim_start_matches("./").replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::step_runner::{ExpectedResult, StepPlanStep, WorkIntent};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn future_required_artifact_is_visible_for_inspect_violation() {
        let root = temp_workspace("future-required");
        let plan = StepPlan {
            goal: "inspect missing future file".to_string(),
            profile: "rust".to_string(),
            style: "default".to_string(),
            intent: WorkIntent::Modify,
            required_artifacts: Vec::new(),
            steps: vec![StepPlanStep {
                id: "inspect-lib".to_string(),
                kind: StepKind::Inspect,
                instruction: "Inspect src/lib.rs".to_string(),
                expected_result: ExpectedResult::Pass,
                expected_paths: vec!["src/lib.rs".to_string()],
                verify: Vec::new(),
            }],
        };

        let graph = ArtifactGraph::from_step_plan(&plan, Some(&root));

        assert!(graph.is_future_required("src/lib.rs"));
        assert_eq!(
            graph.node("src/lib.rs").unwrap().role,
            ArtifactRole::Implementation
        );
    }

    #[test]
    fn existing_artifact_is_not_future_required() {
        let root = temp_workspace("existing");
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/lib.rs"), "pub fn ok() {}\n").unwrap();
        let plan = StepPlan {
            goal: "inspect existing file".to_string(),
            profile: "rust".to_string(),
            style: "default".to_string(),
            intent: WorkIntent::Modify,
            required_artifacts: Vec::new(),
            steps: vec![StepPlanStep {
                id: "inspect-lib".to_string(),
                kind: StepKind::Inspect,
                instruction: "Inspect src/lib.rs".to_string(),
                expected_result: ExpectedResult::Pass,
                expected_paths: vec!["src/lib.rs".to_string()],
                verify: Vec::new(),
            }],
        };

        let graph = ArtifactGraph::from_step_plan(&plan, Some(&root));

        assert!(!graph.is_future_required("src/lib.rs"));
        assert_eq!(
            graph.node("src/lib.rs").unwrap().lifecycle,
            ArtifactLifecycle::Existing
        );
    }

    #[test]
    fn contract_evidence_graph_names_manifest_and_route_edges() {
        let evidence = ContractEvidence::new("profile_verification")
            .with_violated_contract("nextjs_route_not_integrated")
            .with_reason_code("nextjs_route_not_integrated")
            .with_active_job("route_integration_repair")
            .with_required_paths(vec!["app/page.tsx"])
            .with_candidate_artifacts(vec![
                "app/page.tsx",
                "components/Game.tsx",
                "components/GameBoard.tsx",
            ])
            .with_repair_target("components/GameBoard.tsx");

        let graph = ArtifactGraph::from_contract_evidence(&evidence);

        assert_eq!(
            graph.node("components/GameBoard.tsx").unwrap().lifecycle,
            ArtifactLifecycle::IntegrationTarget
        );
        assert_eq!(
            graph.integration_targets_for("components/Game.tsx"),
            vec!["app/page.tsx"]
        );
    }

    #[test]
    fn dependency_evidence_points_to_manifest() {
        let evidence = ContractEvidence::new("verifier")
            .with_reason_code("dependency_missing")
            .with_active_job("setup_bootstrap")
            .with_setup_implication("setup_blocker");

        let graph = ArtifactGraph::from_contract_evidence(&evidence);

        assert_eq!(
            graph.setup_manifest_for("app/page.tsx").as_deref(),
            Some("package.json")
        );
    }

    #[test]
    fn generated_outputs_are_not_recovery_targets() {
        let role = role_for_path("node_modules/react/index.js", ArtifactLifecycle::Existing);

        assert_eq!(role, ArtifactRole::DependencyCache);
        assert!(!recovery_target_admissible(role));
    }

    fn temp_workspace(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("commandagent-artifact-graph-{name}-{nanos}"));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        root
    }
}
