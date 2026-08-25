//! Bounded, opt-in execution of typed Recovery Plan candidates.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::Context;
use serde_json::json;

use crate::config::Config;
use crate::planner::recovery_validation::RecoveryPlanValidationError;
use crate::planner::ultra_plan::UltraPlan;
use crate::providers::ChatClient;
use crate::tui::InteractionUi;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecoveryCandidate {
    path: PathBuf,
    plan: UltraPlan,
    failure_kind: String,
}

#[derive(Debug)]
enum AttemptFailure {
    Interrupted,
    Recoverable(RecoveryCandidate),
    NonRecoverable,
}

#[derive(Debug)]
struct AttemptOutcome {
    result: anyhow::Result<String>,
    failure: Option<AttemptFailure>,
}

#[derive(Debug, Default)]
struct AttemptCapture {
    active: bool,
    candidate: Option<RecoveryCandidate>,
}

thread_local! {
    static ATTEMPT_CAPTURE: RefCell<AttemptCapture> = RefCell::new(AttemptCapture::default());
}

struct AttemptCaptureGuard;

impl AttemptCaptureGuard {
    fn begin() -> Self {
        ATTEMPT_CAPTURE.with(|capture| {
            *capture.borrow_mut() = AttemptCapture {
                active: true,
                candidate: None,
            };
        });
        Self
    }

    fn finish(self) -> Option<RecoveryCandidate> {
        let candidate = ATTEMPT_CAPTURE.with(|capture| {
            let mut capture = capture.borrow_mut();
            capture.active = false;
            capture.candidate.take()
        });
        std::mem::forget(self);
        candidate
    }
}

impl Drop for AttemptCaptureGuard {
    fn drop(&mut self) {
        ATTEMPT_CAPTURE.with(|capture| *capture.borrow_mut() = AttemptCapture::default());
    }
}

pub(crate) fn record_candidate(path: PathBuf, plan: UltraPlan, failure_kind: String) {
    ATTEMPT_CAPTURE.with(|capture| {
        let mut capture = capture.borrow_mut();
        if capture.active {
            capture.candidate = Some(RecoveryCandidate {
                path,
                plan,
                failure_kind,
            });
        }
    });
}

fn capture_attempt(
    ui: &dyn InteractionUi,
    run: impl FnOnce() -> anyhow::Result<String>,
) -> AttemptOutcome {
    let capture = AttemptCaptureGuard::begin();
    let result = run();
    let candidate = capture.finish();
    let failure = if result.is_ok() {
        None
    } else if ui.interrupted() {
        Some(AttemptFailure::Interrupted)
    } else if let Some(candidate) = candidate {
        Some(AttemptFailure::Recoverable(candidate))
    } else {
        Some(AttemptFailure::NonRecoverable)
    };
    AttemptOutcome { result, failure }
}

enum InitialExecution<'a> {
    Generate(&'a str),
    File(&'a Path),
    Plan(&'a UltraPlan),
}

trait RecoveryDriver {
    type Prepared;

    fn prepare(&mut self, candidate: &RecoveryCandidate) -> Result<Self::Prepared, CandidateStop>;
    fn normalized(&self, prepared: &Self::Prepared) -> anyhow::Result<Vec<u8>>;
    fn start(&mut self, used: u8, candidate: &RecoveryCandidate, prepared: &Self::Prepared);
    fn execute(&mut self, prepared: Self::Prepared) -> AttemptOutcome;
}

struct RunnerRecoveryDriver<'a> {
    planner: &'a mut dyn ChatClient,
    execution: &'a mut dyn ChatClient,
    config: &'a Config,
    ui: &'a dyn InteractionUi,
}

impl RecoveryDriver for RunnerRecoveryDriver<'_> {
    type Prepared = crate::runs::ResumePlan;

    fn prepare(&mut self, candidate: &RecoveryCandidate) -> Result<Self::Prepared, CandidateStop> {
        prepare_candidate(self.config, candidate)
    }

    fn normalized(&self, prepared: &Self::Prepared) -> anyhow::Result<Vec<u8>> {
        normalized_plan(&prepared.plan)
    }

    fn start(&mut self, used: u8, candidate: &RecoveryCandidate, prepared: &Self::Prepared) {
        emit_with_candidate(self.config, "recovery_plan_auto_run_start", used, candidate);
        crate::runs::emit_resume_start(self.config, prepared);
    }

    fn execute(&mut self, prepared: Self::Prepared) -> AttemptOutcome {
        capture_attempt(self.ui, || {
            super::run_ultra_plan_with_ui(
                self.planner,
                self.execution,
                &prepared.plan,
                self.config,
                self.ui,
            )
        })
    }
}

