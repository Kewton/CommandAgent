use super::NearestMiss;
use super::candidates::Candidate;

pub(super) fn nearest_miss(
    normalized_value: f64,
    printed_precision: u32,
    candidates: &[Candidate],
) -> Option<NearestMiss> {
    candidates
        .iter()
        .map(|candidate| {
            let rounded = round(candidate.value, printed_precision);
            NearestMiss {
                key: candidate.key.clone(),
                result_value: candidate.value,
                rounded_result_value: rounded,
                absolute_difference: (rounded - normalized_value).abs(),
            }
        })
        .min_by(|left, right| {
            left.absolute_difference
                .total_cmp(&right.absolute_difference)
        })
}

pub(super) fn round(value: f64, precision: u32) -> f64 {
    if precision > 15 {
        return value;
    }
    let scale = 10_f64.powi(precision as i32);
    (value * scale).round() / scale
}

pub(super) fn values_match(actual: f64, printed: f64, precision: u32) -> bool {
    let scale = 10_f64.powi(-(precision.min(15) as i32));
    let tolerance = (scale * 1e-9).max(f64::EPSILON * actual.abs().max(1.0) * 4.0);
    (actual - printed).abs() <= tolerance
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::profiles::data::checks::check_claims_binding;

    #[test]
    fn nearest_candidate_uses_the_same_printed_precision_as_binding() {
        let candidates = [
            Candidate::new("reconciliation.used_rows", 56.0),
            Candidate::new("reconciliation.input_rows", 60.0),
        ];

        assert_eq!(
            nearest_miss(61.0, 0, &candidates),
            Some(NearestMiss {
                key: "reconciliation.input_rows".to_string(),
                result_value: 60.0,
                rounded_result_value: 60.0,
                absolute_difference: 1.0,
            })
        );
    }

    #[test]
    fn evidence_binds_reconciliation_paths_and_records_a_violation_nearest_miss() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("output")).unwrap();
        std::fs::write(
            dir.path().join("output/results.json"),
            r#"{"reconciliation":{"input_rows":5,"used_rows":3,"excluded":[{"reason":"missing","rows":1},{"reason":"invalid","rows":1}]},"values":{"total":12.5}}"#,
        )
        .unwrap();
        std::fs::write(
            dir.path().join("output/report.md"),
            "Input 5; used 3; excluded 2; missing 1; stated input 6.",
        )
        .unwrap();

        let evidence = check_claims_binding(dir.path()).unwrap();

        assert!(!evidence.ok);
        assert_eq!(
            evidence
                .claims
                .iter()
                .filter_map(|claim| claim.matched_key.as_deref())
                .collect::<Vec<_>>(),
            [
                "reconciliation.input_rows",
                "reconciliation.used_rows",
                "reconciliation.excluded_rows_total",
                "reconciliation.excluded[0].rows",
            ]
        );
        let violation = evidence.claims.last().unwrap();
        assert!(!violation.ok);
        assert!(violation.matched_key.is_none());
        assert_eq!(
            violation.nearest_miss.as_ref().unwrap().key,
            "reconciliation.input_rows"
        );
        assert_eq!(
            violation.nearest_miss.as_ref().unwrap().absolute_difference,
            1.0
        );
    }
}
