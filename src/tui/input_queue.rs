use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

pub const MAX_QUEUED_LINES: usize = 10;
pub const MAX_LINE_BYTES: usize = 4 * 1024;
pub const PREVIEW_WIDTH: usize = 40;

const LINE_LIMIT_NOTICE: &str = "input rejected: pending line limit is 4096 bytes";
const QUEUE_LIMIT_NOTICE: &str = "input rejected: queue limit is 10 lines";

#[derive(Debug, Clone, Default)]
pub struct InputQueue {
    state: Arc<Mutex<InputState>>,
}

#[derive(Debug, Default)]
struct InputState {
    enabled: bool,
    pending: String,
    queued: VecDeque<String>,
    notice: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputSnapshot {
    pub enabled: bool,
    pub pending: String,
    pub queued_count: usize,
    pub notice: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EditResult {
    Ignored,
    Updated,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SubmitResult {
    Ignored,
    Queued(String),
    Rejected,
}

impl InputQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_enabled(&self, enabled: bool) {
        let mut state = self.lock_state();
        state.enabled = enabled;
        if !enabled {
            state.pending.clear();
            state.queued.clear();
            state.notice = None;
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.lock_state().enabled
    }

    pub fn snapshot(&self) -> InputSnapshot {
        let state = self.lock_state();
        InputSnapshot {
            enabled: state.enabled,
            pending: state.pending.clone(),
            queued_count: state.queued.len(),
            notice: state.notice.clone(),
        }
    }

    pub fn take_queued(&self) -> Option<String> {
        let mut state = self.lock_state();
        if !state.enabled {
            return None;
        }
        let line = state.queued.pop_front()?;
        state.notice = None;
        Some(line)
    }

    pub fn take_pending(&self) -> Option<String> {
        let mut state = self.lock_state();
        if !state.enabled || state.pending.is_empty() {
            return None;
        }
        state.notice = None;
        Some(std::mem::take(&mut state.pending))
    }

    pub(crate) fn push_char(&self, ch: char) -> EditResult {
        if !is_printable(ch) {
            return EditResult::Ignored;
        }
        let mut state = self.lock_state();
        if !state.enabled {
            return EditResult::Ignored;
        }
        if state.pending.len() + ch.len_utf8() > MAX_LINE_BYTES {
            state.notice = Some(LINE_LIMIT_NOTICE.to_string());
            return EditResult::Rejected;
        }
        state.pending.push(ch);
        state.notice = None;
        EditResult::Updated
    }

    pub(crate) fn backspace(&self) -> EditResult {
        let mut state = self.lock_state();
        if !state.enabled || state.pending.pop().is_none() {
            return EditResult::Ignored;
        }
        state.notice = None;
        EditResult::Updated
    }

    pub(crate) fn clear_pending(&self) -> bool {
        let mut state = self.lock_state();
        if !state.enabled || state.pending.is_empty() {
            return false;
        }
        state.pending.clear();
        state.notice = Some("pending input cleared".to_string());
        true
    }

    pub(crate) fn submit(&self) -> SubmitResult {
        let mut state = self.lock_state();
        if !state.enabled || state.pending.is_empty() {
            return SubmitResult::Ignored;
        }
        if state.queued.len() >= MAX_QUEUED_LINES {
            state.notice = Some(QUEUE_LIMIT_NOTICE.to_string());
            return SubmitResult::Rejected;
        }
        let line = std::mem::take(&mut state.pending);
        let line_preview = preview(&line);
        state.queued.push_back(line);
        state.notice = Some(format!("queued: {line_preview}"));
        SubmitResult::Queued(line_preview)
    }

    fn lock_state(&self) -> std::sync::MutexGuard<'_, InputState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

pub fn preview(value: &str) -> String {
    crate::util::fit_display_width(value, PREVIEW_WIDTH, "…")
}

fn is_printable(ch: char) -> bool {
    !ch.is_control()
        && !matches!(
            ch as u32,
            0x061c | 0x200e..=0x200f | 0x202a..=0x202e | 0x2066..=0x2069
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enabled_queue() -> InputQueue {
        let queue = InputQueue::new();
        queue.set_enabled(true);
        queue
    }

    fn type_text(queue: &InputQueue, text: &str) {
        for ch in text.chars() {
            assert_eq!(queue.push_char(ch), EditResult::Updated);
        }
    }

    #[test]
    fn pending_input_supports_unicode_backspace_and_clear() {
        let queue = enabled_queue();
        type_text(&queue, "next 日本");
        assert_eq!(queue.backspace(), EditResult::Updated);
        assert_eq!(queue.snapshot().pending, "next 日");
        assert!(queue.clear_pending());
        assert_eq!(queue.snapshot().pending, "");
        assert_eq!(
            queue.snapshot().notice.as_deref(),
            Some("pending input cleared")
        );
    }

    #[test]
    fn submitted_lines_are_fifo_and_confirmation_clears_on_take() {
        let queue = enabled_queue();
        type_text(&queue, "first");
        assert_eq!(queue.submit(), SubmitResult::Queued("first".to_string()));
        type_text(&queue, "second");
        assert_eq!(queue.submit(), SubmitResult::Queued("second".to_string()));

        assert_eq!(queue.take_queued().as_deref(), Some("first"));
        assert_eq!(queue.snapshot().queued_count, 1);
        assert_eq!(queue.snapshot().notice, None);
        assert_eq!(queue.take_queued().as_deref(), Some("second"));
        assert_eq!(queue.take_queued(), None);
    }

    #[test]
    fn unfinished_pending_input_can_move_to_the_next_prompt() {
        let queue = enabled_queue();
        type_text(&queue, "partially typed");

        assert_eq!(queue.take_pending().as_deref(), Some("partially typed"));
        assert!(queue.snapshot().pending.is_empty());
        assert_eq!(queue.take_pending(), None);
    }

    #[test]
    fn pending_line_limit_counts_utf8_bytes_and_keeps_editable_text() {
        let queue = enabled_queue();
        for _ in 0..(MAX_LINE_BYTES / '日'.len_utf8()) {
            assert_eq!(queue.push_char('日'), EditResult::Updated);
        }
        assert_eq!(queue.push_char('日'), EditResult::Rejected);
        let snapshot = queue.snapshot();
        assert_eq!(snapshot.pending.len(), MAX_LINE_BYTES - 1);
        assert_eq!(snapshot.notice.as_deref(), Some(LINE_LIMIT_NOTICE));
        assert_eq!(queue.backspace(), EditResult::Updated);
        assert_eq!(queue.push_char('a'), EditResult::Updated);
    }

    #[test]
    fn queue_limit_rejects_submission_without_losing_pending_line() {
        let queue = enabled_queue();
        for index in 0..MAX_QUEUED_LINES {
            type_text(&queue, &format!("line {index}"));
            assert!(matches!(queue.submit(), SubmitResult::Queued(_)));
        }
        type_text(&queue, "one too many");
        assert_eq!(queue.submit(), SubmitResult::Rejected);
        let snapshot = queue.snapshot();
        assert_eq!(snapshot.pending, "one too many");
        assert_eq!(snapshot.queued_count, MAX_QUEUED_LINES);
        assert_eq!(snapshot.notice.as_deref(), Some(QUEUE_LIMIT_NOTICE));
    }

    #[test]
    fn disabled_queue_discards_edits_and_clears_memory() {
        let queue = enabled_queue();
        type_text(&queue, "temporary");
        assert!(matches!(queue.submit(), SubmitResult::Queued(_)));
        queue.set_enabled(false);

        assert_eq!(queue.push_char('x'), EditResult::Ignored);
        assert_eq!(queue.submit(), SubmitResult::Ignored);
        assert_eq!(queue.take_queued(), None);
        assert_eq!(
            queue.snapshot(),
            InputSnapshot {
                enabled: false,
                pending: String::new(),
                queued_count: 0,
                notice: None,
            }
        );
    }

    #[test]
    fn preview_is_unicode_safe_and_bounded_to_forty_columns() {
        let exact = "a".repeat(PREVIEW_WIDTH);
        assert_eq!(preview(&exact), exact);
        let long = format!("{exact}本");
        assert_eq!(preview(&long), format!("{exact}…"));

        let japanese = "日".repeat(PREVIEW_WIDTH / 2 + 1);
        assert_eq!(
            preview(&japanese),
            format!("{}…", "日".repeat(PREVIEW_WIDTH / 2))
        );
    }

    #[test]
    fn terminal_control_characters_are_not_accepted() {
        let queue = enabled_queue();
        for ch in ['\n', '\u{1b}', '\u{202e}'] {
            assert_eq!(queue.push_char(ch), EditResult::Ignored);
        }
        assert!(queue.snapshot().pending.is_empty());
    }
}
