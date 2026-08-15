use std::any::Any;
use std::io::{self, IsTerminal, Write};
use std::panic::{AssertUnwindSafe, PanicHookInfo};
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

use anyhow::bail;
use serde_json::json;

use crate::config::Config;
use crate::providers::ChatClient;

static PANIC_HOOK_LOCK: Mutex<()> = Mutex::new(());
type PanicHook = Box<dyn Fn(&PanicHookInfo<'_>) + Sync + Send + 'static>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TuiTerminalStatus {
    Completed,
    Partial,
    Failed,
    Aborted,
    Interrupted,
}

impl TuiTerminalStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Partial => "partial",
            Self::Failed => "failed",
            Self::Aborted => "aborted",
            Self::Interrupted => "interrupted",
        }
    }

    fn ok(self) -> bool {
        matches!(self, Self::Completed | Self::Partial)
    }

    fn failure_kind(self) -> &'static str {
        match self {
            Self::Completed | Self::Partial => "",
            Self::Failed => "tui_command_failed",
            Self::Aborted => "tui_command_aborted",
            Self::Interrupted => "tui_command_interrupted",
        }
    }
}

struct TuiCommandCompletionGuard {
    config: Config,
    command: String,
    finalized: bool,
}

impl TuiCommandCompletionGuard {
    fn new(config: &Config, command: &str) -> Self {
        Self {
            config: config.clone(),
            command: command.to_string(),
            finalized: false,
        }
    }

    fn finalize(
        mut self,
        result: &anyhow::Result<String>,
        status: TuiTerminalStatus,
    ) -> crate::eval_events::CompletionProjection {
        crate::bounded_process::reap_registered_server_children_for_workspace(
            self.config.eval_events_path.as_deref(),
            "tui_command_finalize",
            &self.config.workspace_root,
        );
        let completion =
            emit_tui_command_stop_with_status(&self.config, &self.command, result, status);
        self.finalized = true;
        completion
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PanicDiagnostic {
    message: String,
    location: String,
}

struct PanicHookGuard {
    previous: Option<PanicHook>,
    _lock: MutexGuard<'static, ()>,
}

impl PanicHookGuard {
    fn install(diagnostic: Arc<Mutex<Option<PanicDiagnostic>>>) -> Self {
        let lock = PANIC_HOOK_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let mut diagnostic = diagnostic
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            *diagnostic = Some(PanicDiagnostic::from_hook(info));
        }));
        Self {
            previous: Some(previous),
            _lock: lock,
        }
    }
}

impl Drop for PanicHookGuard {
    fn drop(&mut self) {
        let _ = std::panic::take_hook();
        if let Some(previous) = self.previous.take() {
            std::panic::set_hook(previous);
        }
    }
}

impl PanicDiagnostic {
    fn from_hook(info: &PanicHookInfo<'_>) -> Self {
        let location = info
            .location()
            .map(|location| {
                format!(
                    "{}:{}:{}",
                    location.file(),
                    location.line(),
                    location.column()
                )
            })
            .unwrap_or_else(|| "unknown".to_string());
        Self {
            message: panic_payload_message(info.payload()),
            location,
        }
    }

    fn from_payload(payload: &(dyn Any + Send)) -> Self {
        Self {
            message: panic_payload_message(payload),
            location: "unknown".to_string(),
        }
    }

    fn stop_reason(&self) -> String {
        format!("panic caught: {} at {}", self.message, self.location)
    }
}

