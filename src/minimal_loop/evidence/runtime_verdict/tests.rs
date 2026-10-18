//! Pins the runtime acceptance verdict through the existing public entrypoints.

use std::path::Path;

use crate::minimal_loop::evidence::{
    RuntimeAcceptanceReport, verify_runtime_acceptance, verify_runtime_acceptance_with_hints,
};

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| item.to_string()).collect()
}

fn write(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

fn run(
    root: &Path,
    paths: &[&str],
    verify: &[&str],
    capabilities: &[&str],
    evidence: &[&str],
    obligations: &[&str],
    deferred: &[&str],
) -> RuntimeAcceptanceReport {
    verify_runtime_acceptance(
        root,
        &strings(paths),
        &strings(verify),
        &strings(capabilities),
        &strings(evidence),
        &strings(obligations),
        &strings(deferred),
    )
}

fn tiers(report: &RuntimeAcceptanceReport) -> Vec<(&str, &str)> {
    report
        .evidence_tiers
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect()
}

fn rust_project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "Cargo.toml",
        "[package]\nname=\"x\"\nversion=\"0.1.0\"\n",
    );
    write(
        dir.path(),
        "src/main.rs",
        "fn main(){}\n#[cfg(test)] mod tests { #[test] fn t(){ assert_eq!(2,2); } }\n",
    );
    write(
        dir.path(),
        "tests/it.rs",
        "#[test]\nfn it(){ assert_eq!(1,1); }\n",
    );
    dir
}

#[test]
fn no_requirements_passes_with_default_report() {
    let dir = tempfile::tempdir().unwrap();
    let report = run(dir.path(), &[], &[], &[], &[], &[], &[]);
    assert!(report.passed);
    assert!(!report.inconclusive);
    assert_eq!(report.primary_reason, "pass");
    assert!(report.missing_capabilities.is_empty());
    assert!(report.missing_evidence.is_empty());
    assert!(report.missing_obligations.is_empty());
    assert!(report.evidence_tiers.is_empty());
    assert!(report.capability_evidence_bindings.is_empty());
}

#[test]
fn unsupported_capability_and_evidence_are_never_filled_in_as_success() {
    let dir = tempfile::tempdir().unwrap();
    let report = run(
        dir.path(),
        &[],
        &[],
        &["bogus_cap"],
        &["bogus_ev", "  "],
        &[],
        &[],
    );
    assert!(!report.passed);
    assert!(!report.inconclusive);
    assert_eq!(
        report.missing_capabilities,
        ["unsupported_required_capability:bogus_cap"]
    );
    assert_eq!(
        report.missing_evidence,
        ["unsupported_required_evidence:bogus_ev"]
    );
    assert_eq!(tiers(&report), [("bogus_ev", "absent")]);
    assert_eq!(
        report.primary_reason,
        "missing_required_capabilities:unsupported_required_capability:bogus_cap"
    );
}

#[test]
fn empty_workspace_reports_missing_test_evidence_in_set_order() {
    let dir = tempfile::tempdir().unwrap();
    let report = run(
        dir.path(),
        &["src/main.rs"],
        &[],
        &["deterministic_test"],
        &[],
        &[],
        &[],
    );
    assert!(!report.passed);
    assert_eq!(
        report.missing_evidence,
        ["bound_verify_command", "test_artifact"]
    );
    assert_eq!(
        tiers(&report),
        [
            ("bound_verify_command", "absent"),
            ("test_artifact", "absent")
        ]
    );
    assert_eq!(report.capability_evidence_bindings.len(), 1);
    let binding = &report.capability_evidence_bindings[0];
    assert_eq!(binding.capability, "deterministic_test");
    assert_eq!(
        binding.required_evidence,
        ["test_artifact", "bound_verify_command"]
    );
    assert!(binding.satisfied_evidence.is_empty());
    assert_eq!(
        binding.missing_evidence,
        ["test_artifact", "bound_verify_command"]
    );
    assert_eq!(
        report.primary_reason,
        "missing_required_evidence:bound_verify_command"
    );
}

#[test]
fn partial_evidence_without_bound_verify_command_does_not_pass() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "tests/it.rs",
        "#[test]\nfn it(){ assert_eq!(1,1); }\n",
    );
    let report = run(
        dir.path(),
        &[],
        &["echo ok"],
        &["deterministic_test"],
        &[],
        &[],
        &[],
    );
    assert!(!report.passed);
    assert_eq!(report.missing_evidence, ["bound_verify_command"]);
    assert_eq!(
        tiers(&report),
        [
            ("bound_verify_command", "absent"),
            ("test_artifact", "strong")
        ]
    );
    assert_eq!(
        report.primary_reason,
        "missing_required_evidence:bound_verify_command"
    );
}

