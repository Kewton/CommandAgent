//! Bounded mechanical repair hints derived from deterministic diagnostics.
//!
//! This module does not mutate files and does not choose recovery jobs. It
//! consumes an already admitted owner/target/action context and returns a small
//! hint or proposal that the existing recovery task can render.

use crate::agent::step_runner::verifier_diagnostic::VerifierDiagnosticCode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MechanicalRepairInput {
    pub(crate) diagnostic_code: VerifierDiagnosticCode,
    pub(crate) failure_kind: String,
    pub(crate) active_job: String,
    pub(crate) target_path: Option<String>,
    pub(crate) target_role: Option<String>,
    pub(crate) repair_action: Option<String>,
    pub(crate) source_of_truth: String,
    pub(crate) allowed_change_kind: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MechanicalRepairStatus {
    NotApplicable,
    Admitted,
    Rejected,
}

impl MechanicalRepairStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::NotApplicable => "not_applicable",
            Self::Admitted => "admitted",
            Self::Rejected => "rejected",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MechanicalRepairOutput {
    pub(crate) adapter_id: String,
    pub(crate) status: MechanicalRepairStatus,
    pub(crate) action: String,
    pub(crate) target_path: Option<String>,
    pub(crate) hint: String,
    pub(crate) reason: String,
    pub(crate) rerun_authority: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MechanicalAdapterFamilySpec {
    pub(crate) id: &'static str,
    pub(crate) diagnostic_codes: &'static [&'static str],
    pub(crate) allowed_target_roles: &'static [&'static str],
    pub(crate) source_of_truth: &'static str,
    pub(crate) rerun_authority: &'static str,
}

pub(crate) fn mechanical_adapter_family_specs() -> &'static [MechanicalAdapterFamilySpec] {
    &[
        MechanicalAdapterFamilySpec {
            id: "rust_compile",
            diagnostic_codes: &["rust_compile_error"],
            allowed_target_roles: &["source", "test"],
            source_of_truth: "cargo_diagnostic",
            rerun_authority: "original_cargo_verifier",
        },
        MechanicalAdapterFamilySpec {
            id: "rust_cargo_manifest",
            diagnostic_codes: &["dependency_missing", "cargo_manifest_missing"],
            allowed_target_roles: &["manifest"],
            source_of_truth: "cargo_or_manifest_diagnostic",
            rerun_authority: "original_cargo_verifier",
        },
        MechanicalAdapterFamilySpec {
            id: "rust_assertion",
            diagnostic_codes: &["rust_test_assertion_mismatch"],
            allowed_target_roles: &["source", "test"],
            source_of_truth: "cargo_test_diagnostic",
            rerun_authority: "original_cargo_verifier",
        },
        MechanicalAdapterFamilySpec {
            id: "python_import",
            diagnostic_codes: &["python_import_missing"],
            allowed_target_roles: &["source", "test"],
            source_of_truth: "python_traceback",
            rerun_authority: "original_python_verifier",
        },
        MechanicalAdapterFamilySpec {
            id: "python_assertion",
            diagnostic_codes: &["python_assertion_mismatch"],
            allowed_target_roles: &["source", "test"],
            source_of_truth: "pytest_diagnostic",
            rerun_authority: "original_python_verifier",
        },
        MechanicalAdapterFamilySpec {
            id: "fastapi_response",
            diagnostic_codes: &["fastapi_response_mismatch"],
            allowed_target_roles: &["source", "test"],
            source_of_truth: "pytest_or_http_diagnostic",
            rerun_authority: "original_python_verifier",
        },
        MechanicalAdapterFamilySpec {
            id: "node_next_type",
            diagnostic_codes: &["typescript_type_error", "nextjs_event_handler_boundary"],
            allowed_target_roles: &["source", "route_entry", "config"],
            source_of_truth: "next_or_typescript_diagnostic",
            rerun_authority: "original_nextjs_verifier",
        },
        MechanicalAdapterFamilySpec {
            id: "nextjs_route_integration",
            diagnostic_codes: &["nextjs_route_not_integrated"],
            allowed_target_roles: &["route_entry", "source"],
            source_of_truth: "profile_route_graph",
            rerun_authority: "profile_verification_then_original_nextjs_verifier",
        },
        MechanicalAdapterFamilySpec {
            id: "manifest_dependency",
            diagnostic_codes: &["dependency_missing"],
            allowed_target_roles: &["manifest"],
            source_of_truth: "verifier_dependency_diagnostic",
            rerun_authority: "verifier_owned_setup_then_original_verifier",
        },
        MechanicalAdapterFamilySpec {
            id: "docs_literal",
            diagnostic_codes: &["docs_literal_missing"],
            allowed_target_roles: &["documentation"],
            source_of_truth: "profile_completion_evidence",
            rerun_authority: "original_docs_check",
        },
        MechanicalAdapterFamilySpec {
            id: "data_schema",
            diagnostic_codes: &["derived_output_missing", "schema_mismatch"],
            allowed_target_roles: &["derived_output", "pipeline"],
            source_of_truth: "data_output_contract",
            rerun_authority: "original_data_verifier",
        },
    ]
}