impl Drop for TuiCommandCompletionGuard {
    fn drop(&mut self) {
        if self.finalized {
            return;
        }
        let result: anyhow::Result<String> =
            Err(anyhow::anyhow!("command aborted before completion"));
        crate::bounded_process::reap_registered_server_children_for_workspace(
            self.config.eval_events_path.as_deref(),
            "tui_command_drop",
            &self.config.workspace_root,
        );
        let _ = emit_tui_command_stop_with_status(
            &self.config,
            &self.command,
            &result,
            TuiTerminalStatus::Aborted,
        );
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedSlash {
    pub command: String,
    pub profile: String,
    pub profile_explicit: bool,
    pub profile_inference: Option<crate::planner::profile::ProfileInference>,
    pub prompt_layout: crate::config::PromptLayout,
    pub prompt_layout_explicit: bool,
    pub style: String,
    pub style_explicit: bool,
    pub goal: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlashCommandKind {
    Help,
    Status,
    Doctor,
    Runs,
    Resume,
    Plan,
    PlanSteps,
    PlanRun,
    RunPlan,
    UltraPlan,
    UltraPlanRun,
    RunUltraPlan,
    SetupInteractionProbe,
    ModelProbe,
    Exit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlashCommandSpec {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub help_usage: &'static str,
    pub description: &'static str,
    pub kind: SlashCommandKind,
}

pub const SLASH_COMMANDS: &[SlashCommandSpec] = &[
    SlashCommandSpec {
        name: "/help",
        aliases: &[],
        help_usage: "/help",
        description: "show this command list",
        kind: SlashCommandKind::Help,
    },
    SlashCommandSpec {
        name: "/status",
        aliases: &[],
        help_usage: "/status",
        description: "show effective configuration and readiness",
        kind: SlashCommandKind::Status,
    },
    SlashCommandSpec {
        name: "/doctor",
        aliases: &[],
        help_usage: "/doctor",
        description: "diagnose configuration, providers, probes, and environment",
        kind: SlashCommandKind::Doctor,
    },
    SlashCommandSpec {
        name: "/runs",
        aliases: &[],
        help_usage: "/runs",
        description: "list recent runs and recovery availability",
        kind: SlashCommandKind::Runs,
    },
    SlashCommandSpec {
        name: "/resume",
        aliases: &[],
        help_usage: "/resume [run-id|yaml-path]",
        description: "resume from a recovery UltraPlan",
        kind: SlashCommandKind::Resume,
    },
    SlashCommandSpec {
        name: "/plan",
        aliases: &[],
        help_usage: "/plan",
        description: "show the active plan and current activity",
        kind: SlashCommandKind::Plan,
    },
    SlashCommandSpec {
        name: "/plan-steps",
        aliases: &[],
        help_usage: "/plan-steps <goal>",
        description: "write a step plan",
        kind: SlashCommandKind::PlanSteps,
    },
    SlashCommandSpec {
        name: "/plan-run",
        aliases: &[],
        help_usage: "/plan-run <goal>",
        description: "generate and run a step plan",
        kind: SlashCommandKind::PlanRun,
    },
    SlashCommandSpec {
        name: "/run-plan",
        aliases: &[],
        help_usage: "/run-plan <path>",
        description: "run an existing step plan",
        kind: SlashCommandKind::RunPlan,
    },
    SlashCommandSpec {
        name: "/ultra-plan",
        aliases: &[],
        help_usage: "/ultra-plan <goal>",
        description: "write an UltraPlan",
        kind: SlashCommandKind::UltraPlan,
    },
    SlashCommandSpec {
        name: "/ultra-plan-run",
        aliases: &[],
        help_usage: "/ultra-plan-run <goal>",
        description: "generate and run an UltraPlan",
        kind: SlashCommandKind::UltraPlanRun,
    },
    SlashCommandSpec {
        name: "/run-ultra-plan",
        aliases: &[],
        help_usage: "/run-ultra-plan <path>",
        description: "run an existing UltraPlan",
        kind: SlashCommandKind::RunUltraPlan,
    },
    SlashCommandSpec {
        name: "/setup-interaction-probe",
        aliases: &[],
        help_usage: "/setup-interaction-probe",
        description: "install the interaction readiness probe",
        kind: SlashCommandKind::SetupInteractionProbe,
    },
    SlashCommandSpec {
        name: "/model-probe",
        aliases: &[],
        help_usage: "/model-probe",
        description: "run the bounded model behavior probe battery",
        kind: SlashCommandKind::ModelProbe,
    },
    SlashCommandSpec {
        name: "/exit",
        aliases: &["/quit"],
        help_usage: "/exit or /quit",
        description: "leave the TUI",
        kind: SlashCommandKind::Exit,
    },
];

pub fn slash_command_spec(name: &str) -> Option<&'static SlashCommandSpec> {
    SLASH_COMMANDS
        .iter()
        .find(|spec| spec.name == name || spec.aliases.contains(&name))
}

pub fn is_exit_command(name: &str) -> bool {
    slash_command_spec(name).is_some_and(|spec| spec.kind == SlashCommandKind::Exit)
}

pub fn parse_slash(line: &str, config: &Config) -> anyhow::Result<ParsedSlash> {
    let args = parse_words(line);
    let Some(command) = args.first() else {
        bail!("empty command");
    };
    let parsed_profile = parse_profile_style_details(&args[1..], config);
    let goal = expand_goal_references(&parsed_profile.rest.join(" "), config)?;
    let mut profile = parsed_profile.profile;
    let mut profile_inference = None;
    if !parsed_profile.profile_explicit
        && let Some(inference) =
            crate::planner::profile::infer_profile(Some(&goal), &config.workspace_root)
    {
        profile = inference.profile.to_string();
        profile_inference = Some(inference);
    }
    Ok(ParsedSlash {
        command: command.clone(),
        profile,
        profile_explicit: parsed_profile.profile_explicit,
        profile_inference,
        prompt_layout: parsed_profile.prompt_layout,
        prompt_layout_explicit: parsed_profile.prompt_layout_explicit,
        style: parsed_profile.style,
        style_explicit: parsed_profile.style_explicit,
        goal,
    })
}

pub fn handle_command(
    line: &str,
    config: &Config,
    planner: &mut dyn ChatClient,
    execution: &mut dyn ChatClient,
    ui: &dyn crate::tui::InteractionUi,
) -> anyhow::Result<String> {
    let words = parse_words(line);
    let Some(command_name) = words.first() else {
        bail!("empty command");
    };
    if !command_name.starts_with('/') {
        return Ok(crate::tui::repl_output::render_plain_text_guidance(line));
    }
    if slash_command_spec(command_name).is_none() {
        let candidates = SLASH_COMMANDS
            .iter()
            .flat_map(|spec| std::iter::once(spec.name).chain(spec.aliases.iter().copied()))
            .collect::<Vec<_>>();
        return Ok(crate::tui::repl_output::render_unknown_command(
            command_name,
            &candidates,
        ));
    }
    let parsed = parse_slash(line, config)?;
    let command =
        slash_command_spec(&parsed.command).expect("command was validated before slash parsing");
    let mut config = config.clone();
    config.profile = parsed.profile.clone();
    config.profile_explicit = parsed.profile_explicit;
    config.profile_inference = parsed.profile_inference;
    config.prompt_layout = parsed.prompt_layout;
    if parsed.prompt_layout_explicit {
        config.field_sources.prompt_layout = "slash".to_string();
    }
    config.style = parsed.style.clone();
    let receipt = crate::tui::command_receipt::render(
        line,
        &parsed,
        config.eval_events_path.as_deref(),
        crossterm::terminal::size()
            .map(|(cols, _)| cols)
            .unwrap_or(80),
    );
    ui.render_command_receipt(&receipt)?;
    match command.kind {
        SlashCommandKind::Help => return Ok(render_help()),
        SlashCommandKind::Status => {
            return Ok(crate::tui::presentation::render_status_card(&config));
        }
        SlashCommandKind::Doctor => {
            return Ok(crate::doctor::diagnose(&config).render_human());
        }
        SlashCommandKind::Plan => return Ok(crate::tui::presentation::render_current_plan()),
        SlashCommandKind::Runs => {
            return Ok(crate::runs::render_runs_table(&config.workspace_root));
        }
        SlashCommandKind::Exit => return Ok(String::new()),
        SlashCommandKind::Resume
        | SlashCommandKind::PlanSteps
        | SlashCommandKind::PlanRun
        | SlashCommandKind::RunPlan
        | SlashCommandKind::UltraPlan
        | SlashCommandKind::UltraPlanRun
        | SlashCommandKind::RunUltraPlan
        | SlashCommandKind::SetupInteractionProbe
        | SlashCommandKind::ModelProbe => {}
    }
    crate::tui::presentation::accept_command(&parsed, &config);
    if let Some(inference) = config.profile_inference {
        crate::eval_events::emit(
            config.eval_events_path.as_deref(),
            json!({
                "event": "profile_inferred",
                "profile": inference.profile,
                "from": inference.source.as_str(),
                "lifecycle_stage": "tui_command",
                "command": &parsed.command,
            }),
        );
    }
    crate::eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "tui_command_start",
            "lifecycle_stage": "tui_command",
            "command": &parsed.command,
            "profile": &config.profile,
            "prompt_layout": config.prompt_layout.as_str(),
            "profile_inferred": config
                .profile_inference
                .map(|inference| inference.profile)
                .unwrap_or(""),
            "profile_inference_source": config
                .profile_inference
                .map(|inference| inference.source.as_str())
                .unwrap_or(""),
            "style": &config.style,
            "goal": crate::eval_events::body_snippet(&parsed.goal),
        }),
    );
    let completion_guard = TuiCommandCompletionGuard::new(&config, &parsed.command);
    let panic_diagnostic = Arc::new(Mutex::new(None));
    let panic_hook_guard = PanicHookGuard::install(Arc::clone(&panic_diagnostic));
    let command_result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        crate::preflight::run_for_slash_command(&config, &parsed.command, &parsed.goal)?;
        match command.kind {
            SlashCommandKind::PlanSteps => {
                let plan =
                    crate::planner::generate_step_plan_with_ui(planner, &parsed.goal, &config, ui)?;
                let path = crate::planner::save_step_plan(&config.workspace_root, &plan)?;
                Ok(path.display().to_string())
            }
            SlashCommandKind::PlanRun => crate::planner::generate_and_run_step_plan_with_ui(
                planner,
                execution,
                &parsed.goal,
                &config,
                ui,
            ),
            SlashCommandKind::RunPlan => crate::planner::run_plan_file_with_ui(
                execution,
                Path::new(&parsed.goal),
                &config,
                ui,
            ),
            SlashCommandKind::UltraPlan => {
                let plan = crate::planner::generate_ultra_plan_with_ui(
                    planner,
                    &parsed.goal,
                    &config,
                    ui,
                )?;
                let path = crate::planner::save_ultra_plan(&config.workspace_root, &plan)?;
                Ok(path.display().to_string())
            }
            SlashCommandKind::UltraPlanRun => crate::planner::generate_and_run_ultra_plan_with_ui(
                planner,
                execution,
                &parsed.goal,
                &config,
                ui,
            ),
            SlashCommandKind::RunUltraPlan => crate::planner::run_ultra_plan_file_with_ui(
                planner,
                execution,
                Path::new(&parsed.goal),
                &config,
                ui,
            ),
            SlashCommandKind::Resume => {
                let resume = crate::runs::prepare_resume(&config.workspace_root, &parsed.goal)?;
                if let Some(error) = resume.workspace_drift_error() {
                    bail!("{error}");
                }
                confirm_resume(&config, &resume)?;
                let progress = crate::tui::presentation::PlanProgress {
                    completed_phases: resume.completed_phase_ids.clone(),
                    current_phase: None,
                    current_step: None,
                };
                crate::tui::presentation::emit_ultra_plan_card(&resume.plan, &progress);
                let plan_card =
                    crate::tui::presentation::render_ultra_plan_card(&resume.plan, &progress);
                crate::runs::emit_resume_start(&config, &resume);
                let output = crate::planner::run_ultra_plan_with_ui(
                    planner,
                    execution,
                    &resume.plan,
                    &config,
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
                let output = crate::model_probe::run_configured(&config, planner, execution)?;
                Ok(output.card)
            }
            SlashCommandKind::Help
            | SlashCommandKind::Status
            | SlashCommandKind::Doctor
            | SlashCommandKind::Runs
            | SlashCommandKind::Plan
            | SlashCommandKind::Exit => unreachable!("handled before command execution"),
        }
    }));
    drop(panic_hook_guard);
    let result = match command_result {
        Ok(result) => result,
        Err(payload) => {
            let diagnostic = panic_diagnostic
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .clone()
                .unwrap_or_else(|| PanicDiagnostic::from_payload(payload.as_ref()));
            emit_panic_caught(&config, &parsed.command, &diagnostic);
            let result = Err(anyhow::anyhow!(diagnostic.stop_reason()));
            let _ = completion_guard.finalize(&result, TuiTerminalStatus::Aborted);
            std::panic::resume_unwind(payload);
        }
    };
    let terminal_status = terminal_status_for_result(&result, ui);
    let stop_reason = stop_reason_for_result(&result, terminal_status);
    let completion = completion_guard.finalize(&result, terminal_status);
    match result {
        Err(err) => {
            let markdown = if terminal_status == TuiTerminalStatus::Interrupted {
                crate::tui::repl_output::render_interrupted(line, &stop_reason, &completion)
            } else {
                crate::eval_events::render_tui_command_failure_block(
                    config.eval_events_path.as_deref(),
                    &err.to_string(),
                    &completion,
                )
            };
            Err(crate::tui::repl_output::RenderedCommandError::new(markdown).into())
        }
        Ok(output) => {
            if let Some(notice) = crate::eval_events::render_tui_command_incomplete_notice(
                config.eval_events_path.as_deref(),
                &completion,
            ) {
                eprintln!("{notice}");
            }
            let output = crate::eval_events::render_tui_completion_output(&output, &completion);
            if crate::tui::terminal_summary::applies_to(&parsed.command) {
                let card = crate::eval_events::render_terminal_summary_card(
                    config.eval_events_path.as_deref(),
                    &stop_reason,
                    &completion,
                );
                Ok(format!("{output}\n\n{card}"))
            } else {
                Ok(output)
            }
        }
    }
}

