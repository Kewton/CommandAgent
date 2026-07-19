use std::borrow::Cow;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use rustyline::completion::{Completer, Pair};
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::hint::{Hinter, HistoryHinter};
use rustyline::history::DefaultHistory;
use rustyline::validate::{ValidationContext, ValidationResult, Validator};
use rustyline::{
    Cmd, ColorMode, CompletionType, ConditionalEventHandler, Context, Editor, Event, EventContext,
    EventHandler, Helper, KeyCode, KeyEvent, Modifiers, RepeatCount,
};

use crate::config::Config;
use crate::tui::slash::{SLASH_COMMANDS, SlashCommandKind, slash_command_spec};

const FLAGS: &[&str] = &["--profile", "--style", "--prompt-layout"];
const CONTINUATION_PROMPT: &str = "... ";

pub struct ReplEditor {
    editor: Editor<ReplHelper, DefaultHistory>,
    prompt_state: Arc<PromptState>,
}

impl ReplEditor {
    pub fn new(config: &Config) -> anyhow::Result<Self> {
        let hints_use_color =
            !crate::tui::terminal::no_color() && crate::tui::terminal::utf8_locale();
        let rustyline_config = editor_config();
        let mut editor = Editor::with_config(rustyline_config)?;
        let prompt_state = Arc::new(PromptState::default());
        editor.set_helper(Some(ReplHelper::new(
            config.workspace_root.clone(),
            hints_use_color,
        )));
        editor.bind_sequence(
            KeyEvent::ctrl('C'),
            EventHandler::Conditional(Box::new(CtrlCHandler {
                state: Arc::clone(&prompt_state),
            })),
        );
        editor.bind_sequence(
            KeyEvent(KeyCode::End, Modifiers::NONE),
            EventHandler::Conditional(Box::new(EndHandler {
                state: Arc::clone(&prompt_state),
            })),
        );
        editor.bind_sequence(
            Event::Any,
            EventHandler::Conditional(Box::new(InputActivityHandler {
                state: Arc::clone(&prompt_state),
            })),
        );
        Ok(Self {
            editor,
            prompt_state,
        })
    }

    pub fn readline(&mut self, prompt: &str) -> Result<String, ReadlineError> {
        self.editor.readline(prompt)
    }

    pub fn load_history(&mut self, path: &Path) -> Result<(), ReadlineError> {
        self.editor.load_history(path)
    }

    pub fn save_history(&mut self, path: &Path) -> Result<(), ReadlineError> {
        self.editor.save_history(path)
    }

    pub fn add_history_entry(&mut self, line: &str) -> Result<bool, ReadlineError> {
        self.editor.add_history_entry(line)
    }

    pub fn take_interrupt_action(&self) -> PromptInterruptAction {
        self.prompt_state.take_action()
    }
}

