//! What a run's wallet held at the end of the run, for its summary (Issue #1015).
//!
//! The summary never reads a ledger: a ledger goes on across runs, so reading one later would
//! report another run's state. Instead the run itself records, once at its end, the
//! `run_budget_summary` event, and the summary reads that event alone.
//!
//! Every value here is one the wallet already holds, through its own reducers: the cost limit of
//! the declaration, [`WalletState::spent`] or [`WalletStateV2::spent`], `held`, each send's state
//! and unknown cause, the latch, and the refusal the handle kept for the command. Nothing is
//! estimated, nothing is priced again, and no limit is inferred from the amounts: `stop` is only
//! a refusal the wallet really made. When the wallet cannot be read in time, or its file is no
//! longer what the handle wrote, the record says `unavailable` and carries no amount at all.

use std::sync::{Mutex, MutexGuard, TryLockError};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::ledger::LedgerFailure;
use super::ledger_v2::WalletStateV2;
use super::reservation::{AttemptState, BudgetRefusal, LatchReason, Money, WalletState};
use super::responses::AccountingBasis;
use super::transport::{ObservedWallet, RunBudgetHandle, SendRefusal};

/// The event the end of a run records.
pub(crate) const EVENT: &str = "run_budget_summary";

/// The whole time the end of a run waits for the locks a send or a stage call may hold, in the
/// normal path and on SIGINT: one deadline shared by the wallet and the stages, not one per lock.
pub(crate) const END_OF_RUN_WAIT: Duration = Duration::from_secs(1);

/// The lock, if it is free before `deadline`. A poisoned lock is no lock: what it guards may be
/// half changed, and a summary does not vouch for it.
pub(crate) fn lock_within<T>(mutex: &Mutex<T>, deadline: Instant) -> Option<MutexGuard<'_, T>> {
    loop {
        match mutex.try_lock() {
            Ok(guard) => return Some(guard),
            Err(TryLockError::Poisoned(_)) => return None,
            Err(TryLockError::WouldBlock) if Instant::now() >= deadline => return None,
            Err(TryLockError::WouldBlock) => std::thread::sleep(Duration::from_millis(5)),
        }
    }
}

/// Whether a record was taken at the end of this run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SummaryRecord {
    Observed,
    /// Nothing is known of the amounts: the lock was not free before the deadline (a send held
    /// it when SIGINT came), it was poisoned, the ledger file is not what the handle wrote, a sum
    /// overflowed, or (in the summary) the recorded body could not be read back. There is no
    /// amount, and no zero in its place.
    Unavailable,
}

/// The wallet at the end of a run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct RunBudgetSummary {
    pub(crate) record: SummaryRecord,
    /// Always recorded; `None` only for a recorded event the summary could not read back.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) accounting_basis: Option<AccountingBasis>,
    /// The handle's typed refusal, read without the ledger lock, so it is kept in an
    /// `unavailable` record too. `null` when the wallet refused nothing in this command.
    #[serde(default)]
    pub(crate) stop: Option<StopSummary>,
    /// Present only when `record` is `observed`.
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub(crate) observed: Option<ObservedBudget>,
}

impl RunBudgetSummary {
    /// What the summary reads back from a recorded event. A record that says `unavailable` keeps
    /// no amount, and one that says `observed` but whose body does not decode (a word the scrub
    /// rewrote, an edited file) is reported as `unavailable`, in the JSON and in `summary.md`
    /// alike. An event that does not decode at all is `unavailable` with nothing else.
    pub(crate) fn read_back(event: &Value) -> Self {
        match serde_json::from_value::<Self>(event.clone()) {
            Ok(mut summary) => {
                if summary.record == SummaryRecord::Unavailable || summary.observed.is_none() {
                    summary.record = SummaryRecord::Unavailable;
                    summary.observed = None;
                }
                summary
            }
            Err(_) => Self {
                record: SummaryRecord::Unavailable,
                accounting_basis: None,
                stop: None,
                observed: None,
            },
        }
    }
}

