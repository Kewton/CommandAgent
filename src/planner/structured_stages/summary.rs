//! What a run's declared stages stood at when the run ended, for its summary (Issue #1015).
//!
//! The summary never reads the stage ledger: a fresh run keeps the previous run's file until its
//! first `begin`, and a later judgment would read the bytes as they are then. Instead the run
//! records, once at its end, the `structured_stages_summary` event, and the summary reads it.
//!
//! The record is the one judgment there is, [`super::ledger::StageLedger::verdict`]'s body, run on
//! a copy of the ledger this process holds ([`super::ledger::StageLedger::observed_verdict`]). It
//! opens no ledger, writes no file, keeps no refusal, and reads the workspace only to compare
//! bytes. A record of a ledger that cannot be vouched for is `verdict_without_ledger`'s, as the
//! verdict's. When this process never opened the ledger (the run ended before its first stage
//! call or gate), nothing of the stages was read, and the record is `unavailable`.
//!
//! What failed in an attempt's checks is recorded where that failure is decided, as the
//! `structured_stage_checks` event: the state machine keeps only the failure's kind.

use std::collections::BTreeSet;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::receipt::{CheckResult, CheckStatus};
use super::state::{AttemptTicket, Phase, StageMachine, StageVerdict};
use super::{BOUND_RUNS, BoundAdmission, StageRun, bound_key, ledger};
use crate::config::Config;
use crate::structured_budget::summary::{SummaryRecord, lock_within};
use crate::tools::profile_tools::selection;

/// The event the end of a run records.
pub(crate) const EVENT: &str = "structured_stages_summary";
/// The event an attempt's failed checks record.
pub(crate) const CHECKS_EVENT: &str = "structured_stage_checks";

/// The declared stages at the end of a run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct StagesSummary {
    pub(crate) record: SummaryRecord,
    /// Present only when `record` is `observed`.
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub(crate) observed: Option<ObservedStages>,
}

impl StagesSummary {
    /// What the summary reads back from a recorded event, held to its `record` as
    /// [`crate::structured_budget::summary::RunBudgetSummary::read_back`] is: an `observed` record
    /// whose body does not decode, or an event that does not decode at all, is `unavailable`.
    pub(crate) fn read_back(event: &Value) -> Self {
        match serde_json::from_value::<Self>(event.clone()) {
            Ok(summary)
                if summary.record == SummaryRecord::Observed && summary.observed.is_some() =>
            {
                summary
            }
            _ => summary_of(None),
        }
    }
}

/// The verdict's answers, as `StageVerdict` gives them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ObservedStages {
    pub(crate) complete: bool,
    pub(crate) ledger_intact: bool,
    pub(crate) contract_sha256: String,
    pub(crate) run_nonce: String,
    pub(crate) stages: Vec<StageEntry>,
}

/// One target of the contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct StageEntry {
    pub(crate) stage: String,
    pub(crate) target: String,
    pub(crate) phase: Phase,
    pub(crate) sealed: bool,
    pub(crate) attempts: AttemptsSummary,
    pub(crate) artifact: ArtifactSummary,
    /// Filled by the summary from the latest `structured_stage_checks` event of this run and
    /// target. Absent when there is none: never an empty value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) failed_checks: Option<FailedChecks>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct AttemptsSummary {
    pub(crate) used: u32,
    pub(crate) limit: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ArtifactSummary {
    /// Workspace-relative, as the contract declares it.
    pub(crate) path: String,
    /// Of the bytes the current receipt adopted; `None` without one.
    pub(crate) committed_sha256: Option<String>,
}

/// What the checks of one failed attempt returned. Only required check IDs are named: an ID the
/// operator did not declare is counted, never quoted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct FailedChecks {
    pub(crate) attempt: u32,
    /// The failure's fixed code, as the tool's answer gave it.
    pub(crate) code: String,
    /// Each returned result of a required check, in the order returned.
    pub(crate) checks: Vec<CheckEntry>,
    /// Required checks with no result.
    pub(crate) missing: Vec<String>,
    /// Results whose ID is not a required check.
    pub(crate) unrequested: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CheckEntry {
    pub(crate) id: String,
    pub(crate) status: CheckStatus,
}

/// The `structured_stage_checks` event of one failed attempt: what its checks returned, which
/// the state machine does not keep. It changes no check and no decision.
pub(crate) fn failed_checks_event(
    ticket: &AttemptTicket,
    required: &BTreeSet<String>,
    results: &[CheckResult],
    code: &str,
) -> Value {
    let checks = results
        .iter()
        .filter(|result| required.contains(&result.id))
        .map(|result| CheckEntry {
            id: result.id.clone(),
            status: result.status,
        })
        .collect::<Vec<_>>();
    let missing = required
        .iter()
        .filter(|id| !results.iter().any(|result| &result.id == *id))
        .cloned()
        .collect::<Vec<_>>();
    let unrequested = results
        .iter()
        .filter(|result| !required.contains(&result.id))
        .count();
    json!({
        "event": CHECKS_EVENT,
        "run_nonce": ticket.run_nonce,
        "stage": ticket.stage,
        "target": ticket.target,
        "attempt": ticket.attempt,
        "code": code,
        "checks": checks,
        "missing": missing,
        "unrequested": unrequested,
    })
}

