use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::Path;
use std::sync::OnceLock;

use anyhow::Context;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::accounting::{self, CandidateFreeze, FrozenCandidate};
use crate::planner::failure_vocabulary::ViolationId;

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
    DocumentYearContext,
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
pub struct BoundSourceFragment {
    pub source_path: String,
    pub byte_start: usize,
    pub byte_end: usize,
    pub raw_source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldBinding {
    pub record_index: usize,
    pub candidate_id: String,
    #[serde(default)]
    pub candidate_id_resolution: accounting::CandidateIdResolution,
    pub source_path: Option<String>,
    pub candidate_byte_start: Option<usize>,
    pub candidate_byte_end: Option<usize>,
    pub field: String,
    pub output_value: String,
    pub declared_normalizations: Vec<NormalizationRule>,
    pub raw_source: Option<String>,
    pub normalized_source: Option<String>,
    pub transformations: Vec<NormalizationRule>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_fragment: Option<BoundSourceFragment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document_context: Option<BoundSourceFragment>,
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
    candidate_relative_range: Option<(usize, usize)>,
    document_context: Option<BoundSourceFragment>,
}

struct FieldBindingInput<'a> {
    record_index: usize,
    candidate_id: &'a str,
    candidate_id_resolution: &'a accounting::CandidateIdResolution,
    field: &'a str,
    output_value: &'a str,
    rules: &'a [NormalizationRule],
    candidate: Option<&'a FrozenCandidate>,
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
        let candidate_id_resolution = accounting::resolve_candidate_id(&candidate_id, frozen);
        let candidate = candidate_id_resolution
            .resolved()
            .and_then(|resolved| candidates.get(resolved))
            .copied();
        for field in &format.fields {
            let Some(value) = record.get(&field.name).and_then(value_text) else {
                continue;
            };
            let binding = bind_field(
                root,
                FieldBindingInput {
                    record_index,
                    candidate_id: &candidate_id,
                    candidate_id_resolution: &candidate_id_resolution,
                    field: &field.name,
                    output_value: &value,
                    rules: &field.normalizations,
                    candidate,
                },
                &frozen.candidates,
            )?;
            if !binding.matched {
                failure_kinds.push(
                    ViolationId::source_binding(format!(
                        "record={record_index}:field={}:value={value}",
                        field.name
                    ))
                    .to_string(),
                );
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
            || (field
                .normalizations
                .contains(&NormalizationRule::DocumentYearContext)
                && !field
                    .normalizations
                    .contains(&NormalizationRule::JapaneseDateToIso))
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
    root: &Path,
    input: FieldBindingInput<'_>,
    frozen_candidates: &[FrozenCandidate],
) -> anyhow::Result<FieldBinding> {
    let FieldBindingInput {
        record_index,
        candidate_id,
        candidate_id_resolution,
        field,
        output_value,
        rules,
        candidate,
    } = input;
    let raw_candidate = candidate.map_or("", |candidate| candidate.raw.as_str());
    let mut values = source_values(raw_candidate, rules);
    if let Some(candidate) = candidate {
        values.extend(document_context_values(
            root,
            candidate,
            frozen_candidates,
            rules,
        )?);
    }
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
    let candidate_fragment = matched
        .and_then(|value| value.candidate_relative_range)
        .zip(candidate)
        .map(|((start, end), candidate)| BoundSourceFragment {
            source_path: candidate.source_path.clone(),
            byte_start: candidate.byte_start + start,
            byte_end: candidate.byte_start + end,
            raw_source: raw_candidate[start..end].to_string(),
        });
    Ok(FieldBinding {
        record_index,
        candidate_id: candidate_id.to_string(),
        candidate_id_resolution: candidate_id_resolution.clone(),
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
        candidate_fragment,
        document_context: matched.and_then(|candidate| candidate.document_context.clone()),
        matched: matched.is_some(),
        nearest_miss,
    })
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
                candidate_relative_range: None,
                document_context: None,
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
            candidate_relative_range: None,
            document_context: None,
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
            candidate_relative_range: None,
            document_context: None,
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
                candidate_relative_range: None,
                document_context: None,
            })
        })
        .collect()
}

