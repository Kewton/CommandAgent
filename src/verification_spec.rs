//! Versioned, non-authoritative VerificationSpec shadow artifacts.
//!
//! This module deliberately has no dependency on adjudication or execution.
//! A shadow failure is diagnostic data and cannot alter the caller's verdict.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::tools::path_guard::validate_workspace_relative;

pub mod create_shadow;

pub const SCHEMA_VERSION: &str = "commandagent.verification_spec.v0";
pub const PROMPT_VERSION: &str = "commandagent.verification_spec.prompt.v0";
pub const MAX_INPUT_BYTES: usize = 65_536;
pub const MAX_GOAL_BYTES: usize = 8_192;
pub const MAX_CLAIMS: usize = 64;
pub const MAX_ORACLES: usize = 64;
pub const MAX_ORACLES_PER_CLAIM: usize = 16;
pub const MAX_FIXTURES: usize = 64;
pub const MAX_ID_BYTES: usize = 64;
pub const MAX_STATEMENT_BYTES: usize = 2_048;
pub const MAX_PATH_BYTES: usize = 1_024;
pub const MAX_ARGV: usize = 32;
pub const MAX_ARG_BYTES: usize = 4_096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationIntent {
    Create,
    Fix,
    Investigate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VerificationSpec {
    pub schema_version: String,
    pub prompt_version: String,
    pub goal: String,
    pub goal_sha256: String,
    pub provenance: GoalProvenance,
    pub intent: VerificationIntent,
    pub profile: String,
    pub generation: GenerationProvenance,
    pub claims: Vec<AcceptanceClaim>,
    pub oracles: Vec<Oracle>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GoalProvenance {
    pub source: String,
    pub provider_goal_sha256: String,
    pub provider_goal_matched: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GenerationProvenance {
    pub provider: String,
    pub model: String,
    pub request_id: String,
    #[serde(default)]
    pub raw_response_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptanceClaim {
    pub id: String,
    pub origin: ClaimOrigin,
    pub normalized_requirement: String,
    pub required: bool,
    pub kind: ClaimKind,
    pub oracle_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "source_kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ClaimOrigin {
    Goal {
        start_byte: usize,
        end_byte: usize,
    },
    FixRequirement {
        artifact_path: String,
        requirement_id: String,
        stage: String,
        expected_polarity: ExpectedPolarity,
        lineage: String,
        epoch: u64,
    },
    InvestigationRequirement {
        artifact_path: String,
        requirement_id: String,
        binding_id: String,
        stage: String,
        lineage: String,
        epoch: u64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimKind {
    Behavior,
    State,
    NegativeCondition,
    Regression,
    ReproducerObservation,
    DiagnosisBinding,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Oracle {
    pub id: String,
    pub claim_id: String,
    pub strategy: OracleStrategy,
    pub expected_polarity: ExpectedPolarity,
    pub minimum_strength: EvidenceStrength,
    pub observed_strength: Option<EvidenceStrength>,
    pub setup: OracleSetup,
    pub input: OracleInput,
    pub observation: OracleObservation,
    pub timeout_ms: u64,
    pub lifecycle: OracleLifecycle,
    pub result: OracleResult,
    pub lineage: BindingLineage,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OracleSetup {
    pub argv: Vec<String>,
    pub cwd: String,
    pub fixture_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum OracleInput {
    None,
    Text {
        value: String,
    },
    Fixture {
        path: String,
        sha256: String,
    },
    Http {
        method: String,
        port: u16,
        path: String,
    },
    Dom {
        route: String,
        selector: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum OracleObservation {
    ExitCode { expected: i32 },
    Stdout { expected: String },
    Stderr { expected: String },
    File { path: String, exists: bool },
    HttpStatus { expected: u16 },
    Dom { expected: String },
    Interaction { expected: String },
    ExistingBinding { artifact_path: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BindingLineage {
    pub proposed_binding_sha256: String,
    pub concretized_binding_sha256: String,
    pub semantic_equivalence: bool,
    pub repair_kind: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OracleStrategy {
    Command,
    Fixture,
    ExitCode,
    Stdout,
    Stderr,
    File,
    Http,
    Dom,
    Interaction,
    ExistingFixEvidence,
    ExistingInvestigationBinding,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExpectedPolarity {
    Success,
    Failure,
    Present,
    Absent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceStrength {
    Weak,
    Deterministic,
    Runtime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OracleLifecycle {
    Proposed,
    Validated,
    Bound,
    Executed,
    Blocked,
    Unverified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OracleResult {
    Pass,
    Fail,
    Partial,
    Unverified,
    Blocked,
    OracleError,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderVerificationSpec {
    schema_version: String,
    prompt_version: String,
    goal: String,
    intent: VerificationIntent,
    profile: String,
    generation: GenerationProvenance,
    claims: Vec<AcceptanceClaim>,
    oracles: Vec<Oracle>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ValidationError {
    pub codes: Vec<String>,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.codes.join(","))
    }
}

impl std::error::Error for ValidationError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ShadowFailureKind {
    SchemaInvalid,
    Timeout,
    EmptyClaims,
    ProviderUnavailable,
    PolicyRejected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ShadowFailure {
    pub kind: ShadowFailureKind,
    pub error: ValidationError,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ShadowGeneration {
    Generated(Box<VerificationSpec>),
    Rejected(ShadowFailure),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShadowObservation<T> {
    pub authoritative: T,
    pub generation: ShadowGeneration,
}

/// Parse a provider proposal while preserving the caller-owned goal.
pub fn parse_provider_spec(
    original_goal: &str,
    expected_intent: VerificationIntent,
    raw: &str,
) -> Result<VerificationSpec, ValidationError> {
    if raw.len() > MAX_INPUT_BYTES {
        return Err(error("input_too_large"));
    }
    let provider: ProviderVerificationSpec = serde_json::from_str(raw)
        .map_err(|parse_error| error(format!("schema_invalid:{parse_error}")))?;
    let mut codes = validate_provider(original_goal, expected_intent, &provider);
    if !codes.is_empty() {
        codes.sort();
        codes.dedup();
        return Err(ValidationError { codes });
    }
    let provider_goal_sha256 = sha256(provider.goal.as_bytes());
    let raw_response_sha256 = sha256(raw.as_bytes());
    let mut generation = provider.generation;
    generation.raw_response_sha256 = raw_response_sha256;
    Ok(VerificationSpec {
        schema_version: SCHEMA_VERSION.to_string(),
        prompt_version: PROMPT_VERSION.to_string(),
        goal: original_goal.to_string(),
        goal_sha256: sha256(original_goal.as_bytes()),
        provenance: GoalProvenance {
            source: "caller_original_goal".to_string(),
            provider_goal_matched: provider.goal == original_goal,
            provider_goal_sha256,
        },
        intent: provider.intent,
        profile: provider.profile,
        generation,
        claims: provider.claims,
        oracles: provider.oracles,
    })
}

/// Attach a shadow proposal to an authoritative result without giving the
/// proposal a mutation path to that result.
pub fn observe_shadow<T: Clone>(
    authoritative: &T,
    original_goal: &str,
    expected_intent: VerificationIntent,
    raw: &str,
) -> ShadowObservation<T> {
    let generation = match parse_provider_spec(original_goal, expected_intent, raw) {
        Ok(spec) => ShadowGeneration::Generated(Box::new(spec)),
        Err(error) => {
            let kind = if error.codes.iter().any(|code| code == "claim_count_invalid") {
                ShadowFailureKind::EmptyClaims
            } else {
                ShadowFailureKind::SchemaInvalid
            };
            ShadowGeneration::Rejected(ShadowFailure { kind, error })
        }
    };
    ShadowObservation {
        authoritative: authoritative.clone(),
        generation,
    }
}

/// Record a provider-side shadow failure such as timeout without involving an
/// authoritative result or event stream.
pub fn provider_failure(kind: ShadowFailureKind, detail: impl Into<String>) -> ShadowGeneration {
    ShadowGeneration::Rejected(ShadowFailure {
        kind,
        error: error(detail),
    })
}

/// Persist an optional shadow artifact only at a caller-selected path.
pub fn write_shadow_artifact(
    run_dir: &std::path::Path,
    generation: &ShadowGeneration,
) -> anyhow::Result<std::path::PathBuf> {
    std::fs::create_dir_all(run_dir)?;
    let path = run_dir.join("verification-spec-shadow.json");
    let mut bytes = serde_json::to_vec_pretty(generation)?;
    bytes.push(b'\n');
    std::fs::write(&path, bytes)?;
    Ok(path)
}

fn validate_provider(
    original_goal: &str,
    expected_intent: VerificationIntent,
    provider: &ProviderVerificationSpec,
) -> Vec<String> {
    let mut codes = Vec::new();
    if provider.schema_version != SCHEMA_VERSION {
        codes.push(format!(
            "unsupported_schema_version:{}",
            provider.schema_version
        ));
    }
    if provider.prompt_version != PROMPT_VERSION {
        codes.push(format!(
            "unsupported_prompt_version:{}",
            provider.prompt_version
        ));
    }
    if original_goal.is_empty() || original_goal.len() > MAX_GOAL_BYTES {
        codes.push("original_goal_size_invalid".to_string());
    }
    if provider.goal.is_empty() || provider.goal.len() > MAX_GOAL_BYTES {
        codes.push("provider_goal_size_invalid".to_string());
    }
    if provider.intent != expected_intent {
        codes.push("intent_mismatch".to_string());
    }
    if provider.profile.is_empty() || provider.profile.len() > MAX_ID_BYTES {
        codes.push("profile_invalid".to_string());
    }
    for (field, value) in [
        ("provider", provider.generation.provider.as_str()),
        ("model", provider.generation.model.as_str()),
        ("request_id", provider.generation.request_id.as_str()),
    ] {
        if value.is_empty() || value.len() > MAX_STATEMENT_BYTES {
            codes.push(format!("generation_{field}_invalid"));
        }
    }
    if provider.claims.is_empty() || provider.claims.len() > MAX_CLAIMS {
        codes.push("claim_count_invalid".to_string());
    }
    if provider.oracles.is_empty() || provider.oracles.len() > MAX_ORACLES {
        codes.push("oracle_count_invalid".to_string());
    }

    let claim_counts = counts(provider.claims.iter().map(|claim| claim.id.as_str()));
    let oracle_counts = counts(provider.oracles.iter().map(|oracle| oracle.id.as_str()));
    for claim in &provider.claims {
        validate_id("claim", &claim.id, &mut codes);
        if claim.normalized_requirement.is_empty()
            || claim.normalized_requirement.len() > MAX_STATEMENT_BYTES
        {
            codes.push(format!("claim_statement_size_invalid:{}", claim.id));
        }
        validate_claim_origin(original_goal, expected_intent, claim, &mut codes);
        if claim.oracle_ids.is_empty() {
            codes.push(format!("claim_unbound:{}", claim.id));
        }
        if claim.oracle_ids.len() > MAX_ORACLES_PER_CLAIM {
            codes.push(format!("claim_binding_count_invalid:{}", claim.id));
        }
        let local = counts(claim.oracle_ids.iter().map(String::as_str));
        for (oracle_id, count) in local {
            if count > 1 {
                codes.push(format!("claim_binding_duplicate:{}:{oracle_id}", claim.id));
            }
            if !oracle_counts.contains_key(oracle_id) {
                codes.push(format!("oracle_reference_missing:{}:{oracle_id}", claim.id));
            }
        }
    }
    for (id, count) in &claim_counts {
        if *count > 1 {
            codes.push(format!("claim_id_duplicate:{id}"));
        }
    }

    let referenced: BTreeSet<&str> = provider
        .claims
        .iter()
        .flat_map(|claim| claim.oracle_ids.iter().map(String::as_str))
        .collect();
    for oracle in &provider.oracles {
        validate_id("oracle", &oracle.id, &mut codes);
        if !referenced.contains(oracle.id.as_str()) {
            codes.push(format!("oracle_orphan:{}", oracle.id));
        }
        if !claim_counts.contains_key(oracle.claim_id.as_str()) {
            codes.push(format!(
                "oracle_claim_reference_missing:{}:{}",
                oracle.id, oracle.claim_id
            ));
        } else if !provider.claims.iter().any(|claim| {
            claim.id == oracle.claim_id && claim.oracle_ids.iter().any(|id| id == &oracle.id)
        }) {
            codes.push(format!(
                "oracle_claim_binding_unmatched:{}:{}",
                oracle.id, oracle.claim_id
            ));
        }
        validate_binding(oracle, &mut codes);
    }
    for (id, count) in oracle_counts {
        if count > 1 {
            codes.push(format!("oracle_id_duplicate:{id}"));
        }
    }
    codes
}

fn validate_binding(oracle: &Oracle, codes: &mut Vec<String>) {
    validate_path(&oracle.id, &oracle.setup.cwd, codes);
    if oracle.setup.argv.len() > MAX_ARGV {
        codes.push(format!("oracle_argv_count_invalid:{}", oracle.id));
    }
    if matches!(
        oracle.strategy,
        OracleStrategy::Command
            | OracleStrategy::ExitCode
            | OracleStrategy::Stdout
            | OracleStrategy::Stderr
    ) && oracle.setup.argv.is_empty()
    {
        codes.push(format!("oracle_argv_count_invalid:{}", oracle.id));
    }
    for (index, arg) in oracle.setup.argv.iter().enumerate() {
        if arg.is_empty() || arg.len() > MAX_ARG_BYTES || arg.chars().any(char::is_control) {
            codes.push(format!("oracle_argv_unsafe:{}:{index}", oracle.id));
        }
    }
    if oracle.setup.fixture_paths.len() > MAX_FIXTURES {
        codes.push(format!("oracle_fixture_count_invalid:{}", oracle.id));
    }
    for path in &oracle.setup.fixture_paths {
        validate_path(&oracle.id, path, codes);
    }
    match &oracle.input {
        OracleInput::Fixture { path, sha256 } => {
            validate_path(&oracle.id, path, codes);
            validate_hash(&oracle.id, "fixture", sha256, codes);
        }
        OracleInput::Http { method, path, .. } => {
            validate_http_path(&oracle.id, path, codes);
            if !matches!(method.as_str(), "GET" | "HEAD") {
                codes.push(format!("oracle_http_method_unsafe:{}", oracle.id));
            }
        }
        OracleInput::Dom { route, selector } => {
            validate_http_path(&oracle.id, route, codes);
            if selector.is_empty() || selector.len() > MAX_STATEMENT_BYTES {
                codes.push(format!("oracle_selector_invalid:{}", oracle.id));
            }
        }
        OracleInput::Text { value } if value.len() > MAX_STATEMENT_BYTES => {
            codes.push(format!("oracle_input_size_invalid:{}", oracle.id));
        }
        _ => {}
    }
    match &oracle.observation {
        OracleObservation::File { path, .. }
        | OracleObservation::ExistingBinding {
            artifact_path: path,
        } => validate_path(&oracle.id, path, codes),
        OracleObservation::Stdout { expected }
        | OracleObservation::Stderr { expected }
        | OracleObservation::Dom { expected }
        | OracleObservation::Interaction { expected }
            if expected.is_empty() || expected.len() > MAX_STATEMENT_BYTES =>
        {
            codes.push(format!("oracle_expected_size_invalid:{}", oracle.id));
        }
        _ => {}
    }
    if oracle.timeout_ms == 0 || oracle.timeout_ms > 300_000 {
        codes.push(format!("oracle_timeout_invalid:{}", oracle.id));
    }
    if oracle.lifecycle == OracleLifecycle::Executed && oracle.observed_strength.is_none() {
        codes.push(format!("oracle_observed_strength_missing:{}", oracle.id));
    }
    if oracle.result == OracleResult::Pass
        && oracle
            .observed_strength
            .is_none_or(|observed| strength_rank(observed) < strength_rank(oracle.minimum_strength))
    {
        codes.push(format!("oracle_pass_under_strength:{}", oracle.id));
    }
    validate_hash(
        &oracle.id,
        "proposed_binding",
        &oracle.lineage.proposed_binding_sha256,
        codes,
    );
    validate_hash(
        &oracle.id,
        "concretized_binding",
        &oracle.lineage.concretized_binding_sha256,
        codes,
    );
    if !oracle.lineage.semantic_equivalence
        && oracle.lineage.proposed_binding_sha256 != oracle.lineage.concretized_binding_sha256
    {
        codes.push(format!("binding_semantics_changed:{}", oracle.id));
    }
}

fn strength_rank(strength: EvidenceStrength) -> u8 {
    match strength {
        EvidenceStrength::Weak => 0,
        EvidenceStrength::Deterministic => 1,
        EvidenceStrength::Runtime => 2,
    }
}

fn validate_claim_origin(
    original_goal: &str,
    intent: VerificationIntent,
    claim: &AcceptanceClaim,
    codes: &mut Vec<String>,
) {
    match (&claim.origin, intent) {
        (
            ClaimOrigin::Goal {
                start_byte,
                end_byte,
            },
            VerificationIntent::Create,
        ) => {
            if start_byte >= end_byte
                || *end_byte > original_goal.len()
                || !original_goal.is_char_boundary(*start_byte)
                || !original_goal.is_char_boundary(*end_byte)
            {
                codes.push(format!("claim_goal_range_invalid:{}", claim.id));
            }
        }
        (
            ClaimOrigin::FixRequirement {
                artifact_path,
                requirement_id,
                stage,
                lineage,
                epoch,
                ..
            },
            VerificationIntent::Fix,
        ) => {
            validate_path(&claim.id, artifact_path, codes);
            if !matches!(
                requirement_id.as_str(),
                "before_fails" | "after_passes" | "no_regression"
            ) || !matches!(stage.as_str(), "before" | "after")
                || lineage.is_empty()
                || *epoch == 0
            {
                codes.push(format!("fix_requirement_reference_invalid:{}", claim.id));
            }
        }
        (
            ClaimOrigin::InvestigationRequirement {
                artifact_path,
                requirement_id,
                binding_id,
                stage,
                lineage,
                epoch,
            },
            VerificationIntent::Investigate,
        ) => {
            validate_path(&claim.id, artifact_path, codes);
            if !matches!(
                requirement_id.as_str(),
                "reproducer_fails" | "diagnosis_bound"
            ) || binding_id.is_empty()
                || !matches!(stage.as_str(), "reproduce" | "diagnosis")
                || lineage.is_empty()
                || *epoch == 0
            {
                codes.push(format!(
                    "investigation_requirement_reference_invalid:{}",
                    claim.id
                ));
            }
        }
        _ => codes.push(format!("claim_origin_intent_mismatch:{}", claim.id)),
    }
}

fn validate_http_path(oracle_id: &str, path: &str, codes: &mut Vec<String>) {
    if !path.starts_with('/')
        || path.starts_with("//")
        || path.contains("..")
        || path.chars().any(char::is_control)
        || path.len() > MAX_PATH_BYTES
    {
        codes.push(format!("oracle_http_path_unsafe:{oracle_id}"));
    }
}

fn validate_hash(oracle_id: &str, field: &str, hash: &str, codes: &mut Vec<String>) {
    if hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        codes.push(format!("oracle_{field}_hash_invalid:{oracle_id}"));
    }
}

fn validate_path(oracle_id: &str, path: &str, codes: &mut Vec<String>) {
    if path.len() > MAX_PATH_BYTES || validate_workspace_relative(path).is_err() {
        codes.push(format!("oracle_path_unsafe:{oracle_id}"));
    }
}

fn validate_id(kind: &str, id: &str, codes: &mut Vec<String>) {
    let valid = !id.is_empty()
        && id.len() <= MAX_ID_BYTES
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'));
    if !valid {
        codes.push(format!("{kind}_id_invalid:{id}"));
    }
}

fn counts<'a>(values: impl Iterator<Item = &'a str>) -> BTreeMap<&'a str, usize> {
    let mut counts = BTreeMap::new();
    for value in values {
        *counts.entry(value).or_insert(0) += 1;
    }
    counts
}

fn error(code: impl Into<String>) -> ValidationError {
    ValidationError {
        codes: vec![code.into()],
    }
}

fn sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn sha256_is_stable() {
        assert_eq!(
            super::sha256(b"goal"),
            "0304523efff53f243d76dc81b7c271f922292543cead846ee714f066c3331e5f"
        );
    }

    #[test]
    fn unsafe_http_routes_are_deterministic() {
        let mut codes = Vec::new();
        super::validate_http_path("http", "//host/../escape", &mut codes);
        assert_eq!(codes, ["oracle_http_path_unsafe:http"]);
    }
}
