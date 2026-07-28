use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionStatus {
    Exact,
    UniqueSuffix,
    AmbiguousSuffix,
    NotFound,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateIdResolution {
    pub provided_id: String,
    pub status: ResolutionStatus,
    pub matched_ids: Vec<String>,
    pub resolved_id: Option<String>,
}

impl Default for CandidateIdResolution {
    fn default() -> Self {
        Self {
            provided_id: String::new(),
            status: ResolutionStatus::NotFound,
            matched_ids: Vec::new(),
            resolved_id: None,
        }
    }
}

impl CandidateIdResolution {
    pub fn resolved(&self) -> Option<&str> {
        self.resolved_id.as_deref()
    }
}

pub fn resolve(provided_id: &str, known: &BTreeSet<&str>) -> CandidateIdResolution {
    if known.contains(provided_id) {
        return CandidateIdResolution {
            provided_id: provided_id.to_string(),
            status: ResolutionStatus::Exact,
            matched_ids: vec![provided_id.to_string()],
            resolved_id: Some(provided_id.to_string()),
        };
    }

    let suffix = format!("/{provided_id}");
    let matched_ids = known
        .iter()
        .filter(|candidate| candidate.ends_with(&suffix))
        .map(|candidate| (*candidate).to_string())
        .collect::<Vec<_>>();
    let (status, resolved_id) = match matched_ids.as_slice() {
        [only] => (ResolutionStatus::UniqueSuffix, Some(only.clone())),
        [] => (ResolutionStatus::NotFound, None),
        _ => (ResolutionStatus::AmbiguousSuffix, None),
    };
    CandidateIdResolution {
        provided_id: provided_id.to_string(),
        status,
        matched_ids,
        resolved_id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_and_unique_suffix_resolve_but_ambiguous_and_false_ids_do_not() {
        let known = [
            "data/snapshots/a/events.html#0",
            "data/snapshots/b/events.html#0",
            "data/snapshots/events-list.html#1",
        ]
        .into_iter()
        .collect();

        let exact = resolve("data/snapshots/events-list.html#1", &known);
        assert_eq!(exact.status, ResolutionStatus::Exact);
        assert_eq!(exact.resolved(), Some("data/snapshots/events-list.html#1"));

        let unique = resolve("events-list.html#1", &known);
        assert_eq!(unique.status, ResolutionStatus::UniqueSuffix);
        assert_eq!(unique.resolved(), Some("data/snapshots/events-list.html#1"));

        let ambiguous = resolve("events.html#0", &known);
        assert_eq!(ambiguous.status, ResolutionStatus::AmbiguousSuffix);
        assert_eq!(ambiguous.matched_ids.len(), 2);
        assert_eq!(ambiguous.resolved(), None);

        let false_id = resolve("invented.html#9", &known);
        assert_eq!(false_id.status, ResolutionStatus::NotFound);
        assert!(false_id.matched_ids.is_empty());
        assert_eq!(false_id.resolved(), None);
    }
}
