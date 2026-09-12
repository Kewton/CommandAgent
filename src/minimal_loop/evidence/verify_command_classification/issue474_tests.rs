use super::super::{SourceFile, collect_workspace_evidence, verify_command_kind};
use super::*;

fn fixture() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/corpus/apps/issue474-compound-node-hooks")
}

fn command() -> String {
    let commands: Vec<String> = serde_json::from_str(include_str!(
        "../../../../tests/corpus/apps/issue474-compound-node-hooks/fixtures/original-commands.json"
    ))
    .unwrap();
    commands[1].clone()
}

#[test]
fn issue474_saved_compound_command_is_structural_only() {
    let workspace = collect_workspace_evidence(&fixture());
    assert_eq!(
        verify_command_kind(&command(), &workspace),
        VerifyCommandKind::StaticSyntax
    );
    assert!(!super::super::has_bound_verify_command(
        &[command()],
        &workspace
    ));
}

#[test]
fn issue474_negative_corpus_cannot_borrow_unrelated_assertions() {
    let cases: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "../../../../tests/corpus/apps/issue474-compound-node-hooks/fixtures/negative-commands.json"
    ))
    .unwrap();
    let mut workspace = collect_workspace_evidence(&fixture());
    workspace.test_files.push(SourceFile::new(
        "tests/unrelated.test.js".into(),
        "const assert = require('assert'); assert(true);".into(),
    ));
    assert_eq!(
        verify_command_kind(&command(), &workspace),
        VerifyCommandKind::StaticSyntax
    );
    for case in cases {
        let command = case["command"].as_str().unwrap();
        assert!(
            !crate::planner::profiles::nextjs::recovery_authority::is_generated_hook_check(command),
            "{case}"
        );
        assert!(
            matches!(
                verify_command_kind(command, &workspace),
                VerifyCommandKind::Weak(_)
            ),
            "{case}"
        );
    }
    // Existing, genuine command-local Test evidence remains independent.
    assert_eq!(
        verify_command_kind("node --test", &workspace),
        VerifyCommandKind::Test
    );
    assert_eq!(
        verify_command_kind(
            "node -e \"const assert=require('assert');assert.equal(1+1,2)\"",
            &workspace
        ),
        VerifyCommandKind::Test
    );
}

#[test]
fn issue474_hook_checks_do_not_satisfy_business_requirements() {
    let contract: crate::minimal_loop::completion::CompletionContract =
        serde_json::from_value(serde_json::json!({
            "required_paths": ["src/app/page.tsx"],
            "verify_commands": [command()],
            "required_evidence": ["bound_verify_command", "user_input_handler_evidence"]
        }))
        .unwrap();
    let report = contract.runtime_acceptance_report(&fixture());
    assert!(!report.passed);
    assert!(report.weak_evidence.is_empty(), "{report:?}");
    assert!(
        report
            .missing_evidence
            .contains(&"bound_verify_command".into())
    );
    assert!(
        report
            .missing_evidence
            .contains(&"user_input_handler_evidence".into())
    );
}
