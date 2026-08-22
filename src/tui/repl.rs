use std::io::{self, IsTerminal, Write};
use std::path::Path;

use anyhow::{Context, bail};
use rustyline::error::ReadlineError;

use crate::config::Config;
use crate::planner::pack::catalog::PackLocator;
use crate::tui::editor::{PromptInterruptAction, ReplEditor, normalize_multiline_input};
use crate::tui::markdown::TerminalMarkdownRenderer;
use crate::tui::{InteractionUi, OutputRenderer, TerminalUi};

pub fn run(mut config: Config) -> anyhow::Result<()> {
    let stdin_is_terminal = io::stdin().is_terminal();
    if !stdin_is_terminal {
        if !config.fresh_session
            && let Some(resume) = config.resume.as_deref()
        {
            validate_saved_session_resume(&config.state_dir, resume)?;
        }
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
    let mut active_identity = boundary_shell.restore_latest_terminal()?;
    let mut directive_round = 0_u32;
    let mut editor = ReplEditor::new(&config)?;
    let history_path = crate::tui::history::prepare_workspace_history_path(
        &config.state_dir,
        &config.workspace_root,
    )
    .ok();
    if let Some(history_path) = history_path.as_deref() {
        let _ = editor.load_history(history_path);
    }
    let mut last_result = None;

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
        let words = crate::tui::slash::parse_words(line);
        match words.first().map(String::as_str) {
            Some("/model") => {
                let result = update_executor_model(&config, &words).map(|updated| {
                    config = updated;
                    ui.publish_status(crate::tui::status::UiStatus::from_config(&config));
                    format!(
                        "Executor model set to `{}` for new Gate 1 cards. Existing cards remain unchanged.",
                        config.model
                    )
                });
                render_command_result(&renderer, &mut last_result, result)?;
                ui.reset_interrupt();
                continue;
            }
            Some("/provider") => {
                let result = update_executor_provider(&config, &words).and_then(|updated| {
                    let replacement = crate::providers::client_from_config(&updated, false)?;
                    config = updated;
                    execution = replacement;
                    ui.publish_status(crate::tui::status::UiStatus::from_config(&config));
                    Ok(format!(
                        "Executor provider set to `{}` for new Gate 1 cards. Existing cards remain unchanged.",
                        config.provider.as_str()
                    ))
                });
                render_command_result(&renderer, &mut last_result, result)?;
                ui.reset_interrupt();
                continue;
            }
            Some("/profile") => {
                let result = update_profile(&config, &words).map(|updated| {
                    config = updated;
                    ui.publish_status(crate::tui::status::UiStatus::from_config(&config));
                    format!(
                        "Profile set to `{}` for new Gate 1 cards. Existing cards remain unchanged.",
                        config.profile
                    )
                });
                render_command_result(&renderer, &mut last_result, result)?;
                ui.reset_interrupt();
                continue;
            }
            Some("/clear") => {
                let result = require_no_arguments(&words, "/clear").and_then(|()| clear_screen());
                if let Err(error) = result {
                    render_and_remember(&renderer, &mut last_result, error.to_string())?;
                }
                ui.reset_interrupt();
                continue;
            }
            Some("/last") => {
                if let Err(error) = require_no_arguments(&words, "/last") {
                    render_and_remember(&renderer, &mut last_result, error.to_string())?;
                } else {
                    renderer.render_assistant(
                        last_result
                            .as_deref()
                            .unwrap_or("No previous REPL result is available."),
                    )?;
                }
                ui.reset_interrupt();
                continue;
            }
            _ => {}
        }
        if line == "/packs" {
            let (profile, intent) = active_identity
                .as_ref()
                .map(|identity| (identity.profile.as_str(), identity.intent.as_str()))
                .unwrap_or_else(|| {
                    (
                        config.profile.as_str(),
                        config.resolved_run_intent().as_str(),
                    )
                });
            let extension_root = crate::config::configured_extension_root(&config.workspace_root)?;
            render_command_result(
                &renderer,
                &mut last_result,
                crate::pack_actions::render_list(profile, intent, extension_root.as_deref()),
            )?;
            ui.reset_interrupt();
            continue;
        }
        if line == "/runs" {
            let listed = crate::tui::slash::handle_command(
                line,
                &config,
                &mut *planner,
                &mut *execution,
                &ui,
            )
            .map(|_| {
                crate::runs::render_runs_table_with_current(
                    &config.workspace_root,
                    config.eval_events_path.as_deref(),
                )
            });
            render_command_result(&renderer, &mut last_result, listed)?;
            ui.reset_interrupt();
            continue;
        }
        if line == "/pack" || line.starts_with("/pack ") {
            let selector = line.strip_prefix("/pack").unwrap_or_default().trim();
            let Some(identity) = active_identity.clone() else {
                render_and_remember(
                    &renderer,
                    &mut last_result,
                    "Pack change refused: no failed confirmed run is active at Gate 4.",
                )?;
                ui.reset_interrupt();
                continue;
            };
            let changed = begin_gate_four_pack_change(&mut boundary_shell, &identity, selector);
            let changed = match changed {
                Ok(identity) => identity.clone(),
                Err(error) => {
                    render_and_remember(
                        &renderer,
                        &mut last_result,
                        format!("Pack change refused: {error}"),
                    )?;
                    ui.reset_interrupt();
                    continue;
                }
            };
            let card = crate::tui::boundary_shell::presentation::render_gate_one(
                &changed,
                &PackLocator::new(&config.workspace_root),
            )?;
            crate::tui::boundary_shell::transcript::append(
                &config.state_dir,
                "Gate 1 pack change proposal",
                &card,
            )?;
            render_and_remember(&renderer, &mut last_result, card)?;
            ui.reset_interrupt();
            continue;
        }
        if let Some(raw) = line.strip_prefix("/directive ").map(str::trim) {
            let target_run_id = match boundary_run_id(config.eval_events_path.as_deref()) {
                Ok(run_id) => run_id,
                Err(error) => {
                    render_and_remember(
                        &renderer,
                        &mut last_result,
                        format!("Directive refused: {error}"),
                    )?;
                    ui.reset_interrupt();
                    continue;
                }
            };
            let next_round = match boundary_shell.next_directive_round(&target_run_id) {
                Ok(round) => round.max(directive_round.saturating_add(1)),
                Err(error) => {
                    render_and_remember(
                        &renderer,
                        &mut last_result,
                        format!("Directive refused: {error}"),
                    )?;
                    ui.reset_interrupt();
                    continue;
                }
            };
            let directive = match boundary_shell.begin_directive(raw, &target_run_id, next_round) {
                Ok(directive) => directive,
                Err(error) => {
                    render_and_remember(
                        &renderer,
                        &mut last_result,
                        format!("Directive refused: {error}"),
                    )?;
                    ui.reset_interrupt();
                    continue;
                }
            };
            let gate_label = if directive.artifact().issued_gate == "gate_3" {
                "Gate 3"
            } else {
                "Gate 4"
            };
            let proposal = format!(
                "# {gate_label} — Directive confirmation\n\n\
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
                &format!("{gate_label} directive proposal"),
                &proposal,
            )?;
            render_and_remember(&renderer, &mut last_result, proposal)?;
            ui.reset_interrupt();
            continue;
        }
        if let Some(directive_hash) = line.strip_prefix("/confirm-directive ").map(str::trim) {
            let Some(identity) = active_identity.clone() else {
                render_and_remember(
                    &renderer,
                    &mut last_result,
                    "Directive confirmation refused: no confirmed terminal run is active.",
                )?;
                ui.reset_interrupt();
                continue;
            };
            let directive = match boundary_shell.confirm_directive(directive_hash) {
                Ok(confirmed) => confirmed.directive().clone(),
                Err(error) => {
                    render_and_remember(
                        &renderer,
                        &mut last_result,
                        format!("Directive confirmation refused: {error}"),
                    )?;
                    ui.reset_interrupt();
                    continue;
                }
            };
            let Some(events_path) = config.eval_events_path.as_deref() else {
                render_and_remember(
                    &renderer,
                    &mut last_result,
                    "Directive continuation refused: the failed run has no event stream.",
                )?;
                ui.reset_interrupt();
                continue;
            };
            let continuation = match boundary_shell.prepare_confirmed_continuation(
                &config.workspace_root,
                events_path,
                &identity,
                &directive,
            ) {
                Ok(continuation) => continuation,
                Err(error) => {
                    render_and_remember(
                        &renderer,
                        &mut last_result,
                        format!("Directive continuation refused: {error}"),
                    )?;
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
            render_and_remember(&renderer, &mut last_result, confirmed_text.clone())?;
            crate::tui::boundary_shell::transcript::append(
                &config.state_dir,
                if directive.artifact().issued_gate == "gate_3" {
                    "Gate 3 directive confirmation"
                } else {
                    "Gate 4 directive confirmation"
                },
                &confirmed_text,
            )?;
            let mut dispatch_config = config.clone();
            apply_confirmed_identity(&mut dispatch_config, &identity);
            let command = format!("/run-ultra-plan {}", continuation.plan_workspace_path);
            let result = boundary_shell.dispatch_directive(&continuation, || {
                with_confirmed_pack(&dispatch_config, &identity, || {
                    crate::tui::slash::handle_command(
                        &command,
                        &dispatch_config,
                        &mut *planner,
                        &mut *execution,
                        &ui,
                    )
                })
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
            render_and_remember(&renderer, &mut last_result, rendered)?;
            directive_round = continuation.directive_round;
            ui.reset_interrupt();
            continue;
        }
        if let Some(card_hash) = line.strip_prefix("/confirm ").map(str::trim) {
            let resolved_hash = match resolve_confirmation_hash(
                boundary_shell.state(),
                card_hash,
                strict_confirmation_enabled(),
            ) {
                Ok(hash) => hash,
                Err(error) => {
                    render_and_remember(
                        &renderer,
                        &mut last_result,
                        format!("Confirmation refused: {error}"),
                    )?;
                    ui.reset_interrupt();
                    continue;
                }
            };
            let identity = match boundary_shell.confirm(&resolved_hash) {
                Ok(confirmed) => confirmed.identity().clone(),
                Err(error) => {
                    render_and_remember(
                        &renderer,
                        &mut last_result,
                        format!("Confirmation refused: {error}"),
                    )?;
                    ui.reset_interrupt();
                    continue;
                }
            };
            let confirmed_text = format!(
                "Persisted confirmation: `{}`\n\nDispatching {} × {} × {}.",
                resolved_hash, identity.profile, identity.intent, identity.task_family
            );
            render_and_remember(&renderer, &mut last_result, confirmed_text.clone())?;
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
                with_confirmed_pack(&dispatch_config, &identity, || {
                    crate::tui::slash::handle_command(
                        &command,
                        &dispatch_config,
                        &mut *planner,
                        &mut *execution,
                        &ui,
                    )
                })
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
            render_and_remember(&renderer, &mut last_result, rendered)?;
            active_identity = Some(identity);
            ui.reset_interrupt();
            continue;
        }
        if let Some(error) = resume_preflight_error(&config.workspace_root, line) {
            render_and_remember(
                &renderer,
                &mut last_result,
                format!("Resume unavailable: {error:#}"),
            )?;
            ui.reset_interrupt();
            continue;
        }
        if crate::tui::boundary_shell::execution_slash_requires_gate_one(line) {
            render_and_remember(
                &renderer,
                &mut last_result,
                crate::tui::repl_output::GATE_ONE_REQUIRED_GUIDANCE,
            )?;
            ui.reset_interrupt();
            continue;
        }
        if !line.starts_with('/') {
            let parsed_request = match crate::tui::slash::parse_inline_request(line) {
                Ok(parsed) => parsed,
                Err(error) => {
                    render_and_remember(
                        &renderer,
                        &mut last_result,
                        format!("Gate 1 request refused: {error}"),
                    )?;
                    ui.reset_interrupt();
                    continue;
                }
            };
            let deterministic = crate::tui::boundary_shell::route::deterministic_route(
                crate::tui::boundary_shell::route::RouteRequest {
                    request: &parsed_request.request,
                    workspace: &config.workspace_root,
                    explicit: crate::tui::boundary_shell::route::ExplicitRouteBinding {
                        profile: config
                            .profile_explicit
                            .then(|| crate::planner::profile::ProfileId::parse(&config.profile)),
                        intent: config.intent_override,
                        family: None,
                    },
                },
            );
            let proposal = crate::tui::boundary_shell::ambiguity::propose_route(
                deterministic,
                &parsed_request.request,
                config.provider_label(crate::config::ProviderRole::Planner),
                &config.planner_model,
                &mut *planner,
                &config,
                &|| ui.interrupted(),
            );
            let pins = crate::tui::boundary_shell::confirmation::ExecutionPins {
                planner_provider: config
                    .provider_label(crate::config::ProviderRole::Planner)
                    .to_string(),
                planner_model: config.planner_model.clone(),
                executor_provider: config
                    .provider_label(crate::config::ProviderRole::Executor)
                    .to_string(),
                executor_model: config.model.clone(),
                preset: "profile".to_string(),
            };
            let pack = match (&proposal.selected, parsed_request.pack.as_deref()) {
                (_, None) => crate::tui::boundary_shell::confirmation::PackSelection::None,
                (Some(route), Some(selector)) => {
                    match crate::tui::boundary_shell::pack_catalog::select(
                        route.profile.as_str(),
                        route.intent.as_str(),
                        selector,
                    ) {
                        Ok(pack) => pack,
                        Err(error) => {
                            render_and_remember(
                                &renderer,
                                &mut last_result,
                                format!("Gate 1 request refused: {error}"),
                            )?;
                            ui.reset_interrupt();
                            continue;
                        }
                    }
                }
                (None, Some(_)) => {
                    render_and_remember(
                        &renderer,
                        &mut last_result,
                        "Gate 1 request refused: choose a typed route before selecting a pack.",
                    )?;
                    ui.reset_interrupt();
                    continue;
                }
            };
            let identity = match boundary_shell.begin_gate_one(
                proposal,
                &parsed_request.request,
                &config.workspace_root,
                pins,
                pack,
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
                    render_and_remember(&renderer, &mut last_result, correction)?;
                    ui.reset_interrupt();
                    continue;
                }
            };
            let card = crate::tui::boundary_shell::presentation::render_gate_one(
                identity,
                &PackLocator::new(&config.workspace_root),
            )?;
            crate::tui::boundary_shell::transcript::append(
                &config.state_dir,
                "Gate 1 proposal",
                &card,
            )?;
            render_and_remember(&renderer, &mut last_result, card)?;
            ui.reset_interrupt();
            continue;
        }
        render_command_result(
            &renderer,
            &mut last_result,
            crate::tui::slash::handle_command(line, &config, &mut *planner, &mut *execution, &ui),
        )?;
        ui.reset_interrupt();
    }
    if let Some(history_path) = history_path.as_deref() {
        let _ = editor.save_history(history_path);
    }
    Ok(())
}

fn validate_saved_session_resume(state_dir: &Path, resume: &str) -> anyhow::Result<()> {
    crate::state::SessionStore::new(state_dir.to_path_buf())
        .load(resume)
        .map(|_| ())
        .with_context(|| format!("no resumable saved session `{resume}` could be loaded"))
}

fn resume_preflight_error(root: &Path, line: &str) -> Option<anyhow::Error> {
    let words = crate::tui::slash::parse_words(line);
    if words.first().map(String::as_str) != Some("/resume") {
        return None;
    }
    let target = words[1..].join(" ");
    crate::runs::prepare_resume(root, &target).err()
}

fn update_executor_model(config: &Config, words: &[String]) -> anyhow::Result<Config> {
    let value = single_argument(words, "/model", "<id>")?;
    if value.chars().any(char::is_whitespace) {
        bail!("/model requires one model ID without whitespace");
    }
    if config.provider == crate::config::Provider::Openai {
        crate::openai_model::validate_strict_id(value, "executor")?;
    }
    let mut updated = config.clone();
    updated.model = value.to_string();
    updated.field_sources.model = "repl".to_string();
    Ok(updated)
}

fn update_executor_provider(config: &Config, words: &[String]) -> anyhow::Result<Config> {
    let value = single_argument(words, "/provider", "<name>")?;
    let provider = match value {
        "ollama" => crate::config::Provider::Ollama,
        "lm-studio" => crate::config::Provider::LmStudio,
        "openai" => crate::config::Provider::Openai,
        "gemini" => crate::config::Provider::Gemini,
        _ => bail!("unknown provider `{value}`; expected ollama, lm-studio, openai, or gemini"),
    };
    if provider == crate::config::Provider::Openai {
        crate::openai_model::validate_strict_id(&config.model, "executor")?;
    }
    let mut updated = config.clone();
    updated.provider = provider;
    updated.field_sources.provider = "repl".to_string();
    Ok(updated)
}

fn update_profile(config: &Config, words: &[String]) -> anyhow::Result<Config> {
    let value = single_argument(words, "/profile", "<name>")?;
    let Some(profile) = crate::planner::profile_descriptor::descriptor_for_name(value) else {
        bail!("unknown profile `{value}`; use Tab after `/profile ` to list profiles");
    };
    let mut updated = config.clone();
    updated.profile = profile.canonical.to_string();
    updated.profile_explicit = true;
    updated.profile_inference = None;
    updated.field_sources.profile = "repl".to_string();
    Ok(updated)
}

fn single_argument<'a>(
    words: &'a [String],
    command: &str,
    value_name: &str,
) -> anyhow::Result<&'a str> {
    if words.len() != 2 || words[1].is_empty() {
        bail!("usage: {command} {value_name}");
    }
    Ok(&words[1])
}

fn require_no_arguments(words: &[String], command: &str) -> anyhow::Result<()> {
    if words.len() != 1 {
        bail!("usage: {command}");
    }
    Ok(())
}

fn clear_screen() -> anyhow::Result<()> {
    let mut stdout = io::stdout();
    crossterm::execute!(
        stdout,
        crossterm::cursor::MoveTo(0, 0),
        crossterm::terminal::Clear(crossterm::terminal::ClearType::All)
    )?;
    stdout.flush()?;
    Ok(())
}

fn strict_confirmation_enabled() -> bool {
    crate::env_compat::var("COMMANDAGENT_STRICT_CONFIRM")
        .ok()
        .as_deref()
        == Some("1")
}

fn resolve_confirmation_hash(
    state: &crate::tui::boundary_shell::BoundaryState,
    supplied: &str,
    strict: bool,
) -> anyhow::Result<String> {
    let crate::tui::boundary_shell::BoundaryState::AwaitingConfirmation {
        card_hash: expected,
        ..
    } = state
    else {
        bail!("no Gate 1 proposal is awaiting confirmation");
    };
    if supplied == expected {
        return Ok(expected.clone());
    }
    if strict {
        bail!("COMMANDAGENT_STRICT_CONFIRM=1 requires the full Gate 1 confirmation hash");
    }
    let Some(prefix) = supplied.strip_prefix("sha256:") else {
        bail!("confirmation prefixes must use the `sha256:` form shown on the Gate 1 card");
    };
    if !(8..64).contains(&prefix.len()) {
        bail!("confirmation prefixes require 8 to 63 lowercase hexadecimal digits");
    }
    if !prefix
        .bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        bail!("confirmation prefixes require lowercase hexadecimal digits");
    }
    let Some(expected_digest) = expected.strip_prefix("sha256:") else {
        bail!("the pending Gate 1 card has an invalid confirmation identity");
    };
    if !expected_digest.starts_with(prefix) {
        bail!("confirmation prefix does not match the latest Gate 1 card");
    }
    Ok(expected.clone())
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

fn begin_gate_four_pack_change<'a>(
    boundary_shell: &'a mut crate::tui::boundary_shell::BoundaryShell,
    identity: &crate::tui::boundary_shell::confirmation::ConfirmationIdentity,
    selector: &str,
) -> anyhow::Result<&'a crate::tui::boundary_shell::confirmation::ConfirmationIdentity> {
    if selector.is_empty() {
        bail!("/pack requires <id@version>");
    }
    let pack = crate::tui::boundary_shell::pack_catalog::select(
        &identity.profile,
        &identity.intent,
        selector,
    )?;
    boundary_shell.begin_pack_change(identity, pack)
}

