pub mod acceptance;
pub mod ambiguity;
pub mod band_catalog;
pub mod confirmation;
pub mod directive;
pub mod directive_regression;
pub mod directive_session;
pub mod family_catalog;
pub mod pack_catalog;
pub mod presentation;
pub mod route;
pub mod sheet;
pub mod transcript;

use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use serde_json::json;

use self::acceptance::{NextAction, TerminalPresentation};
use self::ambiguity::RouteProposal;
use self::confirmation::{ConfirmationIdentity, ConfirmedDispatch, ExecutionPins, PackSelection};
use self::directive::{ConfirmedDirective, DirectiveContinuation, PersistedDirective};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundaryState {
    Collecting,
    AwaitingConfirmation {
        identity: ConfirmationIdentity,
        card_hash: String,
    },
    Confirmed(ConfirmedDispatch),
    Running {
        card_hash: String,
    },
    AcceptanceReady(TerminalPresentation),
    FailureReady(TerminalPresentation),
    AwaitingDirectiveConfirmation {
        directive: PersistedDirective,
        card_hash: String,
    },
    DirectiveConfirmed {
        confirmed: ConfirmedDirective,
        card_hash: String,
    },
    DirectiveRunning {
        card_hash: String,
        directive_hash: String,
        directive_round: u32,
        target_run_id: String,
    },
    NeedsGateOne(NextAction),
    Closed,
}

pub struct BoundaryShell {
    state: BoundaryState,
    confirmation_root: PathBuf,
    directive_root: PathBuf,
    directive_confirmation_root: PathBuf,
    directive_run_metadata_root: PathBuf,
    directive_session_root: PathBuf,
    regression_freeze_root: PathBuf,
    audit_events_path: Option<PathBuf>,
}

impl BoundaryShell {
    pub fn new(confirmation_root: PathBuf, audit_events_path: Option<PathBuf>) -> Self {
        let state_root = confirmation_root
            .parent()
            .unwrap_or(&confirmation_root)
            .to_path_buf();
        Self {
            state: BoundaryState::Collecting,
            confirmation_root,
            directive_root: state_root.join("boundary-directives"),
            directive_confirmation_root: state_root.join("boundary-directive-confirmations"),
            directive_run_metadata_root: state_root.join("boundary-directive-runs"),
            directive_session_root: state_root.join("boundary-sessions"),
            regression_freeze_root: state_root.join("boundary-regression-freezes"),
            audit_events_path,
        }
    }

    pub fn state(&self) -> &BoundaryState {
        &self.state
    }

    pub fn restore_latest_terminal(&mut self) -> anyhow::Result<Option<ConfirmationIdentity>> {
        if !matches!(self.state, BoundaryState::Collecting) {
            bail!("boundary terminal restoration requires the collecting state");
        }
        let Some(confirmed) = confirmation::load_latest_confirmation(&self.confirmation_root)?
        else {
            return Ok(None);
        };
        let events_path = self
            .audit_events_path
            .as_deref()
            .context("persisted boundary session requires an event stream")?;
        let event = crate::eval_events::latest_tui_command_stop_event(Some(events_path))
            .context("persisted boundary session has no terminal evidence")?;
        let command_succeeded = event.get("ok").and_then(serde_json::Value::as_bool) == Some(true);
        let generated =
            sheet::generate(confirmed.identity(), Some(events_path), command_succeeded)?;
        let terminal = TerminalPresentation::new(
            confirmed.card_hash().to_string(),
            generated.markdown,
            generated.full,
            generated.section5,
        )?;
        let identity = confirmed.identity().clone();
        self.state = if terminal.full {
            BoundaryState::AcceptanceReady(terminal)
        } else {
            BoundaryState::FailureReady(terminal)
        };
        Ok(Some(identity))
    }

    pub fn next_directive_round(&self, target_run_id: &str) -> anyhow::Result<u32> {
        directive_session::next_round(
            &self.directive_session_root,
            &self.directive_root,
            target_run_id,
        )
    }