fn editor_config() -> rustyline::Config {
    rustyline::Config::builder()
        .completion_type(CompletionType::List)
        .bracketed_paste(true)
        // Keep the Highlighter active under NO_COLOR so it can still render
        // the plain ASCII continuation prompt. ReplHelper itself controls
        // whether any SGR sequence is emitted.
        .color_mode(ColorMode::Enabled)
        .build()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptInterruptAction {
    ClearLine,
    WarnBeforeExit,
    Exit,
}

const NO_ACTION: u8 = 0;
const CLEAR_LINE: u8 = 1;
const WARN_BEFORE_EXIT: u8 = 2;
const EXIT: u8 = 3;

#[derive(Default)]
struct PromptState {
    consecutive_empty_interrupts: AtomicU8,
    pending_action: AtomicU8,
}

impl PromptState {
    fn record_input(&self) {
        self.consecutive_empty_interrupts.store(0, Ordering::SeqCst);
    }

    fn record_interrupt(&self, line_is_empty: bool) {
        let action = if line_is_empty {
            let previous = self
                .consecutive_empty_interrupts
                .fetch_add(1, Ordering::SeqCst);
            if previous == 0 {
                WARN_BEFORE_EXIT
            } else {
                EXIT
            }
        } else {
            self.consecutive_empty_interrupts.store(0, Ordering::SeqCst);
            CLEAR_LINE
        };
        self.pending_action.store(action, Ordering::SeqCst);
    }

    fn take_action(&self) -> PromptInterruptAction {
        match self.pending_action.swap(NO_ACTION, Ordering::SeqCst) {
            WARN_BEFORE_EXIT => PromptInterruptAction::WarnBeforeExit,
            EXIT => PromptInterruptAction::Exit,
            _ => PromptInterruptAction::ClearLine,
        }
    }
}

struct CtrlCHandler {
    state: Arc<PromptState>,
}

impl ConditionalEventHandler for CtrlCHandler {
    fn handle(
        &self,
        _evt: &Event,
        _n: RepeatCount,
        _positive: bool,
        ctx: &EventContext,
    ) -> Option<Cmd> {
        self.state.record_interrupt(ctx.line().is_empty());
        Some(Cmd::Interrupt)
    }
}

struct EndHandler {
    state: Arc<PromptState>,
}

impl ConditionalEventHandler for EndHandler {
    fn handle(
        &self,
        _evt: &Event,
        _n: RepeatCount,
        _positive: bool,
        ctx: &EventContext,
    ) -> Option<Cmd> {
        self.state.record_input();
        if ctx.has_hint() && ctx.pos() == ctx.line().len() {
            Some(Cmd::CompleteHint)
        } else {
            None
        }
    }
}

struct InputActivityHandler {
    state: Arc<PromptState>,
}

impl ConditionalEventHandler for InputActivityHandler {
    fn handle(
        &self,
        _evt: &Event,
        _n: RepeatCount,
        _positive: bool,
        _ctx: &EventContext,
    ) -> Option<Cmd> {
        self.state.record_input();
        None
    }
}

struct ReplHelper {
    workspace_root: PathBuf,
    history_hinter: HistoryHinter,
    hints_use_color: bool,
}

impl ReplHelper {
    fn new(workspace_root: PathBuf, hints_use_color: bool) -> Self {
        Self {
            workspace_root,
            history_hinter: HistoryHinter::new(),
            hints_use_color,
        }
    }

    fn prefixed_candidates(
        start: usize,
        prefix: &str,
        values: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> (usize, Vec<Pair>) {
        let mut candidates = values
            .into_iter()
            .filter_map(|value| {
                let value = value.as_ref();
                value.starts_with(prefix).then(|| Pair {
                    display: value.to_string(),
                    replacement: value.to_string(),
                })
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| left.display.cmp(&right.display));
        (start, candidates)
    }

    fn complete_workspace_path(&self, start: usize, fragment: &str) -> (usize, Vec<Pair>) {
        if Path::new(fragment).is_absolute()
            || Path::new(fragment).components().any(|component| {
                matches!(
                    component,
                    Component::ParentDir | Component::RootDir | Component::Prefix(_)
                )
            })
        {
            return (start, Vec::new());
        }

        let fragment_path = Path::new(fragment);
        let (relative_dir, name_prefix) = if fragment.ends_with(['/', '\\']) {
            (fragment_path, "")
        } else {
            (
                fragment_path.parent().unwrap_or_else(|| Path::new("")),
                fragment_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or(""),
            )
        };
        let directory = self.workspace_root.join(relative_dir);
        let Ok(workspace) = self.workspace_root.canonicalize() else {
            return (start, Vec::new());
        };
        let Ok(canonical_directory) = directory.canonicalize() else {
            return (start, Vec::new());
        };
        if !canonical_directory.starts_with(&workspace) {
            return (start, Vec::new());
        }
        let Ok(entries) = std::fs::read_dir(directory) else {
            return (start, Vec::new());
        };

        let mut candidates = entries
            .filter_map(Result::ok)
            .filter_map(|entry| {
                if !entry.path().canonicalize().ok()?.starts_with(&workspace) {
                    return None;
                }
                let name = entry.file_name().into_string().ok()?;
                if !name.starts_with(name_prefix) {
                    return None;
                }
                let mut replacement = relative_dir.join(&name).to_string_lossy().into_owned();
                if std::path::MAIN_SEPARATOR != '/' {
                    replacement = replacement.replace(std::path::MAIN_SEPARATOR, "/");
                }
                if entry.file_type().ok()?.is_dir() {
                    replacement.push('/');
                }
                Some(Pair {
                    display: replacement.clone(),
                    replacement,
                })
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| left.display.cmp(&right.display));
        (start, candidates)
    }
}

impl Completer for ReplHelper {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Self::Candidate>)> {
        let before_cursor = &line[..pos];
        if let Some(marker) = before_cursor.rfind("$(cat ") {
            let start = marker + "$(cat ".len();
            if !before_cursor[start..].contains(')') {
                return Ok(self.complete_workspace_path(start, &before_cursor[start..]));
            }
        }

        let token_start = before_cursor
            .char_indices()
            .rev()
            .find_map(|(index, ch)| ch.is_whitespace().then_some(index + ch.len_utf8()))
            .unwrap_or(0);
        let token = &before_cursor[token_start..];
        if token_start == 0 {
            if token.starts_with('/') {
                return Ok(Self::prefixed_candidates(
                    0,
                    token,
                    SLASH_COMMANDS.iter().map(|command| command.name),
                ));
            }
            return Ok((0, Vec::new()));
        }

        let prior = before_cursor[..token_start].trim_end();
        let previous_token = prior.split_whitespace().next_back().unwrap_or("");
        if previous_token == "--profile" {
            return Ok(Self::prefixed_candidates(
                token_start,
                token,
                crate::planner::profile::profile_names(),
            ));
        }
        if token.starts_with('-') {
            return Ok(Self::prefixed_candidates(token_start, token, FLAGS));
        }

        let command = before_cursor.split_whitespace().next().unwrap_or("");
        let path_command = slash_command_spec(command).is_some_and(|spec| {
            matches!(
                spec.kind,
                SlashCommandKind::RunPlan
                    | SlashCommandKind::RunUltraPlan
                    | SlashCommandKind::Resume
            )
        });
        if token.is_empty() && slash_command_spec(command).is_some() {
            let (_, mut candidates) = Self::prefixed_candidates(token_start, token, FLAGS);
            if path_command {
                candidates.extend(self.complete_workspace_path(token_start, token).1);
                candidates.sort_by(|left, right| left.display.cmp(&right.display));
            }
            return Ok((token_start, candidates));
        }
        if path_command {
            let (start, fragment) = token
                .strip_prefix('"')
                .map_or((token_start, token), |fragment| (token_start + 1, fragment));
            return Ok(self.complete_workspace_path(start, fragment));
        }

        Ok((token_start, Vec::new()))
    }
}

impl Hinter for ReplHelper {
    type Hint = String;

    fn hint(&self, line: &str, pos: usize, ctx: &Context<'_>) -> Option<Self::Hint> {
        if line.is_empty() || pos != line.len() {
            return None;
        }
        if let Some(hint) = self.history_hinter.hint(line, pos, ctx) {
            return Some(hint);
        }
        SLASH_COMMANDS
            .iter()
            .map(|command| command.name)
            .find(|command| command.starts_with(line) && *command != line)
            .map(|command| command[pos..].to_string())
    }
}

impl Highlighter for ReplHelper {
    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> Cow<'l, str> {
        if !line.contains('\n') {
            return Cow::Borrowed(line);
        }
        let marker = if self.hints_use_color {
            "\x1b[2m... \x1b[0m"
        } else {
            CONTINUATION_PROMPT
        };
        Cow::Owned(line.replace('\n', &format!("\n{marker}")))
    }

    fn highlight_hint<'h>(&self, hint: &'h str) -> Cow<'h, str> {
        if self.hints_use_color {
            Cow::Owned(format!("\x1b[2m{hint}\x1b[0m"))
        } else {
            Cow::Borrowed(hint)
        }
    }

    fn highlight_char(&self, line: &str, _pos: usize, _forced: bool) -> bool {
        line.contains('\n')
    }
}

