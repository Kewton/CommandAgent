use std::path::Path;

use super::summary_language::SummaryLanguage;
use super::terminal_report::{self, TerminalReport, VerificationObservation, VerificationStatus};

const MACHINE_DETAILS_HEADING: &str = "## Machine details";

const STATUS_PREFIXES: &[&str] = &["Status:"];
const NEXT_ACTION_PREFIXES: &[&str] = &["Next action:", "Recovery next action:"];

pub(super) fn render(
    events_path: &Path,
    machine_body: &str,
    fallback_next_action_unverified: Option<bool>,
) -> String {
    let events = terminal_report::read_events(Some(events_path));
    let report = terminal_report::project(&events, Some(events_path), None, None, None);
    render_document(
        machine_body,
        SummaryLanguage::from_process_locale(),
        &report,
        fallback_next_action_unverified,
    )
}

/// The body's newest `Next action:` value, as the renderer resolves it, and
/// whether it is the acceptance-unverified action. `None` when the body states
/// no next action. The caller passes the same body it displays (before masking),
/// so the decision and the display cannot diverge.
pub(super) fn body_next_action_unverified(body: &str) -> Option<bool> {
    machine_value_last(body, NEXT_ACTION_PREFIXES)
        .map(|next_action| next_action == super::ACCEPTANCE_UNVERIFIED_NEXT_ACTION)
}

