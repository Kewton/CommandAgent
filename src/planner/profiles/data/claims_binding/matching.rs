use std::collections::BTreeMap;

use super::{ClaimBinding, ExtractedClaim, date_labels};

pub(super) fn bind_claim(
    report_path: &str,
    claim: ExtractedClaim,
    values: &BTreeMap<String, f64>,
) -> ClaimBinding {
    let is_date_label = claim.claim_kind == date_labels::DATE_LABEL;
    let matched = (!is_date_label)
        .then(|| {
            values.iter().find_map(|(key, value)| {
                let rounded = round_to_printed_precision(*value, claim.printed_precision);
                values_match(rounded, claim.normalized_value, claim.printed_precision)
                    .then(|| (key.clone(), *value, rounded))
            })
        })
        .flatten();
    ClaimBinding {
        report_path: report_path.to_string(),
        byte_offset: claim.byte_offset,
        raw: claim.raw,
        claim_kind: claim.claim_kind.to_string(),
        normalized_value: claim.normalized_value,
        printed_precision: claim.printed_precision,
        percent: claim.percent,
        unit: claim.unit,
        matched_key: matched.as_ref().map(|(key, _, _)| key.clone()),
        matched_result_value: matched.as_ref().map(|(_, value, _)| *value),
        rounded_result_value: matched.as_ref().map(|(_, _, rounded)| *rounded),
        ok: is_date_label || matched.is_some(),
    }
}

fn round_to_printed_precision(value: f64, precision: u32) -> f64 {
    if precision > 15 {
        return value;
    }
    let scale = 10_f64.powi(precision as i32);
    (value * scale).round() / scale
}

fn values_match(actual: f64, printed: f64, precision: u32) -> bool {
    let scale = 10_f64.powi(-(precision.min(15) as i32));
    let tolerance = (scale * 1e-9).max(f64::EPSILON * actual.abs().max(1.0) * 4.0);
    (actual - printed).abs() <= tolerance
}
