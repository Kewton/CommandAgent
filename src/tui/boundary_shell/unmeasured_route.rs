use crate::planner::adjudication::contract::IntentId;
use crate::planner::profile_descriptor::{NEXTJS_PROFILE_ID, descriptor_for_name};

use super::band_catalog::{BandValue, NEXTJS_MEANING};
use super::confirmation::ConfirmationIdentity;
use super::family_catalog::TaskFamilyId;
use super::route::{DeterministicResolution, DeterministicRouteResult, RouteBasis, RouteCandidate};

pub const UNMEASURED_LABEL: &str = "未計測";

static UNMEASURED_CREATE_BAND: BandValue = BandValue {
    profile: NEXTJS_PROFILE_ID,
    intent: IntentId::Create,
    family: TaskFamilyId::Unknown,
    full: 0,
    denominator: 0,
    display_rate: UNMEASURED_LABEL,
    arm: UNMEASURED_LABEL,
    measurement: UNMEASURED_LABEL,
    source: UNMEASURED_LABEL,
    full_meaning: NEXTJS_MEANING,
};

pub fn candidate_for(result: &DeterministicRouteResult) -> Option<RouteCandidate> {
    if result.resolution != DeterministicResolution::Ambiguous
        || result.candidates.is_empty()
        || result.observations.iter().any(is_family_observation)
        || !result.candidates.iter().all(is_create_candidate)
    {
        return None;
    }
    let descriptor = descriptor_for_name(NEXTJS_PROFILE_ID)?;
    Some(RouteCandidate {
        profile: descriptor.id.clone(),
        intent: IntentId::Create,
        family: TaskFamilyId::Unknown,
        bases: result
            .observations
            .iter()
            .cloned()
            .chain(std::iter::once(RouteBasis {
                rule: "gui.family.unmeasured",
                observation: UNMEASURED_LABEL.to_string(),
            }))
            .collect(),
        contract_ref: descriptor.contract_ref?,
    })
}

pub fn band_for(candidate: &RouteCandidate) -> Option<&'static BandValue> {
    (is_create_candidate(candidate) && candidate.family == TaskFamilyId::Unknown)
        .then_some(&UNMEASURED_CREATE_BAND)
}

pub fn is_unmeasured_identity(identity: &ConfirmationIdentity) -> bool {
    identity.draft_manifest.is_none()
        && identity.intent == IntentId::Create.as_str()
        && identity.task_family == TaskFamilyId::Unknown.as_str()
        && identity.band_full == 0
        && identity.band_denominator == 0
        && [
            identity.band_rate.as_str(),
            identity.band_arm.as_str(),
            identity.band_measurement.as_str(),
            identity.band_source.as_str(),
        ]
        .into_iter()
        .all(|value| value == UNMEASURED_LABEL)
        && descriptor_for_name(&identity.profile)
            .is_some_and(|descriptor| descriptor.canonical == NEXTJS_PROFILE_ID)
}

fn is_create_candidate(candidate: &RouteCandidate) -> bool {
    candidate.intent == IntentId::Create
        && descriptor_for_name(candidate.profile.as_str())
            .is_some_and(|descriptor| descriptor.canonical == NEXTJS_PROFILE_ID)
}

fn is_family_observation(basis: &RouteBasis) -> bool {
    basis.rule == "explicit.family"
        || basis.rule == "request.family"
        || basis.rule.starts_with("material.family.")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::profile::ProfileId;
    use crate::planner::profile_descriptor::PYTHON_CLI_PROFILE_ID;
    use crate::tui::boundary_shell::route::{
        ExplicitRouteBinding, RouteRequest, deterministic_route,
    };

    #[test]
    fn unclassified_create_request_gets_only_the_unmeasured_family() {
        let workspace = tempfile::tempdir().unwrap();
        let result = deterministic_route(RouteRequest {
            request: "Create a small team homepage",
            workspace: workspace.path(),
            explicit: ExplicitRouteBinding {
                profile: Some(ProfileId::parse(NEXTJS_PROFILE_ID)),
                intent: Some(IntentId::Create),
                family: None,
            },
        });

        assert_eq!(result.resolution, DeterministicResolution::Ambiguous);
        let selected = candidate_for(&result).unwrap();
        assert_eq!(selected.family, TaskFamilyId::Unknown);
        let band = band_for(&selected).unwrap();
        assert_eq!(band.denominator, 0);
        assert_eq!(band.display_rate, UNMEASURED_LABEL);
        assert_eq!(band.source, UNMEASURED_LABEL);
    }

    #[test]
    fn inferred_create_without_a_family_is_also_admitted() {
        let workspace = tempfile::tempdir().unwrap();
        let result = deterministic_route(RouteRequest {
            request: "Create a small team homepage",
            workspace: workspace.path(),
            explicit: ExplicitRouteBinding {
                profile: Some(ProfileId::parse(NEXTJS_PROFILE_ID)),
                ..ExplicitRouteBinding::default()
            },
        });

        assert_eq!(
            candidate_for(&result).map(|candidate| candidate.intent),
            Some(IntentId::Create)
        );
    }

    #[test]
    fn family_evidence_and_other_ambiguous_routes_are_not_promoted() {
        let workspace = tempfile::tempdir().unwrap();
        for (profile, request) in [
            (NEXTJS_PROFILE_ID, "Create a Quiz and Breakout game"),
            (NEXTJS_PROFILE_ID, "Create a schema explorer"),
            (PYTHON_CLI_PROFILE_ID, "Create a CLI tool"),
        ] {
            let result = deterministic_route(RouteRequest {
                request,
                workspace: workspace.path(),
                explicit: ExplicitRouteBinding {
                    profile: Some(ProfileId::parse(profile)),
                    intent: Some(IntentId::Create),
                    family: None,
                },
            });
            assert!(candidate_for(&result).is_none(), "{profile}: {result:?}");
        }
    }

    #[test]
    fn missing_intent_evidence_is_not_silently_treated_as_create() {
        let workspace = tempfile::tempdir().unwrap();
        let result = deterministic_route(RouteRequest {
            request: "Polish the team homepage",
            workspace: workspace.path(),
            explicit: ExplicitRouteBinding {
                profile: Some(ProfileId::parse(NEXTJS_PROFILE_ID)),
                ..ExplicitRouteBinding::default()
            },
        });

        assert!(candidate_for(&result).is_none());
    }
}