#[test]
fn complete_test_evidence_passes_with_strong_tiers() {
    let dir = rust_project();
    let report = run(
        dir.path(),
        &["src/main.rs"],
        &["cargo test"],
        &["deterministic_test"],
        &[],
        &[],
        &[],
    );
    assert!(report.passed);
    assert!(report.missing_evidence.is_empty());
    assert_eq!(
        tiers(&report),
        [
            ("bound_verify_command", "strong"),
            ("test_artifact", "strong")
        ]
    );
    assert_eq!(report.primary_reason, "pass");
    assert_eq!(report.artifact_obligations.len(), 1);
    assert_eq!(report.artifact_obligations[0].path, "src/main.rs");
    assert_eq!(report.artifact_obligations[0].role, "implementation");
    assert_eq!(
        report.artifact_obligations[0].evidence,
        [
            "implementation_artifact",
            "non_zero_test_or_assertion_evidence",
            "test_artifact"
        ]
    );
    let binding = &report.capability_evidence_bindings[0];
    assert_eq!(
        binding.satisfied_evidence,
        ["test_artifact", "bound_verify_command"]
    );
    assert!(binding.missing_evidence.is_empty());
    assert_eq!(binding.artifact_paths, ["src/main.rs"]);
}

#[test]
fn implementation_obligation_passes_with_source_and_fails_without_it() {
    let dir = rust_project();
    let ok = run(
        dir.path(),
        &["src/main.rs"],
        &[],
        &["implementation"],
        &[],
        &["implementation"],
        &[],
    );
    assert!(ok.passed);
    assert!(ok.missing_obligations.is_empty());
    assert!(ok.obligation_repair_targets.is_empty());
    assert_eq!(tiers(&ok), [("implementation_artifact", "strong")]);

    let empty = tempfile::tempdir().unwrap();
    let missing = run(
        empty.path(),
        &["src/main.rs"],
        &[],
        &["implementation"],
        &[],
        &["implementation"],
        &[],
    );
    assert!(!missing.passed);
    assert_eq!(missing.missing_evidence, ["implementation_artifact"]);
    assert_eq!(missing.missing_obligations, ["implementation"]);
    assert_eq!(tiers(&missing), [("implementation_artifact", "absent")]);
    assert_eq!(missing.obligation_repair_targets.len(), 1);
    let target = &missing.obligation_repair_targets[0];
    assert_eq!(target.obligation, "implementation");
    assert_eq!(target.target_role, "implementation");
    assert_eq!(target.target_path, "src/main.rs");
    assert_eq!(
        target.reason,
        "missing implementation obligation; replace setup/scaffold/style/docs-only output with executable source"
    );
    assert_eq!(
        missing.primary_reason,
        "missing_required_evidence:implementation_artifact"
    );
}

#[test]
fn scaffold_only_output_is_weak_and_not_accepted_as_implementation() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "src/main.rs", "\n");
    write(dir.path(), "README.md", "# hi\n");
    let report = run(
        dir.path(),
        &["src/main.rs", "README.md"],
        &[],
        &["implementation"],
        &[],
        &["implementation"],
        &[],
    );
    assert!(!report.passed);
    assert_eq!(report.missing_evidence, ["implementation_artifact"]);
    assert_eq!(report.missing_obligations, ["implementation"]);
    assert_eq!(
        report.weak_evidence,
        [
            "non_implementation_obligation_only:acceptance_evidence",
            "non_implementation_obligation_only:scaffold"
        ]
    );
    assert_eq!(
        report.primary_reason,
        "missing_required_evidence:implementation_artifact"
    );
}

#[test]
fn investigation_binding_without_diagnosis_records_diagnostic() {
    let dir = tempfile::tempdir().unwrap();
    let report = run(
        dir.path(),
        &[],
        &[],
        &[],
        &["investigation_binding"],
        &[],
        &[],
    );
    assert!(!report.passed);
    assert_eq!(report.missing_evidence, ["investigation_binding"]);
    assert_eq!(
        report.diagnostics,
        ["investigation_binding:diagnosis_not_written"]
    );
    assert_eq!(tiers(&report), [("investigation_binding", "absent")]);
    assert_eq!(
        report.primary_reason,
        "missing_required_evidence:investigation_binding"
    );
}