pub fn render_help() -> String {
    let mut lines = vec!["Commands:".to_string()];
    lines.extend(
        SLASH_COMMANDS
            .iter()
            .map(|spec| format!("{} - {}", spec.help_usage, spec.description)),
    );
    lines.push(
        "Footer: use --footer off to disable the fixed footer; breadcrumbs remain in scrollback."
            .to_string(),
    );
    lines.push(
        "Running input: type and Backspace edit pending input; Enter queues up to 10 lines (4096 bytes each)."
            .to_string(),
    );
    lines.push(
        "Multi-line input: end a line with \\ or leave a double quote open; Enter continues at the ... prompt. Submit with quotes closed and no trailing \\."
            .to_string(),
    );
    lines.push(
        "Interrupt: Ctrl-C always interrupts; Esc clears non-empty pending input, otherwise interrupts; a repeated interrupt force-finalizes."
            .to_string(),
    );
    lines.join("\n")
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

fn emit_panic_caught(config: &Config, command: &str, diagnostic: &PanicDiagnostic) {
    crate::eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "panic_caught",
            "lifecycle_stage": "tui_command",
            "command": command,
            "status": "aborted",
            "failure_kind": "panic",
            "message": crate::eval_events::body_snippet(&diagnostic.message),
            "location": diagnostic.location,
            "panic_message": crate::eval_events::body_snippet(&diagnostic.message),
            "panic_location": diagnostic.location,
        }),
    );
}

