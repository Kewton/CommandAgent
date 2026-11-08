//! G2's refusals of a stage tool's calls, counted apart from G4's attempts (Issue #998).
//!
//! A call the schema refuses ends before G4 is asked: it spends no attempt, opens no record, and
//! starts no process. A model that keeps getting the shape wrong, a little differently each time,
//! would otherwise be stopped only by the session's iteration budget, one provider call each.
//! Each refusal counts against its target and against all the targets of the workspace and
//! profile (the event's `run` scope), with the limits the descriptor gives G4's attempts, and the
//! refusal that reaches either ends the run.
//!
//! The counts are the process's, kept as G4's record is: one set for each workspace and profile
//! of the process's ledger registry. They go on over sessions, phases, and final repairs, and
//! nothing starts them over, a success or a step forward included: once a limit is reached, a
//! later session of the process ends at its first refusal. They are in memory only: a new process,
//! a resume among them, starts them at zero (a known limit), and G4's record is not touched.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError, Weak};

use serde_json::{Value, json};

use super::StageRun;
use super::contract::{StageLimits, TargetKey};
use super::ledger::LedgerRegistry;

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

/// What the refusals of one workspace and profile in the process have come to so far.
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

    /// The counts of this workspace and profile in the process `ledgers` stands for, shared by
    /// every stage run of it, as its ledger is.
    pub(crate) fn of_process(
        ledgers: &Arc<LedgerRegistry>,
        workspace: &Path,
        profile: &str,
    ) -> Arc<Self> {
        static SHARED: Mutex<Shared> = Mutex::new(Shared { held: Vec::new() });
        SHARED
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .counts_for(ledgers, workspace, profile)
    }
}

/// One registry's counts, keyed as its ledgers are: by canonical workspace and profile.
type OfRegistry = BTreeMap<(PathBuf, String), Arc<SchemaRefusals>>;

/// Each live registry's counts.
#[derive(Debug, Default)]
struct Shared {
    held: Vec<(Weak<LedgerRegistry>, OfRegistry)>,
}

impl Shared {
    /// A registry dropped leaves its counts here until the next lookup, which drops them: none
    /// could be another's that came to the same address.
    fn counts_for(
        &mut self,
        ledgers: &Arc<LedgerRegistry>,
        workspace: &Path,
        profile: &str,
    ) -> Arc<SchemaRefusals> {
        self.held
            .retain(|(registry, _)| registry.strong_count() > 0);
        let index = match self
            .held
            .iter()
            .position(|(registry, _)| std::ptr::eq(registry.as_ptr(), Arc::as_ptr(ledgers)))
        {
            Some(index) => index,
            None => {
                self.held.push((Arc::downgrade(ledgers), BTreeMap::new()));
                self.held.len() - 1
            }
        };
        // As the ledger is keyed; a workspace not there to resolve is taken as given.
        let workspace = std::fs::canonicalize(workspace).unwrap_or_else(|_| workspace.into());
        self.held[index]
            .1
            .entry((workspace, profile.to_string()))
            .or_default()
            .clone()
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
    use std::path::Path;
    use std::sync::Arc;

    use super::super::StageRun;
    use super::super::contract::{
        HostToolFacts, NarrowedStageLimits, StageContractDraft, StageDeclaration, StageLimits,
        StageRunFacts, StageSetDeclaration, TargetDeclaration, TargetKey,
    };
    use super::super::ledger::{LedgerEntry, LedgerRegistry};
    use super::{LimitReached, SchemaRefusals, Scope, Shared};

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
        run_in(
            temp,
            attempts,
            bind,
            &Arc::new(LedgerRegistry::default()),
            "a-profile",
        )
    }

