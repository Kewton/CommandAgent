use std::io::{self, IsTerminal, Write};
use std::net::TcpListener;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use anyhow::{Context, bail};

use crate::config::{Action, Config};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortOwner {
    pub pid: Option<u32>,
    pub command: String,
}

impl PortOwner {
    fn display(&self) -> String {
        match self.pid {
            Some(pid) if !self.command.trim().is_empty() => {
                format!("pid {pid} ({})", self.command.trim())
            }
            Some(pid) => format!("pid {pid}"),
            None if !self.command.trim().is_empty() => self.command.trim().to_string(),
            None => "unknown owner".to_string(),
        }
    }
}

trait PreflightIo {
    fn interactive(&self) -> bool;
    fn line(&mut self, line: &str);
    fn choice(&mut self) -> anyhow::Result<String>;
}

trait PortControl {
    fn inspect(&self, port: u16) -> anyhow::Result<Option<PortOwner>>;
    fn kill(&self, pid: u32) -> anyhow::Result<()>;
}

trait ProbeCheck {
    fn unavailable_reason(&self, root: &Path) -> Option<String>;
}

pub fn run_for_action(config: &Config) -> anyhow::Result<()> {
    let Some(goal) = goal_for_preflight(&config.action) else {
        return Ok(());
    };
    let mut io = TerminalPreflightIo;
    run_for_goal_with(config, goal, &mut io, &SystemPortControl, &SystemProbeCheck)
}

pub fn run_for_slash_command(config: &Config, command: &str, goal: &str) -> anyhow::Result<()> {
    if !slash_command_needs_preflight(command) {
        return Ok(());
    }
    let mut io = TerminalPreflightIo;
    run_for_goal_with(config, goal, &mut io, &SystemPortControl, &SystemProbeCheck)
}

fn goal_for_preflight(action: &Action) -> Option<&str> {
    match action {
        Action::PlanRun(goal) | Action::UltraPlanRun(goal) => Some(goal.as_str()),
        _ => None,
    }
}

fn slash_command_needs_preflight(command: &str) -> bool {
    matches!(command, "/plan-run" | "/ultra-plan-run")
}

fn run_for_goal_with(
    config: &Config,
    goal: &str,
    io: &mut dyn PreflightIo,
    ports: &dyn PortControl,
    probes: &dyn ProbeCheck,
) -> anyhow::Result<()> {
    let runtime = crate::planner::profile::resolve_profile_runtime(&config.profile);
    let Some(default_port) = runtime.default_requested_port() else {
        return Ok(());
    };
    let port = crate::planner::signals::requested_port_from_text(goal).unwrap_or(default_port);
    if let Some(owner) = ports.inspect(port)? {
        handle_busy_port(config, io, ports, port, owner)?;
    }
    if let Some(reason) = probes.unavailable_reason(&config.workspace_root) {
        io.line(&format!(
            "preflight: interaction probe unavailable ({reason}); {}; continuing with degraded interaction gates",
            crate::minimal_loop::interaction_probe::INTERACTION_PROBE_SETUP_REMEDIATION
        ));
    }
    Ok(())
}

fn handle_busy_port(
    config: &Config,
    io: &mut dyn PreflightIo,
    ports: &dyn PortControl,
    port: u16,
    owner: PortOwner,
) -> anyhow::Result<()> {
    let message = format!("preflight: port {port} is busy: {}", owner.display());
    if config.yes {
        bail!("{message}; --yes never auto-kills processes");
    }
    if !io.interactive() {
        bail!("{message}; no TTY available for [k]ill/[a]bort");
    }
    io.line(&format!("{message}. Choose [k]ill / [a]bort:"));
    match io.choice()?.trim().to_ascii_lowercase().as_str() {
        "k" | "kill" => {
            let Some(pid) = owner.pid else {
                bail!("{message}; cannot kill because no owning pid was detected");
            };
            ports.kill(pid)?;
            io.line(&format!(
                "preflight: sent SIGTERM to pid {pid} for port {port}"
            ));
            Ok(())
        }
        _ => bail!("preflight aborted: port {port} is busy"),
    }
}

struct TerminalPreflightIo;

impl PreflightIo for TerminalPreflightIo {
    fn interactive(&self) -> bool {
        io::stdin().is_terminal()
    }

    fn line(&mut self, line: &str) {
        eprintln!("{line}");
    }

    fn choice(&mut self) -> anyhow::Result<String> {
        eprint!("preflight> ");
        io::stderr().flush()?;
        let mut answer = String::new();
        io::stdin().read_line(&mut answer)?;
        Ok(answer)
    }
}

struct SystemProbeCheck;

impl ProbeCheck for SystemProbeCheck {
    fn unavailable_reason(&self, root: &Path) -> Option<String> {
        crate::minimal_loop::interaction_probe::playwright_availability(root)
            .unavailable_reason()
            .map(ToOwned::to_owned)
    }
}