fn panic_payload_message(payload: &(dyn Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "non-string panic payload".to_string()
    }
}

#[cfg(test)]
fn emit_tui_command_stop(
    config: &Config,
    command: &str,
    result: &anyhow::Result<String>,
) -> crate::eval_events::CompletionProjection {
    emit_tui_command_stop_with_status(config, command, result, status_without_ui(result))
}

fn emit_tui_command_stop_with_status(
    config: &Config,
    command: &str,
    result: &anyhow::Result<String>,
    terminal_status: TuiTerminalStatus,
) -> crate::eval_events::CompletionProjection {
    let requested_ok = terminal_status.ok();
    let mut completion_snapshot =
        crate::eval_events::latest_completion_snapshot(config.eval_events_path.as_deref());
    crate::completion_metadata::apply_config_completion_metadata(config, &mut completion_snapshot);
    let mut completion = crate::eval_events::project_completion(requested_ok, &completion_snapshot);
    crate::completion_metadata::apply_config_completion_projection(config, &mut completion);
    let terminal_status = effective_terminal_status(terminal_status, &completion);
    let ok = terminal_status.ok();
    let failure_kind = terminal_status.failure_kind();
    let stop_reason = stop_reason_for_result(result, terminal_status);
    let event_projection = event_projection_for_terminal_status(&completion, terminal_status);
    let time_profile =
        crate::time_profile::aggregate_event_path(config.eval_events_path.as_deref());
    let time_profile_event = time_profile.to_event_json();
    crate::eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "tui_command_stop",
            "lifecycle_stage": "tui_command",
            "command": command,
            "ok": ok,
            "status": terminal_status.as_str(),
            "build_commit": crate::build_info::COMMIT,
            "build_dirty": crate::build_info::dirty(),
            "build_timestamp": crate::build_info::TIMESTAMP,
            "failure_kind": failure_kind,
            "stop_reason": stop_reason,
            "resumed_from": crate::runs::latest_resumed_from(config.eval_events_path.as_deref()).unwrap_or_default(),
            "completion_status": &completion.status,
            "task_status": &event_projection.task_status,
            "profile": &completion.profile,
            "effective_profile": &completion.effective_profile,
            "prompt_layout": config.prompt_layout.as_str(),
            "contract_origin": &completion.contract_origin,
            "assurance_level": &completion.assurance_level,
            "assurance_reason": &completion.assurance_reason,
            "profile_inferred": &completion.profile_inferred,
            "profile_inference_source": &completion.profile_inference_source,
            "requested_port": &completion.requested_port,
            "session_status": "repl_ready",
            "repl_status": "ready",
            "command_completion_state": &event_projection.command_completion,
            "runtime_acceptance_status": &completion.runtime_acceptance,
            "final_acceptance_status": &completion.final_acceptance,
            "release_gate_status": &completion.release_gate,
            "completion_contract_verification_enabled": completion.completion_contract_verification_enabled,
            "completion_contract_path_merge_enabled": completion.completion_contract_path_merge_enabled,
            "completion_contract_path": &completion.completion_contract_path,
            "completion_contract_generated": completion.completion_contract_generated,
            "external_contract_checked": completion.external_contract_checked,
            "external_contract_ok": completion.external_contract_ok,
            "release_gate_reasons": &completion.release_gate_reasons,
            "browser_readiness_applicable": completion.browser_readiness_applicable,
            "browser_readiness_execution_status": &completion.browser_readiness_execution_status,
            "browser_readiness_status": &completion.browser_readiness,
            "browser_readiness_evidence_path": &completion.browser_readiness_evidence_path,
            "interaction_evidence_applicable": completion.interaction_evidence_applicable,
            "interaction_evidence_execution_status": &completion.interaction_evidence_execution_status,
            "interaction_evidence_status": &completion.interaction_evidence,
            "interaction_evidence_path": &completion.interaction_evidence_path,
            "state_dimensions_changed": &completion.state_dimensions_changed,
            "action_hooks": &completion.action_hooks,
            "text_entry_target": &completion.text_entry_target,
            "typed_token": &completion.typed_token,
            "token_echoed": &completion.token_echoed,
            "text_input_state_change": &completion.text_input_state_change,
            "release_quality_completion": &completion.release_quality_completion,
            "next_action": &event_projection.next_action,
            "recovery_next_action": &event_projection.next_action,
            "recovery_prompt_path": &completion.recovery_prompt_path,
            "recovery_ultra_plan_path": &completion.recovery_ultra_plan_path,
            "suggested_recovery_command": &completion.suggested_recovery_command,
            "suggested_recovery_yaml_command": &completion.suggested_recovery_yaml_command,
            "planner_verify_normalization_count": completion.planner_verify_normalization_count,
            "planner_retry_count": completion.planner_retry_count,
            "planner_quality_warning_count": completion.planner_quality_warning_count,
            "planner_quality_issue_count": completion.planner_quality_issue_count,
            "planner_repaired": completion.planner_repaired,
            "planner_release_risk": completion.planner_release_risk,
            "time_profile_total_ms": time_profile.total_ms(),
            "time_profile_provider_ms": time_profile.provider.total_ms(),
            "time_profile_provider_prompt_eval_duration": time_profile.provider_durations.prompt_eval_duration,
            "time_profile_provider_eval_duration": time_profile.provider_durations.eval_duration,
            "time_profile_provider_load_duration": time_profile.provider_durations.load_duration,
            "time_profile_provider_total_duration": time_profile.provider_durations.total_duration,
            "time_profile_installs_ms": time_profile.installs_ms,
            "time_profile_builds_ms": time_profile.builds_ms,
            "time_profile_probe_ms": time_profile.probe_ms,
            "time_profile": time_profile_event,
        }),
    );
    if time_profile.total_ms() > 0 {
        crate::eval_events::emit(
            config.eval_events_path.as_deref(),
            json!({
                "event": "time_profile",
                "lifecycle_stage": "tui_command",
                "command": command,
                "profile": time_profile.to_event_json(),
            }),
        );
    }
    crate::eval_events::write_tui_command_completion_summary(
        config.eval_events_path.as_deref(),
        command,
        &stop_reason,
        failure_kind,
        terminal_status.as_str(),
        &completion,
    );
    if !ok {
        crate::eval_events::emit(
            config.eval_events_path.as_deref(),
            json!({
                "event": "loop_stop",
                "lifecycle_stage": "tui_command",
                "reason": failure_kind,
                "primary_reason": stop_reason,
            }),
        );
    }
    if ok { completion } else { event_projection }
}

