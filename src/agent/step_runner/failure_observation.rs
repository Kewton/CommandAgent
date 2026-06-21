use crate::agent::step_runner::correction_evidence::ContractEvidence;
use serde::{Deserialize, Serialize};

pub(crate) const FAILURE_OBSERVATION_SCHEMA_VERSION: &str = "1.0";

const MAX_FIELD_CHARS: usize = 240;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TerminalState {
    Ok,
    PlanParseFailed,
    PlanSchemaFailed,
    PlanLintFailed,
    ProviderTransportFailed,
    ProviderParseFailed,
    ToolProtocolFailed,
    StepPolicyFailed,
    ProfileContractFailed,
    VerifierCommandFailed,
    DependencyMissing,
    SetupFailed,
    PortInUse,
    MissingDeliverable,
    MissingEvidence,
    EvidenceBindingFailed,
    CompletionEvidenceFailed,
    EvalAssertionFailed,
    RepairExhausted,
    ExplicitStop,
    Unknown,
}

impl TerminalState {
    #[cfg(test)]
    pub(crate) const ALL: &'static [Self] = &[
        Self::Ok,
        Self::PlanParseFailed,
        Self::PlanSchemaFailed,
        Self::PlanLintFailed,
        Self::ProviderTransportFailed,
        Self::ProviderParseFailed,
        Self::ToolProtocolFailed,
        Self::StepPolicyFailed,
        Self::ProfileContractFailed,
        Self::VerifierCommandFailed,
        Self::DependencyMissing,
        Self::SetupFailed,
        Self::PortInUse,
        Self::MissingDeliverable,
        Self::MissingEvidence,
        Self::EvidenceBindingFailed,
        Self::CompletionEvidenceFailed,
        Self::EvalAssertionFailed,
        Self::RepairExhausted,
        Self::ExplicitStop,
        Self::Unknown,
    ];

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::PlanParseFailed => "plan_parse_failed",
            Self::PlanSchemaFailed => "plan_schema_failed",
            Self::PlanLintFailed => "plan_lint_failed",
            Self::ProviderTransportFailed => "provider_transport_failed",
            Self::ProviderParseFailed => "provider_parse_failed",
            Self::ToolProtocolFailed => "tool_protocol_failed",
            Self::StepPolicyFailed => "step_policy_failed",
            Self::ProfileContractFailed => "profile_contract_failed",
            Self::VerifierCommandFailed => "verifier_command_failed",
            Self::DependencyMissing => "dependency_missing",
            Self::SetupFailed => "setup_failed",
            Self::PortInUse => "port_in_use",
            Self::MissingDeliverable => "missing_deliverable",
            Self::MissingEvidence => "missing_evidence",
            Self::EvidenceBindingFailed => "evidence_binding_failed",
            Self::CompletionEvidenceFailed => "completion_evidence_failed",
            Self::EvalAssertionFailed => "eval_assertion_failed",
            Self::RepairExhausted => "repair_exhausted",
            Self::ExplicitStop => "explicit_stop",
            Self::Unknown => "unknown",
        }
    }

    pub(crate) fn failure_class(self) -> FailureObservationClass {
        match self {
            Self::Ok => FailureObservationClass::Ok,
            Self::PlanParseFailed
            | Self::PlanSchemaFailed
            | Self::PlanLintFailed
            | Self::MissingDeliverable
            | Self::RepairExhausted => FailureObservationClass::Planning,
            Self::ProviderTransportFailed | Self::ProviderParseFailed => {
                FailureObservationClass::ProviderTransport
            }
            Self::ToolProtocolFailed => FailureObservationClass::ToolProtocol,
            Self::StepPolicyFailed => FailureObservationClass::StepPolicy,
            Self::ProfileContractFailed => FailureObservationClass::Profile,
            Self::VerifierCommandFailed => FailureObservationClass::Verifier,
            Self::DependencyMissing | Self::SetupFailed | Self::PortInUse => {
                FailureObservationClass::Setup
            }
            Self::MissingEvidence
            | Self::EvidenceBindingFailed
            | Self::CompletionEvidenceFailed
            | Self::EvalAssertionFailed => FailureObservationClass::Quality,
            Self::ExplicitStop | Self::Unknown => FailureObservationClass::Unknown,
        }
    }

    pub(crate) fn contract_layer(self) -> ContractLayer {
        match self {
            Self::Ok => ContractLayer::Ok,
            Self::PlanParseFailed
            | Self::PlanSchemaFailed
            | Self::PlanLintFailed
            | Self::MissingDeliverable
            | Self::RepairExhausted => ContractLayer::PlanningContract,
            Self::ProviderTransportFailed
            | Self::ProviderParseFailed
            | Self::ToolProtocolFailed
            | Self::StepPolicyFailed => ContractLayer::ExecutionContract,
            Self::ProfileContractFailed => ContractLayer::ProfileContract,
            Self::VerifierCommandFailed => ContractLayer::VerificationContract,
            Self::DependencyMissing | Self::SetupFailed => ContractLayer::SetupBootstrapContract,
            Self::PortInUse => ContractLayer::DevServerPortContract,
            Self::MissingEvidence
            | Self::EvidenceBindingFailed
            | Self::CompletionEvidenceFailed
            | Self::EvalAssertionFailed => ContractLayer::EvalSuccessContract,
            Self::ExplicitStop | Self::Unknown => ContractLayer::UnknownContract,
        }
    }

    pub(crate) fn default_violated_contract(self) -> &'static str {
        match self {
            Self::Ok => "none",
            Self::PlanParseFailed => "plan_file_parse_contract",
            Self::PlanSchemaFailed => "plan_file_schema_contract",
            Self::PlanLintFailed => "planning_contract",
            Self::ProviderTransportFailed => "provider_transport_contract",
            Self::ProviderParseFailed => "provider_tool_call_parse_contract",
            Self::ToolProtocolFailed => "tool_protocol_contract",
            Self::StepPolicyFailed => "step_execution_policy_contract",
            Self::ProfileContractFailed => "profile_contract",
            Self::VerifierCommandFailed => "verification_contract",
            Self::DependencyMissing | Self::SetupFailed => "setup_bootstrap_contract",
            Self::PortInUse => "dev_server_port_contract",
            Self::MissingDeliverable => "eval_success_contract",
            Self::MissingEvidence => "evidence_contract",
            Self::EvidenceBindingFailed => "evidence_binding_contract",
            Self::CompletionEvidenceFailed => "completion_evidence_contract",
            Self::EvalAssertionFailed => "eval_success_contract",
            Self::RepairExhausted => "bounded_repair_contract",
            Self::ExplicitStop => "explicit_stop_contract",
            Self::Unknown => "unknown_contract",
        }
    }

    pub(crate) fn default_source(self) -> ObservationSource {
        match self {
            Self::Ok => ObservationSource::ProcessResult,
            Self::PlanParseFailed | Self::PlanSchemaFailed | Self::PlanLintFailed => {
                ObservationSource::PlanContract
            }
            Self::ProviderTransportFailed | Self::ProviderParseFailed => {
                ObservationSource::ProviderResponse
            }
            Self::ToolProtocolFailed | Self::StepPolicyFailed => ObservationSource::ExecutionGuard,
            Self::ProfileContractFailed => ObservationSource::ProfileVerifier,
            Self::DependencyMissing | Self::SetupFailed | Self::PortInUse => {
                ObservationSource::EnvironmentVerifier
            }
            Self::MissingDeliverable
            | Self::MissingEvidence
            | Self::EvidenceBindingFailed
            | Self::CompletionEvidenceFailed
            | Self::EvalAssertionFailed => ObservationSource::EvalSuccessCheck,
            Self::RepairExhausted | Self::ExplicitStop => ObservationSource::BoundedRepair,
            Self::VerifierCommandFailed => ObservationSource::Verifier,
            Self::Unknown => ObservationSource::Unknown,
        }
    }

    pub(crate) fn default_source_of_truth(self) -> SourceOfTruth {
        match self {
            Self::Ok => SourceOfTruth::ProcessExitAndEvalChecks,
            Self::DependencyMissing | Self::SetupFailed | Self::PortInUse => {
                SourceOfTruth::VerifierOutput
            }
            Self::MissingDeliverable
            | Self::MissingEvidence
            | Self::EvidenceBindingFailed
            | Self::CompletionEvidenceFailed
            | Self::EvalAssertionFailed => SourceOfTruth::EvalSuccessContract,
            Self::Unknown => SourceOfTruth::Unknown,
            _ => SourceOfTruth::RuntimeEvidence,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FailureObservationClass {
    Ok,
    Planning,
    ProviderTransport,
    ToolProtocol,
    StepPolicy,
    Profile,
    Verifier,
    Setup,
    Quality,
    Unknown,
}

impl FailureObservationClass {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Planning => "planning",
            Self::ProviderTransport => "provider_transport",
            Self::ToolProtocol => "tool_protocol",
            Self::StepPolicy => "step_policy",
            Self::Profile => "profile",
            Self::Verifier => "verifier",
            Self::Setup => "setup",
            Self::Quality => "quality",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ContractLayer {
    Ok,
    PlanningContract,
    ExecutionContract,
    ProfileContract,
    SetupBootstrapContract,
    DevServerPortContract,
    VerificationContract,
    EvalSuccessContract,
    UnknownContract,
}

impl ContractLayer {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::PlanningContract => "planning_contract",
            Self::ExecutionContract => "execution_contract",
            Self::ProfileContract => "profile_contract",
            Self::SetupBootstrapContract => "setup_bootstrap_contract",
            Self::DevServerPortContract => "dev_server_port_contract",
            Self::VerificationContract => "verification_contract",
            Self::EvalSuccessContract => "eval_success_contract",
            Self::UnknownContract => "unknown_contract",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ObservationSource {
    ProcessResult,
    PlanContract,
    ProviderResponse,
    ExecutionGuard,
    ProfileVerifier,
    EnvironmentVerifier,
    EvalSuccessCheck,
    BoundedRepair,
    Verifier,
    Unknown,
}

impl ObservationSource {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::ProcessResult => "process_result",
            Self::PlanContract => "plan_contract",
            Self::ProviderResponse => "provider_response",
            Self::ExecutionGuard => "execution_guard",
            Self::ProfileVerifier => "profile_verifier",
            Self::EnvironmentVerifier => "environment_verifier",
            Self::EvalSuccessCheck => "eval_success_check",
            Self::BoundedRepair => "bounded_repair",
            Self::Verifier => "verifier",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SourceOfTruth {
    ProcessExitAndEvalChecks,
    RuntimeEvidence,
    VerifierOutput,
    EvalSuccessContract,
    Unknown,
}

impl SourceOfTruth {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::ProcessExitAndEvalChecks => "process_exit_and_eval_checks",
            Self::RuntimeEvidence => "runtime_evidence",
            Self::VerifierOutput => "verifier_output",
            Self::EvalSuccessContract => "eval_success_contract",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FailureProducer {
    ProcessResult,
    PlanParser,
    PlanSchema,
    PlanLint,
    ProviderTransport,
    ProviderParser,
    ToolProtocol,
    StepPolicy,
    ProfileVerification,
    Verifier,
    SetupRuntime,
    DevServer,
    CompletionEvidence,
    EvidenceBinding,
    EvalSuccess,
    RecoveryLoop,
    Unknown,
}

impl FailureProducer {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::ProcessResult => "process_result",
            Self::PlanParser => "plan_parser",
            Self::PlanSchema => "plan_schema",
            Self::PlanLint => "plan_lint",
            Self::ProviderTransport => "provider_transport",
            Self::ProviderParser => "provider_parser",
            Self::ToolProtocol => "tool_protocol",
            Self::StepPolicy => "step_policy",
            Self::ProfileVerification => "profile_verification",
            Self::Verifier => "verifier",
            Self::SetupRuntime => "setup_runtime",
            Self::DevServer => "dev_server",
            Self::CompletionEvidence => "completion_evidence",
            Self::EvidenceBinding => "evidence_binding",
            Self::EvalSuccess => "eval_success",
            Self::RecoveryLoop => "recovery_loop",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FailureActionability {
    NotApplicable,
    Actionable,
    ExplicitStop,
    Unknown,
}

impl FailureActionability {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::NotApplicable => "not_applicable",
            Self::Actionable => "actionable",
            Self::ExplicitStop => "explicit_stop",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct FailureObservation {
    pub(crate) schema_version: String,
    pub(crate) terminal_state: TerminalState,
    pub(crate) failure_class: FailureObservationClass,
    pub(crate) contract_layer: ContractLayer,
    pub(crate) violated_contract: String,
    pub(crate) source: String,
    pub(crate) source_of_truth: String,
    pub(crate) producer: FailureProducer,
    pub(crate) guard: String,
    pub(crate) diagnostic_code: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) failure_signature: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) tool: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) target_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) target_role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) failed_step: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) setup_state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) port: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) evidence_runner_status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) artifact_ledger_status: Option<String>,
    pub(crate) actionability: FailureActionability,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) explicit_stop_reason: Option<String>,
}

