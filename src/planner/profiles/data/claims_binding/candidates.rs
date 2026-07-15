use std::collections::BTreeMap;

use super::super::results_schema::ResultsDocument;

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Candidate {
    pub(super) key: String,
    pub(super) value: f64,
}

impl Candidate {
    #[cfg(test)]
    pub(super) fn new(key: &str, value: f64) -> Self {
        Self {
            key: key.to_string(),
            value,
        }
    }
}

pub(super) fn from_values(values: &BTreeMap<String, f64>) -> Vec<Candidate> {
    values
        .iter()
        .map(|(key, value)| Candidate {
            key: key.clone(),
            value: *value,
        })
        .collect()
}

pub(super) fn from_results(results: &ResultsDocument) -> Vec<Candidate> {
    let reconciliation = &results.reconciliation;
    let mut candidates = from_values(&results.values);
    candidates.push(row_candidate(
        "reconciliation.input_rows".to_string(),
        reconciliation.input_rows,
    ));
    candidates.push(row_candidate(
        "reconciliation.used_rows".to_string(),
        reconciliation.used_rows,
    ));
    for (index, excluded) in reconciliation.excluded.iter().enumerate() {
        candidates.push(row_candidate(
            format!("reconciliation.excluded[{index}].rows"),
            excluded.rows,
        ));
    }
    if let Some(total) = reconciliation
        .excluded
        .iter()
        .try_fold(0u64, |total, excluded| total.checked_add(excluded.rows))
    {
        candidates.push(row_candidate(
            "reconciliation.excluded_rows_total".to_string(),
            total,
        ));
    }
    candidates
}

fn row_candidate(key: String, value: u64) -> Candidate {
    Candidate {
        key,
        value: value as f64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::profiles::data::results_schema::{ExcludedRows, Reconciliation};

    #[test]
    fn results_candidates_include_values_and_fully_qualified_row_paths() {
        let results = ResultsDocument {
            reconciliation: Reconciliation {
                input_rows: 5,
                used_rows: 3,
                excluded: vec![
                    ExcludedRows {
                        reason: "missing".to_string(),
                        rows: 1,
                    },
                    ExcludedRows {
                        reason: "invalid".to_string(),
                        rows: 1,
                    },
                ],
            },
            values: BTreeMap::from([("total".to_string(), 12.5)]),
        };

        let candidates = from_results(&results);

        assert_eq!(
            candidates,
            [
                Candidate::new("total", 12.5),
                Candidate::new("reconciliation.input_rows", 5.0),
                Candidate::new("reconciliation.used_rows", 3.0),
                Candidate::new("reconciliation.excluded[0].rows", 1.0),
                Candidate::new("reconciliation.excluded[1].rows", 1.0),
                Candidate::new("reconciliation.excluded_rows_total", 2.0),
            ]
        );
    }
}