pub fn generate_and_run_with_ui(
    planner: &mut dyn ChatClient,
    execution: &mut dyn ChatClient,
    goal: &str,
    config: &Config,
    ui: &dyn InteractionUi,
) -> anyhow::Result<String> {
    run_with_ui(
        InitialExecution::Generate(goal),
        planner,
        execution,
        config,
        ui,
    )
}

pub fn run_file_with_ui(
    planner: &mut dyn ChatClient,
    execution: &mut dyn ChatClient,
    path: &Path,
    config: &Config,
    ui: &dyn InteractionUi,
) -> anyhow::Result<String> {
    run_with_ui(InitialExecution::File(path), planner, execution, config, ui)
}

pub fn run_plan_with_ui(
    planner: &mut dyn ChatClient,
    execution: &mut dyn ChatClient,
    plan: &UltraPlan,
    config: &Config,
    ui: &dyn InteractionUi,
) -> anyhow::Result<String> {
    run_with_ui(InitialExecution::Plan(plan), planner, execution, config, ui)
}

fn run_with_ui(
    initial: InitialExecution<'_>,
    planner: &mut dyn ChatClient,
    execution: &mut dyn ChatClient,
    config: &Config,
    ui: &dyn InteractionUi,
) -> anyhow::Result<String> {
    if config.recovery_plan_auto_runs == 0 {
        return execute_initial(initial, planner, execution, config, ui);
    }

    emit(
        config,
        "recovery_plan_auto_run_configured",
        0,
        "initial_run",
    );
    let outcome = capture_attempt(ui, || {
        execute_initial(initial, planner, execution, config, ui)
    });
    drive(
        config,
        outcome,
        &mut RunnerRecoveryDriver {
            planner,
            execution,
            config,
            ui,
        },
    )
}

fn drive(
    config: &Config,
    mut outcome: AttemptOutcome,
    driver: &mut impl RecoveryDriver,
) -> anyhow::Result<String> {
    if outcome.result.is_ok() {
        emit(
            config,
            "recovery_plan_auto_run_complete",
            0,
            "initial_success",
        );
        return outcome.result;
    }
    let mut controller = AutoRecoveryController::new(config.recovery_plan_auto_runs);
    loop {
        let error = outcome.result.expect_err("failed result checked above");
        let candidate = match outcome
            .failure
            .expect("failed attempt always has a typed failure")
        {
            AttemptFailure::Interrupted => {
                emit(
                    config,
                    "recovery_plan_auto_run_stopped",
                    controller.used,
                    "interrupted",
                );
                return Err(error);
            }
            AttemptFailure::NonRecoverable => {
                emit(
                    config,
                    "recovery_plan_auto_run_stopped",
                    controller.used,
                    "not_recoverable",
                );
                return Err(error);
            }
            AttemptFailure::Recoverable(candidate) => candidate,
        };
        let Some(used) = controller.next_run() else {
            emit(
                config,
                "recovery_plan_auto_run_stopped",
                controller.used,
                "limit_reached",
            );
            return Err(error);
        };
        let prepared = match driver.prepare(&candidate) {
            Ok(prepared) => prepared,
            Err(reason) => {
                emit(
                    config,
                    "recovery_plan_auto_run_stopped",
                    used - 1,
                    reason.code(),
                );
                return Err(error.context(format!(
                    "automatic Recovery Plan stopped: {}",
                    reason.code()
                )));
            }
        };
        let normalized = driver.normalized(&prepared)?;
        if !controller.observe_plan(normalized) {
            emit(
                config,
                "recovery_plan_auto_run_stopped",
                used - 1,
                "cycle_detected",
            );
            return Err(error.context("automatic Recovery Plan stopped: cycle detected"));
        }

        driver.start(used, &candidate, &prepared);
        outcome = driver.execute(prepared);
        if outcome.result.is_ok() {
            emit(
                config,
                "recovery_plan_auto_run_complete",
                used,
                "recovery_succeeded",
            );
            return outcome.result;
        }
    }
}