    pub fn begin_gate_one(
        &mut self,
        proposal: RouteProposal,
        request: impl Into<String>,
        workspace: &Path,
        pins: ExecutionPins,
        pack: PackSelection,
    ) -> anyhow::Result<&ConfirmationIdentity> {
        if !matches!(
            self.state,
            BoundaryState::Collecting
                | BoundaryState::AwaitingConfirmation { .. }
                | BoundaryState::NeedsGateOne(_)
        ) {
            bail!("boundary shell cannot start Gate 1 from {:?}", self.state);
        }
        let selected = proposal
            .selected
            .context("typed unknown route requires human correction before Gate 1")?;
        let band = selected
            .band()
            .context("registered route is missing a capability band")?;
        let identity =
            ConfirmationIdentity::new(request.into(), workspace, &selected, band, pins, pack)?;
        let card_hash = identity.card_hash()?;
        crate::eval_events::emit(
            self.audit_events_path.as_deref(),
            json!({
                "event": "route_proposed",
                "profile": identity.profile,
                "intent": identity.intent,
                "task_family": identity.task_family,
                "card_hash": card_hash,
                "confirmation_required": true,
                "classifier_used": proposal.classifier.used,
                "classifier_parse_reason": proposal.classifier.parse_reason,
            }),
        );
        self.state = BoundaryState::AwaitingConfirmation {
            identity,
            card_hash,
        };
        match &self.state {
            BoundaryState::AwaitingConfirmation { identity, .. } => Ok(identity),
            _ => unreachable!(),
        }
    }

    pub fn begin_pack_change(
        &mut self,
        previous: &ConfirmationIdentity,
        pack: PackSelection,
    ) -> anyhow::Result<&ConfirmationIdentity> {
        if !matches!(self.state, BoundaryState::FailureReady(_)) {
            bail!("pack changes are available only at Gate 4");
        }
        pack_catalog::validate_selection(&previous.profile, &previous.intent, &pack)?;
        self.select_next_action(NextAction::PackChange)?;
        let mut identity = previous.clone();
        identity.pack = pack;
        let card_hash = identity.card_hash()?;
        crate::eval_events::emit(
            self.audit_events_path.as_deref(),
            json!({
                "event": "route_proposed",
                "profile": identity.profile,
                "intent": identity.intent,
                "task_family": identity.task_family,
                "card_hash": card_hash,
                "confirmation_required": true,
                "classifier_used": false,
                "classifier_parse_reason": "gate_4_pack_change",
            }),
        );
        self.state = BoundaryState::AwaitingConfirmation {
            identity,
            card_hash,
        };
        match &self.state {
            BoundaryState::AwaitingConfirmation { identity, .. } => Ok(identity),
            _ => unreachable!(),
        }
    }

    pub fn confirm(&mut self, card_hash: &str) -> anyhow::Result<&ConfirmedDispatch> {
        let BoundaryState::AwaitingConfirmation {
            identity,
            card_hash: expected_hash,
        } = &self.state
        else {
            bail!("no Gate 1 proposal is awaiting confirmation");
        };
        if card_hash != expected_hash {
            bail!("confirmation card identity changed; render Gate 1 again");
        }
        let confirmed =
            confirmation::persist_confirmation(&self.confirmation_root, identity, expected_hash)?;
        crate::eval_events::emit(
            self.audit_events_path.as_deref(),
            json!({
                "event": "route_confirmed",
                "profile": identity.profile,
                "intent": identity.intent,
                "task_family": identity.task_family,
                "card_hash": expected_hash,
                "confirmation_record": confirmed.record_path(),
            }),
        );
        self.state = BoundaryState::Confirmed(confirmed);
        match &self.state {
            BoundaryState::Confirmed(confirmed) => Ok(confirmed),
            _ => unreachable!(),
        }
    }

    pub fn dispatch(
        &mut self,
        run: impl FnOnce(&ConfirmationIdentity) -> anyhow::Result<String>,
    ) -> anyhow::Result<String> {
        let BoundaryState::Confirmed(confirmed) = &self.state else {
            bail!("dispatch denied: persisted Gate 1 confirmation is required");
        };
        confirmed.validate()?;
        let identity = confirmed.identity().clone();
        let card_hash = confirmed.card_hash().to_string();
        self.state = BoundaryState::Running { card_hash };
        run(&identity)
    }

