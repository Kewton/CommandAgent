pub mod acceptance;
pub mod ambiguity;
pub mod band_catalog;
pub mod confirmation;
pub mod family_catalog;
pub mod route;

use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use serde_json::json;

use self::acceptance::{NextAction, TerminalPresentation};
use self::ambiguity::RouteProposal;
use self::confirmation::{ConfirmationIdentity, ConfirmedDispatch, ExecutionPins, PackSelection};

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
    NeedsGateOne(NextAction),
    Closed,
}

pub struct BoundaryShell {
    state: BoundaryState,
    confirmation_root: PathBuf,
    audit_events_path: Option<PathBuf>,
}

impl BoundaryShell {
    pub fn new(confirmation_root: PathBuf, audit_events_path: Option<PathBuf>) -> Self {
        Self {
            state: BoundaryState::Collecting,
            confirmation_root,
            audit_events_path,
        }
    }

    pub fn state(&self) -> &BoundaryState {
        &self.state
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
            BoundaryState::Collecting | BoundaryState::NeedsGateOne(_)
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
        let BoundaryState::Running { card_hash } = &self.state else {
            bail!("a terminal sheet can be presented only after confirmed dispatch");
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

    pub fn select_next_action(&mut self, action: NextAction) -> anyhow::Result<()> {
        if !matches!(self.state, BoundaryState::FailureReady(_)) {
            bail!("next actions are available only at Gate 4");
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
}
