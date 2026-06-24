//! Phase29 runtime-support parity facts.
//!
//! This module projects already-selected recovery/evidence state into compact
//! report fields for C34-C44. It is not a dispatcher, executor, provider
//! policy layer, or profile workflow engine.

use crate::agent::step_runner::command_classification::classify_shell_command;
use crate::agent::step_runner::correction_evidence::ContractEvidence;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct RuntimeSupportFacts {
    pub(crate) rows: Vec<&'static str>,
    pub(crate) fields: Vec<String>,
}

impl RuntimeSupportFacts {
    pub(crate) fn eval_report_fields(&self) -> Vec<String> {
        let mut fields = self.fields.clone();
        if !self.rows.is_empty() {
            fields.push(format!("phase29_support_rows={}", self.rows.join("|")));
        }
        fields
    }

    fn add_row(&mut self, row: &'static str) {
        if !self.rows.contains(&row) {
            self.rows.push(row);
        }
    }

    fn add_field(&mut self, field: impl Into<String>) {
        let field = field.into();
        if !self.fields.contains(&field) {
            self.fields.push(field);
        }
    }
}

pub(crate) struct RuntimeSupportInput<'a> {
    pub(crate) evidence: &'a ContractEvidence,
    pub(crate) job: &'a str,
    pub(crate) action: &'a str,
    pub(crate) tool_policy: &'a str,
    pub(crate) lifecycle: &'a str,
    pub(crate) loop_control_action: &'a str,
}

pub(crate) fn phase29_runtime_support_facts(input: RuntimeSupportInput<'_>) -> RuntimeSupportFacts {
    let mut facts = RuntimeSupportFacts::default();
    let evidence = input.evidence;

    if evidence
        .eval_report_fields
        .iter()
        .any(|field| field.starts_with("mechanical_adapter_status="))
    {
        facts.add_row("C34");
        facts.add_field("language_repair_adapter_status=projected");
    }

    if !input.tool_policy.trim().is_empty() {
        facts.add_row("C35");
        facts.add_field(format!("effective_tool_policy={}", input.tool_policy));
        facts.add_field("effective_tool_policy_status=projected");
    }

    if matches!(
        evidence.guard.as_str(),
        "tool_protocol" | "provider_transport"
    ) || primary_text(evidence).contains("tool_args_")
        || primary_text(evidence).contains("provider_transport")
        || primary_text(evidence).contains("final_answer_contract")
    {
        facts.add_row("C36");
        facts.add_field("tool_failure_recovery_status=bounded_correction");
    }

    if let Some(command) = evidence.command.as_deref() {
        let classified = classify_shell_command(command);
        facts.add_row("C37");
        for field in classified.eval_report_fields() {
            facts.add_field(field);
        }
    }

    if evidence.workspace_scope.is_some()
        || !evidence.artifact_graph_summary.is_empty()
        || !evidence.proposed_targets.is_empty()
        || !evidence.rejected_targets.is_empty()
    {
        facts.add_row("C38");
        facts.add_field("workspace_candidate_status=projected");
        facts.add_field("workspace_ignored_dir_policy=single_source_of_truth");
    }

    if !input.job.trim().is_empty() && input.job != "none" {
        facts.add_row("C39");
        facts.add_field("job_report_status=projected");
        facts.add_field(format!(
            "job_report_owner_action={}:{}",
            input.job, input.action
        ));
    }

    if input.job == "scaffold_materialization"
        || primary_text(evidence).contains("scaffold")
        || evidence
            .eval_report_fields
            .iter()
            .any(|field| field.starts_with("scaffold_created_paths="))
    {
        facts.add_row("C40");
        facts.add_field("scaffold_contract_status=artifact_obligation");
    }

    if has_noncoding_evidence(evidence) {
        facts.add_row("C41");
        facts.add_field("noncoding_evidence_status=generic_producer");
    }

    if evidence.guard == "final_answer"
        || primary_text(evidence).contains("answer_only")
        || primary_text(evidence).contains("work_mode")
        || input.loop_control_action == "render_explicit_stop"
    {
        facts.add_row("C42");
        facts.add_field("answer_work_mode_status=deterministic_gate");
    }

    if !input.lifecycle.trim().is_empty() {
        facts.add_row("C43");
        facts.add_field(format!("lifecycle_projection_status={}", input.lifecycle));
    }

    if evidence.guard == "provider_transport" {
        facts.add_row("C44");
        facts.add_field("provider_boundary_status=transport_only");
    }

    facts
}

