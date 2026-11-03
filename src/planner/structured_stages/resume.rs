//! What a saved record can do when another process picks it up (Issue #975, G4): lower.
//!
//! A saved record is never evidence. Attempts and generations are kept so nothing refills, and
//! the real bytes decide how far a target falls. No target comes out sealed, whatever the file
//! says; each must pass its required checks again in this process (D11).
//!
//! The one thing a resume does not lower past is a changed fixed input: it is refused, and only
//! a fresh session goes on (P5). The run's first `begin` pins every target's fixed inputs as G2
//! adopted them, so a target that has not run is held to them too, and a change after G2's first
//! admission differs from its pin. A change before that admission cannot be seen: what is there
//! then is what gets adopted and pinned.

use std::collections::BTreeMap;
use std::sync::Arc;

use super::contract::{AdmittedTarget, StageContract, TargetKey};
use super::invalidation::invalidate_downstream;
use super::receipt::ToolPins;
use super::state::{
    BytesSource, InFlightProgress, MachineSnapshot, ObservedBytes, Phase, SNAPSHOT_SCHEMA,
    StageMachine, TargetRecord, UnverifiedReason,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SavedRefusal {
    WrongSchema,
    ContractChanged,
    TargetsDiffer,
    CountsInconsistent,
    /// A pin on a path the contract does not give its target as a fixed input.
    PinOutsideContract,
    /// A fixed input is no longer what the run first adopted. Names the input, not its content.
    FixedInputChanged {
        path: String,
    },
}

/// A tool's pins as admitted in this process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AdmittedPins {
    pub(crate) pins: ToolPins,
    pub(crate) schema_sha256: String,
}

pub(crate) struct ResumeFacts<'a> {
    pub(crate) process_nonce: String,
    pub(crate) now_ms: u64,
    pub(crate) bytes: &'a dyn BytesSource,
    /// By tool ID.
    pub(crate) pins: &'a BTreeMap<String, AdmittedPins>,
}

/// A record that does not fit the contract it claims is refused, never repaired: repairing it
/// would have to guess a count.
pub(crate) fn validate_saved(
    saved: &MachineSnapshot,
    contract: &StageContract,
) -> Result<(), SavedRefusal> {
    if saved.schema != SNAPSHOT_SCHEMA || saved.run_nonce.is_empty() {
        return Err(SavedRefusal::WrongSchema);
    }
    if saved.contract_sha256 != contract.sha256() {
        return Err(SavedRefusal::ContractChanged);
    }
    let expected = contract.targets().map(|target| &target.key);
    if !expected.eq(saved.targets.iter().map(|record| &record.key)) {
        return Err(SavedRefusal::TargetsDiffer);
    }
    let counted = saved
        .targets
        .iter()
        .try_fold(0u32, |sum, record| sum.checked_add(record.attempts_used));
    if counted.is_none_or(|sum| sum > saved.attempts_used) {
        return Err(SavedRefusal::CountsInconsistent);
    }
    // The contract says which paths are fixed inputs, and its digest was just compared. A path
    // that only the file names is never read and never reaches a message.
    for (record, target) in saved.targets.iter().zip(contract.targets()) {
        let is_fixed_input = |path: &str| {
            target
                .inputs
                .iter()
                .any(|input| input.path == path && input.upstream.is_none())
        };
        let mut pins = record.fixed_inputs.iter().flatten();
        if pins.any(|pin| !is_fixed_input(&pin.path)) {
            return Err(SavedRefusal::PinOutsideContract);
        }
    }
    Ok(())
}

struct Lowered {
    reason: Option<UnverifiedReason>,
    bytes_moved: bool,
}

/// S15: what an intent with no recorded outcome means, by what is there now. Both digests
/// equal counts as moved, the side that costs a recheck.
fn judge_interrupted_commit(record: &TargetRecord, observed: &ObservedBytes) -> Lowered {
    let Some(InFlightProgress::IntentRecorded {
        candidate_sha256,
        previous_sha256,
        ..
    }) = record
        .in_flight
        .as_ref()
        .map(|in_flight| &in_flight.progress)
    else {
        return Lowered {
            reason: None,
            bytes_moved: false,
        };
    };
    let matches_previous = match (observed, previous_sha256) {
        (ObservedBytes::Absent, None) => true,
        (ObservedBytes::Present(_), Some(previous)) => {
            observed.sha256().as_deref() == Some(previous.as_str())
        }
        _ => false,
    };
    if observed.sha256().as_deref() == Some(candidate_sha256.as_str()) {
        Lowered {
            reason: Some(UnverifiedReason::InterruptedCommit),
            bytes_moved: true,
        }
    } else if matches_previous {
        Lowered {
            reason: None,
            bytes_moved: false,
        }
    } else {
        Lowered {
            reason: Some(UnverifiedReason::BytesChanged),
            bytes_moved: true,
        }
    }
}

/// Why the state a saved receipt described no longer stands here, beyond having been resumed.
fn judge_saved_receipt(
    record: &TargetRecord,
    observed: &ObservedBytes,
    target: &AdmittedTarget,
    facts: &ResumeFacts<'_>,
) -> Option<UnverifiedReason> {
    let receipt = record.current_receipt.as_ref()?;
    if observed.sha256().as_deref() != Some(receipt.committed_sha256.as_str()) {
        return Some(UnverifiedReason::BytesChanged);
    }
    let pinned = facts.pins.get(&target.tool_id);
    if pinned.is_none_or(|admitted| {
        admitted.pins != receipt.pins || admitted.schema_sha256 != receipt.schema_sha256
    }) {
        return Some(UnverifiedReason::PinChanged);
    }
    // An input the contract does not name counts as moved, without being read.
    let input_moved = receipt.inputs.iter().any(|input| {
        !target.inputs.iter().any(|known| known.path == input.path)
            || facts.bytes.observe(&input.path).sha256().as_deref() != Some(input.sha256.as_str())
    });
    input_moved.then_some(UnverifiedReason::InputChanged)
}