impl FailureObservation {
    pub(crate) fn from_contract_evidence(evidence: &ContractEvidence) -> Self {
        let terminal_state = terminal_state_from_contract_evidence(evidence);
        let producer = producer_from_contract_evidence(evidence, terminal_state);
        let diagnostic_code = evidence
            .diagnostic_code
            .as_deref()
            .or(evidence.reason_code.as_deref())
            .or(evidence.violated_contract.as_deref())
            .unwrap_or_else(|| terminal_state.as_str());
        let setup_state = setup_state_from_terminal(terminal_state, evidence);
        let port = if terminal_state == TerminalState::PortInUse {
            port_from_texts([
                evidence.diagnostic.as_deref(),
                evidence.command.as_deref(),
                evidence.target_path.as_deref(),
            ])
        } else {
            None
        };
        Self {
            schema_version: FAILURE_OBSERVATION_SCHEMA_VERSION.to_string(),
            terminal_state,
            failure_class: terminal_state.failure_class(),
            contract_layer: terminal_state.contract_layer(),
            violated_contract: evidence
                .violated_contract
                .clone()
                .unwrap_or_else(|| terminal_state.default_violated_contract().to_string()),
            source: terminal_state.default_source().as_str().to_string(),
            source_of_truth: evidence.source_of_truth.clone().unwrap_or_else(|| {
                terminal_state
                    .default_source_of_truth()
                    .as_str()
                    .to_string()
            }),
            producer,
            guard: if evidence.guard.trim().is_empty() {
                "unknown".to_string()
            } else {
                evidence.guard.clone()
            },
            diagnostic_code: bounded(diagnostic_code),
            failure_signature: evidence.failure_signature.as_deref().map(bounded),
            command: evidence.command.as_deref().map(bounded),
            tool: evidence.tool.as_deref().map(bounded),
            target_path: evidence
                .repair_target
                .as_deref()
                .or(evidence.target_path.as_deref())
                .map(bounded),
            target_role: evidence.artifact_role.as_deref().map(bounded),
            failed_step: evidence.failed_step.as_deref().map(bounded),
            setup_state,
            port,
            evidence_runner_status: status_from_list(&evidence.completion_evidence, "status=")
                .or_else(|| status_from_list(&evidence.evidence_binding, "status=")),
            artifact_ledger_status: status_from_eval_fields(&evidence.eval_report_fields),
            actionability: actionability(terminal_state),
            explicit_stop_reason: evidence.explicit_stop_reason.as_deref().map(bounded),
        }
    }

