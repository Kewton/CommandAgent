//! Deterministic recovery-contract projection.
//!
//! This module is not an executor. It turns already-detected contract
//! evidence into the semantic labels and repair boundaries rendered into the
//! existing bounded repair packet.

use crate::agent::step_runner::artifact_graph::{ArtifactGraph, ArtifactRole};
use crate::agent::step_runner::artifact_ownership::classify_artifact_ownership;
use crate::agent::step_runner::correction_evidence::ContractEvidence;
use crate::agent::step_runner::workspace_scope::WorkspaceScope;

pub(crate) fn active_job_priority(job: &str) -> u8 {
    match job {
        "explicit_stop" | "contract_conflict" => 0,
        "tool_protocol_correction" => 5,
        "setup_bootstrap" => 10,
        "verifier_contract_correction" => 15,
        "manifest_repair" => 20,
        "scaffold_materialization" => 30,
        "route_integration_repair" => 40,
        "source_implementation_repair" => 50,
        "evidence_binding_repair" => 55,
        "test_artifact_completion" => 60,
        "test_alignment_repair" => 70,
        "documentation_repair" => 80,
        _ => 200,
    }
}

pub(crate) fn semantic_failure_kind(
    evidence: &ContractEvidence,
    job: &str,
    action: &str,
    role: Option<ArtifactRole>,
) -> &'static str {
    match job {
        "setup_bootstrap" => "setup_dependency_missing",
        "manifest_repair" => "setup_manifest_contract_failure",
        "scaffold_materialization" | "test_artifact_completion" => "missing_required_artifact",
        "route_integration_repair" => "route_integration_failure",
        "source_implementation_repair" => match role {
            Some(ArtifactRole::Entrypoint) => "entrypoint_implementation_failure",
            Some(ArtifactRole::IntegrationTarget) => "integration_target_implementation_failure",
            _ => "source_implementation_failure",
        },
        "test_alignment_repair" => "test_or_verifier_alignment_failure",
        "documentation_repair" => "documentation_contract_failure",
        "evidence_binding_repair" => "evidence_binding_failure",
        "verifier_contract_correction" => "verifier_contract_failure",
        "tool_protocol_correction" => "tool_protocol_failure",
        "explicit_stop" => "unadmitted_recovery_failure",
        "contract_conflict" => "contract_conflict_failure",
        _ if action == "stop_with_structured_evidence" => "unadmitted_recovery_failure",
        _ if evidence.guard == "profile_verification" => "profile_contract_failure",
        _ if evidence.guard == "verifier" => "verifier_failure",
        _ => "contract_failure",
    }
}

pub(crate) fn source_of_truth(evidence: &ContractEvidence, job: &str) -> &'static str {
    match job {
        "setup_bootstrap" | "manifest_repair" => "setup_manifest_and_dependency_diagnostic",
        "scaffold_materialization" | "route_integration_repair" => "profile_contract",
        "source_implementation_repair" => "original_verifier_diagnostic",
        "test_artifact_completion" | "test_alignment_repair" => {
            "test_contract_and_original_verifier"
        }
        "documentation_repair" => "documentation_contract",
        "evidence_binding_repair" => "deterministic_evidence_binding_contract",
        "verifier_contract_correction" => "verifier_contract",
        "tool_protocol_correction" => "tool_schema_contract",
        "explicit_stop" => "deterministic_guard",
        "contract_conflict" => "deterministic_contract_conflict",
        _ if evidence.guard == "profile_verification" => "profile_contract",
        _ if evidence.guard == "verifier" => "original_verifier_diagnostic",
        _ => "deterministic_guard",
    }
}

pub(crate) fn allowed_change_kind(
    job: &str,
    action: &str,
    role: Option<ArtifactRole>,
) -> &'static str {
    match job {
        "setup_bootstrap" => "none_model_must_stop_for_verifier_owned_setup",
        "manifest_repair" => "setup_manifest_or_config_only",
        "scaffold_materialization" => "create_missing_required_artifact_only",
        "route_integration_repair" => "route_or_integration_target_only",
        "source_implementation_repair" => match role {
            Some(ArtifactRole::Entrypoint) => "entrypoint_source_only",
            Some(ArtifactRole::IntegrationTarget) => "integration_target_source_only",
            _ => "implementation_source_only",
        },
        "test_artifact_completion" => "test_artifact_creation_only",
        "test_alignment_repair" => "test_or_verifier_alignment_only",
        "documentation_repair" => "documentation_literal_only",
        "evidence_binding_repair" => "evidence_binding_only",
        "verifier_contract_correction" => "plan_or_verifier_contract_only",
        "tool_protocol_correction" => "tool_call_shape_only",
        "explicit_stop" => "no_change_admitted",
        "contract_conflict" => "no_change_admitted",
        _ if action == "stop_with_structured_evidence" => "no_change_admitted",
        _ => "classified_contract_repair_only",
    }
}