impl Validator for ReplHelper {
    fn validate(&self, ctx: &mut ValidationContext) -> rustyline::Result<ValidationResult> {
        if input_needs_continuation(ctx.input()) {
            Ok(ValidationResult::Incomplete)
        } else {
            Ok(ValidationResult::Valid(None))
        }
    }
}

impl Helper for ReplHelper {}

pub fn input_needs_continuation(input: &str) -> bool {
    let trailing_backslash = input.trim_end_matches([' ', '\t', '\r']).ends_with('\\');
    let unclosed_double_quote = input.chars().filter(|ch| *ch == '"').count() % 2 == 1;
    trailing_backslash || unclosed_double_quote
}

pub fn normalize_multiline_input(input: &str) -> String {
    input
        .split('\n')
        .map(|line| {
            let line = line.trim_end_matches('\r');
            let without_padding = line.trim_end_matches([' ', '\t']);
            without_padding
                .strip_suffix('\\')
                .map(str::trim_end)
                .unwrap_or(line)
                .to_string()
        })
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustyline::completion::Candidate;
    use rustyline::history::History;

    fn helper(root: &Path) -> ReplHelper {
        ReplHelper::new(root.to_path_buf(), false)
    }

    fn completions(root: &Path, line: &str) -> (usize, Vec<String>) {
        let history = DefaultHistory::new();
        let ctx = Context::new(&history);
        let (start, candidates) = helper(root).complete(line, line.len(), &ctx).unwrap();
        (
            start,
            candidates
                .iter()
                .map(|candidate| candidate.replacement().to_string())
                .collect(),
        )
    }

    #[test]
    fn command_completion_uses_all_fourteen_canonical_specs() {
        let dir = tempfile::tempdir().unwrap();
        let (start, candidates) = completions(dir.path(), "/");
        assert_eq!(start, 0);
        assert_eq!(candidates.len(), 14);
        let mut expected = SLASH_COMMANDS
            .iter()
            .map(|command| command.name.to_string())
            .collect::<Vec<_>>();
        expected.sort();
        assert_eq!(candidates, expected);
    }

    #[test]
    fn command_and_flag_completion_are_prefix_matched() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(completions(dir.path(), "/model-p").1, vec!["/model-probe"]);
        assert_eq!(
            completions(dir.path(), "/ultra-plan-run --p").1,
            vec!["--profile", "--prompt-layout"]
        );
        assert_eq!(
            completions(dir.path(), "/ultra-plan-run --s").1,
            vec!["--style"]
        );
        assert_eq!(
            completions(dir.path(), "/ultra-plan-run ").1,
            vec!["--profile", "--prompt-layout", "--style"]
        );
    }

    #[test]
    fn profile_completion_reads_the_domain_profile_registry() {
        let dir = tempfile::tempdir().unwrap();
        let (_, candidates) = completions(dir.path(), "/plan-run --profile ");
        let mut expected = crate::planner::profile::profile_names()
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();
        expected.sort();
        assert_eq!(candidates, expected);
    }

    #[test]
    fn path_completion_is_workspace_relative_for_commands_and_cat() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".anvil/plans")).unwrap();
        std::fs::create_dir_all(dir.path().join("docs")).unwrap();
        std::fs::write(dir.path().join(".anvil/plans/step.yaml"), "plan").unwrap();
        std::fs::write(dir.path().join("docs/goal.md"), "goal").unwrap();

        assert_eq!(
            completions(dir.path(), "/run-plan .anvil/plans/st").1,
            vec![".anvil/plans/step.yaml"]
        );
        assert_eq!(
            completions(dir.path(), "/run-ultra-plan .anvil/plans/st").1,
            vec![".anvil/plans/step.yaml"]
        );
        assert_eq!(
            completions(dir.path(), "/resume .anvil/plans/st").1,
            vec![".anvil/plans/step.yaml"]
        );
        assert_eq!(
            completions(dir.path(), "/run-plan \".anvil/plans/st").1,
            vec![".anvil/plans/step.yaml"]
        );
        assert_eq!(
            completions(dir.path(), "/ultra-plan-run \"$(cat docs/go").1,
            vec!["docs/goal.md"]
        );
        assert!(completions(dir.path(), "/run-plan ../").1.is_empty());
    }

    #[test]
    fn hints_prefer_history_then_fall_back_to_commands() {
        let dir = tempfile::tempdir().unwrap();
        let helper = helper(dir.path());
        let mut history = DefaultHistory::new();
        history.add("/run-plan saved.yaml").unwrap();
        let ctx = Context::new(&history);
        assert_eq!(
            helper.hint("/run", 4, &ctx).as_deref(),
            Some("-plan saved.yaml")
        );

        let empty_history = DefaultHistory::new();
        let ctx = Context::new(&empty_history);
        assert_eq!(helper.hint("/model", 6, &ctx).as_deref(), Some("-probe"));
    }

    #[test]
    fn no_color_hints_and_continuation_prompt_are_plain_ascii() {
        let dir = tempfile::tempdir().unwrap();
        let helper = helper(dir.path());
        assert_eq!(helper.highlight_hint("-plan"), "-plan");
        assert_eq!(helper.highlight("first\nsecond", 12), "first\n... second");
    }

    #[test]
    fn color_hint_uses_dim_sgr_only() {
        let dir = tempfile::tempdir().unwrap();
        let helper = ReplHelper::new(dir.path().to_path_buf(), true);
        assert_eq!(helper.highlight_hint("-plan"), "\x1b[2m-plan\x1b[0m");
    }

    #[test]
    fn validator_detects_trailing_backslash_and_unclosed_quote() {
        assert!(input_needs_continuation("goal \\"));
        assert!(input_needs_continuation("/plan-run \"unfinished"));
        assert!(!input_needs_continuation("/plan-run \"finished\""));
        assert!(!input_needs_continuation("goal"));
    }

    #[test]
    fn multiline_input_normalizes_for_existing_word_parser() {
        let normalized = normalize_multiline_input("/plan-run first \\\nsecond\nthird");
        assert_eq!(normalized, "/plan-run first second third");
        assert_eq!(
            crate::tui::slash::parse_words(&normalized),
            vec!["/plan-run", "first", "second", "third"]
        );
    }

    #[test]
    fn bracketed_paste_is_explicitly_enabled() {
        assert!(editor_config().enable_bracketed_paste());
        assert_eq!(editor_config().color_mode(), ColorMode::Enabled);
    }

    #[test]
    fn ctrl_c_requires_two_uninterrupted_empty_line_presses() {
        let state = PromptState::default();
        state.record_interrupt(true);
        assert_eq!(state.take_action(), PromptInterruptAction::WarnBeforeExit);
        state.record_input();
        state.record_interrupt(true);
        assert_eq!(state.take_action(), PromptInterruptAction::WarnBeforeExit);
        state.record_interrupt(true);
        assert_eq!(state.take_action(), PromptInterruptAction::Exit);

        state.record_interrupt(false);
        assert_eq!(state.take_action(), PromptInterruptAction::ClearLine);
        state.record_interrupt(true);
        assert_eq!(state.take_action(), PromptInterruptAction::WarnBeforeExit);
    }
}
