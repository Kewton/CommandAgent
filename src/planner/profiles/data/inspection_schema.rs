use std::path::Path;

use anyhow::Context;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::evidence_envelope::EvidenceFamily;
use crate::planner::failure_vocabulary::ViolationId;

mod input_selection;
mod input_table;

pub const EVIDENCE_PATH: &str = "evidence/inspection-schema.json";
const INSPECTION_PATH: &str = "output/inspection.json";
const MAX_INSPECTION_BYTES: u64 = 2 * 1024 * 1024;
const REQUIRED_KEYS: [&str; 5] = [
    "column_names",
    "input_row_count",
    "type_summaries",
    "distinct_values",
    "sample_rows",
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InspectionSchemaEvidence {
    pub capability_id: String,
    pub status: String,
    pub ok: bool,
    pub inspection_path: String,
    pub input_path: Option<String>,
    pub failure_kinds: Vec<String>,
}

pub fn check(root: &Path) -> anyhow::Result<InspectionSchemaEvidence> {
    check_with_goal(root, None)
}

pub fn check_with_goal(
    root: &Path,
    goal: Option<&str>,
) -> anyhow::Result<InspectionSchemaEvidence> {
    let mut evidence = InspectionSchemaEvidence {
        capability_id: "data_inspection_schema".to_string(),
        status: "failed".to_string(),
        ok: false,
        inspection_path: INSPECTION_PATH.to_string(),
        input_path: None,
        failure_kinds: Vec::new(),
    };
    evaluate(root, goal, &mut evidence);
    evidence.ok = evidence.failure_kinds.is_empty();
    evidence.status = if evidence.ok { "pass" } else { "failed" }.to_string();
    write_evidence(root, &evidence)?;
    Ok(evidence)
}

fn evaluate(root: &Path, goal: Option<&str>, evidence: &mut InspectionSchemaEvidence) {
    let document = match load_document(root) {
        Ok(document) => document,
        Err(failure) => {
            evidence.failure_kinds.push(failure);
            return;
        }
    };
    let missing = REQUIRED_KEYS
        .iter()
        .filter(|key| !document.contains_key(**key))
        .copied()
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        evidence.failure_kinds.push(
            ViolationId::inspection_schema(format!("missing_keys:{}", missing.join(",")))
                .to_string(),
        );
        return;
    }
    let input = match input_selection::load(root, goal) {
        Ok(input) => input,
        Err(failure) => {
            evidence.failure_kinds.push(failure);
            return;
        }
    };
    evidence.input_path = Some(input.relative_path);
    let columns = validate_columns(&document, &input.headers, &mut evidence.failure_kinds);
    validate_row_count(&document, input.row_count, &mut evidence.failure_kinds);
    validate_type_summaries(&document, &columns, &mut evidence.failure_kinds);
    validate_distinct_values(&document, &columns, &mut evidence.failure_kinds);
    validate_sample_rows(&document, &mut evidence.failure_kinds);
}

fn load_document(root: &Path) -> Result<Map<String, Value>, String> {
    let path =
        crate::tools::path_guard::resolve_existing(root, INSPECTION_PATH).map_err(|error| {
            ViolationId::inspection_schema(format!("inspection_path:{error}")).to_string()
        })?;
    let metadata = path.metadata().map_err(|error| {
        ViolationId::inspection_schema(format!("inspection_metadata:{error}")).to_string()
    })?;
    if !metadata.is_file() || metadata.len() > MAX_INSPECTION_BYTES {
        return Err("inspection_schema_violation:inspection_file_invalid".to_string());
    }
    let text = std::fs::read_to_string(path).map_err(|error| {
        ViolationId::inspection_schema(format!("inspection_unreadable:{error}")).to_string()
    })?;
    serde_json::from_str::<Value>(&text)
        .map_err(|error| {
            ViolationId::inspection_schema(format!("invalid_json:{error}")).to_string()
        })?
        .as_object()
        .cloned()
        .ok_or_else(|| "inspection_schema_violation:root_not_object".to_string())
}

fn validate_columns(
    document: &Map<String, Value>,
    headers: &[String],
    failures: &mut Vec<String>,
) -> Vec<String> {
    let Some(columns) = document["column_names"].as_array().and_then(|values| {
        values
            .iter()
            .map(|value| value.as_str().map(str::to_string))
            .collect::<Option<Vec<_>>>()
    }) else {
        failures.push("inspection_schema_violation:column_names:not_string_array".to_string());
        return Vec::new();
    };
    let missing = headers
        .iter()
        .filter(|header| !columns.contains(header))
        .cloned()
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        failures.push(
            ViolationId::inspection_schema(format!(
                "column_names_missing_headers:{}",
                missing.join(",")
            ))
            .to_string(),
        );
    }
    columns
}

fn validate_row_count(document: &Map<String, Value>, expected: u64, failures: &mut Vec<String>) {
    let Some(reported) = document["input_row_count"].as_number() else {
        failures.push("inspection_schema_violation:input_row_count:not_number".to_string());
        return;
    };
    if reported.as_u64() != Some(expected) && reported.as_f64() != Some(expected as f64) {
        failures.push(
            ViolationId::inspection_schema(format!(
                "input_row_count_mismatch:expected={expected}:reported={reported}"
            ))
            .to_string(),
        );
    }
}

