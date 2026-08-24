use crate::planner::adjudication::contract::IntentId;
use crate::planner::profile::ProfileId;
use crate::planner::profile_descriptor::{
    DATA_PROFILE_ID, INGEST_PROFILE_ID, NEXTJS_PROFILE_ID, PYTHON_CLI_PROFILE_ID,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub enum TaskFamilyId {
    Quiz,
    Breakout,
    Space,
    Aggregation,
    Timeseries,
    Generic,
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
        Self::Generic,
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
            Self::Generic => "generic",
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
        if value.trim().eq_ignore_ascii_case("stats") {
            return Self::Generic;
        }
        Self::ALL
            .into_iter()
            .find(|family| family.as_str().eq_ignore_ascii_case(value.trim()))
            .unwrap_or(Self::Unknown)
    }

    /// Source-compatible spelling for historical band code and classifier output.
    #[allow(non_upper_case_globals)]
    pub const Stats: Self = Self::Generic;
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
        NEXTJS_PROFILE_ID,
        IntentId::Create,
        NEXTJS_BAND,
        "Quiz",
    ),
    entry(
        TaskFamilyId::Breakout,
        NEXTJS_PROFILE_ID,
        IntentId::Create,
        NEXTJS_BAND,
        "Breakout",
    ),
    entry(
        TaskFamilyId::Space,
        NEXTJS_PROFILE_ID,
        IntentId::Create,
        NEXTJS_BAND,
        "Space",
    ),
    entry(
        TaskFamilyId::Aggregation,
        DATA_PROFILE_ID,
        IntentId::Create,
        DATA_BAND,
        "aggregation",
    ),
    entry(
        TaskFamilyId::Timeseries,
        DATA_PROFILE_ID,
        IntentId::Create,
        DATA_BAND,
        "timeseries",
    ),
    entry(
        TaskFamilyId::Generic,
        PYTHON_CLI_PROFILE_ID,
        IntentId::Create,
        CLI_BAND,
        "stats",
    ),
    entry(
        TaskFamilyId::Filter,
        PYTHON_CLI_PROFILE_ID,
        IntentId::Create,
        CLI_BAND,
        "filter",
    ),
    entry(
        TaskFamilyId::List,
        INGEST_PROFILE_ID,
        IntentId::Create,
        INGEST_BAND,
        "list",
    ),
    entry(
        TaskFamilyId::Table,
        INGEST_PROFILE_ID,
        IntentId::Create,
        INGEST_BAND,
        "table",
    ),
    entry(
        TaskFamilyId::CompileErrorFix,
        NEXTJS_PROFILE_ID,
        IntentId::Fix,
        FIX_BAND,
        "compile_error_fix",
    ),
    entry(
        TaskFamilyId::ContractHookFix,
        NEXTJS_PROFILE_ID,
        IntentId::Fix,
        FIX_BAND,
        "contract_hook_fix",
    ),
    entry(
        TaskFamilyId::Pipe,
        DATA_PROFILE_ID,
        IntentId::Investigate,
        INVESTIGATION_BAND,
        "pipe",
    ),
    entry(
        TaskFamilyId::Schema,
        DATA_PROFILE_ID,
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
    crate::planner::profile_descriptor::descriptor_for_name(profile.as_str())
        .map(|descriptor| descriptor.canonical)
        .unwrap_or_else(|| profile.as_str())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::Path;

    use regex::Regex;

    use crate::planner::pack::catalog::PackLocator;
    use crate::tui::boundary_shell::band_catalog::value_for;
    use crate::tui::boundary_shell::confirmation::{
        ConfirmationIdentity, ExecutionPins, PackSelection,
    };
    use crate::tui::boundary_shell::presentation::render_gate_one;
    use crate::tui::boundary_shell::route::{RouteBasis, RouteCandidate};

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
    fn rust_and_python_family_vocabularies_resolve_to_the_same_typed_identities() {
        let body = PYTHON_VOCABULARY
            .lines()
            .find_map(|line| {
                line.strip_prefix("TASK_FAMILY_VOCABULARY = (")
                    .and_then(|body| body.strip_suffix(')'))
            })
            .expect("Python must declare a literal TASK_FAMILY_VOCABULARY tuple");
        let literal = Regex::new(r#""([^"]+)""#).unwrap();
        let python_tokens = literal
            .captures_iter(body)
            .map(|capture| capture[1].to_string())
            .collect::<Vec<_>>();
        assert_eq!(python_tokens.len(), TaskFamilyId::ALL.len());
        let python = python_tokens
            .iter()
            .map(|token| {
                let family = TaskFamilyId::parse(token);
                assert!(
                    token == family.as_str() || token == "stats",
                    "unexpected family alias: {token}"
                );
                family
            })
            .collect::<BTreeSet<_>>();
        let rust = TaskFamilyId::ALL.into_iter().collect::<BTreeSet<_>>();
        assert_eq!(rust, python);
    }

    #[test]
    fn generic_cli_identity_accepts_the_legacy_stats_spelling() {
        assert_eq!(TaskFamilyId::parse("generic"), TaskFamilyId::Generic);
        assert_eq!(TaskFamilyId::parse("stats"), TaskFamilyId::Generic);
        assert_eq!(TaskFamilyId::Stats, TaskFamilyId::Generic);
        assert_eq!(TaskFamilyId::Generic.as_str(), "generic");
    }

    #[test]
    fn cli_alias_and_canonical_profile_share_generic_and_filter_entries() {
        assert_eq!(
            entries_for(&ProfileId::Cli, IntentId::Create),
            entries_for(&ProfileId::PythonCli, IntentId::Create)
        );
        assert_eq!(
            entries_for(&ProfileId::PythonCli, IntentId::Create)
                .into_iter()
                .map(|entry| entry.id)
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([TaskFamilyId::Generic, TaskFamilyId::Filter])
        );
    }

    #[test]
    fn python_cli_gate_one_distinguishes_generic_work_from_filter_work() {
        let generic = render_python_cli_card(TaskFamilyId::Generic, "create greet.py");
        assert!(generic.contains("/ generic (generic)"), "{generic}");
        assert!(!generic.contains("絞り込み"), "{generic}");

        let filter = render_python_cli_card(
            TaskFamilyId::Filter,
            "create a CLI --pattern filter command",
        );
        assert!(filter.contains("絞り込み (filter)"), "{filter}");
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

    fn render_python_cli_card(family: TaskFamilyId, request: &str) -> String {
        let workspace = tempfile::tempdir().unwrap();
        let route = RouteCandidate {
            profile: ProfileId::PythonCli,
            intent: IntentId::Create,
            family,
            bases: vec![RouteBasis {
                rule: "fixture",
                observation: family.to_string(),
            }],
            contract_ref: "docs/cli-profile-contract.md",
        };
        let band = value_for(PYTHON_CLI_PROFILE_ID, IntentId::Create, family).unwrap();
        let identity = ConfirmationIdentity::new(
            request.to_string(),
            workspace.path(),
            &route,
            band,
            ExecutionPins {
                planner_provider: "ollama".to_string(),
                planner_model: "planner".to_string(),
                executor_provider: "ollama".to_string(),
                executor_model: "executor".to_string(),
                preset: "profile".to_string(),
                think: None,
            },
            PackSelection::None,
        )
        .unwrap();
        render_gate_one(
            &identity,
            &PackLocator::new(Path::new(env!("CARGO_MANIFEST_DIR"))),
        )
        .unwrap()
    }
}
