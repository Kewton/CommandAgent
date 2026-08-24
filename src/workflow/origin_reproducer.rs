//! Mechanical origin-derived reproducer selection for workflow entry edges.
//!
//! Candidates are ordered by concrete workspace evidence. A candidate becomes
//! requester-supplied R only after an execution on the origin copy produces a
//! subject failure. Passing, unavailable, or reproducer-defect observations
//! remain recorded attempts and do not become a binding.

use std::collections::BTreeSet;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::config::Config;
use crate::planner::adjudication::contract::{EvidenceStage, ExpectedOutcome, ProbeOutcome};
use crate::planner::external_reproducer::ExternalReproducerBinding;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Candidate {
    pub basis: String,
    pub command: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProbeResult {
    pub outcome: String,
    pub subject_failure: bool,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct PrevalidationRecord {
    pub(crate) basis: String,
    pub(crate) command: String,
    pub(crate) lineage: String,
    pub(crate) outcome: String,
    pub(crate) subject_failure: bool,
    pub(crate) reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct OriginReproducerRecord {
    pub(crate) status: String,
    pub(crate) attempts: Vec<PrevalidationRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) bound: Option<ExternalReproducerBinding>,
}

pub(crate) fn derive_and_prevalidate<F>(
    origin: &Path,
    origin_events: &Path,
    mut probe: F,
) -> OriginReproducerRecord
where
    F: FnMut(&Candidate) -> ProbeResult,
{
    let mut attempts = Vec::new();
    for candidate in candidates(origin, origin_events) {
        let Ok(binding) =
            ExternalReproducerBinding::new(candidate.basis.clone(), candidate.command.clone())
        else {
            continue;
        };
        let result = probe(&Candidate {
            basis: binding.basis.clone(),
            command: binding.command.clone(),
        });
        attempts.push(PrevalidationRecord {
            basis: binding.basis.clone(),
            command: binding.command.clone(),
            lineage: binding.lineage.clone(),
            outcome: result.outcome,
            subject_failure: result.subject_failure,
            reason: result.reason,
        });
        if attempts
            .last()
            .is_some_and(|attempt| attempt.outcome == "failure" && attempt.subject_failure)
        {
            return OriginReproducerRecord {
                status: "bound".into(),
                attempts,
                bound: Some(binding),
            };
        }
    }
    OriginReproducerRecord {
        status: "not_derived".into(),
        attempts,
        bound: None,
    }
}

pub(crate) fn derive_from_origin(
    config: &Config,
    origin: &Path,
    origin_events: &Path,
    origin_goal: &str,
    workflow_events: &Path,
) -> OriginReproducerRecord {
    let mut probe_config = config.clone();
    probe_config.workspace_root = origin.to_path_buf();
    probe_config.eval_events_path =
        Some(origin.join("evidence/workflow-reproducer-prevalidation-events.jsonl"));
    let record = derive_and_prevalidate(origin, origin_events, |candidate| {
        let lineage = crate::planner::adjudication::fix::reproducer_lineage(&candidate.command);
        let result = crate::planner::fix_diagnostics::run_reproducer(
            &probe_config,
            "workflow-origin-prevalidation",
            "reproducer_fails",
            EvidenceStage::Diagnosis,
            ExpectedOutcome::Failure,
            1,
            &candidate.command,
            &lineage,
            "data",
            origin_goal,
        );
        ProbeResult {
            outcome: outcome_name(result.evidence.outcome).into(),
            subject_failure: result.evidence.failure_classification.is_subject(),
            reason: result.evidence.reason,
        }
    });
    for attempt in &record.attempts {
        crate::eval_events::emit(
            Some(workflow_events),
            json!({
                "event":"workflow_reproducer_prevalidated",
                "basis":attempt.basis,
                "command":attempt.command,
                "lineage":attempt.lineage,
                "outcome":attempt.outcome,
                "subject_failure":attempt.subject_failure,
                "reason":attempt.reason,
            }),
        );
    }
    record
}

pub(crate) fn binding_from_investigation(
    origin: &Path,
) -> Result<ExternalReproducerBinding, String> {
    let path = origin.join("evidence/investigation-run.json");
    let run: crate::planner::adjudication::investigate::InvestigationRunEvidence =
        serde_json::from_slice(&std::fs::read(path).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
    let binding = ExternalReproducerBinding::new("investigation_i1", run.reproducer)?;
    if run.reproducer_lineage.is_empty() || run.reproducer_lineage != binding.lineage {
        return Err("investigation reproducer lineage is missing or inconsistent".into());
    }
    Ok(binding)
}

const fn outcome_name(outcome: ProbeOutcome) -> &'static str {
    match outcome {
        ProbeOutcome::Success => "success",
        ProbeOutcome::Failure => "failure",
        ProbeOutcome::Inconclusive => "inconclusive",
        ProbeOutcome::Unavailable => "unavailable",
        ProbeOutcome::NotExecuted => "not_executed",
    }
}

fn candidates(origin: &Path, origin_events: &Path) -> Vec<Candidate> {
    let mut candidates = Vec::new();
    if origin.join("pipeline/main.py").is_file() {
        candidates.push(Candidate {
            basis: "origin_workspace:pipeline_probe".into(),
            command: "python3 -B pipeline/main.py".into(),
        });
    }
    if origin.join("output/results.json").is_file() {
        candidates.push(Candidate {
            basis: "origin_workspace:data_results_schema".into(),
            command: crate::planner::profiles::data::step_policy::catalog_check_command(
                "data_results_schema",
            ),
        });
    }
    if let Ok(bindings) = super::runner::derive_origin_bindings(origin_events) {
        candidates.extend(bindings.into_iter().map(|binding| Candidate {
            basis: format!("origin_verify_binding:{}", binding.check_id),
            command: binding.check_id,
        }));
    }
    let mut seen = BTreeSet::new();
    candidates.retain(|candidate| seen.insert(candidate.command.clone()));
    candidates
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    fn events(root: &Path, lines: &str) -> std::path::PathBuf {
        let path = root.join("events.jsonl");
        std::fs::write(&path, lines).unwrap();
        path
    }

    fn failed() -> ProbeResult {
        ProbeResult {
            outcome: "failure".into(),
            subject_failure: true,
            reason: "expected failure".into(),
        }
    }

    #[test]
    fn pipeline_candidate_has_first_priority() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("pipeline")).unwrap();
        std::fs::create_dir_all(root.path().join("output")).unwrap();
        std::fs::write(root.path().join("pipeline/main.py"), "raise SystemExit(1)").unwrap();
        std::fs::write(root.path().join("output/results.json"), "{}").unwrap();
        let events = events(root.path(), "");

        let record = derive_and_prevalidate(root.path(), &events, |_| failed());
        assert_eq!(record.attempts.len(), 1);
        assert_eq!(record.bound.unwrap().command, "python3 -B pipeline/main.py");
    }

    #[test]
    fn results_schema_is_tried_after_a_passing_pipeline() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("pipeline")).unwrap();
        std::fs::create_dir_all(root.path().join("output")).unwrap();
        std::fs::write(root.path().join("pipeline/main.py"), "").unwrap();
        std::fs::write(root.path().join("output/results.json"), "{}").unwrap();
        let events = events(root.path(), "");

        let record = derive_and_prevalidate(root.path(), &events, |candidate| {
            if candidate.command.contains("pipeline/main.py") {
                ProbeResult {
                    outcome: "success".into(),
                    subject_failure: false,
                    reason: "passed".into(),
                }
            } else {
                failed()
            }
        });
        assert_eq!(record.attempts.len(), 2);
        assert_eq!(
            record.bound.unwrap().command,
            "anvil-catalog-check:data_results_schema"
        );
    }

    #[test]
    fn origin_failure_binding_is_quoted_as_the_third_source() {
        let root = tempfile::tempdir().unwrap();
        let events = events(
            root.path(),
            r#"{"event":"verify_default_bound","bound_checks":["test -f required.txt"]}"#,
        );

        let record = derive_and_prevalidate(root.path(), &events, |_| failed());
        let binding = record.bound.unwrap();
        assert_eq!(binding.basis, "origin_verify_binding:test -f required.txt");
        assert_eq!(binding.command, "test -f required.txt");
    }

    #[test]
    fn passing_and_reproducer_defect_candidates_are_not_supplied() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("pipeline")).unwrap();
        std::fs::write(root.path().join("pipeline/main.py"), "").unwrap();
        let events = events(root.path(), "");

        let record = derive_and_prevalidate(root.path(), &events, |_| ProbeResult {
            outcome: "failure".into(),
            subject_failure: false,
            reason: "reproducer_defect".into(),
        });
        assert_eq!(record.status, "not_derived");
        assert!(record.bound.is_none());
    }

    #[test]
    fn underivable_origin_preserves_node_local_resolution() {
        let root = tempfile::tempdir().unwrap();
        let events = events(root.path(), "");
        let record = derive_and_prevalidate(root.path(), &events, |_| {
            panic!("no candidate may be executed")
        });
        assert_eq!(record.status, "not_derived");
        assert!(record.attempts.is_empty());
        assert!(record.bound.is_none());
    }

    #[test]
    fn dfix002_corpus_fixes_the_prevalidation_evidence_shape() {
        let record: OriginReproducerRecord = serde_json::from_str(include_str!(
            "../../tests/corpus/apps/workflow_circle_dfix002/fixtures/origin-reproducer-prevalidation.json"
        ))
        .unwrap();
        assert_eq!(record.status, "bound");
        assert_eq!(record.attempts.len(), 1);
        let binding = record.bound.unwrap();
        binding.validate().unwrap();
        assert_eq!(
            binding,
            ExternalReproducerBinding::new(
                "origin_workspace:pipeline_probe",
                "python3 -B pipeline/main.py",
            )
            .unwrap()
        );
    }

    #[test]
    fn production_prevalidation_executes_and_records_the_bound_pipeline_r() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("pipeline")).unwrap();
        std::fs::write(
            root.path().join("pipeline/main.py"),
            "raise ValueError('measured origin failure')\n",
        )
        .unwrap();
        let origin_events = events(root.path(), "");
        let workflow_events = root.path().join("evidence/workflow-events.jsonl");
        let cwd = root.path().to_string_lossy();
        let config = Config::from_cli(crate::cli::Cli::parse_from([
            "commandagent",
            "--cwd",
            cwd.as_ref(),
            "goal",
        ]))
        .unwrap();

        let record = derive_from_origin(
            &config,
            root.path(),
            &origin_events,
            "origin goal",
            &workflow_events,
        );

        assert_eq!(record.status, "bound");
        assert_eq!(record.attempts[0].outcome, "failure");
        assert!(record.attempts[0].subject_failure);
        assert_eq!(record.bound.unwrap().command, "python3 -B pipeline/main.py");
        let events = std::fs::read_to_string(workflow_events).unwrap();
        assert!(events.contains("\"event\":\"workflow_reproducer_prevalidated\""));
        assert!(events.contains("\"outcome\":\"failure\""));
    }
}