struct SystemPortControl;

impl PortControl for SystemPortControl {
    fn inspect(&self, port: u16) -> anyhow::Result<Option<PortOwner>> {
        if TcpListener::bind(("127.0.0.1", port)).is_ok() {
            return Ok(None);
        }
        Ok(Some(lsof_owner(port).unwrap_or_else(|| PortOwner {
            pid: None,
            command: "unknown owner".to_string(),
        })))
    }

    fn kill(&self, pid: u32) -> anyhow::Result<()> {
        #[cfg(unix)]
        {
            let pid = i32::try_from(pid).context("owning pid does not fit platform pid type")?;
            // Normal path (Issue #673): a pid that must not be signalled returns
            // `Err`, never a panic. lsof can report pid 1 under socket activation
            // (launchd/systemd), and a user who chose kill must still get the
            // pre-#673 `Err`. 0 (the caller's own group) and 1 (init) are refused
            // here, then the caller's own pid. This is not an ownership proof for
            // the port owner.
            if pid <= 1 {
                bail!("refusing to signal pid {pid}");
            }
            if pid == std::process::id() as i32 {
                bail!("refusing to signal the caller's own pid");
            }
            // Last, non-mutatable defense (Issue #673): the `bail!` paths above
            // are the normal result, and these `assert!`s are the fortress for a
            // mutated tree. cargo-mutants does not rewrite comparisons inside
            // `assert!`, and they are written directly in the function that calls
            // `kill`.
            assert!(pid > 1, "refusing to signal pid {pid}");
            assert!(
                pid != std::process::id() as i32,
                "refusing to signal the caller's own pid"
            );
            // SAFETY: sends SIGTERM to the concrete pid reported by lsof after
            // explicit user confirmation in the preflight prompt.
            let rc = unsafe { libc::kill(pid, libc::SIGTERM) };
            if rc == 0 {
                Ok(())
            } else {
                Err(std::io::Error::last_os_error()).context("failed to kill owning process")
            }
        }
        #[cfg(not(unix))]
        {
            let _ = pid;
            bail!("preflight kill is only supported on Unix platforms");
        }
    }
}

