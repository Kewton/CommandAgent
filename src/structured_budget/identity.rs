//! Who a send and a submission are.
//!
//! The ledger issues the IDs from its own records, so nothing a model returns can name one. A
//! provider send is a physical ID, issued before the send. A logical ID is one submission, which
//! is one tool call the host looked at: a send opens none, and a response with N calls is N
//! submissions of one physical send. A [`SubmissionSource`] ties a submission to that send, and it
//! can only come from a settled response.
//!
//! A final repair is the one count a send opens: a repair that returns only text is still a
//! repair, and its tool calls do not add to it. It has its own ID, apart from the logical ones.
//!
//! G4's attempt identity exists only after a tool call was inspected, too late to key a
//! reservation, so it is bound to a submission afterwards.

use serde::{Deserialize, Serialize};

/// One HTTP POST.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct PhysicalId(pub(super) u64);

/// One submission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct LogicalId(pub(super) u64);

/// One final repair, however many sends it takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct FinalRepairId(pub(super) u64);

/// What a send is for. A label, except that a final repair must say so and must hold a
/// [`FinalRepairClaim`], and nothing else may.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SendPurpose {
    Planner,
    Executor,
    Repair,
    FinalRepair,
    Classifier,
}

/// Which counters a submission draws on. The stage is G4's opaque name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum LogicalKind {
    General,
    Stage { stage: String },
}

/// A send of a final repair either opens one or continues one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FinalRepairClaim {
    New,
    Continue(FinalRepairId),
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
