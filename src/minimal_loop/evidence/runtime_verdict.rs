//! Pure satisfaction verdict for runtime acceptance.
//!
//! `evidence.rs` reads the observations (workspace scan, investigation
//! binding, browser evidence, artifact obligations); this leaf decides which
//! requirements are missing, weak or satisfied from them and assembles the
//! report. Priority between capabilities, evidence and obligations is kept
//! exactly as the entrypoints produced it.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use super::{
    ArtifactObligationEvidence, BrowserInteractionEvidence, EvidenceTier, RuntimeAcceptanceReport,
    WorkspaceEvidence, capability_evidence_bindings, collect_route_unbound_capability_evidence,
    collect_weak_obligation_evidence, collect_weak_verify_evidence, evidence_kinds_for_capability,
    failure_or_collision_signal, generic_interactive_contract_required,
    generic_interactive_evidence_key, has_assertion_or_test_evidence, has_bound_verify_command,
    has_build_command_or_dependency_boundary, has_challenge_or_adversary_evidence,
    has_implementation_artifact, has_interactive_ui_source, has_live_preview_evidence,
    has_nextjs_route_evidence, has_non_static_screen_evidence, has_persistence_evidence,
    has_stateful_update_evidence, has_test_artifact, has_visible_interactive_surface_evidence,
    missing_required_obligations, obligation_repair_targets, prune_non_gate_relevant_weak_evidence,
    record_bool_evidence_tier, record_browser_acceptance_evidence,
    record_generic_interactive_contract_evidence, record_source_signal,
    restart_or_recoverable_state_signal, score_or_progression_signal,
    source_first_completion_authority_required, user_input_handler_signal, weak_sources,
};
use crate::planner::adjudication::evaluate_requirements;

/// What the caller asked acceptance to prove.
pub(super) struct Requirements<'a> {
    pub(super) paths: &'a [String],
    pub(super) verify_commands: &'a [String],
    pub(super) capabilities: &'a [String],
    pub(super) evidence: &'a [String],
    pub(super) obligations: &'a [String],
    pub(super) deferred_verify_requirements: &'a [String],
    pub(super) hint_tokens: &'a [String],
}

/// Everything read from the workspace before the verdict is taken.
pub(super) struct Observations<'a> {
    pub(super) root: &'a Path,
    pub(super) workspace: WorkspaceEvidence,
    pub(super) investigation_fully_bound: bool,
    pub(super) investigation_reason: String,
    pub(super) browser_state_dimension_scan_text: String,
    pub(super) artifact_obligations: Vec<ArtifactObligationEvidence>,
    pub(super) browser_interaction: Option<BrowserInteractionEvidence>,
    pub(super) route_diagnostics: Vec<String>,
}

#[derive(Default)]
struct Findings {
    missing_evidence: Vec<String>,
    weak_evidence: Vec<String>,
    diagnostics: Vec<String>,
    evidence_tiers: BTreeMap<String, String>,
    inconclusive_reasons: Vec<String>,
}

pub(super) fn judge(req: &Requirements<'_>, obs: Observations<'_>) -> RuntimeAcceptanceReport {
    let (required, missing_capabilities) = required_evidence_set(req);
    let generic_interactive_contract = generic_interactive_contract_required(req.capabilities);
    let mut findings = Findings::default();
    if let Some(evidence) = &obs.browser_interaction {
        record_browser_acceptance_evidence(
            evidence,
            &mut findings.missing_evidence,
            &mut findings.inconclusive_reasons,
        );
    }
    for evidence in &required {
        if generic_interactive_contract && generic_interactive_evidence_key(evidence) {
            continue;
        }
        record_required_evidence(evidence, req, &obs, &mut findings);
    }
    if generic_interactive_contract {
        record_generic_interactive_contract_evidence(
            &obs.workspace,
            &required,
            &mut findings.missing_evidence,
            &mut findings.evidence_tiers,
            &mut findings.diagnostics,
        );
    }
    findings
        .diagnostics
        .extend(obs.route_diagnostics.iter().cloned());

    collect_weak_verify_evidence(
        req.verify_commands,
        &obs.workspace,
        &mut findings.weak_evidence,
    );
    if !generic_interactive_contract {
        collect_route_unbound_capability_evidence(
            &obs.workspace,
            &required,
            &findings.missing_evidence,
            req.hint_tokens,
            &obs.browser_state_dimension_scan_text,
            &mut findings.weak_evidence,
            &mut findings.diagnostics,
        );
    }
    collect_weak_obligation_evidence(
        &obs.artifact_obligations,
        &required,
        &mut findings.weak_evidence,
    );
    let missing_obligations = missing_required_obligations(
        req.obligations,
        &obs.artifact_obligations,
        &obs.workspace,
        generic_interactive_contract,
        obs.investigation_fully_bound,
    );
    let capability_evidence_bindings = capability_evidence_bindings(
        req.capabilities,
        &obs.artifact_obligations,
        &findings.missing_evidence,
    );
    let obligation_repair_targets = obligation_repair_targets(
        req.paths,
        &obs.artifact_obligations,
        &obs.workspace,
        &missing_obligations,
    );
    findings.diagnostics.sort();
    findings.diagnostics.dedup();
    prune_non_gate_relevant_weak_evidence(
        &mut findings.weak_evidence,
        &findings.missing_evidence,
        &findings.diagnostics,
    );
    let weak_evidence_blocks_completion = !findings.weak_evidence.is_empty()
        && source_first_completion_authority_required(
            req.capabilities,
            req.evidence,
            req.obligations,
        );
    let evaluation = evaluate_requirements(
        req.capabilities,
        req.evidence,
        req.obligations,
        &missing_capabilities,
        &findings.missing_evidence,
        &missing_obligations,
        &[],
        &findings.inconclusive_reasons,
        &findings.weak_evidence,
        weak_evidence_blocks_completion,
    );
    let browser_interaction = obs.browser_interaction.unwrap_or_default();

    RuntimeAcceptanceReport {
        passed: evaluation.passed,
        inconclusive: evaluation.inconclusive,
        missing_capabilities,
        missing_evidence: findings.missing_evidence,
        missing_obligations,
        weak_evidence_sources: weak_sources::collect(
            &findings.weak_evidence,
            req.verify_commands,
            &obs.workspace,
        ),
        weak_evidence: findings.weak_evidence,
        diagnostics: findings.diagnostics,
        unverified_evidence: Vec::new(),
        evidence_tiers: findings.evidence_tiers,
        inconclusive_reasons: findings.inconclusive_reasons,
        artifact_obligations: obs.artifact_obligations,
        capability_evidence_bindings,
        obligation_repair_targets,
        browser_readiness_status: browser_interaction.browser_readiness_status,
        browser_readiness_evidence_path: browser_interaction.browser_readiness_evidence_path,
        interaction_evidence_status: browser_interaction.interaction_evidence_status,
        interaction_evidence_path: browser_interaction.interaction_evidence_path,
        primary_reason: evaluation.primary_reason,
    }
}