fn with_confirmed_pack<T>(
    config: &Config,
    identity: &crate::tui::boundary_shell::confirmation::ConfirmationIdentity,
    run: impl FnOnce() -> anyhow::Result<T>,
) -> anyhow::Result<T> {
    use crate::tui::boundary_shell::confirmation::PackSelection;

    let PackSelection::Pinned {
        id,
        version,
        hash,
        point,
        source,
    } = &identity.pack
    else {
        return run();
    };
    let locator = PackLocator::new(&config.workspace_root);
    let directory = locator.locate(*source, id, version)?;
    let observed_hash = locator.observed_hash(*source, id, version)?;
    if observed_hash != *hash {
        bail!(
            "confirmed pack changed before dispatch: expected `{hash}`, observed `{observed_hash}`"
        );
    }
    let resolved = crate::cli_pack::ResolvedPack {
        id: id.clone(),
        version: version.clone(),
        hash: hash.clone(),
        source: crate::cli_pack::SelectionSource::Repository,
        directory,
    };
    let _pack_environment = crate::cli_pack::RuntimeEnvironmentGuard::install(Some(&resolved))?;
    crate::eval_events::emit(
        config.eval_events_path.as_deref(),
        serde_json::json!({
            "event": "pack_injected",
            "lifecycle_stage": "tui_confirmed_dispatch",
            "pack_id": id,
            "pack_version": version,
            "pack_hash": hash,
            "pack_point": point,
            "pack_source": source.as_str(),
            "card_hash": identity.card_hash()?,
        }),
    );
    run()
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
    let pack_change_available =
        crate::tui::boundary_shell::pack_catalog::compatible(&identity.profile, &identity.intent)
            .iter()
            .any(|candidate| match &identity.pack {
                crate::tui::boundary_shell::confirmation::PackSelection::None => true,
                crate::tui::boundary_shell::confirmation::PackSelection::Pinned {
                    id,
                    version,
                    ..
                } => candidate.id != id || candidate.version != version,
            });
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
                pack_change_available,
                if pack_change_available {
                    "enter `/pack <id@version>`; returns to Gate 1"
                } else {
                    "no alternative compatible admitted pack"
                },
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

fn render_command_result(
    renderer: &dyn OutputRenderer,
    last_result: &mut Option<String>,
    result: anyhow::Result<String>,
) -> anyhow::Result<()> {
    let output = result.unwrap_or_else(|error| error.to_string());
    render_and_remember(renderer, last_result, output)
}

fn render_and_remember(
    renderer: &dyn OutputRenderer,
    last_result: &mut Option<String>,
    output: impl Into<String>,
) -> anyhow::Result<()> {
    let output = output.into();
    renderer.render_assistant(&output)?;
    *last_result = Some(output);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::adjudication::contract::IntentId;
    use crate::planner::profile::ProfileId;
    use crate::tui::boundary_shell::ambiguity::{
        ClassifierProvenance, ProposalStatus, RouteProposal,
    };
    use crate::tui::boundary_shell::confirmation::{ExecutionPins, PackSelection};
    use crate::tui::boundary_shell::family_catalog::TaskFamilyId;
    use crate::tui::boundary_shell::route::{RouteBasis, RouteCandidate};
    use clap::Parser;

    #[test]
    fn missing_repl_resume_is_reported_before_gate_one() {
        let root = tempfile::tempdir().unwrap();

        let missing = resume_preflight_error(root.path(), "/resume").unwrap();
        let named = resume_preflight_error(root.path(), "/resume missing-run").unwrap();

        assert!(
            missing.to_string().contains("no resumable run exists"),
            "{missing:#}"
        );
        assert!(
            named
                .to_string()
                .contains("no resumable run `missing-run` exists"),
            "{named:#}"
        );
    }

    #[test]
    fn valid_repl_resume_continues_to_the_gate_one_check() {
        let root = tempfile::tempdir().unwrap();
        let plan = root.path().join(".anvil/plans/recover.yaml");
        let events = root
            .path()
            .join(".anvil/runs/018f3333-resumable/events.jsonl");
        std::fs::create_dir_all(plan.parent().unwrap()).unwrap();
        std::fs::create_dir_all(events.parent().unwrap()).unwrap();
        std::fs::write(
            &plan,
            "goal: \"recover\"\nphases:\n  - id: \"repair\"\n    prompt: \"repair\"\n",
        )
        .unwrap();
        std::fs::write(
            &events,
            format!(
                "{}\n",
                serde_json::json!({
                    "event": "run_stop",
                    "ok": false,
                    "status": "failed",
                    "recovery_ultra_plan_path": ".anvil/plans/recover.yaml"
                })
            ),
        )
        .unwrap();

        assert!(
            resume_preflight_error(root.path(), "/resume 018f3333").is_none(),
            "a valid recovery must continue to the existing Gate 1 guard"
        );
    }

    #[test]
    fn non_tty_resume_explains_missing_saved_session() {
        let state = tempfile::tempdir().unwrap();

        let error = validate_saved_session_resume(state.path(), "missing-session").unwrap_err();

        assert!(
            error
                .to_string()
                .contains("no resumable saved session `missing-session` could be loaded"),
            "{error:#}"
        );
    }

    #[test]
    fn non_tty_resume_accepts_an_existing_saved_session_before_tty_check() {
        let state = tempfile::tempdir().unwrap();
        let store = crate::state::SessionStore::new(state.path().to_path_buf());
        let session = crate::state::SessionSnapshot::new();
        store.save(&session).unwrap();

        validate_saved_session_resume(state.path(), &session.id).unwrap();
    }

    fn python_cli_proposal() -> RouteProposal {
        RouteProposal {
            selected: Some(RouteCandidate {
                profile: ProfileId::PythonCli,
                intent: IntentId::Create,
                family: TaskFamilyId::Filter,
                bases: vec![RouteBasis {
                    rule: "fixture",
                    observation: "python CLI filter".to_string(),
                }],
                contract_ref: "docs/python-cli-profile-contract.md",
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

    fn failed_shell(
        root: &std::path::Path,
    ) -> (
        crate::tui::boundary_shell::BoundaryShell,
        crate::tui::boundary_shell::confirmation::ConfirmationIdentity,
    ) {
        let mut shell = crate::tui::boundary_shell::BoundaryShell::new(
            root.join("confirmations"),
            Some(root.join("events.jsonl")),
        );
        shell
            .begin_gate_one(
                python_cli_proposal(),
                "Python CLI filter",
                root,
                pins(),
                PackSelection::None,
            )
            .unwrap();
        let (hash, identity) = match shell.state() {
            crate::tui::boundary_shell::BoundaryState::AwaitingConfirmation {
                card_hash,
                identity,
            } => (card_hash.clone(), identity.clone()),
            state => panic!("unexpected state: {state:?}"),
        };
        shell.confirm(&hash).unwrap();
        shell.dispatch(|_| Ok("failed".to_string())).unwrap();
        shell
            .present_terminal(
                "# sheet\n\n## 5. Stop reason\nfailed".to_string(),
                false,
                Some("failed".to_string()),
            )
            .unwrap();
        (shell, identity)
    }

    #[test]
    fn command_error_is_sent_through_markdown_renderer_once_without_error_prefix() {
        let capture = crate::tui::markdown::capture::start();
        let renderer = TerminalMarkdownRenderer::for_stdout();
        let mut last_result = None;

        render_command_result(
            &renderer,
            &mut last_result,
            Err(crate::tui::repl_output::RenderedCommandError::new(
                "================ TASK FAILED ================".to_string(),
            )
            .into()),
        )
        .unwrap();

        let output = capture.output();
        assert_eq!(output.matches("TASK FAILED").count(), 1, "{output}");
        assert!(!output.contains("error:"), "{output}");
        assert_eq!(
            last_result.as_deref(),
            Some("================ TASK FAILED ================")
        );
    }

    fn session_config() -> Config {
        Config::from_cli(crate::cli::Cli::parse_from([
            "commandagent",
            "--cwd",
            env!("CARGO_MANIFEST_DIR"),
            "--provider",
            "ollama",
            "--model",
            "executor-old",
            "--planner-provider",
            "gemini",
            "--planner-model",
            "planner-fixed",
        ]))
        .unwrap()
    }

    #[test]
    fn session_switches_are_validated_and_preserve_planner_settings() {
        let config = session_config();

        let model = update_executor_model(
            &config,
            &crate::tui::slash::parse_words("/model executor-new"),
        )
        .unwrap();
        assert_eq!(model.model, "executor-new");
        assert_eq!(model.provider, crate::config::Provider::Ollama);
        assert_eq!(model.planner_model, "planner-fixed");
        assert_eq!(model.planner_provider, crate::config::Provider::Gemini);
        assert_eq!(model.field_sources.model, "repl");

        let provider = update_executor_provider(
            &model,
            &crate::tui::slash::parse_words("/provider lm-studio"),
        )
        .unwrap();
        assert_eq!(provider.provider, crate::config::Provider::LmStudio);
        assert_eq!(provider.planner_provider, crate::config::Provider::Gemini);
        assert_eq!(provider.field_sources.provider, "repl");

        let profile = update_profile(
            &provider,
            &crate::tui::slash::parse_words("/profile next.js"),
        )
        .unwrap();
        assert_eq!(profile.profile, "nextjs");
        assert!(profile.profile_explicit);
        assert!(profile.profile_inference.is_none());
        assert_eq!(profile.field_sources.profile, "repl");

        assert!(
            update_executor_provider(
                &config,
                &crate::tui::slash::parse_words("/provider unknown")
            )
            .is_err()
        );
        assert!(
            update_profile(&config, &crate::tui::slash::parse_words("/profile missing")).is_err()
        );
        assert!(update_executor_model(&config, &crate::tui::slash::parse_words("/model")).is_err());
    }

    #[test]
    fn confirmation_accepts_only_matching_bounded_prefix_unless_strict() {
        let root = tempfile::tempdir().unwrap();
        let mut shell = crate::tui::boundary_shell::BoundaryShell::new(
            root.path().join("confirmations"),
            Some(root.path().join("events.jsonl")),
        );
        shell
            .begin_gate_one(
                python_cli_proposal(),
                "Python CLI filter",
                root.path(),
                pins(),
                PackSelection::None,
            )
            .unwrap();
        let expected = match shell.state() {
            crate::tui::boundary_shell::BoundaryState::AwaitingConfirmation {
                card_hash, ..
            } => card_hash.clone(),
            state => panic!("unexpected state: {state:?}"),
        };
        let digest = expected.strip_prefix("sha256:").unwrap();
        let prefix = format!("sha256:{}", &digest[..8]);

        assert_eq!(
            resolve_confirmation_hash(shell.state(), &prefix, false).unwrap(),
            expected
        );
        assert_eq!(
            resolve_confirmation_hash(shell.state(), &expected, true).unwrap(),
            expected
        );
        assert!(resolve_confirmation_hash(shell.state(), &prefix, true).is_err());
        assert!(
            resolve_confirmation_hash(shell.state(), &format!("sha256:{}", &digest[..7]), false)
                .is_err()
        );
        assert!(resolve_confirmation_hash(shell.state(), "sha256:00000000", false).is_err());
        assert!(resolve_confirmation_hash(shell.state(), "sha256:ABCDEF12", false).is_err());

        let expanded = resolve_confirmation_hash(shell.state(), &prefix, false).unwrap();
        let confirmed = shell.confirm(&expanded).unwrap();
        assert_eq!(confirmed.card_hash(), expected);
    }

    #[test]
    fn gate_four_pack_change_creates_a_new_gate_one_boundary() {
        let root = tempfile::tempdir().unwrap();
        let (mut shell, identity) = failed_shell(root.path());
        let changed =
            begin_gate_four_pack_change(&mut shell, &identity, "cli-assist@1.1.0").unwrap();
        assert_eq!(changed.request, identity.request);
        assert_eq!(changed.pins, identity.pins);
        assert!(matches!(
            &changed.pack,
            PackSelection::Pinned { id, version, .. }
                if id == "cli-assist" && version == "1.1.0"
        ));
        let card = crate::tui::boundary_shell::presentation::render_gate_one(
            changed,
            &PackLocator::new(env!("CARGO_MANIFEST_DIR")),
        )
        .unwrap();
        assert!(card.contains("cli-assist@1.1.0"), "{card}");
        assert!(
            card.contains(
                "sha256:3d11e126d3afbcd8a53e23367d53859924c700aeaf5345fa366060d66c917c82"
            ),
            "{card}"
        );
        assert!(card.contains("検証パックの供給元: 承認済み"), "{card}");
        assert!(card.contains("検証箇所: cli-validation"), "{card}");
        assert!(matches!(
            shell.state(),
            crate::tui::boundary_shell::BoundaryState::AwaitingConfirmation { .. }
        ));
        let events = std::fs::read_to_string(root.path().join("events.jsonl")).unwrap();
        assert!(events.contains("\"action\":\"pack_change\""), "{events}");
        assert!(events.contains("\"classifier_parse_reason\":\"gate_4_pack_change\""));
    }

    #[test]
    fn confirmed_repl_pack_is_installed_for_dispatch_and_recorded() {
        let root = tempfile::tempdir().unwrap();
        let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut config = Config::from_cli(crate::cli::Cli::parse_from([
            "commandagent",
            "--cwd",
            repository.to_str().unwrap(),
            "--profile",
            "python-cli",
            "--intent",
            "create",
        ]))
        .unwrap();
        config.eval_events_path = Some(root.path().join("events.jsonl"));
        let identity = crate::tui::boundary_shell::confirmation::ConfirmationIdentity::new(
            "Python CLI filter".to_string(),
            repository,
            python_cli_proposal().selected.as_ref().unwrap(),
            crate::tui::boundary_shell::band_catalog::value_for(
                "python-cli",
                IntentId::Create,
                TaskFamilyId::Filter,
            )
            .unwrap(),
            pins(),
            crate::tui::boundary_shell::pack_catalog::select(
                "python-cli",
                "create",
                "cli-assist@1.1.0",
            )
            .unwrap(),
        )
        .unwrap();
        let previous = std::env::var_os("COMMANDAGENT_PACK_ID");
        with_confirmed_pack(&config, &identity, || {
            assert_eq!(std::env::var("COMMANDAGENT_PACK_ID").unwrap(), "cli-assist");
            assert_eq!(std::env::var("COMMANDAGENT_PACK_VERSION").unwrap(), "1.1.0");
            Ok(())
        })
        .unwrap();
        assert_eq!(std::env::var_os("COMMANDAGENT_PACK_ID"), previous);
        let events = std::fs::read_to_string(config.eval_events_path.unwrap()).unwrap();
        assert!(events.contains("\"event\":\"pack_injected\""), "{events}");
        assert!(events.contains("\"pack_id\":\"cli-assist\""), "{events}");
        assert!(events.contains("\"pack_version\":\"1.1.0\""), "{events}");
    }
}