impl MechanicalRepairOutput {
    pub(crate) fn render_lines(&self) -> Vec<String> {
        vec![format!(
            "mechanical_adapter={} status={} action={} target={} reason={} hint={} rerun_authority={}",
            compact(&self.adapter_id),
            self.status.as_str(),
            compact(&self.action),
            self.target_path.as_deref().unwrap_or("none"),
            compact(&self.reason),
            compact(&self.hint),
            compact(&self.rerun_authority)
        )]
    }

    pub(crate) fn eval_report_fields(&self) -> Vec<String> {
        vec![
            format!("mechanical_adapter={}", compact(&self.adapter_id)),
            format!("mechanical_adapter_status={}", self.status.as_str()),
            format!("mechanical_adapter_action={}", compact(&self.action)),
        ]
    }
}

pub(crate) fn mechanical_repair_hint(input: &MechanicalRepairInput) -> MechanicalRepairOutput {
    let Some((adapter_id, action, hint)) = adapter_mapping(input) else {
        return MechanicalRepairOutput {
            adapter_id: "none".to_string(),
            status: MechanicalRepairStatus::NotApplicable,
            action: "none".to_string(),
            target_path: input.target_path.clone(),
            hint: "no deterministic mechanical adapter matched this diagnostic".to_string(),
            reason: "diagnostic_not_supported_by_mechanical_adapter".to_string(),
            rerun_authority: input.source_of_truth.clone(),
        };
    };

    let Some(target) = input.target_path.clone() else {
        return MechanicalRepairOutput {
            adapter_id: adapter_id.to_string(),
            status: MechanicalRepairStatus::Rejected,
            action: action.to_string(),
            target_path: None,
            hint: hint.to_string(),
            reason: "mechanical_adapter_requires_admitted_target".to_string(),
            rerun_authority: input.source_of_truth.clone(),
        };
    };

    if input.active_job.trim().is_empty()
        || input.active_job == "unknown"
        || input
            .repair_action
            .as_deref()
            .unwrap_or("")
            .trim()
            .is_empty()
        || input.target_role.as_deref().unwrap_or("").trim().is_empty()
        || input.source_of_truth.trim().is_empty()
        || input.source_of_truth == "unknown"
        || input
            .allowed_change_kind
            .as_deref()
            .unwrap_or("")
            .trim()
            .is_empty()
    {
        return MechanicalRepairOutput {
            adapter_id: adapter_id.to_string(),
            status: MechanicalRepairStatus::Rejected,
            action: action.to_string(),
            target_path: Some(target),
            hint: hint.to_string(),
            reason: "mechanical_adapter_requires_owner_action_target_and_verifier_authority"
                .to_string(),
            rerun_authority: input.source_of_truth.clone(),
        };
    }

    if target_is_disallowed(&target) {
        return MechanicalRepairOutput {
            adapter_id: adapter_id.to_string(),
            status: MechanicalRepairStatus::Rejected,
            action: action.to_string(),
            target_path: Some(target),
            hint: hint.to_string(),
            reason: "mechanical_adapter_target_not_admitted_for_mutation".to_string(),
            rerun_authority: input.source_of_truth.clone(),
        };
    }

    MechanicalRepairOutput {
        adapter_id: adapter_id.to_string(),
        status: MechanicalRepairStatus::Admitted,
        action: action.to_string(),
        target_path: Some(target),
        hint: hint.to_string(),
        reason: "mechanical_adapter_admitted_under_existing_target_action_contract".to_string(),
        rerun_authority: input.source_of_truth.clone(),
    }
}

fn adapter_mapping(
    input: &MechanicalRepairInput,
) -> Option<(&'static str, &'static str, &'static str)> {
    match input.diagnostic_code {
        VerifierDiagnosticCode::RustCompileError => Some((
            "rust_compile_diagnostic",
            "repair_rust_compile_error",
            "Use the compiler diagnostic to repair the admitted Rust source target; do not edit Cargo artifacts unless the action targets a manifest.",
        )),
        VerifierDiagnosticCode::RustTestAssertionMismatch => Some((
            "rust_assertion_diagnostic",
            "repair_rust_assertion_mismatch",
            "Repair the admitted implementation or test contract according to the assertion mismatch and rerun the original cargo verifier.",
        )),
        VerifierDiagnosticCode::PythonImportMissing => Some((
            "python_import_diagnostic",
            "repair_python_import_or_module_path",
            "Repair the admitted Python module/import target; do not create unrelated package structure.",
        )),
        VerifierDiagnosticCode::PythonAssertionMismatch
        | VerifierDiagnosticCode::FastapiResponseMismatch => Some((
            "python_assertion_diagnostic",
            "repair_python_assertion_mismatch",
            "Repair the admitted source/test contract for the observed assertion mismatch without weakening the assertion.",
        )),
        VerifierDiagnosticCode::TypescriptTypeError
        | VerifierDiagnosticCode::NextjsEventHandlerBoundary => Some((
            "node_next_type_diagnostic",
            "repair_typescript_or_client_boundary",
            "Repair the admitted TypeScript/Next.js target while preserving build script honesty.",
        )),
        VerifierDiagnosticCode::NextjsRouteNotIntegrated => Some((
            "nextjs_route_integration_diagnostic",
            "connect_existing_artifact_to_selected_route",
            "Connect the existing artifact to the selected route graph; do not create placeholder feature work.",
        )),
        VerifierDiagnosticCode::DependencyMissing => Some((
            "manifest_dependency_diagnostic",
            "repair_manifest_dependency_contract",
            "Repair the admitted manifest dependency declaration; verifier-owned setup remains the only authority for installing dependencies.",
        )),
        _ => None,
    }
}

