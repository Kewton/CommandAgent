use crate::agent::events::{
    ArtifactScope, ArtifactStatus, GuardFeedbackKind, PlanKind, RuntimeEvent, RuntimeObserver,
};
use crate::config::{Config, Provider};
use crate::providers::ToolCallMode;
use crate::providers::planner::ProviderTargets;
use crate::tui::banner::{StartupBanner, decide_banner_style, render_startup_banner};
use crate::tui::env;
use crate::tui::markdown::MarkdownRenderer;
use crate::tui::progress::{paint, sanitize_for_progress, tool_color, truncate_chars};
use crate::tui::spinner::{WaitSpinner, WaitSpinnerConfig};
use std::io::{self, Write};
use std::path::Path;

#[derive(Debug, Clone, Copy)]
pub struct TerminalUiConfig {
    pub progress_enabled: bool,
    pub spinner_enabled: bool,
    pub banner_enabled: bool,
    pub color_enabled: bool,
    pub markdown_enabled: bool,
    pub utf8: bool,
}

impl TerminalUiConfig {
    pub fn from_env() -> Self {
        let env = env::detect();
        Self {
            progress_enabled: env.progress_enabled,
            spinner_enabled: env.spinner_enabled,
            banner_enabled: env.banner_enabled,
            color_enabled: env.stderr_color_enabled,
            markdown_enabled: env.markdown_enabled,
            utf8: env.utf8_locale,
        }
    }
}

pub struct TerminalUi<W: Write> {
    writer: W,
    config: TerminalUiConfig,
    wait_spinner: WaitSpinner,
    current_phase: Option<String>,
    current_step: Option<String>,
    last_tool: Option<String>,
}

impl TerminalUi<io::Stderr> {
    pub fn stderr_from_env() -> Self {
        let config = TerminalUiConfig::from_env();
        Self {
            writer: io::stderr(),
            config,
            wait_spinner: WaitSpinner::new(WaitSpinnerConfig {
                enabled: config.spinner_enabled,
                color_enabled: config.color_enabled,
                utf8: config.utf8,
            }),
            current_phase: None,
            current_step: None,
            last_tool: None,
        }
    }
}

impl<W: Write> TerminalUi<W> {
    pub fn new(writer: W, config: TerminalUiConfig) -> Self {
        Self {
            writer,
            config,
            wait_spinner: WaitSpinner::disabled(),
            current_phase: None,
            current_step: None,
            last_tool: None,
        }
    }

    pub fn disabled(writer: W) -> Self {
        Self::new(
            writer,
            TerminalUiConfig {
                progress_enabled: false,
                spinner_enabled: false,
                banner_enabled: false,
                color_enabled: false,
                markdown_enabled: false,
                utf8: false,
            },
        )
    }

    pub fn into_inner(self) -> W {
        self.writer
    }

    pub fn render_startup_context(
        &mut self,
        version: &str,
        config: &Config,
        targets: &ProviderTargets,
    ) -> io::Result<()> {
        if !self.config.progress_enabled {
            return Ok(());
        }
        let cwd = config
            .cwd
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_else(|| config.cwd.to_str().unwrap_or("."));
        let executor = model_label(targets.executor.provider, targets.executor.model.as_deref());
        let planner = model_label(targets.planner.provider, targets.planner.model.as_deref());
        let mut flags = Vec::new();
        if config.yes {
            flags.push("yes");
        }
        if config.offline {
            flags.push("offline");
        }
        let flags = flags.as_slice();
        let style = decide_banner_style(
            self.config.progress_enabled,
            self.config.banner_enabled,
            self.config.color_enabled,
        );
        let banner = render_startup_banner(&StartupBanner {
            version,
            cwd,
            executor: &executor,
            planner: &planner,
            flags,
            style,
        });
        self.write_raw(&banner)
    }

    fn write_line(&mut self, line: &str) -> io::Result<()> {
        if self.config.progress_enabled {
            self.wait_spinner.stop();
            writeln!(self.writer, "{line}")?;
            self.writer.flush()?;
        }
        Ok(())
    }