    pub fn present_terminal(
        &mut self,
        acceptance_sheet: String,
        full: bool,
        section5: Option<String>,
    ) -> anyhow::Result<&TerminalPresentation> {
        let card_hash = match &self.state {
            BoundaryState::Running { card_hash }
            | BoundaryState::DirectiveRunning { card_hash, .. } => card_hash,
            _ => bail!("a terminal sheet can be presented only after confirmed dispatch"),
        };
        let presentation =
            TerminalPresentation::new(card_hash.clone(), acceptance_sheet, full, section5)?;
        self.state = if full {
            BoundaryState::AcceptanceReady(presentation)
        } else {
            BoundaryState::FailureReady(presentation)
        };
        match &self.state {
            BoundaryState::AcceptanceReady(presentation)
            | BoundaryState::FailureReady(presentation) => Ok(presentation),
            _ => unreachable!(),
        }
    }

    pub fn begin_directive(
        &mut self,
        raw: &str,
        target_run_id: &str,
        round: u32,
    ) -> anyhow::Result<&PersistedDirective> {
        let (terminal, issued_gate) = match &self.state {
            BoundaryState::FailureReady(terminal) => (terminal, "gate_4"),
            BoundaryState::AcceptanceReady(terminal) => (terminal, "gate_3"),
            _ => bail!("human directives are available only at Gate 3 or Gate 4"),
        };
        let card_hash = terminal.card_hash.clone();
        let directive = if issued_gate == "gate_4" {
            // v0 compatibility boundary: preserve the exact Gate 4 artifact bytes.
            directive::persist(&self.directive_root, raw, target_run_id, round)?
        } else {
            directive::persist_for_gate(
                &self.directive_root,
                raw,
                target_run_id,
                round,
                issued_gate,
            )?
        };
        directive_session::record_directive(
            &self.directive_session_root,
            &self.directive_root,
            &directive,
        )?;
        crate::eval_events::emit(
            self.audit_events_path.as_deref(),
            json!({
                "event": "human_directive_proposed",
                "directive_hash": directive.hash(),
                "directive_round": directive.artifact().round,
                "directive_target_run_id": directive.artifact().target_run_id,
                "issued_gate": directive.artifact().issued_gate,
                "confirmation_required": true,
            }),
        );
        self.state = BoundaryState::AwaitingDirectiveConfirmation {
            directive,
            card_hash,
        };
        match &self.state {
            BoundaryState::AwaitingDirectiveConfirmation { directive, .. } => Ok(directive),
            _ => unreachable!(),
        }
    }

    pub fn restore_directive_proposal(
        &mut self,
        directive_hash: &str,
    ) -> anyhow::Result<&PersistedDirective> {
        let (terminal, issued_gate) = match &self.state {
            BoundaryState::FailureReady(terminal) => (terminal, "gate_4"),
            BoundaryState::AcceptanceReady(terminal) => (terminal, "gate_3"),
            _ => bail!("directive restoration requires Gate 3 or Gate 4"),
        };
        let card_hash = terminal.card_hash.clone();
        let directive = directive::load(&self.directive_root, directive_hash)?;
        if directive.artifact().issued_gate != issued_gate {
            bail!("persisted directive gate differs from the current terminal gate");
        }
        self.state = BoundaryState::AwaitingDirectiveConfirmation {
            directive,
            card_hash,
        };
        match &self.state {
            BoundaryState::AwaitingDirectiveConfirmation { directive, .. } => Ok(directive),
            _ => unreachable!(),
        }
    }

    pub fn prepare_confirmed_continuation(
        &self,
        workspace: &Path,
        events_path: &Path,
        identity: &ConfirmationIdentity,
        directive: &PersistedDirective,
    ) -> anyhow::Result<DirectiveContinuation> {
        match directive.artifact().issued_gate.as_str() {
            "gate_4" => directive::prepare_continuation(workspace, events_path, directive),
            "gate_3" => {
                let freeze = directive_regression::freeze_from_full(
                    &self.regression_freeze_root,
                    events_path,
                    &directive.artifact().target_run_id,
                    &identity.profile,
                    &identity.intent,
                    &identity.contract_checks,
                )?;
                let history = if directive.artifact().round >= 2 {
                    let session = directive_session::record_latest_result(
                        &self.directive_session_root,
                        &directive.artifact().target_run_id,
                        directive.artifact().round - 1,
                        events_path,
                    )?;
                    Some(directive_session::render_history(
                        session.session(),
                        directive.artifact().round,
                        directive_session::MAX_HISTORY_RENDERED_BYTES,
                    )?)
                } else {
                    None
                };
                directive_regression::prepare_modification_continuation(
                    workspace,
                    directive,
                    freeze,
                    history.as_deref(),
                )
            }
            other => bail!("unsupported directive issue gate `{other}`"),
        }
    }

