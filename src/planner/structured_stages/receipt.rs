//! What one sealed target is bound to (Issue #975, G4).
//!
//! A [`StageReceipt`] wraps G3's [`CommitReceipt`] without changing it and shares its
//! `committed_sha256`. There is no field a tool or a model could set to say a result was
//! taken: every value comes from the host's attempt record or from the commit.
//!
//! The commit receipt lives in memory only. A record read back from a ledger has none, and a
//! saved record can only lower a state ([`super::resume`]).

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::tools::profile_tools::commit::CommitReceipt;
use crate::tools::profile_tools::protocol::RECEIPT_PROTOCOL;

pub(crate) const STAGE_RECEIPT_SCHEMA: &str = "structured-stage.receipt/v1";

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(crate) fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CheckStatus {
    Passed,
    Failed,
    NotRun,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CheckResult {
    pub(crate) id: String,
    pub(crate) status: CheckStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ToolPins {
    pub(crate) tool_id: String,
    pub(crate) tool_version: String,
    pub(crate) descriptor_sha256: String,
    pub(crate) runtime_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct InputDigest {
    pub(crate) path: String,
    pub(crate) sha256: String,
}

/// The upstream version an attempt ran against.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DependencyPin {
    pub(crate) stage: String,
    pub(crate) target: String,
    pub(crate) generation: u64,
    pub(crate) committed_sha256: String,
}

fn digest_of<T: Serialize>(items: &[T]) -> String {
    sha256_hex(&serde_json::to_vec(items).unwrap_or_default())
}

/// What the attempt record contributes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReceiptParts {
    pub(crate) run_nonce: String,
    pub(crate) process_nonce: String,
    pub(crate) stage: String,
    pub(crate) target: String,
    pub(crate) output_path: String,
    pub(crate) attempt: u32,
    pub(crate) generation: u64,
    pub(crate) pins: ToolPins,
    pub(crate) schema_sha256: String,
    pub(crate) input_revision: String,
    pub(crate) inputs: Vec<InputDigest>,
    pub(crate) dependencies: Vec<DependencyPin>,
    pub(crate) arguments_sha256: String,
    pub(crate) request_id: String,
    pub(crate) candidate_sha256: String,
    pub(crate) checks: Vec<CheckResult>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReceiptRefusal {
    NotCommitted,
    MalformedDigest,
    RequestDiffers,
    ToolDiffers,
    RuntimeDiffers,
    InputRevisionDiffers,
    SchemaDiffers,
    PathDiffers,
    CandidateDiffers,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StageReceipt {
    pub(crate) schema: String,
    pub(crate) run_nonce: String,
    pub(crate) process_nonce: String,
    pub(crate) stage: String,
    pub(crate) target: String,
    pub(crate) attempt: u32,
    pub(crate) generation: u64,
    pub(crate) pins: ToolPins,
    pub(crate) schema_sha256: String,
    pub(crate) input_revision: String,
    pub(crate) inputs: Vec<InputDigest>,
    pub(crate) inputs_sha256: String,
    pub(crate) dependencies: Vec<DependencyPin>,
    pub(crate) dependencies_sha256: String,
    pub(crate) arguments_sha256: String,
    pub(crate) candidate_sha256: String,
    /// The same value as the wrapped commit receipt's.
    pub(crate) committed_sha256: String,
    pub(crate) commit_request_id: String,
    pub(crate) checks: Vec<CheckResult>,
    /// Absent on a record read back from a ledger.
    #[serde(skip)]
    pub(crate) commit: Option<CommitReceipt>,
}

impl StageReceipt {
    /// Refuses a commit that does not describe this attempt's adoption.
    pub(crate) fn bind(parts: ReceiptParts, commit: CommitReceipt) -> Result<Self, ReceiptRefusal> {
        if !commit.committed || commit.protocol != RECEIPT_PROTOCOL {
            return Err(ReceiptRefusal::NotCommitted);
        }
        if !is_sha256_hex(&commit.committed_sha256) || !is_sha256_hex(&parts.candidate_sha256) {
            return Err(ReceiptRefusal::MalformedDigest);
        }
        // An earlier attempt's commit of the same bytes is not this attempt's.
        if commit.request_id != parts.request_id {
            return Err(ReceiptRefusal::RequestDiffers);
        }
        if commit.tool_id != parts.pins.tool_id {
            return Err(ReceiptRefusal::ToolDiffers);
        }
        if commit.runtime_sha256 != parts.pins.runtime_sha256 {
            return Err(ReceiptRefusal::RuntimeDiffers);
        }
        if commit.input_revision != parts.input_revision {
            return Err(ReceiptRefusal::InputRevisionDiffers);
        }
        if commit.schema_sha256 != parts.schema_sha256 {
            return Err(ReceiptRefusal::SchemaDiffers);
        }
        if commit.path != parts.output_path {
            return Err(ReceiptRefusal::PathDiffers);
        }
        if commit.committed_sha256 != parts.candidate_sha256 {
            return Err(ReceiptRefusal::CandidateDiffers);
        }
        let mut inputs = parts.inputs;
        inputs.sort();
        let mut dependencies = parts.dependencies;
        dependencies.sort();
        Ok(Self {
            schema: STAGE_RECEIPT_SCHEMA.to_string(),
            run_nonce: parts.run_nonce,
            process_nonce: parts.process_nonce,
            stage: parts.stage,
            target: parts.target,
            attempt: parts.attempt,
            generation: parts.generation,
            pins: parts.pins,
            schema_sha256: parts.schema_sha256,
            input_revision: parts.input_revision,
            inputs_sha256: digest_of(&inputs),
            inputs,
            dependencies_sha256: digest_of(&dependencies),
            dependencies,
            arguments_sha256: parts.arguments_sha256,
            candidate_sha256: parts.candidate_sha256,
            committed_sha256: commit.committed_sha256.clone(),
            commit_request_id: commit.request_id.clone(),
            checks: parts.checks,
            commit: Some(commit),
        })
    }

    /// Covers every bound value. Equal digests mean equal bindings.
    pub(crate) fn sha256(&self) -> String {
        sha256_hex(&serde_json::to_vec(self).unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{commit_receipt, digest, parts};
    use super::{CommitReceipt, ReceiptRefusal, StageReceipt, is_sha256_hex};

    #[test]
    fn the_stage_receipt_shares_the_commit_digest_and_keeps_the_commit_unchanged() {
        let commit = commit_receipt(&digest("candidate"));
        let receipt = StageReceipt::bind(parts(&digest("candidate")), commit.clone()).unwrap();
        assert_eq!(receipt.committed_sha256, commit.committed_sha256);
        assert_eq!(receipt.candidate_sha256, commit.committed_sha256);
        assert_eq!(receipt.commit.as_ref(), Some(&commit));
    }

    #[test]
    fn a_commit_that_describes_something_else_binds_nothing() {
        let candidate = digest("candidate");
        let good = commit_receipt(&candidate);
        type Change = fn(&mut CommitReceipt);
        let cases: [(&str, Change, ReceiptRefusal); 6] = [
            (
                "committed",
                |c| c.committed = false,
                ReceiptRefusal::NotCommitted,
            ),
            (
                "tool",
                |c| c.tool_id = "other".to_string(),
                ReceiptRefusal::ToolDiffers,
            ),
            (
                "runtime",
                |c| c.runtime_sha256 = "other".to_string(),
                ReceiptRefusal::RuntimeDiffers,
            ),
            (
                "input",
                |c| c.input_revision = "other".to_string(),
                ReceiptRefusal::InputRevisionDiffers,
            ),
            (
                "schema",
                |c| c.schema_sha256 = "other".to_string(),
                ReceiptRefusal::SchemaDiffers,
            ),
            (
                "path",
                |c| c.path = "other".to_string(),
                ReceiptRefusal::PathDiffers,
            ),
        ];
        for (name, change, expected) in cases {
            let mut commit = good.clone();
            change(&mut commit);
            assert_eq!(
                StageReceipt::bind(parts(&candidate), commit).unwrap_err(),
                expected,
                "{name}"
            );
        }

        let mut earlier = good.clone();
        earlier.request_id = "an-earlier-attempt".to_string();
        assert_eq!(
            StageReceipt::bind(parts(&candidate), earlier).unwrap_err(),
            ReceiptRefusal::RequestDiffers
        );
        let mut foreign = good.clone();
        foreign.protocol = "another-protocol".to_string();
        assert_eq!(
            StageReceipt::bind(parts(&candidate), foreign).unwrap_err(),
            ReceiptRefusal::NotCommitted
        );
        let mut other = good.clone();
        other.committed_sha256 = digest("something else");
        assert_eq!(
            StageReceipt::bind(parts(&candidate), other).unwrap_err(),
            ReceiptRefusal::CandidateDiffers
        );
        let mut broken = good;
        broken.committed_sha256 = "not a digest".to_string();
        assert_eq!(
            StageReceipt::bind(parts(&candidate), broken).unwrap_err(),
            ReceiptRefusal::MalformedDigest
        );
    }

    #[test]
    fn every_bound_value_changes_the_receipt_digest() {
        let candidate = digest("candidate");
        let base = StageReceipt::bind(parts(&candidate), commit_receipt(&candidate)).unwrap();
        let again = StageReceipt::bind(parts(&candidate), commit_receipt(&candidate)).unwrap();
        assert_eq!(base.sha256(), again.sha256());

        let changed = |change: &dyn Fn(&mut StageReceipt)| {
            let mut receipt = base.clone();
            change(&mut receipt);
            receipt.sha256()
        };
        let digests = [
            changed(&|r| r.run_nonce = "other".to_string()),
            changed(&|r| r.stage = "other".to_string()),
            changed(&|r| r.target = "other".to_string()),
            changed(&|r| r.attempt += 1),
            changed(&|r| r.generation += 1),
            changed(&|r| r.pins.descriptor_sha256 = "other".to_string()),
            changed(&|r| r.schema_sha256 = "other".to_string()),
            changed(&|r| r.inputs_sha256 = "other".to_string()),
            changed(&|r| r.dependencies_sha256 = "other".to_string()),
            changed(&|r| r.candidate_sha256 = "other".to_string()),
            changed(&|r| r.committed_sha256 = "other".to_string()),
            changed(&|r| r.checks.clear()),
        ];
        for value in digests {
            assert_ne!(value, base.sha256());
        }
    }

    #[test]
    fn a_saved_record_loses_the_commit_and_cannot_carry_a_claim() {
        let candidate = digest("candidate");
        let receipt = StageReceipt::bind(parts(&candidate), commit_receipt(&candidate)).unwrap();
        let saved = serde_json::to_string(&receipt).unwrap();
        let restored: StageReceipt = serde_json::from_str(&saved).unwrap();
        assert!(restored.commit.is_none());

        let mut claim: serde_json::Value = serde_json::from_str(&saved).unwrap();
        claim["accepted"] = serde_json::Value::Bool(true);
        assert!(serde_json::from_value::<StageReceipt>(claim).is_err());
    }

    #[test]
    fn only_a_lowercase_sha256_is_a_digest() {
        assert!(is_sha256_hex(&digest("x")));
        assert!(!is_sha256_hex(&digest("x").to_uppercase()));
        assert!(!is_sha256_hex("abc"));
    }
}
