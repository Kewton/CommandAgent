use std::path::Path;
use std::time::Duration;

use crate::minimal_loop::pipeline_probe::{self, PipelineProbeConfig};
use crate::planner::failure_vocabulary::rerun_id;

use super::{RERUN_CONSISTENCY_EVIDENCE_PATH, RerunConsistencyEvidence};

pub fn check_rerun_consistency(
    root: &Path,
    entry: &str,
    timeout: Duration,
) -> anyhow::Result<RerunConsistencyEvidence> {
    check_rerun_consistency_with_args(root, entry, std::iter::empty::<&str>(), timeout)
}

pub fn check_rerun_consistency_with_args<'a>(
    root: &Path,
    entry: &str,
    args: impl IntoIterator<Item = &'a str>,
    timeout: Duration,
) -> anyhow::Result<RerunConsistencyEvidence> {
    let args = args.into_iter().map(str::to_string).collect::<Vec<_>>();
    let mut evidence = RerunConsistencyEvidence {
        capability_id: "data_rerun_consistency".to_string(),
        status: "failed".to_string(),
        ok: false,
        entry: entry.to_string(),
        pipeline_run_ok: false,
        baseline_results: None,
        rerun_results: None,
        failure_kinds: Vec::new(),
    };
    if !args.is_empty() {
        run_bound_baseline(root, entry, &args, timeout, &mut evidence);
    }
    match super::results_schema::load(root) {
        Ok(results) => evidence.baseline_results = Some(results),
        Err(error) => evidence
            .failure_kinds
            .push(rerun_id!("baseline_results:{error}")),
    }
    if evidence.baseline_results.is_some() {
        match pipeline_probe::run(
            root,
            PipelineProbeConfig::new(entry)
                .with_args(args)
                .with_timeout(timeout),
        ) {
            Ok(report) => {
                evidence.pipeline_run_ok = report.ok;
                if !report.ok {
                    evidence
                        .failure_kinds
                        .push(rerun_id!("pipeline_run:{}", report.failure_kinds.join(",")));
                }
            }
            Err(error) => evidence
                .failure_kinds
                .push(rerun_id!("pipeline_run_error:{error}")),
        }
        match super::results_schema::load(root) {
            Ok(results) => evidence.rerun_results = Some(results),
            Err(error) => evidence
                .failure_kinds
                .push(rerun_id!("rerun_results:{error}")),
        }
    }
    if let (Some(baseline), Some(rerun)) = (&evidence.baseline_results, &evidence.rerun_results)
        && !crate::minimal_loop::rerun_consistency::reproduced(baseline, rerun)
    {
        evidence
            .failure_kinds
            .push("rerun_consistency_violation:results_changed".to_string());
    }
    evidence.ok = evidence.failure_kinds.is_empty()
        && evidence.pipeline_run_ok
        && evidence.rerun_results.is_some();
    evidence.status = super::status(evidence.ok);
    super::write_evidence(root, RERUN_CONSISTENCY_EVIDENCE_PATH, &evidence)?;
    Ok(evidence)
}

fn run_bound_baseline(
    root: &Path,
    entry: &str,
    args: &[String],
    timeout: Duration,
    evidence: &mut RerunConsistencyEvidence,
) {
    match pipeline_probe::run(
        root,
        PipelineProbeConfig::new(entry)
            .with_args(args.iter().cloned())
            .with_timeout(timeout),
    ) {
        Ok(report) if report.ok => {}
        Ok(report) => evidence.failure_kinds.push(rerun_id!(
            "bound_baseline_pipeline_run:{}",
            report.failure_kinds.join(",")
        )),
        Err(error) => evidence
            .failure_kinds
            .push(rerun_id!("bound_baseline_pipeline_run_error:{error}")),
    }
}
