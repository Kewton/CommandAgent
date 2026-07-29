use std::fmt;

/// Machine-consumed stop-class identifiers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StopClassId {
    EdgeNotEarned { edge: String, reason: String },
}

impl StopClassId {
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
}

impl ViolationId {
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
        };
        write!(formatter, "{prefix}:{detail}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    }
}
