use std::path::Path;
use std::time::Duration;

use super::super::{checks, internal_checks, manifest};
use crate::minimal_loop::pipeline_probe::{self, PipelineProbeConfig};
use crate::minimal_loop::python_traceback;
use crate::planner::capability_catalog::{InternalCapability, ProbeCapability, ResolvedCapability};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CatalogCheckOutcome {
    pub id: String,
    pub ok: bool,
    pub reasons: Vec<String>,
}

pub(crate) fn execute_catalog_check(
    root: &Path,
    command: &str,
    report: &mut crate::planner::verify::VerificationReport,
    eval_events_path: Option<&Path>,
    goal: Option<&str>,
) -> Option<anyhow::Result<CatalogCheckOutcome>> {
    let id = super::catalog_check_id(command)?.to_string();
    let input = super::input_binding::parts(command)?.1.map(str::to_string);
    Some(execute_bound_check(
        root,
        id,
        input.as_deref(),
        report,
        eval_events_path,
        goal,
    ))
}

fn execute_bound_check(
    root: &Path,
    id: String,
    input: Option<&str>,
    report: &mut crate::planner::verify::VerificationReport,
    eval_events_path: Option<&Path>,
    goal: Option<&str>,
) -> anyhow::Result<CatalogCheckOutcome> {
    if let Some(input) = input {
        crate::tools::path_guard::validate_workspace_relative(input).map_err(|error| {
            anyhow::anyhow!("data probe input is not workspace-relative: {error}")
        })?;
    }
    let resolved = manifest::get()
        .resolve()?
        .into_values()
        .flatten()
        .find(|check| check.id == id)
        .ok_or_else(|| anyhow::anyhow!("data manifest check `{id}` is not bound"))?
        .capability;
    let (ok, reasons) = match resolved {
        ResolvedCapability::Internal(InternalCapability::Data(check)) => {
            if input.is_some() {
                anyhow::bail!("data internal check `{id}` does not accept an input binding");
            }
            internal_checks::execute(root, check, goal)?
        }
        ResolvedCapability::Probe(ProbeCapability::Pipeline {
            entry,
            timeout_seconds,
        }) => {
            let evidence =
                pipeline_probe::run(root, pipeline_probe_config(entry, timeout_seconds, input))?;
            python_traceback::attach_pipeline_report(&evidence, eval_events_path, report);
            (evidence.ok, evidence.failure_kinds)
        }
        ResolvedCapability::Probe(ProbeCapability::DataRerunConsistency {
            entry,
            timeout_seconds,
        }) => {
            let evidence = checks::check_rerun_consistency_with_args(
                root,
                &entry,
                input.into_iter(),
                Duration::from_secs(timeout_seconds.into()),
            )?;
            (evidence.ok, evidence.failure_kinds)
        }
        capability => anyhow::bail!("unsupported data catalog check adapter: {capability:?}"),
    };
    Ok(CatalogCheckOutcome { id, ok, reasons })
}

fn pipeline_probe_config(
    entry: String,
    timeout_seconds: u16,
    input: Option<&str>,
) -> PipelineProbeConfig {
    let config =
        PipelineProbeConfig::new(entry).with_timeout(Duration::from_secs(timeout_seconds.into()));
    match input {
        Some(input) => config.with_args([input.to_string()]),
        None => config,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rerun_input_binding_rejects_paths_outside_the_workspace() {
        let root = tempfile::tempdir().unwrap();
        let mut report = crate::planner::verify::VerificationReport::pass();

        let error = execute_bound_check(
            root.path(),
            "data_rerun_consistency".to_string(),
            Some("../outside.csv"),
            &mut report,
            None,
            None,
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains("not workspace-relative"), "{error}");
    }
}
