use std::io::{self, IsTerminal};

use anyhow::bail;
use rustyline::error::ReadlineError;

use crate::config::Config;
use crate::tui::editor::{PromptInterruptAction, ReplEditor, normalize_multiline_input};
use crate::tui::markdown::TerminalMarkdownRenderer;
use crate::tui::{OutputRenderer, TerminalUi};

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
        if crate::tui::boundary_shell::execution_slash_requires_gate_one(line) {
            renderer.render_assistant(
                "D-3c Gate 1 confirmation is required before this REPL execution command.",
            )?;
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
