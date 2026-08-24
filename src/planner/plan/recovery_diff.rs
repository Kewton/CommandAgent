use crate::planner::repair::RecoveryHandoff;

pub fn render_recovery_diff_comments(handoff: &RecoveryHandoff) -> String {
    let mut out =
        String::from("# Recovery diff summary (informational; these comments are not executed).\n");
    push_summary(&mut out, "retained changed paths", &handoff.changed_paths);
    push_summary(&mut out, "missing paths", &handoff.missing_paths);
    push_summary(
        &mut out,
        "missing capabilities",
        &handoff.missing_capabilities,
    );
    push_summary(&mut out, "repair targets", &handoff.repair_targets);
    push_summary(&mut out, "checks to rerun", &handoff.verify_commands);
    out
}

fn push_summary(out: &mut String, label: &str, values: &[String]) {
    out.push_str("# - ");
    out.push_str(label);
    out.push_str(": ");
    if values.is_empty() {
        out.push_str("none recorded\n");
        return;
    }
    let visible = values
        .iter()
        .take(8)
        .map(|value| bounded_single_line(value))
        .collect::<Vec<_>>()
        .join(", ");
    out.push_str(&visible);
    if values.len() > 8 {
        out.push_str(&format!(", +{} more", values.len() - 8));
    }
    out.push('\n');
}

fn bounded_single_line(value: &str) -> String {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut chars = normalized.chars();
    let visible = chars.by_ref().take(160).collect::<String>();
    if chars.next().is_some() {
        format!("{visible}…")
    } else if visible.is_empty() {
        "(empty)".to_string()
    } else {
        visible
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovery_diff_is_bounded_and_every_line_is_a_comment() {
        let handoff = RecoveryHandoff {
            changed_paths: vec!["src/app/page.tsx".to_string()],
            missing_paths: vec!["src/app/layout.tsx\nintent: injected".to_string()],
            missing_capabilities: vec!["interaction".to_string()],
            verify_commands: vec!["npm run build".to_string()],
            repair_targets: vec!["implementation".to_string()],
            ..RecoveryHandoff::default()
        };
        let rendered = render_recovery_diff_comments(&handoff);
        assert!(rendered.lines().all(|line| line.starts_with('#')));
        assert!(rendered.contains("retained changed paths: src/app/page.tsx"));
        assert!(rendered.contains("src/app/layout.tsx intent: injected"));
        assert!(rendered.contains("checks to rerun: npm run build"));
    }
}