/// The pins hold what G2 adopted for each target at the run's first `begin`. An input from
/// upstream is not among them: its change is an invalidation, handled below.
fn refuse_changed_fixed_input(
    saved: &MachineSnapshot,
    facts: &ResumeFacts<'_>,
) -> Result<(), SavedRefusal> {
    let pins = saved
        .targets
        .iter()
        .filter_map(|record| record.fixed_inputs.as_ref())
        .flatten();
    for pin in pins {
        if facts.bytes.observe(&pin.path).sha256().as_deref() != Some(pin.sha256.as_str()) {
            return Err(SavedRefusal::FixedInputChanged {
                path: pin.path.clone(),
            });
        }
    }
    Ok(())
}

fn lower_one(record: &mut TargetRecord, target: &AdmittedTarget, facts: &ResumeFacts<'_>) -> bool {
    let observed = facts.bytes.observe(&target.output_path);
    let interrupted = judge_interrupted_commit(record, &observed);
    let saved = if interrupted.bytes_moved {
        None
    } else {
        judge_saved_receipt(record, &observed, target, facts)
    };
    let reason = interrupted.reason.or(saved);
    let moved = reason.is_some();
    let held_a_state = record.current_receipt.is_some()
        || matches!(record.phase, Phase::Sealed | Phase::Unverified);
    if moved {
        record.generation += 1;
    }
    match reason {
        Some(reason) => record.lower_to_unverified(reason),
        None if held_a_state => {
            let kept = record.unverified_reason;
            record.lower_to_unverified(kept.unwrap_or(UnverifiedReason::Resumed));
        }
        None => record.settle_after_attempt(),
    }
    // Whatever it was, it has been judged, and the process it ran in is gone.
    record.in_flight = None;
    moved
}

/// The machine a resumed process starts from. Everything it may keep is a count.
pub(super) fn lower_saved(
    saved: MachineSnapshot,
    contract: &Arc<StageContract>,
    facts: &ResumeFacts<'_>,
) -> Result<StageMachine, SavedRefusal> {
    validate_saved(&saved, contract)?;
    refuse_changed_fixed_input(&saved, facts)?;
    let mut snapshot = saved;
    snapshot.process_nonce = facts.process_nonce.clone();
    snapshot.latest_clock_ms = snapshot.latest_clock_ms.max(facts.now_ms);
    snapshot.last_commit_turn = None;
    let mut moved = Vec::<TargetKey>::new();
    for record in &mut snapshot.targets {
        let Some(target) = contract.target(&record.key) else {
            continue;
        };
        if lower_one(record, target, facts) {
            moved.push(record.key.clone());
        }
    }
    for key in moved {
        invalidate_downstream(contract, &mut snapshot.targets, &key.stage);
    }
    Ok(StageMachine::from_lowered_snapshot(
        contract.clone(),
        snapshot,
    ))
}

