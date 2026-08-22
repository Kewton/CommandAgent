use std::path::Path;

use super::summary_language::SummaryLanguage;
use super::terminal_report::{self, TerminalReport, VerificationObservation, VerificationStatus};

const MACHINE_DETAILS_HEADING: &str = "## Machine details";

pub(super) fn render(events_path: &Path, machine_body: &str) -> String {
    let events = terminal_report::read_events(Some(events_path));
    let report = terminal_report::project(&events, Some(events_path), None, None, None);
    render_document(
        machine_body,
        SummaryLanguage::from_process_locale(),
        &report,
    )
}

pub(super) fn machine_body(document: &str) -> String {
    if let Some((_, body)) = document.split_once(&format!("{MACHINE_DETAILS_HEADING}\n\n")) {
        return body.trim().to_string();
    }
    let build_line = crate::build_info::summary_line();
    document
        .strip_prefix(&build_line)
        .unwrap_or(document)
        .trim()
        .to_string()
}

fn render_document(
    machine_body: &str,
    language: SummaryLanguage,
    report: &TerminalReport,
) -> String {
    let status = report
        .status
        .map(|status| status.as_str().to_string())
        .or_else(|| machine_value(machine_body, &["Status:"]))
        .unwrap_or_else(|| language.unavailable().to_string());
    let assurance = report
        .assurance
        .clone()
        .or_else(|| machine_value(machine_body, &["Assurance:"]))
        .unwrap_or_else(|| language.unavailable().to_string());
    let gate = report
        .gate
        .clone()
        .or_else(|| machine_value(machine_body, &["Release gate:"]))
        .unwrap_or_else(|| language.unavailable().to_string());
    let stop_reason = report
        .stop_reason
        .clone()
        .or_else(|| machine_value(machine_body, &["Stop reason:"]))
        .unwrap_or_else(|| language.unavailable().to_string());
    let stop_reason = stop_reason.lines().next().unwrap_or(language.unavailable());
    let next_action = report
        .next_action
        .clone()
        .or_else(|| machine_value(machine_body, &["Next action:", "Recovery next action:"]))
        .unwrap_or_else(|| language.unavailable().to_string());
    let exit_code = report
        .exit_code
        .map(|code| code.to_string())
        .unwrap_or_else(|| language.unavailable().to_string());
    let verification_summary = verification_summary(language, &report.verifications);
    let mut lines = vec![
        crate::build_info::summary_line(),
        format!("# {}", language.heading()),
        String::new(),
        format!(
            "- {}: {}",
            language.result_label(),
            language.status(&status)
        ),
        format!("- {}: {assurance}", language.assurance_label()),
        format!("- {}: {}", language.gate_label(), language.gate(&gate)),
        format!(
            "- {}: {}",
            language.stop_reason_label(),
            language.stop_reason(stop_reason)
        ),
        format!(
            "- {}: {}",
            language.next_action_label(),
            language.next_action(&next_action)
        ),
        format!(
            "- {}: {}",
            language.changed_files_label(),
            report.changed_files.len()
        ),
        format!(
            "- {}: {verification_summary}",
            language.verification_label()
        ),
        format!("- {}: {exit_code}", language.exit_code_label()),
        String::new(),
        format!("## {}", language.changed_files_label()),
        String::new(),
    ];
    push_list(&mut lines, &report.changed_files, language.none());
    lines.extend([
        String::new(),
        format!("## {}", language.verification_commands_heading()),
        String::new(),
    ]);
    if report.verifications.is_empty() {
        lines.push(format!("- {}", language.none()));
    } else {
        lines.extend(report.verifications.iter().map(render_verification));
    }
    lines.extend([
        String::new(),
        MACHINE_DETAILS_HEADING.to_string(),
        String::new(),
    ]);
    if !machine_body.trim().is_empty() {
        lines.push(machine_body.trim().to_string());
    }
    lines.join("\n")
}

fn machine_value(body: &str, prefixes: &[&str]) -> Option<String> {
    body.lines().find_map(|line| {
        prefixes.iter().find_map(|prefix| {
            line.trim()
                .strip_prefix(prefix)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
        })
    })
}

