//! One-line TUI projection of the `bash_sandbox_status` event (Issue #803).
//!
//! Only a sandbox that is not applied (and not switched off) is shown. The
//! sentence is fixed: the event's `reason` can carry paths, so it is never echoed.

use serde_json::Value;

/// The scrollback line for a `bash_sandbox_status` event, or `None` when the
/// sandbox is applied, switched off, or the event is malformed.
pub fn render_line(event: &Value) -> Option<String> {
    if event.get("applied").and_then(Value::as_bool)? {
        return None;
    }
    let mode = event.get("mode").and_then(Value::as_str)?;
    if mode == "off" {
        return None;
    }
    let backend = event
        .get("backend")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .unwrap_or("none");
    let backend: String = backend
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        .take(24)
        .collect();
    let outcome = if mode == "required" {
        "commands are refused"
    } else {
        "continuing with lexical checks only"
    };
    Some(format!(
        "⚠ sandbox not applied (mode={mode}, backend={backend}): {outcome}"
    ))
}

#[cfg(test)]
mod tests {
    use super::render_line;
    use crate::tui::presentation::render_activity_line;
    use serde_json::json;

    fn event(mode: &str, applied: bool, reason: &str) -> serde_json::Value {
        json!({
            "event": "bash_sandbox_status",
            "mode": mode,
            "backend": "landlock",
            "applied": applied,
            "reason": reason,
        })
    }

    #[test]
    fn applied_status_renders_nothing() {
        assert_eq!(render_line(&event("auto", true, "")), None);
        assert_eq!(render_activity_line(&event("auto", true, "")), None);
    }

    #[test]
    fn off_mode_renders_nothing() {
        assert_eq!(render_activity_line(&event("off", false, "")), None);
    }

    #[test]
    fn auto_downgrade_is_one_line() {
        let line = render_activity_line(&event("auto", false, "abi too old")).unwrap();
        assert!(!line.contains('\n'));
        assert!(line.contains("not applied"));
        assert!(line.contains("landlock"));
        assert!(!line.contains("refused"));
    }

    #[test]
    fn required_downgrade_says_refused() {
        let line = render_activity_line(&event("required", false, "x")).unwrap();
        assert!(!line.contains('\n'));
        assert!(line.contains("refused"));
    }

    #[test]
    fn reason_with_home_path_is_not_echoed() {
        let reason = "cannot read /Users/someone/.ssh/secret and /home/someone/token";
        let line = render_activity_line(&event("auto", false, reason)).unwrap();
        assert!(!line.contains("/Users"));
        assert!(!line.contains("/home"));
        assert!(!line.contains("secret"));
    }

    #[test]
    fn cases_follow_the_corpus_table() {
        let corpus: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/corpus/apps/issue803-sandbox-status-projection/fixtures/cases.json"
        )))
        .unwrap();
        for case in corpus["cases"].as_array().unwrap() {
            let id = case["id"].as_str().unwrap();
            let status = event(
                case["mode"].as_str().unwrap(),
                case["applied"].as_bool().unwrap(),
                "r",
            );
            let line = render_activity_line(&status);
            assert_eq!(
                line.is_some() as u64,
                case["lines"].as_u64().unwrap(),
                "{id}"
            );
            for want in case["contains"].as_array().unwrap() {
                assert!(
                    line.as_deref().unwrap().contains(want.as_str().unwrap()),
                    "{id}"
                );
            }
        }
    }
}
