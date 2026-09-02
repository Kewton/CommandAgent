use crate::planner::adjudication::contract::IntentId;
use crate::planner::profile_descriptor::{
    DATA_PROFILE_ID, INGEST_PROFILE_ID, NEXTJS_PROFILE_ID, PYTHON_CLI_PROFILE_ID,
};

use super::family_catalog::TaskFamilyId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BandValue {
    pub profile: &'static str,
    pub intent: IntentId,
    pub family: TaskFamilyId,
    pub full: u16,
    pub denominator: u16,
    pub display_rate: &'static str,
    pub arm: &'static str,
    pub measurement: &'static str,
    pub source: &'static str,
    pub full_meaning: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BandDurationEstimate {
    pub profile: &'static str,
    pub intent: IntentId,
    pub family: TaskFamilyId,
    pub sample_count: u16,
    pub total_seconds: u64,
}

impl BandDurationEstimate {
    pub fn average_seconds(self) -> f64 {
        self.total_seconds as f64 / f64::from(self.sample_count)
    }

    pub fn average_minutes(self) -> f64 {
        self.average_seconds() / 60.0
    }
}

pub(super) const NEXTJS_MEANING: &str = "build + real-browser route, interaction, and state-change evidence; T1 testimony binding is active, with violations failing and claims_absent/unrecognized prose recorded without promotion.";
const DATA_MEANING: &str = "pipeline execution plus E1 inspection, E2 claim binding, E3 rerun consistency, and E4 schema conformance; testimony binding is active as E2.";
const CLI_MEANING: &str = "C1-C4 pass, including README output claims bound to live CLI output by C3; testimony binding is active as C3.";
const INGEST_MEANING: &str = "N1-N5 pass, including source-bound record values and complete candidate accounting; testimony/source binding is active as N2.";
const FIX_MEANING: &str = "the before-state reproduces, the repair makes the check pass, and no regression remains under F1-F3; no separate testimony check is active.";
const INVESTIGATION_MEANING: &str = "I1 executes a failing reproducer and I2 binds report claims to observed evidence; testimony binding is active as I2.";

pub const BAND_VALUES: &[BandValue] = &[
    band(
        NEXTJS_PROFILE_ID,
        IntentId::Create,
        TaskFamilyId::Quiz,
        23,
        26,
        "88%",
        "scenario history",
        "through nextjs-t1-001",
        "workspace/management/runs/band_summary.md",
        NEXTJS_MEANING,
    ),
    band(
        NEXTJS_PROFILE_ID,
        IntentId::Create,
        TaskFamilyId::Breakout,
        5,
        17,
        "29%",
        "scenario history",
        "through nextjs-t1-001",
        "workspace/management/runs/band_summary.md",
        NEXTJS_MEANING,
    ),
    band(
        NEXTJS_PROFILE_ID,
        IntentId::Create,
        TaskFamilyId::Space,
        3,
        35,
        "9%",
        "scenario history",
        "through nextjs-t1-001",
        "workspace/management/runs/band_summary.md",
        NEXTJS_MEANING,
    ),
    band(
        DATA_PROFILE_ID,
        IntentId::Create,
        TaskFamilyId::Aggregation,
        2,
        6,
        "33%",
        "Window B",
        "uat-test0715-data-007",
        "workspace/management/runs/band_summary_data.md",
        DATA_MEANING,
    ),
    band(
        DATA_PROFILE_ID,
        IntentId::Create,
        TaskFamilyId::Timeseries,
        0,
        6,
        "0%",
        "Window B",
        "uat-test0716-data-009",
        "workspace/management/runs/band_summary_data.md",
        DATA_MEANING,
    ),
    band(
        PYTHON_CLI_PROFILE_ID,
        IntentId::Create,
        TaskFamilyId::Stats,
        0,
        3,
        "0%",
        "formal Window B",
        "uat-test0725-cli-elev-004",
        "workspace/management/runs/band_summary_cli.md",
        CLI_MEANING,
    ),
    band(
        PYTHON_CLI_PROFILE_ID,
        IntentId::Create,
        TaskFamilyId::Filter,
        0,
        3,
        "0%",
        "formal Window B",
        "uat-test0725-cli-elev-004",
        "workspace/management/runs/band_summary_cli.md",
        CLI_MEANING,
    ),
    band(
        INGEST_PROFILE_ID,
        IntentId::Create,
        TaskFamilyId::List,
        4,
        6,
        "66.7%",
        "formal elevated Window B",
        "uat-test0726-ingest-elev-008",
        "workspace/management/runs/band_summary_ingest.md",
        INGEST_MEANING,
    ),
    band(
        INGEST_PROFILE_ID,
        IntentId::Create,
        TaskFamilyId::Table,
        4,
        6,
        "66.7%",
        "formal elevated Window B",
        "uat-test0726-ingest-elev-008",
        "workspace/management/runs/band_summary_ingest.md",
        INGEST_MEANING,
    ),
    band(
        NEXTJS_PROFILE_ID,
        IntentId::Fix,
        TaskFamilyId::CompileErrorFix,
        0,
        3,
        "0%",
        "Window B",
        "post-FIX-5",
        "workspace/management/runs/band_summary_fix.md",
        FIX_MEANING,
    ),
    band(
        NEXTJS_PROFILE_ID,
        IntentId::Fix,
        TaskFamilyId::ContractHookFix,
        0,
        2,
        "0%",
        "Window B",
        "post-FIX-5",
        "workspace/management/runs/band_summary_fix.md",
        FIX_MEANING,
    ),
    band(
        DATA_PROFILE_ID,
        IntentId::Investigate,
        TaskFamilyId::Pipe,
        0,
        3,
        "0%",
        "Window B",
        "uat-test0718-inv-002",
        "workspace/management/runs/band_summary_investigation.md",
        INVESTIGATION_MEANING,
    ),
    band(
        DATA_PROFILE_ID,
        IntentId::Investigate,
        TaskFamilyId::Schema,
        0,
        3,
        "0%",
        "Window B",
        "uat-test0718-inv-002",
        "workspace/management/runs/band_summary_investigation.md",
        INVESTIGATION_MEANING,
    ),
];

// These are the formal duration rows used by GUI Gate 1 at this catalog
// measurement. Keeping the frozen totals here makes the CLI estimate available
// outside the CommandAgent source checkout without reparsing evidence files.
pub const BAND_DURATION_ESTIMATES: &[BandDurationEstimate] = &[
    duration(
        PYTHON_CLI_PROFILE_ID,
        IntentId::Create,
        TaskFamilyId::Stats,
        3,
        1_896,
    ),
    duration(
        PYTHON_CLI_PROFILE_ID,
        IntentId::Create,
        TaskFamilyId::Filter,
        3,
        1_843,
    ),
    duration(
        INGEST_PROFILE_ID,
        IntentId::Create,
        TaskFamilyId::List,
        3,
        83,
    ),
    duration(
        INGEST_PROFILE_ID,
        IntentId::Create,
        TaskFamilyId::Table,
        3,
        109,
    ),
];

const fn duration(
    profile: &'static str,
    intent: IntentId,
    family: TaskFamilyId,
    sample_count: u16,
    total_seconds: u64,
) -> BandDurationEstimate {
    BandDurationEstimate {
        profile,
        intent,
        family,
        sample_count,
        total_seconds,
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "a band row is a fixed ten-field displayed identity"
)]
const fn band(
    profile: &'static str,
    intent: IntentId,
    family: TaskFamilyId,
    full: u16,
    denominator: u16,
    display_rate: &'static str,
    arm: &'static str,
    measurement: &'static str,
    source: &'static str,
    full_meaning: &'static str,
) -> BandValue {
    BandValue {
        profile,
        intent,
        family,
        full,
        denominator,
        display_rate,
        arm,
        measurement,
        source,
        full_meaning,
    }
}

pub fn value_for(
    profile: &str,
    intent: IntentId,
    family: TaskFamilyId,
) -> Option<&'static BandValue> {
    BAND_VALUES
        .iter()
        .find(|value| value.profile == profile && value.intent == intent && value.family == family)
}