fn document_context_values(
    root: &Path,
    candidate: &FrozenCandidate,
    frozen_candidates: &[FrozenCandidate],
    rules: &[NormalizationRule],
) -> anyhow::Result<Vec<SourceValue>> {
    if !rules.contains(&NormalizationRule::JapaneseDateToIso)
        || !rules.contains(&NormalizationRule::DocumentYearContext)
    {
        return Ok(Vec::new());
    }
    let path = crate::tools::path_guard::resolve_existing(root, &candidate.source_path)
        .context("source_binding_violation:document_context_path")?;
    let document = std::fs::read_to_string(path)
        .context("source_binding_violation:document_context_unreadable")?;
    let years = shared_document_years(&document, &candidate.source_path, frozen_candidates);
    if years.len() != 1 {
        return Ok(Vec::new());
    }
    let (year, context) = years.into_iter().next().expect("one checked year");
    Ok(partial_date_regex()
        .captures_iter(&candidate.raw)
        .filter_map(|captures| {
            let whole = captures.get(0)?;
            if candidate_prefix_has_year(&candidate.raw[..whole.start()]) {
                return None;
            }
            let month = ascii_digits(captures.get(1)?.as_str())
                .parse::<u32>()
                .ok()?;
            let day = ascii_digits(captures.get(2)?.as_str())
                .parse::<u32>()
                .ok()?;
            valid_date(year, month, day).then(|| SourceValue {
                raw: whole.as_str().to_string(),
                normalized: format!("{year:04}-{month:02}-{day:02}"),
                transformations: vec![
                    NormalizationRule::JapaneseDateToIso,
                    NormalizationRule::DocumentYearContext,
                ],
                candidate_relative_range: Some((whole.start(), whole.end())),
                document_context: Some(context.clone()),
            })
        })
        .collect())
}

fn shared_document_years(
    document: &str,
    source_path: &str,
    frozen_candidates: &[FrozenCandidate],
) -> BTreeMap<u32, BoundSourceFragment> {
    let candidate_ranges = frozen_candidates
        .iter()
        .filter(|candidate| candidate.source_path == source_path)
        .map(|candidate| (candidate.byte_start, candidate.byte_end))
        .collect::<Vec<_>>();
    let mut context_ranges = Vec::new();
    for regex in [title_context_regex(), heading_context_regex()] {
        for captures in regex.captures_iter(document) {
            let Some(whole) = captures.get(0) else {
                continue;
            };
            let Some(content) = captures.get(1) else {
                continue;
            };
            let overlaps_candidate = candidate_ranges
                .iter()
                .any(|(start, end)| whole.start() < *end && whole.end() > *start);
            if !overlaps_candidate {
                context_ranges.push((content.start(), content.end()));
            }
        }
    }
    context_ranges.sort();

    let mut years = BTreeMap::new();
    for (start, end) in context_ranges {
        let context = &document[start..end];
        for captures in context_year_regex().captures_iter(context) {
            let Some(whole) = captures.get(0) else {
                continue;
            };
            let Some(year) = captures
                .get(1)
                .and_then(|value| ascii_digits(value.as_str()).parse::<u32>().ok())
                .filter(|year| (1..=9999).contains(year))
            else {
                continue;
            };
            let byte_start = start + whole.start();
            let byte_end = start + whole.end();
            years.entry(year).or_insert_with(|| BoundSourceFragment {
                source_path: source_path.to_string(),
                byte_start,
                byte_end,
                raw_source: document[byte_start..byte_end].to_string(),
            });
        }
    }
    years
}

fn partial_date_regex() -> &'static Regex {
    static PARTIAL_DATE: OnceLock<Regex> = OnceLock::new();
    PARTIAL_DATE.get_or_init(|| {
        Regex::new(r"([0-9０-９]{1,2})\s*(?:/|月)\s*([0-9０-９]{1,2})(?:日)?(?:\s*\([^)]*\))?")
            .expect("static partial date regex")
    })
}

fn candidate_prefix_has_year(prefix: &str) -> bool {
    static YEAR_PREFIX: OnceLock<Regex> = OnceLock::new();
    YEAR_PREFIX
        .get_or_init(|| {
            Regex::new(
                r"(?:(?:[0-9０-９]{4})|(?:(?:令和|平成|昭和)\s*(?:元|[0-9０-９]{1,2})))年\s*$",
            )
            .expect("static candidate year-prefix regex")
        })
        .is_match(prefix)
}