/// What an observed record holds. Amounts are decimal text in dollars, never a float.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ObservedBudget {
    pub(crate) budget_id: String,
    pub(crate) policy_sha256: String,
    pub(crate) workspace_sha256: String,
    pub(crate) limit_usd: String,
    /// v1: the sum of each settled send's consumed amount. v2: the sum of each send's charge.
    pub(crate) spent_usd: String,
    pub(crate) held_usd: String,
    /// v2 only: the operator-declared tariff applied to verified usage, summed over the sends.
    /// Not a provider bill. Under v2 it is what `spent_usd` is.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) declared_tariff_charge_usd_total: Option<String>,
    pub(crate) sends: SendCounts,
    pub(crate) unknown: Vec<UnknownSend>,
    pub(crate) latch: Option<LatchSummary>,
}

/// The wallet's sends by state. A send released before it left is not one.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SendCounts {
    pub(crate) settled: u64,
    pub(crate) reserved: u64,
    pub(crate) started: u64,
    pub(crate) unknown: u64,
}

/// One send left unknown, with its cause in the ledger's own words: v1's `UnknownReason`, or
/// v2's `UnknownCause`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct UnknownSend {
    pub(crate) physical: u64,
    pub(crate) cause: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct LatchSummary {
    pub(crate) cause: LatchReason,
    pub(crate) physical: u64,
}

/// The first send of the command the wallet refused, as the handle kept it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct StopSummary {
    /// `limit` when a declared limit refused it, `refused` for any other refusal.
    pub(crate) kind: String,
    /// The declaration's name of the limit, only when `kind` is `limit`.
    pub(crate) limit: Option<String>,
    /// The refusal in its fixed words, as the run's error says it.
    pub(crate) refusal: String,
}

/// The declaration's name of the limit a refusal is about, if it is about one.
const fn limit_name(refusal: BudgetRefusal) -> Option<&'static str> {
    match refusal {
        BudgetRefusal::CostExceeded => Some("cost_usd"),
        BudgetRefusal::TokensExceeded => Some("tokens"),
        BudgetRefusal::PhysicalExceeded => Some("physical_sends"),
        BudgetRefusal::LogicalGlobalExceeded => Some("logical_global"),
        BudgetRefusal::LogicalStageExceeded => Some("logical_per_stage"),
        BudgetRefusal::WallExceeded => Some("wall_seconds"),
        BudgetRefusal::FinalRepairAttemptsExceeded => Some("final_repair_attempts"),
        BudgetRefusal::FinalRepairWallExceeded => Some("final_repair_seconds"),
        _ => None,
    }
}

fn stop_of(refusal: SendRefusal) -> StopSummary {
    let limit = match refusal {
        SendRefusal::Wallet(LedgerFailure::Budget(budget)) => limit_name(budget),
        _ => None,
    };
    StopSummary {
        kind: if limit.is_some() { "limit" } else { "refused" }.to_string(),
        limit: limit.map(str::to_string),
        refusal: format!("{refusal:?}"),
    }
}

fn count(sends: &mut SendCounts, state: AttemptState) {
    match state {
        AttemptState::Settled => sends.settled += 1,
        AttemptState::Reserved => sends.reserved += 1,
        AttemptState::Started => sends.started += 1,
        AttemptState::Unknown => sends.unknown += 1,
        AttemptState::Released => {}
    }
}

fn latch_of(latch: Option<&super::reservation::Latch>) -> Option<LatchSummary> {
    latch.map(|latch| LatchSummary {
        cause: latch.reason,
        physical: latch.physical.0,
    })
}

