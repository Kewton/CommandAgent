//! Circle-level admission cap for workflow schema v0.2.
//!
//! Node runtimes already cap draft-profile assurance at `static`. This leaf
//! closes the remaining projection path where a draft entry or terminal node
//! could otherwise be followed by an all-admitted route and become
//! `circle_full` after origin verification.

use crate::planner::adjudication::PROFILE_NOT_ADMITTED_REASON;
use crate::planner::profile_manifest::ManifestStatus;

use super::schema::Workflow;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct VerifiedTerminalAdjudication {
    pub(super) verdict: &'static str,
    pub(super) event_reason: Option<&'static str>,
    pub(super) evidence_reason: &'static str,
}

pub(super) fn after_origin_verification(workflow: &Workflow) -> VerifiedTerminalAdjudication {
    cap_for_statuses(
        workflow
            .nodes
            .values()
            .map(|node| crate::planner::profile_admission::status(&node.profile)),
    )
}

fn cap_for_statuses(
    statuses: impl IntoIterator<Item = ManifestStatus>,
) -> VerifiedTerminalAdjudication {
    if statuses
        .into_iter()
        .any(|status| status == ManifestStatus::Draft)
    {
        VerifiedTerminalAdjudication {
            verdict: "circle_failed",
            event_reason: Some(PROFILE_NOT_ADMITTED_REASON),
            evidence_reason: PROFILE_NOT_ADMITTED_REASON,
        }
    } else {
        VerifiedTerminalAdjudication {
            verdict: "circle_full",
            event_reason: None,
            evidence_reason: "verify_origin",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn any_draft_status_caps_a_verified_circle_below_full() {
        let result = cap_for_statuses([ManifestStatus::Admitted, ManifestStatus::Draft]);

        assert_eq!(result.verdict, "circle_failed");
        assert_eq!(result.event_reason, Some("profile_not_admitted"));
        assert_eq!(result.evidence_reason, "profile_not_admitted");
    }

    #[test]
    fn admitted_statuses_preserve_the_existing_verified_projection() {
        let result = cap_for_statuses([ManifestStatus::Admitted, ManifestStatus::Admitted]);

        assert_eq!(result.verdict, "circle_full");
        assert_eq!(result.event_reason, None);
        assert_eq!(result.evidence_reason, "verify_origin");
    }
}
