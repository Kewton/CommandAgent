use crate::planner::adjudication::contract::IntentId;
use crate::planner::profile::ProfileId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub enum TaskFamilyId {
    Quiz,
    Breakout,
    Space,
    Aggregation,
    Timeseries,
    Stats,
    Filter,
    List,
    Table,
    CompileErrorFix,
    ContractHookFix,
    Pipe,
    Schema,
    Unknown,
}

impl TaskFamilyId {
    pub const ALL: [Self; 14] = [
        Self::Quiz,
        Self::Breakout,
        Self::Space,
        Self::Aggregation,
        Self::Timeseries,
        Self::Stats,
        Self::Filter,
        Self::List,
        Self::Table,
        Self::CompileErrorFix,
        Self::ContractHookFix,
        Self::Pipe,
        Self::Schema,
        Self::Unknown,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Quiz => "Quiz",
            Self::Breakout => "Breakout",
            Self::Space => "Space",
            Self::Aggregation => "aggregation",
            Self::Timeseries => "timeseries",
            Self::Stats => "stats",
            Self::Filter => "filter",
            Self::List => "list",
            Self::Table => "table",
            Self::CompileErrorFix => "compile_error_fix",
            Self::ContractHookFix => "contract_hook_fix",
            Self::Pipe => "pipe",
            Self::Schema => "schema",
            Self::Unknown => "unknown",
        }
    }

    pub fn parse(value: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|family| family.as_str().eq_ignore_ascii_case(value.trim()))
            .unwrap_or(Self::Unknown)
    }
}

impl std::fmt::Display for TaskFamilyId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskFamilyCatalogEntry {
    pub id: TaskFamilyId,
    pub profile: &'static str,
    pub intent: IntentId,
    pub band_source: &'static str,
    pub band_row: &'static str,
}

const NEXTJS_BAND: &str = "workspace/management/runs/band_summary.md";
const DATA_BAND: &str = "workspace/management/runs/band_summary_data.md";
const CLI_BAND: &str = "workspace/management/runs/band_summary_cli.md";
const INGEST_BAND: &str = "workspace/management/runs/band_summary_ingest.md";
const FIX_BAND: &str = "workspace/management/runs/band_summary_fix.md";
const INVESTIGATION_BAND: &str = "workspace/management/runs/band_summary_investigation.md";

pub const TASK_FAMILY_CATALOG: &[TaskFamilyCatalogEntry] = &[
    entry(
        TaskFamilyId::Quiz,
        "nextjs",
        IntentId::Create,
        NEXTJS_BAND,
        "Quiz",
    ),
    entry(
        TaskFamilyId::Breakout,
        "nextjs",
        IntentId::Create,
        NEXTJS_BAND,
        "Breakout",
    ),
    entry(
        TaskFamilyId::Space,
        "nextjs",
        IntentId::Create,
        NEXTJS_BAND,
        "Space",
    ),
    entry(
        TaskFamilyId::Aggregation,
        "data",
        IntentId::Create,
        DATA_BAND,
        "aggregation",
    ),
    entry(
        TaskFamilyId::Timeseries,
        "data",
        IntentId::Create,
        DATA_BAND,
        "timeseries",
    ),
    entry(
        TaskFamilyId::Stats,
        "python-cli",
        IntentId::Create,
        CLI_BAND,
        "stats",
    ),
    entry(
        TaskFamilyId::Filter,
        "python-cli",
        IntentId::Create,
        CLI_BAND,
        "filter",
    ),
    entry(
        TaskFamilyId::List,
        "ingest",
        IntentId::Create,
        INGEST_BAND,
        "list",
    ),
    entry(
        TaskFamilyId::Table,
        "ingest",
        IntentId::Create,
        INGEST_BAND,
        "table",
    ),
    entry(
        TaskFamilyId::CompileErrorFix,
        "nextjs",
        IntentId::Fix,
        FIX_BAND,
        "compile_error_fix",
    ),
    entry(
        TaskFamilyId::ContractHookFix,
        "nextjs",
        IntentId::Fix,
        FIX_BAND,
        "contract_hook_fix",
    ),
    entry(
        TaskFamilyId::Pipe,
        "data",
        IntentId::Investigate,
        INVESTIGATION_BAND,
        "pipe",
    ),
    entry(
        TaskFamilyId::Schema,
        "data",
        IntentId::Investigate,
        INVESTIGATION_BAND,
        "schema",
    ),
];

