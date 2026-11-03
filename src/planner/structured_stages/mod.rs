//! Stage obligations bound to host receipts and input revisions (Issue #975, G4).
//!
//! A profile or pack can declare stages over its propose-artifacts tools. A stage is complete
//! only when, for its current inputs, the host adopted the bytes and every check the operator
//! requires passed. A new upstream version takes the state from everything below it.
//!
//! - [`contract`] admits the declaration before any provider call.
//! - [`state`] is the state machine: typed inputs, no file, no process, clock as an argument.
//! - [`invalidation`] computes what an upstream change takes away.
//! - [`receipt`] binds a target's state to G3's commit receipt without changing it.
//! - [`resume`] lowers a saved record. A saved record never raises a state.
//! - [`ledger`] keeps attempts and generations across sessions and for a later process.
//!
//! [`StageRun`] joins them to a set of admitted tools. A tool in no declared stage never reaches
//! any of this, so a run with no stage declaration reads and writes nothing here.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use contract::{AdmittedTarget, ContractRefusal, StageContract, StageContractDraft, StageRunFacts};
use ledger::{HeldLedger, LedgerEntry, LedgerRefusal, LedgerRegistry, LedgerRequest};
use receipt::{InputDigest, sha256_hex};
use resume::AdmittedPins;
use state::{AttemptRequest, BytesSource, ObservedBytes};

use crate::tools::profile_tools::commit::CommitReceipt;
use crate::tools::profile_tools::schema::binding::AdoptedInputs;
use crate::tools::profile_tools::schema::invocation::SchemaGuard;
use crate::tools::profile_tools::{ProfileToolRefusal, RefusalKind};

pub(crate) fn contract_refused(refusal: ContractRefusal) -> ProfileToolRefusal {
    ProfileToolRefusal::new(
        RefusalKind::StageContractRefused,
        format!(
            "the declared stages cannot be admitted: {:?} {}",
            refusal.kind, refusal.subject
        ),
    )
}

/// A fixed sentence by kind, with the path and the input's name in `data`: a later step reads a
/// message by its words, and a word in a path could make this end look like something to retry.
pub(crate) fn ledger_refused(refusal: LedgerRefusal) -> ProfileToolRefusal {
    let mut data = serde_json::json!({
        "code": refusal.code(),
        "recovery": refusal.recovery(),
    });
    if let LedgerRefusal::FixedInputChanged { path } = &refusal {
        data["input"] = serde_json::Value::String(path.clone());
    }
    ProfileToolRefusal {
        kind: RefusalKind::StageLedgerRefused,
        reason: format!(
            "the record of this run's stages cannot be used: {}. {}",
            refusal.code(),
            refusal.recovery()
        ),
        data: Some(data),
    }
}

/// One tool of a stage set: its pins as admitted, and the guard holding its adopted inputs.
#[derive(Debug)]
pub(crate) struct StageMember {
    pub(crate) pins: AdmittedPins,
    pub(crate) guard: Arc<SchemaGuard>,
}

#[derive(Debug, Clone)]
struct StageSession {
    contract: Arc<StageContract>,
    state_dir: PathBuf,
    entry: LedgerEntry,
}

/// What one tool call brings that the host cannot read elsewhere.
#[derive(Debug)]
pub(crate) struct StageCallFacts<'a> {
    pub(crate) turn: u64,
    pub(crate) now_ms: u64,
    pub(crate) expected_revision: &'a str,
    pub(crate) arguments_sha256: String,
    pub(crate) request_id: String,
    pub(crate) current_output: &'a ObservedBytes,
    pub(crate) adopted: &'a AdoptedInputs,
}

/// The stage set of one admission. Opening its ledger waits for the first call: an entry that
/// only admits, as a preflight does, holds no lock and touches no file.
#[derive(Debug)]
pub(crate) struct StageRun {
    draft: StageContractDraft,
    members: BTreeMap<String, StageMember>,
    workspace: PathBuf,
    profile: String,
    ledgers: Arc<LedgerRegistry>,
    session: Mutex<Option<StageSession>>,
}

impl StageRun {
    pub(crate) fn new(
        draft: StageContractDraft,
        members: BTreeMap<String, StageMember>,
        workspace: PathBuf,
        profile: String,
        ledgers: Arc<LedgerRegistry>,
    ) -> Self {
        Self {
            draft,
            members,
            workspace,
            profile,
            ledgers,
            session: Mutex::new(None),
        }
    }