fn primary_text(evidence: &ContractEvidence) -> String {
    [
        evidence.reason_code.as_deref(),
        evidence.violated_contract.as_deref(),
        evidence.failure_kind.as_deref(),
        evidence.diagnostic_code.as_deref(),
        evidence.explicit_stop_reason.as_deref(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" ")
    .to_ascii_lowercase()
}

fn has_noncoding_evidence(evidence: &ContractEvidence) -> bool {
    let text = evidence
        .completion_evidence
        .iter()
        .chain(evidence.evidence_binding.iter())
        .chain(evidence.deliverable_obligations.iter())
        .chain(evidence.eval_report_fields.iter())
        .chain(evidence.candidate_artifacts.iter())
        .chain(evidence.required_paths.iter())
        .chain(evidence.missing_paths.iter())
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    [
        "docs",
        "documentation",
        "required_section",
        "schema",
        "structured_data",
        "citation",
        "research",
        "ops",
        "derived_output",
        ".md",
        ".csv",
        ".json",
    ]
    .iter()
    .any(|needle| text.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projects_mechanical_policy_workspace_and_job_rows() {
        let evidence = ContractEvidence::new("verifier")
            .with_command("cargo test")
            .with_workspace_scope("implementation_artifact_scope")
            .with_eval_report_fields(vec!["mechanical_adapter_status=admitted"]);

        let facts = phase29_runtime_support_facts(RuntimeSupportInput {
            evidence: &evidence,
            job: "source_implementation_repair",
            action: "edit_source_for_diagnostic",
            tool_policy: "file_mutation_repair",
            lifecycle: "selected",
            loop_control_action: "run_bounded_repair_task",
        });
        let fields = facts.eval_report_fields();

        assert!(fields.contains(&"language_repair_adapter_status=projected".to_string()));
        assert!(fields.contains(&"effective_tool_policy_status=projected".to_string()));
        assert!(fields.contains(&"setup_command_classification=verifier".to_string()));
        assert!(fields.contains(&"workspace_candidate_status=projected".to_string()));
        assert!(fields.contains(&"job_report_status=projected".to_string()));
        assert!(fields.contains(&"lifecycle_projection_status=selected".to_string()));
        assert!(
            fields
                .iter()
                .any(|field| field == "phase29_support_rows=C34|C35|C37|C38|C39|C43")
        );
    }

    #[test]
    fn projects_tool_failure_and_provider_boundary_rows() {
        let evidence = ContractEvidence::new("provider_transport")
            .with_reason_code("provider_transport_parse_failure");

        let facts = phase29_runtime_support_facts(RuntimeSupportInput {
            evidence: &evidence,
            job: "tool_protocol_correction",
            action: "correct_tool_protocol",
            tool_policy: "tool_protocol_correction",
            lifecycle: "selected",
            loop_control_action: "run_tool_protocol_correction",
        });
        let fields = facts.eval_report_fields();

        assert!(fields.contains(&"tool_failure_recovery_status=bounded_correction".to_string()));
        assert!(fields.contains(&"provider_boundary_status=transport_only".to_string()));
        assert!(
            fields
                .iter()
                .any(|field| field == "phase29_support_rows=C35|C36|C39|C43|C44")
        );
    }

    #[test]
    fn projects_scaffold_and_noncoding_rows_without_profile_workflow() {
        let evidence = ContractEvidence::new("profile_verification")
            .with_reason_code("missing_required_artifact")
            .with_candidate_artifacts(vec!["README.md"])
            .with_completion_evidence(vec![
                "kind=docs_section_pass target=README.md status=missing source=docs_section_check",
            ]);

        let facts = phase29_runtime_support_facts(RuntimeSupportInput {
            evidence: &evidence,
            job: "scaffold_materialization",
            action: "create_required_artifact",
            tool_policy: "file_mutation_repair",
            lifecycle: "selected",
            loop_control_action: "run_bounded_repair_task",
        });
        let fields = facts.eval_report_fields();

        assert!(fields.contains(&"scaffold_contract_status=artifact_obligation".to_string()));
        assert!(fields.contains(&"noncoding_evidence_status=generic_producer".to_string()));
        assert!(
            fields
                .iter()
                .any(|field| field.contains("C40") && field.contains("C41"))
        );
    }
}