#[test]
fn requested_content_needs_a_readme_or_markdown_path() {
    let dir = tempfile::tempdir().unwrap();
    let with_md = run(
        dir.path(),
        &["NOTES.md"],
        &[],
        &[],
        &["requested_content_evidence"],
        &[],
        &[],
    );
    assert!(with_md.passed);
    assert_eq!(tiers(&with_md), [("requested_content_evidence", "strong")]);

    let without = run(
        dir.path(),
        &["notes.txt"],
        &[],
        &[],
        &["requested_content_evidence"],
        &[],
        &[],
    );
    assert!(!without.passed);
    assert_eq!(without.missing_evidence, ["requested_content_evidence"]);
    assert_eq!(tiers(&without), [("requested_content_evidence", "absent")]);
}

#[test]
fn buildable_accepts_a_deferred_build_requirement() {
    let dir = tempfile::tempdir().unwrap();
    let report = run(
        dir.path(),
        &[],
        &[],
        &["buildable"],
        &[],
        &[],
        &["cargo build"],
    );
    assert!(report.passed);
    assert_eq!(
        tiers(&report),
        [("build_command_or_dependency_missing_boundary", "strong")]
    );
    let none = run(dir.path(), &[], &[], &["buildable"], &[], &[], &[]);
    assert!(!none.passed);
    assert_eq!(
        none.missing_evidence,
        ["build_command_or_dependency_missing_boundary"]
    );
}

#[test]
fn capability_missing_outranks_evidence_and_obligation_in_primary_reason() {
    let dir = tempfile::tempdir().unwrap();
    let report = run(
        dir.path(),
        &["src/main.rs"],
        &[],
        &["bogus_cap", "implementation"],
        &["bogus_ev"],
        &["implementation"],
        &[],
    );
    assert!(!report.passed);
    assert_eq!(
        report.missing_capabilities,
        ["unsupported_required_capability:bogus_cap"]
    );
    assert_eq!(
        report.missing_evidence,
        [
            "unsupported_required_evidence:bogus_ev",
            "implementation_artifact"
        ]
    );
    assert_eq!(report.missing_obligations, ["implementation"]);
    assert_eq!(
        tiers(&report),
        [
            ("bogus_ev", "absent"),
            ("implementation_artifact", "absent")
        ]
    );
    assert_eq!(
        report.primary_reason,
        "missing_required_capabilities:unsupported_required_capability:bogus_cap"
    );
}

#[test]
fn interactive_capability_reports_missing_evidence_with_hints() {
    let dir = tempfile::tempdir().unwrap();
    let report = verify_runtime_acceptance_with_hints(
        dir.path(),
        &[],
        &[],
        &strings(&["player_control"]),
        &[],
        &[],
        &[],
        &strings(&["hint"]),
    );
    assert!(!report.passed);
    assert_eq!(
        report.missing_evidence,
        [
            "implementation_artifact",
            "interactive_ui_source_evidence",
            "user_input_handler_evidence",
            "visible_interactive_surface_evidence"
        ]
    );
    assert_eq!(
        report.capability_evidence_bindings[0].required_evidence,
        [
            "implementation_artifact",
            "visible_interactive_surface_evidence",
            "user_input_handler_evidence",
            "interactive_ui_source_evidence"
        ]
    );
    assert_eq!(
        report.primary_reason,
        "missing_required_evidence:implementation_artifact"
    );
}

#[test]
fn browser_interaction_without_readiness_evidence_is_inconclusive() {
    let dir = tempfile::tempdir().unwrap();
    let report = run(
        dir.path(),
        &["src/main.rs"],
        &[],
        &["browser_interaction"],
        &[],
        &[],
        &[],
    );
    assert!(!report.passed);
    assert!(report.inconclusive);
    assert_eq!(
        report.missing_evidence,
        [
            "implementation_artifact",
            "interactive_ui_source_evidence",
            "non_static_screen_evidence",
            "stateful_update_evidence",
            "user_input_handler_evidence",
            "visible_interactive_surface_evidence"
        ]
    );
    assert_eq!(
        report.browser_readiness_status,
        "unavailable:browser_readiness_evidence_missing"
    );
    assert!(
        report
            .inconclusive_reasons
            .iter()
            .any(|reason| reason
                == "browser_readiness_unavailable:browser_readiness_evidence_missing")
    );
    assert_eq!(
        report.primary_reason,
        "missing_required_evidence:implementation_artifact"
    );
}