    /// Whether this run may hold stages at all, with the values in force now. An entry can ask
    /// this before it spends a provider call. It keeps nothing.
    pub(crate) fn judge_run(
        &self,
        run: StageRunFacts,
        state_dir: &Path,
    ) -> Result<StageContract, ProfileToolRefusal> {
        let contract = self.draft.for_run(run).map_err(contract_refused)?;
        ledger::refuse_state_dir_inside_workspace(state_dir, &self.workspace)
            .map_err(ledger_refused)?;
        Ok(contract)
    }

    /// Where the operator finds the record, on a refusal about it.
    pub(crate) fn with_ledger_path(&self, mut refusal: ProfileToolRefusal) -> ProfileToolRefusal {
        let session = self.session.lock().unwrap_or_else(PoisonError::into_inner);
        if let (Some(data), Some(session)) = (refusal.data.as_mut(), session.as_ref())
            && let Some(file) =
                ledger::ledger_file(&session.state_dir, &self.workspace, &self.profile)
        {
            data["ledger_path"] = serde_json::Value::String(file.display().to_string());
        }
        refusal
    }

    /// What only a session can answer, asked anew for each one.
    pub(crate) fn bind_session(
        &self,
        run: StageRunFacts,
        state_dir: &Path,
        entry: LedgerEntry,
    ) -> Result<(), ProfileToolRefusal> {
        let contract = self.judge_run(run, state_dir)?;
        *self.session.lock().unwrap_or_else(PoisonError::into_inner) = Some(StageSession {
            contract: Arc::new(contract),
            state_dir: state_dir.to_path_buf(),
            entry,
        });
        Ok(())
    }

    /// The ledger as of now, and the contract it is held to. Never called during an admission.
    pub(crate) fn open_ledger(
        &self,
        now_ms: u64,
        bytes: &dyn BytesSource,
    ) -> Result<(HeldLedger, Arc<StageContract>), ProfileToolRefusal> {
        let session = self
            .session
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
            .ok_or_else(|| {
                ProfileToolRefusal::new(
                    RefusalKind::StageContractRefused,
                    "a stage tool was called outside a session that admitted its stages",
                )
            })?;
        let pins = self
            .members
            .iter()
            .map(|(tool, member)| (tool.clone(), member.pins.clone()))
            .collect();
        let held = self
            .ledgers
            .open(&LedgerRequest {
                state_dir: &session.state_dir,
                workspace: &self.workspace,
                profile: &self.profile,
                entry: session.entry,
                contract: session.contract.clone(),
                now_ms,
                bytes,
                pins: &pins,
            })
            .map_err(ledger_refused)?;
        Ok((held, session.contract))
    }

    /// The one place a call's values become what the state machine judges. Paths come from the
    /// admitted target and digests from what G2 adopted, so no spelling and no digest here is
    /// the caller's.
    pub(crate) fn attempt_request<'a>(
        &self,
        contract: &StageContract,
        target: &AdmittedTarget,
        call: StageCallFacts<'a>,
    ) -> Option<AttemptRequest<'a>> {
        let member = self.members.get(&target.tool_id)?;
        let digest_of = |adopted: &AdoptedInputs, path: &str| InputDigest {
            path: path.to_string(),
            sha256: adopted
                .inputs
                .iter()
                .find(|input| input.path == path)
                .map(|input| sha256_hex(&input.bytes))
                .unwrap_or_default(),
        };
        let mut adopted_fixed = Vec::new();
        for other in contract.targets() {
            let adopted = self.members.get(&other.tool_id)?.guard.adopted();
            let fixed = other
                .inputs
                .iter()
                .filter(|input| input.upstream.is_none())
                .map(|input| digest_of(&adopted, &input.path))
                .collect();
            adopted_fixed.push((other.key.clone(), fixed));
        }
        Some(AttemptRequest {
            turn: call.turn,
            now_ms: call.now_ms,
            expected_revision: call.expected_revision,
            pins: member.pins.pins.clone(),
            schema_sha256: member.pins.schema_sha256.clone(),
            input_revision: call.adopted.revision.clone(),
            inputs: target
                .inputs
                .iter()
                .map(|input| digest_of(call.adopted, &input.path))
                .collect(),
            arguments_sha256: call.arguments_sha256,
            request_id: call.request_id,
            current_output: call.current_output,
            adopted_fixed,
        })
    }

    /// A seal is what every reader of this output takes its new version from (S6).
    pub(crate) fn readopt_readers(
        &self,
        contract: &StageContract,
        writer: &AdmittedTarget,
        receipt: &CommitReceipt,
    ) -> Result<(), ProfileToolRefusal> {
        for reader in contract.targets() {
            let reads_it = reader
                .inputs
                .iter()
                .any(|input| input.upstream.as_ref() == Some(&writer.key));
            if reads_it && let Some(member) = self.members.get(&reader.tool_id) {
                member.guard.readopt(receipt)?;
            }
        }
        Ok(())
    }
}