/// The file changed under a live process. The higher count wins on each line, and since the
/// file was not what this process wrote, nothing here is vouched for any more. An attempt past
/// its intent keeps its way out, as in [`TargetRecord::drop_unless_intent`].
pub(crate) fn absorb_diverged(held: &mut MachineSnapshot, disk: &MachineSnapshot) {
    held.started_at_ms = held.started_at_ms.min(disk.started_at_ms);
    held.latest_clock_ms = held.latest_clock_ms.max(disk.latest_clock_ms);
    held.last_commit_turn = None;
    held.ended_by_secret_value |= disk.ended_by_secret_value;
    for record in &mut held.targets {
        let saved = disk.targets.iter().find(|saved| saved.key == record.key);
        if let Some(saved) = saved {
            record.attempts_used = record.attempts_used.max(saved.attempts_used);
            record.generation = record.generation.max(saved.generation);
        }
        record.generation += 1;
        record.drop_unless_intent();
        if record.phase == Phase::Sealed || record.current_receipt.is_some() {
            record.lower_to_unverified(UnverifiedReason::LedgerDiverged);
        } else {
            record.settle_after_attempt();
        }
    }
    // The total keeps to at least the sum of its lines, or a later resume could not read it.
    let counted = held
        .targets
        .iter()
        .fold(0u32, |sum, record| sum.saturating_add(record.attempts_used));
    held.attempts_used = held.attempts_used.max(disk.attempts_used).max(counted);
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use std::sync::Arc;

    use super::super::contract::{
        NarrowedStageLimits, StageContract, StageSetDeclaration, TargetKey,
    };
    use super::super::receipt::{CheckResult, CheckStatus, InputDigest};
    use super::super::state::{
        BeginDecision, BytesSource, ExhaustedLimit, LocalDecision, MachineSnapshot, ObservedBytes,
        Phase, StageMachine, UnverifiedReason,
    };
    use super::super::test_support::{
        FakeBytes, admit_enforced, current_inputs, facts, key, one_target, passed_checks, pins_for,
        request_for, schema_for, seal, sha_of, two_stage_contract,
    };
    use super::{
        AdmittedPins, ResumeFacts, SavedRefusal, absorb_diverged, lower_saved, validate_saved,
    };

    const NOW: u64 = 5_000;

    fn alpha() -> TargetKey {
        key("alpha", "main")
    }

    fn beta() -> TargetKey {
        key("beta", "main")
    }

    fn current_pins(contract: &StageContract) -> BTreeMap<String, AdmittedPins> {
        contract
            .targets()
            .map(|target| {
                (
                    target.tool_id.clone(),
                    AdmittedPins {
                        pins: pins_for(&target.tool_id),
                        schema_sha256: schema_for(&target.tool_id),
                    },
                )
            })
            .collect()
    }

    fn both_sealed() -> (StageMachine, FakeBytes) {
        let mut machine = StageMachine::new(
            Arc::new(two_stage_contract()),
            "run-a".to_string(),
            "process-a".to_string(),
            NOW,
        );
        let mut bytes = FakeBytes::default();
        bytes.put("fixed/source.json", b"fixed source");
        seal(&mut machine, &alpha(), b"alpha v1", &mut bytes, 1, NOW, "a");
        seal(&mut machine, &beta(), b"beta v1", &mut bytes, 2, NOW, "b");
        (machine, bytes)
    }

    /// A saved copy, through the same text a ledger would hold.
    fn saved(machine: &StageMachine) -> MachineSnapshot {
        serde_json::from_str(&serde_json::to_string(machine.snapshot()).unwrap()).unwrap()
    }

    fn resume(
        saved: MachineSnapshot,
        contract: &Arc<StageContract>,
        bytes: &FakeBytes,
    ) -> StageMachine {
        let pins = current_pins(contract);
        lower_saved(
            saved,
            contract,
            &ResumeFacts {
                process_nonce: "process-b".to_string(),
                now_ms: NOW + 1,
                bytes,
                pins: &pins,
            },
        )
        .unwrap()
    }

    fn reason(machine: &StageMachine, target: &TargetKey) -> Option<UnverifiedReason> {
        machine.record(target).unwrap().unverified_reason
    }

    fn generation(machine: &StageMachine, target: &TargetKey) -> u64 {
        machine.record(target).unwrap().generation
    }

    fn phase(machine: &StageMachine, target: &TargetKey) -> Phase {
        machine.record(target).unwrap().phase
    }

    #[test]
    fn a_record_of_sealed_targets_with_matching_bytes_still_comes_back_unverified() {
        let (machine, bytes) = both_sealed();
        let contract = machine.contract().clone();
        let mut resumed = resume(saved(&machine), &contract, &bytes);
        for target in [alpha(), beta()] {
            assert_eq!(phase(&resumed, &target), Phase::Unverified);
            assert_eq!(reason(&resumed, &target), Some(UnverifiedReason::Resumed));
            assert_eq!(generation(&resumed, &target), generation(&machine, &target));
            let record = resumed.record(&target).unwrap();
            assert!(record.current_receipt.is_none());
            assert_eq!(record.history.len(), 1);
            assert!(record.history[0].commit.is_none());
        }
        assert!(!resumed.judge(&bytes).complete());
    }

    #[test]
    fn nothing_in_the_file_can_raise_a_state() {
        let mut machine = StageMachine::new(
            Arc::new(two_stage_contract()),
            "run-a".to_string(),
            "process-a".to_string(),
            NOW,
        );
        let mut bytes = FakeBytes::default();
        bytes.put("fixed/source.json", b"fixed source");
        seal(&mut machine, &alpha(), b"alpha v1", &mut bytes, 1, NOW, "a");
        let mut forged = saved(&machine);
        // The record claims sealed, with no receipt at all, on something never run.
        forged.targets[1].phase = Phase::Sealed;
        forged.targets[1].current_receipt = None;
        let contract = machine.contract().clone();
        let mut resumed = resume(forged.clone(), &contract, &bytes);
        assert_eq!(phase(&resumed, &beta()), Phase::Unverified);
        assert!(!resumed.judge(&bytes).complete());

        // And with the upstream's receipt pasted in.
        forged.targets[1].current_receipt = forged.targets[0].current_receipt.clone();
        let mut resumed = resume(forged, &contract, &bytes);
        assert_ne!(phase(&resumed, &beta()), Phase::Sealed);
        assert!(!resumed.judge(&bytes).complete());
    }

    #[test]
    fn counts_and_the_deadline_start_are_kept_and_the_run_is_the_same_run() {
        let (machine, bytes) = both_sealed();
        let contract = machine.contract().clone();
        let resumed = resume(saved(&machine), &contract, &bytes);
        assert_eq!(resumed.snapshot().attempts_used, 2);
        assert_eq!(resumed.record(&alpha()).unwrap().attempts_used, 1);
        assert_eq!(resumed.snapshot().started_at_ms, NOW);
        assert_eq!(resumed.snapshot().latest_clock_ms, NOW + 1);
        assert_eq!(resumed.snapshot().run_nonce, "run-a");
        assert_eq!(resumed.snapshot().process_nonce, "process-b");
    }

    #[test]
    fn a_saved_clock_ahead_of_this_one_is_not_wound_back() {
        let (mut machine, bytes) = both_sealed();
        machine.snapshot_mut().latest_clock_ms = NOW + 500;
        let contract = machine.contract().clone();
        let resumed = resume(saved(&machine), &contract, &bytes);
        assert_eq!(resumed.snapshot().latest_clock_ms, NOW + 500);
    }

    #[test]
    fn a_swapped_pin_or_schema_moves_the_generation_and_stays_unverified() {
        let (machine, bytes) = both_sealed();
        let contract = machine.contract().clone();
        let tool = contract.target(&alpha()).unwrap().tool_id.clone();
        let changes: [fn(&mut AdmittedPins); 3] = [
            |admitted| admitted.pins.runtime_sha256 = sha_of(b"another runtime"),
            |admitted| admitted.pins.descriptor_sha256 = sha_of(b"another descriptor"),
            |admitted| admitted.schema_sha256 = sha_of(b"another schema"),
        ];
        for change in changes {
            let mut pins = current_pins(&contract);
            change(pins.get_mut(&tool).unwrap());
            let resumed = lower_saved(
                saved(&machine),
                &contract,
                &ResumeFacts {
                    process_nonce: "process-b".to_string(),
                    now_ms: NOW,
                    bytes: &bytes,
                    pins: &pins,
                },
            )
            .unwrap();
            assert_eq!(
                reason(&resumed, &alpha()),
                Some(UnverifiedReason::PinChanged)
            );
            assert_eq!(
                generation(&resumed, &alpha()),
                generation(&machine, &alpha()) + 1
            );
            assert_eq!(phase(&resumed, &beta()), Phase::NotReady);
        }
    }

    #[test]
    fn a_changed_output_is_named_and_takes_the_downstream() {
        let (machine, bytes) = both_sealed();
        let contract = machine.contract().clone();

        let edited_output = bytes.with("out/alpha.json", b"rewritten");
        let resumed = resume(saved(&machine), &contract, &edited_output);
        assert_eq!(
            reason(&resumed, &alpha()),
            Some(UnverifiedReason::BytesChanged)
        );
        assert_eq!(phase(&resumed, &beta()), Phase::NotReady);

        for damage in [
            bytes.without("out/alpha.json"),
            bytes.unreadable("out/alpha.json"),
        ] {
            let resumed = resume(saved(&machine), &contract, &damage);
            assert_eq!(
                reason(&resumed, &alpha()),
                Some(UnverifiedReason::BytesChanged)
            );
        }
    }

    /// `alpha` was sealed with "alpha v1", and a new attempt wrote its intent and went no further.
    fn intent_recorded() -> (StageMachine, FakeBytes) {
        let mut machine = StageMachine::new(
            Arc::new(two_stage_contract()),
            "run-a".to_string(),
            "process-a".to_string(),
            NOW,
        );
        let mut bytes = FakeBytes::default();
        bytes.put("fixed/source.json", b"fixed source");
        seal(&mut machine, &alpha(), b"alpha v1", &mut bytes, 1, NOW, "a");
        seal(&mut machine, &beta(), b"beta v1", &mut bytes, 2, NOW, "b");
        let revision = machine.revision_of(&alpha()).unwrap();
        let output = bytes.observe_path("out/alpha.json");
        let contract = machine.contract().clone();
        let BeginDecision::Proceed(ticket) = machine.begin(
            &alpha(),
            request_for(
                &contract,
                &alpha(),
                &revision,
                &bytes,
                3,
                NOW,
                "a2",
                &output,
            ),
        ) else {
            panic!("started");
        };
        assert_eq!(
            machine.record_local_result(
                &ticket,
                &sha_of(b"alpha v2"),
                passed_checks(&contract, &alpha()),
                &output,
            ),
            LocalDecision::ReadyToCommit
        );
        machine.record_intent(&ticket, output.sha256()).unwrap();
        (machine, bytes)
    }

    #[test]
    fn s15_the_candidate_on_disk_means_the_rename_stood() {
        let (machine, bytes) = intent_recorded();
        let contract = machine.contract().clone();
        let on_disk = bytes.with("out/alpha.json", b"alpha v2");
        let mut resumed = resume(saved(&machine), &contract, &on_disk);
        assert_eq!(phase(&resumed, &alpha()), Phase::Unverified);
        assert_eq!(
            reason(&resumed, &alpha()),
            Some(UnverifiedReason::InterruptedCommit)
        );
        assert_eq!(
            generation(&resumed, &alpha()),
            generation(&machine, &alpha()) + 1
        );
        assert_eq!(phase(&resumed, &beta()), Phase::NotReady);
        assert_eq!(resumed.record(&alpha()).unwrap().attempts_used, 2);
        assert!(resumed.record(&alpha()).unwrap().in_flight.is_none());
        assert!(!resumed.judge(&on_disk).complete());
    }

    #[test]
    fn s15_the_old_bytes_on_disk_mean_no_rename_and_the_attempt_stays_spent() {
        let (machine, bytes) = intent_recorded();
        let contract = machine.contract().clone();
        let mut resumed = resume(saved(&machine), &contract, &bytes);
        assert_eq!(
            generation(&resumed, &alpha()),
            generation(&machine, &alpha())
        );
        assert_eq!(reason(&resumed, &alpha()), Some(UnverifiedReason::Resumed));
        assert_eq!(resumed.record(&alpha()).unwrap().attempts_used, 2);
        assert_eq!(generation(&resumed, &beta()), generation(&machine, &beta()));
        assert!(!resumed.judge(&bytes).complete());
    }

    #[test]
    fn s15_anything_else_on_disk_takes_the_target_and_the_downstream() {
        let (machine, bytes) = intent_recorded();
        let contract = machine.contract().clone();
        for damage in [
            bytes.with("out/alpha.json", b"neither"),
            bytes.without("out/alpha.json"),
            bytes.unreadable("out/alpha.json"),
        ] {
            let mut resumed = resume(saved(&machine), &contract, &damage);
            assert_eq!(
                reason(&resumed, &alpha()),
                Some(UnverifiedReason::BytesChanged)
            );
            assert_eq!(
                generation(&resumed, &alpha()),
                generation(&machine, &alpha()) + 1
            );
            assert_eq!(phase(&resumed, &beta()), Phase::NotReady);
            assert!(!resumed.judge(&damage).complete());
        }
    }

    #[test]
    fn s15_a_first_commit_interrupted_with_nothing_there_is_no_rename() {
        let mut machine = StageMachine::new(
            Arc::new(two_stage_contract()),
            "run-a".to_string(),
            "process-a".to_string(),
            NOW,
        );
        let mut bytes = FakeBytes::default();
        bytes.put("fixed/source.json", b"fixed source");
        let absent = ObservedBytes::Absent;
        let contract = machine.contract().clone();
        let revision = machine.revision_of(&alpha()).unwrap();
        let BeginDecision::Proceed(ticket) = machine.begin(
            &alpha(),
            request_for(&contract, &alpha(), &revision, &bytes, 1, NOW, "a", &absent),
        ) else {
            panic!("started");
        };
        machine.record_local_result(
            &ticket,
            &sha_of(b"first"),
            passed_checks(&contract, &alpha()),
            &absent,
        );
        machine.record_intent(&ticket, None).unwrap();

        let resumed = resume(saved(&machine), &contract, &bytes);
        assert_eq!(phase(&resumed, &alpha()), Phase::Proposed);
        assert_eq!(generation(&resumed, &alpha()), 0);

        // A file that cannot be read matches neither side of the intent.
        let unreadable = bytes.unreadable("out/alpha.json");
        let resumed = resume(saved(&machine), &contract, &unreadable);
        assert_eq!(
            reason(&resumed, &alpha()),
            Some(UnverifiedReason::BytesChanged)
        );

        let there = bytes.with("out/alpha.json", b"first");
        let resumed = resume(saved(&machine), &contract, &there);
        assert_eq!(
            reason(&resumed, &alpha()),
            Some(UnverifiedReason::InterruptedCommit)
        );
        assert_eq!(generation(&resumed, &alpha()), 1);
    }

    #[test]
    fn an_attempt_cut_short_before_its_intent_is_only_a_spent_attempt() {
        let (mut machine, bytes) = both_sealed();
        let revision = machine.revision_of(&alpha()).unwrap();
        let output = bytes.observe_path("out/alpha.json");
        let contract = machine.contract().clone();
        assert!(
            machine
                .begin(
                    &alpha(),
                    request_for(
                        &contract,
                        &alpha(),
                        &revision,
                        &bytes,
                        3,
                        NOW,
                        "a2",
                        &output
                    ),
                )
                .starts_process()
        );
        let resumed = resume(saved(&machine), &contract, &bytes);
        assert_eq!(resumed.record(&alpha()).unwrap().attempts_used, 2);
        assert!(resumed.record(&alpha()).unwrap().in_flight.is_none());
        assert_eq!(reason(&resumed, &alpha()), Some(UnverifiedReason::Resumed));
    }

    #[test]
    fn a_locally_valid_result_is_not_believed_across_a_process() {
        let mut machine = StageMachine::new(
            Arc::new(two_stage_contract()),
            "run-a".to_string(),
            "process-a".to_string(),
            NOW,
        );
        let mut bytes = FakeBytes::default();
        bytes.put("fixed/source.json", b"fixed source");
        let absent = ObservedBytes::Absent;
        let contract = machine.contract().clone();
        let revision = machine.revision_of(&alpha()).unwrap();
        let BeginDecision::Proceed(ticket) = machine.begin(
            &alpha(),
            request_for(&contract, &alpha(), &revision, &bytes, 1, NOW, "a", &absent),
        ) else {
            panic!("started");
        };
        machine.record_local_result(
            &ticket,
            &sha_of(b"first"),
            passed_checks(&contract, &alpha()),
            &absent,
        );
        assert_eq!(phase(&machine, &alpha()), Phase::LocallyValid);
        let resumed = resume(saved(&machine), &contract, &bytes);
        assert_eq!(phase(&resumed, &alpha()), Phase::Proposed);
        assert!(resumed.record(&alpha()).unwrap().in_flight.is_none());
    }

    #[test]
    fn a_resumed_run_recovers_only_through_its_checks_upstream_first() {
        let (machine, mut bytes) = both_sealed();
        let contract = machine.contract().clone();
        let mut resumed = resume(saved(&machine), &contract, &bytes);

        // The downstream waits: its upstream is not sealed in this process.
        let revision = resumed.revision_of(&beta()).unwrap();
        let output = bytes.observe_path("out/beta.json");
        assert!(matches!(
            resumed.begin(
                &beta(),
                request_for(
                    &contract,
                    &beta(),
                    &revision,
                    &bytes,
                    10,
                    NOW + 2,
                    "b",
                    &output
                ),
            ),
            BeginDecision::NotReady { .. }
        ));

        seal(
            &mut resumed,
            &alpha(),
            b"alpha v1",
            &mut bytes,
            11,
            NOW + 2,
            "a",
        );
        assert_eq!(phase(&resumed, &beta()), Phase::NotReady);
        seal(
            &mut resumed,
            &beta(),
            b"beta v1",
            &mut bytes,
            12,
            NOW + 2,
            "b",
        );
        assert!(resumed.judge(&bytes).complete());
        assert_eq!(resumed.snapshot().attempts_used, 4);
        let receipt = resumed
            .record(&beta())
            .unwrap()
            .current_receipt
            .clone()
            .unwrap();
        assert_eq!(receipt.process_nonce, "process-b");
    }

    #[test]
    fn a_resume_gives_no_attempts_back_at_the_limit() {
        let (mut machine, bytes) = both_sealed();
        let limit = machine.contract().limits().attempts_per_target;
        machine.snapshot_mut().targets[0].attempts_used = limit;
        machine.snapshot_mut().attempts_used = limit + 1;
        let contract = machine.contract().clone();
        let mut resumed = resume(saved(&machine), &contract, &bytes);
        let revision = resumed.revision_of(&alpha()).unwrap();
        let output = bytes.observe_path("out/alpha.json");
        let decision = resumed.begin(
            &alpha(),
            request_for(
                &contract,
                &alpha(),
                &revision,
                &bytes,
                20,
                NOW + 2,
                "x",
                &output,
            ),
        );
        assert_eq!(
            decision,
            BeginDecision::AttemptsExhausted(ExhaustedLimit::Target)
        );
        assert_eq!(resumed.record(&alpha()).unwrap().attempts_used, limit);
    }

    #[test]
    fn a_later_turn_number_from_an_earlier_process_blocks_nothing() {
        let (machine, bytes) = both_sealed();
        assert_eq!(machine.snapshot().last_commit_turn, Some(2));
        let contract = machine.contract().clone();
        let mut resumed = resume(saved(&machine), &contract, &bytes);
        assert_eq!(resumed.snapshot().last_commit_turn, None);
        // The turn the earlier process last committed in means nothing to this one.
        let revision = resumed.revision_of(&alpha()).unwrap();
        let output = bytes.observe_path("out/alpha.json");
        assert!(
            resumed
                .begin(
                    &alpha(),
                    request_for(
                        &contract,
                        &alpha(),
                        &revision,
                        &bytes,
                        2,
                        NOW + 2,
                        "a",
                        &output,
                    ),
                )
                .starts_process()
        );
    }

    #[test]
    fn a_second_resume_keeps_every_count_and_loses_nothing_it_had() {
        let (machine, bytes) = both_sealed();
        let contract = machine.contract().clone();
        let first = resume(saved(&machine), &contract, &bytes);
        let second = resume(saved(&first), &contract, &bytes);
        assert_eq!(second.snapshot().attempts_used, 2);
        for target in [alpha(), beta()] {
            let record = second.record(&target).unwrap();
            assert_eq!(record.phase, Phase::Unverified);
            assert_eq!(record.unverified_reason, Some(UnverifiedReason::Resumed));
            assert_eq!(record.attempts_used, 1);
            assert_eq!(record.generation, generation(&machine, &target));
            assert_eq!(record.history.len(), 1);
        }
        assert_eq!(second.snapshot().run_nonce, "run-a");
        assert!(second.record(&alpha()).unwrap().fixed_inputs.is_some());
    }

    fn resume_outcome(
        machine: &StageMachine,
        bytes: &FakeBytes,
    ) -> Result<StageMachine, SavedRefusal> {
        let contract = machine.contract().clone();
        let pins = current_pins(&contract);
        lower_saved(
            saved(machine),
            &contract,
            &ResumeFacts {
                process_nonce: "process-c".to_string(),
                now_ms: NOW + 1,
                bytes,
                pins: &pins,
            },
        )
    }

    #[test]
    fn a_fixed_input_changed_before_a_later_resume_refuses_that_one_too() {
        let (machine, bytes) = both_sealed();
        let contract = machine.contract().clone();
        let first = resume(saved(&machine), &contract, &bytes);
        let changed = bytes.with("fixed/source.json", b"rewritten after the first resume");
        assert_eq!(
            resume_outcome(&first, &changed).unwrap_err(),
            SavedRefusal::FixedInputChanged {
                path: "fixed/source.json".to_string()
            }
        );
    }

    #[test]
    fn a_fixed_input_gone_or_unreadable_refuses_the_resume_as_well() {
        let (machine, bytes) = both_sealed();
        for damaged in [
            bytes.without("fixed/source.json"),
            bytes.unreadable("fixed/source.json"),
        ] {
            assert_eq!(
                resume_outcome(&machine, &damaged).unwrap_err(),
                SavedRefusal::FixedInputChanged {
                    path: "fixed/source.json".to_string()
                }
            );
        }
    }

    #[test]
    fn a_changed_upstream_output_is_an_invalidation_and_not_a_refusal() {
        // The downstream's input is the upstream's output. The same kind of edit as above, on an
        // input that has an owner, lowers instead of refusing.
        let (machine, bytes) = both_sealed();
        let edited = bytes.with("out/alpha.json", b"edited behind the host");
        let resumed = resume_outcome(&machine, &edited).unwrap();
        assert_eq!(phase(&resumed, &alpha()), Phase::Unverified);
        assert_eq!(
            reason(&resumed, &alpha()),
            Some(UnverifiedReason::BytesChanged)
        );
        assert_eq!(phase(&resumed, &beta()), Phase::NotReady);
        assert!(
            resumed
                .record(&beta())
                .unwrap()
                .fixed_inputs
                .iter()
                .flatten()
                .next()
                .is_none(),
            "the downstream pinned nothing: all its input comes from upstream"
        );
    }

    /// Remembers what it was asked to read.
    struct WatchedBytes {
        bytes: FakeBytes,
        asked: RefCell<Vec<String>>,
    }

    impl WatchedBytes {
        fn over(bytes: &FakeBytes) -> Self {
            Self {
                bytes: bytes.clone(),
                asked: RefCell::new(Vec::new()),
            }
        }

        fn was_asked(&self, path: &str) -> bool {
            self.asked.borrow().iter().any(|asked| asked == path)
        }
    }

    impl BytesSource for WatchedBytes {
        fn observe(&self, path: &str) -> ObservedBytes {
            self.asked.borrow_mut().push(path.to_string());
            self.bytes.observe_path(path)
        }
    }

    fn lower_watched(
        forged: MachineSnapshot,
        contract: &Arc<StageContract>,
        bytes: &WatchedBytes,
    ) -> Result<StageMachine, SavedRefusal> {
        let pins = current_pins(contract);
        lower_saved(
            forged,
            contract,
            &ResumeFacts {
                process_nonce: "process-d".to_string(),
                now_ms: NOW + 1,
                bytes,
                pins: &pins,
            },
        )
    }

    #[test]
    fn a_pin_the_contract_does_not_name_for_its_target_is_refused_before_anything_is_read() {
        let (machine, mut bytes) = both_sealed();
        bytes.put("elsewhere/outside.json", b"outside bytes");
        let contract = machine.contract().clone();

        let honest = WatchedBytes::over(&bytes);
        assert!(validate_saved(&saved(&machine), &contract).is_ok());
        assert!(lower_watched(saved(&machine), &contract, &honest).is_ok());
        assert!(honest.was_asked("fixed/source.json"));

        for (index, path, why) in [
            (0, "elsewhere/outside.json", "not in the contract at all"),
            (1, "fixed/source.json", "another target's fixed input"),
            (1, "out/alpha.json", "an input from upstream is not fixed"),
        ] {
            let mut forged = saved(&machine);
            forged.targets[index].fixed_inputs = Some(vec![InputDigest {
                path: path.to_string(),
                sha256: bytes.observe_path(path).sha256().unwrap(),
            }]);
            assert_eq!(
                validate_saved(&forged, &contract),
                Err(SavedRefusal::PinOutsideContract),
                "{why}"
            );
            let watched = WatchedBytes::over(&bytes);
            assert_eq!(
                lower_watched(forged, &contract, &watched).unwrap_err(),
                SavedRefusal::PinOutsideContract,
                "{why}"
            );
            assert!(watched.asked.borrow().is_empty(), "{why}");
        }
    }

    #[test]
    fn a_receipt_input_the_contract_does_not_name_is_not_read_and_lowers() {
        let (machine, mut bytes) = both_sealed();
        bytes.put("elsewhere/outside.json", b"outside bytes");
        let contract = machine.contract().clone();
        let mut forged = saved(&machine);
        forged.targets[1]
            .current_receipt
            .as_mut()
            .unwrap()
            .inputs
            .push(InputDigest {
                path: "elsewhere/outside.json".to_string(),
                sha256: sha_of(b"outside bytes"),
            });
        let watched = WatchedBytes::over(&bytes);
        let resumed = lower_watched(forged, &contract, &watched).unwrap();
        assert!(!watched.was_asked("elsewhere/outside.json"));
        assert_eq!(phase(&resumed, &beta()), Phase::Unverified);
        assert_eq!(
            reason(&resumed, &beta()),
            Some(UnverifiedReason::InputChanged)
        );
    }

    /// `beta` reads `alpha`'s output and a fixed input of its own.
    fn downstream_with_a_fixed_input() -> Arc<StageContract> {
        let declaration = StageSetDeclaration {
            stages: vec![one_target("alpha", "main"), one_target("beta", "beta-tool")],
            limits: NarrowedStageLimits::default(),
        };
        let tools = [
            facts("main", "out/alpha.json", &["fixed/source.json"]),
            facts(
                "beta-tool",
                "out/beta.json",
                &["out/alpha.json", "fixed/beta-source.json"],
            ),
        ];
        Arc::new(admit_enforced(&declaration, &tools).unwrap())
    }

    #[test]
    fn a_downstream_fixed_input_changed_before_it_ever_ran_refuses_the_resume() {
        let contract = downstream_with_a_fixed_input();
        let mut machine = StageMachine::new(
            contract.clone(),
            "run-a".to_string(),
            "process-a".to_string(),
            NOW,
        );
        let upstream = key("alpha", "main");
        let mut bytes = FakeBytes::default();
        bytes.put("fixed/source.json", b"fixed source");
        bytes.put("fixed/beta-source.json", b"as adopted");
        seal(
            &mut machine,
            &upstream,
            b"alpha v1",
            &mut bytes,
            1,
            NOW,
            "a",
        );
        let target = key("beta", "beta-tool");
        assert_eq!(generation(&machine, &upstream), 1);
        assert_eq!(machine.record(&target).unwrap().attempts_used, 0);

        assert!(resume_outcome(&machine, &bytes).is_ok());
        let changed = bytes.with("fixed/beta-source.json", b"rewritten by a command");
        assert_eq!(
            resume_outcome(&machine, &changed).unwrap_err(),
            SavedRefusal::FixedInputChanged {
                path: "fixed/beta-source.json".to_string()
            }
        );
    }

    /// An attempt under way on `alpha`, as a crash or a diverged file would find it.
    fn mid_attempt(machine: &mut StageMachine, bytes: &FakeBytes) {
        let contract = machine.contract().clone();
        let revision = machine.revision_of(&alpha()).unwrap();
        let output = bytes.observe_path("out/alpha.json");
        let decision = machine.begin(
            &alpha(),
            request_for(
                &contract,
                &alpha(),
                &revision,
                bytes,
                3,
                NOW,
                "again",
                &output,
            ),
        );
        assert!(
            matches!(decision, BeginDecision::Proceed(_)),
            "{decision:?}"
        );
        assert_eq!(phase(machine, &alpha()), Phase::Proposed);
    }

    fn unverified_mid_attempt() -> (StageMachine, FakeBytes) {
        let (mut machine, bytes) = both_sealed();
        let edited = bytes.with("out/alpha.json", b"edited behind the host");
        assert!(!machine.judge(&edited).complete());
        mid_attempt(&mut machine, &edited);
        (machine, edited)
    }

    fn never_held_mid_attempt() -> (StageMachine, FakeBytes) {
        let mut machine = StageMachine::new(
            Arc::new(two_stage_contract()),
            "run-a".to_string(),
            "process-a".to_string(),
            NOW,
        );
        let mut bytes = FakeBytes::default();
        bytes.put("fixed/source.json", b"fixed source");
        mid_attempt(&mut machine, &bytes);
        (machine, bytes)
    }

    /// T4: the phase and the reason agree, whichever way the attempt was found.
    #[test]
    fn an_attempt_found_on_a_resume_leaves_the_phase_and_the_reason_in_agreement() {
        let (machine, bytes) = unverified_mid_attempt();
        let contract = machine.contract().clone();
        let resumed = resume(saved(&machine), &contract, &bytes);
        assert_eq!(phase(&resumed, &alpha()), Phase::Unverified);
        assert_eq!(
            reason(&resumed, &alpha()),
            Some(UnverifiedReason::BytesChanged)
        );

        let (machine, bytes) = never_held_mid_attempt();
        let contract = machine.contract().clone();
        let resumed = resume(saved(&machine), &contract, &bytes);
        assert_eq!(phase(&resumed, &alpha()), Phase::Proposed);
        assert_eq!(reason(&resumed, &alpha()), None);
    }

    #[test]
    fn an_attempt_found_under_a_diverged_file_leaves_the_phase_and_the_reason_in_agreement() {
        let (mut machine, _) = unverified_mid_attempt();
        let disk = machine.snapshot().clone();
        absorb_diverged(machine.snapshot_mut(), &disk);
        assert_eq!(phase(&machine, &alpha()), Phase::Unverified);
        assert_eq!(
            reason(&machine, &alpha()),
            Some(UnverifiedReason::BytesChanged)
        );

        let (mut machine, _) = never_held_mid_attempt();
        let disk = machine.snapshot().clone();
        absorb_diverged(machine.snapshot_mut(), &disk);
        assert_eq!(phase(&machine, &alpha()), Phase::Proposed);
        assert_eq!(reason(&machine, &alpha()), None);
    }

    /// F6: what ended the run goes on ending it, from a resume and from a diverged file.
    #[test]
    fn a_secret_ending_the_run_survives_a_resume_and_a_diverged_file() {
        let (mut machine, bytes) = both_sealed();
        machine.snapshot_mut().ended_by_secret_value = true;
        let contract = machine.contract().clone();
        let resumed = resume(saved(&machine), &contract, &bytes);
        assert!(resumed.snapshot().ended_by_secret_value);

        let (mut held, _) = both_sealed();
        let mut disk = held.snapshot().clone();
        disk.ended_by_secret_value = true;
        absorb_diverged(held.snapshot_mut(), &disk);
        assert!(held.snapshot().ended_by_secret_value);
    }

    /// The limit of the pin: a change before G2 first adopted the input is what gets pinned.
    #[test]
    fn a_fixed_input_changed_before_the_first_begin_cannot_be_seen() {
        let machine = StageMachine::new(
            Arc::new(two_stage_contract()),
            "run-a".to_string(),
            "process-a".to_string(),
            NOW,
        );
        let mut bytes = FakeBytes::default();
        bytes.put("fixed/source.json", b"as the first process saw it");
        let changed = bytes.with("fixed/source.json", b"changed before anything started");
        let contract = machine.contract().clone();
        let mut resumed = resume_outcome(&machine, &changed).unwrap();
        assert_eq!(phase(&resumed, &alpha()), Phase::NotReady);
        assert_eq!(generation(&resumed, &alpha()), 0);

        // Its first attempt pins what is there now, and from then on it is held to that.
        let mut held = changed.clone();
        seal(&mut resumed, &alpha(), b"first", &mut held, 1, NOW + 2, "a");
        assert_eq!(
            resumed.record(&alpha()).unwrap().fixed_inputs,
            Some(current_inputs(&contract, &alpha(), &changed))
        );
        let again = held.with("fixed/source.json", b"changed once more");
        assert!(matches!(
            resume_outcome(&resumed, &again).unwrap_err(),
            SavedRefusal::FixedInputChanged { .. }
        ));
    }

    #[test]
    fn a_file_that_diverged_gives_no_time_back_and_keeps_the_highest_generation() {
        let (mut held, _) = both_sealed();
        let mut disk = held.snapshot().clone();
        disk.started_at_ms = NOW - 100;
        disk.latest_clock_ms = NOW + 900;
        disk.targets[1].generation = 30;
        absorb_diverged(held.snapshot_mut(), &disk);
        assert_eq!(held.snapshot().started_at_ms, NOW - 100);
        assert_eq!(held.snapshot().latest_clock_ms, NOW + 900);
        assert_eq!(held.record(&beta()).unwrap().generation, 31);

        // The other way round, this process's values stand.
        let (mut held, _) = both_sealed();
        let mut disk = held.snapshot().clone();
        disk.started_at_ms = NOW + 100;
        disk.latest_clock_ms = NOW - 100;
        absorb_diverged(held.snapshot_mut(), &disk);
        assert_eq!(held.snapshot().started_at_ms, NOW);
        assert_eq!(held.snapshot().latest_clock_ms, NOW);
    }

    #[test]
    fn a_file_that_diverged_leaves_an_intent_its_way_out() {
        let (mut machine, _) = intent_recorded();
        let disk = machine.snapshot().clone();
        absorb_diverged(machine.snapshot_mut(), &disk);
        assert!(machine.record(&alpha()).unwrap().has_recorded_intent());
        assert_eq!(phase(&machine, &alpha()), Phase::Unverified);
    }

    #[test]
    fn a_record_that_does_not_fit_is_refused_and_not_repaired() {
        let (machine, _) = both_sealed();
        let contract = machine.contract().clone();

        let mut wrong = saved(&machine);
        wrong.schema = "structured-stage.ledger/v0".to_string();
        assert_eq!(
            validate_saved(&wrong, &contract),
            Err(SavedRefusal::WrongSchema)
        );

        let mut wrong = saved(&machine);
        wrong.run_nonce.clear();
        assert_eq!(
            validate_saved(&wrong, &contract),
            Err(SavedRefusal::WrongSchema)
        );

        let mut wrong = saved(&machine);
        wrong.contract_sha256 = sha_of(b"another contract");
        assert_eq!(
            validate_saved(&wrong, &contract),
            Err(SavedRefusal::ContractChanged)
        );

        let mut wrong = saved(&machine);
        wrong.targets.pop();
        assert_eq!(
            validate_saved(&wrong, &contract),
            Err(SavedRefusal::TargetsDiffer)
        );

        let mut wrong = saved(&machine);
        wrong.targets.swap(0, 1);
        assert_eq!(
            validate_saved(&wrong, &contract),
            Err(SavedRefusal::TargetsDiffer)
        );

        let mut wrong = saved(&machine);
        wrong.attempts_used = 0;
        assert_eq!(
            validate_saved(&wrong, &contract),
            Err(SavedRefusal::CountsInconsistent)
        );

        let mut wrong = saved(&machine);
        wrong.targets[0].attempts_used = u32::MAX;
        assert_eq!(
            validate_saved(&wrong, &contract),
            Err(SavedRefusal::CountsInconsistent)
        );
    }

    #[test]
    fn a_tampered_record_cannot_lower_a_count_or_raise_a_state_in_a_live_process() {
        let (mut held, _) = both_sealed();
        let mut disk = held.snapshot().clone();
        // Counts cut down on the file.
        disk.attempts_used = 0;
        disk.targets[0].attempts_used = 0;
        disk.targets[0].generation = 0;
        let before_attempts = held.snapshot().attempts_used;
        let before_generation = held.record(&alpha()).unwrap().generation;
        absorb_diverged(held.snapshot_mut(), &disk);
        assert_eq!(held.snapshot().attempts_used, before_attempts);
        assert_eq!(held.record(&alpha()).unwrap().attempts_used, 1);
        assert_eq!(
            held.record(&alpha()).unwrap().generation,
            before_generation + 1
        );
        assert_eq!(phase(&held, &alpha()), Phase::Unverified);
        assert_eq!(phase(&held, &beta()), Phase::Unverified);

        // Counts higher on the file are taken.
        let (mut held, _) = both_sealed();
        let mut disk = held.snapshot().clone();
        disk.attempts_used = 40;
        disk.targets[0].attempts_used = 7;
        absorb_diverged(held.snapshot_mut(), &disk);
        assert_eq!(held.snapshot().attempts_used, 40);
        assert_eq!(held.record(&alpha()).unwrap().attempts_used, 7);
    }

    #[test]
    fn counts_taken_from_a_file_stay_a_record_the_next_resume_can_read() {
        let (mut held, _) = both_sealed();
        let contract = held.contract().clone();
        let mut disk = held.snapshot().clone();
        disk.targets[0].attempts_used = 0;
        disk.targets[1].attempts_used = 5;
        disk.attempts_used = 5;
        absorb_diverged(held.snapshot_mut(), &disk);
        assert_eq!(held.record(&alpha()).unwrap().attempts_used, 1);
        assert_eq!(held.record(&beta()).unwrap().attempts_used, 5);
        assert_eq!(validate_saved(held.snapshot(), &contract), Ok(()));
        assert_eq!(held.snapshot().attempts_used, 6);
    }

    #[test]
    fn a_changed_fixed_input_refuses_the_resume() {
        let (machine, bytes) = both_sealed();
        let changed = bytes.with("fixed/source.json", b"rewritten while nobody watched");
        assert_eq!(
            resume_outcome(&machine, &changed).unwrap_err(),
            SavedRefusal::FixedInputChanged {
                path: "fixed/source.json".to_string()
            }
        );
    }

    #[test]
    fn a_forged_check_list_in_a_saved_receipt_gets_no_further_than_the_honest_one() {
        let (machine, bytes) = both_sealed();
        let contract = machine.contract().clone();
        let honest = resume(saved(&machine), &contract, &bytes);
        let mut forged = saved(&machine);
        let receipt = forged.targets[0].current_receipt.as_mut().unwrap();
        receipt.checks = vec![CheckResult {
            id: "claimed".to_string(),
            status: CheckStatus::Passed,
        }];
        let resumed = resume(forged, &contract, &bytes);
        for target in [alpha(), beta()] {
            assert_eq!(phase(&resumed, &target), phase(&honest, &target));
            assert_eq!(reason(&resumed, &target), reason(&honest, &target));
            assert_eq!(generation(&resumed, &target), generation(&honest, &target));
        }
        assert_eq!(phase(&resumed, &alpha()), Phase::Unverified);
        // It stays only as a record, where no judgment reads it.
        let record = resumed.record(&alpha()).unwrap();
        assert!(record.current_receipt.is_none());
        assert_eq!(record.history[0].checks[0].id, "claimed");
    }
}