fn terminal_status_for_result(
    result: &anyhow::Result<String>,
    ui: &dyn crate::tui::InteractionUi,
) -> TuiTerminalStatus {
    match result {
        Ok(_) => TuiTerminalStatus::Completed,
        Err(err) if ui.interrupted() || error_is_interrupted(err) => TuiTerminalStatus::Interrupted,
        Err(_) => TuiTerminalStatus::Failed,
    }
}

#[cfg(test)]
fn status_without_ui(result: &anyhow::Result<String>) -> TuiTerminalStatus {
    match result {
        Ok(_) => TuiTerminalStatus::Completed,
        Err(err) if error_is_interrupted(err) => TuiTerminalStatus::Interrupted,
        Err(_) => TuiTerminalStatus::Failed,
    }
}

fn stop_reason_for_result(
    result: &anyhow::Result<String>,
    terminal_status: TuiTerminalStatus,
) -> String {
    match result {
        Ok(_) => "completed".to_string(),
        Err(err) => {
            let reason = crate::eval_events::render_stop_reason_text(&err.to_string());
            if terminal_status == TuiTerminalStatus::Interrupted && reason.trim().is_empty() {
                "interrupted by user".to_string()
            } else {
                reason
            }
        }
    }
}

fn effective_terminal_status(
    requested: TuiTerminalStatus,
    completion: &crate::eval_events::CompletionProjection,
) -> TuiTerminalStatus {
    if requested == TuiTerminalStatus::Completed
        && completion.release_gate == "partial"
        && completion
            .release_gate_reasons
            .iter()
            .any(|reason| reason.contains("interaction_unverified:probe_unavailable"))
    {
        TuiTerminalStatus::Partial
    } else {
        requested
    }
}

fn error_is_interrupted(err: &anyhow::Error) -> bool {
    err.to_string().contains("interrupted by user")
}

fn event_projection_for_terminal_status(
    completion: &crate::eval_events::CompletionProjection,
    terminal_status: TuiTerminalStatus,
) -> crate::eval_events::CompletionProjection {
    let mut projection = completion.clone();
    if matches!(
        terminal_status,
        TuiTerminalStatus::Completed | TuiTerminalStatus::Partial
    ) {
        return projection;
    }
    projection.status = terminal_status.as_str().to_string();
    projection.command_completion = terminal_status.as_str().to_string();
    projection.task_status = terminal_status.as_str().to_string();
    projection.next_action = match terminal_status {
        TuiTerminalStatus::Interrupted => "resume_or_rerun_command".to_string(),
        TuiTerminalStatus::Aborted => "inspect_summary_and_resume_or_rerun".to_string(),
        TuiTerminalStatus::Failed => "fix_command_failure".to_string(),
        TuiTerminalStatus::Completed | TuiTerminalStatus::Partial => projection.next_action,
    };
    projection
}

pub fn parse_profile_style(args: &[String], config: &Config) -> (String, String, Vec<String>) {
    let parsed = parse_profile_style_details(args, config);
    (parsed.profile, parsed.style, parsed.rest)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedProfileStyle {
    profile: String,
    profile_explicit: bool,
    prompt_layout: crate::config::PromptLayout,
    prompt_layout_explicit: bool,
    style: String,
    style_explicit: bool,
    rest: Vec<String>,
}

fn parse_profile_style_details(args: &[String], config: &Config) -> ParsedProfileStyle {
    let mut profile = config.profile.clone();
    let mut profile_explicit = config.profile_explicit;
    let mut prompt_layout = config.prompt_layout;
    let mut prompt_layout_explicit = false;
    let mut style = config.style.clone();
    let mut style_explicit = false;
    let mut rest = Vec::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--profile" if i + 1 < args.len() => {
                profile = args[i + 1].clone();
                profile_explicit = true;
                i += 2;
            }
            "--style" if i + 1 < args.len() => {
                style = args[i + 1].clone();
                style_explicit = true;
                i += 2;
            }
            "--prompt-layout" if i + 1 < args.len() => {
                prompt_layout = match args[i + 1].as_str() {
                    "stable" => crate::config::PromptLayout::Stable,
                    "legacy" => crate::config::PromptLayout::Legacy,
                    _ => prompt_layout,
                };
                prompt_layout_explicit = true;
                i += 2;
            }
            _ => {
                rest.push(args[i].clone());
                i += 1;
            }
        }
    }
    ParsedProfileStyle {
        profile,
        profile_explicit,
        prompt_layout,
        prompt_layout_explicit,
        style,
        style_explicit,
        rest,
    }
}

pub fn expand_goal_references(goal: &str, config: &Config) -> anyhow::Result<String> {
    let mut out = goal.to_string();
    while let Some(start) = out.find("$(cat ") {
        let Some(end_rel) = out[start..].find(')') else {
            break;
        };
        let end = start + end_rel;
        let raw = out[start + "$(cat ".len()..end].trim();
        let path = crate::tools::path_guard::resolve_existing(&config.workspace_root, raw)?;
        let content = std::fs::read_to_string(path)?;
        out.replace_range(start..=end, &content);
    }
    Ok(out)
}

