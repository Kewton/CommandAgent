//! What a session's stage set says about the session ending (Issue #975, G4).
//!
//! The loop has many ways to stop. [`StageHold`] sends a model back while it can still do
//! something about an unmet obligation, a bounded number of times. [`StageFunnel`] stands at the
//! one place every way out passes, and turns a success with an unmet obligation into a refusal.
//! Neither reads what a model or a tool said: the host's ledger decides. Without a stage
//! declaration there is no stage run, and both do nothing.

use std::cell::{Cell, RefCell};
use std::sync::Arc;

use crate::config::Config;
use crate::planner::structured_stages::{
    StageNote, StageRun, StageScope, StageShortfall, refuse_unmet_session,
};

use super::super::context::{RunSessionOptions, RunSessionScope};

/// How many times one session is sent back. The session's own budgets may stop it sooner.
const MAX_STAGE_HOLDS: usize = 8;

pub(in super::super) const STAGE_HOLD_PREFIX: &str = "Stage obligations are not met:";

thread_local! {
    /// The stage run of the session on this thread. The funnel stands outside the frame that
    /// owns the session's registry, so it finds the run here.
    static CURRENT: RefCell<Option<Arc<StageRun>>> = const { RefCell::new(None) };
}

fn scope_of(options: &RunSessionOptions) -> StageScope {
    if options.scope == RunSessionScope::PlanRunStep {
        StageScope::Touched
    } else {
        StageScope::All
    }
}

/// Called once a session has bound its stage set.
pub(in super::super) fn enter_stage_run(run: Arc<StageRun>) {
    CURRENT.with(|current| *current.borrow_mut() = Some(run));
}

fn current_stage_run() -> Option<Arc<StageRun>> {
    CURRENT.with(|current| current.borrow().clone())
}

/// Around one session. A nested session puts the outer one's run back when it ends.
pub(in super::super) struct StageFunnel {
    previous: Option<Arc<StageRun>>,
    scope: StageScope,
}

impl Drop for StageFunnel {
    fn drop(&mut self) {
        let previous = self.previous.take();
        CURRENT.with(|current| *current.borrow_mut() = previous);
    }
}

impl StageFunnel {
    pub(in super::super) fn enter(options: &RunSessionOptions) -> Self {
        Self {
            previous: CURRENT.with(|current| current.borrow_mut().take()),
            scope: scope_of(options),
        }
    }

    pub(in super::super) fn finish<T>(
        self,
        result: anyhow::Result<T>,
        config: &Config,
    ) -> anyhow::Result<T> {
        let run = current_stage_run();
        // G5 before G4 (Issue #976): a command whose wallet refused a send is no success.
        let result = crate::structured_budget::outcome::gate_session(result, config);
        refuse_unmet_session(
            result,
            run.as_deref(),
            self.scope,
            config.eval_events_path.as_deref(),
        )
    }
}

/// Sends a model back while another submission can still change the answer.
#[derive(Default)]
pub(in super::super) struct StageHold {
    run: Option<Arc<StageRun>>,
    scope: StageScope,
    holds: Cell<usize>,
    /// This batch's answers included one a model has to read to go on.
    unread_answer: Cell<bool>,
}

impl StageHold {
    pub(super) fn capture(options: &RunSessionOptions) -> Self {
        Self {
            run: current_stage_run(),
            scope: scope_of(options),
            ..Self::default()
        }
    }

    fn hold_once(&self) -> bool {
        if self.holds.get() >= MAX_STAGE_HOLDS {
            return false;
        }
        self.holds.set(self.holds.get() + 1);
        true
    }

    fn shortfall(&self) -> Option<StageShortfall> {
        self.run.as_ref()?.judge(self.scope)
    }

