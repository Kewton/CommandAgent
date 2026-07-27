use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::Path;
use std::sync::OnceLock;

use anyhow::Context;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::accounting::{self, CandidateFreeze, FrozenCandidate};

pub const EVIDENCE_PATH: &str = "evidence/source-binding.json";
const RECORDS_PATH: &str = "output/records.json";
const MAX_RECORDS: usize = 10_000;
const MAX_FIELDS: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldType {
    String,
    Number,
    Boolean,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NormalizationRule {
    Identity,
    JapaneseDateToIso,
    NumberCanonical,
    Time24h,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FieldDeclaration {
    pub name: String,
    #[serde(rename = "type")]
    pub field_type: FieldType,
    pub normalizations: Vec<NormalizationRule>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordFormat {
    pub fields: Vec<FieldDeclaration>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NearestMiss {
    pub raw_source: String,
    pub normalized_source: String,
    pub distance: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldBinding {
    pub record_index: usize,
    pub candidate_id: String,
    pub source_path: Option<String>,
    pub candidate_byte_start: Option<usize>,
    pub candidate_byte_end: Option<usize>,
    pub field: String,
    pub output_value: String,
    pub declared_normalizations: Vec<NormalizationRule>,
    pub raw_source: Option<String>,
    pub normalized_source: Option<String>,
    pub transformations: Vec<NormalizationRule>,
    pub matched: bool,
    pub nearest_miss: Option<NearestMiss>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceBindingEvidence {
    pub capability_id: String,
    pub status: String,
    pub ok: bool,
    pub records_path: String,
    pub bindings: Vec<FieldBinding>,
    pub failure_kinds: Vec<String>,
}

#[derive(Debug, Clone)]
struct SourceValue {
    raw: String,
    normalized: String,
    transformations: Vec<NormalizationRule>,
}

pub fn check(root: &Path, frozen: &CandidateFreeze) -> anyhow::Result<SourceBindingEvidence> {
    let inspection = accounting::load_inspection(root)?;
    let format: RecordFormat = serde_json::from_value(frozen.record_format.clone())
        .context("source_binding_violation:record_format_invalid")?;
    validate_format(&format)?;
    let records = load_records(root)?;
    let accepted = inspection
        .candidate_accounting
        .accepted
        .into_iter()
        .map(|item| (item.record_index, item.candidate_id))
        .collect::<BTreeMap<_, _>>();
    let candidates = frozen
        .candidates
        .iter()
        .map(|candidate| (candidate.id.as_str(), candidate))
        .collect::<BTreeMap<_, _>>();
    let mut bindings = Vec::new();
    let mut failure_kinds = Vec::new();

    for (record_index, record) in records.iter().enumerate() {
        let candidate_id = accepted.get(&record_index).cloned().unwrap_or_default();
        let candidate = candidates.get(candidate_id.as_str()).copied();
        for field in &format.fields {
            let Some(value) = record.get(&field.name).and_then(value_text) else {
                continue;
            };
            let binding = bind_field(
                record_index,
                &candidate_id,
                &field.name,
                &value,
                &field.normalizations,
                candidate,
            );
            if !binding.matched {
                failure_kinds.push(format!(
                    "source_binding_violation:record={record_index}:field={}:value={value}",
                    field.name
                ));
            }
            bindings.push(binding);
        }
    }
    failure_kinds.sort();
    let claims_absent = records.is_empty();
    let ok = failure_kinds.is_empty();
    let status = if !ok {
        "failed"
    } else if claims_absent {
        "claims_absent"
    } else {
        "pass"
    };
    let evidence = SourceBindingEvidence {
        capability_id: "ingest_source_binding".to_string(),
        status: status.to_string(),
        ok,
        records_path: RECORDS_PATH.to_string(),
        bindings,
        failure_kinds,
    };
    write_evidence(root, &evidence)?;
    Ok(evidence)
}

pub fn record_format(frozen: &CandidateFreeze) -> anyhow::Result<RecordFormat> {
    serde_json::from_value(frozen.record_format.clone())
        .context("source_binding_violation:record_format_invalid")
}

pub fn records(root: &Path) -> anyhow::Result<Vec<serde_json::Map<String, Value>>> {
    load_records(root)
}

fn validate_format(format: &RecordFormat) -> anyhow::Result<()> {
    if format.fields.is_empty() || format.fields.len() > MAX_FIELDS {
        anyhow::bail!("source_binding_violation:field_count");
    }
    let mut names = BTreeSet::new();
    for field in &format.fields {
        if field.name.trim().is_empty()
            || !names.insert(field.name.as_str())
            || field.normalizations.is_empty()
        {
            anyhow::bail!("source_binding_violation:field_declaration");
        }
    }
    Ok(())
}

fn load_records(root: &Path) -> anyhow::Result<Vec<serde_json::Map<String, Value>>> {
    let path = crate::tools::path_guard::resolve_existing(root, RECORDS_PATH)
        .context("source_binding_violation:records_path")?;
    let value: Value = serde_json::from_slice(
        &std::fs::read(path).context("source_binding_violation:records_unreadable")?,
    )
    .context("source_binding_violation:records_invalid_json")?;
    let records = value
        .as_array()
        .context("source_binding_violation:records_not_array")?;
    if records.len() > MAX_RECORDS {
        anyhow::bail!("source_binding_violation:record_count_limit");
    }
    records
        .iter()
        .map(|record| {
            record
                .as_object()
                .cloned()
                .context("source_binding_violation:record_not_object")
        })
        .collect()
}

fn bind_field(
    record_index: usize,
    candidate_id: &str,
    field: &str,
    output_value: &str,
    rules: &[NormalizationRule],
    candidate: Option<&FrozenCandidate>,
) -> FieldBinding {
    let raw_candidate = candidate.map_or("", |candidate| candidate.raw.as_str());
    let values = source_values(raw_candidate, rules);
    let matched = values.iter().find(|candidate| {
        candidate.normalized == output_value || contains_value(&candidate.normalized, output_value)
    });
    let nearest_miss = matched
        .is_none()
        .then(|| {
            values
                .iter()
                .min_by_key(|candidate| edit_distance(output_value, &candidate.normalized))
                .map(|candidate| NearestMiss {
                    raw_source: candidate.raw.clone(),
                    normalized_source: candidate.normalized.clone(),
                    distance: edit_distance(output_value, &candidate.normalized),
                })
        })
        .flatten();
    FieldBinding {
        record_index,
        candidate_id: candidate_id.to_string(),
        source_path: candidate.map(|candidate| candidate.source_path.clone()),
        candidate_byte_start: candidate.map(|candidate| candidate.byte_start),
        candidate_byte_end: candidate.map(|candidate| candidate.byte_end),
        field: field.to_string(),
        output_value: output_value.to_string(),
        declared_normalizations: rules.to_vec(),
        raw_source: matched.map(|candidate| candidate.raw.clone()),
        normalized_source: matched.map(|candidate| candidate.normalized.clone()),
        transformations: matched
            .map(|candidate| candidate.transformations.clone())
            .unwrap_or_default(),
        matched: matched.is_some(),
        nearest_miss,
    }
}

fn source_values(raw_candidate: &str, rules: &[NormalizationRule]) -> Vec<SourceValue> {
    let mut values = Vec::new();
    for raw in fragments(raw_candidate) {
        let identity = normalize_space(&decode_entities(&raw));
        if rules.contains(&NormalizationRule::Identity) && !identity.is_empty() {
            values.push(SourceValue {
                raw: raw.clone(),
                normalized: identity.clone(),
                transformations: vec![NormalizationRule::Identity],
            });
        }
        if rules.contains(&NormalizationRule::JapaneseDateToIso) {
            values.extend(date_values(&raw));
        }
        if rules.contains(&NormalizationRule::NumberCanonical) {
            values.extend(number_values(&raw));
        }
        if rules.contains(&NormalizationRule::Time24h) {
            values.extend(time_values(&raw));
        }
    }
    values
}

fn fragments(raw: &str) -> Vec<String> {
    if !raw.contains('<') {
        return vec![raw.to_string()];
    }
    raw.split('<')
        .filter_map(|part| part.split_once('>').map(|(_, text)| text.trim()))
        .filter(|text| !text.is_empty())
        .map(str::to_string)
        .collect()
}

fn date_values(raw: &str) -> Vec<SourceValue> {
    static ERA: OnceLock<Regex> = OnceLock::new();
    static YEAR: OnceLock<Regex> = OnceLock::new();
    let era = ERA.get_or_init(|| {
        Regex::new(r"(令和|平成|昭和)\s*(元|[0-9０-９]{1,2})年\s*([0-9０-９]{1,2})月\s*([0-9０-９]{1,2})日").unwrap()
    });
    let year = YEAR.get_or_init(|| {
        Regex::new(r"([0-9０-９]{4})年\s*([0-9０-９]{1,2})月\s*([0-9０-９]{1,2})日").unwrap()
    });
    let mut values = Vec::new();
    for captures in era.captures_iter(raw) {
        let era_year = if &captures[2] == "元" {
            Some(1)
        } else {
            ascii_digits(&captures[2]).parse::<u32>().ok()
        };
        let base = match &captures[1] {
            "令和" => 2018,
            "平成" => 1988,
            "昭和" => 1925,
            _ => 0,
        };
        if let Some(era_year) = era_year {
            push_date(
                &mut values,
                captures.get(0).unwrap().as_str(),
                base + era_year,
                &captures[3],
                &captures[4],
            );
        }
    }
    for captures in year.captures_iter(raw) {
        if let Ok(year) = ascii_digits(&captures[1]).parse::<u32>() {
            push_date(
                &mut values,
                captures.get(0).unwrap().as_str(),
                year,
                &captures[2],
                &captures[3],
            );
        }
    }
    values
}

fn push_date(values: &mut Vec<SourceValue>, raw: &str, year: u32, month: &str, day: &str) {
    let parsed = ascii_digits(month)
        .parse::<u32>()
        .ok()
        .zip(ascii_digits(day).parse::<u32>().ok());
    if let Some((month, day)) = parsed
        && valid_date(year, month, day)
    {
        values.push(SourceValue {
            raw: raw.to_string(),
            normalized: format!("{year:04}-{month:02}-{day:02}"),
            transformations: vec![NormalizationRule::JapaneseDateToIso],
        });
    }
}

fn valid_date(year: u32, month: u32, day: u32) -> bool {
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    (1..=12).contains(&month) && (1..=days[(month - 1) as usize]).contains(&day)
}

fn number_values(raw: &str) -> Vec<SourceValue> {
    static NUMBER: OnceLock<Regex> = OnceLock::new();
    NUMBER
        .get_or_init(|| Regex::new(r"[0-9０-９][0-9０-９,，.．]*").unwrap())
        .find_iter(raw)
        .map(|found| SourceValue {
            raw: found.as_str().to_string(),
            normalized: ascii_digits(found.as_str())
                .replace(['，', ','], "")
                .replace('．', "."),
            transformations: vec![NormalizationRule::NumberCanonical],
        })
        .collect()
}

fn time_values(raw: &str) -> Vec<SourceValue> {
    static TIME: OnceLock<Regex> = OnceLock::new();
    let pattern = TIME.get_or_init(|| {
        Regex::new(r"(午前|午後)?\s*([0-9０-９]{1,2})(?::|：|時)([0-9０-９]{1,2})?\s*分?").unwrap()
    });
    pattern
        .captures_iter(raw)
        .filter_map(|captures| {
            let mut hour = ascii_digits(&captures[2]).parse::<u32>().ok()?;
            let minute = captures.get(3).map_or(0, |value| {
                ascii_digits(value.as_str()).parse().unwrap_or(60)
            });
            match captures.get(1).map(|value| value.as_str()) {
                Some("午前") if hour == 12 => hour = 0,
                Some("午後") if hour < 12 => hour += 12,
                _ => {}
            }
            (hour < 24 && minute < 60).then(|| SourceValue {
                raw: captures.get(0).unwrap().as_str().to_string(),
                normalized: format!("{hour:02}:{minute:02}"),
                transformations: vec![NormalizationRule::Time24h],
            })
        })
        .collect()
}

fn value_text(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}

fn decode_entities(value: &str) -> String {
    value
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

fn normalize_space(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn contains_value(source: &str, value: &str) -> bool {
    !value.is_empty()
        && source.match_indices(value).any(|(start, _)| {
            let end = start + value.len();
            let before = source[..start].chars().next_back();
            let after = source[end..].chars().next();
            before.is_none_or(|ch| !ch.is_alphanumeric())
                && after.is_none_or(|ch| !ch.is_alphanumeric())
        })
}

fn ascii_digits(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            '０'..='９' => char::from_u32('0' as u32 + (ch as u32 - '０' as u32)).unwrap(),
            _ => ch,
        })
        .collect()
}

fn edit_distance(left: &str, right: &str) -> usize {
    let mut row = (0..=right.chars().count()).collect::<Vec<_>>();
    for (i, lch) in left.chars().enumerate() {
        let mut previous = row[0];
        row[0] = i + 1;
        for (j, rch) in right.chars().enumerate() {
            let upper = row[j + 1];
            row[j + 1] = (row[j + 1] + 1)
                .min(row[j] + 1)
                .min(previous + usize::from(lch != rch));
            previous = upper;
        }
    }
    row[right.chars().count()]
}

fn write_evidence(root: &Path, evidence: &SourceBindingEvidence) -> anyhow::Result<()> {
    let path = crate::tools::path_guard::resolve_optional_existing(root, EVIDENCE_PATH)
        .context("source binding evidence path escapes workspace")?;
    std::fs::create_dir_all(
        path.parent()
            .context("source binding evidence parent missing")?,
    )?;
    let mut file = std::fs::File::create(path)?;
    serde_json::to_writer_pretty(&mut file, evidence)?;
    file.write_all(b"\n")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::profiles::ingest::accounting::{self, SelectorKind};
    use serde_json::json;

    #[test]
    fn declared_era_normalization_binds_to_iso_without_changing_the_date() {
        let dir = fixture(
            "<article><time>令和7年7月25日</time><span>市民会館</span></article>",
            json!([{"date":"2025-07-25","venue":"市民会館"}]),
        );
        let frozen = accounting::freeze(dir.path()).unwrap();

        let evidence = check(dir.path(), &frozen).unwrap();

        assert!(evidence.ok, "{evidence:?}");
        let date = evidence
            .bindings
            .iter()
            .find(|item| item.field == "date")
            .unwrap();
        assert_eq!(date.raw_source.as_deref(), Some("令和7年7月25日"));
        assert_eq!(date.normalized_source.as_deref(), Some("2025-07-25"));
        assert_eq!(date.transformations, [NormalizationRule::JapaneseDateToIso]);
        assert_eq!(
            date.source_path.as_deref(),
            Some("data/snapshots/events.html")
        );
        assert_eq!(date.candidate_byte_start, Some(0));
        assert!(date.candidate_byte_end.is_some_and(|end| end > 0));
    }

    #[test]
    fn shifted_date_is_rejected_with_the_real_date_as_nearest_miss() {
        let dir = fixture(
            "<article><time>令和7年7月25日</time><span>市民会館</span></article>",
            json!([{"date":"2025-07-26","venue":"市民会館"}]),
        );

        let evidence = check(dir.path(), &accounting::freeze(dir.path()).unwrap()).unwrap();

        assert!(!evidence.ok);
        let miss = evidence.bindings[0].nearest_miss.as_ref().unwrap();
        assert_eq!(miss.normalized_source, "2025-07-25");
        assert!(evidence.failure_kinds[0].contains("source_binding_violation"));
    }

    #[test]
    fn text_from_two_html_fragments_cannot_be_spliced_into_one_field() {
        let dir = fixture(
            "<article><span>中央</span><span>公園</span><time>令和7年7月25日</time></article>",
            json!([{"date":"2025-07-25","venue":"中央 公園"}]),
        );

        let evidence = check(dir.path(), &accounting::freeze(dir.path()).unwrap()).unwrap();

        assert!(!evidence.ok);
        let venue = evidence
            .bindings
            .iter()
            .find(|item| item.field == "venue")
            .unwrap();
        assert!(!venue.matched);
        assert!(venue.nearest_miss.is_some());
    }

    fn fixture(snapshot: &str, records: Value) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("data/snapshots")).unwrap();
        std::fs::create_dir_all(dir.path().join("output")).unwrap();
        std::fs::write(dir.path().join("data/snapshots/events.html"), snapshot).unwrap();
        std::fs::write(
            dir.path().join(accounting::INSPECTION_PATH),
            serde_json::to_vec_pretty(&json!({
                "candidate_selector": {"kind":SelectorKind::HtmlTag,"value":"article"},
                "candidate_accounting": {
                    "accepted":[{"candidate_id":"data/snapshots/events.html#0","record_index":0}],
                    "excluded":[]
                },
                "record_format": {"fields":[
                    {"name":"date","type":"string","normalizations":["japanese_date_to_iso"]},
                    {"name":"venue","type":"string","normalizations":["identity"]}
                ]}
            }))
            .unwrap(),
        )
        .unwrap();
        std::fs::write(
            dir.path().join(RECORDS_PATH),
            serde_json::to_vec_pretty(&records).unwrap(),
        )
        .unwrap();
        dir
    }
}
