use std::collections::BTreeSet;
use std::io::{BufRead, Read, Write};
use std::path::{Path, PathBuf};

use anyhow::Context;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub const EVIDENCE_PATH: &str = "evidence/inspection-schema.json";
const INSPECTION_PATH: &str = "output/inspection.json";
const MAX_INSPECTION_BYTES: u64 = 2 * 1024 * 1024;
const MAX_HEADER_BYTES: u64 = 1024 * 1024;
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
    let mut evidence = InspectionSchemaEvidence {
        capability_id: "data_inspection_schema".to_string(),
        status: "failed".to_string(),
        ok: false,
        inspection_path: INSPECTION_PATH.to_string(),
        input_path: None,
        failure_kinds: Vec::new(),
    };
    evaluate(root, &mut evidence);
    evidence.ok = evidence.failure_kinds.is_empty();
    evidence.status = if evidence.ok { "pass" } else { "failed" }.to_string();
    write_evidence(root, &evidence)?;
    Ok(evidence)
}

fn evaluate(root: &Path, evidence: &mut InspectionSchemaEvidence) {
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
        evidence.failure_kinds.push(format!(
            "inspection_schema_violation:missing_keys:{}",
            missing.join(",")
        ));
        return;
    }
    let (input_path, headers) = match discover_input(root).and_then(|path| load_headers(root, path))
    {
        Ok(input) => input,
        Err(failure) => {
            evidence.failure_kinds.push(failure);
            return;
        }
    };
    evidence.input_path = Some(input_path);
    let columns = validate_columns(&document, &headers, &mut evidence.failure_kinds);
    validate_row_count(&document, &mut evidence.failure_kinds);
    validate_type_summaries(&document, &columns, &mut evidence.failure_kinds);
    validate_distinct_values(&document, &columns, &mut evidence.failure_kinds);
    validate_sample_rows(&document, &mut evidence.failure_kinds);
}

fn load_document(root: &Path) -> Result<Map<String, Value>, String> {
    let path = crate::tools::path_guard::resolve_existing(root, INSPECTION_PATH)
        .map_err(|error| format!("inspection_schema_violation:inspection_path:{error}"))?;
    let metadata = path
        .metadata()
        .map_err(|error| format!("inspection_schema_violation:inspection_metadata:{error}"))?;
    if !metadata.is_file() || metadata.len() > MAX_INSPECTION_BYTES {
        return Err("inspection_schema_violation:inspection_file_invalid".to_string());
    }
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("inspection_schema_violation:inspection_unreadable:{error}"))?;
    serde_json::from_str::<Value>(&text)
        .map_err(|error| format!("inspection_schema_violation:invalid_json:{error}"))?
        .as_object()
        .cloned()
        .ok_or_else(|| "inspection_schema_violation:root_not_object".to_string())
}

fn discover_input(root: &Path) -> Result<PathBuf, String> {
    let mut files = Vec::new();
    for directory in ["data", "input"] {
        collect_inputs(&root.join(directory), &mut files)
            .map_err(|error| format!("inspection_schema_violation:input_scan:{error}"))?;
    }
    files.sort();
    match files.as_slice() {
        [path] => Ok(path.clone()),
        [] => Err("inspection_schema_violation:input_missing".to_string()),
        _ => Err(format!(
            "inspection_schema_violation:multiple_inputs:{}",
            files
                .iter()
                .map(|path| crate::tools::path_guard::relative_display(root, path))
                .collect::<Vec<_>>()
                .join(",")
        )),
    }
}

fn collect_inputs(directory: &Path, files: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if !directory.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_inputs(&entry.path(), files)?;
        } else if file_type.is_file()
            && entry
                .path()
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| {
                    matches!(extension.to_ascii_lowercase().as_str(), "csv" | "tsv")
                })
        {
            files.push(entry.path());
        }
    }
    Ok(())
}

fn load_headers(root: &Path, path: PathBuf) -> Result<(String, Vec<String>), String> {
    let metadata = path
        .metadata()
        .map_err(|error| format!("inspection_schema_violation:input_metadata:{error}"))?;
    if !metadata.is_file() {
        return Err("inspection_schema_violation:input_not_file".to_string());
    }
    let file = std::fs::File::open(&path)
        .map_err(|error| format!("inspection_schema_violation:input_unreadable:{error}"))?;
    let mut line = String::new();
    std::io::BufReader::new(file)
        .take(MAX_HEADER_BYTES + 1)
        .read_line(&mut line)
        .map_err(|error| format!("inspection_schema_violation:input_header:{error}"))?;
    if line.len() as u64 > MAX_HEADER_BYTES || line.trim_end_matches(['\r', '\n']).is_empty() {
        return Err("inspection_schema_violation:input_header_invalid".to_string());
    }
    let delimiter = if path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("tsv"))
    {
        '\t'
    } else {
        ','
    };
    let mut headers = parse_record(line.trim_end_matches(['\r', '\n']), delimiter)?;
    if let Some(first) = headers.first_mut() {
        *first = first.trim_start_matches('\u{feff}').to_string();
    }
    if headers.iter().any(|header| header.trim().is_empty())
        || headers.iter().collect::<BTreeSet<_>>().len() != headers.len()
    {
        return Err("inspection_schema_violation:input_header_invalid".to_string());
    }
    Ok((
        crate::tools::path_guard::relative_display(root, &path),
        headers,
    ))
}

fn parse_record(line: &str, delimiter: char) -> Result<Vec<String>, String> {
    let mut fields = vec![String::new()];
    let mut chars = line.chars().peekable();
    let mut quoted = false;
    while let Some(ch) = chars.next() {
        match ch {
            '"' if quoted && chars.peek() == Some(&'"') => {
                fields.last_mut().unwrap().push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            value if value == delimiter && !quoted => fields.push(String::new()),
            value => fields.last_mut().unwrap().push(value),
        }
    }
    if quoted {
        Err("inspection_schema_violation:input_header_unclosed_quote".to_string())
    } else {
        Ok(fields)
    }
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
        failures.push(format!(
            "inspection_schema_violation:column_names_missing_headers:{}",
            missing.join(",")
        ));
    }
    columns
}

fn validate_row_count(document: &Map<String, Value>, failures: &mut Vec<String>) {
    if !document["input_row_count"].is_number() {
        failures.push("inspection_schema_violation:input_row_count:not_number".to_string());
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
        failures.push(format!(
            "inspection_schema_violation:type_summaries_missing_columns:{}",
            missing.join(",")
        ));
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
        failures.push(format!(
            "inspection_schema_violation:distinct_values_missing_categorical_columns:{}",
            missing.join(",")
        ));
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
    let mut file = std::fs::File::create(path)?;
    serde_json::to_writer_pretty(&mut file, evidence)?;
    file.write_all(b"\n")?;
    Ok(())
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