/// The summary of a verdict and the state it was given on.
pub(super) fn of_verdict(
    verdict: &StageVerdict,
    machine: &StageMachine,
    run_nonce: String,
) -> ObservedStages {
    let contract = machine.contract();
    let stages = verdict
        .obligations()
        .iter()
        .map(|status| {
            let counts = machine.attempt_counts(&status.key);
            let receipt = machine
                .record(&status.key)
                .and_then(|record| record.current_receipt.as_ref());
            StageEntry {
                stage: status.key.stage.clone(),
                target: status.key.target.clone(),
                phase: status.phase,
                sealed: status.phase == Phase::Sealed,
                attempts: AttemptsSummary {
                    used: counts.map_or(0, |counts| counts.target.used),
                    limit: counts.map_or(0, |counts| counts.target.limit),
                },
                artifact: ArtifactSummary {
                    path: contract
                        .target(&status.key)
                        .map(|target| target.output_path.clone())
                        .unwrap_or_default(),
                    committed_sha256: receipt.map(|receipt| receipt.committed_sha256.clone()),
                },
                failed_checks: None,
            }
        })
        .collect();
    ObservedStages {
        complete: verdict.complete(),
        ledger_intact: verdict.ledger_intact(),
        contract_sha256: contract.sha256(),
        run_nonce,
        stages,
    }
}

/// The ledger held for `run`, judged on a copy. `None`: nothing was read, because a lock was not
/// free before `deadline`, no session is bound, or this process never opened the ledger.
fn observe(run: &StageRun, deadline: Instant) -> Option<ObservedStages> {
    lock_within(&run.session, deadline)?.as_ref()?;
    // A ledger no call or gate of this process opened: nothing of it was read.
    let held = run
        .ledgers
        .held_within(&run.workspace, &run.profile, deadline)
        .ok()??;
    let ledger = lock_within(&held, deadline)?;
    let (verdict, machine) = ledger.observed_verdict(&ledger::WorkspaceBytes::new(&run.workspace));
    let run_nonce = machine.snapshot().run_nonce.clone();
    Some(of_verdict(&verdict, &machine, run_nonce))
}

fn summary_of(observed: Option<ObservedStages>) -> StagesSummary {
    StagesSummary {
        record: if observed.is_some() {
            SummaryRecord::Observed
        } else {
            SummaryRecord::Unavailable
        },
        observed,
    }
}

/// The run's declared stages now, or `None` for a run that declared none. A run with no
/// declaration returns at the first line, reading nothing, as the final gate does.
pub(crate) fn snapshot(config: &Config, deadline: Instant) -> Option<StagesSummary> {
    let pack = selection::selection_that_may_hold_stages(&config.profile)?;
    let bound = bound_key(&config.workspace_root, &config.profile, pack).and_then(|key| {
        // A table held past the deadline tells nothing, as no entry would.
        lock_within(&BOUND_RUNS, deadline).and_then(|runs| runs.get(&key).cloned())
    });
    match bound {
        Some(BoundAdmission::Stages(run)) => Some(summary_of(observe(&run, deadline))),
        Some(BoundAdmission::NoStages) => None,
        // Declared, with no session of this process bound to them that can be read: nothing
        // is known of their state.
        None => {
            selection::stages_declared_without_a_read(&config.profile).then(|| summary_of(None))
        }
    }
}

/// The event of a snapshot.
pub(crate) fn event(summary: &StagesSummary) -> Value {
    let mut value = serde_json::to_value(summary).unwrap_or(Value::Null);
    if let Value::Object(object) = &mut value {
        object.insert("event".to_string(), Value::String(EVENT.to_string()));
    }
    value
}

/// One entry of a directory tree as a test that nothing was written compares it: its kind, its
/// permission bits, its modification time, and a file's bytes.
#[cfg(test)]
pub(crate) type TreeEntry = (bool, u32, std::time::SystemTime, Option<Vec<u8>>);

/// Every entry under `root`, directories (empty ones too) and files, by path, with no link
/// followed. Equal trees before and after a call mean the call created, removed, renamed,
/// rewrote, or touched nothing there.
#[cfg(test)]
pub(crate) fn tree_state(
    root: &std::path::Path,
) -> std::collections::BTreeMap<std::path::PathBuf, TreeEntry> {
    use std::os::unix::fs::PermissionsExt;
    let mut found = std::collections::BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(path) = stack.pop() {
        let metadata = std::fs::symlink_metadata(&path).unwrap();
        let is_dir = metadata.is_dir();
        let bytes = if is_dir {
            for entry in std::fs::read_dir(&path).unwrap() {
                stack.push(entry.unwrap().path());
            }
            None
        } else {
            std::fs::read(&path).ok()
        };
        found.insert(
            path,
            (
                is_dir,
                metadata.permissions().mode(),
                metadata.modified().unwrap(),
                bytes,
            ),
        );
    }
    found
}

#[cfg(test)]
#[path = "summary_tests.rs"]
mod tests;
