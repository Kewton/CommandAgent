use std::sync::OnceLock;

use toml::value::Table;

use super::{
    CapabilityKind, CapabilitySpec, CatalogError, InternalCapability, ParamSpec, ParamType,
    ProbeCapability, ResolvedCapability, required_path, required_u16,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IngestInternalCheck {
    SourceBinding,
    CandidateAccounting,
    FormatSchema,
}

static PIPELINE_PARAMS: [ParamSpec; 2] = [
    ParamSpec {
        name: "entry",
        param_type: ParamType::Path,
        required: true,
        default: None,
    },
    ParamSpec {
        name: "timeout_seconds",
        param_type: ParamType::U16,
        required: true,
        default: None,
    },
];

const fn internal(id: &'static str, description: &'static str) -> CapabilitySpec {
    CapabilitySpec {
        id,
        kind: CapabilityKind::InternalCheck,
        params: &super::NO_PARAMS,
        description,
    }
}

static REGISTRY: [CapabilitySpec; 4] = [
    internal(
        "ingest_source_binding",
        "Bind every output field to one frozen source candidate.",
    ),
    internal(
        "ingest_candidate_accounting",
        "Reconcile frozen source candidates to accepted or excluded outcomes.",
    ),
    internal(
        "ingest_format_schema",
        "Validate records.json against the declared record format.",
    ),
    CapabilitySpec {
        id: "ingest_rerun_consistency",
        kind: CapabilityKind::Probe,
        params: &PIPELINE_PARAMS,
        description: "Rerun an ingest pipeline and compare canonical output artifacts.",
    },
];

pub(super) fn registry(base: &'static [CapabilitySpec]) -> &'static [CapabilitySpec] {
    static COMBINED: OnceLock<Vec<CapabilitySpec>> = OnceLock::new();
    COMBINED.get_or_init(|| {
        super::cli::combined_registry(super::data::combined_registry(base))
            .iter()
            .chain(REGISTRY.iter())
            .copied()
            .collect()
    })
}

pub(super) fn is_id(id: &str) -> bool {
    REGISTRY.iter().any(|spec| spec.id == id)
}

pub(super) fn resolve(
    spec: &CapabilitySpec,
    params: &Table,
) -> Result<ResolvedCapability, CatalogError> {
    let internal = match spec.id {
        "ingest_source_binding" => Some(IngestInternalCheck::SourceBinding),
        "ingest_candidate_accounting" => Some(IngestInternalCheck::CandidateAccounting),
        "ingest_format_schema" => Some(IngestInternalCheck::FormatSchema),
        _ => None,
    };
    if let Some(check) = internal {
        return Ok(ResolvedCapability::Internal(InternalCapability::Ingest(
            check,
        )));
    }

    let entry = required_path(spec, params, "entry")?;
    let timeout_seconds = required_u16(spec, params, "timeout_seconds")?;
    if !entry.starts_with("pipeline/") || !entry.ends_with(".py") {
        return Err(invalid(
            spec,
            "entry",
            "expected a .py path under pipeline/",
        ));
    }
    if timeout_seconds == 0 {
        return Err(invalid(
            spec,
            "timeout_seconds",
            "must be greater than zero",
        ));
    }
    Ok(ResolvedCapability::Probe(
        ProbeCapability::DataRerunConsistency {
            entry,
            timeout_seconds,
        },
    ))
}

pub(super) fn resolve_or_data(
    spec: &CapabilitySpec,
    params: &Table,
) -> Result<ResolvedCapability, CatalogError> {
    if is_id(spec.id) {
        resolve(spec, params)
    } else {
        super::data::resolve(spec, params)
    }
}

fn invalid(spec: &CapabilitySpec, parameter: &str, reason: &str) -> CatalogError {
    CatalogError::InvalidParameter {
        id: spec.id.to_string(),
        parameter: parameter.to_string(),
        reason: reason.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use toml::Value;

    use super::*;

    #[test]
    fn all_ingest_bindings_resolve_to_typed_adapters() {
        for (id, expected) in [
            ("ingest_source_binding", IngestInternalCheck::SourceBinding),
            (
                "ingest_candidate_accounting",
                IngestInternalCheck::CandidateAccounting,
            ),
            ("ingest_format_schema", IngestInternalCheck::FormatSchema),
        ] {
            assert_eq!(
                super::super::resolve(id, &Table::new()).unwrap(),
                ResolvedCapability::Internal(InternalCapability::Ingest(expected))
            );
        }
        let params = Table::from_iter([
            (
                "entry".to_string(),
                Value::String("pipeline/main.py".to_string()),
            ),
            ("timeout_seconds".to_string(), Value::Integer(30)),
        ]);
        assert_eq!(
            super::super::resolve("ingest_rerun_consistency", &params).unwrap(),
            ResolvedCapability::Probe(ProbeCapability::DataRerunConsistency {
                entry: "pipeline/main.py".to_string(),
                timeout_seconds: 30,
            })
        );
    }
}
