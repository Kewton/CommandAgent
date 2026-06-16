use crate::session::store::SessionMessage;

pub fn render_messages_for_summary(messages: &[SessionMessage], max_chars: usize) -> String {
    let mut rendered = String::new();
    for message in messages {
        let chunk = format!("{:?}: {}\n", message.role, message.content);
        if rendered.len() + chunk.len() > max_chars {
            break;
        }
        rendered.push_str(&chunk);
    }
    rendered
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::store::{SessionMessage, SessionRole};

    #[test]
    fn respects_char_limit() {
        let messages = vec![
            SessionMessage {
                role: SessionRole::User,
                content: "short".to_string(),
                name: None,
            },
            SessionMessage {
                role: SessionRole::Assistant,
                content: "very long message".to_string(),
                name: None,
            },
        ];

        let rendered = render_messages_for_summary(&messages, 20);

        assert!(rendered.contains("short"));
        assert!(!rendered.contains("very long"));
    }
}