pub fn parse_words(input: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    for ch in input.chars() {
        match ch {
            '"' => quoted = !quoted,
            ' ' | '\t' if !quoted => {
                if !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::{AssistantReply, ChatClient};
    use crate::state::ConversationMessage;
    use crate::tools::registry::ToolSpec;

    #[derive(Clone)]
    struct DummyClient;

    impl ChatClient for DummyClient {
        fn label(&self) -> &str {
            "dummy"
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
            Ok(AssistantReply::text("unused"))
        }
    }

    fn config() -> Config {
        Config {
            workspace_root: std::path::PathBuf::from("."),
            state_dir: std::path::PathBuf::from("state"),
            eval_events_path: None,
            completion_contract_path: None,
            yes: true,
            offline: false,
            context_budget: 1000,
            model: "m".to_string(),
            provider: crate::config::Provider::Ollama,
            tool_protocol: None,
            openai_api: crate::config::OpenAiApi::ChatCompletions,
            prompt_layout: crate::config::PromptLayout::Stable,
            plan_preset: crate::config::PlanPreset::None,
            intent_override: None,
            planner_model: "m".to_string(),
            planner_provider: crate::config::Provider::Ollama,
            ollama_host: "http://localhost:11434".to_string(),
            lm_studio_host: "http://localhost:1234".to_string(),
            num_predict: 100,
            max_iterations: 4,
            chat_timeout_secs: 1,
            chat_timeout_source: "override:test".to_string(),
            field_sources: crate::config::ConfigFieldSources::default(),
            chat_retries: 1,
            stream: false,
            resume: None,
            fresh_session: false,
            no_footer: false,
            narration: crate::config::NarrationMode::Normal,
            profile: "generic".to_string(),
            profile_explicit: false,
            profile_inference: None,
            style: "default".to_string(),
            action: crate::config::Action::Repl,
        }
    }

    #[test]
    fn slash_command_parse_quoted_prompt() {
        let words = parse_words(r#"/ultra-plan-run --profile nextjs "3011 port app""#);
        assert_eq!(words[0], "/ultra-plan-run");
        assert_eq!(words[2], "nextjs");
        assert_eq!(words[3], "3011 port app");
    }

    #[test]
    fn slash_command_parse_profile_style_and_goal() {
        let words = parse_words(r#"/plan-run --profile nextjs --style compact "3011 port app""#);
        let (profile, style, rest) = parse_profile_style(&words[1..], &config());
        assert_eq!(profile, "nextjs");
        assert_eq!(style, "compact");
        assert_eq!(rest, vec!["3011 port app"]);
    }

    #[test]
    fn parse_slash_uses_config_defaults() {
        let parsed = parse_slash("/plan-run goal", &config()).unwrap();
        assert_eq!(parsed.profile, "generic");
        assert_eq!(parsed.prompt_layout, crate::config::PromptLayout::Stable);
        assert_eq!(parsed.style, "default");
        assert_eq!(parsed.goal, "goal");
    }

    #[test]
    fn parse_slash_accepts_prompt_layout_override() {
        let parsed = parse_slash("/ultra-plan-run --prompt-layout legacy goal", &config()).unwrap();
        assert_eq!(parsed.prompt_layout, crate::config::PromptLayout::Legacy);
        assert_eq!(parsed.goal, "goal");
    }

    #[test]
    fn parse_slash_infers_profile_from_goal_when_unset() {
        let parsed = parse_slash("/ultra-plan-run Web アプリを作って", &config()).unwrap();
        assert_eq!(parsed.profile, "nextjs");
        assert_eq!(
            parsed
                .profile_inference
                .expect("profile inference")
                .source
                .as_str(),
            "goal"
        );
    }

    #[test]
    fn parse_slash_explicit_generic_wins_over_inference() {
        let parsed = parse_slash(
            "/ultra-plan-run --profile generic Web アプリを作って",
            &config(),
        )
        .unwrap();
        assert_eq!(parsed.profile, "generic");
        assert!(parsed.profile_explicit);
        assert!(parsed.profile_inference.is_none());
    }

    #[test]
    fn help_lists_discovery_commands_and_interrupt_semantics() {
        let help = render_help();
        for expected in [
            "/ultra-plan-run <goal> - generate and run an UltraPlan",
            "/plan-run <goal> - generate and run a step plan",
            "/plan - show the active plan and current activity",
            "/runs - list recent runs and recovery availability",
            "/resume [run-id|yaml-path] - resume from a recovery UltraPlan",
            "/status - show effective configuration and readiness",
            "/doctor - diagnose configuration, providers, probes, and environment",
            "/setup-interaction-probe - install the interaction readiness probe",
            "/exit or /quit - leave the TUI",
            "Footer: use --footer off to disable the fixed footer; breadcrumbs remain in scrollback.",
            "Running input: type and Backspace edit pending input; Enter queues up to 10 lines (4096 bytes each).",
            "Multi-line input: end a line with \\ or leave a double quote open; Enter continues at the ... prompt. Submit with quotes closed and no trailing \\.",
            "Interrupt: Ctrl-C always interrupts; Esc clears non-empty pending input, otherwise interrupts; a repeated interrupt force-finalizes.",
        ] {
            assert!(help.contains(expected), "{help}");
        }
    }

    #[test]
    fn slash_registry_is_the_help_and_alias_source() {
        assert_eq!(SLASH_COMMANDS.len(), 15);
        let help = render_help();
        for command in SLASH_COMMANDS {
            assert!(
                help.contains(&format!("{} - {}", command.help_usage, command.description)),
                "{help}"
            );
            assert_eq!(slash_command_spec(command.name), Some(command));
        }
        assert_eq!(
            slash_command_spec("/quit").map(|command| command.name),
            Some("/exit")
        );
    }

    #[test]
    fn doctor_slash_command_renders_diagnostics_without_provider_calls() {
        let dir = tempfile::tempdir().unwrap();
        let state = dir.path().join("state");
        std::fs::create_dir_all(&state).unwrap();
        let mut config = config();
        config.workspace_root = dir.path().to_path_buf();
        config.state_dir = state;
        config.provider = crate::config::Provider::Openai;
        config.planner_provider = crate::config::Provider::Openai;
        let mut planner = DummyClient;
        let mut execution = DummyClient;

        let output = handle_command(
            "/doctor",
            &config,
            &mut planner,
            &mut execution,
            &crate::tui::NOOP_UI,
        )
        .unwrap();

        assert!(output.starts_with("CommandAgent doctor:"), "{output}");
        assert!(output.contains("OpenAI key"), "{output}");
        assert!(output.contains("Playwright probe"), "{output}");
    }

    #[test]
    fn unknown_slash_command_suggests_help() {
        let mut planner = DummyClient;
        let mut execution = DummyClient;
        let output = handle_command(
            "/hepl",
            &config(),
            &mut planner,
            &mut execution,
            &crate::tui::NOOP_UI,
        )
        .unwrap();

        assert_eq!(output.lines().count(), 2, "{output}");
        assert!(output.contains("Unknown command: /hepl"), "{output}");
        assert!(output.contains("Did you mean /help?"), "{output}");
    }

    #[test]
    fn tui_command_stop_marks_generic_completion_reduced_assurance() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("workspace");
        let events = workspace.join(".anvil/runs/test/events.jsonl");
        std::fs::create_dir_all(events.parent().unwrap()).unwrap();
        let mut cfg = config();
        cfg.workspace_root = workspace;
        cfg.eval_events_path = Some(events.clone());

        let result: anyhow::Result<String> = Ok("done".to_string());
        let projection = emit_tui_command_stop(&cfg, "/ultra-plan-run", &result);

        assert_eq!(projection.assurance_level, "reduced");
        assert_eq!(projection.task_status, "completed (reduced assurance)");
        let event_text = std::fs::read_to_string(&events).unwrap();
        assert!(event_text.contains(r#""assurance_level":"reduced""#));
        assert!(event_text.contains(r#""task_status":"completed (reduced assurance)""#));
        let summary = std::fs::read_to_string(events.parent().unwrap().join("summary.md")).unwrap();
        assert!(summary.contains(
            "Assurance: reduced (generic profile — no capability contract, no behavioral verification)"
        ));
    }

    #[test]
    fn tui_command_stop_preserves_generic_static_assurance() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("workspace");
        let events = workspace.join(".anvil/runs/test/events.jsonl");
        std::fs::create_dir_all(events.parent().unwrap()).unwrap();
        let mut cfg = config();
        cfg.workspace_root = workspace;
        cfg.eval_events_path = Some(events.clone());
        crate::eval_events::emit(
            cfg.eval_events_path.as_deref(),
            serde_json::json!({
                "event": "ultra_final_acceptance",
                "runtime_acceptance_status": "pass",
                "final_acceptance_status": "full_success",
                "release_gate_status": "pass",
                "assurance_level": "static",
                "assurance_reason": crate::eval_events::GENERIC_STATIC_ASSURANCE_REASON,
            }),
        );

        let result: anyhow::Result<String> = Ok("done".to_string());
        let projection = emit_tui_command_stop(&cfg, "/ultra-plan-run", &result);

        assert_eq!(projection.assurance_level, "static");
        assert_eq!(projection.task_status, "completed (static assurance)");
        let event_text = std::fs::read_to_string(&events).unwrap();
        assert!(event_text.contains(r#""assurance_level":"static""#));
        assert!(event_text.contains(r#""task_status":"completed (static assurance)""#));
        let summary = std::fs::read_to_string(events.parent().unwrap().join("summary.md")).unwrap();
        assert!(summary.contains(
            "Assurance: static (generic profile — minimal interactive contract verified statically; no behavioral verification)"
        ));
    }

    #[test]
    fn tui_command_stop_preserves_known_profile_from_early_death_events() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("workspace");
        let events = workspace.join(".anvil/runs/test/events.jsonl");
        std::fs::create_dir_all(events.parent().unwrap()).unwrap();
        let mut cfg = config();
        cfg.workspace_root = workspace;
        cfg.eval_events_path = Some(events.clone());
        crate::eval_events::emit(
            cfg.eval_events_path.as_deref(),
            serde_json::json!({
                "event": "tui_command_start",
                "command": "/ultra-plan-run",
                "profile": "nextjs",
            }),
        );
        crate::eval_events::emit(
            cfg.eval_events_path.as_deref(),
            serde_json::json!({
                "event": "ultra_context_initialized",
                "profile": "nextjs",
                "requested_port": "3011 (goal)",
            }),
        );

        let result: anyhow::Result<String> =
            Err(anyhow::anyhow!("invalid StepPlan after corrective retries"));
        let projection = emit_tui_command_stop(&cfg, "/ultra-plan-run", &result);

        assert_eq!(projection.effective_profile, "nextjs");
        assert_eq!(projection.assurance_level, "partial");
        assert_eq!(projection.assurance_reason, "acceptance_not_full_success");
        let event_text = std::fs::read_to_string(&events).unwrap();
        let tui_stop = event_text
            .lines()
            .rfind(|line| line.contains(r#""event":"tui_command_stop""#))
            .unwrap();
        assert!(
            tui_stop.contains(r#""effective_profile":"nextjs""#),
            "{tui_stop}"
        );
        assert!(
            tui_stop.contains(r#""assurance_level":"partial""#),
            "{tui_stop}"
        );
        assert!(
            tui_stop.contains(r#""assurance_reason":"acceptance_not_full_success""#),
            "{tui_stop}"
        );
        assert!(!tui_stop.contains("generic_profile_reduced_assurance"));
        let summary = std::fs::read_to_string(events.parent().unwrap().join("summary.md")).unwrap();
        assert!(summary.contains("Effective profile: nextjs"), "{summary}");
        assert!(!summary.contains("Assurance: reduced"), "{summary}");
    }

    #[test]
    fn data_early_failure_projection_matches_investigation_run_table() {
        let cases = [
            (
                "run1-script-not-generated",
                false,
                "failed",
                "data_profile_script_not_generated",
            ),
            (
                "run2-generated-and-run-outside-contract-probe",
                true,
                "static",
                "data_profile_probe_not_run",
            ),
            (
                "run3-generated-before-confinement-rejection",
                true,
                "static",
                "data_profile_probe_not_run",
            ),
            (
                "run4-generated-before-authority-denial",
                true,
                "static",
                "data_profile_probe_not_run",
            ),
        ];

        for (case, pipeline_generated, expected_level, expected_reason) in cases {
            let dir = tempfile::tempdir().unwrap();
            let workspace = dir.path().join("workspace");
            let events = workspace.join(".anvil/runs/test/events.jsonl");
            std::fs::create_dir_all(events.parent().unwrap()).unwrap();
            if pipeline_generated {
                std::fs::create_dir_all(workspace.join("pipeline")).unwrap();
                std::fs::write(workspace.join("pipeline/main.py"), "# generated\n").unwrap();
            }
            let mut cfg = config();
            cfg.workspace_root = workspace;
            cfg.eval_events_path = Some(events);
            cfg.profile = "data".to_string();
            cfg.profile_explicit = true;

            let result: anyhow::Result<String> = Err(anyhow::anyhow!("phase failed"));
            let projection = emit_tui_command_stop(&cfg, "/ultra-plan-run", &result);

            assert_eq!(projection.effective_profile, "data", "{case}");
            assert_eq!(projection.assurance_level, expected_level, "{case}");
            assert_eq!(projection.assurance_reason, expected_reason, "{case}");
        }
    }

    #[test]
    fn tui_command_stop_uses_phase_scoped_recovery_paths() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("workspace");
        let events = workspace.join(".anvil/runs/test/events.jsonl");
        let prompt_path = workspace.join(".anvil/repairs/repair-phase.md");
        let yaml_path = workspace.join(".anvil/plans/recovery-phase.yaml");
        std::fs::create_dir_all(events.parent().unwrap()).unwrap();
        let mut cfg = config();
        cfg.workspace_root = workspace.clone();
        cfg.eval_events_path = Some(events.clone());
        crate::eval_events::emit(
            cfg.eval_events_path.as_deref(),
            serde_json::json!({
                "event": "recovery_prompt_saved",
                "recovery_prompt_path": prompt_path.display().to_string(),
                "recovery_ultra_plan_path": yaml_path.display().to_string(),
                "suggested_recovery_command": format!("/ultra-plan-run --profile nextjs \"$(cat {})\"", prompt_path.display()),
                "suggested_recovery_yaml_command": format!("/run-ultra-plan {}", yaml_path.display()),
            }),
        );

        let result: anyhow::Result<String> = Err(anyhow::anyhow!("phase invariant failed"));
        let projection = emit_tui_command_stop(&cfg, "/ultra-plan-run", &result);

        assert_eq!(
            projection.recovery_prompt_path,
            ".anvil/repairs/repair-phase.md"
        );
        assert_eq!(
            projection.recovery_ultra_plan_path,
            ".anvil/plans/recovery-phase.yaml"
        );
        let event_text = std::fs::read_to_string(&events).unwrap();
        let tui_stop = event_text
            .lines()
            .rfind(|line| line.contains(r#""event":"tui_command_stop""#))
            .unwrap();
        assert!(
            tui_stop.contains(r#""recovery_prompt_path":".anvil/repairs/repair-phase.md""#),
            "{tui_stop}"
        );
        assert!(
            tui_stop.contains(r#""recovery_ultra_plan_path":".anvil/plans/recovery-phase.yaml""#),
            "{tui_stop}"
        );
        assert!(
            tui_stop.contains(&format!(
                r#""build_commit":"{}""#,
                crate::build_info::COMMIT
            )),
            "{tui_stop}"
        );
        assert!(tui_stop.contains(r#""suggested_recovery_command":"/ultra-plan-run"#));
        assert!(tui_stop.contains(r#""suggested_recovery_yaml_command":"/run-ultra-plan"#));
    }

    #[test]
    fn tui_command_stop_reports_interaction_unverified_as_partial() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("workspace");
        let events = workspace.join(".anvil/runs/test/events.jsonl");
        std::fs::create_dir_all(events.parent().unwrap()).unwrap();
        let mut cfg = config();
        cfg.workspace_root = workspace;
        cfg.eval_events_path = Some(events.clone());
        crate::eval_events::emit(
            cfg.eval_events_path.as_deref(),
            serde_json::json!({
                "event": "ultra_final_acceptance",
                "profile": "nextjs",
                "effective_profile": "nextjs",
                "contract_origin": "promoted_union",
                "runtime_acceptance_status": "pass",
                "final_acceptance_status": "partial",
                "release_gate_status": "partial",
                "release_gate_reasons": [
                    "interaction_unverified:probe_unavailable",
                    crate::minimal_loop::interaction_probe::INTERACTION_PROBE_SETUP_REMEDIATION
                ],
                "browser_readiness_applicable": true,
                "browser_readiness_execution_status": "performed",
                "browser_readiness_status": "passed",
                "browser_readiness_evidence_path": "browser-readiness.json",
                "interaction_evidence_applicable": true,
                "interaction_evidence_execution_status": "unavailable",
                "interaction_evidence_status": "unavailable:playwright_not_installed",
                "interaction_evidence_path": "",
                "completion_contract_verification_enabled": true,
                "completion_contract_path_merge_enabled": true,
                "external_contract_checked": true,
                "external_contract_ok": true,
            }),
        );

        let result: anyhow::Result<String> = Ok("ultra-plan-run complete".to_string());
        let projection = emit_tui_command_stop(&cfg, "/ultra-plan-run", &result);

        assert_eq!(projection.release_gate, "partial");
        assert_eq!(projection.task_status, "partial (interaction unverified)");
        assert_eq!(
            projection.next_action,
            "run_setup_interaction_probe_to_enable_interaction_release_checks"
        );
        let event_text = std::fs::read_to_string(&events).unwrap();
        let tui_stop = event_text
            .lines()
            .rfind(|line| line.contains(r#""event":"tui_command_stop""#))
            .unwrap();
        assert!(tui_stop.contains(r#""status":"partial""#), "{tui_stop}");
        assert!(
            tui_stop.contains(r#""task_status":"partial (interaction unverified)""#),
            "{tui_stop}"
        );
        assert!(
            tui_stop.contains(r#""effective_profile":"nextjs""#),
            "{tui_stop}"
        );
        assert!(
            tui_stop.contains(r#""contract_origin":"promoted_union""#),
            "{tui_stop}"
        );
        assert!(
            tui_stop.contains(r#""browser_readiness_applicable":true"#),
            "{tui_stop}"
        );
        assert!(
            tui_stop.contains(r#""browser_readiness_execution_status":"performed""#),
            "{tui_stop}"
        );
        assert!(
            tui_stop.contains(r#""interaction_evidence_applicable":true"#),
            "{tui_stop}"
        );
        assert!(
            tui_stop.contains(r#""interaction_evidence_execution_status":"unavailable""#),
            "{tui_stop}"
        );
        let summary = std::fs::read_to_string(events.with_file_name("summary.md")).unwrap();
        assert!(summary.contains("Status: partial"), "{summary}");
        assert!(
            summary.contains("interaction_unverified:probe_unavailable"),
            "{summary}"
        );
        assert!(
            summary.contains(
                crate::minimal_loop::interaction_probe::INTERACTION_PROBE_SETUP_REMEDIATION
            ),
            "{summary}"
        );
    }

    #[test]
    fn parses_user_ultra_plan_run_nextjs_command() {
        let parsed = parse_slash(
            "/ultra-plan-run --profile nextjs あなたが考える最高に面白くかっこいいスペースインベーダーゲームを3011ポートで起動可能なnext.jsアプリとして開発してください。",
            &config(),
        )
        .unwrap();
        assert_eq!(parsed.command, "/ultra-plan-run");
        assert_eq!(parsed.profile, "nextjs");
        assert_eq!(
            parsed.goal,
            "あなたが考える最高に面白くかっこいいスペースインベーダーゲームを3011ポートで起動可能なnext.jsアプリとして開発してください。"
        );
    }
}