/// Evidence keys the capabilities and explicit requirements demand, plus the
/// capabilities that no evidence kind or manifest check understands.
fn required_evidence_set(req: &Requirements<'_>) -> (BTreeSet<String>, Vec<String>) {
    let mut required = BTreeSet::new();
    let mut missing_capabilities = Vec::new();
    for capability in req.capabilities {
        let kinds = evidence_kinds_for_capability(capability);
        if kinds.is_empty()
            && !crate::planner::profiles::data::manifest::is_manifest_check_id(capability)
            && !crate::planner::profiles::ingest::manifest::is_manifest_check_id(capability)
        {
            missing_capabilities.push(format!("unsupported_required_capability:{capability}"));
        }
        for kind in kinds {
            required.insert(kind.as_str().to_string());
        }
    }
    for evidence in req.evidence {
        let trimmed = evidence.trim();
        if !trimmed.is_empty() {
            required.insert(trimmed.to_string());
        }
    }
    (required, missing_capabilities)
}

fn record_required_evidence(
    evidence: &str,
    req: &Requirements<'_>,
    obs: &Observations<'_>,
    findings: &mut Findings,
) {
    let workspace = &obs.workspace;
    let scan_text = &obs.browser_state_dimension_scan_text;
    let present = match evidence {
        "implementation_artifact" => has_implementation_artifact(obs.root, req.paths, workspace),
        "test_artifact" => has_test_artifact(workspace),
        "bound_verify_command" => has_bound_verify_command(req.verify_commands, workspace),
        "non_zero_test_or_assertion_evidence" => has_assertion_or_test_evidence(workspace),
        "build_command_or_dependency_missing_boundary" => has_build_command_or_dependency_boundary(
            req.verify_commands,
            req.deferred_verify_requirements,
            workspace,
        ),
        "interactive_ui_source_evidence" => has_interactive_ui_source(workspace),
        "non_static_screen_evidence" => has_non_static_screen_evidence(workspace),
        "visible_interactive_surface_evidence" => {
            has_visible_interactive_surface_evidence(workspace)
        }
        "stateful_update_evidence" => has_stateful_update_evidence(workspace),
        "challenge_or_adversary_evidence" => {
            has_challenge_or_adversary_evidence(workspace, req.hint_tokens, scan_text)
        }
        "persistence_evidence" => has_persistence_evidence(workspace),
        "live_preview_evidence" => has_live_preview_evidence(workspace),
        "nextjs_route_evidence" => has_nextjs_route_evidence(workspace),
        "requested_content_evidence" => {
            workspace.readme || req.paths.iter().any(|path| path.ends_with(".md"))
        }
        "investigation_binding" => {
            if !obs.investigation_fully_bound {
                findings.diagnostics.push(format!(
                    "investigation_binding:{}",
                    obs.investigation_reason
                ));
            }
            obs.investigation_fully_bound
        }
        _ => return record_signal_or_unknown(evidence, workspace, scan_text, findings),
    };
    record_bool_evidence_tier(
        evidence,
        present,
        &mut findings.missing_evidence,
        &mut findings.evidence_tiers,
    );
}

fn record_signal_or_unknown(
    evidence: &str,
    workspace: &WorkspaceEvidence,
    scan_text: &str,
    findings: &mut Findings,
) {
    let signal = match evidence {
        "user_input_handler_evidence" => user_input_handler_signal(workspace),
        "score_or_progression_evidence" => score_or_progression_signal(workspace, scan_text),
        "failure_or_collision_evidence" => failure_or_collision_signal(workspace, scan_text),
        "restart_or_recoverable_state_evidence" => restart_or_recoverable_state_signal(workspace),
        unknown => {
            findings
                .missing_evidence
                .push(format!("unsupported_required_evidence:{unknown}"));
            findings.evidence_tiers.insert(
                unknown.to_string(),
                EvidenceTier::Absent.as_str().to_string(),
            );
            return;
        }
    };
    record_source_signal(
        evidence,
        signal,
        &mut findings.missing_evidence,
        &mut findings.weak_evidence,
        &mut findings.evidence_tiers,
    );
}

#[cfg(test)]
#[path = "runtime_verdict/tests.rs"]
mod tests;
