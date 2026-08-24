use std::path::Path;

use super::super::checks::{CLAIMS_BINDING_EVIDENCE_PATH, ClaimsBindingEvidence};
use crate::planner::verify::VerificationReport;

const MAX_EVIDENCE_BYTES: u64 = 8 * 1024 * 1024;

pub(crate) fn combined(
    profile: Option<&str>,
    report: &VerificationReport,
    root: Option<&Path>,
) -> Option<String> {
    let guidance = [
        super::profile_guidance(profile, report),
        for_failure(profile, report, root),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join("\n");
    (!guidance.is_empty()).then_some(guidance)
}

pub(crate) fn for_failure(
    profile: Option<&str>,
    report: &VerificationReport,
    root: Option<&Path>,
) -> Option<String> {
    if profile.is_none_or(|profile| {
        !crate::planner::profile::resolve_profile_runtime(profile).synthesizes_fix_plan()
    }) || !report_mentions_failure(report)
    {
        return None;
    }
    from_evidence(root?)
}

fn from_evidence(root: &Path) -> Option<String> {
    let path =
        crate::tools::path_guard::resolve_existing(root, CLAIMS_BINDING_EVIDENCE_PATH).ok()?;
    let metadata = path.metadata().ok()?;
    if !metadata.is_file() || metadata.len() > MAX_EVIDENCE_BYTES {
        return None;
    }
    let evidence =
        serde_json::from_str::<ClaimsBindingEvidence>(&std::fs::read_to_string(path).ok()?).ok()?;
    if evidence.ok || evidence.capability_id != "data_claims_binding" {
        return None;
    }
    let lines = evidence
        .claims
        .iter()
        .filter(|claim| !claim.ok)
        .filter_map(|claim| {
            let nearest = claim.nearest_miss.as_ref()?;
            Some(format!(
                "- Report claim {} at {}:{} has no corresponding results.json value. Nearest candidate {}={}; absolute difference={}. Export every report numeric claim under an explicit values key (for example regional_名古屋).",
                quoted(&claim.raw),
                claim.report_path,
                claim.byte_offset,
                quoted(&nearest.key),
                nearest.result_value,
                nearest.absolute_difference,
            ))
        })
        .collect::<Vec<_>>();
    (!lines.is_empty()).then(|| {
        format!(
            "Claims-binding nearest-miss evidence (measured):\n{}",
            lines.join("\n")
        )
    })
}

fn report_mentions_failure(report: &VerificationReport) -> bool {
    report.profile_failures.iter().any(|reason| {
        reason.contains("data_claims_binding") || reason.contains("claims_binding_violation")
    }) || report.command_failures.iter().any(|failure| {
        failure.command.contains("data_claims_binding")
            || failure.reason.contains("data_claims_binding")
            || failure.reason.contains("claims_binding_violation")
    })
}

fn quoted(value: &str) -> String {
    serde_json::to_string(value).expect("serializing a JSON string is infallible")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PromptLayout;
    use crate::planner::repair::{RepairContext, build_repair_prompt_with_context};
    use crate::planner::verify::VerificationReport;

    const MEASURED_VIOLATIONS: &str = include_str!(
        "../../../../../tests/corpus/apps/test0715_data_b2g_e2_calibration/fixtures/data5_qwen35_profile_001/evidence/claims-binding.json"
    );

    #[test]
    fn measured_three_violation_evidence_reaches_repair_guidance() {
        let dir = tempfile::tempdir().unwrap();
        let evidence_path = dir.path().join(CLAIMS_BINDING_EVIDENCE_PATH);
        std::fs::create_dir_all(evidence_path.parent().unwrap()).unwrap();
        std::fs::write(&evidence_path, MEASURED_VIOLATIONS).unwrap();
        let mut report = VerificationReport::pass();
        report.push_profile_failure(
            "data_claims_binding:claims_binding_violation:output/report.md".to_string(),
        );
        let context = RepairContext {
            profile: Some("data".to_string()),
            workspace_root: Some(dir.path().to_path_buf()),
            prompt_layout: PromptLayout::Stable,
            ..RepairContext::default()
        };

        let prompt = build_repair_prompt_with_context("verify-results", &report, &context);
        assert_measured_details(&prompt);
        assert!(for_failure(Some("nextjs"), &report, Some(dir.path())).is_none());
        assert!(for_failure(Some("data"), &VerificationReport::pass(), Some(dir.path())).is_none());
    }

    fn assert_measured_details(text: &str) {
        for (claim, difference) in [
            ("40497.00", "19027"),
            ("40127.00", "18657"),
            ("36814.00", "15344"),
        ] {
            assert!(
                text.contains(&format!("Report claim \"{claim}\"")),
                "{text}"
            );
            assert!(
                text.contains(&format!(
                    "Nearest candidate \"2026-05_大阪\"=21470; absolute difference={difference}"
                )),
                "{text}"
            );
        }
        assert!(text.contains("regional_名古屋"));
    }
}
