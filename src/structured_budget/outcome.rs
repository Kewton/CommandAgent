//! Whether a command under a wallet may be called a success (Issue #976, G5: N13).
//!
//! A send the wallet refused ends its command as no success, whatever came after it: a caller that
//! passed the refusal over as text, as nothing, or as `false`, a later send that went through, and
//! a stage set with nothing left unmet change nothing. Neither may a command succeed while its
//! wallet cannot be called settled (a latch, or a send unresolved).
//!
//! The refusal is kept where the send was refused, in its fixed words, before anything could read
//! it: on the handle, in memory. Nothing of it goes to the ledger, and it stops no later send: the
//! wallet judges each send as before. A command line runs one command, and keeps it to the end. A
//! REPL begins each command with none, so a command is judged by its own refusals and by what the
//! wallet holds then; with the wallet spent, its sends are refused again. Nothing ends a command's
//! record earlier: a flow run again within one command still has it.
//!
//! G5 is asked before G4. Every gate that can call a session, a plan, or a run a success asks here
//! first, and only a success can become a refusal here: an error stays the error it was. With no
//! wallet, every function returns at its first line and reads nothing.

use super::transport::SendRefusal;
use crate::config::Config;

/// Why a command under a wallet is no success. Fixed words only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CommandRefusal {
    /// The wallet refused a send of this command.
    SendRefused(SendRefusal),
    /// The wallet may not be called settled.
    WalletUnsettled(SendRefusal),
}

impl std::fmt::Display for CommandRefusal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "the run budget refused the run: {self:?}")
    }
}

impl std::error::Error for CommandRefusal {}

/// G5's answer for the command running now.
pub(crate) fn refusal(config: &Config) -> Option<CommandRefusal> {
    let handle = config.field_sources.run_budget.as_ref()?;
    if let Some(refused) = handle.refused_in_command() {
        return Some(CommandRefusal::SendRefused(refused));
    }
    handle
        .success_gate()
        .err()
        .map(CommandRefusal::WalletUnsettled)
}

/// G5, as the error of a gate that returns one.
pub(crate) fn gate(config: &Config) -> anyhow::Result<()> {
    refusal(config).map_or(Ok(()), |refusal| Err(anyhow::Error::new(refusal)))
}

/// G5 on how a session ended: a success may become a refusal, an error stays as it is.
pub(crate) fn gate_session<T>(result: anyhow::Result<T>, config: &Config) -> anyhow::Result<T> {
    let outcome = result?;
    gate(config)?;
    Ok(outcome)
}

/// G5, for a gate whose answer is text.
pub(crate) fn refusal_text(config: &Config) -> Option<String> {
    refusal(config).map(|refusal| refusal.to_string())
}

/// A REPL's next command: the refusals of the commands before it are theirs.
pub(crate) fn begin_command(config: &Config) {
    if let Some(handle) = config.field_sources.run_budget.as_ref() {
        handle.forget_command_refusals();
    }
}

#[cfg(test)]
#[path = "outcome_tests.rs"]
mod tests;
