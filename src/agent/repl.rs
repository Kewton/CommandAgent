use crate::agent::minimal_loop::loop_run::{
    ChatClient, MinimalLoopConfig, MinimalLoopError, run_session_with_observer,
};
use crate::agent::slash_command::parse_slash_command;
use crate::agent::step_runner::runtime::{PlannerRuntimeConfig, SlashRuntime};
use crate::session::store::{SessionRole, SessionStore};
use crate::tui::terminal::{TerminalUi, render_final_answer};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

pub trait ReplTurnRunner {
    fn run_turn(&mut self, prompt: &str) -> Result<String, String>;
    fn save(&mut self) -> Result<(), String>;
    fn format_answer(&mut self, answer: &str) -> String {
        answer.trim().to_string()
    }
}

pub fn run_repl<R, W, T>(mut input: R, mut output: W, runner: &mut T) -> Result<(), ReplError>
where
    R: BufRead,
    W: Write,
    T: ReplTurnRunner,
{
    let mut line = String::new();
    loop {
        write!(output, "commandagent> ")?;
        output.flush()?;

        line.clear();
        let read = input.read_line(&mut line)?;
        if read == 0 {
            writeln!(output)?;
            return Ok(());
        }

        let prompt = line.trim();
        if prompt.is_empty() {
            continue;
        }
        if prompt == "/exit" || prompt == "/quit" {
            writeln!(output, "bye")?;
            return Ok(());
        }

        match runner.run_turn(prompt) {
            Ok(answer) => {
                if !answer.trim().is_empty() {
                    writeln!(output, "{}", runner.format_answer(&answer))?;
                }
                runner.save().map_err(ReplError::Runner)?;
            }
            Err(err) => {
                writeln!(output, "ERROR: {err}")?;
            }
        }
    }
}

pub struct MinimalReplRunner<C, P> {
    client: C,
    planner_client: P,
    cwd: PathBuf,
    loop_config: MinimalLoopConfig,
    planner_config: PlannerRuntimeConfig,
    ui: TerminalUi<std::io::Stderr>,
    store: SessionStore,
    snapshot: crate::session::store::SessionSnapshot,
}

impl<C, P> MinimalReplRunner<C, P>
where
    C: ChatClient,
    P: ChatClient,
{
    pub fn new(
        client: C,
        planner_client: P,
        cwd: impl AsRef<Path>,
        loop_config: MinimalLoopConfig,
        planner_config: PlannerRuntimeConfig,
    ) -> Result<Self, String> {
        let cwd = cwd.as_ref().to_path_buf();
        let store = SessionStore::new(&cwd).map_err(|err| err.to_string())?;
        let snapshot = store.create();
        Ok(Self {
            client,
            planner_client,
            cwd,
            loop_config,
            planner_config,
            ui: TerminalUi::stderr_from_env(),
            store,
            snapshot,
        })
    }

    pub fn session_id(&self) -> &str {
        &self.snapshot.id
    }
}

impl<C, P> ReplTurnRunner for MinimalReplRunner<C, P>
where
    C: ChatClient,
    P: ChatClient,
{
    fn run_turn(&mut self, prompt: &str) -> Result<String, String> {
        if let Some(command) =
            parse_slash_command(prompt, &self.cwd).map_err(|err| err.to_string())?
        {
            let mut runtime = SlashRuntime {
                executor: &mut self.client,
                planner: &mut self.planner_client,
                cwd: &self.cwd,
                loop_config: self.loop_config.clone(),
                planner_config: self.planner_config.clone(),
            };
            let answer = runtime.run_with_observer(command, &mut self.ui)?;
            self.snapshot.push(SessionRole::User, prompt);
            self.snapshot.push(SessionRole::Assistant, answer.clone());
            return Ok(answer);
        }

        let result = run_session_with_observer(
            &mut self.client,
            &self.cwd,
            prompt,
            self.loop_config.clone(),
            &mut self.ui,
        )
        .map_err(format_loop_error)?;

        self.snapshot.push(SessionRole::User, prompt);
        self.snapshot
            .push(SessionRole::Assistant, result.final_answer.clone());
        Ok(result.final_answer)
    }

    fn save(&mut self) -> Result<(), String> {
        self.store
            .save(&mut self.snapshot)
            .map_err(|err| err.to_string())
    }

    fn format_answer(&mut self, answer: &str) -> String {
        render_final_answer(answer)
    }
}

#[derive(Debug)]
pub enum ReplError {
    Io(std::io::Error),
    Runner(String),
}

impl std::fmt::Display for ReplError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "{}", err),
            Self::Runner(message) => write!(f, "{}", message),
        }
    }
}