fn target_is_disallowed(target: &str) -> bool {
    target.starts_with('/')
        || target.starts_with("../")
        || target.contains("/../")
        || target.starts_with("node_modules/")
        || target.contains("/node_modules/")
        || target.starts_with(".next/")
        || target.contains("/.next/")
        || target.starts_with("target/")
}

fn compact(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join("_")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(code: VerifierDiagnosticCode, target: Option<&str>) -> MechanicalRepairInput {
        MechanicalRepairInput {
            diagnostic_code: code,
            failure_kind: "compile_or_type_error".to_string(),
            active_job: "source_implementation_repair".to_string(),
            target_path: target.map(str::to_string),
            target_role: Some("implementation".to_string()),
            repair_action: Some("repair_source_error".to_string()),
            source_of_truth: "original_verifier_diagnostic".to_string(),
            allowed_change_kind: Some("source_edit".to_string()),
        }
    }

    #[test]
    fn admits_rust_compile_hint_for_admitted_target() {
        let output = mechanical_repair_hint(&input(
            VerifierDiagnosticCode::RustCompileError,
            Some("src/main.rs"),
        ));

        assert_eq!(output.status, MechanicalRepairStatus::Admitted);
        assert_eq!(output.adapter_id, "rust_compile_diagnostic");
        assert!(
            output
                .eval_report_fields()
                .contains(&"mechanical_adapter_status=admitted".to_string())
        );
    }

    #[test]
    fn rejects_adapter_without_target() {
        let output =
            mechanical_repair_hint(&input(VerifierDiagnosticCode::TypescriptTypeError, None));

        assert_eq!(output.status, MechanicalRepairStatus::Rejected);
        assert_eq!(output.reason, "mechanical_adapter_requires_admitted_target");
    }

    #[test]
    fn rejects_dependency_cache_targets() {
        let output = mechanical_repair_hint(&input(
            VerifierDiagnosticCode::DependencyMissing,
            Some("node_modules/react/index.js"),
        ));

        assert_eq!(output.status, MechanicalRepairStatus::Rejected);
        assert_eq!(
            output.reason,
            "mechanical_adapter_target_not_admitted_for_mutation"
        );
    }

    #[test]
    fn rejects_adapter_without_action_and_verifier_authority() {
        let mut input = input(
            VerifierDiagnosticCode::RustCompileError,
            Some("src/main.rs"),
        );
        input.repair_action = None;
        input.source_of_truth = "unknown".to_string();

        let output = mechanical_repair_hint(&input);

        assert_eq!(output.status, MechanicalRepairStatus::Rejected);
        assert_eq!(
            output.reason,
            "mechanical_adapter_requires_owner_action_target_and_verifier_authority"
        );
    }

    #[test]
    fn reports_not_applicable_for_unknown_diagnostics() {
        let output = mechanical_repair_hint(&input(
            VerifierDiagnosticCode::UnknownVerifierFailure,
            Some("src/main.rs"),
        ));

        assert_eq!(output.status, MechanicalRepairStatus::NotApplicable);
        assert_eq!(output.adapter_id, "none");
    }

    #[test]
    fn exposes_adapter_family_registry_for_profile_parity() {
        let specs = mechanical_adapter_family_specs();
        for expected in [
            "rust_compile",
            "rust_cargo_manifest",
            "python_import",
            "fastapi_response",
            "node_next_type",
            "nextjs_route_integration",
            "manifest_dependency",
            "docs_literal",
            "data_schema",
        ] {
            assert!(
                specs.iter().any(|spec| spec.id == expected),
                "missing adapter family {expected}"
            );
        }
        assert!(specs.iter().all(|spec| !spec.diagnostic_codes.is_empty()));
        assert!(
            specs
                .iter()
                .all(|spec| !spec.allowed_target_roles.is_empty())
        );
    }
}
