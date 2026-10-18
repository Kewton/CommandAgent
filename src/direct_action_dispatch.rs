//! Selection of, and delegation to, the CLI's direct (non-workflow) actions.
//!
//! `run_config` keeps the process-level lifecycle (run evidence, completion
//! guard, preflight); this leaf only chooses the action to execute and hands over
//! to its runner.

use anyhow::Context;
use serde_json::json;

use crate::config::{Action, Config};
use crate::tui::markdown::PlainRenderer;
use crate::tui::{OutputRenderer, TerminalUi};
use crate::{doctor, eval_events, minimal_loop, model_probe, planner, providers, repl, state, tui};

/// Runs the direct action selected in `config`.
pub(crate) fn run(config: &Config) -> anyhow::Result<()> {
    match config.action.clone() {
        Action::Repl => repl::run_repl(config.clone()),
        Action::Prompt(prompt)
            if config.profile == planner::profiles::community_mini_app::PROFILE_ID
                && config.offline =>
        {
            let report = planner::profile::verify_profile_final(
                &config.workspace_root,
                planner::profiles::community_mini_app::PROFILE_ID,
                &prompt,
            );
            let ok = report.is_pass();
            let violation = if ok {
                String::new()
            } else {
                report.primary_reason()
            };
            eval_events::emit(
                config.eval_events_path.as_deref(),
                json!({
                    "event": "community_profile_verification",
                    "profile": planner::profiles::community_mini_app::PROFILE_ID,
                    "verdict": if ok { "full" } else { "failed" },
                    "assurance": if ok { "full" } else { "failed" },
                    "violation": violation,
                    "ok": ok,
                }),
            );
            if ok {
                Ok(())
            } else {
                anyhow::bail!("community_profile_violation:{}", report.primary_reason())
            }
        }
        Action::Prompt(prompt) => {
            let mut client = providers::client_from_config(config, false)?;
            let ui = DirectActionUi::new(config);
            let resume = if config.fresh_session {
                None
            } else {
                config.resume.as_deref()
            };
            let mut session =
                state::SessionStore::new(config.state_dir.clone()).load_or_create(resume)?;
            let reply = minimal_loop::loop_run::run_session_with_required_paths_with_ui(
                &mut *client,
                &mut session,
                &prompt,
                &[],
                config,
                ui.as_interaction(),
            )?;
            state::SessionStore::new(config.state_dir.clone()).save(&session)?;
            drop(ui);
            PlainRenderer.render_assistant(&reply)?;
            Ok(())
        }
        Action::PlanSteps(goal) => {
            let mut planner = providers::client_from_config(config, true)?;
            let ui = DirectActionUi::new(config);
            let plan = planner::generate_step_plan_with_ui(
                &mut *planner,
                &goal,
                config,
                ui.as_interaction(),
            )
            .context("failed to generate step plan")?;
            let path = planner::save_step_plan(&config.workspace_root, &plan)?;
            drop(ui);
            println!("{}", path.display());
            println!(
                "{}",
                planner::plan::saved_plan_guidance(&path, planner::plan::PlanFileKind::Step)
            );
            Ok(())
        }
        Action::PlanRun(goal) => {
            let mut execution = providers::client_from_config(config, false)?;
            let mut planner_client = providers::client_from_config(config, true)?;
            let ui = DirectActionUi::new(config);
            let report = planner::generate_and_run_step_plan_with_ui(
                &mut *planner_client,
                &mut *execution,
                &goal,
                config,
                ui.as_interaction(),
            )?;
            drop(ui);
            println!("{report}");
            Ok(())
        }
        Action::RunPlan(path) => {
            let mut execution = providers::client_from_config(config, false)?;
            let ui = DirectActionUi::new(config);
            let report = planner::run_plan_file_with_ui(
                &mut *execution,
                &path,
                config,
                ui.as_interaction(),
            )?;
            drop(ui);
            println!("{report}");
            Ok(())
        }
        Action::UltraPlan(goal) => {
            let mut planner_client = providers::client_from_config(config, true)?;
            let ui = DirectActionUi::new(config);
            let plan = planner::generate_ultra_plan_with_ui(
                &mut *planner_client,
                &goal,
                config,
                ui.as_interaction(),
            )?;
            let path = planner::save_ultra_plan(&config.workspace_root, &plan)?;
            drop(ui);
            println!("{}", path.display());
            println!(
                "{}",
                planner::plan::saved_plan_guidance(&path, planner::plan::PlanFileKind::Ultra)
            );
            Ok(())
        }
        Action::UltraPlanRun(_) | Action::RunUltraPlan(_) => {
            let mut execution = providers::client_from_config(config, false)?;
            let mut planner_client = providers::client_from_config(config, true)?;
            let ui = DirectActionUi::new(config);
            let report = run_ultra_plan_action_with_ui(
                &config.action,
                &mut *planner_client,
                &mut *execution,
                config,
                ui.as_interaction(),
            )?;
            drop(ui);
            println!("{report}");
            Ok(())
        }
        Action::SetupInteractionProbe => {
            let report =
                minimal_loop::interaction_probe::setup_interaction_probe_with_stdout_progress()?;
            for line in report.summary_lines() {
                println!("{line}");
            }
            Ok(())
        }
        Action::ModelProbe => {
            let mut execution = providers::client_from_config(config, false)?;
            let mut planner_client = providers::client_from_config(config, true)?;
            let output =
                model_probe::run_configured(config, &mut *planner_client, &mut *execution)?;
            println!("{}", output.card);
            Ok(())
        }
        Action::Doctor => {
            let report = doctor::diagnose(config);
            println!("{}", report.render_human());
            if report.failed() {
                anyhow::bail!("doctor found failed checks");
            }
            Ok(())
        }
        Action::Workflow(..) => unreachable!("workflow action dispatched before match"),
        Action::UxDemo => tui::ux_demo::run(config),
        Action::Runs(_) => Ok(()),
    }
}

pub(crate) fn run_ultra_plan_action_with_ui(
    action: &Action,
    planner: &mut dyn providers::ChatClient,
    execution: &mut dyn providers::ChatClient,
    config: &Config,
    ui: &dyn tui::InteractionUi,
) -> anyhow::Result<String> {
    match action {
        Action::UltraPlanRun(goal) => {
            planner::auto_recovery::generate_and_run_with_ui(planner, execution, goal, config, ui)
        }
        Action::RunUltraPlan(path) => {
            planner::auto_recovery::run_file_with_ui(planner, execution, path, config, ui)
        }
        _ => anyhow::bail!("action is not an UltraPlan execution entry point"),
    }
}

enum DirectActionUi {
    Terminal(TerminalUi),
    Noop,
}

impl DirectActionUi {
    fn new(config: &Config) -> Self {
        if tui::footer::FooterEnv::detect(config).enabled {
            Self::Terminal(TerminalUi::new(config))
        } else {
            Self::Noop
        }
    }

    fn as_interaction(&self) -> &dyn tui::InteractionUi {
        match self {
            Self::Terminal(ui) => ui,
            Self::Noop => &tui::NOOP_UI,
        }
    }
}
