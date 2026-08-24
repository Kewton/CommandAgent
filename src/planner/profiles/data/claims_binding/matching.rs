use super::candidates::Candidate;
use super::{ClaimBinding, ExtractedClaim, comparison, date_labels};

pub(super) fn bind_claim(
    report_path: &str,
    claim: ExtractedClaim,
    candidates: &[Candidate],
) -> ClaimBinding {
    let is_date_label = claim.claim_kind == date_labels::DATE_LABEL;
    let matched = (!is_date_label)
        .then(|| {
            candidates.iter().find_map(|candidate| {
                let rounded = comparison::round(candidate.value, claim.printed_precision);
                comparison::values_match(rounded, claim.normalized_value, claim.printed_precision)
                    .then_some((candidate, rounded))
            })
        })
        .flatten();
    let nearest_miss = (!is_date_label && matched.is_none())
        .then(|| {
            comparison::nearest_miss(claim.normalized_value, claim.printed_precision, candidates)
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
        matched_key: matched.map(|(candidate, _)| candidate.key.clone()),
        matched_result_value: matched.map(|(candidate, _)| candidate.value),
        rounded_result_value: matched.map(|(_, rounded)| rounded),
        nearest_miss,
        ok: is_date_label || matched.is_some(),
    }
}