fn title_context_regex() -> &'static Regex {
    static TITLE: OnceLock<Regex> = OnceLock::new();
    TITLE.get_or_init(|| {
        Regex::new(r"(?is)<title\b[^>]*>(.*?)</title\s*>").expect("static title context regex")
    })
}

fn heading_context_regex() -> &'static Regex {
    static HEADING: OnceLock<Regex> = OnceLock::new();
    HEADING.get_or_init(|| {
        Regex::new(r"(?is)<h[1-6]\b[^>]*>(.*?)</h[1-6]\s*>").expect("static heading context regex")
    })
}

fn context_year_regex() -> &'static Regex {
    static YEAR: OnceLock<Regex> = OnceLock::new();
    YEAR.get_or_init(|| {
        Regex::new(r"([0-9０-９]{4})年").expect("static document context year regex")
    })
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
    use serde::Deserialize;
    use serde_json::json;

    const ELEV_006_CONTEXT_FIXTURE: &str = include_str!(
        "../../../../tests/fixtures/ingest-source-binding/elev-006-document-year-context.json"
    );
    const ELEV_007_ID_FIXTURE: &str = include_str!(
        "../../../../tests/fixtures/ingest-source-binding/elev-007-candidate-id-resolution.json"
    );
    const ELEV_006_LIST_SNAPSHOT: &str = include_str!(
        "../../../../workspace/management/bench/assets/ingest/list/data/snapshots/events-list.html"
    );

    #[derive(Deserialize)]
    struct MeasuredContextFixture {
        selector: String,
        candidate_id: String,
        candidate_fragment: String,
        context_fragment: String,
        output_value: String,
        declared_normalizations: Vec<NormalizationRule>,
    }

    #[derive(Deserialize)]
    struct MeasuredIdFixture {
        campaign: String,
        selector: String,
        observed_runs: usize,
        observed_n2_date_bindings: usize,
        observed_n2_field_violations: usize,
        provided_accepted_id: String,
        canonical_accepted_id: String,
        provided_excluded_ids: Vec<String>,
        output_value: String,
        candidate_fragment: String,
        context_fragment: String,
        declared_normalizations: Vec<NormalizationRule>,
        expected_resolution: accounting::ResolutionStatus,
        ambiguous_probe: AmbiguousIdProbe,
        false_probe: String,
    }

    #[derive(Deserialize)]
    struct AmbiguousIdProbe {
        provided_id: String,
        frozen_ids: Vec<String>,
    }

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
    fn elev_006_document_year_completion_records_both_source_fragments() {
        let (dir, fixture) = measured_document_context_fixture();
        let frozen = accounting::freeze(dir.path()).unwrap();

        let evidence = check(dir.path(), &frozen).unwrap();

        assert!(evidence.ok, "{evidence:?}");
        let date = &evidence.bindings[0];
        assert_eq!(date.raw_source.as_deref(), Some("8/3(月)"));
        assert_eq!(date.normalized_source.as_deref(), Some("2026-08-03"));
        assert_eq!(
            date.transformations,
            [
                NormalizationRule::JapaneseDateToIso,
                NormalizationRule::DocumentYearContext
            ]
        );
        let candidate_fragment = date.candidate_fragment.as_ref().unwrap();
        let document_context = date.document_context.as_ref().unwrap();
        assert_eq!(candidate_fragment.raw_source, fixture.candidate_fragment);
        assert_eq!(document_context.raw_source, fixture.context_fragment);
        let document =
            std::fs::read_to_string(dir.path().join(&candidate_fragment.source_path)).unwrap();
        assert_eq!(
            &document[candidate_fragment.byte_start..candidate_fragment.byte_end],
            fixture.candidate_fragment
        );
        assert_eq!(
            &document[document_context.byte_start..document_context.byte_end],
            fixture.context_fragment
        );
    }

    #[test]
    fn elev_007_missing_prefix_resolves_and_records_binding_and_accounting_lineage() {
        let fixture: MeasuredIdFixture = serde_json::from_str(ELEV_007_ID_FIXTURE).unwrap();
        assert_eq!(fixture.campaign, "uat-test0726-ingest-elev-007");
        assert_eq!(fixture.observed_runs, 6);
        assert_eq!(fixture.observed_n2_date_bindings, 54);
        assert_eq!(fixture.observed_n2_field_violations, 216);
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("data/snapshots")).unwrap();
        std::fs::create_dir_all(dir.path().join("output")).unwrap();
        std::fs::write(
            dir.path().join("data/snapshots/events-list.html"),
            ELEV_006_LIST_SNAPSHOT,
        )
        .unwrap();
        let excluded = fixture
            .provided_excluded_ids
            .iter()
            .map(|candidate_id| {
                json!({"candidate_id":candidate_id,"reason":"not selected in measured fixture"})
            })
            .collect::<Vec<_>>();
        std::fs::write(
            dir.path().join(accounting::INSPECTION_PATH),
            serde_json::to_vec_pretty(&json!({
                "candidate_selector": {"kind":"css","value":fixture.selector},
                "candidate_accounting": {
                    "accepted":[{
                        "candidate_id":fixture.provided_accepted_id,
                        "record_index":0
                    }],
                    "excluded":excluded
                },
                "record_format": {"fields":[{
                    "name":"date",
                    "type":"string",
                    "normalizations":fixture.declared_normalizations
                }]}
            }))
            .unwrap(),
        )
        .unwrap();
        std::fs::write(
            dir.path().join(RECORDS_PATH),
            serde_json::to_vec_pretty(&json!([{"date":fixture.output_value}])).unwrap(),
        )
        .unwrap();
        let frozen = accounting::freeze(dir.path()).unwrap();

        let source_binding = check(dir.path(), &frozen).unwrap();
        let candidate_accounting = accounting::check(dir.path(), &frozen).unwrap();

        assert!(source_binding.ok, "{source_binding:?}");
        assert!(candidate_accounting.ok, "{candidate_accounting:?}");
        let date = &source_binding.bindings[0];
        assert_eq!(date.candidate_id, "events-list.html#1");
        assert_eq!(
            date.candidate_id_resolution.status,
            fixture.expected_resolution
        );
        assert_eq!(
            date.candidate_id_resolution.resolved(),
            Some(fixture.canonical_accepted_id.as_str())
        );
        assert_eq!(
            date.candidate_fragment.as_ref().unwrap().raw_source,
            fixture.candidate_fragment
        );
        assert_eq!(
            date.document_context.as_ref().unwrap().raw_source,
            fixture.context_fragment
        );
        assert_eq!(candidate_accounting.candidate_id_resolutions.len(), 10);
        assert!(
            candidate_accounting
                .candidate_id_resolutions
                .iter()
                .all(|resolution| {
                    resolution.status == accounting::ResolutionStatus::UniqueSuffix
                        && resolution.resolved_id.is_some()
                })
        );
    }

    #[test]
    fn elev_007_fixture_ambiguous_and_false_suffixes_remain_violations() {
        let fixture: MeasuredIdFixture = serde_json::from_str(ELEV_007_ID_FIXTURE).unwrap();
        let frozen = CandidateFreeze {
            capability_id: "ingest_candidate_freeze".to_string(),
            selector: accounting::CandidateSelector {
                kind: SelectorKind::Css,
                value: "article.event".to_string(),
            },
            record_format: json!({}),
            snapshots: Vec::new(),
            candidates: fixture
                .ambiguous_probe
                .frozen_ids
                .iter()
                .enumerate()
                .map(|(ordinal, id)| FrozenCandidate {
                    id: id.clone(),
                    source_path: id.split('#').next().unwrap().to_string(),
                    ordinal,
                    byte_start: 0,
                    byte_end: 0,
                    fnv1a64: format!("{ordinal:016x}"),
                    raw: String::new(),
                })
                .collect(),
        };

        let ambiguous =
            accounting::resolve_candidate_id(&fixture.ambiguous_probe.provided_id, &frozen);
        let false_id = accounting::resolve_candidate_id(&fixture.false_probe, &frozen);

        assert_eq!(
            ambiguous.status,
            accounting::ResolutionStatus::AmbiguousSuffix
        );
        assert_eq!(ambiguous.matched_ids.len(), 2);
        assert!(ambiguous.resolved_id.is_none());
        assert_eq!(false_id.status, accounting::ResolutionStatus::NotFound);
        assert!(false_id.matched_ids.is_empty());
        assert!(false_id.resolved_id.is_none());
    }

    #[test]
    fn document_context_does_not_turn_a_shifted_date_into_a_match() {
        let (dir, _) = measured_document_context_fixture();
        std::fs::write(
            dir.path().join(RECORDS_PATH),
            serde_json::to_vec_pretty(&json!([{"date":"2026-08-04"}])).unwrap(),
        )
        .unwrap();

        let evidence = check(dir.path(), &accounting::freeze(dir.path()).unwrap()).unwrap();

        assert!(!evidence.ok);
        let miss = evidence.bindings[0].nearest_miss.as_ref().unwrap();
        assert_eq!(miss.raw_source, "8/3(月)");
        assert_eq!(miss.normalized_source, "2026-08-03");
    }

    #[test]
    fn document_context_never_joins_a_field_from_another_candidate() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("data/snapshots")).unwrap();
        std::fs::create_dir_all(dir.path().join("output")).unwrap();
        std::fs::write(
            dir.path().join("data/snapshots/events.html"),
            "<title>2026年行事</title><article><h2>第一行事</h2></article>\
             <article><h2>第二行事</h2><p>第二会場</p></article>",
        )
        .unwrap();
        std::fs::write(
            dir.path().join(accounting::INSPECTION_PATH),
            serde_json::to_vec_pretty(&json!({
                "candidate_selector": {"kind":"html_tag","value":"article"},
                "candidate_accounting": {
                    "accepted":[{"candidate_id":"data/snapshots/events.html#0","record_index":0}],
                    "excluded":[{"candidate_id":"data/snapshots/events.html#1","reason":"not selected"}]
                },
                "record_format": {"fields":[{
                    "name":"venue","type":"string","normalizations":["identity"]
                }]}
            }))
            .unwrap(),
        )
        .unwrap();
        std::fs::write(
            dir.path().join(RECORDS_PATH),
            serde_json::to_vec_pretty(&json!([{"venue":"第二会場"}])).unwrap(),
        )
        .unwrap();

        let evidence = check(dir.path(), &accounting::freeze(dir.path()).unwrap()).unwrap();

        assert!(!evidence.ok);
        assert!(!evidence.bindings[0].matched);
        assert!(evidence.bindings[0].document_context.is_none());
    }

    #[test]
    fn document_year_context_requires_the_declared_date_conversion() {
        let format = RecordFormat {
            fields: vec![FieldDeclaration {
                name: "date".to_string(),
                field_type: FieldType::String,
                normalizations: vec![NormalizationRule::DocumentYearContext],
            }],
        };

        assert!(
            validate_format(&format)
                .unwrap_err()
                .to_string()
                .contains("source_binding_violation:field_declaration")
        );
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

    fn measured_document_context_fixture() -> (tempfile::TempDir, MeasuredContextFixture) {
        let fixture: MeasuredContextFixture =
            serde_json::from_str(ELEV_006_CONTEXT_FIXTURE).unwrap();
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("data/snapshots")).unwrap();
        std::fs::create_dir_all(dir.path().join("output")).unwrap();
        std::fs::write(
            dir.path().join("data/snapshots/events-list.html"),
            ELEV_006_LIST_SNAPSHOT,
        )
        .unwrap();
        std::fs::write(
            dir.path().join(accounting::INSPECTION_PATH),
            serde_json::to_vec_pretty(&json!({
                "candidate_selector": {"kind":"css","value":fixture.selector},
                "candidate_accounting": {
                    "accepted":[{"candidate_id":fixture.candidate_id,"record_index":0}],
                    "excluded":[]
                },
                "record_format": {"fields":[{
                    "name":"date",
                    "type":"string",
                    "normalizations":fixture.declared_normalizations
                }]}
            }))
            .unwrap(),
        )
        .unwrap();
        std::fs::write(
            dir.path().join(RECORDS_PATH),
            serde_json::to_vec_pretty(&json!([{"date":fixture.output_value}])).unwrap(),
        )
        .unwrap();
        (dir, fixture)
    }
}
