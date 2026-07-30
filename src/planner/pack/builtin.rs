//! Builtin assist routes preserving the historical no-pack byte protocol.
//!
//! Each route is a typed equivalent of one fixed `assist.yaml` binding. The
//! renderer closure remains the existing Rust implementation; selecting the
//! renderer now crosses this single pack registry rather than a call-site
//! string branch.

use super::{AssistSource, InjectionPoint, VocabularySource};

pub(crate) const BUILTIN_PACK_SET_ID: &str = "commandagent-default";
pub(crate) const BUILTIN_PACK_SET_VERSION: &str = "1.0.0";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BuiltinAssistRoute {
    IngestSnapshotStructure,
    IngestFrozenCandidateIds,
    InvestigationReproducerOutput,
    InvestigationWorkspaceFiles,
    FixFailureOutput,
    VerifiedDiagnosisCarry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BuiltinRouteSource {
    Injection(AssistSource),
    Vocabulary(VocabularySource),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BuiltinRouteDescriptor {
    pub source: BuiltinRouteSource,
    pub points: &'static [InjectionPoint],
}

const DECLARE_INGEST: &[InjectionPoint] = &[InjectionPoint::DeclareIngestInspection];
const IMPLEMENT_INGEST: &[InjectionPoint] = &[InjectionPoint::ImplementIngestDelivery];
const DIAGNOSE: &[InjectionPoint] = &[InjectionPoint::Diagnose];
const FIX_FAILURE: &[InjectionPoint] = &[InjectionPoint::IsolateCause, InjectionPoint::Repair];
const VERIFIED_DIAGNOSIS: &[InjectionPoint] =
    &[InjectionPoint::ImplementFix, InjectionPoint::Repair];

impl BuiltinAssistRoute {
    pub(crate) const ALL: [Self; 6] = [
        Self::IngestSnapshotStructure,
        Self::IngestFrozenCandidateIds,
        Self::InvestigationReproducerOutput,
        Self::InvestigationWorkspaceFiles,
        Self::FixFailureOutput,
        Self::VerifiedDiagnosisCarry,
    ];

    pub(crate) const fn descriptor(self) -> BuiltinRouteDescriptor {
        match self {
            Self::IngestSnapshotStructure => BuiltinRouteDescriptor {
                source: BuiltinRouteSource::Injection(AssistSource::IngestSnapshotStructure),
                points: DECLARE_INGEST,
            },
            Self::IngestFrozenCandidateIds => BuiltinRouteDescriptor {
                source: BuiltinRouteSource::Vocabulary(VocabularySource::IngestCandidateIds),
                points: IMPLEMENT_INGEST,
            },
            Self::InvestigationReproducerOutput => BuiltinRouteDescriptor {
                source: BuiltinRouteSource::Injection(AssistSource::ReproducerOutput),
                points: DIAGNOSE,
            },
            Self::InvestigationWorkspaceFiles => BuiltinRouteDescriptor {
                source: BuiltinRouteSource::Vocabulary(
                    VocabularySource::InvestigationWorkspaceFiles,
                ),
                points: DIAGNOSE,
            },
            Self::FixFailureOutput => BuiltinRouteDescriptor {
                source: BuiltinRouteSource::Injection(AssistSource::FixFailureOutput),
                points: FIX_FAILURE,
            },
            Self::VerifiedDiagnosisCarry => BuiltinRouteDescriptor {
                source: BuiltinRouteSource::Injection(AssistSource::VerifiedDiagnosis),
                points: VERIFIED_DIAGNOSIS,
            },
        }
    }
}

pub(crate) fn render<T>(route: BuiltinAssistRoute, renderer: impl FnOnce() -> T) -> T {
    // Touching the descriptor is intentional: this is the one typed selection
    // seam used by all migrated producers. No prompt/event data is transformed.
    let binding = BuiltinAssistRoute::ALL
        .into_iter()
        .find(|candidate| *candidate == route)
        .map(BuiltinAssistRoute::descriptor);
    let _identity = (BUILTIN_PACK_SET_ID, BUILTIN_PACK_SET_VERSION);
    match binding {
        Some(_) => renderer(),
        None => unreachable!("typed builtin route missing from builtin pack registry"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_registry_covers_the_four_reference_migration_families() {
        assert_eq!(BuiltinAssistRoute::ALL.len(), 6);
        assert_eq!(BUILTIN_PACK_SET_ID, "commandagent-default");
        assert_eq!(BUILTIN_PACK_SET_VERSION, "1.0.0");
        assert_eq!(
            BuiltinAssistRoute::IngestSnapshotStructure
                .descriptor()
                .points,
            [InjectionPoint::DeclareIngestInspection]
        );
        assert_eq!(
            BuiltinAssistRoute::IngestFrozenCandidateIds
                .descriptor()
                .points,
            [InjectionPoint::ImplementIngestDelivery]
        );
        assert_eq!(
            BuiltinAssistRoute::InvestigationReproducerOutput
                .descriptor()
                .points,
            [InjectionPoint::Diagnose]
        );
        assert_eq!(
            BuiltinAssistRoute::FixFailureOutput.descriptor().points,
            [InjectionPoint::IsolateCause, InjectionPoint::Repair]
        );
        assert_eq!(
            BuiltinAssistRoute::VerifiedDiagnosisCarry
                .descriptor()
                .points,
            [InjectionPoint::ImplementFix, InjectionPoint::Repair]
        );
    }
}
