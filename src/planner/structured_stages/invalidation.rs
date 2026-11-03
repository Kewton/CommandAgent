//! Which obligations an upstream change takes away (Issue #975, G4).
//!
//! The closure is computed from the admitted graph. A retired receipt moves to the target's
//! history, which no judgment reads: a digest that comes back does not bring it back (S4).

use std::collections::{BTreeMap, BTreeSet};

use super::contract::{StageContract, TargetKey};
use super::state::{Phase, TargetRecord};

/// Every stage that depends on `stage`, directly or not. `stage` itself is not in it.
pub(crate) fn downstream_closure(contract: &StageContract, stage: &str) -> BTreeSet<String> {
    let mut readers: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for admitted in contract.stages() {
        for upstream in &admitted.depends_on {
            readers
                .entry(upstream.as_str())
                .or_default()
                .push(admitted.id.as_str());
        }
    }
    let mut closure = BTreeSet::new();
    let mut waiting = vec![stage];
    while let Some(next) = waiting.pop() {
        for reader in readers.get(next).into_iter().flatten() {
            if closure.insert((*reader).to_string()) {
                waiting.push(reader);
            }
        }
    }
    closure
}

/// Takes the current state from everything below `stage` and gives each a new generation, so
/// an attempt started before this change can no longer finish. Attempts stay counted.
pub(crate) fn invalidate_downstream(
    contract: &StageContract,
    records: &mut [TargetRecord],
    stage: &str,
) -> Vec<TargetKey> {
    let closure = downstream_closure(contract, stage);
    let mut invalidated = Vec::new();
    for record in records {
        if !closure.contains(&record.key.stage) {
            continue;
        }
        record.retire_current_receipt();
        record.in_flight = None;
        record.phase = Phase::NotReady;
        record.unverified_reason = None;
        record.generation += 1;
        invalidated.push(record.key.clone());
    }
    invalidated
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{facts, one_target};
    use super::downstream_closure;
    use crate::planner::structured_stages::contract::{
        NarrowedStageLimits, StageContract, StageSetDeclaration, admit,
    };

    /// `root → left → join`, `root → right → join`, `join → tail`, and `apart` on its own.
    fn diamond() -> StageContract {
        let declaration = StageSetDeclaration {
            stages: vec![
                one_target("root", "t_root"),
                one_target("left", "t_left"),
                one_target("right", "t_right"),
                one_target("join", "t_join"),
                one_target("tail", "t_tail"),
                one_target("apart", "t_apart"),
            ],
            limits: NarrowedStageLimits::default(),
        };
        let tools = [
            facts("t_root", "out/root", &[]),
            facts("t_left", "out/left", &["out/root"]),
            facts("t_right", "out/right", &["out/root"]),
            facts("t_join", "out/join", &["out/left", "out/right"]),
            facts("t_tail", "out/tail", &["out/join"]),
            facts("t_apart", "out/apart", &[]),
        ];
        admit(&declaration, &tools).unwrap()
    }

    fn names(contract: &StageContract, stage: &str) -> Vec<String> {
        downstream_closure(contract, stage).into_iter().collect()
    }

    #[test]
    fn the_closure_reaches_every_level_and_every_branch() {
        let contract = diamond();
        assert_eq!(
            names(&contract, "root"),
            ["join", "left", "right", "tail"].map(String::from)
        );
    }

    #[test]
    fn the_closure_takes_only_what_lies_below() {
        let contract = diamond();
        assert_eq!(names(&contract, "left"), ["join", "tail"].map(String::from));
        assert_eq!(names(&contract, "join"), ["tail"].map(String::from));
        assert!(names(&contract, "tail").is_empty());
        assert!(names(&contract, "apart").is_empty());
        assert!(!names(&contract, "root").contains(&"apart".to_string()));
        assert!(!names(&contract, "root").contains(&"root".to_string()));
    }
}
