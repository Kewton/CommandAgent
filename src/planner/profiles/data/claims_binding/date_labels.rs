use std::ops::Range;
use std::sync::OnceLock;

use regex::Regex;

pub(super) const QUANTITY: &str = "quantity";
pub(super) const DATE_LABEL: &str = "date_label";

pub(super) struct DateLabelSpans(Vec<Range<usize>>);

impl DateLabelSpans {
    pub(super) fn in_text(text: &str) -> Self {
        Self(
            date_label_regex()
                .find_iter(text)
                .filter(|found| has_non_digit_boundaries(text, found.start(), found.end()))
                .map(|found| found.range())
                .collect(),
        )
    }

    pub(super) fn kind_for(&self, start: usize, end: usize) -> &'static str {
        let index = self.0.partition_point(|span| span.end <= start);
        if self
            .0
            .get(index)
            .is_some_and(|span| span.start <= start && end <= span.end)
        {
            DATE_LABEL
        } else {
            QUANTITY
        }
    }
}

pub(super) fn default_quantity() -> String {
    QUANTITY.to_string()
}

fn date_label_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"[0-9]{4}-[0-9]{2}(?:-[0-9]{2})?").expect("ISO date label regex")
    })
}

fn has_non_digit_boundaries(text: &str, start: usize, end: usize) -> bool {
    let before = text
        .get(..start)
        .and_then(|prefix| prefix.chars().next_back());
    let after = text.get(end..).and_then(|suffix| suffix.chars().next());
    before.is_none_or(|character| !character.is_numeric())
        && after.is_none_or(|character| !character.is_numeric())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::profiles::data::checks::{
        CLAIMS_BINDING_EVIDENCE_PATH, check_claims_binding,
    };

    #[test]
    fn recognizes_month_and_day_spans_only_at_non_digit_boundaries() {
        let text = "2026-01 2026-01-31 12026-02 2026-031";
        let spans = DateLabelSpans::in_text(text);

        assert_eq!(spans.0, [0..7, 8..18]);
        assert_eq!(spans.kind_for(0, 4), DATE_LABEL);
        assert_eq!(spans.kind_for(4, 7), DATE_LABEL);
        assert_eq!(spans.kind_for(19, 23), QUANTITY);
    }

    #[test]
    fn evidence_audits_date_tokens_without_hiding_a_standalone_year_claim() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("output")).unwrap();
        std::fs::write(
            dir.path().join("output/results.json"),
            r#"{"reconciliation":{"input_rows":1,"used_rows":1,"excluded":[]},"values":{"year":2026}}"#,
        )
        .unwrap();
        std::fs::write(
            dir.path().join("output/report.md"),
            "Month 2026-07; day 2026-07-15; standalone year 2026.",
        )
        .unwrap();

        let evidence = check_claims_binding(dir.path()).unwrap();
        let claims = evidence.claims;

        assert_eq!(claims.len(), 6);
        assert_eq!(
            claims
                .iter()
                .filter(|claim| claim.claim_kind == DATE_LABEL)
                .count(),
            5
        );
        assert!(claims.iter().all(|claim| claim.ok));
        assert!(claims[..5].iter().all(|claim| claim.matched_key.is_none()));
        assert_eq!(claims[5].claim_kind, QUANTITY);
        assert_eq!(claims[5].matched_key.as_deref(), Some("year"));
        let json = std::fs::read_to_string(dir.path().join(CLAIMS_BINDING_EVIDENCE_PATH)).unwrap();
        assert!(json.contains(r#""claim_kind": "date_label""#));
    }
}
