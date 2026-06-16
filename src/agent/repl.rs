use crate::agent::minimal_loop::loop_run::{
    ChatClient, MinimalLoopConfig, MinimalLoopError, run_session,
};
use crate::session::store::{SessionRole, SessionStore};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

pub trait ReplTurnRunner {
    fn run_turn(&mut self, prompt: &str) -> Result<String, String>;
    fn save(&mut self) -> Result<(), String>;
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
                    writeln!(output, "{}", answer.trim())?;
                }
                runner.save().map_err(ReplError::Runner)?;
            }
            Err(err) => {
                writeln!(output, "ERROR: {err}")?;
            }
        }
    }
}

pub struct MinimalReplRunner<C> {
    client: C,
    cwd: PathBuf,
    loop_config: MinimalLoopConfig,
    store: SessionStore,
    snapshot: crate::session::store::SessionSnapshot,
}

impl<C> MinimalReplRunner<C>
where
    C: ChatClient,
{
    pub fn new(
        client: C,
        cwd: impl AsRef<Path>,
        loop_config: MinimalLoopConfig,
    ) -> Result<Self, String> {
        let cwd = cwd.as_ref().to_path_buf();
        let store = SessionStore::new(&cwd).map_err(|err| err.to_string())?;
        let snapshot = store.create();
        Ok(Self {
            client,
            cwd,
            loop_config,
            store,
            snapshot,
        })
    }

    pub fn session_id(&self) -> &str {
        &self.snapshot.id
    }
}

impl<C> ReplTurnRunner for MinimalReplRunner<C>
where
    C: ChatClient,
{
    fn run_turn(&mut self, prompt: &str) -> Result<String, String> {
        let result = run_session(
            &mut self.client,
            &self.cwd,
            prompt,
            self.loop_config.clone(),
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
    use std::collections::VecDeque;
    use std::io::Cursor;

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
}
