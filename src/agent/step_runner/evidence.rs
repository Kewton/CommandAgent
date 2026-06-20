use crate::agent::step_runner::correction_evidence::ContractEvidence;
use serde::{Deserialize, Serialize};

pub const EVIDENCE_SCHEMA_VERSION: &str = "1.0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceProducer {
    PlanLint,
    ProviderParser,
    ToolSchemaGuard,
    StepToolPolicy,
    Verifier,
    ProfileVerification,
    SetupRuntime,
    RecoveryLoop,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureClass {
    Planning,
    ProviderTransport,
    ToolProtocol,
    StepPolicy,
    Verification,
    Profile,
    Setup,
    RecoveryAttempt,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceEnvelope {
    pub schema_version: String,
    pub evidence_id: String,
    pub producer: EvidenceProducer,
    pub failure_class: FailureClass,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failed_step: Option<String>,
    pub reason_code: String,
    pub timestamp: String,
    pub payload: EvidencePayload,
}

impl EvidenceEnvelope {
    pub fn from_contract_evidence(
        evidence_id: impl Into<String>,
        timestamp: impl Into<String>,
        evidence: &ContractEvidence,
    ) -> Self {
        let producer = producer_from_guard(&evidence.guard);
        let payload = EvidencePayload::from_contract_evidence(evidence);
        Self {
            schema_version: EVIDENCE_SCHEMA_VERSION.to_string(),
            evidence_id: evidence_id.into(),
            producer,
            failure_class: payload.failure_class(),
            failed_step: evidence.failed_step.clone(),
            reason_code: evidence
                .reason_code
                .clone()
                .or_else(|| evidence.violated_contract.clone())
                .unwrap_or_else(|| evidence.guard.clone()),
            timestamp: timestamp.into(),
            payload,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EvidencePayload {
    Planning(PlanningEvidence),
    ProviderTransport(ProviderTransportEvidence),
    ToolProtocol(ToolProtocolEvidence),
    StepPolicy(StepPolicyEvidence),
    Verification(VerificationEvidence),
    Profile(ProfileEvidence),
    Setup(SetupEvidence),
    RecoveryAttempt(RecoveryAttemptEvidence),
    Unsupported(UnsupportedEvidence),
}

impl EvidencePayload {
    pub fn failure_class(&self) -> FailureClass {
        match self {
            Self::Planning(_) => FailureClass::Planning,
            Self::ProviderTransport(_) => FailureClass::ProviderTransport,
            Self::ToolProtocol(_) => FailureClass::ToolProtocol,
            Self::StepPolicy(_) => FailureClass::StepPolicy,
            Self::Verification(_) => FailureClass::Verification,
            Self::Profile(_) => FailureClass::Profile,
            Self::Setup(_) => FailureClass::Setup,
            Self::RecoveryAttempt(_) => FailureClass::RecoveryAttempt,
            Self::Unsupported(_) => FailureClass::Unsupported,
        }
    }

    pub fn from_contract_evidence(evidence: &ContractEvidence) -> Self {
        let guard = evidence.guard.as_str();
        if guard.starts_with("plan_lint") || guard.contains("planning") {
            return Self::Planning(PlanningEvidence {
                violated_contract: evidence.violated_contract.clone(),
                target_field: evidence.target_field.clone(),
                target_path: evidence.target_path.clone(),
                rejected_value: evidence.rejected_value.clone(),
                required_values: merge_lists(&evidence.required_literals, &evidence.required_paths),
                missing_values: merge_lists(&evidence.missing_literals, &evidence.missing_paths),
            });
        }
        if guard == "tool_protocol"
            || evidence
                .reason_code
                .as_deref()
                .is_some_and(|code| code.starts_with("tool_args_"))
        {
            return Self::ToolProtocol(ToolProtocolEvidence {
                tool: evidence.tool.clone(),
                error_kind: evidence
                    .reason_code
                    .clone()
                    .or_else(|| evidence.diagnostic_code.clone()),
                required_fields: evidence.required_fields.clone(),
                missing_fields: evidence
                    .target_field
                    .iter()
                    .cloned()
                    .chain(evidence.missing_literals.iter().cloned())
                    .collect(),
                invalid_payload_excerpt: evidence.diagnostic.clone(),
            });
        }
        if guard == "step_policy" {
            return Self::StepPolicy(StepPolicyEvidence {
                step_id: evidence.failed_step.clone(),
                policy: evidence.violated_contract.clone(),
                disallowed_tool: evidence.tool.clone(),
                violation: evidence
                    .reason_code
                    .clone()
                    .or_else(|| evidence.diagnostic.clone()),
            });
        }
        if guard == "verifier" || evidence.command.is_some() {
            return Self::Verification(VerificationEvidence {
                command: evidence.command.clone(),
                failure_kind: evidence.failure_kind.clone(),
                failure_signature: evidence.failure_signature.clone(),
                diagnostic_excerpt: evidence.diagnostic.clone(),
                related_source_excerpt: evidence.related_source_excerpt.clone(),
                candidate_artifacts: evidence.candidate_artifacts.clone(),
            });
        }
        if guard == "profile" || guard.starts_with("profile_") {
            return Self::Profile(ProfileEvidence {
                profile: evidence
                    .affected_cases
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "unknown".to_string()),
                check_code: evidence
                    .diagnostic_code
                    .clone()
                    .or_else(|| evidence.reason_code.clone()),
                affected_paths: merge_lists(&evidence.required_paths, &evidence.missing_paths),
                observed: evidence.observed_expected_pairs.clone(),
                expected: evidence.required_literals.clone(),
            });
        }
        if guard == "setup"
            || evidence
                .reason_code
                .as_deref()
                .is_some_and(|code| code.contains("dependency"))
        {
            return Self::Setup(SetupEvidence {
                setup_command: evidence.command.clone(),
                setup_status: evidence.reason_code.clone(),
                missing_dependency: evidence.missing_literals.first().cloned(),
                affected_manifest: evidence
                    .repair_target
                    .clone()
                    .or_else(|| evidence.target_path.clone()),
            });
        }
        if guard == "recovery" || guard == "repair" {
            return Self::RecoveryAttempt(RecoveryAttemptEvidence {
                attempt_id: evidence.prior_attempts.first().cloned(),
                recovery_task_id: evidence.repair_kind.clone(),
                observed_result: evidence.reason_code.clone(),
                rerun_verifier: evidence.rerun_authority.first().cloned(),
                final_status: evidence.failure_kind.clone(),
            });
        }
        if guard.starts_with("provider") {
            return Self::ProviderTransport(ProviderTransportEvidence {
                provider: None,
                model: None,
                parse_error: evidence
                    .reason_code
                    .clone()
                    .or_else(|| evidence.diagnostic.clone()),
                raw_response_excerpt: evidence.related_source_excerpt.clone(),
            });
        }
        Self::Unsupported(UnsupportedEvidence {
            original_guard: evidence.guard.clone(),
            diagnostic: evidence.diagnostic.clone(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanningEvidence {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub violated_contract: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_field: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rejected_value: Option<String>,
    #[serde(default)]
    pub required_values: Vec<String>,
    #[serde(default)]
    pub missing_values: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderTransportEvidence {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parse_error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_response_excerpt: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolProtocolEvidence {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_kind: Option<String>,
    #[serde(default)]
    pub required_fields: Vec<String>,
    #[serde(default)]
    pub missing_fields: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invalid_payload_excerpt: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepPolicyEvidence {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disallowed_tool: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub violation: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationEvidence {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_signature: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagnostic_excerpt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub related_source_excerpt: Option<String>,
    #[serde(default)]
    pub candidate_artifacts: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileEvidence {
    pub profile: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check_code: Option<String>,
    #[serde(default)]
    pub affected_paths: Vec<String>,
    #[serde(default)]
    pub observed: Vec<String>,
    #[serde(default)]
    pub expected: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetupEvidence {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setup_command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setup_status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub missing_dependency: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub affected_manifest: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryAttemptEvidence {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attempt_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recovery_task_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_result: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rerun_verifier: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub final_status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnsupportedEvidence {
    pub original_guard: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagnostic: Option<String>,
}

fn producer_from_guard(guard: &str) -> EvidenceProducer {
    if guard.starts_with("plan_lint") {
        EvidenceProducer::PlanLint
    } else if guard.starts_with("provider") {
        EvidenceProducer::ProviderParser
    } else if guard == "tool_protocol" {
        EvidenceProducer::ToolSchemaGuard
    } else if guard == "step_policy" {
        EvidenceProducer::StepToolPolicy
    } else if guard == "verifier" {
        EvidenceProducer::Verifier
    } else if guard == "profile" || guard.starts_with("profile_") {
        EvidenceProducer::ProfileVerification
    } else if guard == "setup" {
        EvidenceProducer::SetupRuntime
    } else if guard == "recovery" || guard == "repair" {
        EvidenceProducer::RecoveryLoop
    } else {
        EvidenceProducer::Unknown
    }
}

fn merge_lists(left: &[String], right: &[String]) -> Vec<String> {
    let mut merged = Vec::new();
    for value in left.iter().chain(right.iter()) {
        if !merged.contains(value) {
            merged.push(value.clone());
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_lint_contract_evidence_maps_to_planning_payload() {
        let evidence = ContractEvidence::new("plan_lint.profile_obligations")
            .with_failed_step("create-package-json")
            .with_violated_contract("nextjs_dependencies_required")
            .with_target_field("instruction")
            .with_required_literals(["next", "react", "react-dom"])
            .with_missing_literals(["react-dom"]);

        let envelope =
            EvidenceEnvelope::from_contract_evidence("ev1", "2026-06-20T00:00:00Z", &evidence);

        assert_eq!(envelope.producer, EvidenceProducer::PlanLint);
        assert_eq!(envelope.failure_class, FailureClass::Planning);
        assert_eq!(envelope.failed_step.as_deref(), Some("create-package-json"));
        match envelope.payload {
            EvidencePayload::Planning(payload) => {
                assert_eq!(payload.target_field.as_deref(), Some("instruction"));
                assert_eq!(payload.missing_values, vec!["react-dom"]);
            }
            other => panic!("unexpected payload: {other:?}"),
        }
    }

    #[test]
    fn tool_protocol_contract_evidence_maps_to_tool_payload() {
        let evidence = ContractEvidence::new("tool_protocol")
            .with_reason_code("tool_args_missing_required_field")
            .with_tool("Write")
            .with_target_field("path")
            .with_required_fields(["path", "content"]);

        let envelope = EvidenceEnvelope::from_contract_evidence("ev2", "ts", &evidence);

        assert_eq!(envelope.failure_class, FailureClass::ToolProtocol);
        match envelope.payload {
            EvidencePayload::ToolProtocol(payload) => {
                assert_eq!(payload.tool.as_deref(), Some("Write"));
                assert_eq!(payload.missing_fields, vec!["path"]);
            }
            other => panic!("unexpected payload: {other:?}"),
        }
    }

    #[test]
    fn unknown_contract_evidence_stays_unsupported() {
        let evidence = ContractEvidence::new("custom_guard").with_diagnostic("custom failure");

        let envelope = EvidenceEnvelope::from_contract_evidence("ev3", "ts", &evidence);

        assert_eq!(envelope.failure_class, FailureClass::Unsupported);
        assert!(matches!(envelope.payload, EvidencePayload::Unsupported(_)));
    }
}
