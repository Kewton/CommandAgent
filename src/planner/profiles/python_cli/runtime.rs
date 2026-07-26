use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};

use super::{argv_probe, help_binding, manifest};
use crate::planner::capability_catalog::{
    CliCapability, CliCheckKind, ProbeCapability, ResolvedCapability,
};

pub const EVIDENCE_PATH: &str = "evidence/cli-assurance.json";
pub const C1: &str = "cli_probe";
pub const C2: &str = "help_binding";
pub const C3: &str = "cli_output_claims";
pub const C4: &str = "cli_rerun_consistency";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    Pass,
    ClaimsAbsent,
    Failed,
    NotExecuted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CliAssurance {
    Full,
    Partial,
    Static,
    Failed,
}

impl CliAssurance {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Partial => "partial",
            Self::Static => "static",
            Self::Failed => "failed",
        }
    }

    pub const fn behavior_status(self) -> &'static str {
        match self {
            Self::Full => "pass",
            Self::Partial => "partial",
            Self::Static => "static",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceState {
    pub probe_attempted: bool,
    pub binding_intact: bool,
    pub checks: BTreeMap<String, CheckStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CliCheckSummary {
    pub status: String,
    pub assurance: CliAssurance,
    pub evidence: EvidenceState,
    pub reasons: Vec<String>,
}

pub fn run_manifest_checks(root: &Path) -> anyhow::Result<CliCheckSummary> {
    let adapters = adapters()?;
    let probe_adapter = adapter(&adapters, CliCheckKind::Probe)?;
    ensure_same_inputs(&adapters, &probe_adapter)?;
    let usage = probe_adapter
        .usage_paths
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let timeout = Duration::from_secs(probe_adapter.timeout_seconds.into());
    let probe = argv_probe::run(
        root,
        argv_probe::Config::new(&probe_adapter.entry, &usage).with_timeout(timeout),
    )?;
    let help = help_binding::run(
        root,
        Path::new(&probe_adapter.entry),
        &probe.binding.cases[0].args,
        timeout,
    )?;
    let claims = if probe.output_claims.is_empty() {
        CheckStatus::ClaimsAbsent
    } else if probe.output_claims.iter().all(|claim| claim.matched) {
        CheckStatus::Pass
    } else {
        CheckStatus::Failed
    };
    let evidence = EvidenceState {
        probe_attempted: true,
        binding_intact: probe.binding_intact,
        checks: BTreeMap::from([
            (C1.to_string(), status(probe.c1_ok)),
            (
                C2.to_string(),
                match help.status.as_str() {
                    "pass" => CheckStatus::Pass,
                    "claims_absent" => CheckStatus::ClaimsAbsent,
                    _ => CheckStatus::Failed,
                },
            ),
            (C3.to_string(), claims),
            (C4.to_string(), status(probe.c4_ok)),
        ]),
    };
    let assurance = classify(&evidence);
    let mut reasons = probe.failure_kinds;
    reasons.extend(help.failure_kinds);
    if claims == CheckStatus::Failed {
        reasons.push("cli_output_claims:observed_stdout_mismatch".to_string());
    }
    reasons.sort();
    reasons.dedup();
    let summary = CliCheckSummary {
        status: assurance.as_str().to_string(),
        assurance,
        evidence,
        reasons,
    };
    argv_probe::write_json(root, EVIDENCE_PATH, &summary)?;
    Ok(summary)
}

pub fn classify(evidence: &EvidenceState) -> CliAssurance {
    if !evidence.probe_attempted {
        return CliAssurance::Static;
    }
    let get = |id| {
        evidence
            .checks
            .get(id)
            .copied()
            .unwrap_or(CheckStatus::NotExecuted)
    };
    let statuses = [get(C1), get(C2), get(C3), get(C4)];
    if !evidence.binding_intact
        || statuses
            .iter()
            .any(|status| matches!(status, CheckStatus::Failed | CheckStatus::NotExecuted))
    {
        return CliAssurance::Failed;
    }
    if statuses.iter().all(|status| *status == CheckStatus::Pass) {
        return CliAssurance::Full;
    }
    if get(C1) == CheckStatus::Pass
        && get(C4) == CheckStatus::Pass
        && get(C2) == CheckStatus::ClaimsAbsent
        && get(C3) == CheckStatus::ClaimsAbsent
    {
        return CliAssurance::Partial;
    }
    CliAssurance::Failed
}

fn status(ok: bool) -> CheckStatus {
    if ok {
        CheckStatus::Pass
    } else {
        CheckStatus::Failed
    }
}

fn adapters() -> anyhow::Result<Vec<CliCapability>> {
    Ok(manifest::get()
        .resolve()?
        .into_values()
        .flatten()
        .filter_map(|check| match check.capability {
            ResolvedCapability::Probe(ProbeCapability::Cli(capability)) => Some(capability),
            _ => None,
        })
        .collect())
}

fn adapter(adapters: &[CliCapability], kind: CliCheckKind) -> anyhow::Result<CliCapability> {
    adapters
        .iter()
        .find(|adapter| adapter.check == kind)
        .cloned()
        .with_context(|| format!("CLI manifest adapter missing: {kind:?}"))
}

fn ensure_same_inputs(adapters: &[CliCapability], expected: &CliCapability) -> anyhow::Result<()> {
    let kinds = adapters
        .iter()
        .map(|adapter| adapter.check)
        .collect::<BTreeSet<_>>();
    if kinds.len() != 4
        || adapters.iter().any(|adapter| {
            adapter.entry != expected.entry
                || adapter.usage_paths != expected.usage_paths
                || adapter.timeout_seconds != expected.timeout_seconds
        })
    {
        bail!("CLI C1-C4 adapters do not share one frozen execution input");
    }
    Ok(())
}