    pub(crate) fn render_inline(&self) -> String {
        let mut fields = vec![
            format!("terminal_state={}", self.terminal_state.as_str()),
            format!("failure_class={}", self.failure_class.as_str()),
            format!("contract_layer={}", self.contract_layer.as_str()),
            format!("violated_contract={}", self.violated_contract),
            format!("source={}", self.source),
            format!("source_of_truth={}", self.source_of_truth),
            format!("producer={}", self.producer.as_str()),
            format!("guard={}", self.guard),
            format!("diagnostic_code={}", self.diagnostic_code),
            format!("actionability={}", self.actionability.as_str()),
        ];
        if let Some(signature) = &self.failure_signature {
            fields.push(format!("failure_signature={signature}"));
        }
        if let Some(reason) = &self.explicit_stop_reason {
            fields.push(format!("explicit_stop_reason={reason}"));
        }
        fields.join(" ")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg(test)]
pub(crate) struct TaxonomyRow {
    pub(crate) terminal_state: &'static str,
    pub(crate) failure_class: &'static str,
    pub(crate) contract_layer: &'static str,
    pub(crate) violated_contract: &'static str,
    pub(crate) source: &'static str,
    pub(crate) source_of_truth: &'static str,
}

#[cfg(test)]
pub(crate) fn taxonomy_rows() -> Vec<TaxonomyRow> {
    TerminalState::ALL
        .iter()
        .map(|state| TaxonomyRow {
            terminal_state: state.as_str(),
            failure_class: state.failure_class().as_str(),
            contract_layer: state.contract_layer().as_str(),
            violated_contract: state.default_violated_contract(),
            source: state.default_source().as_str(),
            source_of_truth: state.default_source_of_truth().as_str(),
        })
        .collect()
}

fn terminal_state_from_contract_evidence(evidence: &ContractEvidence) -> TerminalState {
    let guard = evidence.guard.as_str();
    let combined = [
        Some(guard),
        evidence.violated_contract.as_deref(),
        evidence.reason_code.as_deref(),
        evidence.failure_kind.as_deref(),
        evidence.diagnostic_code.as_deref(),
        evidence.diagnostic.as_deref(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join("\n")
    .to_ascii_lowercase();

    if guard.starts_with("plan_parse") || combined.contains("invalid plan yaml") {
        return TerminalState::PlanParseFailed;
    }
    if guard.starts_with("plan_schema") || combined.contains("plan_file_schema") {
        return TerminalState::PlanSchemaFailed;
    }
    if guard.starts_with("plan_lint") {
        return TerminalState::PlanLintFailed;
    }
    if guard.starts_with("provider") {
        if combined.contains("parse") || combined.contains("tool_call") {
            return TerminalState::ProviderParseFailed;
        }
        return TerminalState::ProviderTransportFailed;
    }
    if guard == "tool_protocol" {
        return TerminalState::ToolProtocolFailed;
    }
    if guard == "step_policy" {
        return TerminalState::StepPolicyFailed;
    }
    if guard == "profile" || guard.starts_with("profile_") {
        return TerminalState::ProfileContractFailed;
    }
    if guard == "setup" {
        if combined.contains("eaddrinuse") || combined.contains("port_in_use") {
            return TerminalState::PortInUse;
        }
        if combined.contains("dependency_missing") || combined.contains("dependency") {
            return TerminalState::DependencyMissing;
        }
        return TerminalState::SetupFailed;
    }
    if guard == "verifier" {
        if combined.contains("dependency_missing") || combined.contains("node_modules/.bin") {
            return TerminalState::DependencyMissing;
        }
        if combined.contains("eaddrinuse") || combined.contains("address already in use") {
            return TerminalState::PortInUse;
        }
        return TerminalState::VerifierCommandFailed;
    }
    if guard == "evidence_binding" {
        return TerminalState::EvidenceBindingFailed;
    }
    if guard == "completion_evidence" {
        if combined.contains("missing") {
            return TerminalState::MissingEvidence;
        }
        return TerminalState::CompletionEvidenceFailed;
    }
    if guard == "recovery" && combined.contains("missing_required_artifact") {
        return TerminalState::MissingDeliverable;
    }
    if guard == "repair" || guard == "recovery" {
        if evidence.explicit_stop_reason.is_some()
            || evidence.active_job.as_deref() == Some("explicit_stop")
        {
            return TerminalState::ExplicitStop;
        }
        if combined.contains("exhaust") || combined.contains("no_progress") {
            return TerminalState::RepairExhausted;
        }
    }
    TerminalState::Unknown
}

fn producer_from_contract_evidence(
    evidence: &ContractEvidence,
    terminal_state: TerminalState,
) -> FailureProducer {
    match terminal_state {
        TerminalState::Ok => FailureProducer::ProcessResult,
        TerminalState::PlanParseFailed => FailureProducer::PlanParser,
        TerminalState::PlanSchemaFailed => FailureProducer::PlanSchema,
        TerminalState::PlanLintFailed => FailureProducer::PlanLint,
        TerminalState::ProviderTransportFailed => FailureProducer::ProviderTransport,
        TerminalState::ProviderParseFailed => FailureProducer::ProviderParser,
        TerminalState::ToolProtocolFailed => FailureProducer::ToolProtocol,
        TerminalState::StepPolicyFailed => FailureProducer::StepPolicy,
        TerminalState::ProfileContractFailed => FailureProducer::ProfileVerification,
        TerminalState::VerifierCommandFailed => FailureProducer::Verifier,
        TerminalState::DependencyMissing | TerminalState::SetupFailed => {
            FailureProducer::SetupRuntime
        }
        TerminalState::PortInUse => FailureProducer::DevServer,
        TerminalState::MissingDeliverable | TerminalState::EvalAssertionFailed => {
            FailureProducer::EvalSuccess
        }
        TerminalState::MissingEvidence | TerminalState::CompletionEvidenceFailed => {
            FailureProducer::CompletionEvidence
        }
        TerminalState::EvidenceBindingFailed => FailureProducer::EvidenceBinding,
        TerminalState::RepairExhausted | TerminalState::ExplicitStop => {
            FailureProducer::RecoveryLoop
        }
        TerminalState::Unknown => {
            if evidence.guard.starts_with("provider") {
                FailureProducer::ProviderTransport
            } else {
                FailureProducer::Unknown
            }
        }
    }
}

fn actionability(terminal_state: TerminalState) -> FailureActionability {
    match terminal_state {
        TerminalState::Ok => FailureActionability::NotApplicable,
        TerminalState::ExplicitStop | TerminalState::RepairExhausted => {
            FailureActionability::ExplicitStop
        }
        TerminalState::Unknown => FailureActionability::Unknown,
        _ => FailureActionability::Actionable,
    }
}

fn setup_state_from_terminal(
    terminal_state: TerminalState,
    evidence: &ContractEvidence,
) -> Option<String> {
    match terminal_state {
        TerminalState::DependencyMissing => Some("dependency_missing".to_string()),
        TerminalState::SetupFailed => Some(
            evidence
                .failure_kind
                .as_deref()
                .or(evidence.reason_code.as_deref())
                .unwrap_or("setup_failed")
                .to_string(),
        ),
        TerminalState::PortInUse => Some("port_in_use".to_string()),
        _ => None,
    }
}

fn status_from_list(values: &[String], key: &str) -> Option<String> {
    values.iter().find_map(|value| {
        value
            .split_whitespace()
            .find_map(|part| part.strip_prefix(key).map(bounded))
    })
}

fn status_from_eval_fields(values: &[String]) -> Option<String> {
    values
        .iter()
        .find_map(|value| value.strip_prefix("artifact_ledger_status=").map(bounded))
}

fn port_from_texts<'a>(values: impl IntoIterator<Item = Option<&'a str>>) -> Option<String> {
    for value in values.into_iter().flatten() {
        for marker in [":::", "localhost:", "127.0.0.1:"] {
            if let Some(rest) = value.split(marker).nth(1) {
                let digits: String = rest.chars().take_while(|ch| ch.is_ascii_digit()).collect();
                if !digits.is_empty() {
                    return Some(digits);
                }
            }
        }
    }
    None
}

fn bounded(value: &str) -> String {
    let mut out = value.trim().replace('\n', " ");
    if out.len() > MAX_FIELD_CHARS {
        out.truncate(MAX_FIELD_CHARS);
        out.push_str("...");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn taxonomy_matches_shared_fixture() {
        let actual = taxonomy_rows()
            .into_iter()
            .map(|row| {
                format!(
                    "{}\t{}\t{}\t{}\t{}\t{}",
                    row.terminal_state,
                    row.failure_class,
                    row.contract_layer,
                    row.violated_contract,
                    row.source,
                    row.source_of_truth
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let expected = include_str!("../../../scripts/failure_observation_taxonomy.tsv")
            .lines()
            .skip(1)
            .collect::<Vec<_>>()
            .join("\n");

        assert_eq!(actual, expected);
    }

    #[test]
    fn plan_lint_evidence_maps_to_planning_observation() {
        let evidence = ContractEvidence::new("plan_lint.task_contract")
            .with_violated_contract("task_behavior_obligation_plan_projection")
            .with_reason_code("missing_owner_step")
            .with_failed_step("create-package-json");

        let observation = FailureObservation::from_contract_evidence(&evidence);

        assert_eq!(observation.terminal_state, TerminalState::PlanLintFailed);
        assert_eq!(observation.contract_layer, ContractLayer::PlanningContract);
        assert_eq!(observation.producer, FailureProducer::PlanLint);
        assert_eq!(observation.guard, "plan_lint.task_contract");
        assert_eq!(observation.actionability, FailureActionability::Actionable);
    }

    #[test]
    fn provider_parse_evidence_maps_to_provider_parse_observation() {
        let evidence = ContractEvidence::new("provider_transport")
            .with_violated_contract("provider_transport_parse_failure")
            .with_diagnostic_code("xml_tool_call_missing_name");

        let observation = FailureObservation::from_contract_evidence(&evidence);

        assert_eq!(
            observation.terminal_state,
            TerminalState::ProviderParseFailed
        );
        assert_eq!(
            observation.failure_class,
            FailureObservationClass::ProviderTransport
        );
        assert_eq!(observation.producer, FailureProducer::ProviderParser);
    }

    #[test]
    fn verifier_evidence_preserves_diagnostic_and_source_of_truth() {
        let evidence = ContractEvidence::new("verifier")
            .with_violated_contract("command_failed:1")
            .with_diagnostic_code("rust_compile_error")
            .with_failure_signature("verifier|cargo test|rust_compile_error")
            .with_source_of_truth("original_verifier_diagnostic")
            .with_command("cargo test");

        let observation = FailureObservation::from_contract_evidence(&evidence);

        assert_eq!(
            observation.terminal_state,
            TerminalState::VerifierCommandFailed
        );
        assert_eq!(observation.diagnostic_code, "rust_compile_error");
        assert_eq!(observation.source_of_truth, "original_verifier_diagnostic");
        assert_eq!(
            observation.failure_signature.as_deref(),
            Some("verifier|cargo test|rust_compile_error")
        );
    }

    #[test]
    fn explicit_stop_remains_visible_not_source_repair() {
        let evidence = ContractEvidence::new("repair")
            .with_active_job("explicit_stop")
            .with_repair_action("stop_with_structured_evidence")
            .with_explicit_stop_reason("patch_validation_rejected_unsafe_repair");

        let observation = FailureObservation::from_contract_evidence(&evidence);

        assert_eq!(observation.terminal_state, TerminalState::ExplicitStop);
        assert_eq!(
            observation.actionability,
            FailureActionability::ExplicitStop
        );
        assert_eq!(
            observation.explicit_stop_reason.as_deref(),
            Some("patch_validation_rejected_unsafe_repair")
        );
    }

    #[test]
    fn unknown_guard_stays_unknown() {
        let evidence = ContractEvidence::new("custom_guard").with_diagnostic("custom failure");

        let observation = FailureObservation::from_contract_evidence(&evidence);

        assert_eq!(observation.terminal_state, TerminalState::Unknown);
        assert_eq!(observation.actionability, FailureActionability::Unknown);
    }
}
