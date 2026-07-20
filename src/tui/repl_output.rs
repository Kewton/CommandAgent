use std::fmt;

use crate::eval_events::CompletionProjection;

#[derive(Debug)]
pub struct RenderedCommandError {
    markdown: String,
}

impl RenderedCommandError {
    pub fn new(markdown: String) -> Self {
        Self { markdown }
    }
}

impl fmt::Display for RenderedCommandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.markdown)
    }
}

impl std::error::Error for RenderedCommandError {}

pub fn render_plain_text_guidance(line: &str) -> String {
    format!(
        "Input was not run: {}\nUse /ultra-plan-run <goal> or /plan-run <goal>.",
        sanitize_inline(line)
    )
}

pub fn render_unknown_command(command: &str, candidates: &[&str]) -> String {
    let command = sanitize_inline(command);
    match nearest_command(&command, candidates) {
        Some(suggestion) => format!(
            "Unknown command: {command}\nDid you mean {suggestion}? Type /help for all commands."
        ),
        None => format!("Unknown command: {command}\nType /help for all commands."),
    }
}

pub fn render_interrupted(
    accepted_line: &str,
    stop_reason: &str,
    projection: &CompletionProjection,
) -> String {
    let mut lines = vec![
        "### INTERRUPTED".to_string(),
        format!("- Stop reason: {}", sanitize_inline(stop_reason)),
    ];
    if !projection.recovery_ultra_plan_path.is_empty() {
        lines.push(format!(
            "- Resume: /resume {}",
            sanitize_inline(&projection.recovery_ultra_plan_path)
        ));
    } else if !projection.suggested_recovery_yaml_command.is_empty() {
        lines.push(format!(
            "- Resume: {}",
            sanitize_inline(&projection.suggested_recovery_yaml_command)
        ));
    }
    lines.push(format!("- Rerun: {}", sanitize_inline(accepted_line)));
    lines.join("\n")
}

fn nearest_command<'a>(command: &str, candidates: &'a [&str]) -> Option<&'a str> {
    let max_distance = if command.chars().count() <= 6 { 2 } else { 3 };
    candidates
        .iter()
        .copied()
        .map(|candidate| (edit_distance(command, candidate), candidate))
        .filter(|(distance, _)| *distance <= max_distance)
        .min_by_key(|(distance, candidate)| (*distance, candidate.len()))
        .map(|(_, candidate)| candidate)
}

fn edit_distance(left: &str, right: &str) -> usize {
    let right = right.chars().collect::<Vec<_>>();
    let mut previous = (0..=right.len()).collect::<Vec<_>>();
    for (left_index, left_char) in left.chars().enumerate() {
        let mut current = Vec::with_capacity(right.len() + 1);
        current.push(left_index + 1);
        for (right_index, right_char) in right.iter().enumerate() {
            current.push(
                (current[right_index] + 1)
                    .min(previous[right_index + 1] + 1)
                    .min(previous[right_index] + usize::from(left_char != *right_char)),
            );
        }
        previous = current;
    }
    previous[right.len()]
}

fn sanitize_inline(value: &str) -> String {
    crate::tui::markdown::sanitize(&value.replace(['\n', '\r'], " "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typo_guidance_is_two_lines_and_suggests_help() {
        let output = render_unknown_command("/hepl", &["/help", "/plan", "/status"]);

        assert_eq!(output.lines().count(), 2, "{output}");
        assert!(output.contains("Did you mean /help?"), "{output}");
        assert!(!output.contains("TASK FAILED"), "{output}");
        assert!(!output.contains("Terminal summary"), "{output}");
        assert!(!output.contains("error:"), "{output}");
    }

    #[test]
    fn plain_text_guidance_sanitizes_echo_and_names_only_execution_entries() {
        let output = render_plain_text_guidance("日本語\u{1b}[31m\u{0085}\u{202e} goal\nnext");

        assert_eq!(output.lines().count(), 2, "{output:?}");
        assert!(output.contains("日本語?[31m?? goal next"), "{output:?}");
        assert!(output.contains("/ultra-plan-run <goal>"), "{output}");
        assert!(output.contains("/plan-run <goal>"), "{output}");
        assert!(!output.contains('\u{1b}'), "{output:?}");
        assert!(!output.contains('\u{0085}'), "{output:?}");
        assert!(!output.contains('\u{202e}'), "{output:?}");
    }

    #[test]
    fn interruption_is_one_distinct_block_with_resume_and_rerun() {
        let mut projection = crate::eval_events::project_completion(
            false,
            &crate::eval_events::CompletionSnapshot::empty(),
        );
        projection.recovery_ultra_plan_path =
            ".anvil/plans/recovery-ultra-plan-phase.yaml".to_string();

        let output = render_interrupted(
            "/run-ultra-plan plan.yaml",
            "interrupted by user",
            &projection,
        );

        assert_eq!(output.matches("INTERRUPTED").count(), 1, "{output}");
        assert!(!output.contains("TASK FAILED"), "{output}");
        assert!(
            output.contains("/resume .anvil/plans/recovery-ultra-plan-phase.yaml"),
            "{output}"
        );
        assert!(
            output.contains("Rerun: /run-ultra-plan plan.yaml"),
            "{output}"
        );
    }
}
