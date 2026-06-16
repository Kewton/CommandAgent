use crate::session::store::SessionMessage;

pub fn render_recent_context(messages: &[SessionMessage], max_chars: usize) -> String {
    crate::session::compact::render_messages_for_summary(messages, max_chars)
}