    fn write_raw(&mut self, text: &str) -> io::Result<()> {
        if self.config.progress_enabled {
            self.wait_spinner.stop();
            write!(self.writer, "{text}")?;
            self.writer.flush()?;
        }
        Ok(())
    }

    fn start_wait(&mut self, label: impl Into<String>) {
        if self.config.progress_enabled && self.config.spinner_enabled {
            self.wait_spinner.start(label);
        }
    }
}

impl<W: Write> RuntimeObserver for TerminalUi<W> {
    fn on_event(&mut self, event: RuntimeEvent) {
        let _ = self.render_event(event);
    }
}

impl<W: Write> TerminalUi<W> {
    fn render_event(&mut self, event: RuntimeEvent) -> io::Result<()> {
        if !self.config.progress_enabled {
            return Ok(());
        }

        match event {
            RuntimeEvent::PlanGenerationStarted {
                kind,
                goal,
                profile,
            } => {
                let kind_label = plan_kind_label(kind);
                let profile = sanitize_for_progress(&profile);
                let goal = truncate_chars(&sanitize_for_progress(&goal), 72);
                self.write_line(&format!(
                    "{kind_label}: generating profile={profile} goal={goal}"
                ))?;
                self.start_wait(format!("{kind_label} generating profile={profile}"));
                Ok(())
            }
            RuntimeEvent::PlanGenerationFinished { kind, item_count } => self.write_line(&format!(
                "{}: generated {} item{}",
                plan_kind_label(kind),
                item_count,
                if item_count == 1 { "" } else { "s" }
            )),
            RuntimeEvent::PlanSaved {
                kind,
                path,
                item_ids,
            } => {
                self.write_line(&format!(
                    "saved {}: {}",
                    plan_kind_label(kind),
                    sanitize_for_progress(&path)
                ))?;
                if !item_ids.is_empty() {
                    self.write_line(&format!("plan preview: {}", preview_ids(&item_ids)))?;
                }
                Ok(())
            }
            RuntimeEvent::UltraPhaseStarted {
                index,
                total,
                phase_id,
            } => {
                self.current_phase = Some(format!("ultra phase {index}/{total} {phase_id}"));
                self.write_line(&format!(
                    "phase {index}/{total} {}: running",
                    sanitize_for_progress(&phase_id)
                ))
            }
            RuntimeEvent::UltraPhaseFinished {
                index,
                total,
                phase_id,
            } => self.write_line(&format!(
                "phase {index}/{total} {}: ok",
                sanitize_for_progress(&phase_id)
            )),
            RuntimeEvent::UltraPhaseFailed {
                index,
                total,
                phase_id,
                error,
            } => self.write_line(&format!(
                "failed at: ultra phase {index}/{total} {} ({})",
                sanitize_for_progress(&phase_id),
                sanitize_for_progress(&error)
            )),
            RuntimeEvent::ProfileVerificationFailed { profile, failures } => {
                self.write_line(&format!(
                    "profile verification {}: failed",
                    sanitize_for_progress(&profile)
                ))?;
                for failure in failures.iter().take(6) {
                    self.write_line(&format!(
                        "profile failure: {}",
                        sanitize_for_progress(failure)
                    ))?;
                }
                if failures.len() > 6 {
                    self.write_line(&format!("profile failure: ... +{}", failures.len() - 6))?;
                }
                Ok(())
            }
            RuntimeEvent::StepStarted {
                index,
                total,
                step_id,
            } => {
                self.current_step = Some(format!("step {step_id}"));
                self.write_line(&format!(
                    "step {index}/{total} {}: running",
                    sanitize_for_progress(&step_id)
                ))
            }
            RuntimeEvent::StepFinished {
                index,
                total,
                step_id,
            } => self.write_line(&format!(
                "step {index}/{total} {}: ok",
                sanitize_for_progress(&step_id)
            )),
            RuntimeEvent::StepFailed {
                index,
                total,
                step_id,
                error,
                missing_expected_paths,
            } => {
                self.write_line(&format!(
                    "failed at: {} > step {index}/{total} {}",
                    self.current_phase.as_deref().unwrap_or("plan"),
                    sanitize_for_progress(&step_id)
                ))?;
                if !missing_expected_paths.is_empty() {
                    self.write_line(&format!(
                        "expected paths: {}",
                        missing_expected_paths
                            .iter()
                            .map(|path| sanitize_for_progress(path))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ))?;
                }
                if let Some(last_tool) = &self.last_tool {
                    self.write_line(&format!("last tool: {last_tool}"))?;
                }
                self.write_line(&format!("reason: {}", sanitize_for_progress(&error)))
            }
            RuntimeEvent::VerifierStarted { step_id, command } => {
                let step_id = sanitize_for_progress(&step_id);
                let command = truncate_chars(&sanitize_for_progress(&command), 72);
                self.write_line(&format!("verify {step_id}: {command}"))?;
                self.start_wait(format!("verify {step_id}: {command}"));
                Ok(())
            }
            RuntimeEvent::VerifierFinished {
                step_id,
                command,
                ok,
                failure_count,
            } => self.write_line(&format!(
                "verify {}: {} {}",
                sanitize_for_progress(&step_id),
                if ok { "ok" } else { "failed" },
                if ok {
                    String::new()
                } else {
                    format!(
                        "{} failure(s): {}",
                        failure_count,
                        truncate_chars(&sanitize_for_progress(&command), 60)
                    )
                }
            )),
            RuntimeEvent::DependencySetupStarted { step_id, command } => {
                let step_id = sanitize_for_progress(&step_id);
                let command = truncate_chars(&sanitize_for_progress(&command), 72);
                self.write_line(&format!("dependency setup {step_id}: {command}"))?;
                self.start_wait(format!("dependency setup {step_id}: {command}"));
                Ok(())
            }
            RuntimeEvent::DependencySetupFinished {
                step_id,
                command,
                ok,
                elapsed_ms,
                status,
            } => self.write_line(&format!(
                "dependency setup {}: {} in {}ms ({}) {}",
                sanitize_for_progress(&step_id),
                if ok { "ok" } else { "failed" },
                elapsed_ms,
                sanitize_for_progress(&status),
                truncate_chars(&sanitize_for_progress(&command), 60)
            )),
            RuntimeEvent::RepairAttemptStarted {
                step_id,
                attempt,
                max_attempts,
                missing_expected_paths,
            } => {
                let step_id = sanitize_for_progress(&step_id);
                self.write_line(&format!(
                    "repair {} attempt {attempt}/{max_attempts}",
                    step_id
                ))?;
                if !missing_expected_paths.is_empty() {
                    self.write_line(&format!(
                        "missing step expected paths: {}",
                        missing_expected_paths
                            .iter()
                            .map(|path| sanitize_for_progress(path))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ))?;
                }
                self.start_wait(format!("repair {step_id} attempt {attempt}/{max_attempts}"));
                Ok(())
            }
            RuntimeEvent::RepairExhausted {
                step_id,
                repair_path,
                suggested_command,
                missing_expected_paths,
            } => {
                self.write_line(&format!(
                    "repair {}: exhausted",
                    sanitize_for_progress(&step_id)
                ))?;
                if !missing_expected_paths.is_empty() {
                    self.write_line(&format!(
                        "missing step expected paths: {}",
                        missing_expected_paths
                            .iter()
                            .map(|path| sanitize_for_progress(path))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ))?;
                }
                self.write_line(&format!("repair: {}", sanitize_for_progress(&repair_path)))?;
                self.write_line(
                    "repair note: suggested command starts a standalone repair plan; original ultra plan remains incomplete until explicitly resumed or replanned",
                )?;
                self.write_line("next command:")?;
                self.write_line(&suggested_command)
            }
            RuntimeEvent::ModelRequestStarted {
                iteration,
                model,
                tool_call_mode,
            } => {
                let model = sanitize_for_progress(&model);
                let mode = tool_mode_label(tool_call_mode);
                self.write_line(&format!("model iter {iteration}: {model} ({mode})"))?;
                self.start_wait(format!("model iter {iteration}: {model} ({mode})"));
                Ok(())
            }
            RuntimeEvent::ModelResponseReceived {
                iteration,
                tool_call_count,
                elapsed_ms,
                ..
            } => self.write_line(&format!(
                "model iter {iteration}: received {tool_call_count} tool call(s) in {elapsed_ms}ms"
            )),
            RuntimeEvent::ParserFeedbackSent {
                next_tool_call_mode,
                error,
                ..
            } => self.write_line(&format!(
                "parser: fallback={} reason={}",
                tool_mode_label(next_tool_call_mode),
                sanitize_for_progress(&error)
            )),
            RuntimeEvent::GuardFeedbackSent {
                kind,
                missing_artifacts,
                ..
            } => self.write_line(&format!(
                "guard: {}{}",
                guard_label(kind),
                if missing_artifacts.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", missing_artifacts.join(", "))
                }
            )),
            RuntimeEvent::ArtifactStatus {
                scope,
                path,
                status,
            } => self.write_line(&format!(
                "[{}] {} {}",
                artifact_status_label(status),
                artifact_scope_label(scope),
                sanitize_for_progress(&path)
            )),
            RuntimeEvent::ToolCallStarted {
                tool_name,
                args_summary,
                ..
            } => {
                let tool_wait_label = format!(
                    "tool {} {}",
                    sanitize_for_progress(&tool_name),
                    sanitize_for_progress(&args_summary)
                );
                let label = paint(
                    &tool_name,
                    tool_color(&tool_name),
                    self.config.color_enabled,
                );
                let summary = format!("{label} {}", sanitize_for_progress(&args_summary));
                self.last_tool = Some(summary.clone());
                self.write_line(&format!("tool: {summary}"))?;
                self.start_wait(tool_wait_label);
                Ok(())
            }
            RuntimeEvent::ToolCallFinished {
                tool_name,
                ok,
                error,
                ..
            } => {
                if ok {
                    self.write_line(&format!("tool: {} ok", sanitize_for_progress(&tool_name)))
                } else {
                    self.write_line(&format!(
                        "tool: {} failed {}",
                        sanitize_for_progress(&tool_name),
                        error.unwrap_or_default()
                    ))
                }
            }
            RuntimeEvent::ToolResultTruncated {
                tool_name,
                original_chars,
                returned_chars,
                reason,
                ..
            } => self.write_line(&format!(
                "tool: {} output truncated {original_chars}->{returned_chars} chars ({})",
                sanitize_for_progress(&tool_name),
                sanitize_for_progress(&reason)
            )),
            RuntimeEvent::FinalAnswerAccepted { iteration, .. } => {
                self.write_line(&format!("model iter {iteration}: final accepted"))
            }
            RuntimeEvent::SessionError { message } => {
                if self.current_phase.is_some() || self.current_step.is_some() {
                    self.write_line(&format!(
                        "failed at: {}{}",
                        self.current_phase.as_deref().unwrap_or("direct"),
                        self.current_step
                            .as_ref()
                            .map(|step| format!(" > {step}"))
                            .unwrap_or_default()
                    ))?;
                }
                self.write_line(&format!(
                    "error context: {}",
                    sanitize_for_progress(&message)
                ))
            }
        }
    }
}

