//! One guard for a command whose wallet refused it (Issue #976, G5: H1, H3, R1), and what a later
//! session, profile, or resume of a command asks of the wallet it already holds (A).
//!
//! Once the wallet refused anything of a command, nothing more of that command starts: no provider
//! send, final repair or not (`transport.rs`, where every send begins), no reconciliation of
//! dependencies (`ultra_final_acceptance_report_with_deterministic_remedies`), no run of a plan
//! (`run_ultra_plan_with_ui`), and no recovery run (`auto_recovery.rs`, where a failure is told
//! recoverable). Each asks [`refusal_of`], and nothing else decides it. The refusal is the
//! command's own: a send keeps the number of the command it began in, so a late worker of an
//! earlier command that is refused now lands on none.
//!
//! A REPL's later profile and `/resume` go on in the wallet the handle holds: nothing here opens
//! one again. With no wallet, every function returns at its first line and reads nothing.

use super::entry::EntryRefusal;
use super::outcome::CommandRefusal;
use super::transport::{RunBudgetHandle, SendRefusal};
use crate::config::Config;

/// The one judgement: the refusal kept for command `command` of this wallet, if any.
pub(super) fn refusal_of(handle: &RunBudgetHandle, command: u64) -> Option<SendRefusal> {
    handle.refusal_kept_for(command)
}

/// The refusal of the command running now, if its wallet refused anything of it.
pub(crate) fn refused(config: &Config) -> Option<SendRefusal> {
    let handle = config.field_sources.run_budget.as_ref()?;
    refusal_of(handle, handle.command_now())
}

/// The guard, as the error of a step that returns one: the command's refusal, in fixed words.
pub(crate) fn guard(config: &Config) -> anyhow::Result<()> {
    refused(config).map_or(Ok(()), |refused| {
        Err(anyhow::Error::new(CommandRefusal::SendRefused(refused)))
    })
}

/// `/resume` goes on in the wallet the handle holds, and only in one with a run before it: a wallet
/// this process made is refused as a resume with no record would be. Nothing is opened again.
pub(crate) fn refuse_resume(config: &Config) -> anyhow::Result<()> {
    let Some(handle) = config.field_sources.run_budget.as_ref() else {
        return Ok(());
    };
    handle
        .refuse_resume()
        .map_err(|refusal| anyhow::Error::new(EntryRefusal::Wallet(refusal)))
}

/// A profile named after the wallet opened is pinned in it as one named at the start would be:
/// the first once, another refused, one with nothing in its name refused. Only an explicit one.
pub(crate) fn pin_profile(config: &Config) -> anyhow::Result<()> {
    let Some(handle) = config.field_sources.run_budget.as_ref() else {
        return Ok(());
    };
    if !config.profile_explicit {
        return Ok(());
    }
    handle
        .pin_profile(&config.profile)
        .map_err(|refusal| anyhow::Error::new(EntryRefusal::Wallet(refusal)))
}

#[cfg(test)]
#[path = "command_tests.rs"]
mod tests;
