use std::fmt;

macro_rules! reconciliation_id {
    ($($arg:tt)*) => {
        $crate::planner::failure_vocabulary::ViolationId::reconciliation(format!($($arg)*)).to_string()
    };
}

macro_rules! claims_id {
    ($($arg:tt)*) => {
        $crate::planner::failure_vocabulary::ViolationId::claims_binding(format!($($arg)*)).to_string()
    };
}

macro_rules! rerun_id {
    ($($arg:tt)*) => {
        $crate::planner::failure_vocabulary::ViolationId::rerun_consistency(format!($($arg)*)).to_string()
    };
}

macro_rules! inspection_id {
    ($($arg:tt)*) => {
        $crate::planner::failure_vocabulary::ViolationId::inspection_schema(format!($($arg)*)).to_string()
    };
}

pub(crate) use {claims_id, inspection_id, reconciliation_id, rerun_id};

/// Machine-consumed stop-class identifiers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StopClassId {
    EdgeNotEarned { edge: String, reason: String },
}

impl StopClassId {
    #[cfg(test)]
    pub(crate) const ID_FAMILIES: &'static [&'static str] = &["edge_not_earned"];

    pub(crate) fn edge_not_earned(edge: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::EdgeNotEarned {
            edge: edge.into(),
            reason: reason.into(),
        }
    }
}

impl fmt::Display for StopClassId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EdgeNotEarned { edge, reason } => {
                write!(formatter, "edge_not_earned:{edge}:{reason}")
            }
        }
    }
}

/// Terminal reason identifiers that are not themselves stop classes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TerminalReasonId {
    OriginGoalUnderivable(String),
    OriginVerifyUnderivable(String),
}

impl TerminalReasonId {
    #[cfg(test)]
    pub(crate) const ID_FAMILIES: &'static [&'static str] =
        &["origin_goal_underivable", "origin_verify_underivable"];

    pub(crate) fn origin_goal_underivable(detail: impl Into<String>) -> Self {
        Self::OriginGoalUnderivable(detail.into())
    }

    pub(crate) fn origin_verify_underivable(detail: impl Into<String>) -> Self {
        Self::OriginVerifyUnderivable(detail.into())
    }
}

impl fmt::Display for TerminalReasonId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OriginGoalUnderivable(detail) => {
                write!(formatter, "origin_goal_underivable:{detail}")
            }
            Self::OriginVerifyUnderivable(detail) => {
                write!(formatter, "origin_verify_underivable:{detail}")
            }
        }
    }
}

/// Reasons attached to a projected assurance level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AssuranceReasonId {
    DataAssurance(String),
}

impl AssuranceReasonId {
    #[cfg(test)]
    pub(crate) const ID_FAMILIES: &'static [&'static str] = &["data_assurance_"];

    pub(crate) fn data_assurance(level: impl Into<String>) -> Self {
        Self::DataAssurance(level.into())
    }
}

impl fmt::Display for AssuranceReasonId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DataAssurance(level) => write!(formatter, "data_assurance_{level}"),
        }
    }
}

/// Evidence violation identifiers. The variant fixes the protocol prefix;
/// callers supply only the existing detail payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ViolationId {
    InspectionSchema(String),
    Reconciliation(String),
    ClaimsBinding(String),
    RerunConsistency(String),
    SourceBinding(String),
    CandidateSet(String),
    Accounting(String),
    FormatSchema(String),
    TestimonyBinding(String),
}

impl ViolationId {
    const TESTIMONY_BINDING_FAMILY: &'static str = "testimony_binding_violation";

    #[cfg(test)]
    pub(crate) const ID_FAMILIES: &'static [&'static str] = &[
        "inspection_schema_violation",
        "reconciliation_violation",
        "claims_binding_violation",
        "rerun_consistency_violation",
        "source_binding_violation",
        "candidate_set_violation",
        "accounting_violation",
        "format_schema_violation",
        "testimony_binding_violation",
    ];

    pub(crate) fn inspection_schema(detail: impl Into<String>) -> Self {
        Self::InspectionSchema(detail.into())
    }

    pub(crate) fn reconciliation(detail: impl Into<String>) -> Self {
        Self::Reconciliation(detail.into())
    }

    pub(crate) fn claims_binding(detail: impl Into<String>) -> Self {
        Self::ClaimsBinding(detail.into())
    }

    pub(crate) fn rerun_consistency(detail: impl Into<String>) -> Self {
        Self::RerunConsistency(detail.into())
    }

    pub(crate) fn source_binding(detail: impl Into<String>) -> Self {
        Self::SourceBinding(detail.into())
    }

    pub(crate) fn candidate_set(detail: impl Into<String>) -> Self {
        Self::CandidateSet(detail.into())
    }

    pub(crate) fn accounting(detail: impl Into<String>) -> Self {
        Self::Accounting(detail.into())
    }

    pub(crate) fn format_schema(detail: impl Into<String>) -> Self {
        Self::FormatSchema(detail.into())
    }

    pub(crate) fn testimony_binding(detail: impl Into<String>) -> Self {
        Self::TestimonyBinding(detail.into())
    }

    pub(crate) fn is_testimony_binding(value: &str) -> bool {
        value == Self::TESTIMONY_BINDING_FAMILY
            || value.starts_with(&format!("{}:", Self::TESTIMONY_BINDING_FAMILY))
    }
}