    pub fn confirm_directive(
        &mut self,
        directive_hash: &str,
    ) -> anyhow::Result<&ConfirmedDirective> {
        let BoundaryState::AwaitingDirectiveConfirmation {
            directive,
            card_hash,
        } = &self.state
        else {
            bail!("no Gate 4 directive is awaiting confirmation");
        };
        if directive.hash() != directive_hash {
            bail!("directive identity changed; render the Gate 4 directive again");
        }
        let card_hash = card_hash.clone();
        let confirmed = directive::confirm(&self.directive_confirmation_root, directive)?;
        crate::eval_events::emit(
            self.audit_events_path.as_deref(),
            json!({
                "event": "human_directive_confirmed",
                "directive_hash": confirmed.directive().hash(),
                "directive_round": confirmed.directive().artifact().round,
                "directive_target_run_id": confirmed.directive().artifact().target_run_id,
                "confirmation_record": confirmed.record_path(),
            }),
        );
        self.state = BoundaryState::DirectiveConfirmed {
            confirmed,
            card_hash,
        };
        match &self.state {
            BoundaryState::DirectiveConfirmed { confirmed, .. } => Ok(confirmed),
            _ => unreachable!(),
        }
    }

    pub fn dispatch_directive(
        &mut self,
        continuation: &DirectiveContinuation,
        run: impl FnOnce() -> anyhow::Result<String>,
    ) -> anyhow::Result<String> {
        let BoundaryState::DirectiveConfirmed {
            confirmed,
            card_hash,
        } = &self.state
        else {
            bail!("directive dispatch denied: persisted directive confirmation is required");
        };
        confirmed.validate()?;
        if continuation.directive_hash != confirmed.directive().hash()
            || continuation.directive_round != confirmed.directive().artifact().round
            || continuation.target_run_id != confirmed.directive().artifact().target_run_id
        {
            bail!("directive continuation does not match the confirmed directive");
        }
        if !continuation.plan_path.is_file() {
            bail!("confirmed directive continuation plan is missing");
        }
        let issued_gate = confirmed.directive().artifact().issued_gate.as_str();
        if issued_gate == "gate_3" && continuation.regression_freeze.is_none() {
            bail!("post-full directive dispatch denied: regression freeze is required");
        }
        if issued_gate == "gate_4" && continuation.regression_freeze.is_some() {
            bail!("failed-run directive must not carry a post-full regression freeze");
        }
        if let Some(freeze) = &continuation.regression_freeze {
            freeze.validate()?;
        }
        let card_hash = card_hash.clone();
        self.state = BoundaryState::DirectiveRunning {
            card_hash,
            directive_hash: continuation.directive_hash.clone(),
            directive_round: continuation.directive_round,
            target_run_id: continuation.target_run_id.clone(),
        };
        crate::eval_events::emit(
            self.audit_events_path.as_deref(),
            json!({
                "event": "human_directive_continuation_started",
                "directive_hash": continuation.directive_hash,
                "directive_round": continuation.directive_round,
                "directive_target_run_id": continuation.target_run_id,
                "continuation_plan_path": continuation.plan_workspace_path,
                "same_workspace": true,
            }),
        );
        let result = run();
        let result = match (result, continuation.regression_freeze.as_ref()) {
            (Ok(output), Some(freeze)) => {
                let events_path = self
                    .audit_events_path
                    .as_deref()
                    .context("post-full regression verification requires an event stream")?;
                directive_regression::verify_preserved_full(freeze, events_path)?;
                Ok(output)
            }
            (result, _) => result,
        };
        directive::persist_run_metadata(
            &self.directive_run_metadata_root,
            continuation,
            result.is_ok(),
        )?;
        if let Some(events_path) = self.audit_events_path.as_deref() {
            directive_session::record_latest_result(
                &self.directive_session_root,
                &continuation.target_run_id,
                continuation.directive_round,
                events_path,
            )?;
        }
        crate::eval_events::emit(
            self.audit_events_path.as_deref(),
            json!({
                "event": "human_directive_continuation_stopped",
                "directive_hash": continuation.directive_hash,
                "directive_round": continuation.directive_round,
                "directive_target_run_id": continuation.target_run_id,
                "ok": result.is_ok(),
            }),
        );
        result
    }