pub fn render_final_answer(answer: &str) -> String {
    let env = env::detect();
    render_final_answer_with(
        answer,
        env.markdown_enabled,
        env.stdout_color_enabled,
        env.utf8_locale,
    )
}

pub fn render_final_answer_with(
    answer: &str,
    markdown_enabled: bool,
    color_enabled: bool,
    utf8: bool,
) -> String {
    if !markdown_enabled {
        return answer.trim().to_string();
    }
    let mut renderer = MarkdownRenderer::new(color_enabled, utf8);
    let mut out = renderer.push_chunk(answer);
    out.push_str(&renderer.flush());
    out.trim().to_string()
}

fn model_label(provider: Provider, model: Option<&str>) -> String {
    format!("{}:{}", provider.as_str(), model.unwrap_or("default"))
}

fn plan_kind_label(kind: PlanKind) -> &'static str {
    match kind {
        PlanKind::StepPlan => "step plan",
        PlanKind::UltraPlan => "ultra plan",
        PlanKind::PhaseStepPlan => "phase step plan",
    }
}

fn tool_mode_label(mode: ToolCallMode) -> &'static str {
    match mode {
        ToolCallMode::Native => "native",
        ToolCallMode::XmlFallback => "xml",
    }
}

fn guard_label(kind: GuardFeedbackKind) -> &'static str {
    match kind {
        GuardFeedbackKind::FutureAction => "future action feedback",
        GuardFeedbackKind::RequestedArtifacts => "requested artifact feedback",
        GuardFeedbackKind::ActionRequired => "action required feedback",
    }
}

