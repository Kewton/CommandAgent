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
    // The decision reads the same values that are displayed, so it can never
    // disagree with them: both come from the joined, capped body and the events
    // projection, per field.
    let result = if status == terminal_report::TerminalStatus::Completed.as_str()
        && next_action == super::ACCEPTANCE_UNVERIFIED_NEXT_ACTION
    {
        language.acceptance_unverified_result().to_string()
    } else {
        language.status(&status).into_owned()
    };
    let verification_summary = verification_summary(language, &report.verifications);
    let mut lines = vec![
        crate::build_info::summary_line(),
        format!("# {}", language.heading()),
        String::new(),
        format!("- {}: {result}", language.result_label()),
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
    use serde_json::json;

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
    fn issue500_unverified_run_header_does_not_claim_completion_or_no_action() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "tests/corpus/apps/issue500-unverified-minimal-loop/fixtures/e2e-3b-events.jsonl",
        );
        let events = terminal_report::read_events(Some(&path));
        let report = terminal_report::project(&events, Some(&path), None, None, None);
        assert_eq!(report.exit_code, Some(0));

        let english = render_document("", SummaryLanguage::English, &report);
        let header = english.lines().take(10).collect::<Vec<_>>().join("\n");
        assert!(
            header.contains("- Result: acceptance unverified (process completed)"),
            "{english}"
        );
        assert!(
            header.contains("- Next action: run_acceptance_or_review_changes"),
            "{english}"
        );
        assert!(!header.contains("- Result: completed"), "{english}");
        assert!(!header.contains("- Next action: none"), "{english}");

        let japanese = render_document("", SummaryLanguage::Japanese, &report);
        let header = japanese.lines().take(10).collect::<Vec<_>>().join("\n");
        assert!(
            header.contains("- 結果: 受入未検証（処理は終了）"),
            "{japanese}"
        );
        assert!(
            header.contains("- 次の一手: 受入検査を実行するか、変更内容とテスト結果を確認する"),
            "{japanese}"
        );
        assert!(!header.contains("- 結果: 完了"), "{japanese}");
        assert!(!header.contains("追加操作なし"), "{japanese}");
        assert!(japanese.contains("- 終了コード: 0"), "{japanese}");
    }

    #[test]
    fn checked_completion_keeps_completed_result() {
        let mut report = report();
        report.status = Some(TerminalStatus::Completed);
        report.next_action = Some("none".to_string());
        let rendered = render_document("", SummaryLanguage::Japanese, &report);
        assert!(rendered.contains("- 結果: 完了"), "{rendered}");
        assert!(rendered.contains("- 次の一手: 追加操作なし"), "{rendered}");
    }

    /// r6/r7/r8: the acceptance-unverified decision reads the same joined,
    /// capped body the summary displays. Every shape below ends with the result
    /// agreeing with the next action actually shown.
    #[test]
    fn append_decides_from_the_body_it_displays() {
        crate::eval_events::summary_language::with_language_for_test(
            SummaryLanguage::English,
            append_decision_matches_display,
        );
    }

    fn append_decision_matches_display() {
        let unverified = crate::eval_events::ACCEPTANCE_UNVERIFIED_NEXT_ACTION;

        // r6: no terminal event, so the body supplies both values.
        let no_terminal_event = |body: &str, appended: &str| {
            let summary = append_pair(
                json!({"event": "run_start", "ok": true, "status": "running"}),
                body,
                appended,
            );
            assert!(
                summary.contains("- Result: acceptance unverified (process completed)"),
                "{summary}"
            );
            assert!(
                summary.contains(&format!("- Next action: {unverified}")),
                "decision and display disagree: {summary}"
            );
        };

        // r8 shape 1: the appended action word is dropped by the 8,000-character
        // cap, so it must not change the decision.
        let dropped_by_cap = format!(
            "Status: completed\nNext action: none\n{}",
            "x".repeat(8_001)
        );
        let summary = append_pair(
            json!({"event": "run_start", "ok": true, "status": "running"}),
            &dropped_by_cap,
            &format!("Next action: {unverified}"),
        );
        assert!(
            summary.contains("- Result: completed"),
            "a value the display dropped changed the decision: {summary}"
        );
        assert!(
            !summary.contains("- Result: acceptance unverified"),
            "{summary}"
        );

        // r8 shape 2: an example line in the body is not a decision.
        let summary = append_pair(
            json!({"event": "run_start", "ok": true, "status": "running"}),
            "Status: completed\nExample:\n- Result: acceptance unverified (process completed)",
            "Follow-up detail only",
        );
        assert!(
            summary.contains("- Result: completed"),
            "an example line decided the result: {summary}"
        );

        // r8 shape 3: a failed body then a completed one. Result and the
        // displayed next action come from the same joined body, so the result is
        // exactly what the displayed status and action imply. Both with the
        // value registered and without.
        for registered in [false, true] {
            let summary = append_pair_registered(
                json!({"event": "run_start", "ok": true, "status": "running"}),
                &format!("Status: failed\nNext action: {unverified}"),
                "Status: completed",
                registered,
            );
            assert_result_follows_display(&summary, registered);
        }

        // r8 shape 4: after an unverified body, appending only a completed
        // status keeps the displayed action, so the result is completed too.
        for registered in [false, true] {
            let summary = append_pair_registered(
                json!({"event": "run_start", "ok": true, "status": "running"}),
                &format!("Status: completed\nNext action: {unverified}"),
                "Status: completed",
                registered,
            );
            assert_result_follows_display(&summary, registered);
        }

        // r6: a supplement after an unverified body keeps the result.
        no_terminal_event(
            &format!("Status: completed\nNext action: {unverified}"),
            "Follow-up detail only",
        );

        // Only this test's scope is restored. The body is unregistered
        // throughout, so the display cannot hide the value it decided on.
        crate::sensitive_data::set_current(None);
    }

    /// Write `body`, then append `appended`, with no registered scope, and
    /// return the rendered `summary.md`. Each call gets its own tempdir.
    fn append_pair(event: serde_json::Value, body: &str, appended: &str) -> String {
        append_pair_registered(event, body, appended, false)
    }

    /// Like [`append_pair`], optionally registering the acceptance-unverified
    /// action in this thread's scope. Registration refuses it (it is a fixed
    /// identifier), so the scope stays empty either way.
    fn append_pair_registered(
        event: serde_json::Value,
        body: &str,
        appended: &str,
        registered: bool,
    ) -> String {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join(".commandagent/runs/pair/events.jsonl");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, format!("{event}\n")).unwrap();
        if registered {
            let mut catalog = crate::sensitive_data::SecretCatalog::new();
            assert!(
                catalog
                    .register_credential(
                        "SYNTHETIC_CLOSED_VALUE",
                        crate::eval_events::ACCEPTANCE_UNVERIFIED_NEXT_ACTION
                    )
                    .is_err(),
                "a closed action word must not be registrable"
            );
            crate::sensitive_data::install_scope(catalog, Some(root.path()), Some(&path));
        }
        crate::eval_events::write_run_summary(Some(&path), body);
        crate::eval_events::append_run_summary(Some(&path), appended);
        std::fs::read_to_string(path.parent().unwrap().join("summary.md")).unwrap()
    }

    /// The result must be exactly what the displayed status and next action
    /// imply: "acceptance unverified" only when the shown status is completed
    /// and the shown action is the unverified one.
    fn assert_result_follows_display(summary: &str, registered: bool) {
        let unverified = crate::eval_events::ACCEPTANCE_UNVERIFIED_NEXT_ACTION;
        let shown_action = summary
            .lines()
            .find(|line| line.starts_with("- Next action: "))
            .unwrap_or_default();
        let shown_status_completed = summary
            .lines()
            .find(|line| line.starts_with("Status: "))
            .is_some_and(|line| line.trim() == "Status: completed");
        let expects_unverified = shown_status_completed && shown_action.ends_with(unverified);
        assert_eq!(
            summary.contains("- Result: acceptance unverified (process completed)"),
            expects_unverified,
            "registered={registered}: result and display disagree: {summary}"
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