    pub fn select_next_action(&mut self, action: NextAction) -> anyhow::Result<()> {
        if !matches!(self.state, BoundaryState::FailureReady(_)) {
            bail!("next actions are available only at Gate 4");
        }
        if action == NextAction::HumanDirective {
            bail!("human_directive requires persisted text through begin_directive");
        }
        crate::eval_events::emit(
            self.audit_events_path.as_deref(),
            json!({
                "event": "next_action_selected",
                "action": action.as_str(),
                "returns_to_gate_one": action != NextAction::Close,
            }),
        );
        self.state = if action == NextAction::Close {
            BoundaryState::Closed
        } else {
            BoundaryState::NeedsGateOne(action)
        };
        Ok(())
    }
}

pub fn execution_slash_requires_gate_one(line: &str) -> bool {
    let command = crate::tui::slash::parse_words(line)
        .into_iter()
        .next()
        .unwrap_or_default();
    crate::tui::slash::slash_command_spec(&command).is_some_and(|spec| {
        matches!(
            spec.kind,
            crate::tui::slash::SlashCommandKind::Resume
                | crate::tui::slash::SlashCommandKind::PlanRun
                | crate::tui::slash::SlashCommandKind::RunPlan
                | crate::tui::slash::SlashCommandKind::UltraPlanRun
                | crate::tui::slash::SlashCommandKind::RunUltraPlan
        )
    })
}

#[cfg(test)]
mod tests {
    use crate::planner::adjudication::contract::IntentId;
    use crate::planner::profile::ProfileId;
    use crate::tui::boundary_shell::ambiguity::{ClassifierProvenance, ProposalStatus};
    use crate::tui::boundary_shell::family_catalog::TaskFamilyId;
    use crate::tui::boundary_shell::route::{RouteBasis, RouteCandidate};

    use super::*;

    fn proposal() -> RouteProposal {
        RouteProposal {
            selected: Some(RouteCandidate {
                profile: ProfileId::Ingest,
                intent: IntentId::Create,
                family: TaskFamilyId::List,
                bases: vec![RouteBasis {
                    rule: "fixture",
                    observation: "list".to_string(),
                }],
                contract_ref: "docs/ingest-profile-contract.md",
            }),
            alternatives: Vec::new(),
            classifier: ClassifierProvenance {
                used: false,
                provider: "ollama".to_string(),
                model: "planner".to_string(),
                prompt_version: "fixture",
                candidate_keys: Vec::new(),
                raw_response_hash: None,
                parse_reason: "deterministic_unique".to_string(),
            },
            status: ProposalStatus::AwaitingConfirmation,
            confirmation_required: true,
        }
    }

    fn pins() -> ExecutionPins {
        ExecutionPins {
            planner_provider: "ollama".to_string(),
            planner_model: "planner".to_string(),
            executor_provider: "ollama".to_string(),
            executor_model: "executor".to_string(),
            preset: "profile".to_string(),
        }
    }

    #[test]
    fn dispatch_is_impossible_without_a_persisted_confirmation_record() {
        let dir = tempfile::tempdir().unwrap();
        let mut shell = BoundaryShell::new(dir.path().join("confirmations"), None);
        let mut calls = 0;
        let denied = shell.dispatch(|_| {
            calls += 1;
            Ok("must not run".to_string())
        });
        assert!(denied.is_err());
        assert_eq!(calls, 0);

        shell
            .begin_gate_one(
                proposal(),
                "request",
                dir.path(),
                pins(),
                PackSelection::None,
            )
            .unwrap();
        let denied = shell.dispatch(|_| {
            calls += 1;
            Ok("must not run".to_string())
        });
        assert!(denied.is_err());
        assert_eq!(calls, 0);

        let hash = match shell.state() {
            BoundaryState::AwaitingConfirmation { card_hash, .. } => card_hash.clone(),
            state => panic!("wrong state: {state:?}"),
        };
        shell.confirm(&hash).unwrap();
        let output = shell
            .dispatch(|identity| {
                calls += 1;
                Ok(identity.task_family.clone())
            })
            .unwrap();
        assert_eq!(output, "list");
        assert_eq!(calls, 1);
        assert!(matches!(shell.state(), BoundaryState::Running { .. }));
    }