const fn entry(
    id: TaskFamilyId,
    profile: &'static str,
    intent: IntentId,
    band_source: &'static str,
    band_row: &'static str,
) -> TaskFamilyCatalogEntry {
    TaskFamilyCatalogEntry {
        id,
        profile,
        intent,
        band_source,
        band_row,
    }
}

pub fn entries_for(profile: &ProfileId, intent: IntentId) -> Vec<&'static TaskFamilyCatalogEntry> {
    let profile = canonical_route_profile(profile);
    TASK_FAMILY_CATALOG
        .iter()
        .filter(|entry| entry.profile == profile && entry.intent == intent)
        .collect()
}

fn canonical_route_profile(profile: &ProfileId) -> &str {
    match profile {
        ProfileId::Cli => "python-cli",
        _ => profile.as_str(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use regex::Regex;

    use super::*;

    const PYTHON_VOCABULARY: &str =
        include_str!("../../../workspace/management/scripts/task_family_vocabulary.py");
    const NEXTJS_BAND_SOURCE: &str =
        include_str!("../../../workspace/management/runs/band_summary.md");
    const DATA_BAND_SOURCE: &str =
        include_str!("../../../workspace/management/runs/band_summary_data.md");
    const CLI_BAND_SOURCE: &str =
        include_str!("../../../workspace/management/runs/band_summary_cli.md");
    const INGEST_BAND_SOURCE: &str =
        include_str!("../../../workspace/management/runs/band_summary_ingest.md");
    const FIX_BAND_SOURCE: &str =
        include_str!("../../../workspace/management/runs/band_summary_fix.md");
    const INVESTIGATION_BAND_SOURCE: &str =
        include_str!("../../../workspace/management/runs/band_summary_investigation.md");

    #[test]
    fn rust_and_python_family_vocabularies_are_bidirectionally_equal() {
        let body = PYTHON_VOCABULARY
            .lines()
            .find_map(|line| {
                line.strip_prefix("TASK_FAMILY_VOCABULARY = (")
                    .and_then(|body| body.strip_suffix(')'))
            })
            .expect("Python must declare a literal TASK_FAMILY_VOCABULARY tuple");
        let literal = Regex::new(r#""([^"]+)""#).unwrap();
        let python = literal
            .captures_iter(body)
            .map(|capture| capture[1].to_string())
            .collect::<BTreeSet<_>>();
        let rust = TaskFamilyId::ALL
            .into_iter()
            .map(|family| family.as_str().to_string())
            .collect::<BTreeSet<_>>();
        assert_eq!(rust, python);
    }

    #[test]
    fn every_catalog_entry_cites_an_existing_formal_band_row() {
        for entry in TASK_FAMILY_CATALOG {
            let source = match entry.band_source {
                NEXTJS_BAND => NEXTJS_BAND_SOURCE,
                DATA_BAND => DATA_BAND_SOURCE,
                CLI_BAND => CLI_BAND_SOURCE,
                INGEST_BAND => INGEST_BAND_SOURCE,
                FIX_BAND => FIX_BAND_SOURCE,
                INVESTIGATION_BAND => INVESTIGATION_BAND_SOURCE,
                other => panic!("unregistered band source: {other}"),
            };
            assert!(
                source.lines().any(|line| {
                    line.starts_with("| ") && line.contains(&format!("| {} |", entry.band_row))
                }),
                "{} missing from {}",
                entry.band_row,
                entry.band_source
            );
        }
    }

    #[test]
    fn catalog_misses_have_only_the_typed_unknown_result() {
        assert!(entries_for(&ProfileId::Ingest, IntentId::Fix).is_empty());
        assert_eq!(
            TaskFamilyId::parse("invented-family"),
            TaskFamilyId::Unknown
        );
    }
}