fn execute_initial(
    initial: InitialExecution<'_>,
    planner: &mut dyn ChatClient,
    execution: &mut dyn ChatClient,
    config: &Config,
    ui: &dyn InteractionUi,
) -> anyhow::Result<String> {
    match initial {
        InitialExecution::Generate(goal) => {
            super::generate_and_run_ultra_plan_with_ui(planner, execution, goal, config, ui)
        }
        InitialExecution::File(path) => {
            super::run_ultra_plan_file_with_ui(planner, execution, path, config, ui)
        }
        InitialExecution::Plan(plan) => {
            super::run_ultra_plan_with_ui(planner, execution, plan, config, ui)
        }
    }
}

#[derive(Debug)]
struct AutoRecoveryController {
    limit: u8,
    used: u8,
    seen_plans: BTreeSet<Vec<u8>>,
}

impl AutoRecoveryController {
    fn new(limit: u8) -> Self {
        Self {
            limit,
            used: 0,
            seen_plans: BTreeSet::new(),
        }
    }

    fn next_run(&mut self) -> Option<u8> {
        if self.used == self.limit {
            return None;
        }
        self.used += 1;
        Some(self.used)
    }

    fn observe_plan(&mut self, normalized: Vec<u8>) -> bool {
        self.seen_plans.insert(normalized)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CandidateStop {
    PathEscape,
    RecoveryYamlMissing,
    RecoveryYamlInvalid,
    RecoveryNeedsReview,
    ResumeSafetyRejected,
    WorkspaceDrift,
}

impl CandidateStop {
    const fn code(self) -> &'static str {
        match self {
            Self::PathEscape => "path_escape",
            Self::RecoveryYamlMissing => "recovery_yaml_missing",
            Self::RecoveryYamlInvalid => "recovery_yaml_invalid",
            Self::RecoveryNeedsReview => "recovery_needs_review",
            Self::ResumeSafetyRejected => "resume_safety_rejected",
            Self::WorkspaceDrift => "workspace_drift",
        }
    }
}

fn prepare_candidate(
    config: &Config,
    candidate: &RecoveryCandidate,
) -> Result<crate::runs::ResumePlan, CandidateStop> {
    let root = config
        .workspace_root
        .canonicalize()
        .map_err(|_| CandidateStop::ResumeSafetyRejected)?;
    let path = candidate.path.canonicalize().map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            CandidateStop::RecoveryYamlMissing
        } else {
            CandidateStop::RecoveryYamlInvalid
        }
    })?;
    if !path.starts_with(&root) {
        return Err(CandidateStop::PathEscape);
    }
    let parsed = super::recovery_validation::validate(&path).map_err(|error| match error {
        RecoveryPlanValidationError::Missing => CandidateStop::RecoveryYamlMissing,
        RecoveryPlanValidationError::NeedsReview => CandidateStop::RecoveryNeedsReview,
        RecoveryPlanValidationError::Unreadable
        | RecoveryPlanValidationError::Parse
        | RecoveryPlanValidationError::Roundtrip => CandidateStop::RecoveryYamlInvalid,
    })?;
    if parsed != candidate.plan {
        return Err(CandidateStop::RecoveryYamlInvalid);
    }
    let resume = crate::runs::prepare_resume(&root, path.to_string_lossy().as_ref())
        .map_err(|_| CandidateStop::ResumeSafetyRejected)?;
    if resume.workspace_drift_error().is_some() {
        return Err(CandidateStop::WorkspaceDrift);
    }
    Ok(resume)
}