pub fn duration_for(
    profile: &str,
    intent: IntentId,
    family: TaskFamilyId,
) -> Option<&'static BandDurationEstimate> {
    BAND_DURATION_ESTIMATES.iter().find(|estimate| {
        estimate.profile == profile && estimate.intent == intent && estimate.family == family
    })
}

pub fn duration_for_labels(
    profile: &str,
    intent: &str,
    family: &str,
) -> Option<&'static BandDurationEstimate> {
    BAND_DURATION_ESTIMATES.iter().find(|estimate| {
        estimate.profile == profile
            && estimate.intent.as_str() == intent
            && estimate.family.as_str() == family
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::boundary_shell::family_catalog::TASK_FAMILY_CATALOG;

    #[test]
    fn every_formal_family_has_exactly_one_band_value_and_full_meaning() {
        for family in TASK_FAMILY_CATALOG {
            let matches = BAND_VALUES
                .iter()
                .filter(|value| {
                    value.profile == family.profile
                        && value.intent == family.intent
                        && value.family == family.id
                })
                .collect::<Vec<_>>();
            assert_eq!(matches.len(), 1, "{family:?}");
            assert!(!matches[0].full_meaning.is_empty());
            assert!(matches[0].denominator > 0);
        }
    }

    #[test]
    fn duration_estimates_are_positive_unique_registered_band_rows() {
        for (index, estimate) in BAND_DURATION_ESTIMATES.iter().enumerate() {
            assert!(estimate.sample_count > 0, "{estimate:?}");
            assert!(estimate.total_seconds > 0, "{estimate:?}");
            assert!(
                value_for(estimate.profile, estimate.intent, estimate.family).is_some(),
                "{estimate:?}"
            );
            assert!(
                BAND_DURATION_ESTIMATES[index + 1..].iter().all(|other| (
                    other.profile,
                    other.intent,
                    other.family
                ) != (
                    estimate.profile,
                    estimate.intent,
                    estimate.family
                )),
                "duplicate duration estimate: {estimate:?}"
            );
        }
    }

    #[test]
    fn duration_lookup_preserves_measured_and_unmeasured_bands() {
        let stats =
            duration_for(PYTHON_CLI_PROFILE_ID, IntentId::Create, TaskFamilyId::Stats).unwrap();
        assert_eq!(stats.sample_count, 3);
        assert_eq!(stats.total_seconds, 1_896);
        assert!((stats.average_minutes() - 10.533_333).abs() < 0.000_001);

        assert!(duration_for(NEXTJS_PROFILE_ID, IntentId::Create, TaskFamilyId::Quiz).is_none());
    }
}
