use std::sync::OnceLock;

use toml::value::Table;

use super::{CapabilityKind, CapabilitySpec, CatalogError, InternalCapability, ResolvedCapability};

static REGISTRY: [CapabilitySpec; 3] = [
    CapabilitySpec {
        id: crate::planner::profiles::nextjs::testimony_binding::CHECK_ID,
        kind: CapabilityKind::InternalCheck,
        params: &super::NO_PARAMS,
        description: "Bind Next.js product testimony to route and browser observations.",
    },
    CapabilitySpec {
        id: "browser_readiness",
        kind: CapabilityKind::Probe,
        params: &super::NO_PARAMS,
        description: "Registered browser readiness probe.",
    },
    CapabilitySpec {
        id: "browser_interaction",
        kind: CapabilityKind::Probe,
        params: &super::NO_PARAMS,
        description: "Registered browser interaction probe.",
    },
];

pub(super) fn combined_registry(base: &'static [CapabilitySpec]) -> &'static [CapabilitySpec] {
    static COMBINED: OnceLock<Vec<CapabilitySpec>> = OnceLock::new();
    COMBINED.get_or_init(|| base.iter().chain(REGISTRY.iter()).copied().collect())
}

pub(super) fn is_id(id: &str) -> bool {
    REGISTRY.iter().any(|spec| spec.id == id)
}

pub(super) fn resolve(
    spec: &CapabilitySpec,
    _params: &Table,
) -> Result<ResolvedCapability, CatalogError> {
    match spec.id {
        crate::planner::profiles::nextjs::testimony_binding::CHECK_ID => Ok(
            ResolvedCapability::Internal(InternalCapability::NextjsTestimonyBinding),
        ),
        "browser_readiness" | "browser_interaction" => {
            Err(CatalogError::ProbeBindingUnimplemented {
                id: spec.id.to_string(),
            })
        }
        _ => unreachable!("Next.js registry id without resolver: {}", spec.id),
    }
}
