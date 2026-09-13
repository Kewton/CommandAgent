use super::super::*;
use super::issue474_acceptance_tests::tests::{PAGE, command, final_event, setup, write};
use crate::minimal_loop::build_verifier::FullCommandOutput;
use crate::planner::contract_attribute_repair;
use crate::planner::runner::PromptLayout;
use crate::planner::runner::final_acceptance::*;
use crate::planner::verify::VerificationReport;
use serde_json::Value;

fn raw(root: &Path, command: &str) -> crate::bounded_process::BoundedProcessOutput {
    crate::bounded_process::run_with_timeout(
        std::process::Command::new("sh")
            .args(["-c", command])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .current_dir(root),
        std::time::Duration::from_secs(5),
    )
    .unwrap()
}

fn prompt(
    root: &Path,
    plan: &UltraPlan,
    report: &VerificationReport,
    layout: PromptLayout,
) -> String {
    final_acceptance_repair_prompt(
        root,
        layout,
        plan,
        report,
        &UltraRunContext::default(),
        "verification_failed",
        &["src/app/page.tsx".into()],
        &[],
        (1, 2),
        false,
        false,
    )
}

#[test]
fn issue480_ordered_missing_unicode_long_source_and_checked_path_reach_prompts() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../tests/corpus/apps/issue480-node-failure/positive-cases.json"
    ))
    .unwrap();
    for case in cases {
        let root = tempfile::tempdir().unwrap();
        let path = case["path"].as_str().unwrap();
        let length = case["prefix_chars"].as_u64().unwrap() as usize;
        let mut command = command().replace("src/app/page.tsx", path);
        if case["swap_first_checks"] == true {
            let mut statements = command.split(';').collect::<Vec<_>>();
            statements.swap(2, 3);
            command = statements.join(";");
        }

        if case["misleading_error_label"] == true {
            command = command.replace("new Error('missing primary')", "new Error('missing state')");
        }
        if length > 0 {
            let comment = format!(
                "/*日本語🙂 readFileSync('src/app/api/decoy.ts') Error: missing state {}*/",
                "x".repeat(length)
            );
            command = command.replacen("const fs=", &format!("{comment}const fs="), 1);
        }
        let (config, plan) = setup(root.path(), &command);
        let mut page = PAGE.to_string();
        for remove in case["remove"].as_array().unwrap() {
            page = page.replace(remove.as_str().unwrap(), "data-removed");
        }
        write(root.path(), path, &page);
        write(root.path(), "src/app/api/decoy.ts", PAGE);
        let actual = raw(root.path(), &command);
        assert!(!actual.success());
        let report = ultra_final_acceptance_report(&plan, &config).unwrap();
        assert!(!report.is_pass());
        let issue = contract_attribute_repair::detect(&report)
            .unwrap_or_else(|| panic!("{case}: {report:?}"));
        assert_eq!(issue.attribute, case["expected_attribute"]);
        assert_eq!(issue.path, path);
        let expected_error = if case["misleading_error_label"] == true {
            "Error: missing state"
        } else if issue.attribute.contains("primary") {
            "Error: missing primary"
        } else {
            "Error: missing input"
        };
        assert!(String::from_utf8_lossy(&actual.stderr).contains(expected_error));
        assert!(
            report
                .command_failures
                .iter()
                .any(|f| f.reason.contains(expected_error))
        );
        for failure in &report.command_failures {
            assert!(
                failure.reason.chars().count() <= 521,
                "{}",
                failure.reason.len()
            );
        }
        for layout in [PromptLayout::Stable, PromptLayout::Legacy] {
            let prompt = prompt(root.path(), &plan, &report, layout);
            assert!(prompt.contains(expected_error));
            assert!(prompt.contains(&format!("missing attribute: `{}`", issue.attribute)));
            assert!(prompt.contains(&format!("target source file: `{path}`")));
            assert!(!prompt.contains("missing attribute: `data-anvil-state`"));
            assert!(!prompt.contains("target source file: `src/app/api/decoy.ts`"));
        }
        let step_prompt = crate::planner::repair::build_repair_prompt_with_context(
            "verify-hooks",
            &report,
            &crate::planner::repair::RepairContext::default(),
        );
        assert!(step_prompt.contains(&format!("missing attribute: `{}`", issue.attribute)));
        if length == 0
            && case["swap_first_checks"] != true
            && case["misleading_error_label"] != true
        {
            assert_eq!(
                final_event(&config)["command_diagnoses"][0]["kind"],
                "static_syntax"
            );
        }
    }
}

