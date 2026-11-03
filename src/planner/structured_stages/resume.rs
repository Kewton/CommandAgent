//! What a saved record can do when another process picks it up (Issue #975, G4): lower.
//!
//! A saved record is never evidence. Attempts and generations are kept so nothing refills, and
//! the real bytes decide how far a target falls. No target comes out sealed, whatever the file
//! says; each must pass its required checks again in this process (D11).

use std::collections::BTreeMap;
use std::sync::Arc;

use super::contract::{StageContract, TargetKey};
use super::invalidation::invalidate_downstream;
use super::receipt::ToolPins;
use super::state::{
    BytesSource, InFlightProgress, MachineSnapshot, ObservedBytes, Phase, SNAPSHOT_SCHEMA,
    StageMachine, TargetRecord, UnverifiedReason,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SavedRefusal {
    WrongSchema,
    ContractChanged,
    TargetsDiffer,
    CountsInconsistent,
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
    tool_id: &str,
    facts: &ResumeFacts<'_>,
) -> Option<UnverifiedReason> {
    let receipt = record.current_receipt.as_ref()?;
    if observed.sha256().as_deref() != Some(receipt.committed_sha256.as_str()) {
        return Some(UnverifiedReason::BytesChanged);
    }
    let pinned = facts.pins.get(tool_id);
    if pinned.is_none_or(|admitted| {
        admitted.pins != receipt.pins || admitted.schema_sha256 != receipt.schema_sha256
    }) {
        return Some(UnverifiedReason::PinChanged);
    }
    let input_moved = receipt.inputs.iter().any(|input| {
        facts.bytes.observe(&input.path).sha256().as_deref() != Some(input.sha256.as_str())
    });
    input_moved.then_some(UnverifiedReason::InputChanged)
}

fn fixed_input_moved(record: &TargetRecord, facts: &ResumeFacts<'_>) -> bool {
    record.fixed_inputs.iter().flatten().any(|input| {
        facts.bytes.observe(&input.path).sha256().as_deref() != Some(input.sha256.as_str())
    })
}

fn lower_one(
    record: &mut TargetRecord,
    output_path: &str,
    tool_id: &str,
    facts: &ResumeFacts<'_>,
) -> bool {
    let observed = facts.bytes.observe(output_path);
    let interrupted = judge_interrupted_commit(record, &observed);
    let saved = if interrupted.bytes_moved {
        None
    } else {
        judge_saved_receipt(record, &observed, tool_id, facts)
    };
    let inputs_moved = fixed_input_moved(record, facts);
    let reason = interrupted
        .reason
        .or(saved)
        .or(inputs_moved.then_some(UnverifiedReason::InputChanged));
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
        None => {
            record.in_flight = None;
            if record.phase == Phase::LocallyValid {
                record.phase = Phase::Proposed;
            }
        }
    }
    // The next attempt adopts what is there now, and has lost any state resting on the old.
    record.fixed_inputs = None;
    moved
}

/// The machine a resumed process starts from. Everything it may keep is a count.
pub(super) fn lower_saved(
    saved: MachineSnapshot,
    contract: &Arc<StageContract>,
    facts: &ResumeFacts<'_>,
) -> Result<StageMachine, SavedRefusal> {
    validate_saved(&saved, contract)?;
    let mut snapshot = saved;
    snapshot.process_nonce = facts.process_nonce.clone();
    snapshot.latest_clock_ms = snapshot.latest_clock_ms.max(facts.now_ms);
    snapshot.last_commit_turn = None;
    let mut moved = Vec::<TargetKey>::new();
    for record in &mut snapshot.targets {
        let Some(target) = contract.target(&record.key) else {
            continue;
        };
        if lower_one(record, &target.output_path, &target.tool_id, facts) {
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
/// file was not what this process wrote, nothing here is vouched for any more.
pub(crate) fn absorb_diverged(held: &mut MachineSnapshot, disk: &MachineSnapshot) {
    held.attempts_used = held.attempts_used.max(disk.attempts_used);
    held.started_at_ms = held.started_at_ms.min(disk.started_at_ms);
    held.latest_clock_ms = held.latest_clock_ms.max(disk.latest_clock_ms);
    held.last_commit_turn = None;
    for record in &mut held.targets {
        let saved = disk.targets.iter().find(|saved| saved.key == record.key);
        if let Some(saved) = saved {
            record.attempts_used = record.attempts_used.max(saved.attempts_used);
            record.generation = record.generation.max(saved.generation);
        }
        record.generation += 1;
        record.in_flight = None;
        if record.phase == Phase::Sealed || record.current_receipt.is_some() {
            record.lower_to_unverified(UnverifiedReason::LedgerDiverged);
        } else if record.phase == Phase::LocallyValid {
            record.phase = Phase::Proposed;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Arc;

    use super::super::contract::{StageContract, TargetKey};
    use super::super::receipt::{CheckResult, CheckStatus};
    use super::super::state::{
        BeginDecision, LocalDecision, MachineSnapshot, ObservedBytes, Phase, StageMachine,
        UnverifiedReason,
    };
    use super::super::test_support::{
        FakeBytes, key, passed_checks, pins_for, request_for, schema_for, seal, sha_of,
        two_stage_contract,
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
        assert!(!resumed.judge(&bytes).complete);
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
        assert!(!resumed.judge(&bytes).complete);

        // And with the upstream's receipt pasted in.
        forged.targets[1].current_receipt = forged.targets[0].current_receipt.clone();
        let mut resumed = resume(forged, &contract, &bytes);
        assert_ne!(phase(&resumed, &beta()), Phase::Sealed);
        assert!(!resumed.judge(&bytes).complete);
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
    fn a_changed_input_or_output_is_named_and_takes_the_downstream() {
        let (machine, bytes) = both_sealed();
        let contract = machine.contract().clone();

        let edited_input = bytes.with("fixed/source.json", b"rewritten");
        let resumed = resume(saved(&machine), &contract, &edited_input);
        assert_eq!(
            reason(&resumed, &alpha()),
            Some(UnverifiedReason::InputChanged)
        );
        assert_eq!(phase(&resumed, &beta()), Phase::NotReady);

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
        assert!(!resumed.judge(&on_disk).complete);
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
        assert!(!resumed.judge(&bytes).complete);
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
            assert!(!resumed.judge(&damage).complete);
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
        assert!(resumed.judge(&bytes).complete);
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
        assert!(decision.ends_run(), "{decision:?}");
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
    fn a_check_status_in_a_saved_receipt_is_only_a_record() {
        let (machine, bytes) = both_sealed();
        let mut forged = saved(&machine);
        let receipt = forged.targets[0].current_receipt.as_mut().unwrap();
        receipt.checks = vec![CheckResult {
            id: "claimed".to_string(),
            status: CheckStatus::Passed,
        }];
        let contract = machine.contract().clone();
        let resumed = resume(forged, &contract, &bytes);
        assert_eq!(phase(&resumed, &alpha()), Phase::Unverified);
    }
}
