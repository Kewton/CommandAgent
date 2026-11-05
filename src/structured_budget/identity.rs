//! Who a send and a submission are.
//!
//! The ledger issues both IDs from its own records, before a send, so nothing a model returns can
//! name one. A provider send is a physical ID. The tool calls one response carries are submissions
//! of that one physical send, and a [`SubmissionSource`] can only come from a settled response.
//!
//! G4's attempt identity exists only after a tool call was inspected, too late to key a
//! reservation, so it is bound to a submission afterwards.

use serde::{Deserialize, Serialize};

/// One HTTP POST.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct PhysicalId(pub(super) u64);

/// One logical request or submission, which a retry of the send continues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct LogicalId(pub(super) u64);

/// What a send is for. A label, except that a final repair must say so (see [`LogicalKind`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SendPurpose {
    Planner,
    Executor,
    Repair,
    FinalRepair,
    Classifier,
}

/// Which counters a logical request draws on. The stage is G4's opaque name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum LogicalKind {
    General,
    Stage { stage: String },
    FinalRepair,
}

/// A send either opens a logical request or continues one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LogicalClaim {
    New(LogicalKind),
    Continue(LogicalId),
}

/// The one physical send behind a settled response. Only settling gives one out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ResponseIdentity {
    physical: PhysicalId,
}

impl ResponseIdentity {
    pub(super) fn of_settled(physical: PhysicalId) -> Self {
        Self { physical }
    }

    pub(crate) fn physical(self) -> PhysicalId {
        self.physical
    }

    /// One tool call of this response. However many there are, the physical send is this one.
    pub(crate) fn call(self, call_index: u32) -> SubmissionSource {
        SubmissionSource {
            physical: self.physical,
            call_index,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SubmissionSource {
    pub(super) physical: PhysicalId,
    pub(super) call_index: u32,
}

/// The fields of G4's `AttemptIdentity`, kept apart so that this stage changes nothing of G4's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AttemptBinding {
    pub(crate) run_nonce: String,
    pub(crate) stage: String,
    pub(crate) target: String,
    pub(crate) attempt: u32,
    pub(crate) generation: u64,
}
