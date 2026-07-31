use std::io::{self, IsTerminal};

use anyhow::bail;
use rustyline::error::ReadlineError;

use crate::config::Config;
use crate::tui::editor::{PromptInterruptAction, ReplEditor, normalize_multiline_input};
use crate::tui::markdown::TerminalMarkdownRenderer;
use crate::tui::{InteractionUi, OutputRenderer, TerminalUi};

pub fn run(config: Config) -> anyhow::Result<()> {
    let stdin_is_terminal = io::stdin().is_terminal();
    if !stdin_is_terminal {
        bail!("stdin is not a TTY; pass --prompt or an action flag");
    }

    crate::tui::banner::print_startup_banner(&config)?;
    for warning in crate::providers::startup::warnings(&config, stdin_is_terminal) {
        eprintln!("{warning}");
    }
    let ui = TerminalUi::new_with_input_queue(&config);
    let renderer = TerminalMarkdownRenderer::for_stdout();
    let mut execution = crate::providers::client_from_config(&config, false)?;
    let mut planner = crate::providers::client_from_config(&config, true)?;
    let mut boundary_shell = crate::tui::boundary_shell::BoundaryShell::new(
        config.state_dir.join("boundary-confirmations"),
        config.eval_events_path.clone(),
    );
    let mut editor = ReplEditor::new(&config)?;
    let history_path = config.state_dir.join("history.txt");
    if let Some(parent) = history_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = editor.load_history(&history_path);

    loop {
        let line = if let Some(line) = ui.take_queued_input() {
            eprintln!(
                "processing queued: {}",
                crate::tui::input_queue::preview(&line)
            );
            line
        } else {
            let _prompt_guard = ui.pause_for_prompt();
            let pending = ui.take_pending_input().unwrap_or_default();
            match editor.readline_with_initial("commandagent> ", (&pending, "")) {
                Ok(line) => line,
                Err(ReadlineError::Interrupted) => match editor.take_interrupt_action() {
                    PromptInterruptAction::ClearLine => continue,
                    PromptInterruptAction::WarnBeforeExit => {
                        eprintln!("press Ctrl+C again to exit");
                        continue;
                    }
                    PromptInterruptAction::Exit => break,
                },
                Err(ReadlineError::Eof) => break,
                Err(err) => return Err(err.into()),
            }
        };
        let line = normalize_multiline_input(&line);
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if crate::tui::slash::is_exit_command(line) {
            break;
        }
        let _ = editor.add_history_entry(line);
        if let Some(card_hash) = line.strip_prefix("/confirm ").map(str::trim) {
            let identity = match boundary_shell.confirm(card_hash) {
                Ok(confirmed) => confirmed.identity().clone(),
                Err(error) => {
                    renderer.render_assistant(&format!("Confirmation refused: {error}"))?;
                    ui.reset_interrupt();
                    continue;
                }
            };
            let confirmed_text = format!(
                "Persisted confirmation: `{}`\n\nDispatching {} × {} × {}.",
                card_hash, identity.profile, identity.intent, identity.task_family
            );
            renderer.render_assistant(&confirmed_text)?;
            crate::tui::boundary_shell::transcript::append(
                &config.state_dir,
                "Gate 1 confirmation",
                &confirmed_text,
            )?;
            let mut dispatch_config = config.clone();
            dispatch_config.profile = identity.profile.clone();
            dispatch_config.profile_explicit = true;
            dispatch_config.profile_inference = None;
            dispatch_config.intent_override = match identity.intent.as_str() {
                "create" => Some(crate::config::IntentId::Create),
                "fix" => Some(crate::config::IntentId::Fix),
                "investigate" => Some(crate::config::IntentId::Investigate),
                _ => None,
            };
            dispatch_config.plan_preset = crate::config::PlanPreset::Profile;
            let command = format!(
                "/ultra-plan-run --profile {} {}",
                identity.profile, identity.request
            );
            let result = boundary_shell.dispatch(|_| {
                crate::tui::slash::handle_command(
                    &command,
                    &dispatch_config,
                    &mut *planner,
                    &mut *execution,
                    &ui,
                )
            });
            let generated = crate::tui::boundary_shell::sheet::generate(
                &identity,
                config.eval_events_path.as_deref(),
                result.is_ok(),
            )?;
            let sheet_path = crate::tui::boundary_shell::sheet::persist(
                &config.state_dir,
                &identity,
                &generated,
            )?;
            let terminal = boundary_shell.present_terminal(
                generated.markdown,
                generated.full,
                generated.section5,
            )?;
            let rendered = if terminal.full {
                crate::tui::boundary_shell::presentation::render_gate_three(&identity, terminal)?
            } else {
                crate::tui::boundary_shell::presentation::render_gate_four(
                    &identity,
                    terminal,
                    &[
                        (
                            crate::tui::boundary_shell::acceptance::NextAction::Retry,
                            true,
                            "human confirmation required",
                        ),
                        (
                            crate::tui::boundary_shell::acceptance::NextAction::RecoveryCircle,
                            false,
                            "availability must be earned by workflow evidence",
                        ),
                        (
                            crate::tui::boundary_shell::acceptance::NextAction::ElevatedModel,
                            true,
                            "returns to Gate 1 with a new model pin",
                        ),
                        (
                            crate::tui::boundary_shell::acceptance::NextAction::PackChange,
                            false,
                            "no pack selected for this confirmed run",
                        ),
                        (
                            crate::tui::boundary_shell::acceptance::NextAction::Close,
                            true,
                            "records no further action",
                        ),
                    ],
                )?
            };
            let gate = if terminal.full { "Gate 3" } else { "Gate 4" };
            crate::tui::boundary_shell::transcript::append(
                &config.state_dir,
                gate,
                &format!("{rendered}\n\nSheet path: {}", sheet_path.display()),
            )?;
            renderer.render_assistant(&rendered)?;
            ui.reset_interrupt();
            continue;
        }
        if crate::tui::boundary_shell::execution_slash_requires_gate_one(line) {
            renderer.render_assistant(
                "D-3c Gate 1 confirmation is required before this REPL execution command.",
            )?;
            ui.reset_interrupt();
            continue;
        }
        if !line.starts_with('/') {
            let deterministic = crate::tui::boundary_shell::route::deterministic_route(
                crate::tui::boundary_shell::route::RouteRequest {
                    request: line,
                    workspace: &config.workspace_root,
                    explicit: crate::tui::boundary_shell::route::ExplicitRouteBinding::default(),
                },
            );
            let proposal = crate::tui::boundary_shell::ambiguity::propose_route(
                deterministic,
                line,
                provider_name(config.planner_provider),
                &config.planner_model,
                &mut *planner,
                &config,
                &|| ui.interrupted(),
            );
            let pins = crate::tui::boundary_shell::confirmation::ExecutionPins {
                planner_provider: provider_name(config.planner_provider).to_string(),
                planner_model: config.planner_model.clone(),
                executor_provider: provider_name(config.provider).to_string(),
                executor_model: config.model.clone(),
                preset: "profile".to_string(),
            };
            let identity = match boundary_shell.begin_gate_one(
                proposal,
                line,
                &config.workspace_root,
                pins,
                crate::tui::boundary_shell::confirmation::PackSelection::None,
            ) {
                Ok(identity) => identity,
                Err(error) => {
                    let correction = format!(
                        "Route remains typed unknown; correct or clarify the request before Gate 1: {error}"
                    );
                    crate::tui::boundary_shell::transcript::append(
                        &config.state_dir,
                        "Route correction required",
                        &correction,
                    )?;
                    renderer.render_assistant(&correction)?;
                    ui.reset_interrupt();
                    continue;
                }
            };
            let card = crate::tui::boundary_shell::presentation::render_gate_one(
                identity,
                std::path::Path::new(env!("CARGO_MANIFEST_DIR")),
            )?;
            crate::tui::boundary_shell::transcript::append(
                &config.state_dir,
                "Gate 1 proposal",
                &card,
            )?;
            renderer.render_assistant(&card)?;
            ui.reset_interrupt();
            continue;
        }
        render_command_result(
            &renderer,
            crate::tui::slash::handle_command(line, &config, &mut *planner, &mut *execution, &ui),
        )?;
        ui.reset_interrupt();
    }
    let _ = editor.save_history(&history_path);
    Ok(())
}

fn provider_name(provider: crate::config::Provider) -> &'static str {
    match provider {
        crate::config::Provider::Ollama => "ollama",
        crate::config::Provider::Openai => "openai",
        crate::config::Provider::Gemini => "gemini",
    }
}

fn render_command_result(
    renderer: &dyn OutputRenderer,
    result: anyhow::Result<String>,
) -> anyhow::Result<()> {
    match result {
        Ok(output) => renderer.render_assistant(&output),
        Err(err) => renderer.render_assistant(&err.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_error_is_sent_through_markdown_renderer_once_without_error_prefix() {
        let capture = crate::tui::markdown::capture::start();
        let renderer = TerminalMarkdownRenderer::for_stdout();

        render_command_result(
            &renderer,
            Err(crate::tui::repl_output::RenderedCommandError::new(
                "================ TASK FAILED ================".to_string(),
            )
            .into()),
        )
        .unwrap();

        let output = capture.output();
        assert_eq!(output.matches("TASK FAILED").count(), 1, "{output}");
        assert!(!output.contains("error:"), "{output}");
    }
}