/// Whether a previously rendered summary recorded the acceptance-unverified
/// result. That header line is fixed renderer text and is never masked, so it
/// still states the decision after the earlier machine body's free
/// `Next action:` value was masked on disk. An append that states no next action
/// of its own inherits this, which is the value the joined display shows.
pub(super) fn document_result_unverified(document: &str) -> bool {
    [SummaryLanguage::English, SummaryLanguage::Japanese]
        .into_iter()
        .any(|language| {
            let line = format!(
                "- {}: {}",
                language.result_label(),
                language.acceptance_unverified_result()
            );
            document.lines().any(|candidate| candidate.trim() == line)
        })
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
    fallback_next_action_unverified: Option<bool>,
) -> String {
    // The body fallback supplies a value only where the projection has none, so
    // the decision and the display are resolved from the same source per field.
    let body_status = machine_value_last(machine_body, STATUS_PREFIXES);
    let status_value = report
        .status
        .map(|status| status.as_str().to_string())
        .or_else(|| body_status.clone())
        .unwrap_or_else(|| language.unavailable().to_string());
    let status = status_value.as_str();
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
        .or_else(|| machine_value_last(machine_body, NEXT_ACTION_PREFIXES))
        .unwrap_or_else(|| language.unavailable().to_string());
    let exit_code = report
        .exit_code
        .map(|code| code.to_string())
        .unwrap_or_else(|| language.unavailable().to_string());
    // Resolve "acceptance unverified" from the same status and next action that
    // are displayed, using each field's own source: the projection when it has
    // the value, otherwise the body's newest original value.
    let next_action_unverified = report
        .next_action
        .as_ref()
        .map(|_| report.next_action_unverified)
        .or(fallback_next_action_unverified)
        .unwrap_or(false);
    let acceptance_unverified =
        status == terminal_report::TerminalStatus::Completed.as_str() && next_action_unverified;
    let result = if acceptance_unverified {
        language.acceptance_unverified_result().to_string()
    } else {
        language.status(status).into_owned()
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

/// The last value a prefix carries in the body. A summary accumulates bodies
/// separated by `---`, so the newest statement wins; the display and the
/// "acceptance unverified" decision both read this same value.
fn machine_value_last(body: &str, prefixes: &[&str]) -> Option<String> {
    body.lines().rev().find_map(|line| {
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
            next_action_unverified: false,
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
            None,
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
            None,
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

        let english = render_document("", SummaryLanguage::English, &report, None);
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

        let japanese = render_document("", SummaryLanguage::Japanese, &report, None);
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
        let rendered = render_document("", SummaryLanguage::Japanese, &report, None);
        assert!(rendered.contains("- 結果: 完了"), "{rendered}");
        assert!(rendered.contains("- 次の一手: 追加操作なし"), "{rendered}");
    }

    /// r6 blocker 2: with no terminal event, the status and next action come
    /// from the body fallback. The "acceptance unverified" decision must use
    /// the fallback's *original* values, not the events projection, and the
    /// displayed next action stays masked when it is registered. Through both
    /// writers.
    #[test]
    fn fallback_body_keeps_acceptance_unverified_result() {
        crate::eval_events::summary_language::with_language_for_test(
            SummaryLanguage::English,
            fallback_body_acceptance_unverified,
        );
    }

    fn fallback_body_acceptance_unverified() {
        let secret = crate::eval_events::ACCEPTANCE_UNVERIFIED_NEXT_ACTION;
        let body = format!("Status: completed\nNext action: {secret}");
        let root = tempfile::tempdir().unwrap();
        let path = root
            .path()
            .join(".commandagent/runs/fallback-acceptance/events.jsonl");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        // Only `run_start`: no terminal event, so the fallback supplies both.
        let start = serde_json::json!({"event": "run_start", "ok": true, "status": "running"});
        std::fs::write(&path, format!("{start}\n")).unwrap();

        let mut catalog = crate::sensitive_data::SecretCatalog::new();
        catalog.register(secret);
        crate::sensitive_data::install_scope(catalog, Some(root.path()), Some(&path));

        let check = |summary: &str, stage: &str| {
            assert!(
                summary.contains("- Result: acceptance unverified (process completed)"),
                "{stage}: decision lost: {summary}"
            );
            assert!(
                !summary.contains("- Result: completed"),
                "{stage}: {summary}"
            );
            assert!(!summary.contains(secret), "{stage}: {summary}");
            assert!(summary.contains("<redacted>"), "{stage}: {summary}");
        };

        crate::eval_events::write_run_summary(Some(&path), &body);
        let summary = std::fs::read_to_string(path.parent().unwrap().join("summary.md")).unwrap();
        check(&summary, "after write");

        crate::eval_events::append_run_summary(Some(&path), &body);
        let summary = std::fs::read_to_string(path.parent().unwrap().join("summary.md")).unwrap();
        check(&summary, "after append");

        // Only this test's scope is restored.
        crate::sensitive_data::set_current(None);
    }

    /// r7: the decision and the display must read the same body. Append renders
    /// the previous body plus this call's text, so the decision reads that
    /// joined body too. Three shapes, each with the value registered (masked on
    /// disk by the earlier write) and unregistered: a supplement appended after
    /// an acceptance-unverified body keeps the result; a failed body followed by
    /// an unverified body follows the joined body; and a body supplying the next
    /// action the events lack agrees with the display.
    #[test]
    fn append_decides_from_the_joined_body_it_displays() {
        crate::eval_events::summary_language::with_language_for_test(
            SummaryLanguage::English,
            append_decision_matches_display,
        );
    }

    fn append_decision_matches_display() {
        let secret = crate::eval_events::ACCEPTANCE_UNVERIFIED_NEXT_ACTION;
        // (write body, appended text, whether the value is registered)
        let cases: [(&str, &str, bool); 6] = [
            (
                "Status: completed\nNext action: run_acceptance_or_review_changes",
                "Follow-up detail only",
                false,
            ),
            (
                "Status: completed\nNext action: run_acceptance_or_review_changes",
                "Follow-up detail only",
                true,
            ),
            (
                "Status: failed\nStop reason: checks failed",
                "Status: completed\nNext action: run_acceptance_or_review_changes",
                false,
            ),
            (
                "Status: failed\nStop reason: checks failed",
                "Status: completed\nNext action: run_acceptance_or_review_changes",
                true,
            ),
            (
                "Status: completed",
                "Next action: run_acceptance_or_review_changes",
                false,
            ),
            (
                "Status: completed",
                "Next action: run_acceptance_or_review_changes",
                true,
            ),
        ];

        for (index, (write_body, appended, registered)) in cases.into_iter().enumerate() {
            let root = tempfile::tempdir().unwrap();
            let path = root
                .path()
                .join(format!(".commandagent/runs/joined-{index}/events.jsonl"));
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            // No terminal event: status and next action come from the body.
            let start = serde_json::json!({"event": "run_start", "ok": true, "status": "running"});
            std::fs::write(&path, format!("{start}\n")).unwrap();

            if registered {
                let mut catalog = crate::sensitive_data::SecretCatalog::new();
                catalog.register(secret);
                crate::sensitive_data::install_scope(catalog, Some(root.path()), Some(&path));
            }

            crate::eval_events::write_run_summary(Some(&path), write_body);
            crate::eval_events::append_run_summary(Some(&path), appended);
            let summary =
                std::fs::read_to_string(path.parent().unwrap().join("summary.md")).unwrap();

            assert!(
                summary.contains("- Result: acceptance unverified (process completed)"),
                "case {index} (registered={registered}): decision lost: {summary}"
            );
            assert!(
                !summary.contains("- Result: completed"),
                "case {index} (registered={registered}): {summary}"
            );
            // The display shows the same next action the decision used.
            let shown = if registered {
                "- Next action: <redacted>"
            } else {
                "- Next action: run_acceptance_or_review_changes"
            };
            assert!(
                summary.contains(shown),
                "case {index}: decision and display disagree: {summary}"
            );
            if registered {
                assert!(!summary.contains(secret), "case {index}: {summary}");
            }

            // Only this test's scope is restored.
            crate::sensitive_data::set_current(None);
        }
    }

    #[test]
    fn machine_body_unwraps_new_and_legacy_documents() {
        let rendered = render_document(
            "Status: completed",
            SummaryLanguage::English,
            &report(),
            None,
        );
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