fn verification_summary(
    language: SummaryLanguage,
    verifications: &[VerificationObservation],
) -> String {
    if verifications.is_empty() {
        return language.none().to_string();
    }
    let passed = count_status(verifications, VerificationStatus::Passed);
    let failed = count_status(verifications, VerificationStatus::Failed);
    let timed_out = count_status(verifications, VerificationStatus::TimedOut);
    let not_run = count_status(verifications, VerificationStatus::NotRun);
    let not_recorded = count_status(verifications, VerificationStatus::NotRecorded);
    let counts = match language {
        SummaryLanguage::English => [
            (passed, "passed"),
            (failed, "failed"),
            (timed_out, "timed out"),
            (not_run, "not run"),
            (not_recorded, "not recorded"),
        ],
        SummaryLanguage::Japanese => [
            (passed, "合格"),
            (failed, "不合格"),
            (timed_out, "タイムアウト"),
            (not_run, "未実行"),
            (not_recorded, "結果未記録"),
        ],
    };
    counts
        .into_iter()
        .filter(|(count, _)| *count > 0)
        .map(|(count, label)| format!("{count} {label}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn count_status(verifications: &[VerificationObservation], status: VerificationStatus) -> usize {
    verifications
        .iter()
        .filter(|observation| observation.status == status)
        .count()
}

fn render_verification(observation: &VerificationObservation) -> String {
    let exit = observation
        .exit_code
        .map(|code| format!(", exit={code}"))
        .unwrap_or_default();
    format!(
        "- `{}`: {}{}",
        observation.command,
        observation.status.as_str(),
        exit
    )
}

fn push_list(lines: &mut Vec<String>, values: &[String], empty: &str) {
    if values.is_empty() {
        lines.push(format!("- {empty}"));
    } else {
        lines.extend(values.iter().map(|value| format!("- `{value}`")));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval_events::terminal_report::{TerminalStatus, VerificationStatus};

    fn report() -> TerminalReport {
        TerminalReport {
            status: Some(TerminalStatus::Failed),
            assurance: Some("partial".to_string()),
            gate: Some("failed".to_string()),
            stop_reason: Some("tests failed\nPaths:\n- src/main.rs".to_string()),
            next_action: Some("fix_command_failure".to_string()),
            changed_files: vec!["src/main.rs".to_string()],
            verifications: vec![
                VerificationObservation {
                    command: "cargo test --test focused".to_string(),
                    status: VerificationStatus::Failed,
                    exit_code: Some(101),
                },
                VerificationObservation {
                    command: "cargo fmt --all -- --check".to_string(),
                    status: VerificationStatus::NotRecorded,
                    exit_code: None,
                },
            ],
            exit_code: Some(1),
        }
    }

    #[test]
    fn human_header_explains_next_action_within_first_ten_lines() {
        let rendered = render_document(
            "Status: failed\ncompletion_contract_verification_enabled=false",
            SummaryLanguage::English,
            &report(),
        );
        let first_ten = rendered.lines().take(10).collect::<Vec<_>>().join("\n");
        assert!(first_ten.contains("- Result: failed"), "{rendered}");
        assert!(
            first_ten.contains("- Stop reason: tests failed"),
            "{rendered}"
        );
        assert!(
            first_ten.contains("- Next action: fix_command_failure"),
            "{rendered}"
        );
        assert!(
            rendered.contains("## Machine details\n\nStatus: failed"),
            "{rendered}"
        );
        assert!(
            rendered.contains("`cargo test --test focused`: failed, exit=101"),
            "{rendered}"
        );
    }

    #[test]
    fn japanese_header_localizes_closed_values_and_keeps_machine_details() {
        let rendered = render_document(
            "Status: failed\nFailure kind: direct_cli_command_failed",
            SummaryLanguage::Japanese,
            &report(),
        );
        assert!(rendered.contains("# 実行結果"), "{rendered}");
        assert!(rendered.contains("- 結果: 失敗"), "{rendered}");
        assert!(
            rendered.contains("- 次の一手: コマンドの失敗を修正する"),
            "{rendered}"
        );
        assert!(
            rendered.contains("Failure kind: direct_cli_command_failed"),
            "{rendered}"
        );
    }

    #[test]
    fn machine_body_unwraps_new_and_legacy_documents() {
        let rendered = render_document("Status: completed", SummaryLanguage::English, &report());
        assert_eq!(machine_body(&rendered), "Status: completed");
        assert_eq!(
            machine_body(&format!(
                "{}\nStatus: running",
                crate::build_info::summary_line()
            )),
            "Status: running"
        );
    }
}
