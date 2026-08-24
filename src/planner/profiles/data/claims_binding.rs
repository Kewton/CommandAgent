use std::path::Path;
use std::sync::OnceLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use super::results_schema::ResultsDocument;

mod candidates;
mod comparison;
mod date_labels;
mod matching;

const MAX_CLAIMS: usize = 10_000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClaimBinding {
    pub report_path: String,
    pub byte_offset: usize,
    pub raw: String,
    #[serde(default = "date_labels::default_quantity")]
    pub claim_kind: String,
    pub normalized_value: f64,
    pub printed_precision: u32,
    pub percent: bool,
    pub unit: Option<String>,
    pub matched_key: Option<String>,
    pub matched_result_value: Option<f64>,
    pub rounded_result_value: Option<f64>,
    #[serde(default)]
    pub nearest_miss: Option<NearestMiss>,
    pub ok: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NearestMiss {
    pub key: String,
    pub result_value: f64,
    pub rounded_result_value: f64,
    pub absolute_difference: f64,
}

#[derive(Debug, Clone, PartialEq)]
struct ExtractedClaim {
    byte_offset: usize,
    raw: String,
    claim_kind: &'static str,
    normalized_value: f64,
    printed_precision: u32,
    percent: bool,
    unit: Option<String>,
}

pub fn bind_report_claims_to_results(
    report_path: &str,
    text: &str,
    results: &ResultsDocument,
) -> Vec<ClaimBinding> {
    bind_report_claims_to_candidates(report_path, text, &candidates::from_results(results))
}

fn bind_report_claims_to_candidates(
    report_path: &str,
    text: &str,
    candidates: &[candidates::Candidate],
) -> Vec<ClaimBinding> {
    let visible = report_visible_text(Path::new(report_path), text);
    extract_numeric_claims(&visible)
        .into_iter()
        .take(MAX_CLAIMS)
        .map(|claim| matching::bind_claim(report_path, claim, candidates))
        .collect()
}

pub fn claim_limit_exceeded(report_path: &str, text: &str) -> bool {
    let visible = report_visible_text(Path::new(report_path), text);
    numeric_claim_regex().find_iter(&visible).count() > MAX_CLAIMS
}

fn extract_numeric_claims(text: &str) -> Vec<ExtractedClaim> {
    let date_labels = date_labels::DateLabelSpans::in_text(text);
    numeric_claim_regex()
        .find_iter(text)
        .filter_map(|found| {
            let raw = found.as_str().trim();
            let percent = raw.ends_with('%') || raw.ends_with('％');
            let numeric = raw.trim_end_matches(['%', '％']).trim().replace(',', "");
            let printed_precision = numeric
                .split_once('.')
                .map_or(0, |(_, decimals)| decimals.len() as u32);
            let normalized_value = numeric.parse::<f64>().ok()?;
            Some(ExtractedClaim {
                byte_offset: found.start(),
                raw: raw.to_string(),
                claim_kind: date_labels.kind_for(found.start(), found.end()),
                normalized_value,
                printed_precision,
                percent,
                unit: (!percent).then(|| unit_after(text, found.end())).flatten(),
            })
        })
        .collect()
}

fn numeric_claim_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"[-+]?(?:[0-9]{1,3}(?:,[0-9]{3})+|[0-9]+)(?:\.[0-9]+)?(?:[ \t]*[%％])?")
            .expect("numeric claim regex")
    })
}

fn report_visible_text(path: &Path, text: &str) -> String {
    if path.extension().and_then(|extension| extension.to_str()) == Some("html") {
        html_visible_text(text)
    } else {
        markdown_visible_text(text)
    }
}

fn html_visible_text(text: &str) -> String {
    static SCRIPT_STYLE: OnceLock<Regex> = OnceLock::new();
    static TAGS: OnceLock<Regex> = OnceLock::new();
    let without_code = SCRIPT_STYLE
        .get_or_init(|| {
            Regex::new(r"(?is)<(?:script|style)\b[^>]*>.*?</(?:script|style)\s*>")
                .expect("script/style regex")
        })
        .replace_all(text, " ");
    let visible = TAGS
        .get_or_init(|| Regex::new(r"(?s)<[^>]*>").expect("HTML tag regex"))
        .replace_all(&without_code, " ");
    decode_common_entities(&visible)
}

fn markdown_visible_text(text: &str) -> String {
    static LINK_TARGET: OnceLock<Regex> = OnceLock::new();
    LINK_TARGET
        .get_or_init(|| Regex::new(r"\]\([^\r\n)]*\)").expect("Markdown link regex"))
        .replace_all(text, "]")
        .to_string()
}

fn decode_common_entities(text: &str) -> String {
    text.replace("&nbsp;", " ")
        .replace("&#160;", " ")
        .replace("&percnt;", "%")
        .replace("&#37;", "%")
        .replace("&minus;", "-")
}

fn unit_after(text: &str, end: usize) -> Option<String> {
    let suffix = text.get(end..)?.trim_start();
    let unit = suffix
        .chars()
        .take_while(|character| {
            character.is_alphabetic()
                || matches!(character, '円' | '件' | '人' | '個' | '秒' | '分' | '時')
        })
        .take(16)
        .collect::<String>();
    (!unit.is_empty()).then_some(unit)
}

#[cfg(test)]
mod tests;
