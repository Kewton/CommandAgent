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
    let mut active_identity: Option<
        crate::tui::boundary_shell::confirmation::ConfirmationIdentity,
    > = None;
    let mut directive_round = 0_u32;
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
        if let Some(raw) = line.strip_prefix("/directive ").map(str::trim) {
            let target_run_id = match boundary_run_id(config.eval_events_path.as_deref()) {
                Ok(run_id) => run_id,
                Err(error) => {
                    renderer.render_assistant(&format!("Directive refused: {error}"))?;
                    ui.reset_interrupt();
                    continue;
                }
            };
            let directive = match boundary_shell.begin_directive(
                raw,
                &target_run_id,
                directive_round.saturating_add(1),
            ) {
                Ok(directive) => directive,
                Err(error) => {
                    renderer.render_assistant(&format!("Directive refused: {error}"))?;
                    ui.reset_interrupt();
                    continue;
                }
            };
            let proposal = format!(
                "# Gate 4 — Directive confirmation\n\n\
- Directive: {}\n\
- Directive hash: {}\n\
- Target run ID: {}\n\
- Directive round: {}\n\
- Source: human_directive (bounded verbatim)\n\
- Contract floor: unchanged\n\n\
Confirm with `/confirm-directive {}` before continuation dispatch.",
                directive.artifact().raw,
                directive.hash(),
                directive.artifact().target_run_id,
                directive.artifact().round,
                directive.hash(),
            );
            crate::tui::boundary_shell::transcript::append(
                &config.state_dir,
                "Gate 4 directive proposal",
                &proposal,
            )?;
            renderer.render_assistant(&proposal)?;
            ui.reset_interrupt();
            continue;
        }
        if let Some(directive_hash) = line.strip_prefix("/confirm-directive ").map(str::trim) {
            let Some(identity) = active_identity.clone() else {
                renderer.render_assistant(
                    "Directive confirmation refused: no failed confirmed run is active.",
                )?;
                ui.reset_interrupt();
                continue;
            };
            let directive = match boundary_shell.confirm_directive(directive_hash) {
                Ok(confirmed) => confirmed.directive().clone(),
                Err(error) => {
                    renderer
                        .render_assistant(&format!("Directive confirmation refused: {error}"))?;
                    ui.reset_interrupt();
                    continue;
                }
            };
            let Some(events_path) = config.eval_events_path.as_deref() else {
                renderer.render_assistant(
                    "Directive continuation refused: the failed run has no event stream.",
                )?;
                ui.reset_interrupt();
                continue;
            };
            let continuation = match crate::tui::boundary_shell::directive::prepare_continuation(
                &config.workspace_root,
                events_path,
                &directive,
            ) {
                Ok(continuation) => continuation,
                Err(error) => {
                    renderer
                        .render_assistant(&format!("Directive continuation refused: {error}"))?;
                    ui.reset_interrupt();
                    continue;
                }
            };
            let confirmed_text = format!(
                "Persisted directive confirmation: `{}`\n\nContinuing target run `{}` at directive round {} in the same workspace.",
                continuation.directive_hash,
                continuation.target_run_id,
                continuation.directive_round,
            );
            renderer.render_assistant(&confirmed_text)?;
            crate::tui::boundary_shell::transcript::append(
                &config.state_dir,
                "Gate 4 directive confirmation",
                &confirmed_text,
            )?;
            let mut dispatch_config = config.clone();
            apply_confirmed_identity(&mut dispatch_config, &identity);
            let command = format!("/run-ultra-plan {}", continuation.plan_workspace_path);
            let result = boundary_shell.dispatch_directive(&continuation, || {
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
            let generated = crate::tui::boundary_shell::sheet::with_directive_metadata(
                generated,
                &continuation,
            );
            let sheet_path = crate::tui::boundary_shell::sheet::persist_directive_round(
                &config.state_dir,
                &identity,
                &generated,
                continuation.directive_round,
            )?;
            let terminal = boundary_shell.present_terminal(
                generated.markdown,
                generated.full,
                generated.section5,
            )?;
            let rendered = render_terminal(&identity, terminal)?;
            let gate = if terminal.full { "Gate 3" } else { "Gate 4" };
            crate::tui::boundary_shell::transcript::append(
                &config.state_dir,
                gate,
                &format!("{rendered}\n\nSheet path: {}", sheet_path.display()),
            )?;
            renderer.render_assistant(&rendered)?;
            directive_round = continuation.directive_round;
            ui.reset_interrupt();
            continue;
        }
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
            apply_confirmed_identity(&mut dispatch_config, &identity);
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
            let rendered = render_terminal(&identity, terminal)?;
            let gate = if terminal.full { "Gate 3" } else { "Gate 4" };
            crate::tui::boundary_shell::transcript::append(
                &config.state_dir,
                gate,
                &format!("{rendered}\n\nSheet path: {}", sheet_path.display()),
            )?;
            renderer.render_assistant(&rendered)?;
            active_identity = Some(identity);
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

fn apply_confirmed_identity(
    config: &mut Config,
    identity: &crate::tui::boundary_shell::confirmation::ConfirmationIdentity,
) {
    config.profile = identity.profile.clone();
    config.profile_explicit = true;
    config.profile_inference = None;
    config.intent_override = match identity.intent.as_str() {
        "create" => Some(crate::config::IntentId::Create),
        "fix" => Some(crate::config::IntentId::Fix),
        "investigate" => Some(crate::config::IntentId::Investigate),
        _ => None,
    };
    config.plan_preset = crate::config::PlanPreset::Profile;
}

fn boundary_run_id(events_path: Option<&std::path::Path>) -> anyhow::Result<String> {
    events_path
        .and_then(std::path::Path::parent)
        .and_then(std::path::Path::file_name)
        .and_then(std::ffi::OsStr::to_str)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| anyhow::anyhow!("the failed run has no stable event-stream run ID"))
}

fn render_terminal(
    identity: &crate::tui::boundary_shell::confirmation::ConfirmationIdentity,
    terminal: &crate::tui::boundary_shell::acceptance::TerminalPresentation,
) -> anyhow::Result<String> {
    if terminal.full {
        return crate::tui::boundary_shell::presentation::render_gate_three(identity, terminal);
    }
    crate::tui::boundary_shell::presentation::render_gate_four(
        identity,
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
                crate::tui::boundary_shell::acceptance::NextAction::HumanDirective,
                true,
                "enter `/directive <instruction>`; persisted confirmation is required",
            ),
            (
                crate::tui::boundary_shell::acceptance::NextAction::Close,
                true,
                "records no further action",
            ),
        ],
    )
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
