//! Command-local evidence repairability. A diagnosis grants no write authority
//! and cannot replace execution or the frozen-verifier preservation check.
use super::{VerifyCommandKind, collect_workspace_evidence, verify_command_kind};
use serde::Serialize;
use std::path::Path;

/// Run the final command classifier without application files, then the same
/// requirement normalization/pruning used by final acceptance. This is only a
/// preclosure refusal: it never grants evidence or substitutes for execution.
pub(crate) fn source_independent_inline_failure(
    contract: &crate::minimal_loop::completion::CompletionContract,
    command: &str,
) -> Option<String> {
    let words = super::verify_command_classification::single_command_words(command)?;
    if words.first()?.as_str() != "node" {
        return None;
    }
    super::verify_command_classification::inline_argument(&words[1..])?;
    let super::VerifyCommandKind::Weak(reason) =
        super::verify_command_kind(command, &super::WorkspaceEvidence::default())
    else {
        return None;
    };
    let mut report = super::RuntimeAcceptanceReport {
        weak_evidence: vec![reason.clone()],
        ..Default::default()
    };
    super::refresh_runtime_acceptance_report(
        &mut report,
        &contract.required_capabilities,
        &contract.required_evidence,
        &contract.required_obligations,
    );
    (!report.passed && report.weak_evidence.contains(&reason)).then_some(reason)
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct Diagnosis {
    pub(crate) command: String,
    pub(crate) kind: &'static str,
    pub(crate) reason: Option<String>,
    pub(crate) repairability: &'static str,
    pub(crate) script: Option<String>,
    pub(crate) execution_failure: Option<String>,
}

impl Diagnosis {
    pub(crate) fn immutable_inline(&self) -> bool {
        self.repairability == "immutable_inline_evidence"
    }

    fn blocks_app_repair(&self) -> bool {
        self.immutable_inline() || self.repairability == "frozen_verifier_evidence"
    }
}

pub(crate) fn collect(root: &Path, commands: &[String]) -> Vec<Diagnosis> {
    let workspace = collect_workspace_evidence(root);
    commands
        .iter()
        .map(|command| {
            let script = crate::planner::recovery_contract_authority::local_script_path(command);
            let (kind, reason) = match verify_command_kind(command, &workspace) {
                VerifyCommandKind::Weak(reason) => ("weak", Some(reason)),
                VerifyCommandKind::ArtifactOnly => ("artifact_only", None),
                VerifyCommandKind::StaticSyntax => ("static_syntax", None),
                VerifyCommandKind::Test => ("test", None),
                VerifyCommandKind::Build => ("build", None),
                VerifyCommandKind::Other => ("other", None),
            };
            let inline = super::verify_command_classification::single_command_words(command)
                .is_some_and(|w| {
                    w.first().is_some_and(|w| w == "node")
                        && super::verify_command_classification::inline_argument(&w[1..]).is_some()
                });
            let repairability = if kind == "weak" && inline {
                "immutable_inline_evidence"
            } else if kind == "weak" && script.is_some() {
                "file_backed_verifier_requires_owner"
            } else if matches!(kind, "test" | "build" | "static_syntax") {
                "runtime_input_dependent"
            } else {
                "unknown"
            };
            Diagnosis {
                command: command.clone(),
                kind,
                reason,
                repairability,
                script,
                execution_failure: None,
            }
        })
        .collect()
}

pub(crate) fn annotate(
    diagnoses: &mut [Diagnosis],
    report: &crate::planner::verify::VerificationReport,
) {
    for diagnosis in diagnoses {
        diagnosis.execution_failure = report
            .command_failures
            .iter()
            .find(|f| f.command == diagnosis.command)
            .map(|f| f.reason.clone());
    }
}

pub(crate) fn immutable_failure(diagnoses: &[Diagnosis]) -> Option<String> {
    let reasons = diagnoses
        .iter()
        .filter(|d| d.blocks_app_repair())
        .map(|d| {
            format!(
                "{}: {} ({}; app edits cannot strengthen this fixed verifier)",
                d.command,
                d.reason.as_deref().unwrap_or("weak_verification_evidence"),
                d.repairability
            )
        })
        .collect::<Vec<_>>();
    (!reasons.is_empty()).then(|| reasons.join("\n"))
}

/// A fixed contract without source-evidence obligations relies on execution,
/// not the evidence-quality gate. Preserve that existing contract semantics.
pub(crate) fn contract_failure(
    contract: &crate::minimal_loop::completion::CompletionContract,
    diagnoses: &[Diagnosis],
) -> Option<String> {
    super::source_first_completion_authority_required(
        &contract.required_capabilities,
        &contract.required_evidence,
        &contract.required_obligations,
    )
    .then(|| immutable_failure(diagnoses))
    .flatten()
}