/// A v1 wallet, by its own reducers. `None` when a sum overflows: there is then no amount to show.
pub(super) fn of_v1(wallet: &WalletState, limit: Money) -> Option<ObservedBudget> {
    let mut sends = SendCounts::default();
    let mut unknown = Vec::new();
    for attempt in &wallet.attempts {
        count(&mut sends, attempt.state);
        if attempt.state == AttemptState::Unknown {
            unknown.push(UnknownSend {
                physical: attempt.physical.0,
                cause: serde_json::to_value(attempt.unknown_reason).ok()?,
            });
        }
    }
    Some(ObservedBudget {
        budget_id: wallet.budget_id.clone(),
        policy_sha256: wallet.policy_sha256.clone(),
        workspace_sha256: wallet.workspace_sha256.clone(),
        limit_usd: limit.decimal_text(),
        spent_usd: wallet.spent().ok()?.decimal_text(),
        held_usd: wallet.held().ok()?.decimal_text(),
        declared_tariff_charge_usd_total: None,
        sends,
        unknown,
        latch: latch_of(wallet.latch()),
    })
}

/// A v2 wallet, by its own reducers. Its spending is the sum of the tariff's charges, so the
/// declared tariff charge is that same sum, under its own name.
pub(super) fn of_v2(wallet: &WalletStateV2, limit: Money) -> Option<ObservedBudget> {
    let mut sends = SendCounts::default();
    let mut unknown = Vec::new();
    for attempt in &wallet.attempts {
        count(&mut sends, attempt.state);
        if attempt.state == AttemptState::Unknown {
            unknown.push(UnknownSend {
                physical: attempt.physical.0,
                cause: serde_json::to_value(attempt.unknown).ok()?,
            });
        }
    }
    let charged = wallet.spent().ok()?.decimal_text();
    Some(ObservedBudget {
        budget_id: wallet.budget_id.clone(),
        policy_sha256: wallet.policy_sha256.clone(),
        workspace_sha256: wallet.workspace_sha256.clone(),
        limit_usd: limit.decimal_text(),
        spent_usd: charged.clone(),
        held_usd: wallet.held().ok()?.decimal_text(),
        declared_tariff_charge_usd_total: Some(charged),
        sends,
        unknown,
        latch: latch_of(wallet.latch()),
    })
}

/// The wallet of `handle` now, waiting for its ledger lock no later than `deadline`. It reads and
/// changes nothing. The typed refusal is the handle's own, behind no ledger lock, so it is kept
/// whatever the ledger answers.
pub(crate) fn snapshot(handle: &RunBudgetHandle, deadline: Instant) -> RunBudgetSummary {
    let accounting_basis = handle.accounting_basis();
    let stop = handle.refused_in_command().map(stop_of);
    let observed = handle
        .observed_within(deadline)
        .and_then(|observed| match observed {
            ObservedWallet::Chat { wallet, limit } => of_v1(&wallet, limit),
            ObservedWallet::Responses { wallet, limit } => of_v2(&wallet, limit),
        });
    RunBudgetSummary {
        record: if observed.is_some() {
            SummaryRecord::Observed
        } else {
            SummaryRecord::Unavailable
        },
        accounting_basis: Some(accounting_basis),
        stop,
        observed,
    }
}

/// Every word the declaration names a limit by, for a test that holds them reserved.
#[cfg(test)]
pub(crate) fn limit_names() -> Vec<&'static str> {
    [
        BudgetRefusal::CostExceeded,
        BudgetRefusal::TokensExceeded,
        BudgetRefusal::PhysicalExceeded,
        BudgetRefusal::LogicalGlobalExceeded,
        BudgetRefusal::LogicalStageExceeded,
        BudgetRefusal::WallExceeded,
        BudgetRefusal::FinalRepairAttemptsExceeded,
        BudgetRefusal::FinalRepairWallExceeded,
    ]
    .into_iter()
    .filter_map(limit_name)
    .chain(["limit", "refused"])
    .collect()
}

/// The event of a snapshot.
pub(crate) fn event(summary: &RunBudgetSummary) -> Value {
    let mut value = serde_json::to_value(summary).unwrap_or(Value::Null);
    if let Value::Object(object) = &mut value {
        object.insert("event".to_string(), Value::String(EVENT.to_string()));
    }
    value
}

#[cfg(test)]
#[path = "summary_tests.rs"]
mod tests;