#[allow(
    dead_code,
    reason = "Issue #975 step 1 has no caller; the attribute goes when step 3 wires it in"
)]
pub(crate) mod contract;
#[allow(
    dead_code,
    reason = "Issue #975 step 1 has no caller; the attribute goes when step 3 wires it in"
)]
pub(crate) mod invalidation;
#[allow(
    dead_code,
    reason = "Issue #975 step 1 has no caller; the attribute goes when step 3 wires it in"
)]
pub(crate) mod ledger;
#[allow(
    dead_code,
    reason = "Issue #975 step 1 has no caller; the attribute goes when step 3 wires it in"
)]
pub(crate) mod receipt;
#[allow(
    dead_code,
    reason = "Issue #975 step 1 has no caller; the attribute goes when step 3 wires it in"
)]
pub(crate) mod resume;
#[allow(
    dead_code,
    reason = "Issue #975 step 1 has no caller; the attribute goes when step 3 wires it in"
)]
pub(crate) mod state;

#[cfg(test)]
mod test_support {
    //! Synthetic stage sets and a fake byte source. The IDs and paths mean nothing to the host.

    use std::collections::BTreeMap;
    use std::sync::Arc;

    use sha2::{Digest, Sha256};

    use super::contract::{
        ContractRefusal, HostToolFacts, NarrowedStageLimits, StageContract, StageDeclaration,
        StageLimits, StageRunFacts, StageSetDeclaration, TargetDeclaration, TargetKey, admit,
    };
    use super::receipt::{CheckResult, CheckStatus, InputDigest, ReceiptParts, ToolPins};
    use super::state::{
        AttemptRequest, BeginDecision, BytesSource, CommitDecision, CommitObservation,
        LocalDecision, ObservedBytes, StageMachine,
    };
    use crate::tools::profile_tools::commit::CommitReceipt;
    use crate::tools::profile_tools::protocol::RECEIPT_PROTOCOL;

    /// The admission, in a run an OS boundary holds and no workflow owns.
    pub(crate) fn admit_enforced(
        declaration: &StageSetDeclaration,
        tools: &[HostToolFacts],
    ) -> Result<StageContract, ContractRefusal> {
        admit(StageRunFacts::for_test(true, false), declaration, tools)
    }

    pub(crate) fn digest(text: &str) -> String {
        sha_of(text.as_bytes())
    }

    pub(crate) fn sha_of(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }

    pub(crate) fn key(stage: &str, target: &str) -> TargetKey {
        TargetKey {
            stage: stage.to_string(),
            target: target.to_string(),
        }
    }

    pub(crate) fn host_limits() -> StageLimits {
        StageLimits {
            attempts_per_target: 8,
            attempts_per_run: 64,
            deadline_seconds: 3600,
        }
    }

    pub(crate) fn facts(tool: &str, output: &str, inputs: &[&str]) -> HostToolFacts {
        HostToolFacts {
            tool_id: tool.to_string(),
            output_id: "artifact".to_string(),
            output_path: output.to_string(),
            schema_inputs: inputs.iter().map(|path| path.to_string()).collect(),
            has_revision_argument: false,
            has_compiler: false,
            required_checks: vec!["shape".to_string(), "content".to_string()],
            limits: host_limits(),
        }
    }

    /// A stage with one target, named after its tool.
    pub(crate) fn one_target(stage: &str, tool: &str) -> StageDeclaration {
        StageDeclaration {
            id: stage.to_string(),
            targets: vec![TargetDeclaration {
                id: tool.to_string(),
                tool_id: tool.to_string(),
            }],
            depends_on: Vec::new(),
            required_checks: Vec::new(),
        }
    }

    fn chained(
        first: (&str, &str, &str),
        second: (&str, &str, &str),
        fixed: &str,
    ) -> StageContract {
        let target = |id: &str, tool: &str| TargetDeclaration {
            id: id.to_string(),
            tool_id: tool.to_string(),
        };
        let stage = |id: &str, declared: TargetDeclaration| StageDeclaration {
            id: id.to_string(),
            targets: vec![declared],
            depends_on: Vec::new(),
            required_checks: Vec::new(),
        };
        let first_tool = format!("{}-tool", first.0);
        let second_tool = format!("{}-tool", second.0);
        // The downstream comes first in the file: the order comes from the paths.
        let declaration = StageSetDeclaration {
            stages: vec![
                stage(second.0, target(second.1, &second_tool)),
                stage(first.0, target(first.1, &first_tool)),
            ],
            limits: NarrowedStageLimits::default(),
        };
        let tools = [
            facts(&first_tool, first.2, &[fixed]),
            facts(&second_tool, second.2, &[first.2]),
        ];
        admit_enforced(&declaration, &tools).unwrap()
    }

