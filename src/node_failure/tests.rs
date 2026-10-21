use super::*;
use crate::minimal_loop::build_verifier::FullCommandOutput;
use crate::planner::verify::VerificationReport;
use crate::tools::bash;
use std::path::Path;
use std::time::Duration;

pub(crate) fn failure_report(root: &Path, attribute: &str) -> VerificationReport {
    let literal = serde_json::to_string(attribute).unwrap();
    let command = format!(
        "node -e 'const fs=require(\"fs\");const s=fs.readFileSync(\"src/app/page.tsx\",\"utf8\");if(!s.includes({literal}))throw new Error(\"missing hook\")'"
    );
    let output = bash::run_checked(&command, root, false)
        .unwrap_err()
        .to_string();
    VerificationReport::command_failed(&command, output)
}

#[test]
fn issue480_both_executors_preserve_bounded_redacted_cause() {
    let root = tempfile::tempdir().unwrap();
    let command = "node -e 'throw new Error(\"failure sk-example-secret api_key=example \"+String.fromCharCode(47,85,115,101,114,115,47,101,120,97,109,112,108,101,47,112,114,105,118,97,116,101))'";
    let normalized: crate::planner::verify::NormalizedVerifyCommand =
        crate::planner::verify::normalize_verify_command(command).unwrap();
    let outcomes = [
        bash::run_structured(command, root.path(), false, Duration::from_secs(5), || {
            false
        })
        .unwrap(),
        crate::minimal_loop::verifier_env::run_structured_for_verify_with_profile(
            &normalized,
            root.path(),
            None,
            false,
        )
        .unwrap(),
    ];
    for outcome in outcomes {
        assert!(!outcome.is_success());
        assert!(outcome.summary.starts_with(PREFIX), "{}", outcome.summary);
        assert!(outcome.summary.contains("Error: failure"));
        assert!(outcome.summary.contains("<redacted>"));
        for secret in [
            "sk-example-secret",
            "api_key=example",
            "/Users/example/private",
        ] {
            assert!(!outcome.summary.contains(secret));
        }
        assert!(outcome.summary.len() <= RECORD_LIMIT);
        let formatted = crate::minimal_loop::verifier_env::format_verify_outcome(&outcome);
        let full =
            FullCommandOutput::from_test_text(&format!("command failed: {command}\n{formatted}"));
        let reason = full.failure_reason(command);
        assert!(reason.chars().count() <= 500);
        assert!(reason.contains("Error: failure"));
        assert!(attribute(command, &reason).is_none());
    }
}

#[test]
fn issue480_record_boundaries_and_ambiguous_diagnostics_refuse_attribution() {
    let command = "node -e 'throw new Error(\"unknown\")'";
    let stderr = "Error: unknown\n    at [eval]:1:7\n";
    let summary = summary(command, "", stderr).unwrap();
    for length in 0..summary.len() {
        assert!(record(command, &summary[..length]).is_none());
    }
    assert!(record(command, &summary).is_some());
    assert!(record("node -e 'throw new Error(\"other\")'", &summary).is_none());
    assert!(super::summary(command, "", &stderr.repeat(2)).is_none());
    assert!(super::summary(command, "", "Error: unknown\n").is_none());
    assert!(
        super::summary(
            command,
            "",
            &format!("Error: {}\n    at [eval]:1:7\n", "x".repeat(500))
        )
        .is_none()
    );
    assert!(observation(command, &format!("stdout:\n{summary}")).is_none());
    assert!(observation(command, &format!("command failed: '{summary}'")).is_none());
    assert!(attribute(command, &summary).is_none());
}

#[test]
fn issue480_command_literals_without_execution_evidence_stay_generic() {
    let command = r#"node -p 'String(require("fs").readFileSync("src/app/page.tsx")).includes("data-anvil-state") ? true : process.exit(1)'"#;
    let report = VerificationReport::command_failed(command, "command failed");
    assert!(crate::planner::contract_attribute_repair::detect(&report).is_none());
    assert!(
        crate::planner::contract_attribute_repair::guidance_section(None, &report, None).is_empty()
    );
}

#[test]
fn issue480_bash_private_output_redaction_cannot_create_attribute_authority() {
    let root = tempfile::tempdir().unwrap();
    let encoded = ".commandagent/private"
        .bytes()
        .map(|b| b.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let command = format!("node -e 'throw new Error(String.fromCharCode({encoded}))'");
    let outcome =
        bash::run_structured(&command, root.path(), false, Duration::from_secs(5), || {
            false
        })
        .unwrap();
    assert!(!outcome.is_success());
    assert!(!outcome.stderr.contains("Error: .commandagent/private"));
    assert!(!outcome.summary.starts_with(PREFIX));
    let output = crate::minimal_loop::verifier_env::format_verify_outcome(&outcome);
    assert!(attribute(&command, &output).is_none());
}
