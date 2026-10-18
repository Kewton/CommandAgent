//! Execution dispatch for parsed slash commands.
//!
//! `handle_command` keeps parsing, the help registry, session state, event
//! framing and completion finalisation; this leaf only decides what each
//! command kind runs once its arguments are parsed.

use std::io::{self, IsTerminal, Write};
use std::path::Path;

use anyhow::bail;

use super::{
    ParsedSlash, SlashCommandKind, SlashCommandSpec, render_command_help, render_help,
    render_ordered_status, require_slash_goal,
};
use crate::config::Config;
use crate::providers::ChatClient;

/// Commands answered directly after the receipt, before any event is emitted.
/// Returns `None` for commands that run under the completion guard.
pub(super) fn immediate_result(
    command: &SlashCommandSpec,
    words: &[String],
    config: &Config,
) -> Option<anyhow::Result<String>> {
    match command.kind {
        SlashCommandKind::Help => {
            if words.len() > 2 {
                return Some(Err(anyhow::anyhow!("usage: /help [command]")));
            }
            Some(
                words
                    .get(1)
                    .map_or_else(|| Ok(render_help()), |topic| render_command_help(topic)),
            )
        }
        SlashCommandKind::Confirm => Some(Err(anyhow::anyhow!(
            "/confirm is available only after a Gate 1 card; type a plain-text request first"
        ))),
        SlashCommandKind::Status => Some(Ok(render_ordered_status(config))),
        SlashCommandKind::Model
        | SlashCommandKind::Provider
        | SlashCommandKind::Profile
        | SlashCommandKind::Clear
        | SlashCommandKind::Last => Some(Err(anyhow::anyhow!(
            "{} is available only in the interactive REPL",
            command.name
        ))),
        SlashCommandKind::Doctor => Some(Ok(crate::doctor::diagnose(config).render_human())),
        SlashCommandKind::Packs => Some(
            crate::config::configured_extension_root(&config.workspace_root).and_then(
                |extension_root| {
                    crate::pack_actions::render_list(
                        &config.profile,
                        config.resolved_run_intent().as_str(),
                        extension_root.as_deref(),
                    )
                },
            ),
        ),
        SlashCommandKind::Pack => Some(Err(anyhow::anyhow!(
            "/pack is available only from Gate 4; use /pack <id@version>"
        ))),
        SlashCommandKind::Plan => Some(Ok(crate::tui::presentation::render_current_plan())),
        SlashCommandKind::Runs => Some(Ok(crate::runs::render_runs_table(&config.workspace_root))),
        SlashCommandKind::Exit => Some(Ok(String::new())),
        SlashCommandKind::Resume
        | SlashCommandKind::PlanSteps
        | SlashCommandKind::PlanRun
        | SlashCommandKind::RunPlan
        | SlashCommandKind::UltraPlan
        | SlashCommandKind::UltraPlanRun
        | SlashCommandKind::RunUltraPlan
        | SlashCommandKind::SetupInteractionProbe
        | SlashCommandKind::ModelProbe => None,
    }
}

/// Runs a command that `immediate_result` declined, after preflight.
pub(super) fn run_command(
    command: &SlashCommandSpec,
    parsed: &ParsedSlash,
    config: &Config,
    planner: &mut dyn ChatClient,
    execution: &mut dyn ChatClient,
    ui: &dyn crate::tui::InteractionUi,
) -> anyhow::Result<String> {
    crate::preflight::run_for_slash_command(config, &parsed.command, &parsed.goal)?;
    match command.kind {
        SlashCommandKind::PlanSteps => {
            let plan =
                crate::planner::generate_step_plan_with_ui(planner, &parsed.goal, config, ui)?;
            let path = crate::planner::save_step_plan(&config.workspace_root, &plan)?;
            Ok(path.display().to_string())
        }
        SlashCommandKind::PlanRun => crate::planner::generate_and_run_step_plan_with_ui(
            planner,
            execution,
            &parsed.goal,
            config,
            ui,
        ),
        SlashCommandKind::RunPlan => {
            crate::planner::run_plan_file_with_ui(execution, Path::new(&parsed.goal), config, ui)
        }
        SlashCommandKind::UltraPlan => {
            require_slash_goal(&parsed.goal, command.name)?;
            let plan =
                crate::planner::generate_ultra_plan_with_ui(planner, &parsed.goal, config, ui)?;
            let path = crate::planner::save_ultra_plan(&config.workspace_root, &plan)?;
            Ok(path.display().to_string())
        }
        SlashCommandKind::UltraPlanRun => crate::planner::auto_recovery::generate_and_run_with_ui(
            planner,
            execution,
            &parsed.goal,
            config,
            ui,
        ),
        SlashCommandKind::RunUltraPlan => crate::planner::auto_recovery::run_file_with_ui(
            planner,
            execution,
            Path::new(&parsed.goal),
            config,
            ui,
        ),
        SlashCommandKind::Resume => {
            let resume = crate::runs::prepare_resume(&config.workspace_root, &parsed.goal)?;
            if let Some(error) = resume.workspace_drift_error() {
                bail!("{error}");
            }
            confirm_resume(config, &resume)?;
            let progress = crate::tui::presentation::PlanProgress {
                completed_phases: resume.completed_phase_ids.clone(),
                current_phase: None,
                current_step: None,
            };
            crate::tui::presentation::emit_ultra_plan_card(&resume.plan, &progress);
            let plan_card =
                crate::tui::presentation::render_ultra_plan_card(&resume.plan, &progress);
            crate::runs::emit_resume_start(config, &resume);
            let output = crate::planner::auto_recovery::run_plan_with_ui(
                planner,
                execution,
                &resume.plan,
                config,
                ui,
            )?;
            Ok(format!(
                "{}\n\n{}\n\n{}",
                resume.confirmation_card(),
                plan_card,
                output
            ))
        }
        SlashCommandKind::SetupInteractionProbe => {
            let report =
                crate::minimal_loop::interaction_probe::setup_interaction_probe_with_stdout_progress()?;
            Ok(report.summary_lines().join("\n"))
        }
        SlashCommandKind::ModelProbe => {
            let output = crate::model_probe::run_configured(config, planner, execution)?;
            Ok(output.card)
        }
        SlashCommandKind::Help
        | SlashCommandKind::Confirm
        | SlashCommandKind::Status
        | SlashCommandKind::Model
        | SlashCommandKind::Provider
        | SlashCommandKind::Profile
        | SlashCommandKind::Clear
        | SlashCommandKind::Last
        | SlashCommandKind::Doctor
        | SlashCommandKind::Packs
        | SlashCommandKind::Pack
        | SlashCommandKind::Runs
        | SlashCommandKind::Plan
        | SlashCommandKind::Exit => unreachable!("handled before command execution"),
    }
}

fn confirm_resume(config: &Config, resume: &crate::runs::ResumePlan) -> anyhow::Result<()> {
    let card = resume.confirmation_card();
    if config.yes {
        eprintln!("{card}");
        return Ok(());
    }
    if !io::stdin().is_terminal() {
        bail!("{card}\n\nconfirmation required; rerun with --yes");
    }
    eprintln!("{card}");
    eprint!("Resume recovery plan? [y/N] ");
    io::stderr().flush()?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    if matches!(answer.trim(), "y" | "Y" | "yes" | "YES" | "Yes") {
        Ok(())
    } else {
        bail!("resume cancelled by user")
    }
}