    pub(crate) fn two_stage_contract() -> StageContract {
        chained(
            ("alpha", "main", "out/alpha.json"),
            ("beta", "main", "out/beta.json"),
            "fixed/source.json",
        )
    }

    pub(crate) fn setting_object_contract() -> StageContract {
        chained(
            ("settings", "object", "config/settings.json"),
            ("settings_index", "lookup", "config/settings-index.json"),
            "seed/settings-schema.json",
        )
    }

    pub(crate) fn document_index_contract() -> StageContract {
        chained(
            ("catalog", "documents", "docs/catalog.json"),
            ("summary", "digest", "docs/summary.json"),
            "seed/document-list.json",
        )
    }

    #[derive(Debug, Clone, Default)]
    pub(crate) struct FakeBytes {
        files: BTreeMap<String, ObservedBytes>,
    }

    impl FakeBytes {
        pub(crate) fn put(&mut self, path: &str, bytes: &[u8]) {
            self.files
                .insert(path.to_string(), ObservedBytes::Present(bytes.to_vec()));
        }

        pub(crate) fn with(&self, path: &str, bytes: &[u8]) -> Self {
            let mut copy = self.clone();
            copy.put(path, bytes);
            copy
        }

        pub(crate) fn without(&self, path: &str) -> Self {
            let mut copy = self.clone();
            copy.files.remove(path);
            copy
        }

        pub(crate) fn unreadable(&self, path: &str) -> Self {
            let mut copy = self.clone();
            copy.files
                .insert(path.to_string(), ObservedBytes::Unreadable);
            copy
        }

        pub(crate) fn observe_path(&self, path: &str) -> ObservedBytes {
            self.observe(path)
        }
    }

    impl BytesSource for FakeBytes {
        fn observe(&self, path: &str) -> ObservedBytes {
            self.files
                .get(path)
                .cloned()
                .unwrap_or(ObservedBytes::Absent)
        }
    }

    pub(crate) fn pins_for(tool: &str) -> ToolPins {
        ToolPins {
            tool_id: tool.to_string(),
            tool_version: "1.0.0".to_string(),
            descriptor_sha256: digest(&format!("descriptor {tool}")),
            runtime_sha256: digest(&format!("runtime {tool}")),
        }
    }

    pub(crate) fn schema_for(tool: &str) -> String {
        digest(&format!("schema {tool}"))
    }

