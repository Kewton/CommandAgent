use super::*;

/// Structured, machine-readable details attached to a minimal-loop failure.
///
/// The rendered error remains in [`RunSessionError::message`] for backward
/// compatibility. Consumers should use these fields for repair routing rather
/// than parsing that display text.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RunSessionErrorContext {
    /// Capability contract keys that were still missing.
    pub missing_capabilities: Vec<String>,
    /// Evidence contract keys that were still missing.
    pub missing_evidence: Vec<String>,
    /// Completion obligations that were still missing.
    pub missing_obligations: Vec<String>,
    /// Typed repair target selected by the verifier, when available.
    pub repair_target: Option<String>,
}

impl RunSessionErrorContext {
    pub(super) fn from_runtime_acceptance(
        runtime_acceptance: &RuntimeAcceptanceReport,
        repair_target: RepairTarget,
    ) -> Self {
        Self {
            missing_capabilities: runtime_acceptance.missing_capabilities.clone(),
            missing_evidence: runtime_acceptance.missing_evidence.clone(),
            missing_obligations: runtime_acceptance.missing_obligations.clone(),
            repair_target: Some(repair_target.as_str().to_string()),
        }
    }

    pub(super) fn from_repair_target(repair_target: RepairTarget) -> Self {
        Self {
            repair_target: Some(repair_target.as_str().to_string()),
            ..Self::default()
        }
    }

    pub(super) fn is_empty(&self) -> bool {
        self.missing_capabilities.is_empty()
            && self.missing_evidence.is_empty()
            && self.missing_obligations.is_empty()
            && self.repair_target.is_none()
    }
}

/// Minimal-loop failure with stable display bytes and structured repair data.
#[derive(Debug, Clone)]
pub struct RunSessionError {
    /// Backward-compatible human-readable failure and stop-reason text.
    pub message: String,
    /// Structured contract and repair classification.
    pub context: RunSessionErrorContext,
}

impl RunSessionError {
    pub(super) fn new(message: impl Into<String>, context: RunSessionErrorContext) -> Self {
        Self {
            message: message.into(),
            context,
        }
    }
}

impl std::fmt::Display for RunSessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.message.fmt(f)
    }
}

impl std::error::Error for RunSessionError {}