    /// Why a session that wants to end may not, with what the model can do about it. A shortfall
    /// no submission can mend, and one past the limit, go on to the funnel, which ends the run.
    pub(super) fn feedback(&self) -> Option<String> {
        let shortfall = self.shortfall()?;
        if !shortfall.can_be_resubmitted() || !self.hold_once() {
            return None;
        }
        let unmet = shortfall
            .unmet
            .iter()
            .map(|obligation| {
                format!(
                    "{}/{} at revision {}",
                    obligation.key.stage,
                    obligation.key.target,
                    obligation.revision.as_deref().unwrap_or("unknown"),
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        Some(format!(
            "{STAGE_HOLD_PREFIX} {unmet}. Only the host's record of an accepted submission meets one; saying it is done does not. Call the tool of each one with its revision."
        ))
    }

    /// Whether a step that could end at a tool result must go back to its model instead.
    pub(super) fn holds_tool_only_completion(&self) -> bool {
        if self.run.is_none() {
            return false;
        }
        let unread = self.unread_answer.replace(false);
        let unmet = self.shortfall().is_some_and(|s| s.can_be_resubmitted());
        (unread || unmet) && self.hold_once()
    }

    /// Whether an obligation is open now. Asking does not spend a hold.
    pub(super) fn has_unmet_obligation(&self) -> bool {
        self.run.is_some() && self.shortfall().is_some()
    }

    /// Takes in what the last call did, and says whether a stage moved.
    pub(super) fn absorb_notes(&self, notes: Vec<StageNote>) -> bool {
        if notes.iter().any(|note| !note.progress) {
            self.unread_answer.set(true);
        }
        notes.iter().any(|note| note.progress)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Arc;

    use super::{MAX_STAGE_HOLDS, STAGE_HOLD_PREFIX, StageFunnel, StageHold, enter_stage_run};
    use crate::minimal_loop::loop_run::context::RunSessionOptions;
    use crate::planner::structured_stages::contract::{
        HostToolFacts, NarrowedStageLimits, StageContractDraft, StageDeclaration, StageLimits,
        StageRunFacts, StageSetDeclaration, TargetDeclaration, TargetKey,
    };
    use crate::planner::structured_stages::ledger::{LedgerEntry, LedgerRegistry};
    use crate::planner::structured_stages::{StageNote, StageRun, StageScope};

    /// One bound stage nobody has submitted, with no process behind it.
    fn bound_run(temp: &tempfile::TempDir) -> Arc<StageRun> {
        let root = std::fs::canonicalize(temp.path()).unwrap();
        let workspace = root.join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        let draft = StageContractDraft::new(
            &StageSetDeclaration {
                stages: vec![StageDeclaration {
                    id: "only".to_string(),
                    targets: vec![TargetDeclaration {
                        id: "main".to_string(),
                        tool_id: "a-tool".to_string(),
                    }],
                    depends_on: Vec::new(),
                    required_checks: Vec::new(),
                }],
                limits: NarrowedStageLimits::default(),
            },
            &[HostToolFacts {
                tool_id: "a-tool".to_string(),
                output_id: "artifact".to_string(),
                output_path: "out/a.json".to_string(),
                schema_inputs: Vec::new(),
                has_revision_argument: false,
                has_compiler: false,
                required_checks: vec!["shape".to_string()],
                limits: StageLimits {
                    attempts_per_target: 2,
                    attempts_per_run: 2,
                    deadline_seconds: 60,
                },
            }],
        )
        .unwrap();
        let run = Arc::new(StageRun::new(
            draft,
            BTreeMap::new(),
            workspace,
            "a-profile".to_string(),
            Arc::new(LedgerRegistry::default()),
        ));
        run.bind_session(
            StageRunFacts::for_test(true, false),
            &root.join("state"),
            LedgerEntry::Fresh,
        )
        .unwrap();
        run
    }

    fn hold_over(run: Arc<StageRun>, scope: StageScope) -> StageHold {
        StageHold {
            run: Some(run),
            scope,
            ..StageHold::default()
        }
    }

    fn only() -> TargetKey {
        TargetKey {
            stage: "only".to_string(),
            target: "main".to_string(),
        }
    }

    #[test]
    fn the_hold_sends_a_model_back_a_bounded_number_of_times_and_then_lets_the_funnel_decide() {
        let temp = tempfile::tempdir().unwrap();
        let hold = hold_over(bound_run(&temp), StageScope::All);
        for _ in 0..MAX_STAGE_HOLDS {
            let feedback = hold.feedback().expect("held");
            assert!(feedback.starts_with(STAGE_HOLD_PREFIX), "{feedback}");
            assert!(feedback.contains("only/main"), "{feedback}");
        }
        assert_eq!(hold.feedback(), None, "past the limit");
        assert!(hold.has_unmet_obligation(), "it is still not met");
    }

    /// M9: no submission can mend a record that cannot be vouched for, so nothing is held. The
    /// funnel ends the run, and no hold is spent on the way.
    #[test]
    fn a_shortfall_no_submission_can_mend_sends_nobody_back() {
        let temp = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(temp.path()).unwrap();
        let file = crate::planner::structured_stages::ledger::ledger_file(
            &root.join("state"),
            &{
                std::fs::create_dir_all(root.join("workspace")).unwrap();
                root.join("workspace")
            },
            "a-profile",
        )
        .unwrap();
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, b"not a record").unwrap();
        let hold = hold_over(bound_run(&temp), StageScope::All);
        assert!(hold.has_unmet_obligation());
        assert_eq!(hold.feedback(), None);
        assert_eq!(hold.holds.get(), 0, "no hold was spent");
    }

    #[test]
    fn a_step_answers_only_for_the_stages_it_called() {
        let temp = tempfile::tempdir().unwrap();
        let run = bound_run(&temp);
        let hold = hold_over(run.clone(), StageScope::Touched);
        assert_eq!(hold.feedback(), None, "it called none");
        assert!(!hold.holds_tool_only_completion());

        assert!(!run.note_call(&only(), false));
        assert!(hold.feedback().is_some(), "now it did");
    }

    #[test]
    fn an_answer_a_model_has_not_read_holds_a_step_that_could_end_at_a_tool_result() {
        let temp = tempfile::tempdir().unwrap();
        let hold = hold_over(bound_run(&temp), StageScope::Touched);
        let note = |progress| StageNote {
            target: only(),
            progress,
        };
        assert!(!hold.absorb_notes(vec![note(false)]));
        assert!(hold.holds_tool_only_completion());
        assert!(!hold.holds_tool_only_completion(), "it has been sent back");
        assert!(hold.absorb_notes(vec![note(true)]));
        assert!(!hold.holds_tool_only_completion());
    }

    #[test]
    fn the_tool_only_hold_ends_at_the_same_limit() {
        let temp = tempfile::tempdir().unwrap();
        let run = bound_run(&temp);
        run.note_call(&only(), false);
        let hold = hold_over(run, StageScope::Touched);
        for _ in 0..MAX_STAGE_HOLDS {
            assert!(hold.holds_tool_only_completion());
        }
        assert!(!hold.holds_tool_only_completion());
    }

    #[test]
    fn with_no_stage_set_nothing_holds_and_the_funnel_passes_what_it_got() {
        let hold = StageHold::default();
        assert_eq!(hold.feedback(), None);
        assert!(!hold.holds_tool_only_completion());
        assert!(!hold.has_unmet_obligation());
        assert!(!hold.absorb_notes(Vec::new()));

        let config = crate::tools::profile_tools::effects::config_for_test(std::path::Path::new(
            "/nonexistent",
        ));
        let funnel = StageFunnel::enter(&RunSessionOptions::default());
        assert_eq!(funnel.finish(Ok(7), &config).unwrap(), 7);
        let funnel = StageFunnel::enter(&RunSessionOptions::default());
        let failed: anyhow::Result<u8> = Err(anyhow::anyhow!("as it was"));
        assert_eq!(
            funnel.finish(failed, &config).unwrap_err().to_string(),
            "as it was"
        );
    }

    #[test]
    fn the_funnel_refuses_a_success_and_puts_the_outer_session_back() {
        let temp = tempfile::tempdir().unwrap();
        let config = crate::tools::profile_tools::effects::config_for_test(std::path::Path::new(
            "/nonexistent",
        ));
        let outer = StageFunnel::enter(&RunSessionOptions::default());
        enter_stage_run(bound_run(&temp));
        {
            let inner = StageFunnel::enter(&RunSessionOptions::default());
            assert_eq!(
                inner.finish(Ok(1), &config).unwrap(),
                1,
                "no run of its own"
            );
        }
        let error = outer.finish(Ok(2), &config).unwrap_err();
        assert!(
            error
                .to_string()
                .starts_with("profile_tool_refused: stage_incomplete: obligations_unmet"),
            "{error}"
        );
    }

    /// A configuration whose wallet refused a send of the command running now (Issue #976).
    fn refused_command() -> (
        crate::structured_budget::test_support::TestWallet,
        crate::config::Config,
    ) {
        let wallet = crate::structured_budget::test_support::TestWallet::open(|value| {
            value["limits"]["physical_sends"] = serde_json::json!(1);
        });
        wallet.send(50, None).unwrap();
        wallet.send(50, None).unwrap_err();
        let mut config = crate::tools::profile_tools::effects::config_for_test(&wallet.workspace);
        config.field_sources.run_budget = Some(wallet.handle.clone());
        (wallet, config)
    }

    fn command_refusal(
        error: &anyhow::Error,
    ) -> Option<crate::structured_budget::outcome::CommandRefusal> {
        error
            .downcast_ref::<crate::structured_budget::outcome::CommandRefusal>()
            .copied()
    }

    /// G5 answers before G4: a stage left unmet does not hide that the wallet refused a send.
    #[test]
    fn the_wallet_answers_before_the_stages() {
        let temp = tempfile::tempdir().unwrap();
        let (_wallet, config) = refused_command();
        let funnel = StageFunnel::enter(&RunSessionOptions::default());
        enter_stage_run(bound_run(&temp));
        let error = funnel.finish(Ok(2), &config).unwrap_err();
        assert!(command_refusal(&error).is_some(), "{error}");
    }

    /// A stage set that lets a step end, and no stage set at all, let no refused command succeed.
    #[test]
    fn stages_that_let_a_step_end_let_no_refused_command_succeed() {
        let temp = tempfile::tempdir().unwrap();
        let (_wallet, config) = refused_command();
        let step = RunSessionOptions::plan_step(
            crate::minimal_loop::loop_run::RunSessionStepKind::Implement,
        );
        let funnel = StageFunnel::enter(&step);
        let run = bound_run(&temp);
        assert!(
            run.judge(StageScope::Touched).is_none(),
            "the stages alone let this step end"
        );
        enter_stage_run(run);
        let error = funnel.finish(Ok(1), &config).unwrap_err();
        assert!(command_refusal(&error).is_some(), "{error}");

        let funnel = StageFunnel::enter(&RunSessionOptions::default());
        let error = funnel.finish(Ok(1), &config).unwrap_err();
        assert!(command_refusal(&error).is_some(), "{error}");
    }
}
