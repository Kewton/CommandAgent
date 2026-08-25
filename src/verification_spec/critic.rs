//! Non-authoritative critic judgment and deterministic validation.
//!
//! The provider may classify a proposal, but only this module decides whether
//! the shadow critic evidence is internally valid. No result from this module
//! grants command execution or changes an authoritative verdict.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{EvidenceStrength, ExpectedPolarity, OracleInput, OracleObservation, OracleSetup};
use crate::tools::path_guard::validate_workspace_relative;

pub const CRITIC_SCHEMA_VERSION: &str = "commandagent.verification_spec.critic.v0";
pub const CRITIC_PROMPT_VERSION: &str = "commandagent.verification_spec.critic.prompt.v0";
pub const RESOURCE_BUDGET_VERSION: &str = "commandagent.goal_verify.phase0.resource_budget.v0";
const MAX_CRITIC_BYTES: usize = 16_384;
const MAX_ISSUES: usize = 32;
const MAX_REASON_BYTES: usize = 2_048;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CriticDecision {
    Accept,
    Reject,
    Unverified,
}

/// Provider-owned classification. Parsing this schema does not validate the
/// oracle contract, lineage, counterfactual, or resource envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CriticJudgment {
    pub schema_version: String,
    pub prompt_version: String,
    pub run_id: String,
    pub model: String,
    pub request_id: String,
    pub decision: CriticDecision,
    pub issue_codes: Vec<String>,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OracleContract {
    pub claim_id: String,
    pub expected_polarity: ExpectedPolarity,
    pub minimum_strength: EvidenceStrength,
    pub input: OracleInput,
    pub observation: OracleObservation,
    pub setup: OracleSetup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LineageStage {
    Freeze,
    Bind,
    Execute,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LineageCheckpoint {
    pub stage: LineageStage,
    pub artifact_sha256: String,
    pub epoch: u64,
    pub run_id: String,
    pub model: String,
    pub prompt_version: String,
    pub schema_version: String,
    pub request_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CriticLineage {
    pub frozen: OracleContract,
    pub bound: OracleContract,
    pub freeze: LineageCheckpoint,
    pub bind: LineageCheckpoint,
    pub execute: LineageCheckpoint,
    pub semantic_equivalence: bool,
    pub concretization_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum CounterfactualEvidence {
    Generated {
        frozen_contract_sha256: String,
        executed: bool,
        discriminated: bool,
        evidence_path: String,
    },
    Absent {
        reason: String,
    },
    Unavailable {
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CriticResourceBudget {
    pub budget_version: String,
    pub max_total_tokens: u64,
    pub max_latency_ms: u64,
    pub max_retries: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CriticResourceUsage {
    pub total_tokens: u64,
    pub latency_ms: u64,
    pub retries: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CriticGeneration {
    Generated(CriticJudgment),
    Unavailable { reason: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CriticValidationStatus {
    Verified,
    Rejected,
    Unverified,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CriticValidation {
    pub schema_version: &'static str,
    pub shadow_only: bool,
    pub authoritative_verdict_changed: bool,
    pub candidate_execution_authorized: bool,
    pub status: CriticValidationStatus,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CriticShadowObservation<T> {
    pub authoritative: T,
    pub validation: CriticValidation,
}

pub fn parse_critic_judgment(raw: &str) -> Result<CriticJudgment, String> {
    if raw.len() > MAX_CRITIC_BYTES {
        return Err("critic_input_too_large".to_string());
    }
    let judgment: CriticJudgment =
        serde_json::from_str(raw).map_err(|error| format!("critic_schema_invalid:{error}"))?;
    let mut errors = Vec::new();
    if judgment.schema_version != CRITIC_SCHEMA_VERSION {
        errors.push("critic_schema_version_mismatch".to_string());
    }
    if judgment.prompt_version != CRITIC_PROMPT_VERSION {
        errors.push("critic_prompt_version_mismatch".to_string());
    }
    for (name, value) in [
        ("run_id", judgment.run_id.as_str()),
        ("model", judgment.model.as_str()),
        ("request_id", judgment.request_id.as_str()),
    ] {
        if value.is_empty() || value.len() > MAX_REASON_BYTES {
            errors.push(format!("critic_{name}_invalid"));
        }
    }
    if judgment.issue_codes.len() > MAX_ISSUES
        || judgment.issue_codes.iter().any(|code| {
            code.is_empty()
                || code.len() > 64
                || !code
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
        })
    {
        errors.push("critic_issue_codes_invalid".to_string());
    }
    if judgment.rationale.is_empty() || judgment.rationale.len() > MAX_REASON_BYTES {
        errors.push("critic_rationale_invalid".to_string());
    }
    if judgment.decision == CriticDecision::Reject && judgment.issue_codes.is_empty() {
        errors.push("critic_reject_reason_missing".to_string());
    }
    if errors.is_empty() {
        Ok(judgment)
    } else {
        errors.sort();
        errors.dedup();
        Err(errors.join(","))
    }
}

pub fn checkpoint(
    stage: LineageStage,
    contract: &OracleContract,
    epoch: u64,
    run_id: &str,
    model: &str,
    request_id: &str,
) -> LineageCheckpoint {
    LineageCheckpoint {
        stage,
        artifact_sha256: contract_sha256(contract),
        epoch,
        run_id: run_id.to_string(),
        model: model.to_string(),
        prompt_version: CRITIC_PROMPT_VERSION.to_string(),
        schema_version: CRITIC_SCHEMA_VERSION.to_string(),
        request_id: request_id.to_string(),
    }
}

pub fn observe_critic<T: Clone>(
    authoritative: &T,
    generation: &CriticGeneration,
    lineage: &CriticLineage,
    counterfactual: &CounterfactualEvidence,
    budget: &CriticResourceBudget,
    usage: &CriticResourceUsage,
) -> CriticShadowObservation<T> {
    CriticShadowObservation {
        authoritative: authoritative.clone(),
        validation: validate_critic(generation, lineage, counterfactual, budget, usage),
    }
}

pub fn validate_critic(
    generation: &CriticGeneration,
    lineage: &CriticLineage,
    counterfactual: &CounterfactualEvidence,
    budget: &CriticResourceBudget,
    usage: &CriticResourceUsage,
) -> CriticValidation {
    let mut rejected = Vec::new();
    let mut unverified = Vec::new();
    validate_lineage(lineage, &mut rejected);
    validate_counterfactual(lineage, counterfactual, &mut rejected, &mut unverified);
    validate_resources(budget, usage, &mut unverified);

    match generation {
        CriticGeneration::Unavailable { reason } => unverified.push(if reason.trim().is_empty() {
            "critic_provider_unavailable:reason_missing".to_string()
        } else {
            format!("critic_provider_unavailable:{reason}")
        }),
        CriticGeneration::Generated(judgment) => {
            validate_judgment_lineage(judgment, lineage, &mut rejected);
            match judgment.decision {
                CriticDecision::Reject => {
                    rejected.push("critic_rejected".to_string());
                    rejected.extend(
                        judgment
                            .issue_codes
                            .iter()
                            .map(|code| format!("critic_rejected:{code}")),
                    );
                }
                CriticDecision::Unverified => {
                    unverified.push("critic_decision_unverified".to_string())
                }
                CriticDecision::Accept => {}
            }
        }
    }
    rejected.sort();
    rejected.dedup();
    unverified.sort();
    unverified.dedup();
    let (status, reasons) = if !rejected.is_empty() {
        (CriticValidationStatus::Rejected, rejected)
    } else if !unverified.is_empty() {
        (CriticValidationStatus::Unverified, unverified)
    } else {
        (CriticValidationStatus::Verified, Vec::new())
    };
    CriticValidation {
        schema_version: CRITIC_SCHEMA_VERSION,
        shadow_only: true,
        authoritative_verdict_changed: false,
        candidate_execution_authorized: false,
        status,
        reasons,
    }
}

fn validate_lineage(lineage: &CriticLineage, rejected: &mut Vec<String>) {
    let checkpoints = [&lineage.freeze, &lineage.bind, &lineage.execute];
    let expected_stages = [
        LineageStage::Freeze,
        LineageStage::Bind,
        LineageStage::Execute,
    ];
    for (checkpoint, expected_stage) in checkpoints.iter().zip(expected_stages) {
        if checkpoint.stage != expected_stage {
            rejected.push(format!("lineage_stage_mismatch:{expected_stage:?}"));
        }
        if checkpoint.run_id.is_empty()
            || checkpoint.model.is_empty()
            || checkpoint.request_id.is_empty()
        {
            rejected.push(format!("lineage_provenance_missing:{expected_stage:?}"));
        }
        if checkpoint.run_id != lineage.freeze.run_id
            || checkpoint.model != lineage.freeze.model
            || checkpoint.prompt_version != CRITIC_PROMPT_VERSION
            || checkpoint.schema_version != CRITIC_SCHEMA_VERSION
            || checkpoint.request_id != lineage.freeze.request_id
        {
            rejected.push(format!("lineage_provenance_mismatch:{expected_stage:?}"));
        }
    }
    if !(lineage.freeze.epoch < lineage.bind.epoch && lineage.bind.epoch < lineage.execute.epoch) {
        rejected.push("lineage_epoch_not_monotonic".to_string());
    }
    if lineage.freeze.artifact_sha256 != contract_sha256(&lineage.frozen) {
        rejected.push("freeze_hash_mismatch".to_string());
    }
    if lineage.bind.artifact_sha256 != contract_sha256(&lineage.bound)
        || lineage.execute.artifact_sha256 != lineage.bind.artifact_sha256
    {
        rejected.push("bind_execute_hash_mismatch".to_string());
    }
    validate_semantics(lineage, rejected);
    validate_setup(&lineage.frozen.setup, rejected);
    validate_setup(&lineage.bound.setup, rejected);
}

fn validate_semantics(lineage: &CriticLineage, rejected: &mut Vec<String>) {
    let frozen = &lineage.frozen;
    let bound = &lineage.bound;
    if frozen.claim_id != bound.claim_id {
        rejected.push("claim_identity_changed".to_string());
    }
    if frozen.expected_polarity != bound.expected_polarity {
        rejected.push("expected_polarity_changed".to_string());
    }
    if strength_rank(bound.minimum_strength) < strength_rank(frozen.minimum_strength) {
        rejected.push("minimum_strength_weakened".to_string());
    }
    if frozen.input != bound.input {
        rejected.push("oracle_input_changed".to_string());
    }
    if frozen.observation != bound.observation {
        rejected.push("expected_observation_changed".to_string());
    }
    let setup_changed = frozen.setup != bound.setup;
    if setup_changed && !lineage.semantic_equivalence {
        rejected.push("concretization_not_semantically_equivalent".to_string());
    }
    if setup_changed
        && lineage
            .concretization_reason
            .as_deref()
            .is_none_or(|reason| reason.trim().is_empty())
    {
        rejected.push("concretization_reason_missing".to_string());
    }
}

fn validate_setup(setup: &OracleSetup, rejected: &mut Vec<String>) {
    if validate_workspace_relative(&setup.cwd).is_err()
        || setup
            .fixture_paths
            .iter()
            .any(|path| validate_workspace_relative(path).is_err())
    {
        rejected.push("concretized_path_unsafe".to_string());
    }
    if !setup.argv.is_empty()
        && crate::planner::declarative_command_checks::validate_shadow_argv(&setup.argv).is_err()
    {
        rejected.push("concretized_argv_policy_rejected".to_string());
    }
}

fn validate_counterfactual(
    lineage: &CriticLineage,
    counterfactual: &CounterfactualEvidence,
    rejected: &mut Vec<String>,
    unverified: &mut Vec<String>,
) {
    match counterfactual {
        CounterfactualEvidence::Generated {
            frozen_contract_sha256,
            executed,
            discriminated,
            evidence_path,
        } => {
            if frozen_contract_sha256 != &lineage.freeze.artifact_sha256 {
                rejected.push("counterfactual_lineage_mismatch".to_string());
            }
            if validate_workspace_relative(evidence_path).is_err() {
                rejected.push("counterfactual_evidence_path_unsafe".to_string());
            }
            if !executed {
                unverified.push("counterfactual_not_executed".to_string());
            } else if !discriminated {
                unverified.push("counterfactual_not_discriminating".to_string());
            }
        }
        CounterfactualEvidence::Absent { reason } => {
            unverified.push(reason_code("counterfactual_absent", reason))
        }
        CounterfactualEvidence::Unavailable { reason } => {
            unverified.push(reason_code("counterfactual_unavailable", reason))
        }
    }
}

fn validate_resources(
    budget: &CriticResourceBudget,
    usage: &CriticResourceUsage,
    unverified: &mut Vec<String>,
) {
    if budget.budget_version != RESOURCE_BUDGET_VERSION
        || budget.max_total_tokens == 0
        || budget.max_latency_ms == 0
    {
        unverified.push("phase0_resource_budget_invalid".to_string());
        return;
    }
    if usage.total_tokens > budget.max_total_tokens {
        unverified.push("critic_token_budget_exceeded".to_string());
    }
    if usage.latency_ms > budget.max_latency_ms {
        unverified.push("critic_latency_budget_exceeded".to_string());
    }
    if usage.retries > budget.max_retries {
        unverified.push("critic_retry_budget_exceeded".to_string());
    }
}

fn validate_judgment_lineage(
    judgment: &CriticJudgment,
    lineage: &CriticLineage,
    rejected: &mut Vec<String>,
) {
    if judgment.rationale.trim().is_empty()
        || judgment.rationale.len() > MAX_REASON_BYTES
        || judgment.issue_codes.len() > MAX_ISSUES
        || judgment.issue_codes.iter().any(|code| {
            code.is_empty()
                || code.len() > 64
                || !code
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
        })
    {
        rejected.push("critic_judgment_schema_invalid".to_string());
    }
    if judgment.schema_version != CRITIC_SCHEMA_VERSION
        || judgment.prompt_version != CRITIC_PROMPT_VERSION
        || judgment.run_id != lineage.freeze.run_id
        || judgment.model != lineage.freeze.model
        || judgment.request_id != lineage.freeze.request_id
    {
        rejected.push("critic_lineage_mismatch".to_string());
    }
}

fn reason_code(prefix: &str, reason: &str) -> String {
    if reason.trim().is_empty() {
        format!("{prefix}:reason_missing")
    } else {
        format!("{prefix}:{reason}")
    }
}

fn strength_rank(strength: EvidenceStrength) -> u8 {
    match strength {
        EvidenceStrength::Weak => 0,
        EvidenceStrength::Deterministic => 1,
        EvidenceStrength::Runtime => 2,
    }
}

fn contract_sha256(contract: &OracleContract) -> String {
    let bytes = serde_json::to_vec(contract).expect("OracleContract serialization is infallible");
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}
