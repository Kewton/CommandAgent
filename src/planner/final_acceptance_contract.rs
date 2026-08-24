use std::path::Path;

use crate::planner::contract_attribute_repair::{
    self, ContractAttributeIssue, guidance_for_issue, issue_from_hook_status,
};
use crate::planner::profile::profile_evidence_repair_target_paths;
use crate::planner::state_binding_scan::final_acceptance_actionable_diagnosis;
use crate::planner::verify::VerificationReport;

pub(crate) fn issue_for_hook_status(
    root: &Path,
    profile: &str,
    report: &VerificationReport,
    status: &str,
) -> Option<ContractAttributeIssue> {
    let path = final_acceptance_actionable_diagnosis(root, profile, report)
        .map(|diagnosis| diagnosis.path)
        .or_else(|| {
            profile_evidence_repair_target_paths(
                root,
                profile,
                &["browser_interaction_failed".to_string()],
            )
            .into_iter()
            .next()
        })?;
    issue_from_hook_status(status, path)
}

pub(crate) fn guidance_for_hook_status(
    root: &Path,
    profile: &str,
    report: &VerificationReport,
    status: &str,
    eval_events_path: Option<&Path>,
) -> String {
    issues_for_context(root, profile, report, Some(status))
        .into_iter()
        .map(|issue| guidance_for_issue(Some(root), &issue, eval_events_path))
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub(crate) fn target_paths(
    root: &Path,
    profile: &str,
    report: &VerificationReport,
    hook_status: Option<&str>,
) -> Vec<String> {
    issues_for_context(root, profile, report, hook_status)
        .into_iter()
        .map(|issue| issue.path)
        .fold(Vec::new(), |mut paths, path| {
            if !paths.contains(&path) {
                paths.push(path);
            }
            paths
        })
}

fn issues_for_context(
    root: &Path,
    profile: &str,
    report: &VerificationReport,
    hook_status: Option<&str>,
) -> Vec<ContractAttributeIssue> {
    let mut issues = Vec::new();
    if let Some(issue) = contract_attribute_repair::detect(report) {
        push_unique(&mut issues, issue);
    }
    for (trigger, status) in [
        (
            "contract_instrumentation_missing:primary",
            "primary_missing",
        ),
        (
            "contract_instrumentation_missing:state_change",
            "state_missing",
        ),
        (
            "contract_instrumentation_missing:restart",
            "restart_missing",
        ),
    ] {
        if report_contains(report, trigger)
            && let Some(issue) = issue_for_hook_status(root, profile, report, status)
        {
            push_unique(&mut issues, issue);
        }
    }
    if let Some(issue) =
        hook_status.and_then(|status| issue_for_hook_status(root, profile, report, status))
    {
        push_unique(&mut issues, issue);
    }
    issues
}

fn report_contains(report: &VerificationReport, trigger: &str) -> bool {
    report
        .profile_failures
        .iter()
        .any(|failure| failure.contains(trigger))
        || report
            .command_failures
            .iter()
            .any(|failure| failure.command.contains(trigger) || failure.reason.contains(trigger))
}

fn push_unique(issues: &mut Vec<ContractAttributeIssue>, issue: ContractAttributeIssue) {
    if !issues.contains(&issue) {
        issues.push(issue);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn workspace() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src/app")).unwrap();
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"scripts":{"build":"next build"},"dependencies":{"next":"15.0.0"}}"#,
        )
        .unwrap();
        std::fs::write(
            dir.path().join("src/app/page.tsx"),
            r#"export default function Page(){ return <main data-anvil-state="{}"><button data-anvil-action="primary">Start</button><button>Restart</button></main>; }"#,
        )
        .unwrap();
        dir
    }

    #[test]
    fn instrumentation_reasons_map_to_all_missing_contract_attributes() {
        let dir = workspace();
        let report = VerificationReport::profile_failed(
            "release gate failed: contract_instrumentation_missing:primary; contract_instrumentation_missing:state_change; contract_instrumentation_missing:restart",
        );

        let attributes = issues_for_context(dir.path(), "nextjs", &report, Some("usable"))
            .into_iter()
            .map(|issue| issue.attribute)
            .collect::<Vec<_>>();

        assert_eq!(
            attributes,
            [
                r#"data-anvil-action="primary""#,
                "data-anvil-state",
                r#"data-anvil-action="restart""#,
            ]
        );
    }

    #[test]
    fn restart_instrumentation_reason_emits_final_repair_guidance() {
        let dir = workspace();
        let events = dir.path().join("events.jsonl");
        let report = VerificationReport::profile_failed(
            "interaction evidence status: failed:contract_instrumentation_missing:restart",
        );

        let guidance =
            guidance_for_hook_status(dir.path(), "nextjs", &report, "usable", Some(&events));

        assert!(guidance.contains(r#"missing attribute: `data-anvil-action="restart"`"#));
        assert!(guidance.contains("target source file: `src/app/page.tsx`"));
        assert!(guidance.contains("mark every restart, retry, or new-game affordance"));
        assert!(guidance.contains(r#"data-anvil-action="restart""#));
        assert_eq!(
            target_paths(dir.path(), "nextjs", &report, Some("usable")),
            ["src/app/page.tsx"]
        );
        let event = std::fs::read_to_string(events)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .find(|event| event["event"] == "contract_attribute_repair_guidance")
            .unwrap();
        assert_eq!(event["attribute"], r#"data-anvil-action="restart""#);
        assert_eq!(event["path"], "src/app/page.tsx");
    }
}