fn validate_type_summaries(
    document: &Map<String, Value>,
    columns: &[String],
    failures: &mut Vec<String>,
) {
    let Some(summaries) = document["type_summaries"].as_object() else {
        failures.push("inspection_schema_violation:type_summaries:not_object".to_string());
        return;
    };
    let missing = columns
        .iter()
        .filter(|column| !summaries.contains_key(column.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        failures.push(
            ViolationId::inspection_schema(format!(
                "type_summaries_missing_columns:{}",
                missing.join(",")
            ))
            .to_string(),
        );
    }
}

fn validate_distinct_values(
    document: &Map<String, Value>,
    columns: &[String],
    failures: &mut Vec<String>,
) {
    let Some(distinct) = document["distinct_values"].as_object() else {
        failures.push("inspection_schema_violation:distinct_values:not_object".to_string());
        return;
    };
    let Some(summaries) = document["type_summaries"].as_object() else {
        return;
    };
    let missing = columns
        .iter()
        .filter(|column| {
            summaries.get(column.as_str()).is_some_and(categorical)
                && distinct
                    .get(column.as_str())
                    .is_none_or(|values| !values.is_array())
        })
        .cloned()
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        failures.push(
            ViolationId::inspection_schema(format!(
                "distinct_values_missing_categorical_columns:{}",
                missing.join(",")
            ))
            .to_string(),
        );
    }
}

fn categorical(summary: &Value) -> bool {
    let descriptor = summary.as_str().or_else(|| {
        summary
            .as_object()
            .and_then(|object| object.get("type").or_else(|| object.get("kind")))
            .and_then(Value::as_str)
    });
    descriptor.is_some_and(|descriptor| {
        matches!(
            descriptor.trim().to_ascii_lowercase().as_str(),
            "string" | "text" | "category" | "categorical" | "date" | "datetime"
        )
    })
}

fn validate_sample_rows(document: &Map<String, Value>, failures: &mut Vec<String>) {
    if document["sample_rows"]
        .as_array()
        .is_none_or(|rows| rows.iter().any(|row| !row.is_object()))
    {
        failures.push("inspection_schema_violation:sample_rows:not_object_array".to_string());
    }
}

fn write_evidence(root: &Path, evidence: &InspectionSchemaEvidence) -> anyhow::Result<()> {
    let path = crate::tools::path_guard::resolve_optional_existing(root, EVIDENCE_PATH)
        .context("inspection schema evidence path escapes workspace")?;
    std::fs::create_dir_all(path.parent().context("evidence parent missing")?)?;
    crate::evidence_envelope::write_json_for_path(
        &path,
        evidence,
        EvidenceFamily::E,
        EVIDENCE_PATH,
        true,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const INPUT: &str = include_str!(
        "../../../../tests/corpus/apps/test0715_data_inspection_schema/fixtures/sales.csv"
    );
    const RUN1: &str = include_str!(
        "../../../../tests/corpus/apps/test0715_data_inspection_schema/fixtures/run1-inspection.json"
    );
    const VALID: &str = include_str!(
        "../../../../tests/corpus/apps/test0715_data_inspection_schema/fixtures/valid-inspection.json"
    );

    #[test]
    fn observed_run1_fixture_lists_all_five_missing_keys() {
        assert_eq!(RUN1.len(), 533);
        let dir = materialize(RUN1);
        let evidence = check(dir.path()).unwrap();

        assert_eq!(
            evidence.failure_kinds,
            [
                "inspection_schema_violation:missing_keys:column_names,input_row_count,type_summaries,distinct_values,sample_rows"
            ]
        );
        assert!(dir.path().join(EVIDENCE_PATH).is_file());
    }

    #[test]
    fn complete_five_item_fixture_covers_input_headers() {
        let dir = materialize(VALID);
        let evidence = check(dir.path()).unwrap();

        assert!(evidence.ok, "{evidence:?}");
        assert!(evidence.failure_kinds.is_empty());
    }

    #[test]
    fn catalog_failure_output_preserves_the_missing_key_list() {
        let dir = materialize(RUN1);
        let step = crate::planner::step_plan::PlanStep {
            id: "verify-inspection".to_string(),
            kind: "verify".to_string(),
            expected_result: "pass".to_string(),
            instruction: "Verify inspection schema.".to_string(),
            expected_paths: vec![INSPECTION_PATH.to_string()],
            verify: vec![super::super::step_policy::catalog_check_command(
                "data_inspection_schema",
            )],
        };
        let (report, _) =
            crate::planner::verify::verify_step_with_profile_setup_observed_with_offline(
                dir.path(),
                &step,
                Some("data"),
                crate::minimal_loop::dependency_setup::NodeDependencySetupAuthority::None,
                true,
            );

        assert!(!report.is_pass());
        assert!(report.primary_reason().contains(
            "missing_keys:column_names,input_row_count,type_summaries,distinct_values,sample_rows"
        ));
    }

    fn materialize(inspection: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("data")).unwrap();
        std::fs::create_dir_all(dir.path().join("output")).unwrap();
        std::fs::write(dir.path().join("data/sales.csv"), INPUT).unwrap();
        std::fs::write(dir.path().join(INSPECTION_PATH), inspection).unwrap();
        dir
    }
}
