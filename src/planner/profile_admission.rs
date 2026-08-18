use crate::planner::profile_manifest::ManifestStatus;

#[cfg(test)]
pub(crate) use crate::planner::adjudication::PROFILE_NOT_ADMITTED_REASON;
use crate::planner::adjudication::cap_assurance_for_status as apply_admission_cap;

pub(crate) fn status(profile: &str) -> ManifestStatus {
    crate::planner::profile_descriptor::descriptor_for_name(profile)
        .map(|descriptor| (descriptor.admission)())
        .unwrap_or(ManifestStatus::Draft)
}

pub(crate) fn cap_assurance(profile: &str, level: &mut String, reason: &mut String) {
    cap_assurance_for_status(status(profile), level, reason);
}

pub(crate) fn cap_assurance_for_status(
    status: ManifestStatus,
    level: &mut String,
    reason: &mut String,
) {
    apply_admission_cap(status, level, reason);
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::planner::profile_manifest::ManifestV1;
    use crate::planner::profiles::data::runtime::{DataAssurance, assurance_from_evidence};

    const RUN4_FIXTURE: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/corpus/apps/test0715_data_b2j_terminal_projection/fixtures/",
        "data7_qwen35_none_001"
    );

    #[test]
    fn cli_alias_is_admitted_and_unregistered_profiles_fail_closed() {
        assert_eq!(status("cli"), ManifestStatus::Admitted);
        assert_eq!(status("external-profile"), ManifestStatus::Draft);
    }

    #[test]
    fn draft_caps_only_assurance_above_static() {
        for original in ["full", "partial"] {
            let mut level = original.to_string();
            let mut reason = String::new();
            cap_assurance_for_status(ManifestStatus::Draft, &mut level, &mut reason);
            assert_eq!(level, "static", "original={original}");
            assert_eq!(reason, PROFILE_NOT_ADMITTED_REASON, "original={original}");
        }
        for original in ["static", "failed", "reduced"] {
            let mut level = original.to_string();
            let mut reason = "earned_reason".to_string();
            cap_assurance_for_status(ManifestStatus::Draft, &mut level, &mut reason);
            assert_eq!(level, original);
            assert_eq!(reason, "earned_reason");
        }
    }

    #[test]
    fn run4_full_evidence_is_static_at_both_draft_projection_boundaries() {
        let observed = assurance_from_evidence(Path::new(RUN4_FIXTURE));
        assert_eq!(observed, DataAssurance::Full);
        let draft_source = include_str!("profiles/data/manifest.toml")
            .replace("status = \"admitted\"", "status = \"draft\"");
        let draft = ManifestV1::from_toml(&draft_source).unwrap();
        assert_eq!(draft.metadata.status, ManifestStatus::Draft);

        for boundary in ["ultra_final_acceptance", "terminal_projection"] {
            let mut level = observed.as_str().to_string();
            let mut reason = String::new();
            cap_assurance_for_status(draft.metadata.status, &mut level, &mut reason);
            assert_eq!(level, "static", "boundary={boundary}");
            assert_eq!(reason, PROFILE_NOT_ADMITTED_REASON, "boundary={boundary}");
        }
    }

    #[test]
    fn admitted_status_preserves_earned_full() {
        let mut level = "full".to_string();
        let mut reason = String::new();
        cap_assurance_for_status(ManifestStatus::Admitted, &mut level, &mut reason);
        assert_eq!(level, "full");
        assert!(reason.is_empty());
    }
}