pub(crate) fn expected_evidence_delta(
    evidence: &ContractEvidence,
    job: &str,
    action: &str,
) -> String {
    match job {
        "setup_bootstrap" => {
            "verifier-owned setup runs once, then the original verifier reruns".to_string()
        }
        "manifest_repair" => {
            "setup manifest/config satisfies the dependency or script contract before setup/verifier rerun".to_string()
        }
        "scaffold_materialization" => "missing required artifact exists before route/source repair".to_string(),
        "route_integration_repair" => {
            "selected route imports or renders the existing target, then profile verification passes".to_string()
        }
        "source_implementation_repair" => evidence
            .command
            .as_deref()
            .map(|command| {
                format!(
                    "original verifier `{command}` changes diagnostic or passes without verifier weakening"
                )
            })
            .unwrap_or_else(|| {
                "original verifier diagnostic changes or passes without verifier weakening".to_string()
            }),
        "test_artifact_completion" => {
            "required test artifact exists before implementation repair continues".to_string()
        }
        "test_alignment_repair" => evidence
            .command
            .as_deref()
            .map(|command| format!("test/verifier target aligns honestly, then `{command}` reruns"))
            .unwrap_or_else(|| "test/verifier target aligns honestly, then verifier reruns".to_string()),
        "documentation_repair" => "documented literal or observed/expected mismatch is updated exactly".to_string(),
        "evidence_binding_repair" => {
            "required evidence binding is attached to the target artifact, then the original guard/verifier reruns".to_string()
        }
        "verifier_contract_correction" => "corrected plan/verifier contract is re-linted before source mutation".to_string(),
        "tool_protocol_correction" => "next model response contains exactly one valid tool call".to_string(),
        "explicit_stop" => "no mutation occurs; user-visible failure remains structured".to_string(),
        "contract_conflict" => {
            "no mutation occurs; conflicting recovery candidates remain user-visible".to_string()
        }
        _ if action == "stop_with_structured_evidence" => {
            "no mutation occurs; user-visible failure remains structured".to_string()
        }
        _ => "classified deterministic contract changes before rerun".to_string(),
    }
}

pub(crate) fn workspace_scope(target: Option<&str>, role: Option<ArtifactRole>) -> &'static str {
    let Some(target) = target else {
        return "no_admitted_workspace_scope";
    };
    if target.starts_with("step:") {
        return "step_contract_scope";
    }
    match role {
        Some(ArtifactRole::GeneratedOutput | ArtifactRole::DependencyCache) => {
            "excluded_generated_or_dependency_scope"
        }
        Some(ArtifactRole::SetupManifest | ArtifactRole::SetupConfig) => "setup_artifact_scope",
        Some(ArtifactRole::Entrypoint | ArtifactRole::IntegrationTarget) => {
            "route_integration_scope"
        }
        Some(ArtifactRole::Implementation) => "implementation_artifact_scope",
        Some(ArtifactRole::Test) => "test_artifact_scope",
        Some(ArtifactRole::Docs) => "documentation_artifact_scope",
        Some(ArtifactRole::Unknown) | None => "workspace_artifact_scope",
    }
}

pub(crate) fn artifact_ownership(
    graph: &ArtifactGraph,
    target: Option<&str>,
    role: Option<ArtifactRole>,
) -> String {
    let Some(target) = target else {
        return "none: no admitted target".to_string();
    };
    if target.starts_with("step:") {
        return "step-owned contract target".to_string();
    }
    let inferred_role = role.unwrap_or_else(|| {
        crate::agent::step_runner::artifact_graph::role_for_path(
            target,
            crate::agent::step_runner::artifact_graph::ArtifactLifecycle::Required,
        )
    });
    let scope = WorkspaceScope::from_graph(graph);
    let source = graph
        .node(target)
        .map(|node| node.source.as_str())
        .unwrap_or("inferred_from_path");
    let decision = classify_artifact_ownership(graph, &scope, target, inferred_role, source, &[]);
    format!(
        "{}: role={} reason={} workspace_scope={}",
        decision.ownership.as_str(),
        decision.role.as_str(),
        decision.reason,
        scope.summary()
    )
}