    #[test]
    fn repl_execution_commands_are_classified_as_gate_one_only() {
        for command in [
            "/plan-run goal",
            "/run-plan plan.json",
            "/ultra-plan-run goal",
            "/run-ultra-plan plan.yaml",
            "/resume run-id",
        ] {
            assert!(execution_slash_requires_gate_one(command), "{command}");
        }
        for command in ["/help", "/status", "/plan-steps goal", "ordinary request"] {
            assert!(!execution_slash_requires_gate_one(command), "{command}");
        }
        let repl_source = include_str!("../repl.rs");
        assert!(
            repl_source.contains("execution_slash_requires_gate_one(line)"),
            "REPL execution boundary bypassed the D-3c guard"
        );
    }

    #[test]
    fn terminal_and_next_action_lifecycle_returns_consequences_to_gate_one() {
        let dir = tempfile::tempdir().unwrap();
        let mut shell = BoundaryShell::new(dir.path().join("confirmations"), None);
        shell
            .begin_gate_one(
                proposal(),
                "request",
                dir.path(),
                pins(),
                PackSelection::None,
            )
            .unwrap();
        let hash = match shell.state() {
            BoundaryState::AwaitingConfirmation { card_hash, .. } => card_hash.clone(),
            _ => unreachable!(),
        };
        shell.confirm(&hash).unwrap();
        shell.dispatch(|_| Ok("failed".to_string())).unwrap();
        shell
            .present_terminal(
                "# Acceptance sheet\n\n## 5. Stop reason\nfailed".to_string(),
                false,
                Some("failed".to_string()),
            )
            .unwrap();
        shell.select_next_action(NextAction::ElevatedModel).unwrap();
        assert_eq!(
            shell.state(),
            &BoundaryState::NeedsGateOne(NextAction::ElevatedModel)
        );
    }

    #[test]
    fn directive_cannot_dispatch_without_exact_persisted_confirmation() {
        let dir = tempfile::tempdir().unwrap();
        let events = dir.path().join("events.jsonl");
        let mut shell = BoundaryShell::new(
            dir.path().join("boundary-confirmations"),
            Some(events.clone()),
        );
        shell
            .begin_gate_one(
                proposal(),
                "request",
                dir.path(),
                pins(),
                PackSelection::None,
            )
            .unwrap();
        let card_hash = match shell.state() {
            BoundaryState::AwaitingConfirmation { card_hash, .. } => card_hash.clone(),
            _ => unreachable!(),
        };
        shell.confirm(&card_hash).unwrap();
        shell.dispatch(|_| Ok("failed".to_string())).unwrap();
        shell
            .present_terminal(
                "# sheet\n\n## 5. Stop reason\nfailed".to_string(),
                false,
                Some("failed".to_string()),
            )
            .unwrap();
        crate::eval_events::emit(
            Some(&events),
            serde_json::json!({
                "event": "tui_command_stop",
                "ok": false,
                "status": "failed",
                "stop_reason": "fixture failure",
            }),
        );
        let directive = shell
            .begin_directive("repair README", "run-001", 1)
            .unwrap()
            .clone();
        let plan_path = dir.path().join("continuation.yaml");
        std::fs::write(&plan_path, "goal: x").unwrap();
        let continuation = DirectiveContinuation {
            plan_path,
            plan_workspace_path: ".anvil/plans/directive.yaml".to_string(),
            target_run_id: "run-001".to_string(),
            directive_round: 1,
            directive_hash: directive.hash().to_string(),
            regression_freeze: None,
        };

        let mut calls = 0;
        let denied = shell.dispatch_directive(&continuation, || {
            calls += 1;
            Ok("must not run".to_string())
        });
        assert!(denied.is_err());
        assert_eq!(calls, 0);
        assert!(shell.confirm_directive("sha256:wrong").is_err());
        assert_eq!(calls, 0);

        shell.confirm_directive(directive.hash()).unwrap();
        let result = shell
            .dispatch_directive(&continuation, || {
                calls += 1;
                Ok("continued".to_string())
            })
            .unwrap();
        assert_eq!(result, "continued");
        assert_eq!(calls, 1);
        assert!(matches!(
            shell.state(),
            BoundaryState::DirectiveRunning {
                directive_round: 1,
                ..
            }
        ));
        let event_text = std::fs::read_to_string(events).unwrap();
        assert!(event_text.contains("\"event\":\"human_directive_continuation_started\""));
        assert!(event_text.contains("\"directive_round\":1"));
        assert!(event_text.contains("\"directive_target_run_id\":\"run-001\""));
    }

