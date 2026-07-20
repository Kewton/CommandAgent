use std::path::Path;

use crate::tui::slash::ParsedSlash;

const MIN_WRAP_COLS: usize = 24;

pub fn render(
    accepted_line: &str,
    parsed: &ParsedSlash,
    events_path: Option<&Path>,
    cols: u16,
) -> String {
    let mut lines = vec!["Accepted command".to_string()];
    push_wrapped(
        &mut lines,
        "- Input: ",
        &sanitize_terminal_text(accepted_line),
        cols,
    );
    push_wrapped(
        &mut lines,
        "- Command: ",
        &sanitize_terminal_text(&parsed.command),
        cols,
    );
    push_wrapped(&mut lines, "- Goal: ", &display_or_none(&parsed.goal), cols);
    push_wrapped(
        &mut lines,
        "- Profile: ",
        &explicit_value(
            &parsed.profile,
            parsed.profile_explicit,
            parsed.profile_inference.is_some(),
        ),
        cols,
    );
    push_wrapped(
        &mut lines,
        "- Style: ",
        &explicit_value(&parsed.style, parsed.style_explicit, false),
        cols,
    );
    push_wrapped(
        &mut lines,
        "- Prompt layout: ",
        &explicit_value(
            parsed.prompt_layout.as_str(),
            parsed.prompt_layout_explicit,
            false,
        ),
        cols,
    );
    if let Some(port) = crate::planner::signals::requested_port_from_text(&parsed.goal) {
        push_wrapped(
            &mut lines,
            "- Requested port: ",
            &format!("{port} (goal)"),
            cols,
        );
    }
    if let Some(run_id) = run_id(events_path) {
        push_wrapped(&mut lines, "- Run ID: ", &run_id, cols);
    }
    lines.join("\n")
}

pub fn run_id(events_path: Option<&Path>) -> Option<String> {
    let run_dir = events_path?.parent()?;
    let is_run_dir = run_dir
        .parent()
        .and_then(Path::file_name)
        .and_then(|value| value.to_str())
        .is_some_and(|value| value == "runs");
    is_run_dir
        .then(|| run_dir.file_name()?.to_str().map(ToOwned::to_owned))
        .flatten()
}

pub fn sanitize_terminal_text(value: &str) -> String {
    crate::tui::markdown::sanitize(&value.replace(['\n', '\r'], " "))
}

fn display_or_none(value: &str) -> String {
    let value = sanitize_terminal_text(value);
    if value.trim().is_empty() {
        "(none)".to_string()
    } else {
        value
    }
}

fn explicit_value(value: &str, explicit: bool, inferred: bool) -> String {
    let suffix = if explicit {
        "explicit"
    } else if inferred {
        "inferred"
    } else {
        "effective"
    };
    format!("{} ({suffix})", sanitize_terminal_text(value))
}

fn push_wrapped(lines: &mut Vec<String>, prefix: &str, value: &str, cols: u16) {
    let max_width = usize::from(cols).max(MIN_WRAP_COLS);
    let continuation = " ".repeat(display_width(prefix));
    let mut line = prefix.to_string();
    let mut width = display_width(prefix);
    let mut has_value = false;
    for ch in value.chars() {
        let ch_width = display_char_width(ch);
        if has_value && width.saturating_add(ch_width) > max_width {
            lines.push(line);
            line = continuation.clone();
            width = display_width(&continuation);
        }
        line.push(ch);
        width = width.saturating_add(ch_width);
        has_value = true;
    }
    lines.push(line);
}

fn display_width(value: &str) -> usize {
    value.chars().map(display_char_width).sum()
}

fn display_char_width(ch: char) -> usize {
    let cp = ch as u32;
    if cp < 0x20 || (0x7f..=0x9f).contains(&cp) {
        0
    } else if matches!(
        cp,
        0x1100..=0x115f
            | 0x2329..=0x232a
            | 0x2e80..=0xa4cf
            | 0xac00..=0xd7a3
            | 0xf900..=0xfaff
            | 0xfe10..=0xfe19
            | 0xfe30..=0xfe6f
            | 0xff00..=0xff60
            | 0xffe0..=0xffe6
    ) {
        2
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PromptLayout;

    fn parsed(goal: &str) -> ParsedSlash {
        ParsedSlash {
            command: "/ultra-plan-run".to_string(),
            profile: "nextjs".to_string(),
            profile_explicit: true,
            profile_inference: None,
            prompt_layout: PromptLayout::Stable,
            prompt_layout_explicit: true,
            style: "compact".to_string(),
            style_explicit: true,
            goal: goal.to_string(),
        }
    }

    #[test]
    fn receipt_preserves_wrapped_cjk_goal_and_explicit_fields() {
        let goal =
            "あなたが考える最高に面白くかっこいいスペースインベーダーゲームを3011番ポートで作る";
        let receipt = render(
            &format!(
                "/ultra-plan-run --profile nextjs --style compact --prompt-layout stable {goal}"
            ),
            &parsed(goal),
            Some(Path::new(
                "/tmp/work/.anvil/runs/019f7fca-dc14-7241-bd51-36f0eba856ef/events.jsonl",
            )),
            32,
        );

        let joined = receipt.lines().map(str::trim_start).collect::<String>();
        assert!(joined.contains("- Command: /ultra-plan-run"), "{receipt}");
        assert!(joined.contains("- Profile: nextjs (explicit)"), "{receipt}");
        assert!(joined.contains("- Style: compact (explicit)"), "{receipt}");
        assert!(
            joined.contains("- Prompt layout: stable (explicit)"),
            "{receipt}"
        );
        assert!(
            joined.contains("- Requested port: 3011 (goal)"),
            "{receipt}"
        );
        assert!(
            joined.contains("- Run ID: 019f7fca-dc14-7241-bd51-36f0eba856ef"),
            "{receipt}"
        );
        assert!(joined.contains(goal), "{receipt}");
        assert!(receipt.lines().all(|line| display_width(line) <= 32));
    }

    #[test]
    fn receipt_neutralizes_controls_escape_and_bidi() {
        let goal = "safe\u{1b}[31m\u{202e}goal\nnext";
        let receipt = render(goal, &parsed(goal), None, 80);

        assert!(!receipt.contains('\u{1b}'), "{receipt:?}");
        assert!(!receipt.contains('\u{202e}'), "{receipt:?}");
        assert!(!receipt.contains("goal\nnext"), "{receipt:?}");
        assert!(receipt.contains("safe?[31m?goal next"), "{receipt:?}");
    }
}