impl std::error::Error for ReplError {}

impl From<std::io::Error> for ReplError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

fn format_loop_error(err: MinimalLoopError) -> String {
    err.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::{ChatRequest, ChatResponse, ToolCall, ToolCallMode};
    use std::collections::VecDeque;
    use std::fs;
    use std::io::Cursor;
    use std::path::PathBuf;

    #[test]
    fn repl_skips_empty_lines_and_exits() {
        let mut runner = MockRunner::new(vec![]);
        let input = Cursor::new("\n/exit\n");
        let mut output = Vec::new();

        run_repl(input, &mut output, &mut runner).unwrap();

        assert_eq!(runner.prompts, Vec::<String>::new());
        assert_eq!(runner.save_count, 0);
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("commandagent> "));
        assert!(text.contains("bye"));
    }

    #[test]
    fn repl_runs_turn_and_saves_after_success() {
        let mut runner = MockRunner::new(vec![Ok("done".to_string())]);
        let input = Cursor::new("create file\n/quit\n");
        let mut output = Vec::new();

        run_repl(input, &mut output, &mut runner).unwrap();

        assert_eq!(runner.prompts, vec!["create file"]);
        assert_eq!(runner.save_count, 1);
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("done"));
    }

    #[test]
    fn repl_reports_turn_error_and_continues() {
        let mut runner = MockRunner::new(vec![Err("failed".to_string()), Ok("ok".to_string())]);
        let input = Cursor::new("bad\nnext\n/exit\n");
        let mut output = Vec::new();

        run_repl(input, &mut output, &mut runner).unwrap();

        assert_eq!(runner.prompts, vec!["bad", "next"]);
        assert_eq!(runner.save_count, 1);
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("ERROR: failed"));
        assert!(text.contains("ok"));
    }

    #[test]
    fn minimal_repl_dispatches_plan_run_slash_command() {
        let root = temp_workspace("slash-plan-run");
        let plan_yaml = "goal: \"Create docs\"\nprofile: \"docs\"\nstyle: \"default\"\nsteps:\n  - id: \"write-readme\"\n    instruction: \"Create README.md.\"\n    expected_paths:\n      - \"README.md\"\n    verify:\n      - \"cat README.md\"\n";
        let planner = MockChatClient::new(vec![ChatResponse::new(plan_yaml, Vec::new())]);
        let executor = MockChatClient::new(vec![
            ChatResponse::new(
                String::new(),
                vec![ToolCall {
                    id: None,
                    thought_signature: None,
                    name: "Write".to_string(),
                    args_json: r#"{"path":"README.md","content":"ok"}"#.to_string(),
                }],
            ),
            ChatResponse::new("Created README.md.", Vec::new()),
        ]);
        let mut runner = MinimalReplRunner::new(
            executor,
            planner,
            &root,
            MinimalLoopConfig::default(),
            PlannerRuntimeConfig {
                model: "planner".to_string(),
                tool_call_mode: ToolCallMode::XmlFallback,
            },
        )
        .unwrap();
        let input = Cursor::new("/plan-run --profile docs Create README\n/exit\n");
        let mut output = Vec::new();

        run_repl(input, &mut output, &mut runner).unwrap();

        assert_eq!(fs::read_to_string(root.join("README.md")).unwrap(), "ok");
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("step write-readme: ok"));
    }

    struct MockRunner {
        responses: VecDeque<Result<String, String>>,
        prompts: Vec<String>,
        save_count: usize,
    }

    impl MockRunner {
        fn new(responses: Vec<Result<String, String>>) -> Self {
            Self {
                responses: VecDeque::from(responses),
                prompts: Vec::new(),
                save_count: 0,
            }
        }
    }

    impl ReplTurnRunner for MockRunner {
        fn run_turn(&mut self, prompt: &str) -> Result<String, String> {
            self.prompts.push(prompt.to_string());
            self.responses
                .pop_front()
                .unwrap_or_else(|| Err("missing mock response".to_string()))
        }

        fn save(&mut self) -> Result<(), String> {
            self.save_count += 1;
            Ok(())
        }
    }

    struct MockChatClient {
        responses: VecDeque<ChatResponse>,
    }

    impl MockChatClient {
        fn new(responses: Vec<ChatResponse>) -> Self {
            Self {
                responses: VecDeque::from(responses),
            }
        }
    }

    impl ChatClient for MockChatClient {
        fn chat(&mut self, _request: &ChatRequest) -> Result<ChatResponse, String> {
            self.responses
                .pop_front()
                .ok_or_else(|| "missing mock response".to_string())
        }
    }

    fn temp_workspace(name: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("commandagent-repl-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }
}