    #[test]
    fn gate_three_directive_cannot_dispatch_without_regression_freeze() {
        let dir = tempfile::tempdir().unwrap();
        let events = dir.path().join("events.jsonl");
        let mut shell = BoundaryShell::new(dir.path().join("boundary-confirmations"), Some(events));
        shell
            .begin_gate_one(
                proposal(),
                "request",
                dir.path(),
                pins(),
                PackSelection::None,
            )
            .unwrap();
        let card_hash = match shell.state() {
            BoundaryState::AwaitingConfirmation { card_hash, .. } => card_hash.clone(),
            _ => unreachable!(),
        };
        shell.confirm(&card_hash).unwrap();
        shell.dispatch(|_| Ok("full".to_string())).unwrap();
        shell
            .present_terminal("# full sheet".to_string(), true, None)
            .unwrap();
        let directive = shell
            .begin_directive("change report", "run-001", 1)
            .unwrap()
            .clone();
        assert_eq!(directive.artifact().issued_gate, "gate_3");
        shell.confirm_directive(directive.hash()).unwrap();
        let plan_path = dir.path().join("continuation.yaml");
        std::fs::write(&plan_path, "goal: x").unwrap();
        let continuation = DirectiveContinuation {
            plan_path,
            plan_workspace_path: ".anvil/plans/directive.yaml".to_string(),
            target_run_id: "run-001".to_string(),
            directive_round: 1,
            directive_hash: directive.hash().to_string(),
            regression_freeze: None,
        };
        let mut calls = 0;
        let error = shell
            .dispatch_directive(&continuation, || {
                calls += 1;
                Ok("must not run".to_string())
            })
            .unwrap_err();
        assert!(error.to_string().contains("regression freeze is required"));
        assert_eq!(calls, 0);
    }

    #[test]
    fn persisted_confirmation_and_terminal_evidence_restore_the_session_boundary() {
        let dir = tempfile::tempdir().unwrap();
        let confirmations = dir.path().join("boundary-confirmations");
        let events = dir.path().join("events.jsonl");
        let mut first = BoundaryShell::new(confirmations.clone(), Some(events.clone()));
        first
            .begin_gate_one(
                proposal(),
                "request",
                dir.path(),
                pins(),
                PackSelection::None,
            )
            .unwrap();
        let card_hash = match first.state() {
            BoundaryState::AwaitingConfirmation { card_hash, .. } => card_hash.clone(),
            _ => unreachable!(),
        };
        first.confirm(&card_hash).unwrap();
        std::fs::write(
            &events,
            "{\"event\":\"tui_command_stop\",\"ok\":false,\"status\":\"failed\",\"assurance_level\":\"static\",\"final_acceptance_status\":\"incomplete\",\"effective_profile\":\"ingest\",\"stop_reason\":\"fixture stop\"}\n",
        )
        .unwrap();

        let mut resumed = BoundaryShell::new(confirmations.clone(), Some(events.clone()));
        let identity = resumed.restore_latest_terminal().unwrap().unwrap();
        assert_eq!(identity.profile, "ingest");
        assert!(matches!(resumed.state(), BoundaryState::FailureReady(_)));
        let directive = resumed
            .begin_directive("repair README", "run-001", 1)
            .unwrap()
            .clone();

        let mut restarted = BoundaryShell::new(confirmations, Some(events));
        restarted.restore_latest_terminal().unwrap().unwrap();
        assert!(
            restarted
                .restore_directive_proposal("sha256:wrong")
                .is_err()
        );
        let restored = restarted
            .restore_directive_proposal(directive.hash())
            .unwrap();
        assert_eq!(restored, &directive);
        let mut calls = 0;
        assert!(
            restarted
                .dispatch_directive(
                    &DirectiveContinuation {
                        plan_path: dir.path().join("missing.yaml"),
                        plan_workspace_path: ".anvil/plans/missing.yaml".to_string(),
                        target_run_id: "run-001".to_string(),
                        directive_round: 1,
                        directive_hash: directive.hash().to_string(),
                        regression_freeze: None,
                    },
                    || {
                        calls += 1;
                        Ok("must not run".to_string())
                    },
                )
                .is_err()
        );
        assert_eq!(calls, 0);
    }
}
