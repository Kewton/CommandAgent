//! Closed catalog entry for `artifact_contract_check` (Issue #933).
//!
//! The params name only an operator-registered adapter (`adapter_id`) and a
//! workspace-relative request file (`request`). No executable, argv, module
//! URL, or model can be expressed here; the adapter itself is resolved from the
//! operator registry before any provider exists.

use std::sync::OnceLock;

use toml::value::Table;

use super::{
    CapabilityKind, CapabilitySpec, CatalogError, InternalCapability, ParamSpec, ParamType,
    ResolvedCapability,
};

pub(crate) const ID: &str = "artifact_contract_check";

static PARAMS: [ParamSpec; 2] = [
    ParamSpec {
        name: "adapter_id",
        param_type: ParamType::String,
        required: true,
        default: None,
    },
    ParamSpec {
        name: "request",
        param_type: ParamType::Path,
        required: true,
        default: None,
    },
];
static SPEC: CapabilitySpec = CapabilitySpec {
    id: ID,
    kind: CapabilityKind::AdapterCheck,
    params: &PARAMS,
    description: "Run one operator-registered adapter over a workspace request and verify its envelope.",
};

/// A resolved `artifact_contract_check`: the adapter slug and the request
/// path. Everything executable lives in the operator registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactContractCheck {
    pub adapter_id: String,
    pub request: String,
}

pub(super) fn combined_registry() -> &'static [CapabilitySpec] {
    static COMBINED: OnceLock<Vec<CapabilitySpec>> = OnceLock::new();
    COMBINED.get_or_init(|| {
        super::command_checks::combined_registry(super::pack::combined_registry(
            super::ingest::registry(&super::BASE_REGISTRY),
        ))
        .iter()
        .copied()
        .chain([SPEC])
        .collect()
    })
}

pub(super) fn is_id(id: &str) -> bool {
    id == ID
}

pub(super) fn resolve(
    spec: &CapabilitySpec,
    params: &Table,
) -> Result<ResolvedCapability, CatalogError> {
    let adapter_id = super::required_string(spec, params, "adapter_id")?;
    crate::planner::artifact_contract_checks::protocol::validate_slug(adapter_id).map_err(
        |reason| CatalogError::InvalidParameter {
            id: spec.id.to_string(),
            parameter: "adapter_id".to_string(),
            reason,
        },
    )?;
    let request = super::required_path(spec, params, "request")?;
    crate::planner::artifact_contract_checks::admit_from_environment(adapter_id).map_err(
        |reason| CatalogError::InvalidParameter {
            id: spec.id.to_string(),
            parameter: "adapter_id".to_string(),
            reason,
        },
    )?;
    Ok(ResolvedCapability::Internal(
        InternalCapability::ArtifactContract(ArtifactContractCheck {
            adapter_id: adapter_id.to_string(),
            request,
        }),
    ))
}