fn lsof_owner(port: u16) -> Option<PortOwner> {
    let port_spec = format!("-iTCP:{port}");
    let mut command = Command::new("lsof");
    command
        .args(["-nP", &port_spec, "-sTCP:LISTEN", "-F", "pc"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output =
        crate::bounded_process::run_with_timeout(&mut command, Duration::from_secs(2)).ok()?;
    if !output.success() {
        return None;
    }
    parse_lsof_owner(&String::from_utf8(output.stdout).ok()?)
}

fn parse_lsof_owner(text: &str) -> Option<PortOwner> {
    let mut pid = None;
    let mut command = None;
    for line in text.lines() {
        if let Some(raw) = line.strip_prefix('p') {
            pid = raw.parse::<u32>().ok();
        } else if let Some(raw) = line.strip_prefix('c') {
            command = Some(raw.to_string());
        }
        if pid.is_some() && command.is_some() {
            break;
        }
    }
    pid.or_else(|| command.as_ref().map(|_| 0))
        .map(|pid_value| PortOwner {
            pid: (pid_value != 0).then_some(pid_value),
            command: command.unwrap_or_default(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ConfigFieldSources, NarrationMode, Provider};
    use std::cell::RefCell;
    use std::path::PathBuf;

    #[derive(Default)]
    struct FakeIo {
        interactive: bool,
        choices: Vec<String>,
        lines: Vec<String>,
    }

    impl PreflightIo for FakeIo {
        fn interactive(&self) -> bool {
            self.interactive
        }

        fn line(&mut self, line: &str) {
            self.lines.push(line.to_string());
        }

        fn choice(&mut self) -> anyhow::Result<String> {
            Ok(self.choices.pop().unwrap_or_default())
        }
    }

    struct FakePorts {
        owner: Option<PortOwner>,
        killed: RefCell<Vec<u32>>,
    }

    impl PortControl for FakePorts {
        fn inspect(&self, _port: u16) -> anyhow::Result<Option<PortOwner>> {
            Ok(self.owner.clone())
        }

        fn kill(&self, pid: u32) -> anyhow::Result<()> {
            self.killed.borrow_mut().push(pid);
            Ok(())
        }
    }

    struct FakeProbe(Option<String>);

    impl ProbeCheck for FakeProbe {
        fn unavailable_reason(&self, _root: &Path) -> Option<String> {
            self.0.clone()
        }
    }

    fn config(yes: bool) -> Config {
        Config {
            workspace_root: PathBuf::from("."),
            state_dir: PathBuf::from("state"),
            eval_events_path: None,
            completion_contract_path: None,
            yes,
            offline: false,
            context_budget: 1000,
            model: "m".to_string(),
            provider: Provider::Ollama,
            tool_protocol: None,
            openai_api: crate::config::OpenAiApi::ChatCompletions,
            prompt_layout: crate::config::PromptLayout::Stable,
            plan_preset: crate::config::PlanPreset::None,
            intent_override: None,
            planner_model: "m".to_string(),
            planner_provider: Provider::Ollama,
            planner_think: Some(crate::config::OllamaThink::False),
            classifier_model: "m".to_string(),
            classifier_provider: Provider::Ollama,
            openai_compatible: None,
            ollama_host: "http://localhost:11434".to_string(),
            ollama_think: None,
            lm_studio_host: "http://localhost:1234".to_string(),
            num_predict: 100,
            max_iterations: 4,
            recovery_plan_auto_runs: 0,
            chat_timeout_secs: 1,
            chat_timeout_source: "override:test".to_string(),
            field_sources: ConfigFieldSources::default(),
            chat_retries: 1,
            stream: false,
            resume: None,
            fresh_session: false,
            no_footer: false,
            narration: NarrationMode::Normal,
            profile: "nextjs".to_string(),
            profile_explicit: true,
            profile_inference: None,
            style: "default".to_string(),
            action: Action::Repl,
        }
    }

    #[test]
    fn busy_port_interactive_kill_path_records_owner_and_kills() {
        let mut io = FakeIo {
            interactive: true,
            choices: vec!["k".to_string()],
            lines: Vec::new(),
        };
        let ports = FakePorts {
            owner: Some(PortOwner {
                pid: Some(1234),
                command: "next dev".to_string(),
            }),
            killed: RefCell::new(Vec::new()),
        };

        run_for_goal_with(
            &config(false),
            "Create app on port 4000",
            &mut io,
            &ports,
            &FakeProbe(None),
        )
        .unwrap();

        assert_eq!(&*ports.killed.borrow(), &[1234]);
        assert!(
            io.lines
                .iter()
                .any(|line| line.contains("port 4000 is busy: pid 1234 (next dev)")),
            "{:?}",
            io.lines
        );
    }

    #[test]
    fn busy_port_yes_aborts_without_kill() {
        let mut io = FakeIo::default();
        let ports = FakePorts {
            owner: Some(PortOwner {
                pid: Some(2222),
                command: "server".to_string(),
            }),
            killed: RefCell::new(Vec::new()),
        };

        let err = run_for_goal_with(
            &config(true),
            "Create app on port 4000",
            &mut io,
            &ports,
            &FakeProbe(None),
        )
        .unwrap_err()
        .to_string();

        assert!(
            err.contains("port 4000 is busy: pid 2222 (server)"),
            "{err}"
        );
        assert!(err.contains("--yes never auto-kills"), "{err}");
        assert!(ports.killed.borrow().is_empty());
    }

    #[test]
    fn probe_missing_warns_and_continues() {
        let mut io = FakeIo::default();
        let ports = FakePorts {
            owner: None,
            killed: RefCell::new(Vec::new()),
        };

        run_for_goal_with(
            &config(false),
            "Create app",
            &mut io,
            &ports,
            &FakeProbe(Some("playwright_not_installed".to_string())),
        )
        .unwrap();

        assert_eq!(io.lines.len(), 1);
        assert!(io.lines[0].contains("playwright_not_installed"));
        assert!(io.lines[0].contains("/setup-interaction-probe"));
    }

    #[test]
    fn lsof_field_output_parses_pid_and_command() {
        let owner = parse_lsof_owner("p123\ncnext-server\n").unwrap();
        assert_eq!(owner.pid, Some(123));
        assert_eq!(owner.command, "next-server");
    }

    /// Issue #673 対処の方向 4: pass 0 (the caller's own group), 1 (init), and
    /// the caller's own pid straight to the function that calls `kill`, so a
    /// mutated tree reaches the last, non-mutatable `assert!`. On an unmutated
    /// tree the `bail!` paths return `Err` before `libc::kill`, so the message
    /// is `refusing …`, never `failed to kill owning process`. Under a mutation
    /// it runs only in the isolated container; on an unmutated tree it also runs
    /// in host CI and verify.
    #[cfg(unix)]
    #[test]
    fn dangerous_value_system_port_control_kill_refuses_init_zero_and_own_pid() {
        let ports = SystemPortControl;
        for pid in [0, 1, std::process::id()] {
            let error = ports.kill(pid).expect_err("must refuse");
            let message = error.to_string();
            assert!(message.contains("refusing"), "{message}");
            assert!(
                !message.contains("failed to kill owning process"),
                "the refusal must not reach libc::kill: {message}"
            );
        }
    }
}
