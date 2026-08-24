//! Persisted, per-run input for a requester-supplied reproducer.
//!
//! Workflow nodes receive this binding through their origin-confined run state
//! directory. Ordinary single-intent runs have no such file and retain their
//! existing goal/profile-based reproducer resolution.

use serde::{Deserialize, Serialize};

use crate::config::Config;

const FILE_NAME: &str = "externally-bound-reproducer.json";
const WORKFLOW_NODE_MARKER: &str = "workflow-node";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct ExternalReproducerBinding {
    pub basis: String,
    pub command: String,
    pub lineage: String,
}

impl ExternalReproducerBinding {
    pub(crate) fn new(
        basis: impl Into<String>,
        command: impl Into<String>,
    ) -> Result<Self, String> {
        let basis = basis.into();
        let command = command.into();
        if basis.trim().is_empty() || command.trim().is_empty() || command.lines().count() != 1 {
            return Err("invalid externally-bound reproducer".into());
        }
        let command = crate::planner::verify::normalize_verify_command(&command)
            .map_err(|error| format!("invalid externally-bound reproducer: {error}"))?
            .into_string();
        let lineage = crate::planner::adjudication::fix::reproducer_lineage(&command);
        Ok(Self {
            basis,
            command,
            lineage,
        })
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        let rebuilt = Self::new(self.basis.clone(), self.command.clone())?;
        if rebuilt != *self {
            return Err("externally-bound reproducer lineage mismatch".into());
        }
        Ok(())
    }
}

pub(crate) fn write(config: &Config, binding: &ExternalReproducerBinding) -> Result<(), String> {
    binding.validate()?;
    std::fs::create_dir_all(&config.state_dir).map_err(|error| error.to_string())?;
    std::fs::write(
        config.state_dir.join(FILE_NAME),
        serde_json::to_vec_pretty(binding).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())
}

pub(crate) fn mark_workflow_node(config: &Config) -> Result<(), String> {
    std::fs::create_dir_all(&config.state_dir).map_err(|error| error.to_string())?;
    std::fs::write(config.state_dir.join(WORKFLOW_NODE_MARKER), b"1\n")
        .map_err(|error| error.to_string())
}

pub(crate) fn is_workflow_node(config: &Config) -> bool {
    config.state_dir.join(WORKFLOW_NODE_MARKER).is_file()
}

pub(crate) fn read(config: &Config) -> Result<Option<ExternalReproducerBinding>, String> {
    let path = config.state_dir.join(FILE_NAME);
    if !path.is_file() {
        return Ok(None);
    }
    let binding: ExternalReproducerBinding =
        serde_json::from_slice(&std::fs::read(path).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
    binding.validate()?;
    Ok(Some(binding))
}

pub(crate) fn resolve_candidate(
    config: &Config,
    fallback: Option<crate::planner::profile::ProfileFixReproducerSuggestion>,
) -> Result<Option<(String, String)>, String> {
    if is_workflow_node(config)
        && let Some(binding) = read(config)?
    {
        return Ok(Some((binding.basis, binding.command)));
    }
    Ok(fallback.and_then(|suggestion| {
        primary_command(&suggestion.suggestion).map(|command| (suggestion.basis, command))
    }))
}

pub(crate) fn lineage_for_execution(config: &Config, command: &str) -> Result<String, String> {
    let Some(binding) = is_workflow_node(config)
        .then(|| read(config))
        .transpose()?
        .flatten()
    else {
        return Ok(crate::planner::adjudication::fix::reproducer_lineage(
            command,
        ));
    };
    if binding.command != command {
        return Err("externally-bound reproducer command mismatch".into());
    }
    Ok(binding.lineage)
}

fn primary_command(suggestion: &str) -> Option<String> {
    let candidate = suggestion.split(" | ").next()?.trim();
    let (_, command) = candidate.split_once(" => ")?;
    let command = command.trim();
    (!command.is_empty() && command.lines().count() == 1).then(|| command.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn state_binding_round_trips_and_rejects_a_changed_lineage() {
        let root = tempfile::tempdir().unwrap();
        let cwd = root.path().to_string_lossy();
        let config = Config::from_cli(crate::cli::Cli::parse_from([
            "commandagent",
            "--cwd",
            cwd.as_ref(),
            "--state-dir",
            root.path().join("state").to_string_lossy().as_ref(),
            "goal",
        ]))
        .unwrap();
        let binding =
            ExternalReproducerBinding::new("origin:pipeline_probe", "python3 -B pipeline/main.py")
                .unwrap();
        write(&config, &binding).unwrap();
        assert_eq!(read(&config).unwrap(), Some(binding.clone()));
        assert_eq!(resolve_candidate(&config, None).unwrap(), None);
        mark_workflow_node(&config).unwrap();
        assert_eq!(
            resolve_candidate(&config, None).unwrap(),
            Some((binding.basis.clone(), binding.command.clone()))
        );

        let mut invalid = binding;
        invalid.lineage = "different".into();
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn investigation_binding_is_reused_for_fix_f1_with_the_same_lineage() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("pipeline")).unwrap();
        std::fs::create_dir_all(root.path().join("output")).unwrap();
        std::fs::create_dir_all(root.path().join("data")).unwrap();
        std::fs::write(
            root.path().join("pipeline/main.py"),
            "raise RuntimeError('origin failure')\n",
        )
        .unwrap();
        std::fs::write(root.path().join("output/results.json"), "{}\n").unwrap();
        std::fs::write(
            root.path().join("data/sales.csv"),
            "region,amount\n東京,10\n",
        )
        .unwrap();
        let cwd = root.path().to_string_lossy();
        let goal = "derived workflow fix goal without reproducer vocabulary";
        let mut config = Config::from_cli(crate::cli::Cli::parse_from([
            "commandagent",
            "--cwd",
            cwd.as_ref(),
            "--offline",
            "--profile",
            "data",
            "--intent",
            "fix",
            "--plan-preset",
            "profile",
            "--ultra-plan",
            goal,
        ]))
        .unwrap();
        config.state_dir = root.path().join(".anvil/runs/fix-node/state");
        let binding =
            ExternalReproducerBinding::new("investigation_i1", "python3 -B pipeline/main.py")
                .unwrap();
        mark_workflow_node(&config).unwrap();
        write(&config, &binding).unwrap();
        let plan = crate::planner::intent::explicit_fix_plan(goal, "data", "default");
        let phase = &plan.phases[0];
        let mut runtime =
            crate::planner::fix_runtime::FixRuntime::for_plan(&plan, &config).unwrap();
        let before = match crate::planner::fix_plan_synthesis::phase_plan(
            &config,
            &plan,
            phase,
            Some(&runtime),
        )
        .unwrap()
        {
            crate::planner::fix_plan_synthesis::PhasePlan::Generated(plan) => plan,
            _ => panic!("external R must produce the fixed F1 plan"),
        };
        assert_eq!(
            before.steps[0].verify.as_slice(),
            std::slice::from_ref(&binding.command)
        );
        assert_eq!(
            runtime
                .run_before_phase(&before, &config, &plan, phase, 0)
                .unwrap(),
            crate::planner::fix_reproducer_defect::BeforePhaseOutcome::Confirmed
        );
        let evidence_path = runtime.before_evidence_path().unwrap();
        let evidence: crate::planner::adjudication::fix::FixEvidenceObservation =
            serde_json::from_slice(&std::fs::read(root.path().join(evidence_path)).unwrap())
                .unwrap();
        assert_eq!(evidence.binding_id, binding.command);
        assert_eq!(evidence.lineage, binding.lineage);
    }
}