    /// The same, of a given process's registry and profile.
    fn run_in(
        temp: &tempfile::TempDir,
        attempts: (u32, u32),
        bind: bool,
        ledgers: &Arc<LedgerRegistry>,
        profile: &str,
    ) -> StageRun {
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
            profile.to_string(),
            ledgers.clone(),
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

    /// F-1: the next session's run, of the same process's registry, workspace, and profile, goes
    /// on from the counts the first left, as G4's record does.
    #[test]
    fn the_next_run_of_the_process_goes_on_from_the_targets_count() {
        let temp = tempfile::tempdir().unwrap();
        let ledgers = Arc::new(LedgerRegistry::default());
        let first = run_in(&temp, (3, 16), true, &ledgers, "a-profile");
        assert!(!first.note_schema_refusal("tool-upper", None));
        assert!(!first.note_schema_refusal("tool-upper", None));
        drop(first);
        let second = run_in(&temp, (3, 16), true, &ledgers, "a-profile");
        assert!(second.note_schema_refusal("tool-upper", None));
    }

    #[test]
    fn the_next_run_of_the_process_goes_on_from_the_runs_count() {
        let temp = tempfile::tempdir().unwrap();
        let ledgers = Arc::new(LedgerRegistry::default());
        let first = run_in(&temp, (8, 3), true, &ledgers, "a-profile");
        assert!(!first.note_schema_refusal("tool-upper", None));
        assert!(!first.note_schema_refusal("tool-lower", None));
        drop(first);
        let second = run_in(&temp, (8, 3), true, &ledgers, "a-profile");
        assert!(second.note_schema_refusal("tool-lower", None));
    }

    /// Another registry stands for another process, a resume among them: it starts over.
    #[test]
    fn another_process_starts_the_counts_over() {
        let temp = tempfile::tempdir().unwrap();
        let first = run_in(
            &temp,
            (3, 16),
            true,
            &Arc::new(LedgerRegistry::default()),
            "a-profile",
        );
        assert!(!first.note_schema_refusal("tool-upper", None));
        assert!(!first.note_schema_refusal("tool-upper", None));
        let second = run_in(
            &temp,
            (3, 16),
            true,
            &Arc::new(LedgerRegistry::default()),
            "a-profile",
        );
        assert!(!second.note_schema_refusal("tool-upper", None));
        assert!(!second.note_schema_refusal("tool-upper", None));
        assert!(second.note_schema_refusal("tool-upper", None));
    }

    /// The counts are kept as G4's records are: one for each workspace and profile.
    #[test]
    fn another_workspace_or_profile_counts_its_own() {
        let temp = tempfile::tempdir().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        let ledgers = Arc::new(LedgerRegistry::default());
        let first = run_in(&temp, (2, 16), true, &ledgers, "a-profile");
        assert!(!first.note_schema_refusal("tool-upper", None));
        let other_profile = run_in(&temp, (2, 16), true, &ledgers, "another-profile");
        assert!(!other_profile.note_schema_refusal("tool-upper", None));
        let other_workspace = run_in(&elsewhere, (2, 16), true, &ledgers, "a-profile");
        assert!(!other_workspace.note_schema_refusal("tool-upper", None));
        let again = run_in(&temp, (2, 16), true, &ledgers, "a-profile");
        assert!(again.note_schema_refusal("tool-upper", None));
    }

    /// A registry dropped takes its counts with it at the next lookup, so a new one that comes to
    /// the same address starts at zero.
    #[test]
    fn a_registry_dropped_leaves_no_counts_behind() {
        let mut shared = Shared::default();
        let workspace = Path::new("not-there/workspace");
        let gone = Arc::new(LedgerRegistry::default());
        let counts = shared.counts_for(&gone, workspace, "a-profile");
        assert_eq!(counts.note(&key("upper"), limits(2, 16)), None);
        drop(counts);
        drop(gone);
        let kept = Arc::new(LedgerRegistry::default());
        let counts = shared.counts_for(&kept, workspace, "a-profile");
        assert_eq!(shared.held.len(), 1);
        assert!(std::ptr::eq(shared.held[0].0.as_ptr(), Arc::as_ptr(&kept)));
        assert_eq!(counts.note(&key("upper"), limits(2, 16)), None);
        let same = shared.counts_for(&kept, workspace, "a-profile");
        assert!(Arc::ptr_eq(&counts, &same));
    }
}