impl fmt::Display for ViolationId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (prefix, detail) = match self {
            Self::InspectionSchema(detail) => ("inspection_schema_violation", detail),
            Self::Reconciliation(detail) => ("reconciliation_violation", detail),
            Self::ClaimsBinding(detail) => ("claims_binding_violation", detail),
            Self::RerunConsistency(detail) => ("rerun_consistency_violation", detail),
            Self::SourceBinding(detail) => ("source_binding_violation", detail),
            Self::CandidateSet(detail) => ("candidate_set_violation", detail),
            Self::Accounting(detail) => ("accounting_violation", detail),
            Self::FormatSchema(detail) => ("format_schema_violation", detail),
            Self::TestimonyBinding(detail) => (Self::TESTIMONY_BINDING_FAMILY, detail),
        };
        write!(formatter, "{prefix}:{detail}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use regex::Regex;
    use serde::Deserialize;

    const CLASSES_TOML: &str = include_str!("../../workspace/management/classes.toml");
    const PYTHON_ID_VOCABULARY: &str =
        include_str!("../../workspace/management/scripts/id_vocabulary.py");

    #[derive(Debug, Deserialize)]
    struct Registry {
        class: Vec<RegistryClass>,
    }

    #[derive(Debug, Deserialize)]
    struct RegistryClass {
        id: String,
        match_reason: Option<String>,
        match_stop_class: Option<String>,
    }

    struct RustStopClassProducer {
        matchable_id: &'static str,
        source: &'static str,
        source_marker: &'static str,
    }

    const RUST_STOP_CLASS_PRODUCERS: &[RustStopClassProducer] = &[
        RustStopClassProducer {
            matchable_id: "model_stagnation:read_only_loop",
            source: include_str!("../minimal_loop/repair_pressure.rs"),
            source_marker: "model_stagnation:read_only_loop",
        },
        RustStopClassProducer {
            matchable_id: "edge_not_earned",
            source: include_str!("failure_vocabulary.rs"),
            source_marker: "edge_not_earned:{edge}:{reason}",
        },
        RustStopClassProducer {
            matchable_id: "failure_kind:process_failure",
            source: include_str!("../lib.rs"),
            source_marker: "\"process_failure\"",
        },
        RustStopClassProducer {
            matchable_id: "missing expected paths:",
            source: include_str!("../minimal_loop/loop_run.rs"),
            source_marker: "artifact_follow_through_exhausted: missing expected paths:",
        },
    ];

    #[test]
    fn displays_protocol_ids_byte_for_byte() {
        assert_eq!(
            StopClassId::edge_not_earned("create_to_fix", "run_stop").to_string(),
            "edge_not_earned:create_to_fix:run_stop"
        );
        assert_eq!(
            TerminalReasonId::origin_goal_underivable("no run_start event").to_string(),
            "origin_goal_underivable:no run_start event"
        );
        assert_eq!(
            TerminalReasonId::origin_verify_underivable("empty set").to_string(),
            "origin_verify_underivable:empty set"
        );
        assert_eq!(
            AssuranceReasonId::data_assurance("failed").to_string(),
            "data_assurance_failed"
        );
        assert_eq!(
            ViolationId::inspection_schema("input_header:missing").to_string(),
            "inspection_schema_violation:input_header:missing"
        );
        assert_eq!(
            ViolationId::reconciliation("input_rows=5 used_rows=4 excluded_rows=0").to_string(),
            "reconciliation_violation:input_rows=5 used_rows=4 excluded_rows=0"
        );
        assert_eq!(
            ViolationId::claims_binding("report_not_file:output/report.md").to_string(),
            "claims_binding_violation:report_not_file:output/report.md"
        );
        assert_eq!(
            ViolationId::rerun_consistency("pipeline_run:pipeline_exit_nonzero").to_string(),
            "rerun_consistency_violation:pipeline_run:pipeline_exit_nonzero"
        );
        assert_eq!(
            ViolationId::source_binding("record=1:field=date:value=2026-08-03").to_string(),
            "source_binding_violation:record=1:field=date:value=2026-08-03"
        );
        assert_eq!(
            ViolationId::candidate_set("unknown_candidate:events.html#0").to_string(),
            "candidate_set_violation:unknown_candidate:events.html#0"
        );
        assert_eq!(
            ViolationId::accounting("duplicate_record_index:0").to_string(),
            "accounting_violation:duplicate_record_index:0"
        );
        assert_eq!(
            ViolationId::format_schema("record=0:fields").to_string(),
            "format_schema_violation:record=0:fields"
        );
        assert_eq!(
            ViolationId::testimony_binding("claim=2:restart_not_observed").to_string(),
            "testimony_binding_violation:claim=2:restart_not_observed"
        );
    }

    #[test]
    fn every_rust_and_python_id_family_is_registered_for_adjudication() {
        let registry: Registry = toml::from_str(CLASSES_TOML).expect("classes.toml must parse");
        let terms = registry
            .class
            .iter()
            .flat_map(|class| {
                [
                    class.match_reason.as_deref(),
                    class.match_stop_class.as_deref(),
                ]
            })
            .flatten()
            .collect::<Vec<_>>();
        let mut emitted_ids = StopClassId::ID_FAMILIES
            .iter()
            .chain(TerminalReasonId::ID_FAMILIES)
            .chain(AssuranceReasonId::ID_FAMILIES)
            .chain(ViolationId::ID_FAMILIES)
            .map(|id| (*id).to_string())
            .collect::<Vec<_>>();
        emitted_ids.extend(python_produced_ids());
        let missing = emitted_ids
            .iter()
            .filter(|id| !terms.iter().any(|term| registry_term_covers(id, term)))
            .cloned()
            .collect::<Vec<_>>();

        assert!(
            missing.is_empty(),
            "Rust/Python ID families missing from classes.toml:\n- {}",
            missing.join("\n- ")
        );
    }

    #[test]
    fn every_stop_class_pattern_has_a_cross_language_producer() {
        let registry: Registry = toml::from_str(CLASSES_TOML).expect("classes.toml must parse");
        for producer in RUST_STOP_CLASS_PRODUCERS {
            assert!(
                producer.source.contains(producer.source_marker),
                "declared Rust stop-class producer is stale: {}",
                producer.matchable_id
            );
        }
        let mut produced_ids = RUST_STOP_CLASS_PRODUCERS
            .iter()
            .map(|producer| producer.matchable_id.to_string())
            .collect::<Vec<_>>();
        produced_ids.extend(python_produced_ids());
        let dead = registry
            .class
            .iter()
            .filter_map(|class| {
                class
                    .match_stop_class
                    .as_deref()
                    .filter(|pattern| {
                        !produced_ids
                            .iter()
                            .any(|producer| stop_pattern_matches(pattern, producer))
                    })
                    .map(|pattern| format!("{} => {}", class.id, pattern))
            })
            .collect::<Vec<_>>();

        assert!(
            dead.is_empty(),
            "classes.toml match_stop_class patterns without a Rust/Python producer:\n- {}",
            dead.join("\n- ")
        );
    }

    fn python_produced_ids() -> Vec<String> {
        let body = PYTHON_ID_VOCABULARY
            .lines()
            .find_map(|line| {
                line.strip_prefix("PYTHON_PRODUCED_IDS = (")
                    .and_then(|body| body.strip_suffix(')'))
            })
            .expect("id_vocabulary.py must declare a literal PYTHON_PRODUCED_IDS tuple");
        let literal = Regex::new(r#""([^"]+)""#).expect("literal regex");
        let ids = literal
            .captures_iter(body)
            .map(|capture| capture[1].to_string())
            .collect::<Vec<_>>();
        assert!(
            !ids.is_empty(),
            "PYTHON_PRODUCED_IDS must contain at least one literal ID"
        );
        ids
    }

    fn registry_term_covers(family: &str, term: &str) -> bool {
        if family.ends_with('_') {
            term.starts_with(family)
        } else {
            term == family || term.starts_with(&format!("{family}:"))
        }
    }

    fn stop_pattern_matches(pattern: &str, producer: &str) -> bool {
        pattern == producer || pattern.starts_with(producer) || producer.starts_with(pattern)
    }
}
