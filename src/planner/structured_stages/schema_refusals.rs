//! G2's refusals of a stage tool's calls, counted apart from G4's attempts (Issue #998).
//!
//! A call the schema refuses ends before G4 is asked: it spends no attempt, opens no record, and
//! starts no process. A model that keeps getting the shape wrong, a little differently each time,
//! would otherwise be stopped only by the session's iteration budget, one provider call each.
//! Each refusal counts against its target and against the run, with the limits the descriptor
//! gives G4's attempts, and the refusal that reaches either ends the run.
//!
//! The counts are in memory: a resume in a new process starts them over (a known limit). G4's
//! record and its counts are not touched.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Mutex, PoisonError};

use serde_json::{Value, json};

use super::StageRun;
use super::contract::{StageLimits, TargetKey};

/// The code a run ends with at a limit.
pub(crate) const SCHEMA_REFUSAL_LIMIT: &str = "schema_refusal_limit";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scope {
    Target,
    Run,
}

impl Scope {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Target => "target",
            Self::Run => "run",
        }
    }
}

/// The limit one refusal reached.
#[derive(Debug, Clone, PartialEq, Eq)]
struct LimitReached {
    key: TargetKey,
    refusals: u32,
    limit: u32,
    scope: Scope,
}

impl LimitReached {
    /// IDs and counts only, which admission limits to a few plain characters.
    fn event(&self) -> Value {
        json!({
            "event": "stage_schema_refusal_limit",
            "stage": self.key.stage,
            "target": self.key.target,
            "refusals": self.refusals,
            "limit": self.limit,
            "scope": self.scope.as_str(),
        })
    }
}

/// What a stage run's refusals have come to so far.
#[derive(Debug, Default)]
pub(crate) struct SchemaRefusals {
    counts: Mutex<Counts>,
}

#[derive(Debug, Default)]
struct Counts {
    by_target: BTreeMap<TargetKey, u32>,
    run: u32,
}

impl SchemaRefusals {
    /// Counts one refusal of the target's call, and says which limit it reached. The target's
    /// own is answered first.
    fn note(&self, key: &TargetKey, limits: StageLimits) -> Option<LimitReached> {
        let mut counts = self.counts.lock().unwrap_or_else(PoisonError::into_inner);
        let target = counts.by_target.entry(key.clone()).or_default();
        *target = target.saturating_add(1);
        let target = *target;
        counts.run = counts.run.saturating_add(1);
        let reached = |refusals, limit, scope| LimitReached {
            key: key.clone(),
            refusals,
            limit,
            scope,
        };
        if target >= limits.attempts_per_target {
            return Some(reached(target, limits.attempts_per_target, Scope::Target));
        }
        (counts.run >= limits.attempts_per_run)
            .then(|| reached(counts.run, limits.attempts_per_run, Scope::Run))
    }
}