#[test]
fn issue480_unknown_printed_swallowed_import_and_test_failures_keep_generic_guidance() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../tests/corpus/apps/issue480-node-failure/negative-cases.json"
    ))
    .unwrap();
    for case in cases {
        let root = tempfile::tempdir().unwrap();
        let source = case["source"].as_str().unwrap();
        let command = format!(
            "node -e \"{}\"",
            source.replace('\\', "\\\\").replace('"', "\\\"")
        );
        let (config, plan) = setup(root.path(), &command);
        write(
            root.path(),
            "src/app/page.tsx",
            &PAGE.replace("data-anvil-state", "data-removed"),
        );
        write(root.path(), "src/components/Other.tsx", "<main />");
        write(root.path(), "src/lib/module.mjs", "export const value=1;");
        write(
            root.path(),
            "src/lib/broken.mjs",
            "throw new Error('missing state');",
        );
        let contract_before = std::fs::read(root.path().join("contract.json")).unwrap();
        let actual = raw(root.path(), &command);
        assert_eq!(
            actual.success(),
            case["execution_passes"].as_bool().unwrap(),
            "{case}"
        );
        let report = ultra_final_acceptance_report(&plan, &config).unwrap();
        assert!(!report.is_pass(), "{case}: {report:?}");
        assert!(
            contract_attribute_repair::detect(&report).is_none(),
            "{case}: {report:?}"
        );
        assert!(contract_attribute_repair_target_paths(root.path(), "generic", &report).is_empty());
        for layout in [PromptLayout::Stable, PromptLayout::Legacy] {
            let prompt = prompt(root.path(), &plan, &report, layout);
            assert!(
                !prompt.contains("Contract attribute repair guidance:"),
                "{case}: {prompt}"
            );
        }
        let kind = final_event(&config)["command_diagnoses"][0]["kind"].clone();
        if case["id"] == "namespace-literal" || case["id"] == "module-evaluation" {
            assert_eq!(kind, "static_syntax");
        } else if case["id"] == "genuine-test" {
            assert_eq!(kind, "test");
        }
        assert_eq!(
            std::fs::read(root.path().join("contract.json")).unwrap(),
            contract_before
        );
    }
}

#[test]
fn issue480_single_node_print_requires_observed_closed_predicate_failure() {
    let root = tempfile::tempdir().unwrap();
    let command = r#"node -p 'String(require("fs").readFileSync("src/app/page.tsx")).includes("data-anvil-state") ? true : process.exit(1)'"#;
    let (config, plan) = setup(root.path(), command);
    assert!(raw(root.path(), command).success());
    write(root.path(), "src/app/page.tsx", "<main />");
    let actual = raw(root.path(), command);
    assert!(!actual.success());
    assert!(actual.stderr.is_empty() && actual.stdout.is_empty());
    let report = ultra_final_acceptance_report(&plan, &config).unwrap();
    let issue = contract_attribute_repair::detect(&report).unwrap();
    assert_eq!(issue.attribute, "data-anvil-state");
    assert_eq!(issue.path, "src/app/page.tsx");
    let fabricated = VerificationReport::command_failed(command, "command failed");
    assert!(contract_attribute_repair::detect(&fabricated).is_none());
    for layout in [PromptLayout::Stable, PromptLayout::Legacy] {
        assert!(
            prompt(root.path(), &plan, &report, layout)
                .contains("missing attribute: `data-anvil-state`")
        );
        assert!(
            !prompt(root.path(), &plan, &fabricated, layout)
                .contains("Contract attribute repair guidance:")
        );
    }
    std::fs::remove_file(root.path().join("src/app/page.tsx")).unwrap();
    let report = ultra_final_acceptance_report(&plan, &config).unwrap();
    assert!(!report.is_pass());
    assert!(contract_attribute_repair::detect(&report).is_none());
}

#[test]
fn issue480_display_excerpt_is_distinct_and_redacted_location_loss_is_generic() {
    let root = tempfile::tempdir().unwrap();
    write(root.path(), "src/app/page.tsx", "<main />");
    let command = command();
    let output = crate::tools::bash::run_checked(&command, root.path(), false)
        .unwrap_err()
        .to_string();
    let full = FullCommandOutput::from_test_text(&output);
    assert!(!full.excerpt().as_str().contains("Error: missing primary"));
    assert!(
        full.failure_reason(&command)
            .contains("Error: missing primary")
    );
    let lost = output.replace("node_failure:", "redacted:");
    let lost = FullCommandOutput::from_test_text(&lost);
    let report = VerificationReport::command_failed(&command, lost.failure_reason(&command));
    assert!(contract_attribute_repair::detect(&report).is_none());
}