fn normalized_plan(plan: &UltraPlan) -> anyhow::Result<Vec<u8>> {
    serde_json::to_vec(plan).context("normalize Recovery Plan content")
}

fn emit(config: &Config, event: &str, used: u8, stop_reason: &str) {
    crate::eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": event,
            "recovery_plan_auto_runs": config.recovery_plan_auto_runs,
            "recovery_plan_auto_runs_used": used,
            "recovery_plan_auto_run_current": used,
            "recovery_plan_auto_run_stop_reason": stop_reason,
        }),
    );
}

fn emit_with_candidate(config: &Config, event: &str, used: u8, candidate: &RecoveryCandidate) {
    crate::eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": event,
            "recovery_plan_auto_runs": config.recovery_plan_auto_runs,
            "recovery_plan_auto_runs_used": used,
            "recovery_plan_auto_run_current": used,
            "recovery_plan_auto_run_stop_reason": "running",
            "recovery_handoff_kind": candidate.failure_kind,
            "recovery_ultra_plan_path": crate::planner::repair::workspace_relative_handoff_path(
                &candidate.path
            ),
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::ultra_plan::UltraPhase;
    use crate::providers::AssistantReply;
    use crate::state::ConversationMessage;
    use crate::tools::registry::ToolSpec;
    use clap::Parser;
    use std::collections::VecDeque;

    #[derive(Clone)]
    struct UnusedClient;

    impl ChatClient for UnusedClient {
        fn label(&self) -> &str {
            "unused"
        }

        fn boxed_clone(&self) -> Box<dyn ChatClient> {
            Box::new(self.clone())
        }

        fn chat(
            &mut self,
            _model: &str,
            _messages: &[ConversationMessage],
            _tools: &[ToolSpec],
            _native_tools_enabled: bool,
        ) -> anyhow::Result<AssistantReply> {
            panic!("missing file must fail before a model call")
        }
    }

    fn plan(goal: &str) -> UltraPlan {
        UltraPlan {
            goal: goal.to_string(),
            profile: "generic".to_string(),
            style: "recovery".to_string(),
            intent: "fix".to_string(),
            phases: vec![UltraPhase {
                id: "repair".to_string(),
                prompt: "repair".to_string(),
            }],
        }
    }

    fn config(root: &Path, limit: u8) -> Config {
        let mut config =
            Config::from_cli(crate::cli::Cli::parse_from(["commandagent", "--ux-demo"])).unwrap();
        config.workspace_root = root.to_path_buf();
        config.eval_events_path = Some(root.join("events.jsonl"));
        config.recovery_plan_auto_runs = limit;
        config
    }

    fn candidate(goal: &str) -> RecoveryCandidate {
        RecoveryCandidate {
            path: PathBuf::from(format!("{goal}.yaml")),
            plan: plan(goal),
            failure_kind: "verification_failed".to_string(),
        }
    }

    fn success(report: &str) -> AttemptOutcome {
        AttemptOutcome {
            result: Ok(report.to_string()),
            failure: None,
        }
    }

    fn failed(failure: AttemptFailure) -> AttemptOutcome {
        AttemptOutcome {
            result: Err(anyhow::anyhow!("scripted honest failure")),
            failure: Some(failure),
        }
    }

    struct ScriptedDriver {
        prepare_error: Option<CandidateStop>,
        outcomes: VecDeque<AttemptOutcome>,
        starts: Vec<u8>,
    }

    impl RecoveryDriver for ScriptedDriver {
        type Prepared = UltraPlan;

        fn prepare(
            &mut self,
            candidate: &RecoveryCandidate,
        ) -> Result<Self::Prepared, CandidateStop> {
            if let Some(error) = self.prepare_error {
                return Err(error);
            }
            Ok(candidate.plan.clone())
        }

        fn normalized(&self, prepared: &Self::Prepared) -> anyhow::Result<Vec<u8>> {
            normalized_plan(prepared)
        }

        fn start(&mut self, used: u8, _candidate: &RecoveryCandidate, _prepared: &Self::Prepared) {
            self.starts.push(used);
        }

        fn execute(&mut self, _prepared: Self::Prepared) -> AttemptOutcome {
            self.outcomes.pop_front().expect("scripted outcome")
        }
    }

    fn driver(outcomes: Vec<AttemptOutcome>) -> ScriptedDriver {
        ScriptedDriver {
            prepare_error: None,
            outcomes: outcomes.into(),
            starts: Vec::new(),
        }
    }

    #[test]
    fn initial_success_runs_no_recovery() {
        let root = tempfile::tempdir().unwrap();
        let config = config(root.path(), 3);
        let mut driver = driver(Vec::new());
        assert_eq!(
            drive(&config, success("initial"), &mut driver).unwrap(),
            "initial"
        );
        assert!(driver.starts.is_empty());
    }

    #[test]
    fn zero_uses_the_exact_legacy_path_without_auto_events() {
        let root = tempfile::tempdir().unwrap();
        let config = config(root.path(), 0);
        let mut planner = UnusedClient;
        let mut execution = UnusedClient;
        assert!(
            run_file_with_ui(
                &mut planner,
                &mut execution,
                Path::new("missing.yaml"),
                &config,
                &crate::tui::NOOP_UI,
            )
            .is_err()
        );
        assert!(!config.eval_events_path.unwrap().exists());
    }

    #[test]
    fn failure_then_recovery_success_stops_after_one() {
        let root = tempfile::tempdir().unwrap();
        let config = config(root.path(), 3);
        let mut driver = driver(vec![success("recovered")]);
        let initial = failed(AttemptFailure::Recoverable(candidate("first")));
        assert_eq!(drive(&config, initial, &mut driver).unwrap(), "recovered");
        assert_eq!(driver.starts, vec![1]);
    }

    #[test]
    fn repeated_failures_stop_at_exact_configured_count() {
        let root = tempfile::tempdir().unwrap();
        let config = config(root.path(), 2);
        let mut driver = driver(vec![
            failed(AttemptFailure::Recoverable(candidate("second"))),
            failed(AttemptFailure::Recoverable(candidate("third"))),
        ]);
        let initial = failed(AttemptFailure::Recoverable(candidate("first")));
        assert!(drive(&config, initial, &mut driver).is_err());
        assert_eq!(driver.starts, vec![1, 2]);
        let events = std::fs::read_to_string(config.eval_events_path.unwrap()).unwrap();
        assert!(events.contains("\"recovery_plan_auto_run_stop_reason\":\"limit_reached\""));
    }

    #[test]
    fn non_recoverable_and_invalid_candidates_stop_without_execution() {
        let root = tempfile::tempdir().unwrap();
        let config = config(root.path(), 2);
        let mut non_recoverable = driver(Vec::new());
        assert!(
            drive(
                &config,
                failed(AttemptFailure::NonRecoverable),
                &mut non_recoverable,
            )
            .is_err()
        );
        assert!(non_recoverable.starts.is_empty());

        let mut invalid = driver(Vec::new());
        invalid.prepare_error = Some(CandidateStop::RecoveryYamlInvalid);
        assert!(
            drive(
                &config,
                failed(AttemptFailure::Recoverable(candidate("invalid"))),
                &mut invalid,
            )
            .is_err()
        );
        assert!(invalid.starts.is_empty());
    }

    #[test]
    fn candidate_preparation_returns_typed_safety_stops() {
        let root = tempfile::tempdir().unwrap();
        let config = config(root.path(), 1);
        let expected_plan = plan("candidate");

        let missing = RecoveryCandidate {
            path: root.path().join("missing.yaml"),
            plan: expected_plan.clone(),
            failure_kind: "test".to_string(),
        };
        assert_eq!(
            prepare_candidate(&config, &missing).unwrap_err(),
            CandidateStop::RecoveryYamlMissing
        );

        let invalid_path = root.path().join("invalid.yaml");
        std::fs::write(&invalid_path, "not a plan").unwrap();
        let invalid = RecoveryCandidate {
            path: invalid_path,
            plan: expected_plan.clone(),
            failure_kind: "test".to_string(),
        };
        assert_eq!(
            prepare_candidate(&config, &invalid).unwrap_err(),
            CandidateStop::RecoveryYamlInvalid
        );

        let review_path = root.path().join("review.yaml");
        std::fs::write(
            &review_path,
            format!(
                "recovery_needs_review: true\n{}",
                crate::planner::ultra_plan::render_ultra_plan(&expected_plan)
            ),
        )
        .unwrap();
        let review = RecoveryCandidate {
            path: review_path,
            plan: expected_plan.clone(),
            failure_kind: "test".to_string(),
        };
        assert_eq!(
            prepare_candidate(&config, &review).unwrap_err(),
            CandidateStop::RecoveryNeedsReview
        );

        let outside = tempfile::tempdir().unwrap();
        let outside_path = outside.path().join("outside.yaml");
        std::fs::write(
            &outside_path,
            crate::planner::ultra_plan::render_ultra_plan(&expected_plan),
        )
        .unwrap();
        let escaped = RecoveryCandidate {
            path: outside_path,
            plan: expected_plan,
            failure_kind: "test".to_string(),
        };
        assert_eq!(
            prepare_candidate(&config, &escaped).unwrap_err(),
            CandidateStop::PathEscape
        );
    }

    #[test]
    fn controller_caps_recovery_runs_at_exact_limit() {
        let mut controller = AutoRecoveryController::new(2);
        assert_eq!(controller.next_run(), Some(1));
        assert_eq!(controller.next_run(), Some(2));
        assert_eq!(controller.next_run(), None);
        assert_eq!(controller.used, 2);
    }

    #[test]
    fn normalized_cycle_ignores_yaml_metadata_paths_and_formatting() {
        let original = plan("recover");
        let rendered = crate::planner::ultra_plan::render_ultra_plan(&original);
        let decorated =
            format!("# volatile path: /tmp/first.yaml\nrecovery_failure_kind: first\n{rendered}");
        let parsed = crate::planner::ultra_plan::parse_ultra_plan(&decorated).unwrap();
        let mut controller = AutoRecoveryController::new(2);
        assert!(controller.observe_plan(normalized_plan(&original).unwrap()));
        assert!(!controller.observe_plan(normalized_plan(&parsed).unwrap()));
    }

    #[test]
    fn attempt_outcome_is_typed_without_error_text_classification() {
        struct InterruptedUi;
        impl InteractionUi for InterruptedUi {
            fn before_model_call(&self, _label: &str) -> crate::tui::UiGuard {
                crate::tui::UiGuard::noop()
            }

            fn before_tool_call(&self, _name: &str) -> crate::tui::UiGuard {
                crate::tui::UiGuard::noop()
            }

            fn publish_status(&self, _status: crate::tui::status::UiStatus) {}

            fn interrupted(&self) -> bool {
                true
            }
        }
        let outcome = capture_attempt(&InterruptedUi, || anyhow::bail!("arbitrary wording"));
        assert!(matches!(outcome.failure, Some(AttemptFailure::Interrupted)));

        let outcome = capture_attempt(&crate::tui::NOOP_UI, || {
            anyhow::bail!("interrupted by user")
        });
        assert!(matches!(
            outcome.failure,
            Some(AttemptFailure::NonRecoverable)
        ));

        let expected = candidate("typed");
        let recorded = expected.clone();
        let outcome = capture_attempt(&crate::tui::NOOP_UI, || {
            record_candidate(recorded.path, recorded.plan, recorded.failure_kind);
            anyhow::bail!("unclassified failure")
        });
        assert!(matches!(
            outcome.failure,
            Some(AttemptFailure::Recoverable(candidate)) if candidate == expected
        ));
    }
}