    pub(crate) fn current_inputs(
        contract: &StageContract,
        target: &TargetKey,
        bytes: &FakeBytes,
    ) -> Vec<InputDigest> {
        contract
            .target(target)
            .map(|admitted| {
                admitted
                    .inputs
                    .iter()
                    .map(|input| InputDigest {
                        path: input.path.clone(),
                        sha256: bytes.observe(&input.path).sha256().unwrap_or_default(),
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// One call, its values taken from the operator's pins and the bytes there now.
    #[allow(
        clippy::too_many_arguments,
        reason = "a test helper that spells out each value a call varies"
    )]
    pub(crate) fn request_for<'a>(
        contract: &Arc<StageContract>,
        target: &TargetKey,
        expected_revision: &'a str,
        bytes: &FakeBytes,
        turn: u64,
        now_ms: u64,
        arguments: &str,
        current_output: &'a ObservedBytes,
    ) -> AttemptRequest<'a> {
        let tool = contract
            .target(target)
            .map_or("", |admitted| admitted.tool_id.as_str());
        let inputs = current_inputs(contract, target, bytes);
        let adopted_fixed = contract
            .targets()
            .map(|admitted| {
                let fixed = admitted
                    .inputs
                    .iter()
                    .filter(|input| input.upstream.is_none())
                    .map(|input| InputDigest {
                        path: input.path.clone(),
                        sha256: bytes.observe(&input.path).sha256().unwrap_or_default(),
                    })
                    .collect();
                (admitted.key.clone(), fixed)
            })
            .collect();
        AttemptRequest {
            turn,
            now_ms,
            expected_revision,
            pins: pins_for(tool),
            schema_sha256: schema_for(tool),
            input_revision: digest(&serde_json::to_string(&inputs).unwrap()),
            inputs,
            arguments_sha256: digest(arguments),
            request_id: "request-1".to_string(),
            current_output,
            adopted_fixed,
        }
    }

    pub(crate) fn passed_checks(contract: &StageContract, target: &TargetKey) -> Vec<CheckResult> {
        contract
            .target(target)
            .map(|admitted| {
                admitted
                    .required_checks
                    .iter()
                    .map(|id| CheckResult {
                        id: id.clone(),
                        status: CheckStatus::Passed,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    pub(crate) fn commit_for(
        contract: &StageContract,
        target: &TargetKey,
        pins: &ToolPins,
        schema: &str,
        input_revision: &str,
        candidate: &str,
    ) -> CommitReceipt {
        CommitReceipt {
            protocol: RECEIPT_PROTOCOL.to_string(),
            request_id: "request-1".to_string(),
            tool_id: pins.tool_id.clone(),
            output_id: "artifact".to_string(),
            path: contract
                .target(target)
                .map(|admitted| admitted.output_path.clone())
                .unwrap_or_default(),
            committed: true,
            durable: true,
            cancelled_after_commit: false,
            runtime_sha256: pins.runtime_sha256.clone(),
            input_revision: input_revision.to_string(),
            schema_sha256: schema.to_string(),
            result_sha256: digest("result"),
            committed_sha256: candidate.to_string(),
            committed_bytes: 1,
            diagnostics: Vec::new(),
        }
    }

    /// The whole path of one good submission on the machine, the file written where a rename
    /// would be. Panics on anything but a seal.
    #[allow(
        clippy::too_many_arguments,
        reason = "a test helper that spells out each value a call varies"
    )]
    pub(crate) fn seal(
        machine: &mut StageMachine,
        target: &TargetKey,
        content: &[u8],
        bytes: &mut FakeBytes,
        turn: u64,
        now_ms: u64,
        arguments: &str,
    ) -> CommitDecision {
        let contract = machine.contract().clone();
        let admitted = contract.target(target).unwrap().clone();
        let revision = machine.revision_of(target).unwrap();
        let output = bytes.observe(&admitted.output_path);
        let request = request_for(
            &contract, target, &revision, bytes, turn, now_ms, arguments, &output,
        );
        let (pins, schema, input_revision) = (
            request.pins.clone(),
            request.schema_sha256.clone(),
            request.input_revision.clone(),
        );
        let BeginDecision::Proceed(ticket) = machine.begin(target, request) else {
            panic!("the attempt on {target:?} did not start");
        };
        let candidate = sha_of(content);
        assert_eq!(
            machine.record_local_result(
                &ticket,
                &candidate,
                passed_checks(&contract, target),
                &output,
            ),
            LocalDecision::ReadyToCommit
        );
        machine.record_intent(&ticket, output.sha256()).unwrap();
        bytes.put(&admitted.output_path, content);
        let commit = commit_for(
            &contract,
            target,
            &pins,
            &schema,
            &input_revision,
            &candidate,
        );
        let decision = machine
            .record_commit(&ticket, CommitObservation::Stood(Box::new(commit)))
            .unwrap();
        assert!(decision.is_progress(), "{decision:?}");
        decision
    }

    pub(crate) fn commit_receipt(candidate: &str) -> CommitReceipt {
        CommitReceipt {
            protocol: RECEIPT_PROTOCOL.to_string(),
            request_id: "request-1".to_string(),
            tool_id: "tool-1".to_string(),
            output_id: "artifact".to_string(),
            path: "out/a.json".to_string(),
            committed: true,
            durable: true,
            cancelled_after_commit: false,
            runtime_sha256: digest("runtime"),
            input_revision: digest("inputs"),
            schema_sha256: digest("schema"),
            result_sha256: digest("result"),
            committed_sha256: candidate.to_string(),
            committed_bytes: 9,
            diagnostics: Vec::new(),
        }
    }

    pub(crate) fn parts(candidate: &str) -> ReceiptParts {
        ReceiptParts {
            run_nonce: "run-a".to_string(),
            process_nonce: "process-a".to_string(),
            stage: "alpha".to_string(),
            target: "main".to_string(),
            output_path: "out/a.json".to_string(),
            attempt: 1,
            generation: 1,
            pins: ToolPins {
                tool_id: "tool-1".to_string(),
                tool_version: "1.0.0".to_string(),
                descriptor_sha256: digest("descriptor"),
                runtime_sha256: digest("runtime"),
            },
            schema_sha256: digest("schema"),
            input_revision: digest("inputs"),
            inputs: Vec::new(),
            dependencies: Vec::new(),
            arguments_sha256: digest("arguments"),
            request_id: "request-1".to_string(),
            candidate_sha256: candidate.to_string(),
            checks: vec![CheckResult {
                id: "shape".to_string(),
                status: CheckStatus::Passed,
            }],
        }
    }
}
