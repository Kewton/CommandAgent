#[path = "repl/pack_selection.rs"]
pub mod pack_selection;

use std::io::{self, IsTerminal, Write};
use std::path::Path;

use anyhow::{Context, bail};
use rustyline::error::ReadlineError;

use crate::config::Config;
use crate::tools::profile_tools::pre_run::refused_before_gate_one;
use crate::tui::editor::{PromptInterruptAction, ReplEditor, normalize_multiline_input};
use crate::tui::markdown::TerminalMarkdownRenderer;
use crate::tui::{InteractionUi, OutputRenderer, TerminalUi};

pub fn run(config: Config) -> anyhow::Result<()> {
    let context = pack_selection::ReplPackContext::from_config(&config);
    run_with_pack_context(config, context)
}

pub(crate) fn run_with_pack_context(
    mut config: Config,
    pack_context: pack_selection::ReplPackContext,
) -> anyhow::Result<()> {
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
    let mut boundary_shell = boundary_shell_for(&config);
    let mut active_identity = boundary_shell.restore_latest_terminal()?;
    // The exact directory the latest Gate 1 card froze, if it pinned a pack. Its
    // confirmation revalidates this directory, not a re-searched selector.
    let mut active_pack_directory: Option<std::path::PathBuf> = None;
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
        crate::structured_budget::outcome::begin_command(&config);
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
            // One line per selector a card can choose, resolved through the same
            // locator a card uses, so the list and a card cannot read different
            // supply sources for one selector (Issue #1057).
            render_command_result(
                &renderer,
                &mut last_result,
                Ok(crate::pack_actions::render_repl_list(
                    profile,
                    intent,
                    pack_context.locator(),
                )),
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
            let changed = begin_gate_four_pack_change(
                &mut boundary_shell,
                &identity,
                selector,
                &pack_context,
            );
            let (changed, changed_directory) = match changed {
                Ok(result) => result,
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
            let changed = changed.clone();
            let card = crate::tui::boundary_shell::presentation::render_gate_one(
                &changed,
                pack_context.locator(),
            )?;
            crate::tui::boundary_shell::transcript::append(
                &config.state_dir,
                "Gate 1 pack change proposal",
                &card,
            )?;
            render_and_remember(&renderer, &mut last_result, card)?;
            active_pack_directory = changed_directory;
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
            let directive = match begin_directive_for(
                &mut boundary_shell,
                active_identity.as_ref(),
                raw,
                &target_run_id,
                next_round,
            ) {
                Ok(directive) => directive,
                Err(refused) => {
                    render_and_remember(&renderer, &mut last_result, refused)?;
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
            let directive =
                match confirm_directive_for(&mut boundary_shell, &identity, directive_hash) {
                    Ok(directive) => directive,
                    Err(refused) => {
                        render_and_remember(&renderer, &mut last_result, refused)?;
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
                with_confirmed_pack_in(&pack_context, &dispatch_config, &identity, || {
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
            let rendered = present_and_record_terminal(
                &config,
                &identity,
                terminal,
                &pack_context,
                &sheet_path,
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
            let identity = match confirm_gate_one(
                &mut boundary_shell,
                &config,
                &pack_context,
                active_pack_directory.as_deref(),
                &resolved_hash,
            ) {
                Ok(identity) => identity,
                Err(refused) => {
                    render_and_remember(&renderer, &mut last_result, refused)?;
                    // The refused card stays unconfirmed; the next card needs a
                    // fresh confirmation. No dispatch or terminal follows.
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
                with_confirmed_pack_in(&pack_context, &dispatch_config, &identity, || {
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
            let rendered = present_and_record_terminal(
                &config,
                &identity,
                terminal,
                &pack_context,
                &sheet_path,
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
            if let Some(refused) = pack_context.startup_refusal() {
                render_and_remember(
                    &renderer,
                    &mut last_result,
                    format!("Gate 1 request refused: {refused}"),
                )?;
                ui.reset_interrupt();
                continue;
            }
            if let Some(refused) = refused_before_gate_one(&config, &*execution) {
                render_and_remember(&renderer, &mut last_result, refused)?;
                ui.reset_interrupt();
                continue;
            }
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
            let (proposal, pack) = match route_gate_one(
                &config,
                &pack_context,
                &parsed_request,
                &mut *planner,
                &|| ui.interrupted(),
            ) {
                Ok(routed) => routed,
                Err(refused) => {
                    render_and_remember(&renderer, &mut last_result, refused)?;
                    ui.reset_interrupt();
                    continue;
                }
            };
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
                think: None,
            };
            let identity = match boundary_shell.begin_gate_one_with_locator_and_recovery(
                proposal,
                &parsed_request.request,
                &config.workspace_root,
                pins,
                pack.selection.clone(),
                crate::tui::boundary_shell::GateOneOptions {
                    locator: Some(pack_context.locator()),
                    recovery_plan_auto_runs: config.recovery_plan_auto_runs,
                },
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
                pack_context.locator(),
            )?;
            crate::tui::boundary_shell::transcript::append(
                &config.state_dir,
                "Gate 1 proposal",
                &card,
            )?;
            render_and_remember(&renderer, &mut last_result, card)?;
            // Keep the exact directory this card froze, so its confirmation
            // revalidates that directory rather than re-searching the selector.
            active_pack_directory = pack.directory;
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

/// Gate 1 up to its card: the routed proposal and the pack it freezes, or the refusal the
/// REPL shows.
fn route_gate_one(
    config: &Config,
    pack_context: &pack_selection::ReplPackContext,
    parsed_request: &crate::tui::slash::ParsedInlineRequest,
    planner: &mut dyn crate::providers::ChatClient,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<
    (
        crate::tui::boundary_shell::ambiguity::RouteProposal,
        GateOnePack,
    ),
    String,
> {
    if let Some(refused) = parsed_request
        .pack
        .as_deref()
        .and_then(|selector| refused_gate_one_pack(pack_context, selector))
    {
        return Err(format!("Gate 1 request refused: {refused}"));
    }
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
        planner,
        config,
        is_cancelled,
    );
    let pack = gate_one_pack(
        pack_context,
        proposal.selected.as_ref(),
        parsed_request.pack.clone(),
    )
    .map_err(|reason| format!("Gate 1 request refused: {reason}"))?;
    Ok((proposal, pack))
}

/// The pack a Gate 1 card froze: the selection, and the exact directory that
/// answered for it. The directory is compared again before the confirmation is
/// recorded, so a selector that moved to another directory with the same bytes
/// cannot be confirmed under the old card (Issue #1057).
#[derive(Debug, Clone)]
struct GateOnePack {
    selection: crate::tui::boundary_shell::confirmation::PackSelection,
    directory: Option<std::path::PathBuf>,
}

impl GateOnePack {
    fn none() -> Self {
        Self {
            selection: crate::tui::boundary_shell::confirmation::PackSelection::None,
            directory: None,
        }
    }
}

/// The pack a Gate 1 request freezes, given the route it was proposed and the
/// selector it named. This is the one place a card reads its pack, so a card,
/// the `/packs` list, and the confirmation all resolve the same bytes.
fn gate_one_pack(
    pack_context: &pack_selection::ReplPackContext,
    route: Option<&crate::tui::boundary_shell::route::RouteCandidate>,
    selector: Option<String>,
) -> Result<GateOnePack, String> {
    match (route, selector.as_deref()) {
        // A REPL started with `--pack` fixes that pack on a request that names
        // none (the 2026-11-21 decision in Issue #1057). A held pack the route
        // cannot read is refused with its reason; a request without a held pack
        // stays as it was: no pack.
        (Some(route), None) => match pack_context.startup_selection() {
            Ok(None) => Ok(GateOnePack::none()),
            Ok(Some((located, selection))) => {
                if let Some(reason) = pack_selection::compatibility_refusal(
                    &located,
                    route.profile.as_str(),
                    route.intent.as_str(),
                ) {
                    return Err(reason);
                }
                Ok(GateOnePack {
                    selection,
                    directory: Some(located.directory),
                })
            }
            Err(error) => Err(pack_selection::refusal_reason(&error)),
        },
        (Some(route), Some(selector)) => {
            let (selection, directory) = pack_context
                .select_frozen(route.profile.as_str(), route.intent.as_str(), selector)
                .map_err(|error| pack_selection::refusal_reason(&error))?;
            Ok(GateOnePack {
                selection,
                directory: Some(directory),
            })
        }
        (None, Some(_)) => Err("choose a typed route before selecting a pack.".to_string()),
        (None, None) => Ok(GateOnePack::none()),
    }
}

/// `/confirm`: persists the Gate 1 card awaiting confirmation, or returns the refusal the
/// REPL shows. The pack the card froze is revalidated against its exact directory
/// before anything is recorded; a pack that changed or moved is refused with the
/// reason, nothing is confirmed, and a new card is needed (Issue #1057).
fn confirm_gate_one(
    boundary_shell: &mut crate::tui::boundary_shell::BoundaryShell,
    config: &Config,
    pack_context: &pack_selection::ReplPackContext,
    active_pack_directory: Option<&std::path::Path>,
    card_hash: &str,
) -> Result<crate::tui::boundary_shell::confirmation::ConfirmationIdentity, String> {
    if let crate::tui::boundary_shell::BoundaryState::AwaitingConfirmation { identity, .. } =
        boundary_shell.state()
        && let Some(refused) = refused_for_held_pack(&identity.pack)
    {
        return Err(format!("Confirmation refused: {refused}"));
    }
    let _ = config;
    boundary_shell
        .confirm_with_validation(card_hash, |identity| {
            revalidate_confirmed_pack(pack_context, identity, active_pack_directory).map_err(
                |error| {
                    anyhow::anyhow!(
                        "the pack changed since the Gate 1 card was shown; render Gate 1 again: {}",
                        pack_selection::refusal_reason(&error)
                    )
                },
            )
        })
        .map(|confirmed| confirmed.identity().clone())
        .map_err(|error| format!("Confirmation refused: {error}"))
}

/// The pack-level revalidation a Gate 1 confirmation makes before recording.
fn revalidate_confirmed_pack(
    pack_context: &pack_selection::ReplPackContext,
    identity: &crate::tui::boundary_shell::confirmation::ConfirmationIdentity,
    active_pack_directory: Option<&std::path::Path>,
) -> anyhow::Result<()> {
    let crate::tui::boundary_shell::confirmation::PackSelection::Pinned { .. } = &identity.pack
    else {
        return Ok(());
    };
    let Some(directory) = active_pack_directory else {
        // A card restored without a frozen directory cannot be revalidated
        // against it; fall back to the roots so a moved/removed pack is still
        // refused rather than confirmed.
        return pack_context.validate(&identity.profile, &identity.intent, &identity.pack);
    };
    pack_context.revalidate_frozen(
        &identity.profile,
        &identity.intent,
        &identity.pack,
        directory,
    )
}

/// The boundary shell of the REPL's state directory and event stream, in its collecting
/// state.
fn boundary_shell_for(config: &Config) -> crate::tui::boundary_shell::BoundaryShell {
    crate::tui::boundary_shell::BoundaryShell::new(
        config.state_dir.join("boundary-confirmations"),
        config.eval_events_path.clone(),
    )
}

/// `/directive`: proposes a directive for the run of `identity` at Gate 3 or Gate 4, or
/// returns the refusal the REPL shows.
fn begin_directive_for<'a>(
    boundary_shell: &'a mut crate::tui::boundary_shell::BoundaryShell,
    identity: Option<&crate::tui::boundary_shell::confirmation::ConfirmationIdentity>,
    raw: &str,
    target_run_id: &str,
    round: u32,
) -> Result<&'a crate::tui::boundary_shell::directive::PersistedDirective, String> {
    // A run of another pack can never be continued while this pack is held, so the
    // directive is refused before a proposal, its artifact, or its session round exists,
    // and the shell stays at its gate.
    if let Some(refused) = identity.and_then(|identity| refused_for_held_pack(&identity.pack)) {
        return Err(format!("Directive refused: {refused}"));
    }
    boundary_shell
        .begin_directive(raw, target_run_id, round)
        .map_err(|error| format!("Directive refused: {error}"))
}

/// `/confirm-directive`: persists the directive awaiting confirmation for the run of
/// `identity`, or returns the refusal the REPL shows. `/directive` already refuses a run
/// of a pack other than the held one; this check is the last guard and changes no state.
fn confirm_directive_for(
    boundary_shell: &mut crate::tui::boundary_shell::BoundaryShell,
    identity: &crate::tui::boundary_shell::confirmation::ConfirmationIdentity,
    directive_hash: &str,
) -> Result<crate::tui::boundary_shell::directive::PersistedDirective, String> {
    if let Some(refused) = refused_for_held_pack(&identity.pack) {
        return Err(format!("Directive confirmation refused: {refused}"));
    }
    boundary_shell
        .confirm_directive(directive_hash)
        .map(|confirmed| confirmed.directive().clone())
        .map_err(|error| format!("Directive confirmation refused: {error}"))
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
    crate::structured_budget::command::pin_profile(&updated)?;
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
    config.recovery_plan_auto_runs = identity.recovery_plan_auto_runs;
}

fn begin_gate_four_pack_change<'a>(
    boundary_shell: &'a mut crate::tui::boundary_shell::BoundaryShell,
    identity: &crate::tui::boundary_shell::confirmation::ConfirmationIdentity,
    selector: &str,
    pack_context: &pack_selection::ReplPackContext,
) -> anyhow::Result<(
    &'a crate::tui::boundary_shell::confirmation::ConfirmationIdentity,
    Option<std::path::PathBuf>,
)> {
    if selector.is_empty() {
        bail!("/pack requires <id@version>");
    }
    let (pack, directory) = pack_context
        .select_frozen(&identity.profile, &identity.intent, selector)
        .map_err(|error| anyhow::anyhow!("{}", pack_selection::refusal_reason(&error)))?;
    if let Some(refused) = refused_for_held_pack(&pack) {
        bail!("{refused}");
    }
    let changed =
        boundary_shell.begin_pack_change_with_locator(identity, pack, pack_context.locator())?;
    Ok((changed, Some(directory)))
}

/// The dispatch entry for tests and callers that read packs from the workspace
/// alone. The REPL itself dispatches through [`with_confirmed_pack_in`].
#[cfg(test)]
fn with_confirmed_pack<T>(
    config: &Config,
    identity: &crate::tui::boundary_shell::confirmation::ConfirmationIdentity,
    run: impl FnOnce() -> anyhow::Result<T>,
) -> anyhow::Result<T> {
    let context = pack_selection::ReplPackContext::from_config(config);
    with_confirmed_pack_in(&context, config, identity, run)
}

/// Install and record the pack a confirmed card froze. The directory and the
/// pin are read through the same roots the card was read from, so a dispatch
/// cannot fall back to a same-named pack in another root (Issue #1057).
fn with_confirmed_pack_in<T>(
    pack_context: &pack_selection::ReplPackContext,
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
    // Revalidate the frozen exact bytes through the same locator a card used.
    // A pack that changed, was removed, or now resolves from another supply
    // source is refused before any provider is called.
    pack_context.validate(&identity.profile, &identity.intent, &identity.pack)?;
    let directory = pack_context.directory(*source, id, version)?;
    let observed_hash = pack_context
        .observed_pin(&identity.pack)?
        .unwrap_or_default();
    if observed_hash != *hash {
        bail!(
            "confirmed pack changed before dispatch: expected `{hash}`, observed `{observed_hash}`"
        );
    }
    // A REPL started with `--pack` holds that pack for its whole life. Installing again
    // would take the held lock a second time on this thread (Issue #1036), and replacing
    // the held environment is not allowed: the same pack dispatches under it, and another
    // pack is refused before the dispatch.
    let _pack_environment = match crate::cli_pack::HeldPack::on_this_thread() {
        Some(held) if held.is(id, version, hash) => None,
        Some(held) => bail!("{}", held_pack_conflict(&held, id, version, Some(hash))),
        None => {
            let resolved = crate::cli_pack::ResolvedPack {
                id: id.clone(),
                version: version.clone(),
                hash: hash.clone(),
                source: match source {
                    crate::planner::pack::catalog::PackSource::Local => {
                        crate::cli_pack::SelectionSource::ExtensionRoot
                    }
                    crate::planner::pack::catalog::PackSource::Admitted
                    | crate::planner::pack::catalog::PackSource::Repository => {
                        crate::cli_pack::SelectionSource::Repository
                    }
                },
                directory,
                declared: None,
            };
            Some(crate::cli_pack::RuntimeEnvironmentGuard::install(Some(
                &resolved,
            ))?)
        }
    };
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

/// Why a pack selection is refused in a REPL that holds the `--pack` it was started with:
/// the held pack, the selected one, and what to do. `hash` is the selected exact-byte hash,
/// or `None` for a Gate 1 selector the admitted catalog does not list, which no restart
/// makes selectable. Restarting with the same selector in `--pack` would resolve the same
/// held bytes again, so only a restart without `--pack` is offered.
fn held_pack_conflict(
    held: &crate::cli_pack::HeldPack,
    id: &str,
    version: &str,
    hash: Option<&str>,
) -> String {
    let selector = format!("{id}@{version}");
    let Some(hash) = hash else {
        // Only Gate 1 names a pack without its hash: the admitted catalog has none for it.
        return format!(
            "this REPL was started with `--pack {}` ({}), which stays in place until the REPL exits, and `{selector}` is not an admitted pack, so Gate 1 cannot select it, in this REPL or after a restart.",
            held.selector(),
            held.hash,
        );
    };
    let selected = format!("`{selector}` ({hash})");
    let difference = if selector == held.selector() {
        "does not resolve to the same bytes"
    } else {
        "is a different pack"
    };
    format!(
        "this REPL was started with `--pack {}` ({}), which stays in place until the REPL exits, and {selected} {difference}. Restart the REPL without `--pack` to select {selected} here.",
        held.selector(),
        held.hash,
    )
}

/// The refusal of a selected pack that is not the pack this REPL holds, if it holds one.
fn refused_for_held_pack(
    pack: &crate::tui::boundary_shell::confirmation::PackSelection,
) -> Option<String> {
    let held = crate::cli_pack::HeldPack::on_this_thread()?;
    let crate::tui::boundary_shell::confirmation::PackSelection::Pinned {
        id, version, hash, ..
    } = pack
    else {
        return None;
    };
    (!held.is(id, version, hash)).then(|| held_pack_conflict(&held, id, version, Some(hash)))
}

/// The refusal of a Gate 1 `--pack` selector under a held pack, decided before the planner
/// is asked to route the request. The selector is resolved under the CLI's roots (extension
/// root first, then workspace), so a pack that `--pack` held from the extension root is
/// exactly the one the same selector resolves to here. Only the held hash, whatever route
/// is proposed, makes it acceptable. A malformed selector keeps the error a selection gives
/// it, and a selector the roots cannot read is refused with the loader's reason.
fn refused_gate_one_pack(
    pack_context: &pack_selection::ReplPackContext,
    selector: &str,
) -> Option<String> {
    let held = crate::cli_pack::HeldPack::on_this_thread()?;
    let Some((id, version)) = selector.trim().split_once('@') else {
        return Some(format!(
            "pack selector `{selector}` must pin id@MAJOR.MINOR.PATCH"
        ));
    };
    match pack_context.resolved_hash(selector) {
        Some(hash) if held.is(id, version, &hash) => None,
        Some(hash) => Some(held_pack_conflict(&held, id, version, Some(&hash))),
        None => Some(format!(
            "pack selector `{selector}` cannot be selected from the extension root or workspace: {}",
            pack_context.selector_refusal(selector)
        )),
    }
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
    pack_context: &pack_selection::ReplPackContext,
) -> anyhow::Result<String> {
    if terminal.full {
        return crate::tui::boundary_shell::presentation::render_gate_three_with_locator(
            identity,
            terminal,
            pack_context.locator(),
        );
    }
    // "Available" means a card could actually choose it under the REPL's roots,
    // not merely that the admitted catalog lists one (Issue #1057).
    let pack_change_available = pack_context
        .locator()
        .selectable(&identity.profile, &identity.intent)
        .iter()
        .any(|candidate| match &identity.pack {
            crate::tui::boundary_shell::confirmation::PackSelection::None => true,
            crate::tui::boundary_shell::confirmation::PackSelection::Pinned {
                id, version, ..
            } => candidate.id != *id || candidate.version != *version,
        });
    crate::tui::boundary_shell::presentation::render_gate_four_with_locator(
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
        pack_context.locator(),
    )
}

/// Render and record the Gate 3/Gate 4 terminal for a confirmed run, after its
/// stop event and sheet are recorded. A pack that changed or moved during the
/// run fails revalidation here; the terminal is still recorded and shown with
/// the masked reason, so the transcript is not lost and the caller keeps reading
/// input rather than ending the REPL (Issue #1057 r2).
fn present_and_record_terminal(
    config: &Config,
    identity: &crate::tui::boundary_shell::confirmation::ConfirmationIdentity,
    terminal: &crate::tui::boundary_shell::acceptance::TerminalPresentation,
    pack_context: &pack_selection::ReplPackContext,
    sheet_path: &std::path::Path,
) -> anyhow::Result<String> {
    let rendered = match render_terminal(identity, terminal, pack_context) {
        Ok(rendered) => rendered,
        Err(error) => render_terminal_after_pack_change(identity, terminal, &error),
    };
    let gate = if terminal.full { "Gate 3" } else { "Gate 4" };
    // The transcript is saved outside any later display boundary, so the reason
    // body and the sheet path's value are each masked with the run's registered
    // scope here. A `--state-dir` can contain a registered value, and the path
    // must not be persisted raw (Issue #1057 r3).
    //
    // Only the *value* is masked; the fixed label `Sheet path: ` is joined
    // afterwards, so registering that label cannot rewrite the record's
    // structure (Issue #1057 r4).
    let body = crate::sensitive_data::scrub_everything(&rendered);
    let masked_path = crate::sensitive_data::scrub_everything(&sheet_path.display().to_string());
    let body = format!("{body}\n\nSheet path: {masked_path}");
    crate::tui::boundary_shell::transcript::append(&config.state_dir, gate, &body)?;
    Ok(rendered)
}

/// The terminal transcript for a confirmed run whose pack no longer
/// revalidates after dispatch. The run's stop event and sheet are already
/// recorded and are left untouched; this records and shows the terminal with
/// the masked reason, so the Gate 3/Gate 4 transcript is not lost and the REPL
/// can keep reading input (Issue #1057 r2).
fn render_terminal_after_pack_change(
    identity: &crate::tui::boundary_shell::confirmation::ConfirmationIdentity,
    terminal: &crate::tui::boundary_shell::acceptance::TerminalPresentation,
    error: &anyhow::Error,
) -> String {
    let gate = if terminal.full {
        "Gate 3 — Acceptance"
    } else {
        "Gate 4 — Failure and next action"
    };
    // Mask with the run's registered scope before display, as a startup refusal
    // does; never quote the pack's bytes.
    let reason = crate::sensitive_data::scrub_everything(&format!("{error:#}"));
    let card_hash = identity
        .card_hash()
        .unwrap_or_else(|_| "<unavailable>".to_string());
    format!(
        "# {gate}\n\n\
         - 実行後の検証パック再確認に失敗しました: {reason}\n\
         - この実行の停止 event と acceptance sheet は記録済みです。\n\
         - Confirmation ID: {card_hash}\n\n{}",
        terminal.acceptance_sheet
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
    use crate::planner::pack::catalog::PackLocator;
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
            think: None,
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

    /// Issue #976 (3e): `/profile` under a wallet pins the profile in it once, the same one again
    /// goes on, and another is refused and leaves the configuration as it was.
    #[test]
    fn repl_profile_is_pinned_in_the_wallet_once() {
        use crate::structured_budget::entry::EntryRefusal;
        use crate::structured_budget::ledger::LedgerRefusal;
        let wallet = crate::structured_budget::test_support::TestWallet::open(|_| {});
        let mut config = session_config();
        config.field_sources.run_budget = Some(wallet.handle.clone());
        let words = |line: &str| crate::tui::slash::parse_words(line);
        let pinned = update_profile(&config, &words("/profile next.js")).unwrap();
        assert_eq!(
            wallet.handle.wallet().unwrap().profile.as_deref(),
            Some("nextjs")
        );
        assert!(update_profile(&pinned, &words("/profile nextjs")).is_ok());
        let error = update_profile(&pinned, &words("/profile python-cli")).unwrap_err();
        assert_eq!(
            error.downcast_ref::<EntryRefusal>(),
            Some(&EntryRefusal::Wallet(LedgerRefusal::ProfileChanged))
        );
        assert_eq!(
            wallet.handle.wallet().unwrap().profile.as_deref(),
            Some("nextjs")
        );
    }

    #[test]
    fn confirmed_gate_one_identity_preserves_recovery_limit_for_dispatch() {
        let root = tempfile::tempdir().unwrap();
        let mut shell = crate::tui::boundary_shell::BoundaryShell::new(
            root.path().join("confirmations"),
            Some(root.path().join("events.jsonl")),
        );
        let identity = shell
            .begin_gate_one_with_recovery(
                python_cli_proposal(),
                "Python CLI filter",
                root.path(),
                pins(),
                PackSelection::None,
                7,
            )
            .unwrap()
            .clone();
        let mut config = session_config();
        config.recovery_plan_auto_runs = 0;

        apply_confirmed_identity(&mut config, &identity);

        assert_eq!(config.recovery_plan_auto_runs, 7);
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

    /// The REPL pack context of a workspace-only REPL, as the tests that pin
    /// admitted packs read them.
    fn workspace_context(workspace: &std::path::Path) -> pack_selection::ReplPackContext {
        pack_selection::ReplPackContext::new(workspace.to_path_buf(), None)
    }

    #[test]
    fn gate_four_pack_change_creates_a_new_gate_one_boundary() {
        let root = tempfile::tempdir().unwrap();
        let (mut shell, identity) = failed_shell(root.path());
        let (changed, _directory) = begin_gate_four_pack_change(
            &mut shell,
            &identity,
            "cli-assist@1.1.0",
            &workspace_context(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))),
        )
        .unwrap();
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

    const PACK_ENV_KEYS: [&str; 4] = [
        "COMMANDAGENT_PACK_DIRECTORY",
        "COMMANDAGENT_PACK_ID",
        "COMMANDAGENT_PACK_VERSION",
        "COMMANDAGENT_PACK_HASH",
    ];

    fn pack_environment() -> [Option<std::ffi::OsString>; 4] {
        PACK_ENV_KEYS.map(std::env::var_os)
    }

    /// The python-cli × create confirmation of this repository's `cli-assist@1.1.0`.
    fn confirmed_python_cli_pack(
        events: &std::path::Path,
    ) -> (
        Config,
        crate::tui::boundary_shell::confirmation::ConfirmationIdentity,
    ) {
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
        config.eval_events_path = Some(events.to_path_buf());
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
        (config, identity)
    }

    /// What a Next.js × create run reads of the pack selection: its final acceptance
    /// checks and its phase material.
    fn nextjs_reader(root: &std::path::Path) -> anyhow::Result<Option<String>> {
        let checks = crate::planner::pack::runtime::run_final_acceptance_checks_from_environment(
            root, "nextjs", "create", None,
        )?;
        assert!(checks.is_none(), "a Next.js reader ran pack checks");
        let prompt = crate::planner::pack::runtime::append_phase_material_from_environment(
            "phase prompt".to_string(),
            root,
            "nextjs",
            "create",
            "phase-1",
        )?;
        Ok(Some(prompt))
    }

    /// The control for the shared-process hazard: in one process, a Next.js reader on
    /// another thread, run while the REPL's confirmed python-cli pack is installed, takes
    /// that selection and is refused. It changes the process environment, so it runs in a
    /// process of its own.
    #[test]
    fn a_confirmed_pack_held_in_one_process_is_selected_by_a_nextjs_reader_there() {
        crate::cli_pack::tests::own_process::run(
            "tui::repl::tests::a_confirmed_pack_held_in_one_process_is_selected_by_a_nextjs_reader_there",
            || {
                let root = tempfile::tempdir().unwrap();
                let reader_root = tempfile::tempdir().unwrap();
                let (config, identity) =
                    confirmed_python_cli_pack(&root.path().join("events.jsonl"));
                assert_eq!(pack_environment(), [None, None, None, None]);
                assert_eq!(
                    nextjs_reader(reader_root.path()).unwrap().as_deref(),
                    Some("phase prompt")
                );
                with_confirmed_pack(&config, &identity, || {
                    // The writer holds the selection until this reader has returned.
                    let read = std::thread::scope(|scope| {
                        scope.spawn(|| nextjs_reader(reader_root.path())).join()
                    })
                    .unwrap();
                    let error = format!("{:#}", read.unwrap_err());
                    assert!(
                        error.contains(
                            "selected pack is for python-cli × create, not nextjs × create"
                        ),
                        "{error}"
                    );
                    Ok(())
                })
                .unwrap();
                assert_eq!(pack_environment(), [None, None, None, None]);
                assert_eq!(
                    nextjs_reader(reader_root.path()).unwrap().as_deref(),
                    Some("phase prompt")
                );
            },
        );
    }

    /// The REPL installs its confirmed pack in the process environment, so its body runs in
    /// a process of its own. While that process holds the pack, this process keeps its own
    /// four keys, and its Next.js reader does not take the selection.
    #[test]
    fn confirmed_repl_pack_is_installed_for_dispatch_and_recorded() {
        let before = pack_environment();
        let reader_root = tempfile::tempdir().unwrap();
        crate::cli_pack::tests::own_process::run_holding(
            "tui::repl::tests::confirmed_repl_pack_is_installed_for_dispatch_and_recorded",
            confirmed_repl_pack_is_installed_in_its_own_process,
            || {
                assert_eq!(pack_environment(), before);
                assert_eq!(before, [None, None, None, None]);
                assert_eq!(
                    nextjs_reader(reader_root.path())
                        .map_err(|error| format!("{error:#}"))
                        .unwrap()
                        .as_deref(),
                    Some("phase prompt")
                );
            },
        );
        assert_eq!(pack_environment(), before);
    }

    fn confirmed_repl_pack_is_installed_in_its_own_process() {
        let root = tempfile::tempdir().unwrap();
        let (config, identity) = confirmed_python_cli_pack(&root.path().join("events.jsonl"));
        let previous_environment = pack_environment();
        let previous = std::env::var_os("COMMANDAGENT_PACK_ID");
        with_confirmed_pack(&config, &identity, || {
            assert_eq!(std::env::var("COMMANDAGENT_PACK_ID").unwrap(), "cli-assist");
            assert_eq!(std::env::var("COMMANDAGENT_PACK_VERSION").unwrap(), "1.1.0");
            crate::cli_pack::tests::own_process::hold();
            Ok(())
        })
        .unwrap();
        assert_eq!(std::env::var_os("COMMANDAGENT_PACK_ID"), previous);
        assert_eq!(pack_environment(), previous_environment);
        let events = std::fs::read_to_string(config.eval_events_path.unwrap()).unwrap();
        assert!(events.contains("\"event\":\"pack_injected\""), "{events}");
        assert!(events.contains("\"pack_id\":\"cli-assist\""), "{events}");
        assert!(events.contains("\"pack_version\":\"1.1.0\""), "{events}");
    }

    /// Longest a pack selection made in a REPL that already holds a pack may take to
    /// dispatch or be refused. Taking the held, non-reentrant pack lock again on the same
    /// thread never returns (Issue #1036), so the deadline turns that hang into a failure.
    const HELD_PACK_DEADLINE: std::time::Duration = std::time::Duration::from_secs(60);

    /// Runs `body` on a thread of its own and returns what it returned, or fails when it
    /// has not returned within `HELD_PACK_DEADLINE`.
    fn within_held_pack_deadline<T: Send + 'static>(
        body: impl FnOnce() -> T + Send + 'static,
    ) -> T {
        let (sender, receiver) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            let _ = sender.send(body());
        });
        match receiver.recv_timeout(HELD_PACK_DEADLINE) {
            Ok(value) => {
                worker.join().unwrap();
                value
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => panic!(
                "a pack selection under a held pack neither dispatched nor was refused within {HELD_PACK_DEADLINE:?}: the pack lock was taken twice on one thread"
            ),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                std::panic::resume_unwind(worker.join().unwrap_err())
            }
        }
    }

    /// The confirmation of `cli-assist@1.0.0`, which is not the pack the outer scope holds.
    fn other_python_cli_pack(
        identity: &crate::tui::boundary_shell::confirmation::ConfirmationIdentity,
    ) -> crate::tui::boundary_shell::confirmation::ConfirmationIdentity {
        let mut other = identity.clone();
        other.pack = crate::tui::boundary_shell::pack_catalog::select(
            "python-cli",
            "create",
            "cli-assist@1.0.0",
        )
        .unwrap();
        other
    }

    /// Issue #1036: a REPL started with `--pack` holds that pack for its whole life. A
    /// confirmed selection of the same pack dispatches, within the deadline, under the held
    /// environment instead of installing it again. The outer pack is held by the REPL's own
    /// production install; the body writes the process environment, so it runs alone.
    #[test]
    fn a_held_pack_confirmed_again_dispatches_without_a_second_install() {
        crate::cli_pack::tests::own_process::run(
            "tui::repl::tests::a_held_pack_confirmed_again_dispatches_without_a_second_install",
            || {
                within_held_pack_deadline(|| {
                    let root = tempfile::tempdir().unwrap();
                    let (config, identity) =
                        confirmed_python_cli_pack(&root.path().join("events.jsonl"));
                    assert_eq!(pack_environment(), [None, None, None, None]);
                    with_confirmed_pack(&config, &identity, || {
                        let held = pack_environment();
                        let dispatched = std::cell::Cell::new(false);
                        with_confirmed_pack(&config, &identity, || {
                            dispatched.set(true);
                            assert_eq!(pack_environment(), held);
                            Ok(())
                        })
                        .unwrap();
                        assert!(dispatched.get(), "the same pack was not dispatched");
                        assert_eq!(pack_environment(), held, "the held pack was not kept");
                        assert_eq!(std::env::var("COMMANDAGENT_PACK_ID").unwrap(), "cli-assist");
                        assert_eq!(std::env::var("COMMANDAGENT_PACK_VERSION").unwrap(), "1.1.0");
                        Ok(())
                    })
                    .unwrap();
                    assert_eq!(pack_environment(), [None, None, None, None]);
                    let events = std::fs::read_to_string(config.eval_events_path.unwrap()).unwrap();
                    assert_eq!(
                        events.matches("\"event\":\"pack_injected\"").count(),
                        2,
                        "{events}"
                    );
                });
            },
        );
    }

    /// Issue #1036: under a held pack, a confirmed selection of another pack is refused,
    /// with its reason, before the dispatch (the provider) runs, and the held environment
    /// is not overwritten.
    #[test]
    fn a_different_pack_under_a_held_pack_is_refused_before_dispatch() {
        crate::cli_pack::tests::own_process::run(
            "tui::repl::tests::a_different_pack_under_a_held_pack_is_refused_before_dispatch",
            || {
                within_held_pack_deadline(|| {
                    let root = tempfile::tempdir().unwrap();
                    let (config, identity) =
                        confirmed_python_cli_pack(&root.path().join("events.jsonl"));
                    let other = other_python_cli_pack(&identity);
                    with_confirmed_pack(&config, &identity, || {
                        let held = pack_environment();
                        let dispatched = std::cell::Cell::new(false);
                        let error = with_confirmed_pack(&config, &other, || {
                            dispatched.set(true);
                            Ok(())
                        })
                        .unwrap_err();
                        let error = format!("{error:#}");
                        assert!(!dispatched.get(), "a different pack reached the dispatch");
                        assert!(error.contains("--pack cli-assist@1.1.0"), "{error}");
                        assert!(error.contains("cli-assist@1.0.0"), "{error}");
                        assert!(error.contains("Restart the REPL"), "{error}");
                        assert_eq!(pack_environment(), held, "the held pack was overwritten");
                        Ok(())
                    })
                    .unwrap();
                    let events = std::fs::read_to_string(config.eval_events_path.unwrap()).unwrap();
                    assert_eq!(
                        events.matches("\"event\":\"pack_injected\"").count(),
                        1,
                        "{events}"
                    );
                    assert!(!events.contains("\"pack_version\":\"1.0.0\""), "{events}");
                });
            },
        );
    }

    /// Issue #1036: the Gate 1 request and the Gate 4 `/pack` change refuse another pack
    /// before anything is asked of a provider, and accept the held one; without a held
    /// pack, neither refuses (the REPL without `--pack` is unchanged).
    #[test]
    fn gate_one_and_gate_four_refuse_another_pack_under_a_held_pack() {
        let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let unheld = workspace_context(repository);
        for selector in ["cli-assist@1.0.0", "cli-assist@1.1.0", "nextjs-acme@1.0.0"] {
            assert_eq!(refused_gate_one_pack(&unheld, selector), None, "{selector}");
        }
        crate::cli_pack::tests::own_process::run(
            "tui::repl::tests::gate_one_and_gate_four_refuse_another_pack_under_a_held_pack",
            || {
                within_held_pack_deadline(|| {
                    let root = tempfile::tempdir().unwrap();
                    let (config, identity) =
                        confirmed_python_cli_pack(&root.path().join("events.jsonl"));
                    let shell_root = tempfile::tempdir().unwrap();
                    let context = workspace_context(&config.workspace_root);
                    with_confirmed_pack(&config, &identity, || {
                        assert_eq!(refused_gate_one_pack(&context, "cli-assist@1.1.0"), None);
                        assert_eq!(refused_gate_one_pack(&context, " cli-assist@1.1.0 "), None);
                        let refused = refused_gate_one_pack(&context, "cli-assist@1.0.0").unwrap();
                        assert!(refused.contains("--pack cli-assist@1.1.0"), "{refused}");
                        assert!(refused.contains("`cli-assist@1.0.0`"), "{refused}");
                        assert!(refused.contains("Restart the REPL"), "{refused}");

                        let (mut shell, failed) = failed_shell(shell_root.path());
                        let error = begin_gate_four_pack_change(
                            &mut shell,
                            &failed,
                            "cli-assist@1.0.0",
                            &context,
                        )
                        .unwrap_err()
                        .to_string();
                        assert_eq!(error, refused);
                        let events =
                            std::fs::read_to_string(shell_root.path().join("events.jsonl"))
                                .unwrap_or_default();
                        assert!(!events.contains("\"action\":\"pack_change\""), "{events}");
                        let (changed, _directory) = begin_gate_four_pack_change(
                            &mut shell,
                            &failed,
                            "cli-assist@1.1.0",
                            &context,
                        )
                        .unwrap();
                        assert!(matches!(
                            &changed.pack,
                            PackSelection::Pinned { id, version, .. }
                                if id == "cli-assist" && version == "1.1.0"
                        ));
                        Ok(())
                    })
                    .unwrap();
                    assert_eq!(refused_gate_one_pack(&context, "cli-assist@1.0.0"), None);
                });
            },
        );
    }

    /// A planner that counts its calls; any call is a provider request Gate 1 must not make
    /// for a pack the REPL cannot dispatch.
    #[derive(Clone)]
    struct CountingPlanner {
        calls: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    }

    impl crate::providers::ChatClient for CountingPlanner {
        fn label(&self) -> &str {
            "counting-planner"
        }

        fn boxed_clone(&self) -> Box<dyn crate::providers::ChatClient> {
            Box::new(self.clone())
        }

        fn chat(
            &mut self,
            _model: &str,
            _messages: &[crate::state::ConversationMessage],
            _tools: &[crate::tools::registry::ToolSpec],
            _native_tools_enabled: bool,
        ) -> anyhow::Result<crate::providers::AssistantReply> {
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(crate::providers::AssistantReply::text("unexpected"))
        }
    }

    /// `cli-assist@1.1.0` with bytes other than the admitted ones, as a repository pack of
    /// `root`, and the confirmation that holds it: the same selector, another exact-byte hash.
    fn reshaped_python_cli_pack(
        root: &std::path::Path,
    ) -> (
        Config,
        crate::tui::boundary_shell::confirmation::ConfirmationIdentity,
    ) {
        let (mut config, identity) = confirmed_python_cli_pack(&root.join("events.jsonl"));
        let source =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("packs/cli-assist/1.1.0");
        let directory = root.join("workspace/packs/cli-assist/1.1.0");
        std::fs::create_dir_all(&directory).unwrap();
        let mut assist = std::fs::read(source.join("assist.yaml")).unwrap();
        assist.extend_from_slice(b"\n# the same pack, reshaped\n");
        std::fs::write(directory.join("assist.yaml"), &assist).unwrap();
        // A complete pack carries its own pin; without one the loader refuses
        // it, and a dispatch revalidation would be reading a broken pack.
        let hash = crate::planner::pack::exact_byte_hash(Some(&assist), None);
        std::fs::write(directory.join("pack.sha256"), format!("{hash}\n")).unwrap();
        config.workspace_root = root.join("workspace");
        let hash = PackLocator::new(&config.workspace_root)
            .observed_hash(
                crate::planner::pack::catalog::PackSource::Repository,
                "cli-assist",
                "1.1.0",
            )
            .unwrap();
        let mut reshaped = identity.clone();
        let PackSelection::Pinned {
            hash: admitted,
            point,
            ..
        } = &identity.pack
        else {
            panic!("the confirmation pins no pack");
        };
        assert_ne!(&hash, admitted, "the reshaped bytes kept the admitted hash");
        reshaped.pack = PackSelection::Pinned {
            id: "cli-assist".to_string(),
            version: "1.1.0".to_string(),
            hash,
            point: point.clone(),
            source: crate::planner::pack::catalog::PackSource::Repository,
        };
        (config, reshaped)
    }

    /// Issue #1036 B1: under a held `cli-assist@1.1.0` whose bytes are not the admitted
    /// ones, a Gate 1 request naming that same selector resolves to the admitted bytes, so
    /// it is refused, with its reason, before the planner is asked anything. The request is
    /// one the planner would be asked to route.
    #[test]
    fn gate_one_refuses_the_held_selector_with_other_bytes_before_the_planner() {
        crate::cli_pack::tests::own_process::run(
            "tui::repl::tests::gate_one_refuses_the_held_selector_with_other_bytes_before_the_planner",
            || {
                within_held_pack_deadline(|| {
                    let root = tempfile::tempdir().unwrap();
                    let (routing, _) = confirmed_python_cli_pack(&root.path().join("events.jsonl"));
                    let (held_config, held) = reshaped_python_cli_pack(root.path());
                    let request = crate::tui::slash::parse_inline_request(
                        "Create a Python CLI --pack cli-assist@1.1.0",
                    )
                    .unwrap();
                    // The selector resolves under the repository root, whose
                    // `cli-assist@1.1.0` is the admitted one; the held pack's
                    // bytes are the reshaped ones, so they differ.
                    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
                    let routing_context = workspace_context(repository);
                    let held_context = workspace_context(repository);
                    // Without a held pack this request goes to the planner.
                    let planner = CountingPlanner {
                        calls: Default::default(),
                    };
                    let unheld = route_gate_one(
                        &routing,
                        &routing_context,
                        &request,
                        &mut planner.clone(),
                        &|| false,
                    );
                    assert_eq!(
                        planner.calls.load(std::sync::atomic::Ordering::SeqCst),
                        1,
                        "the request is not one the planner routes: {unheld:?}"
                    );
                    let planner = CountingPlanner {
                        calls: Default::default(),
                    };
                    with_confirmed_pack(&held_config, &held, || {
                        let refused = route_gate_one(
                            &routing,
                            &held_context,
                            &request,
                            &mut planner.clone(),
                            &|| false,
                        )
                        .unwrap_err();
                        assert_eq!(
                            planner.calls.load(std::sync::atomic::Ordering::SeqCst),
                            0,
                            "the planner was asked before the refusal: {refused}"
                        );
                        assert!(refused.starts_with("Gate 1 request refused: "), "{refused}");
                        assert!(refused.contains("--pack cli-assist@1.1.0"), "{refused}");
                        assert!(
                            refused.contains("does not resolve to the same bytes"),
                            "{refused}"
                        );
                        assert!(refused.contains("without `--pack`"), "{refused}");
                        assert!(!refused.contains("Restart the REPL with `"), "{refused}");
                        // N3: a malformed selector keeps its own error.
                        let malformed = crate::tui::slash::parse_inline_request(
                            "Create a Python CLI --pack cli-assist",
                        )
                        .unwrap();
                        let refused = route_gate_one(
                            &routing,
                            &held_context,
                            &malformed,
                            &mut planner.clone(),
                            &|| false,
                        )
                        .unwrap_err();
                        assert!(
                            refused.contains("must pin id@MAJOR.MINOR.PATCH"),
                            "{refused}"
                        );
                        assert!(!refused.contains("different pack"), "{refused}");
                        assert_eq!(planner.calls.load(std::sync::atomic::Ordering::SeqCst), 0);
                        Ok(())
                    })
                    .unwrap();
                });
            },
        );
    }

    /// Issue #1036 B1: a Gate 4 `/pack` naming the held selector with other bytes makes no
    /// pack-change card.
    #[test]
    fn gate_four_refuses_the_held_selector_with_other_bytes() {
        crate::cli_pack::tests::own_process::run(
            "tui::repl::tests::gate_four_refuses_the_held_selector_with_other_bytes",
            || {
                within_held_pack_deadline(|| {
                    let root = tempfile::tempdir().unwrap();
                    let shell_root = tempfile::tempdir().unwrap();
                    let (held_config, held) = reshaped_python_cli_pack(root.path());
                    // The selector resolves under the repository root, whose
                    // `cli-assist@1.1.0` is the admitted one, not the reshaped
                    // bytes the REPL holds.
                    let context =
                        workspace_context(std::path::Path::new(env!("CARGO_MANIFEST_DIR")));
                    with_confirmed_pack(&held_config, &held, || {
                        let (mut shell, failed) = failed_shell(shell_root.path());
                        let refused = begin_gate_four_pack_change(
                            &mut shell,
                            &failed,
                            "cli-assist@1.1.0",
                            &context,
                        )
                        .unwrap_err()
                        .to_string();
                        assert!(
                            refused.contains("does not resolve to the same bytes"),
                            "{refused}"
                        );
                        assert!(refused.contains("without `--pack`"), "{refused}");
                        let events =
                            std::fs::read_to_string(shell_root.path().join("events.jsonl"))
                                .unwrap_or_default();
                        assert!(!events.contains("\"action\":\"pack_change\""), "{events}");
                        // N3: a malformed selector keeps its own error.
                        let refused = begin_gate_four_pack_change(
                            &mut shell,
                            &failed,
                            "cli-assist",
                            &context,
                        )
                        .unwrap_err()
                        .to_string();
                        assert!(
                            refused.contains("must pin id@MAJOR.MINOR.PATCH"),
                            "{refused}"
                        );
                        Ok(())
                    })
                    .unwrap();
                });
            },
        );
    }

    /// Issue #1036 B2: a confirmed card whose pack a REPL holding another `--pack` cannot
    /// dispatch is refused at `/confirm` and `/confirm-directive`, and what the REPL shows
    /// names the held pack, the card's pack, and what to do. Nothing is confirmed.
    #[test]
    fn confirmation_shows_why_a_card_for_another_pack_is_not_dispatched() {
        crate::cli_pack::tests::own_process::run(
            "tui::repl::tests::confirmation_shows_why_a_card_for_another_pack_is_not_dispatched",
            || {
                within_held_pack_deadline(|| {
                    let root = tempfile::tempdir().unwrap();
                    let shell_root = tempfile::tempdir().unwrap();
                    let (config, identity) =
                        confirmed_python_cli_pack(&root.path().join("events.jsonl"));
                    let other = other_python_cli_pack(&identity);
                    with_confirmed_pack(&config, &identity, || {
                        let mut shell = crate::tui::boundary_shell::BoundaryShell::new(
                            shell_root.path().join("confirmations"),
                            Some(shell_root.path().join("events.jsonl")),
                        );
                        let card = shell
                            .begin_gate_one(
                                python_cli_proposal(),
                                "Python CLI filter",
                                shell_root.path(),
                                pins(),
                                other.pack.clone(),
                            )
                            .unwrap()
                            .card_hash()
                            .unwrap();
                        let context_for_confirm =
                            workspace_context(&config.workspace_root);
                        let shown = confirm_gate_one(
                            &mut shell,
                            &config,
                            &context_for_confirm,
                            None,
                            &card,
                        )
                        .unwrap_err();
                        assert!(shown.starts_with("Confirmation refused: "), "{shown}");
                        assert!(shown.contains("--pack cli-assist@1.1.0"), "{shown}");
                        assert!(shown.contains("`cli-assist@1.0.0`"), "{shown}");
                        assert!(shown.contains("without `--pack`"), "{shown}");
                        assert!(
                            matches!(
                                shell.state(),
                                crate::tui::boundary_shell::BoundaryState::AwaitingConfirmation { .. }
                            ),
                            "the card was confirmed"
                        );
                        let events = std::fs::read_to_string(shell_root.path().join("events.jsonl"))
                            .unwrap_or_default();
                        assert!(!events.contains("\"event\":\"route_confirmed\""), "{events}");

                        let shown =
                            confirm_directive_for(&mut shell, &other, &"0".repeat(64)).unwrap_err();
                        assert!(shown.starts_with("Directive confirmation refused: "), "{shown}");
                        assert!(shown.contains("--pack cli-assist@1.1.0"), "{shown}");
                        assert!(shown.contains("`cli-assist@1.0.0`"), "{shown}");
                        assert!(shown.contains("without `--pack`"), "{shown}");
                        Ok(())
                    })
                    .unwrap();
                });
            },
        );
    }

    /// The failed run of a confirmed card for `pack`, taken to its Gate 4 in `shell`, as a
    /// restored or finished run is: the identity it ran.
    fn fail_at_gate_four(
        shell: &mut crate::tui::boundary_shell::BoundaryShell,
        config: &Config,
        pack: PackSelection,
    ) -> crate::tui::boundary_shell::confirmation::ConfirmationIdentity {
        let card = shell
            .begin_gate_one(
                python_cli_proposal(),
                "Python CLI filter",
                &config.workspace_root,
                pins(),
                pack,
            )
            .unwrap()
            .card_hash()
            .unwrap();
        let ran = shell.confirm(&card).unwrap().identity().clone();
        finish_failed(shell, config);
        ran
    }

    /// Ends the dispatched run in `shell` as failed, at its Gate 4.
    fn finish_failed(shell: &mut crate::tui::boundary_shell::BoundaryShell, config: &Config) {
        shell.dispatch(|_| Ok("failed".to_string())).unwrap();
        crate::eval_events::emit(
            config.eval_events_path.as_deref(),
            serde_json::json!({
                "event": "tui_command_stop",
                "profile": "python-cli",
                "status": "failed",
                "ok": false,
                "stop_reason": "failed",
            }),
        );
        shell
            .present_terminal(
                "# sheet\n\n## 5. Stop reason\nfailed".to_string(),
                false,
                Some("failed".to_string()),
            )
            .unwrap();
    }

    /// A REPL configuration whose state directory and event stream are its own.
    fn held_pack_session(
        root: &std::path::Path,
    ) -> (
        Config,
        crate::tui::boundary_shell::confirmation::ConfirmationIdentity,
    ) {
        let events = root.join("run-1036").join("events.jsonl");
        std::fs::create_dir_all(events.parent().unwrap()).unwrap();
        let (mut config, identity) = confirmed_python_cli_pack(&events);
        config.state_dir = root.join("state");
        (config, identity)
    }

    fn count_entries(directory: &std::path::Path) -> usize {
        std::fs::read_dir(directory).map_or(0, |entries| entries.count())
    }

    /// Issue #1036 D2: a REPL holding `cli-assist@1.1.0` restores a failed run of
    /// `cli-assist@1.0.0`. `/directive` for that run is refused, with its reason, before a
    /// proposal, its artifact, or its session round exists, and the shell stays at Gate 4,
    /// where `/pack cli-assist@1.1.0` makes a pack-change card.
    #[test]
    fn a_directive_for_another_pack_run_is_refused_before_it_is_proposed() {
        crate::cli_pack::tests::own_process::run(
            "tui::repl::tests::a_directive_for_another_pack_run_is_refused_before_it_is_proposed",
            || {
                within_held_pack_deadline(|| {
                    let root = tempfile::tempdir().unwrap();
                    let (config, identity) = held_pack_session(root.path());
                    let other = other_python_cli_pack(&identity);
                    with_confirmed_pack(&config, &identity, || {
                        let mut shell = boundary_shell_for(&config);
                        let ran = fail_at_gate_four(&mut shell, &config, other.pack.clone());
                        let target = boundary_run_id(config.eval_events_path.as_deref()).unwrap();
                        let round = shell.next_directive_round(&target).unwrap();
                        let artifacts = config.state_dir.join("boundary-directives");
                        let sessions = config.state_dir.join("boundary-sessions");
                        let before = (count_entries(&artifacts), count_entries(&sessions));

                        let refused =
                            begin_directive_for(&mut shell, Some(&ran), "retry", &target, round)
                                .map(|directive| directive.hash().to_string())
                                .unwrap_err();
                        assert!(refused.starts_with("Directive refused: "), "{refused}");
                        assert!(refused.contains("--pack cli-assist@1.1.0"), "{refused}");
                        assert!(refused.contains("`cli-assist@1.0.0`"), "{refused}");
                        assert!(refused.contains("without `--pack`"), "{refused}");
                        assert_eq!(
                            (count_entries(&artifacts), count_entries(&sessions)),
                            before,
                            "the refused directive left an artifact or a session round"
                        );
                        assert_eq!(shell.next_directive_round(&target).unwrap(), round);
                        assert!(
                            matches!(
                                shell.state(),
                                crate::tui::boundary_shell::BoundaryState::FailureReady(_)
                            ),
                            "{:?}",
                            shell.state()
                        );
                        let (changed, _directory) = begin_gate_four_pack_change(
                            &mut shell,
                            &ran,
                            "cli-assist@1.1.0",
                            &workspace_context(&config.workspace_root),
                        )
                        .unwrap();
                        assert!(matches!(
                            &changed.pack,
                            PackSelection::Pinned { id, version, .. }
                                if id == "cli-assist" && version == "1.1.0"
                        ));
                        Ok(())
                    })
                    .unwrap();
                });
            },
        );
    }

    /// Issue #1036 D2-1: while the `/pack cli-assist@1.1.0` card awaits confirmation, a
    /// `/confirm-directive` with a stale hash for the restored `cli-assist@1.0.0` run is
    /// refused and changes nothing, and `/confirm` of the card still goes through.
    #[test]
    fn an_unrelated_directive_confirmation_keeps_the_pack_change_card() {
        crate::cli_pack::tests::own_process::run(
            "tui::repl::tests::an_unrelated_directive_confirmation_keeps_the_pack_change_card",
            || {
                within_held_pack_deadline(|| {
                    let root = tempfile::tempdir().unwrap();
                    let (config, identity) = held_pack_session(root.path());
                    let other = other_python_cli_pack(&identity);
                    with_confirmed_pack(&config, &identity, || {
                        let mut shell = boundary_shell_for(&config);
                        let ran = fail_at_gate_four(&mut shell, &config, other.pack.clone());
                        let card = begin_gate_four_pack_change(
                            &mut shell,
                            &ran,
                            "cli-assist@1.1.0",
                            &workspace_context(&config.workspace_root),
                        )
                        .unwrap()
                        .0
                        .card_hash()
                        .unwrap();
                        let awaiting = format!("{:?}", shell.state());

                        let refused = confirm_directive_for(
                            &mut shell,
                            &ran,
                            &format!("sha256:{}", "0".repeat(64)),
                        )
                        .unwrap_err();
                        assert!(
                            refused.starts_with("Directive confirmation refused: "),
                            "{refused}"
                        );
                        assert!(refused.contains("`cli-assist@1.0.0`"), "{refused}");
                        assert_eq!(format!("{:?}", shell.state()), awaiting);
                        let confirmed = confirm_gate_one(
                            &mut shell,
                            &config,
                            &workspace_context(&config.workspace_root),
                            None,
                            &card,
                        )
                        .unwrap();
                        assert!(matches!(
                            &confirmed.pack,
                            PackSelection::Pinned { id, version, .. }
                                if id == "cli-assist" && version == "1.1.0"
                        ));
                        Ok(())
                    })
                    .unwrap();
                });
            },
        );
    }

    /// Issue #1036 D2-3: a directive refused for the held pack is not history. After the
    /// REPL goes on to a run of the held pack and proposes a directive for it, the history a
    /// later round of that lineage is given holds that directive and not the refused one.
    #[test]
    fn a_refused_directive_never_reaches_a_later_round_history() {
        crate::cli_pack::tests::own_process::run(
            "tui::repl::tests::a_refused_directive_never_reaches_a_later_round_history",
            || {
                within_held_pack_deadline(|| {
                    let root = tempfile::tempdir().unwrap();
                    let (config, identity) = held_pack_session(root.path());
                    let other = other_python_cli_pack(&identity);
                    with_confirmed_pack(&config, &identity, || {
                        let mut shell = boundary_shell_for(&config);
                        let ran = fail_at_gate_four(&mut shell, &config, other.pack.clone());
                        let target = boundary_run_id(config.eval_events_path.as_deref()).unwrap();
                        let round = shell.next_directive_round(&target).unwrap();
                        let _ = begin_directive_for(
                            &mut shell,
                            Some(&ran),
                            "REFUSED use the 1.0.0 rules",
                            &target,
                            round,
                        )
                        .map(|directive| directive.hash().to_string());

                        // The REPL goes on: a run of the held pack, failed at its Gate 4.
                        let mut shell = boundary_shell_for(&config);
                        let ran = fail_at_gate_four(&mut shell, &config, other.pack.clone());
                        let card = begin_gate_four_pack_change(
                            &mut shell,
                            &ran,
                            "cli-assist@1.1.0",
                            &workspace_context(&config.workspace_root),
                        )
                        .unwrap()
                        .0
                        .card_hash()
                        .unwrap();
                        let held_run = confirm_gate_one(
                            &mut shell,
                            &config,
                            &workspace_context(&config.workspace_root),
                            None,
                            &card,
                        )
                        .unwrap();
                        finish_failed(&mut shell, &config);
                        let round = shell.next_directive_round(&target).unwrap();
                        begin_directive_for(
                            &mut shell,
                            Some(&held_run),
                            "LEGIT keep the held rules",
                            &target,
                            round,
                        )
                        .unwrap();

                        // What the next round of the lineage is given.
                        let sessions = config.state_dir.join("boundary-sessions");
                        let events = config.eval_events_path.clone().unwrap();
                        let recorded =
                            crate::tui::boundary_shell::directive_session::load_for_target(
                                &sessions, &target,
                            )
                            .unwrap();
                        for prior in &recorded.session().rounds {
                            crate::tui::boundary_shell::directive_session::record_latest_result(
                                &sessions,
                                &target,
                                prior.round,
                                &events,
                            )
                            .unwrap();
                        }
                        let session =
                            crate::tui::boundary_shell::directive_session::load_for_target(
                                &sessions, &target,
                            )
                            .unwrap();
                        let next = session.session().rounds.len() as u32 + 1;
                        let history = crate::tui::boundary_shell::directive_session::render_history(
                            session.session(),
                            next,
                            crate::tui::boundary_shell::directive_session::MAX_HISTORY_RENDERED_BYTES,
                        )
                        .unwrap();
                        assert!(history.contains("LEGIT keep the held rules"), "{history}");
                        assert!(!history.contains("REFUSED"), "{history}");
                        Ok(())
                    })
                    .unwrap();
                });
            },
        );
    }

    /// Issue #1036 N1: Gate 1 resolves a `--pack` selector against the admitted catalog
    /// before routing (`refused_gate_one_pack`), and that gives what `pack_catalog::select`
    /// gives only while one id@version has one hash in the catalog. A row that breaks this
    /// fails here: review that lookup before adding it.
    #[test]
    fn gate_one_pack_lookup_matches_select_only_while_each_id_at_version_has_one_hash() {
        let mut hashes =
            std::collections::BTreeMap::<String, std::collections::BTreeSet<&str>>::new();
        for pack in crate::planner::pack::catalog::admitted_packs() {
            hashes
                .entry(format!("{}@{}", pack.id, pack.version))
                .or_default()
                .insert(pack.hash);
        }
        for (selector, found) in &hashes {
            assert_eq!(
                found.len(),
                1,
                "the admitted catalog gives `{selector}` {} hashes; Gate 1 resolves a held-pack selector before routing and no longer matches pack_catalog::select",
                found.len()
            );
        }
        for pack in crate::planner::pack::catalog::admitted_packs() {
            let selector = format!("{}@{}", pack.id, pack.version);
            let selected = crate::tui::boundary_shell::pack_catalog::select(
                pack.profile,
                pack.intent,
                &selector,
            )
            .unwrap();
            assert!(
                matches!(&selected, PackSelection::Pinned { hash, .. } if hash == pack.hash),
                "{selector}: {selected:?}"
            );
        }
    }

    /// Issue #1036 N1: a held pack the admitted catalog does not list cannot be selected at
    /// Gate 1 in this REPL or after a restart, so the refusal does not offer a restart.
    #[test]
    fn a_held_selector_without_an_admitted_pack_is_not_offered_a_restart() {
        let held = crate::cli_pack::HeldPack {
            id: "local-assist".to_string(),
            version: "1.0.0".to_string(),
            hash: "sha256:local".to_string(),
        };
        let refused = held_pack_conflict(&held, "local-assist", "1.0.0", None);
        assert!(refused.contains("--pack local-assist@1.0.0"), "{refused}");
        assert!(refused.contains("is not an admitted pack"), "{refused}");
        assert!(!refused.contains("Restart the REPL"), "{refused}");
    }

    fn python_cli_route() -> crate::tui::boundary_shell::route::RouteCandidate {
        python_cli_proposal().selected.unwrap()
    }

    /// A schema-valid `cli-assist`-style pack under `root/<id>/<version>`, with
    /// `marker` in its bytes, pinned. Returns its exact-byte hash.
    fn pinned_pack(root: &std::path::Path, id: &str, marker: &str) -> String {
        pack_selection::tests::write_pack(
            root,
            id,
            "1.0.0",
            &pack_selection::tests::assist(id, "1.0.0", marker),
        )
    }

    /// Issue #1057: a pack that exists only under `--extension-root` is held by
    /// `--pack`; the Gate 1 card shows it with its source and hash, and the
    /// dispatch injects that same source and hash, not admitted bytes.
    #[test]
    fn a_held_extension_pack_shows_and_dispatches_from_the_extension_root() {
        crate::cli_pack::tests::own_process::run(
            "tui::repl::tests::a_held_extension_pack_shows_and_dispatches_from_the_extension_root",
            || {
                within_held_pack_deadline(|| {
                    let temp = tempfile::tempdir().unwrap();
                    let workspace = temp.path().join("workspace");
                    let extension = temp.path().join("extension");
                    std::fs::create_dir_all(&workspace).unwrap();
                    let hash = pinned_pack(&extension, "canary-pack", "held-extension");

                    let mut config = Config::from_cli(crate::cli::Cli::parse_from([
                        "commandagent",
                        "--cwd",
                        workspace.to_str().unwrap(),
                        "--profile",
                        "python-cli",
                        "--intent",
                        "create",
                    ]))
                    .unwrap();
                    config.eval_events_path = Some(temp.path().join("events.jsonl"));
                    config.state_dir = temp.path().join("state");

                    let startup = crate::cli_pack::ResolvedPack {
                        id: "canary-pack".to_string(),
                        version: "1.0.0".to_string(),
                        hash: hash.clone(),
                        source: crate::cli_pack::SelectionSource::ExtensionRoot,
                        directory: extension.join("canary-pack/1.0.0"),
                        declared: None,
                    };
                    let context = pack_selection::ReplPackContext::with_startup(
                        workspace.clone(),
                        Some(extension.clone()),
                        Some(startup.clone()),
                    );

                    let card = gate_one_pack(&context, Some(&python_cli_route()), None).unwrap();
                    let rendered = crate::tui::boundary_shell::presentation::render_gate_one(
                        &boundary_identity(&config, card.selection.clone(), context.locator()),
                        context.locator(),
                    )
                    .unwrap();
                    assert!(rendered.contains("canary-pack@1.0.0"), "{rendered}");
                    assert!(rendered.contains(&hash), "{rendered}");
                    assert!(
                        rendered.contains("検証パックの供給元: ローカル"),
                        "a local pack must not read as admitted: {rendered}"
                    );

                    let mut shell = boundary_shell_for(&config);
                    shell
                        .begin_gate_one_with_locator_and_recovery(
                            python_cli_proposal(),
                            "Python CLI filter",
                            &config.workspace_root,
                            pins(),
                            card.selection.clone(),
                            crate::tui::boundary_shell::GateOneOptions {
                                locator: Some(context.locator()),
                                recovery_plan_auto_runs: 0,
                            },
                        )
                        .unwrap();
                    let card_hash = match shell.state() {
                        crate::tui::boundary_shell::BoundaryState::AwaitingConfirmation {
                            card_hash,
                            ..
                        } => card_hash.clone(),
                        state => panic!("unexpected state: {state:?}"),
                    };
                    shell.confirm(&card_hash).unwrap();
                    // The production entry installs the pack's environment under
                    // its own guard; this test never touches it directly.
                    let ran = shell
                        .dispatch(|identity| {
                            with_confirmed_pack_in(&context, &config, identity, || Ok(()))
                                .map(|()| "ran".to_string())
                        })
                        .unwrap();
                    assert_eq!(ran, "ran");

                    let events =
                        std::fs::read_to_string(config.eval_events_path.as_ref().unwrap()).unwrap();
                    assert!(
                        events.contains("\"pack_source\":\"local\""),
                        "the dispatch did not inject the local pack: {events}"
                    );
                    assert!(
                        events.contains(&format!("\"pack_hash\":\"{hash}\"")),
                        "{events}"
                    );
                    assert!(
                        !events.contains("\"pack_source\":\"admitted\""),
                        "the local pack was treated as admitted: {events}"
                    );
                    // The refusals and the events never quote the pack bytes.
                    assert!(!events.contains("held-extension"), "{events}");
                    // The second measurement point: the same context against a
                    // startup pack whose bytes changed is refused, not dispatched.
                    std::fs::write(
                        extension.join("canary-pack/1.0.0/assist.yaml"),
                        pack_selection::tests::assist("canary-pack", "1.0.0", "changed"),
                    )
                    .unwrap();
                    let refused = context.startup_refusal().unwrap();
                    assert!(refused.contains("--pack canary-pack@1.0.0"), "{refused}");
                    assert!(!refused.contains("changed"), "{refused}");
                });
            },
        );
    }

    /// A `--pack` that holds an admitted repository pack still revalidates: the
    /// same bytes resolve from the repository root as admitted, and a source
    /// that merely names the same root must not refuse them.
    #[test]
    fn a_held_admitted_repository_pack_revalidates_without_a_false_refusal() {
        let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let hash = crate::planner::pack::catalog::admitted_packs()
            .iter()
            .find(|pack| pack.id == "cli-assist" && pack.version == "1.1.0")
            .unwrap()
            .hash
            .to_string();
        let startup = crate::cli_pack::ResolvedPack {
            id: "cli-assist".to_string(),
            version: "1.1.0".to_string(),
            hash,
            source: crate::cli_pack::SelectionSource::Repository,
            directory: repository.join("packs/cli-assist/1.1.0"),
            declared: None,
        };
        let context = pack_selection::ReplPackContext::with_startup(
            repository.to_path_buf(),
            None,
            Some(startup),
        );
        assert_eq!(context.startup_refusal(), None);
        let (_, selection) = context.startup_selection().unwrap().unwrap();
        assert!(matches!(
            selection,
            PackSelection::Pinned {
                source: crate::planner::pack::catalog::PackSource::Admitted,
                ..
            }
        ));
    }

    /// Issue #1057: after a card is confirmed, changing the frozen pack's bytes
    /// makes dispatch refuse it, and it never falls back to another root's pack.
    /// Nothing is injected, and the refusal does not quote the pack's contents.
    #[test]
    fn a_confirmed_pack_changed_before_dispatch_is_refused_without_falling_back() {
        use crate::tui::boundary_shell::confirmation::PackSelection;

        let temp = tempfile::tempdir().unwrap();
        let workspace = temp.path().join("workspace");
        let extension = temp.path().join("extension");
        let hash = pinned_pack(&extension, "canary-pack", "confirmed-a");
        pinned_pack(&workspace.join("packs"), "canary-pack", "workspace-other");
        let mut config = Config::from_cli(crate::cli::Cli::parse_from([
            "commandagent",
            "--cwd",
            workspace.to_str().unwrap(),
            "--profile",
            "python-cli",
            "--intent",
            "create",
        ]))
        .unwrap();
        config.eval_events_path = Some(temp.path().join("events.jsonl"));
        let context =
            pack_selection::ReplPackContext::new(workspace.clone(), Some(extension.clone()));

        let mut identity = boundary_identity(&config, PackSelection::None, context.locator());
        let selection = context
            .select("python-cli", "create", "canary-pack@1.0.0")
            .unwrap();
        identity.pack = selection;
        assert!(matches!(
            &identity.pack,
            PackSelection::Pinned { hash: h, source, .. }
                if h == &hash && *source == crate::planner::pack::catalog::PackSource::Local
        ));

        // The card changed after it was confirmed: the same selector, other
        // bytes under the extension root.
        std::fs::write(
            extension.join("canary-pack/1.0.0/assist.yaml"),
            pack_selection::tests::assist("canary-pack", "1.0.0", "changed-after-confirm"),
        )
        .unwrap();
        let refused = with_confirmed_pack_in(&context, &config, &identity, || Ok(()))
            .unwrap_err()
            .to_string();
        assert!(!refused.contains("changed-after-confirm"), "{refused}");
        assert!(!refused.contains("workspace-other"), "{refused}");
        let events =
            std::fs::read_to_string(config.eval_events_path.as_ref().unwrap()).unwrap_or_default();
        assert!(!events.contains("pack_injected"), "{events}");
        assert!(!events.contains("workspace-other"), "{events}");
    }

    /// Build the Gate 1 identity a card is rendered from, for the pack test. It
    /// is validated through the same locator the card renders through.
    fn boundary_identity(
        config: &Config,
        pack: PackSelection,
        locator: &PackLocator,
    ) -> crate::tui::boundary_shell::confirmation::ConfirmationIdentity {
        crate::tui::boundary_shell::confirmation::ConfirmationIdentity::new_with_locator(
            "Python CLI filter".to_string(),
            &config.workspace_root,
            &python_cli_route(),
            crate::tui::boundary_shell::band_catalog::value_for(
                "python-cli",
                IntentId::Create,
                TaskFamilyId::Filter,
            )
            .unwrap(),
            pins(),
            pack,
            locator,
        )
        .unwrap()
    }

    /// Issue #1057: a `--pack` held at startup fixes that pack on a request that
    /// names no selector, and a request the held pack cannot serve is refused
    /// with its reason instead of ending the REPL.
    #[test]
    fn a_held_pack_defaults_a_selector_less_request_and_a_mismatch_is_refused() {
        crate::cli_pack::tests::own_process::run(
            "tui::repl::tests::a_held_pack_defaults_a_selector_less_request_and_a_mismatch_is_refused",
            || {
                within_held_pack_deadline(|| {
                    let temp = tempfile::tempdir().unwrap();
                    let workspace = temp.path().join("workspace");
                    let extension = temp.path().join("extension");
                    std::fs::create_dir_all(&workspace).unwrap();
                    let hash = pinned_pack(&extension, "canary-pack", "held-default");

                    let startup = crate::cli_pack::ResolvedPack {
                        id: "canary-pack".to_string(),
                        version: "1.0.0".to_string(),
                        hash: hash.clone(),
                        source: crate::cli_pack::SelectionSource::ExtensionRoot,
                        directory: extension.join("canary-pack/1.0.0"),
                        declared: None,
                    };
                    let context = pack_selection::ReplPackContext::with_startup(
                        workspace.clone(),
                        Some(extension.clone()),
                        Some(startup),
                    );

                    let frozen = gate_one_pack(&context, Some(&python_cli_route()), None).unwrap();
                    assert!(
                        matches!(
                            &frozen.selection,
                            PackSelection::Pinned { id, hash: pinned, source, .. }
                                if id == "canary-pack"
                                    && pinned == &hash
                                    && *source == crate::planner::pack::catalog::PackSource::Local
                        ),
                        "{frozen:?}"
                    );

                    // A held pack that a different route cannot serve is refused
                    // with its reason before anything is chosen.
                    let nextjs = crate::tui::boundary_shell::route::RouteCandidate {
                        profile: ProfileId::Nextjs,
                        intent: IntentId::Create,
                        family: TaskFamilyId::Quiz,
                        bases: vec![RouteBasis {
                            rule: "fixture",
                            observation: "quiz".to_string(),
                        }],
                        contract_ref: "docs/nextjs-profile-contract.md",
                    };
                    let refused = gate_one_pack(&context, Some(&nextjs), None).unwrap_err();
                    assert!(refused.contains("canary-pack@1.0.0"), "{refused}");
                    assert!(!refused.contains("held-default"), "{refused}");

                    // A selectorless request under no held pack stays as it was.
                    let unheld = pack_selection::ReplPackContext::new(
                        workspace.clone(),
                        Some(extension.clone()),
                    );
                    assert!(matches!(
                        gate_one_pack(&unheld, Some(&python_cli_route()), None)
                            .unwrap()
                            .selection,
                        PackSelection::None
                    ));
                });
            },
        );
    }

    /// Issue #1057: a Gate 1 selector resolves through the CLI's roots whatever
    /// route will be proposed. The extension root shadows the workspace, a
    /// selector outside the roots is refused with a reason that never quotes the
    /// pack, and the REPL keeps taking input. (The refusal under a *held* pack
    /// is covered by `gate_one_and_gate_four_refuse_another_pack_under_a_held_pack`.)
    #[test]
    fn a_gate_one_selector_resolves_through_the_extension_root_first() {
        let temp = tempfile::tempdir().unwrap();
        let workspace = temp.path().join("workspace");
        let extension = temp.path().join("extension");
        std::fs::create_dir_all(&workspace).unwrap();
        let local = pinned_pack(&extension, "canary-pack", "held-selector");
        let repository = pinned_pack(&workspace.join("packs"), "canary-pack", "workspace-other");
        assert_ne!(local, repository, "the roots must hold different bytes");

        let context =
            pack_selection::ReplPackContext::new(workspace.clone(), Some(extension.clone()));
        assert_eq!(context.resolved_hash("canary-pack@1.0.0"), Some(local));

        // The selector is not under the roots.
        assert!(context.resolved_hash("missing-pack@1.0.0").is_none());
        let missing = context.selector_refusal("missing-pack@1.0.0");
        assert!(!missing.contains("held-selector"), "{missing}");

        // Changing the held pack's bytes under the extension root makes the
        // selector unreadable there. The workspace copy is never adopted, and
        // the reason does not quote the bytes.
        std::fs::write(
            extension.join("canary-pack/1.0.0/assist.yaml"),
            pack_selection::tests::assist("canary-pack", "1.0.0", "changed"),
        )
        .unwrap();
        // The REPL keeps taking input after a refusal.
        assert!(context.resolved_hash("canary-pack@1.0.0").is_none());
        assert!(
            !context
                .selector_refusal("canary-pack@1.0.0")
                .contains("changed")
        );
        assert!(
            !context
                .selector_refusal("canary-pack@1.0.0")
                .contains("workspace-other")
        );
    }

    // ---- Issue #1057 review regressions (independent review, 3 blockers) ----

    /// Blocker 1: a startup rejection must not print a registered value. Before
    /// the REPL prompt exists, the process entry point prints `error: {err:#}`,
    /// so the refusal is masked with the run's registered scope and the pin file
    /// value is never transcribed.
    #[test]
    fn a_startup_pin_mismatch_never_prints_the_registered_value() {
        use crate::sensitive_data::{SecretCatalog, install_scope, reset_scopes_for_tests};

        const CANARY: &str = "PACK_CANARY_27_canary_4f1";
        let temp = tempfile::tempdir().unwrap();
        let workspace = temp.path().join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();

        // A valid local pack whose pin is made inconsistent, holding the canary
        // as the pin file's value.
        let pack_root = temp.path().join("extension");
        crate::tui::repl::pack_selection::tests::write_pack(
            &pack_root,
            "canary-pack",
            "1.0.0",
            &crate::tui::repl::pack_selection::tests::assist("canary-pack", "1.0.0", "startup-pin"),
        );
        std::fs::write(
            pack_root.join("canary-pack/1.0.0/pack.sha256"),
            format!("{CANARY}\n"),
        )
        .unwrap();

        // Register the same value as a credential in the run's scope, as the
        // config path does for a `${ENV}` expansion.
        reset_scopes_for_tests();
        let mut catalog = SecretCatalog::new();
        catalog
            .register_credential("SYNTHETIC_KEY", CANARY)
            .unwrap();
        let _scope = install_scope(catalog, Some(&workspace), None);

        let cli = crate::cli::Cli::parse_from([
            "commandagent",
            "--cwd",
            workspace.to_str().unwrap(),
            "--profile",
            "python-cli",
            "--intent",
            "create",
            "--extension-root",
            pack_root.to_str().unwrap(),
            "--pack",
            "canary-pack@1.0.0",
        ]);
        let config = Config::from_cli(cli.clone()).unwrap();
        let refusal = crate::cli_pack::resolve(&cli, &config)
            .unwrap_err()
            .to_string();
        reset_scopes_for_tests();

        // The rejection names the failing check and the observed hash, never the
        // registered value the pin file carried.
        assert!(refusal.contains("pin"), "{refusal}");
        assert!(
            !refusal.contains(CANARY),
            "the registered value reached the refusal: {refusal}"
        );
    }

    /// Blocker 2: a confirmed card whose pack changed since it was shown is
    /// refused before any confirmation, dispatch, or terminal is recorded, and
    /// the REPL keeps reading input. The same selector with the same bytes in
    /// the `packs/` compatibility layout must not pass for the old directory.
    #[test]
    fn a_pack_changed_after_the_card_is_refused_before_confirmation_and_records_nothing() {
        use crate::tui::boundary_shell::confirmation::PackSelection;

        crate::cli_pack::tests::own_process::run(
            "tui::repl::tests::a_pack_changed_after_the_card_is_refused_before_confirmation_and_records_nothing",
            || {
                within_held_pack_deadline(|| {
                    let temp = tempfile::tempdir().unwrap();
                    let workspace = temp.path().join("workspace");
                    let extension = temp.path().join("extension");
                    std::fs::create_dir_all(&workspace).unwrap();
                    let bytes =
                        pack_selection::tests::assist("canary-pack", "1.0.0", "card-freeze");
                    let hash = pack_selection::tests::write_pack(
                        &extension,
                        "canary-pack",
                        "1.0.0",
                        &bytes,
                    );

                    let mut config = Config::from_cli(crate::cli::Cli::parse_from([
                        "commandagent",
                        "--cwd",
                        workspace.to_str().unwrap(),
                        "--profile",
                        "python-cli",
                        "--intent",
                        "create",
                    ]))
                    .unwrap();
                    config.eval_events_path = Some(temp.path().join("events.jsonl"));
                    config.state_dir = temp.path().join("state");
                    let context = pack_selection::ReplPackContext::new(
                        workspace.clone(),
                        Some(extension.clone()),
                    );

                    // Gate 1 freezes this directory.
                    let (selection, directory) = context
                        .select_frozen("python-cli", "create", "canary-pack@1.0.0")
                        .unwrap();
                    assert_eq!(
                        selection,
                        PackSelection::Pinned {
                            id: "canary-pack".to_string(),
                            version: "1.0.0".to_string(),
                            hash: hash.clone(),
                            point: "cli-validation".to_string(),
                            source: crate::planner::pack::catalog::PackSource::Local,
                        }
                    );
                    assert_eq!(directory, extension.join("canary-pack/1.0.0"));

                    let mut shell = boundary_shell_for(&config);
                    shell
                        .begin_gate_one_with_locator_and_recovery(
                            python_cli_proposal(),
                            "Python CLI filter",
                            &config.workspace_root,
                            pins(),
                            selection.clone(),
                            crate::tui::boundary_shell::GateOneOptions {
                                locator: Some(context.locator()),
                                recovery_plan_auto_runs: 0,
                            },
                        )
                        .unwrap();

                    // The pack changes (same selector, other bytes, re-pinned)
                    // after the card was shown. Confirmation is refused before
                    // anything is recorded, and no confirmation record or event
                    // exists.
                    let changed =
                        pack_selection::tests::assist("canary-pack", "1.0.0", "changed-after-card");
                    std::fs::write(extension.join("canary-pack/1.0.0/assist.yaml"), &changed)
                        .unwrap();
                    let changed_hash =
                        crate::planner::pack::exact_byte_hash(Some(changed.as_bytes()), None);
                    assert_ne!(changed_hash, hash, "the bytes did not change");
                    std::fs::write(
                        extension.join("canary-pack/1.0.0/pack.sha256"),
                        format!("{changed_hash}\n"),
                    )
                    .unwrap();
                    let card_hash = match shell.state() {
                        crate::tui::boundary_shell::BoundaryState::AwaitingConfirmation {
                            card_hash,
                            ..
                        } => card_hash.clone(),
                        state => panic!("unexpected state: {state:?}"),
                    };
                    let refused = confirm_gate_one(
                        &mut shell,
                        &config,
                        &context,
                        Some(&directory),
                        &card_hash,
                    )
                    .unwrap_err();
                    assert!(refused.starts_with("Confirmation refused: "), "{refused}");
                    assert!(
                        refused.contains("changed since the Gate 1 card was shown"),
                        "{refused}"
                    );
                    assert!(!refused.contains("changed-after-card"), "{refused}");
                    assert!(matches!(
                        shell.state(),
                        crate::tui::boundary_shell::BoundaryState::AwaitingConfirmation { .. }
                    ));
                    let confirmations = config.state_dir.join("boundary-confirmations");
                    assert_eq!(
                        std::fs::read_dir(&confirmations).map_or(0, |e| e.count()),
                        0,
                        "a refused confirmation wrote a record"
                    );
                    let events = std::fs::read_to_string(config.eval_events_path.as_ref().unwrap())
                        .unwrap_or_default();
                    assert!(
                        !events.contains("\"event\":\"route_confirmed\""),
                        "{events}"
                    );
                    assert!(!events.contains("pack_injected"), "{events}");

                    // The REPL keeps reading input: a fresh card for the new
                    // bytes confirms against its own frozen directory.
                    let (fresh, fresh_directory) = context
                        .select_frozen("python-cli", "create", "canary-pack@1.0.0")
                        .unwrap();
                    assert_eq!(
                        fresh_directory, directory,
                        "the changed bytes stay in the same directory"
                    );
                    assert!(
                        matches!(
                            &fresh,
                            PackSelection::Pinned { hash: h, .. } if h == &changed_hash
                        ),
                        "{fresh:?}"
                    );
                    let mut shell = boundary_shell_for(&config);
                    shell
                        .begin_gate_one_with_locator_and_recovery(
                            python_cli_proposal(),
                            "Python CLI filter",
                            &config.workspace_root,
                            pins(),
                            fresh,
                            crate::tui::boundary_shell::GateOneOptions {
                                locator: Some(context.locator()),
                                recovery_plan_auto_runs: 0,
                            },
                        )
                        .unwrap();
                    let fresh_hash = match shell.state() {
                        crate::tui::boundary_shell::BoundaryState::AwaitingConfirmation {
                            card_hash,
                            ..
                        } => card_hash.clone(),
                        state => panic!("unexpected state: {state:?}"),
                    };
                    confirm_gate_one(
                        &mut shell,
                        &config,
                        &context,
                        Some(&fresh_directory),
                        &fresh_hash,
                    )
                    .unwrap();
                });
            },
        );
    }

    /// Blocker 2 (compatible layout): removing the served directory while the
    /// same bytes remain under `packs/` must not confirm the old ID. The frozen
    /// directory is verified directly, never re-searched.
    #[test]
    fn a_frozen_directory_removed_from_a_compatible_layout_does_not_rebind() {
        crate::cli_pack::tests::own_process::run(
            "tui::repl::tests::a_frozen_directory_removed_from_a_compatible_layout_does_not_rebind",
            || {
                within_held_pack_deadline(|| {
                    let temp = tempfile::tempdir().unwrap();
                    let workspace = temp.path().join("workspace");
                    let extension = temp.path().join("extension");
                    std::fs::create_dir_all(&workspace).unwrap();
                    let bytes = pack_selection::tests::assist("canary-pack", "1.0.0", "compat");

                    // The served directory is the direct layout; the same bytes
                    // also sit under the `packs/` compatibility layout.
                    pack_selection::tests::write_pack(&extension, "canary-pack", "1.0.0", &bytes);
                    pack_selection::tests::write_pack(
                        &extension.join("packs"),
                        "canary-pack",
                        "1.0.0",
                        &bytes,
                    );

                    let context = pack_selection::ReplPackContext::new(
                        workspace.clone(),
                        Some(extension.clone()),
                    );
                    let (selection, directory) = context
                        .select_frozen("python-cli", "create", "canary-pack@1.0.0")
                        .unwrap();
                    assert_eq!(directory, extension.join("canary-pack/1.0.0"));

                    // The served direct directory is moved away; the same bytes
                    // remain under `packs/`, which must not be adopted for the
                    // old card.
                    std::fs::rename(
                        extension.join("canary-pack"),
                        extension.join("canary-pack-moved"),
                    )
                    .unwrap();
                    assert!(!directory.exists());

                    let refused = context
                        .revalidate_frozen("python-cli", "create", &selection, &directory)
                        .is_err();
                    assert!(refused, "the moved pack was revalidated");
                    // The old card never re-binds to the compatible layout: a
                    // fresh selection answers from `packs/`, which is a different
                    // directory and a different card.
                    let served = context
                        .select_frozen("python-cli", "create", "canary-pack@1.0.0")
                        .map(|(_, served)| served)
                        .unwrap();
                    assert_eq!(
                        served,
                        extension.join("packs/canary-pack/1.0.0"),
                        "only the compatibility layout remains"
                    );
                    assert_ne!(
                        served, directory,
                        "the compatibility layout is not the confirmed directory"
                    );
                });
            },
        );
    }

    /// Blocker 3: a rightly confirmed local pack renders and records its Gate 4
    /// terminal, not an admitted-only error. Same for a repository pack.
    #[test]
    fn a_confirmed_local_pack_renders_and_records_its_gate_four_terminal() {
        use crate::tui::boundary_shell::confirmation::PackSelection;

        let temp = tempfile::tempdir().unwrap();
        let workspace = temp.path().join("workspace");
        let extension = temp.path().join("extension");
        std::fs::create_dir_all(&workspace).unwrap();
        let hash = pack_selection::tests::write_pack(
            &extension,
            "canary-pack",
            "1.0.0",
            &pack_selection::tests::assist("canary-pack", "1.0.0", "terminal"),
        );

        let mut config = Config::from_cli(crate::cli::Cli::parse_from([
            "commandagent",
            "--cwd",
            workspace.to_str().unwrap(),
            "--profile",
            "python-cli",
            "--intent",
            "create",
        ]))
        .unwrap();
        config.eval_events_path = Some(temp.path().join("events.jsonl"));
        config.state_dir = temp.path().join("state");
        let context =
            pack_selection::ReplPackContext::new(workspace.clone(), Some(extension.clone()));

        let selection = PackSelection::Pinned {
            id: "canary-pack".to_string(),
            version: "1.0.0".to_string(),
            hash,
            point: "cli-validation".to_string(),
            source: crate::planner::pack::catalog::PackSource::Local,
        };
        let mut shell = boundary_shell_for(&config);
        let identity = shell
            .begin_gate_one_with_locator_and_recovery(
                python_cli_proposal(),
                "Python CLI filter",
                &config.workspace_root,
                pins(),
                selection.clone(),
                crate::tui::boundary_shell::GateOneOptions {
                    locator: Some(context.locator()),
                    recovery_plan_auto_runs: 0,
                },
            )
            .unwrap()
            .clone();
        let card = identity.card_hash().unwrap();
        shell.confirm(&card).unwrap();
        shell.dispatch(|_| Ok("failed".to_string())).unwrap();
        crate::eval_events::emit(
            config.eval_events_path.as_deref(),
            serde_json::json!({
                "event": "tui_command_stop",
                "profile": "python-cli",
                "status": "failed",
                "ok": false,
                "stop_reason": "failed",
            }),
        );
        let generated = crate::tui::boundary_shell::sheet::generate(
            &identity,
            config.eval_events_path.as_deref(),
            false,
        )
        .unwrap();
        let terminal = shell
            .present_terminal(generated.markdown, generated.full, generated.section5)
            .unwrap();

        // The admitted-only renderer refuses this local pack; the locator-aware
        // one, which the REPL uses, renders and records the terminal.
        assert!(
            crate::tui::boundary_shell::presentation::render_gate_four(&identity, terminal, &[],)
                .is_err(),
            "the admitted-only renderer must refuse a local pack"
        );
        let rendered = render_terminal(&identity, terminal, &context).unwrap();
        assert!(rendered.contains("# Gate 4"), "{rendered}");
        assert!(rendered.contains("canary-pack@1.0.0"), "{rendered}");
    }

    // ---- Issue #1057 review r2 regressions (dispatch-time pack changes) ----

    /// Blocker 1 (r2): a pin that changes *during* the run makes the terminal
    /// render fail, and that failure must be masked with the run's registered
    /// scope before it is displayed — exactly as a startup refusal is. The
    /// catalog's pin-mismatch reason also never transcribes the pin value.
    #[test]
    fn a_runtime_pin_change_is_masked_before_the_terminal_error_is_displayed() {
        use crate::sensitive_data::{SecretCatalog, install_scope, reset_scopes_for_tests};

        const CANARY: &str = "PACK_CANARY_r2_pin_9c3";
        let temp = tempfile::tempdir().unwrap();
        let workspace = temp.path().join("workspace");
        let extension = temp.path().join("extension");
        std::fs::create_dir_all(&workspace).unwrap();
        let bytes = pack_selection::tests::assist("canary-pack", "1.0.0", "runtime-pin");
        pack_selection::tests::write_pack(&extension, "canary-pack", "1.0.0", &bytes);
        let context =
            pack_selection::ReplPackContext::new(workspace.clone(), Some(extension.clone()));

        // Register the canary, then make the served pin equal it (mismatched
        // against the observed bytes).
        reset_scopes_for_tests();
        let mut catalog = SecretCatalog::new();
        catalog
            .register_credential("SYNTHETIC_KEY", CANARY)
            .unwrap();
        let _scope = install_scope(catalog, Some(&workspace), None);
        std::fs::write(
            extension.join("canary-pack/1.0.0/pack.sha256"),
            format!("{CANARY}\n"),
        )
        .unwrap();

        let selection = crate::tui::boundary_shell::confirmation::PackSelection::Pinned {
            id: "canary-pack".to_string(),
            version: "1.0.0".to_string(),
            hash: crate::planner::pack::exact_byte_hash(Some(bytes.as_bytes()), None),
            point: "cli-validation".to_string(),
            source: crate::planner::pack::catalog::PackSource::Local,
        };
        let directory = extension.join("canary-pack/1.0.0");
        // (a) The catalog's own pin-mismatch reason never transcribes the pin
        // value, even before any masking.
        let catalog_error = context
            .revalidate_frozen("python-cli", "create", &selection, &directory)
            .unwrap_err();
        assert!(
            !format!("{catalog_error:#}").contains(CANARY),
            "the pin value leaked from the catalog reason"
        );

        // (b) A failure that *does* carry a registered value (as the r2 error
        // did) is masked before it reaches the terminal. The render function is
        // the display boundary under test.
        let identity = crate::tui::boundary_shell::confirmation::ConfirmationIdentity::new(
            "Python CLI filter".to_string(),
            &workspace,
            &python_cli_route(),
            crate::tui::boundary_shell::band_catalog::value_for(
                "python-cli",
                IntentId::Create,
                TaskFamilyId::Filter,
            )
            .unwrap(),
            pins(),
            crate::tui::boundary_shell::confirmation::PackSelection::None,
        )
        .unwrap();
        let value_carrying = anyhow::anyhow!(
            "pack `canary-pack@1.0.0` pin mismatch: expected `{CANARY}`, observed `sha256:other`"
        );
        let rendered = render_terminal_after_pack_change(
            &identity,
            &terminal_for(&identity, false),
            &value_carrying,
        );
        reset_scopes_for_tests();

        assert!(rendered.contains("再確認に失敗"), "{rendered}");
        assert!(
            !rendered.contains(CANARY),
            "the registered value reached the terminal: {rendered}"
        );
        assert!(rendered.contains("<redacted>"), "{rendered}");
    }

    /// Blocker 2 (r2): when the pack moves *during* the run, its post-dispatch
    /// revalidation fails. The terminal must still be recorded to the transcript
    /// with the masked reason, the run's stop event and sheet must be untouched,
    /// and no error may escape to end the REPL.
    #[test]
    fn a_runtime_pack_move_still_records_the_gate_four_transcript() {
        use crate::sensitive_data::{SecretCatalog, install_scope, reset_scopes_for_tests};

        const CANARY: &str = "PACK_CANARY_r2_move_5b1";
        let temp = tempfile::tempdir().unwrap();
        let workspace = temp.path().join("workspace");
        let extension = temp.path().join("extension");
        std::fs::create_dir_all(&workspace).unwrap();
        let bytes = pack_selection::tests::assist("canary-pack", "1.0.0", "runtime-move");
        let hash = pack_selection::tests::write_pack(&extension, "canary-pack", "1.0.0", &bytes);

        let mut config = Config::from_cli(crate::cli::Cli::parse_from([
            "commandagent",
            "--cwd",
            workspace.to_str().unwrap(),
            "--profile",
            "python-cli",
            "--intent",
            "create",
        ]))
        .unwrap();
        config.eval_events_path = Some(temp.path().join("events.jsonl"));
        config.state_dir = temp.path().join("state");
        let context =
            pack_selection::ReplPackContext::new(workspace.clone(), Some(extension.clone()));

        let selection = crate::tui::boundary_shell::confirmation::PackSelection::Pinned {
            id: "canary-pack".to_string(),
            version: "1.0.0".to_string(),
            hash,
            point: "cli-validation".to_string(),
            source: crate::planner::pack::catalog::PackSource::Local,
        };
        let identity = selection_identity_for(&workspace, Some(&extension), &selection);
        let mut shell = boundary_shell_for(&config);
        let card = shell
            .begin_gate_one_with_locator_and_recovery(
                python_cli_proposal(),
                "Python CLI filter",
                &config.workspace_root,
                pins(),
                selection.clone(),
                crate::tui::boundary_shell::GateOneOptions {
                    locator: Some(context.locator()),
                    recovery_plan_auto_runs: 0,
                },
            )
            .unwrap()
            .card_hash()
            .unwrap();
        shell.confirm(&card).unwrap();
        shell.dispatch(|_| Ok("failed".to_string())).unwrap();
        // The run's stop evidence is recorded before the pack moves.
        let stop = serde_json::json!({
            "event": "tui_command_stop",
            "profile": "python-cli",
            "status": "failed",
            "ok": false,
            "stop_reason": "failed",
            "runtime_acceptance_status": "failed",
        });
        crate::eval_events::emit(config.eval_events_path.as_deref(), stop);
        let generated = crate::tui::boundary_shell::sheet::generate(
            &identity,
            config.eval_events_path.as_deref(),
            false,
        )
        .unwrap();
        let sheet_path =
            crate::tui::boundary_shell::sheet::persist(&config.state_dir, &identity, &generated)
                .unwrap();
        let stop_before =
            std::fs::read_to_string(config.eval_events_path.as_ref().unwrap()).unwrap();
        let terminal = shell
            .present_terminal(generated.markdown, generated.full, generated.section5)
            .unwrap();

        // The pack moves during the run; revalidation now fails.
        std::fs::rename(
            extension.join("canary-pack"),
            extension.join("canary-pack-moved"),
        )
        .unwrap();
        reset_scopes_for_tests();
        let mut catalog = SecretCatalog::new();
        catalog
            .register_credential("SYNTHETIC_KEY", CANARY)
            .unwrap();
        let _scope = install_scope(catalog, Some(&workspace), None);

        let rendered =
            present_and_record_terminal(&config, &identity, terminal, &context, &sheet_path)
                .unwrap();
        reset_scopes_for_tests();

        // No error escaped (the call above returned Ok): the REPL can continue.
        // The Gate 4 transcript is recorded with the masked reason.
        let transcript =
            std::fs::read_to_string(config.state_dir.join("boundary-transcript.md")).unwrap();
        assert!(transcript.contains("## Gate 4"), "{transcript}");
        assert!(transcript.contains("再確認に失敗"), "{transcript}");
        assert!(transcript.contains("Sheet path:"), "{transcript}");
        assert!(!transcript.contains(CANARY), "{transcript}");
        assert!(
            rendered.contains("## Gate 4") || rendered.contains("# Gate 4"),
            "{rendered}"
        );
        // The run's stop event and sheet are unchanged.
        assert_eq!(
            std::fs::read_to_string(config.eval_events_path.as_ref().unwrap()).unwrap(),
            stop_before,
            "the stop event changed"
        );
        assert!(sheet_path.exists(), "the sheet was removed");
    }

    /// Blocker (r3): the transcript's `Sheet path:` line must be masked with the
    /// run's registered scope, exactly as the reason text is, for a normal run
    /// and a revalidation-failing run alike. A `--state-dir` that contains a
    /// registered value must not persist it through the sheet path.
    #[test]
    fn a_sheet_path_in_the_transcript_is_masked_for_normal_and_failing_runs() {
        use crate::sensitive_data::{SecretCatalog, install_scope, reset_scopes_for_tests};

        const CANARY: &str = "PACK_CANARY_r3_sheet_7a4";
        let temp = tempfile::tempdir().unwrap();
        let workspace = temp.path().join("workspace");
        let extension = temp.path().join("extension");
        std::fs::create_dir_all(&workspace).unwrap();
        let bytes = pack_selection::tests::assist("canary-pack", "1.0.0", "runtime-sheet");
        let hash = pack_selection::tests::write_pack(&extension, "canary-pack", "1.0.0", &bytes);

        let mut config = Config::from_cli(crate::cli::Cli::parse_from([
            "commandagent",
            "--cwd",
            workspace.to_str().unwrap(),
            "--profile",
            "python-cli",
            "--intent",
            "create",
        ]))
        .unwrap();
        // The state dir (and so the persisted sheet path) contains the value.
        config.state_dir = temp.path().join(format!("state-{CANARY}"));
        let context =
            pack_selection::ReplPackContext::new(workspace.clone(), Some(extension.clone()));

        let selection = crate::tui::boundary_shell::confirmation::PackSelection::Pinned {
            id: "canary-pack".to_string(),
            version: "1.0.0".to_string(),
            hash,
            point: "cli-validation".to_string(),
            source: crate::planner::pack::catalog::PackSource::Local,
        };
        let identity = selection_identity_for(&workspace, Some(&extension), &selection);
        let terminal = terminal_for(&identity, false);
        let generated = crate::tui::boundary_shell::sheet::GeneratedSheet {
            markdown: "# sheet".to_string(),
            full: false,
            section5: Some("failed".to_string()),
        };
        let sheet_path =
            crate::tui::boundary_shell::sheet::persist(&config.state_dir, &identity, &generated)
                .unwrap();
        assert!(
            sheet_path.display().to_string().contains(CANARY),
            "the fixture does not put the value in the sheet path"
        );

        reset_scopes_for_tests();
        let mut catalog = SecretCatalog::new();
        catalog
            .register_credential("SYNTHETIC_KEY", CANARY)
            .unwrap();
        let _scope = install_scope(catalog, Some(&workspace), None);

        // A normal run: the pack is readable, so the terminal renders.
        let normal =
            present_and_record_terminal(&config, &identity, &terminal, &context, &sheet_path)
                .unwrap();
        // A revalidation-failing run: the pack moves during the run.
        std::fs::rename(
            extension.join("canary-pack"),
            extension.join("canary-pack-moved"),
        )
        .unwrap();
        let failing =
            present_and_record_terminal(&config, &identity, &terminal, &context, &sheet_path)
                .unwrap();
        reset_scopes_for_tests();

        let transcript =
            std::fs::read_to_string(config.state_dir.join("boundary-transcript.md")).unwrap();
        // Both terminals are recorded, each with a `Sheet path:` line.
        assert_eq!(transcript.matches("## Gate 4").count(), 2, "{transcript}");
        assert_eq!(transcript.matches("Sheet path:").count(), 2, "{transcript}");
        assert!(transcript.contains("<redacted>"), "{transcript}");
        // The value never reaches the transcript or the terminal.
        assert!(
            !transcript.contains(CANARY),
            "the sheet path leaked into the transcript: {transcript}"
        );
        assert!(!normal.contains(CANARY), "{normal}");
        assert!(!failing.contains(CANARY), "{failing}");
    }

    /// Blocker (r4): the fixed label `Sheet path: ` must not itself be masked.
    /// Registering that label as a value must leave the record's structure intact
    /// for a normal run and a revalidation-failing run alike.
    #[test]
    fn the_sheet_path_label_survives_when_it_is_a_registered_value() {
        use crate::sensitive_data::{SecretCatalog, install_scope, reset_scopes_for_tests};

        const LABEL: &str = "Sheet path: ";
        let temp = tempfile::tempdir().unwrap();
        let workspace = temp.path().join("workspace");
        let extension = temp.path().join("extension");
        std::fs::create_dir_all(&workspace).unwrap();
        let bytes = pack_selection::tests::assist("canary-pack", "1.0.0", "r4-label");
        let hash = pack_selection::tests::write_pack(&extension, "canary-pack", "1.0.0", &bytes);

        let mut config = Config::from_cli(crate::cli::Cli::parse_from([
            "commandagent",
            "--cwd",
            workspace.to_str().unwrap(),
            "--profile",
            "python-cli",
            "--intent",
            "create",
        ]))
        .unwrap();
        config.state_dir = temp.path().join("state");
        let context =
            pack_selection::ReplPackContext::new(workspace.clone(), Some(extension.clone()));
        let selection = crate::tui::boundary_shell::confirmation::PackSelection::Pinned {
            id: "canary-pack".to_string(),
            version: "1.0.0".to_string(),
            hash,
            point: "cli-validation".to_string(),
            source: crate::planner::pack::catalog::PackSource::Local,
        };
        let identity = selection_identity_for(&workspace, Some(&extension), &selection);
        let terminal = terminal_for(&identity, false);
        let generated = crate::tui::boundary_shell::sheet::GeneratedSheet {
            markdown: "# sheet".to_string(),
            full: false,
            section5: Some("failed".to_string()),
        };
        let sheet_path =
            crate::tui::boundary_shell::sheet::persist(&config.state_dir, &identity, &generated)
                .unwrap();

        reset_scopes_for_tests();
        let mut catalog = SecretCatalog::new();
        catalog.register_credential("SYNTHETIC_KEY", LABEL).unwrap();
        let _scope = install_scope(catalog, Some(&workspace), None);

        let normal =
            present_and_record_terminal(&config, &identity, &terminal, &context, &sheet_path)
                .unwrap();
        std::fs::rename(
            extension.join("canary-pack"),
            extension.join("canary-pack-moved"),
        )
        .unwrap();
        let failing =
            present_and_record_terminal(&config, &identity, &terminal, &context, &sheet_path)
                .unwrap();
        reset_scopes_for_tests();

        let transcript =
            std::fs::read_to_string(config.state_dir.join("boundary-transcript.md")).unwrap();
        // The structure is intact: the label is not rewritten to a marker.
        assert_eq!(
            transcript.matches("Sheet path: ").count(),
            2,
            "the Sheet path label was masked: {transcript}"
        );
        assert!(!transcript.contains("<redacted>Sheet path"), "{transcript}");
        // The terminal output is the render, which never carries the label.
        assert!(!normal.contains(LABEL), "{normal}");
        assert!(!failing.contains(LABEL), "{failing}");
    }

    // ---- Issue #1082 regressions (Gate 1 keeps its refusal reason) ----

    /// Issue #1082: a local pack whose pin disagrees with its bytes is refused
    /// by Gate 1 with the pin-mismatch reason, without adopting the workspace
    /// copy and without quoting the pin value or the pack's bytes.
    #[test]
    fn gate_one_refuses_a_pin_mismatch_with_its_reason() {
        const PIN_CANARY: &str = "PACK_CANARY_1082_pin_7d2";
        const BODY_CANARY: &str = "PACK_CANARY_1082_body_3e9";
        let temp = tempfile::tempdir().unwrap();
        let workspace = temp.path().join("workspace");
        let extension = temp.path().join("extension");
        std::fs::create_dir_all(&workspace).unwrap();
        pack_selection::tests::write_pack(
            &extension,
            "canary-pack",
            "1.0.0",
            &pack_selection::tests::assist("canary-pack", "1.0.0", BODY_CANARY),
        );
        // A same-named workspace candidate that must not be adopted.
        pack_selection::tests::write_pack(
            &workspace.join("packs"),
            "canary-pack",
            "1.0.0",
            &pack_selection::tests::assist("canary-pack", "1.0.0", "workspace-fallback"),
        );
        // Break the higher candidate's pin, carrying a value that must not be
        // transcribed.
        std::fs::write(
            extension.join("canary-pack/1.0.0/pack.sha256"),
            format!("{PIN_CANARY}\n"),
        )
        .unwrap();

        let context =
            pack_selection::ReplPackContext::new(workspace.clone(), Some(extension.clone()));
        let refused = gate_one_pack(
            &context,
            Some(&python_cli_route()),
            Some("canary-pack@1.0.0".to_string()),
        )
        .unwrap_err();

        assert!(
            refused.contains("pin does not match its bytes"),
            "the refusal dropped the reason: {refused}"
        );
        assert!(!refused.contains(PIN_CANARY), "{refused}");
        assert!(!refused.contains(BODY_CANARY), "{refused}");
        assert!(!refused.contains("workspace-fallback"), "{refused}");
        assert!(
            context
                .locator()
                .selectable("python-cli", "create")
                .iter()
                .all(|pack| pack.id != "canary-pack"),
            "the invalid candidate fell back to another pack"
        );
    }

    /// Issue #1082: a pack directory that is a symlink out of its supply root
    /// is refused by Gate 1 with the non-symlink-directory reason, without
    /// quoting the pack's bytes.
    #[cfg(unix)]
    #[test]
    fn gate_one_refuses_a_symlink_out_of_the_root_with_its_reason() {
        const BODY_CANARY: &str = "PACK_CANARY_1082_symlink_c41";
        let temp = tempfile::tempdir().unwrap();
        let workspace = temp.path().join("workspace");
        let extension = temp.path().join("extension");
        let outside = temp.path().join("outside");
        std::fs::create_dir_all(&workspace).unwrap();
        std::fs::create_dir_all(extension.join("canary-pack")).unwrap();
        // A real pack outside the root, reachable only through the symlink.
        pack_selection::tests::write_pack(
            &outside,
            "canary-pack",
            "1.0.0",
            &pack_selection::tests::assist("canary-pack", "1.0.0", BODY_CANARY),
        );
        // Replace the in-root directory with a symlink to the pack outside it.
        std::fs::remove_dir(extension.join("canary-pack")).unwrap();
        std::os::unix::fs::symlink(outside.join("canary-pack"), extension.join("canary-pack"))
            .unwrap();

        let context =
            pack_selection::ReplPackContext::new(workspace.clone(), Some(extension.clone()));
        let refused = gate_one_pack(
            &context,
            Some(&python_cli_route()),
            Some("canary-pack@1.0.0".to_string()),
        )
        .unwrap_err();

        assert!(
            refused.contains("is not a non-symlink directory"),
            "the refusal dropped the reason: {refused}"
        );
        assert!(!refused.contains(BODY_CANARY), "{refused}");
        assert!(
            context
                .locator()
                .selectable("python-cli", "create")
                .iter()
                .all(|pack| pack.id != "canary-pack"),
            "the symlinked candidate fell back to another pack"
        );
    }

    /// The confirmed identity of `pack` for the python-cli filter route, as the
    /// run loop builds it, validated through the same roots the pack was read
    /// from.
    fn selection_identity_for(
        workspace: &std::path::Path,
        extension_root: Option<&std::path::Path>,
        pack: &crate::tui::boundary_shell::confirmation::PackSelection,
    ) -> crate::tui::boundary_shell::confirmation::ConfirmationIdentity {
        let locator = crate::planner::pack::catalog::PackLocator::with_extension_root(
            workspace,
            extension_root.map(std::path::Path::to_path_buf),
        );
        crate::tui::boundary_shell::confirmation::ConfirmationIdentity::new_with_locator(
            "Python CLI filter".to_string(),
            workspace,
            &python_cli_route(),
            crate::tui::boundary_shell::band_catalog::value_for(
                "python-cli",
                IntentId::Create,
                TaskFamilyId::Filter,
            )
            .unwrap(),
            pins(),
            pack.clone(),
            &locator,
        )
        .unwrap()
    }

    /// A terminal presentation for `identity`, using its own card hash.
    fn terminal_for(
        identity: &crate::tui::boundary_shell::confirmation::ConfirmationIdentity,
        full: bool,
    ) -> crate::tui::boundary_shell::acceptance::TerminalPresentation {
        crate::tui::boundary_shell::acceptance::TerminalPresentation::new(
            identity.card_hash().unwrap(),
            "# sheet\n\n- Command succeeded: false\n\n## 5. Stop reason\nfailed".to_string(),
            full,
            (!full).then(|| "failed".to_string()),
        )
        .unwrap()
    }
}