impl StageRun {
    /// Counts a refusal G2 made of this tool's call. True when it reached a limit: its event is
    /// written, and the call is to end the run. A tool no target of the session's contract names,
    /// and a call outside a session, count nothing.
    pub(crate) fn note_schema_refusal(&self, tool_id: &str, events: Option<&Path>) -> bool {
        let contract = self
            .session
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .map(|session| session.contract.clone());
        let Some(contract) = contract else {
            return false;
        };
        let Some(target) = contract.target_for_tool(tool_id) else {
            return false;
        };
        let Some(reached) = self.schema_refusals.note(&target.key, contract.limits()) else {
            return false;
        };
        crate::eval_events::emit(events, reached.event());
        true
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Arc;

    use super::super::StageRun;
    use super::super::contract::{
        HostToolFacts, NarrowedStageLimits, StageContractDraft, StageDeclaration, StageLimits,
        StageRunFacts, StageSetDeclaration, TargetDeclaration, TargetKey,
    };
    use super::super::ledger::{LedgerEntry, LedgerRegistry};
    use super::{LimitReached, SchemaRefusals, Scope};

    fn key(stage: &str) -> TargetKey {
        TargetKey {
            stage: stage.to_string(),
            target: "main".to_string(),
        }
    }

    fn limits(attempts_per_target: u32, attempts_per_run: u32) -> StageLimits {
        StageLimits {
            attempts_per_target,
            attempts_per_run,
            deadline_seconds: 60,
        }
    }

    fn reached(stage: &str, refusals: u32, limit: u32, scope: Scope) -> Option<LimitReached> {
        Some(LimitReached {
            key: key(stage),
            refusals,
            limit,
            scope,
        })
    }

    #[test]
    fn the_refusal_that_reaches_the_targets_limit_is_the_one_that_ends() {
        let refusals = SchemaRefusals::default();
        let limits = limits(3, 64);
        assert_eq!(refusals.note(&key("upper"), limits), None);
        assert_eq!(refusals.note(&key("upper"), limits), None);
        assert_eq!(
            refusals.note(&key("upper"), limits),
            reached("upper", 3, 3, Scope::Target)
        );
    }

    #[test]
    fn a_limit_of_one_ends_at_the_first_refusal() {
        let refusals = SchemaRefusals::default();
        assert_eq!(
            refusals.note(&key("upper"), limits(1, 64)),
            reached("upper", 1, 1, Scope::Target)
        );
    }

    #[test]
    fn each_target_counts_its_own_and_the_run_counts_them_all() {
        let refusals = SchemaRefusals::default();
        let limits = limits(3, 4);
        assert_eq!(refusals.note(&key("upper"), limits), None);
        assert_eq!(refusals.note(&key("lower"), limits), None);
        assert_eq!(refusals.note(&key("upper"), limits), None);
        assert_eq!(
            refusals.note(&key("lower"), limits),
            reached("lower", 4, 4, Scope::Run)
        );
    }

    #[test]
    fn the_run_counts_every_target_up_to_its_limit_exactly() {
        let refusals = SchemaRefusals::default();
        let limits = limits(8, 5);
        for stage in ["a", "b", "c", "d"] {
            assert_eq!(refusals.note(&key(stage), limits), None, "{stage}");
        }
        assert_eq!(
            refusals.note(&key("e"), limits),
            reached("e", 5, 5, Scope::Run)
        );
    }

    #[test]
    fn a_target_at_its_own_limit_is_told_before_the_run() {
        let refusals = SchemaRefusals::default();
        let limits = limits(2, 2);
        assert_eq!(refusals.note(&key("upper"), limits), None);
        assert_eq!(
            refusals.note(&key("upper"), limits),
            reached("upper", 2, 2, Scope::Target)
        );
    }

    #[test]
    fn the_event_carries_the_target_its_count_the_limit_and_the_scope() {
        let event = reached("upper", 3, 3, Scope::Target).unwrap().event();
        assert_eq!(
            event,
            serde_json::json!({
                "event": "stage_schema_refusal_limit",
                "stage": "upper",
                "target": "main",
                "refusals": 3,
                "limit": 3,
                "scope": "target",
            })
        );
        assert_eq!(
            reached("lower", 4, 4, Scope::Run).unwrap().event()["scope"],
            "run"
        );
    }

    /// Two stages with one tool each, and the given limits.
    fn run_with(temp: &tempfile::TempDir, attempts: (u32, u32), bind: bool) -> StageRun {
        let root = std::fs::canonicalize(temp.path()).unwrap();
        let workspace = root.join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        let tools = ["upper", "lower"]
            .iter()
            .map(|stage| HostToolFacts {
                tool_id: format!("tool-{stage}"),
                output_id: "artifact".to_string(),
                output_path: format!("out/{stage}.json"),
                schema_inputs: Vec::new(),
                has_revision_argument: false,
                has_compiler: false,
                required_checks: vec!["shape".to_string()],
                limits: limits(attempts.0, attempts.1),
            })
            .collect::<Vec<_>>();
        let stages = ["upper", "lower"]
            .iter()
            .map(|stage| StageDeclaration {
                id: (*stage).to_string(),
                targets: vec![TargetDeclaration {
                    id: "main".to_string(),
                    tool_id: format!("tool-{stage}"),
                }],
                depends_on: Vec::new(),
                required_checks: Vec::new(),
            })
            .collect();
        let draft = StageContractDraft::new(
            &StageSetDeclaration {
                stages,
                limits: NarrowedStageLimits::default(),
            },
            &tools,
        )
        .unwrap();
        let run = StageRun::new(
            draft,
            BTreeMap::new(),
            workspace,
            "a-profile".to_string(),
            Arc::new(LedgerRegistry::default()),
        );
        if bind {
            run.bind_session(
                StageRunFacts::for_test(true, false),
                &root.join("state"),
                LedgerEntry::Fresh,
            )
            .unwrap();
        }
        run
    }

    /// A tool no target names counts nothing, toward its own or the run's limit.
    #[test]
    fn a_tool_no_target_names_counts_nothing() {
        let temp = tempfile::tempdir().unwrap();
        let run = run_with(&temp, (3, 3), true);
        for _ in 0..10 {
            assert!(!run.note_schema_refusal("tool-in-no-stage", None));
        }
        assert!(!run.note_schema_refusal("tool-upper", None));
        assert!(!run.note_schema_refusal("tool-lower", None));
        assert!(run.note_schema_refusal("tool-upper", None));
    }

    #[test]
    fn a_call_outside_a_session_counts_nothing() {
        let temp = tempfile::tempdir().unwrap();
        let run = run_with(&temp, (1, 2), false);
        for _ in 0..3 {
            assert!(!run.note_schema_refusal("tool-upper", None));
        }
    }

    /// The limits are the contract's, as the descriptor narrowed them.
    #[test]
    fn the_limits_are_the_contracts() {
        let temp = tempfile::tempdir().unwrap();
        let run = run_with(&temp, (2, 16), true);
        assert!(!run.note_schema_refusal("tool-upper", None));
        assert!(run.note_schema_refusal("tool-upper", None));
        assert!(!run.note_schema_refusal("tool-lower", None));
        assert!(run.note_schema_refusal("tool-lower", None));
    }
}