fn artifact_scope_label(scope: ArtifactScope) -> &'static str {
    match scope {
        ArtifactScope::StepExpectedPath => "step expected path",
        ArtifactScope::FinalRequiredArtifact => "final artifact",
    }
}

fn artifact_status_label(status: ArtifactStatus) -> &'static str {
    match status {
        ArtifactStatus::Ok => "ok",
        ArtifactStatus::Missing => "missing",
        ArtifactStatus::Unchecked => "unchecked",
    }
}

fn preview_ids(ids: &[String]) -> String {
    const MAX_IDS: usize = 8;
    let mut shown = ids
        .iter()
        .take(MAX_IDS)
        .map(|id| sanitize_for_progress(id))
        .collect::<Vec<_>>();
    if ids.len() > MAX_IDS {
        shown.push(format!("... +{}", ids.len() - MAX_IDS));
    }
    shown.join(", ")
}

pub fn compact_path(cwd: &Path, path: &Path) -> String {
    path.strip_prefix(cwd).unwrap_or(path).display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::events::RuntimeEvent;
    use std::path::PathBuf;

    #[test]
    fn renders_step_failure_with_context_and_last_tool() {
        let mut ui = TerminalUi::new(
            Vec::new(),
            TerminalUiConfig {
                progress_enabled: true,
                spinner_enabled: false,
                banner_enabled: false,
                color_enabled: false,
                markdown_enabled: false,
                utf8: false,
            },
        );

        ui.on_event(RuntimeEvent::UltraPhaseStarted {
            index: 1,
            total: 2,
            phase_id: "scaffold".to_string(),
        });
        ui.on_event(RuntimeEvent::ToolCallStarted {
            iteration: 1,
            tool_name: "Write".to_string(),
            args_summary: "package.json 20B".to_string(),
        });
        ui.on_event(RuntimeEvent::StepFailed {
            index: 1,
            total: 3,
            step_id: "create-package-json".to_string(),
            error: "missing expected artifacts: package.json".to_string(),
            missing_expected_paths: vec!["package.json".to_string()],
        });

        let text = String::from_utf8(ui.into_inner()).unwrap();
        assert!(
            text.contains("failed at: ultra phase 1/2 scaffold > step 1/3 create-package-json")
        );
        assert!(text.contains("expected paths: package.json"));
        assert!(text.contains("last tool: Write package.json 20B"));
    }

    #[test]
    fn renders_repair_next_command_block() {
        let mut ui = TerminalUi::new(
            Vec::new(),
            TerminalUiConfig {
                progress_enabled: true,
                spinner_enabled: false,
                banner_enabled: false,
                color_enabled: false,
                markdown_enabled: false,
                utf8: false,
            },
        );

        ui.on_event(RuntimeEvent::RepairExhausted {
            step_id: "write".to_string(),
            repair_path: ".commandagent/repairs/repair.md".to_string(),
            suggested_command: "/plan-run \"$(cat repair.md)\"".to_string(),
            missing_expected_paths: vec!["package.json".to_string()],
        });

        let text = String::from_utf8(ui.into_inner()).unwrap();
        assert!(text.contains("repair: .commandagent/repairs/repair.md"));
        assert!(text.contains("standalone repair plan"));
        assert!(text.contains("next command:"));
        assert!(text.contains("/plan-run"));
    }

    #[test]
    fn renders_profile_verification_failure() {
        let mut ui = TerminalUi::new(
            Vec::new(),
            TerminalUiConfig {
                progress_enabled: true,
                spinner_enabled: false,
                banner_enabled: false,
                color_enabled: false,
                markdown_enabled: false,
                utf8: false,
            },
        );

        ui.on_event(RuntimeEvent::ProfileVerificationFailed {
            profile: "nextjs".to_string(),
            failures: vec!["nextjs_dev_port_drift: scripts.dev lost 3011".to_string()],
        });

        let text = String::from_utf8(ui.into_inner()).unwrap();
        assert!(text.contains("profile verification nextjs: failed"));
        assert!(text.contains("nextjs_dev_port_drift"));
    }

    #[test]
    fn final_answer_markdown_is_tty_gated() {
        assert_eq!(
            render_final_answer_with("# Title", false, true, true),
            "# Title"
        );
        assert_eq!(
            render_final_answer_with("# Title", true, false, true),
            "Title"
        );
    }

    #[test]
    fn compact_path_prefers_workspace_relative() {
        assert_eq!(
            compact_path(Path::new("/tmp/work"), &PathBuf::from("/tmp/work/a/b.txt")),
            "a/b.txt"
        );
    }

    #[test]
    fn renders_plain_startup_context_when_banner_disabled() {
        let root = PathBuf::from("/tmp/work");
        let config = Config {
            cwd: root,
            provider: Provider::Gemini,
            model: Some("gemini-3.1-flash-lite".to_string()),
            planner_provider: None,
            planner_model: Some("gemini-3.5-flash".to_string()),
            context_budget: 1024,
            max_iterations: 8,
            timeout_secs: 120,
            retries: 0,
            yes: true,
            offline: false,
            state_dir: PathBuf::from(".commandagent"),
            resume: None,
            openai_api_key: None,
            gemini_api_key: None,
        };
        let targets = ProviderTargets {
            executor: crate::providers::planner::ModelTarget {
                provider: Provider::Gemini,
                model: Some("gemini-3.1-flash-lite".to_string()),
            },
            planner: crate::providers::planner::ModelTarget {
                provider: Provider::Gemini,
                model: Some("gemini-3.5-flash".to_string()),
            },
        };
        let mut ui = TerminalUi::new(
            Vec::new(),
            TerminalUiConfig {
                progress_enabled: true,
                spinner_enabled: false,
                banner_enabled: false,
                color_enabled: false,
                markdown_enabled: false,
                utf8: false,
            },
        );

        ui.render_startup_context("0.1.0", &config, &targets)
            .unwrap();

        let text = String::from_utf8(ui.into_inner()).unwrap();
        assert_eq!(
            text,
            "CommandAgent 0.1.0 cwd=work provider=gemini:gemini-3.1-flash-lite planner=gemini:gemini-3.5-flash [yes]\n"
        );
    }

    #[test]
    fn renders_startup_logo_when_banner_enabled() {
        let root = PathBuf::from("/tmp/work");
        let config = Config {
            cwd: root,
            provider: Provider::Gemini,
            model: Some("gemini-3.1-flash-lite".to_string()),
            planner_provider: None,
            planner_model: Some("gemini-3.5-flash".to_string()),
            context_budget: 1024,
            max_iterations: 8,
            timeout_secs: 120,
            retries: 0,
            yes: true,
            offline: false,
            state_dir: PathBuf::from(".commandagent"),
            resume: None,
            openai_api_key: None,
            gemini_api_key: None,
        };
        let targets = ProviderTargets {
            executor: crate::providers::planner::ModelTarget {
                provider: Provider::Gemini,
                model: Some("gemini-3.1-flash-lite".to_string()),
            },
            planner: crate::providers::planner::ModelTarget {
                provider: Provider::Gemini,
                model: Some("gemini-3.5-flash".to_string()),
            },
        };
        let mut ui = TerminalUi::new(
            Vec::new(),
            TerminalUiConfig {
                progress_enabled: true,
                spinner_enabled: false,
                banner_enabled: true,
                color_enabled: false,
                markdown_enabled: false,
                utf8: false,
            },
        );

        ui.render_startup_context("0.1.0", &config, &targets)
            .unwrap();

        let text = String::from_utf8(ui.into_inner()).unwrap();
        assert!(text.contains("____"));
        assert!(text.contains("CommandAgent 0.1.0 cwd=work [yes]"));
        assert!(text.contains("provider=gemini:gemini-3.1-flash-lite"));
        assert!(text.contains("planner=gemini:gemini-3.5-flash"));
    }
}
