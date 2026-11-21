//! Every profile tool call a run looks at, counted under its wallet (Issue #976, G5).
//!
//! A response with N calls of a profile tool is N submissions of its one physical send. Each is
//! recorded when the tool is reached, before anything of the call is judged: a call the schema
//! refuses, a malformed one, and one no response stands behind are submissions just the same, and
//! each takes from the counts and the wall. A call whose record is refused does not run, and its
//! command is no success (`outcome.rs`). Once the wallet refused anything of a command, nothing of
//! that command is recorded any more, and nothing of it runs.
//!
//! Which response a call came in is the host's to say, never the model's: the ID of the call, the
//! ID of a request, and the last send the wallet settled are not it. The worker of the provider
//! call that settled the send hands its identity back with the reply it read, so the reply of
//! another call, a late one, a classifier's, or a prefetch's, cannot stand in for it. The call's
//! place in the reply is the host's count.
//!
//! When G4 lets an attempt of a stage start, that attempt is tied to the call's submission before
//! its process starts. A replay, a stage left as it was, a revision, and a resume add submissions
//! and give none back.
//!
//! With no wallet, [`for_call`] returns at its first line and nothing here is read.

use std::sync::Arc;

use super::identity::{AttemptBinding, LogicalId, LogicalKind, ResponseIdentity, SubmissionSource};
use super::transport::{RunBudgetHandle, SendRefusal};
use crate::config::Config;

/// One tool call under the run's wallet: the wallet, and the response it came in when one did.
#[derive(Debug, Clone)]
pub(crate) struct CallBudget {
    handle: Arc<RunBudgetHandle>,
    source: Option<SubmissionSource>,
}

/// The call at `call_index` of `response`, under the run's wallet. `None` when the run has none.
/// A call with no response behind it, one the host made, is still counted, as no response's.
pub(crate) fn for_call(
    config: &Config,
    response: Option<ResponseIdentity>,
    call_index: usize,
) -> Option<CallBudget> {
    let handle = config.field_sources.run_budget.as_ref()?;
    // A place past what the ledger can hold names no place twice: the last one is taken once.
    let index = u32::try_from(call_index).unwrap_or(u32::MAX);
    Some(CallBudget {
        handle: handle.clone(),
        source: response.map(|response| response.call(index)),
    })
}

impl CallBudget {
    /// The call as one submission of `kind`, on disk before it is judged.
    pub(crate) fn record(&self, kind: LogicalKind) -> Result<Submission, SendRefusal> {
        let logical = self.handle.record_submission(kind, self.source)?;
        Ok(Submission {
            handle: self.handle.clone(),
            logical,
        })
    }
}

/// A call the wallet counted.
#[derive(Debug)]
pub(crate) struct Submission {
    handle: Arc<RunBudgetHandle>,
    logical: LogicalId,
}

impl Submission {
    /// Ties the attempt G4 just let start to this submission. No attempt to tie is a refusal too:
    /// a process with no attempt the wallet knows of does not start.
    pub(crate) fn bind(&self, attempt: Option<AttemptBinding>) -> Result<(), SendRefusal> {
        match attempt {
            Some(binding) => self.handle.bind_attempt(self.logical, binding),
            None => Err(self.handle.refuse_for_command(SendRefusal::Unavailable)),
        }
    }
}

#[cfg(test)]
#[path = "submission_tests.rs"]
mod tests;
