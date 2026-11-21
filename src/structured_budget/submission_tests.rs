//! Each tool call a run looks at is one submission of its wallet, tied to the response it came in
//! by the host, and a refused one is the command's. Every send here is reserved, started, and
//! settled on a fake clock with a known report in place of a response; nothing reaches a network.

use std::path::Path;
use std::sync::Arc;

use serde_json::json;

use super::super::identity::{AttemptBinding, LogicalKind, PhysicalId, ResponseIdentity};
use super::super::ledger::LedgerFailure;
use super::super::reservation::BudgetRefusal;
use super::super::test_support::{TestWallet, stage_kind};
use super::super::transport::{RunBudgetHandle, SendRefusal};
use super::{CallBudget, for_call};
use crate::config::Config;

fn config_on(workspace: &Path, handle: Option<Arc<RunBudgetHandle>>) -> Config {
    let parsed = crate::provider_cli::parse_from([
        "commandagent",
        "--cwd",
        workspace.to_str().unwrap(),
        "--state-dir",
        workspace.parent().unwrap().join("state").to_str().unwrap(),
    ])
    .unwrap();
    let mut config =
        Config::from_cli_with_provider_options(parsed.cli, parsed.provider_options).unwrap();
    config.eval_events_path = None;
    config.field_sources.run_budget = handle;
    config
}

/// One send of the wallet, settled, as a client's is: the identity is what the settling gave.
fn settled(wallet: &TestWallet) -> ResponseIdentity {
    wallet.settled_response()
}

fn call(wallet: &TestWallet, response: Option<ResponseIdentity>, index: usize) -> CallBudget {
    for_call(
        &config_on(&wallet.workspace, Some(wallet.handle.clone())),
        response,
        index,
    )
    .expect("a run with a wallet counts its calls")
}

fn counts(wallet: &TestWallet) -> (u64, u64) {
    let state = wallet.handle.wallet().unwrap();
    (state.physical_used(), state.logical_global_used())
}

fn refused(refusal: BudgetRefusal) -> SendRefusal {
    SendRefusal::Wallet(LedgerFailure::Budget(refusal))
}

fn binding(stage: &str, attempt: u32) -> AttemptBinding {
    AttemptBinding {
        run_nonce: "nonce-976".to_string(),
        stage: stage.to_string(),
        target: "main".to_string(),
        attempt,
        generation: 1,
    }
}

#[test]
fn the_calls_of_one_response_are_one_submission_each_of_its_one_send() {
    let wallet = TestWallet::open(|_| {});
    let response = settled(&wallet);
    for index in 0..3 {
        call(&wallet, Some(response), index)
            .record(LogicalKind::General)
            .unwrap();
    }
    assert_eq!(counts(&wallet), (1, 3));
    let state = wallet.handle.wallet().unwrap();
    let sources = state
        .logicals
        .iter()
        .map(|logical| logical.source)
        .collect::<Vec<_>>();
    assert_eq!(
        sources,
        (0..3)
            .map(|index| Some(response.call(index)))
            .collect::<Vec<_>>()
    );
    assert_eq!(wallet.handle.refused_in_command(), None);
}

#[test]
fn a_call_no_response_stands_behind_is_counted_and_starts_the_wall() {
    let wallet = TestWallet::open(|_| {});
    assert_eq!(
        wallet.handle.wallet().unwrap().clock.started_wall_ms,
        None,
        "nothing has started the run yet"
    );
    call(&wallet, None, 0).record(LogicalKind::General).unwrap();
    call(&wallet, None, 0).record(LogicalKind::General).unwrap();
    let state = wallet.handle.wallet().unwrap();
    assert_eq!((state.physical_used(), state.logical_global_used()), (0, 2));
    assert_eq!(state.clock.started_wall_ms, Some(7_000));
    assert!(
        state
            .logicals
            .iter()
            .all(|logical| logical.source.is_none())
    );
}

#[test]
fn a_call_past_the_wall_is_refused_and_kept_for_the_command() {
    let wallet = TestWallet::open(|value| value["limits"]["wall_seconds"] = json!(10));
    let response = settled(&wallet);
    call(&wallet, Some(response), 0)
        .record(LogicalKind::General)
        .unwrap();
    wallet.clock.advance(10_000);
    call(&wallet, Some(response), 1)
        .record(LogicalKind::General)
        .unwrap();
    wallet.clock.advance(1);
    let refusal = call(&wallet, Some(response), 2)
        .record(LogicalKind::General)
        .unwrap_err();
    assert_eq!(refusal, refused(BudgetRefusal::WallExceeded));
    assert_eq!(counts(&wallet), (1, 2));
    assert_eq!(wallet.handle.refused_in_command(), Some(refusal));
}

/// The global limit, exactly at it and one past it, and the stage's alike.
#[test]
fn a_call_past_a_logical_limit_is_refused_and_nothing_more_is_recorded() {
    for (limit, kind, refusal) in [
        (
            "logical_global",
            LogicalKind::General,
            BudgetRefusal::LogicalGlobalExceeded,
        ),
        (
            "logical_per_stage",
            stage_kind("upper"),
            BudgetRefusal::LogicalStageExceeded,
        ),
    ] {
        let wallet = TestWallet::open(|value| value["limits"][limit] = json!(2));
        let response = settled(&wallet);
        for index in 0..2 {
            call(&wallet, Some(response), index)
                .record(kind.clone())
                .unwrap();
        }
        let refused_now = call(&wallet, Some(response), 2)
            .record(kind.clone())
            .unwrap_err();
        assert_eq!(refused_now, refused(refusal), "{limit}");
        assert_eq!(wallet.handle.refused_in_command(), Some(refused_now));
        // Another stage, which its own limit would let through, is not recorded either: the
        // command was refused.
        let after = call(&wallet, Some(response), 3)
            .record(stage_kind("lower"))
            .unwrap_err();
        assert_eq!(after, refused_now, "{limit}");
        assert_eq!(counts(&wallet), (1, 2), "{limit}");
    }
}

/// A refused send of the command stops its calls too, before the ledger is asked.
#[test]
fn after_a_refused_send_no_call_of_the_command_is_recorded() {
    let wallet = TestWallet::open(|value| value["limits"]["physical_sends"] = json!(1));
    let response = settled(&wallet);
    let send = wallet.send(50, None).unwrap_err();
    assert_eq!(send, refused(BudgetRefusal::PhysicalExceeded));
    let refusal = call(&wallet, Some(response), 0)
        .record(LogicalKind::General)
        .unwrap_err();
    assert_eq!(refusal, send);
    assert_eq!(counts(&wallet), (1, 0));
    // A REPL's next command begins with none, and is judged by the wallet as it is.
    wallet.handle.forget_command_refusals();
    call(&wallet, Some(response), 0)
        .record(LogicalKind::General)
        .unwrap();
    assert_eq!(counts(&wallet), (1, 1));
}

/// The response is the host's: one the wallet does not hold settled, or a call counted already,
/// is refused, and so is the command.
#[test]
fn a_call_is_of_a_settled_response_and_of_one_place_in_it() {
    let wallet = TestWallet::open(|_| {});
    let response = settled(&wallet);
    call(&wallet, Some(response), 0)
        .record(LogicalKind::General)
        .unwrap();
    let again = call(&wallet, Some(response), 0)
        .record(LogicalKind::General)
        .unwrap_err();
    assert_eq!(again, refused(BudgetRefusal::DuplicateSubmission));
    assert_eq!(wallet.handle.refused_in_command(), Some(again));
    assert_eq!(counts(&wallet), (1, 1));

    let other = TestWallet::open(|_| {});
    let unknown = ResponseIdentity::of_settled(PhysicalId(9));
    let refusal = call(&other, Some(unknown), 0)
        .record(LogicalKind::General)
        .unwrap_err();
    assert_eq!(refusal, refused(BudgetRefusal::NoSuchSend));
    assert_eq!(counts(&other), (0, 0));
}

#[test]
fn a_started_attempt_is_tied_to_its_submission_once() {
    let wallet = TestWallet::open(|_| {});
    let response = settled(&wallet);
    let submission = call(&wallet, Some(response), 0)
        .record(stage_kind("upper"))
        .unwrap();
    submission.bind(Some(binding("upper", 1))).unwrap();
    submission.bind(Some(binding("upper", 1))).unwrap();
    let state = wallet.handle.wallet().unwrap();
    assert_eq!(state.logicals[0].attempt, Some(binding("upper", 1)));
    assert_eq!(wallet.handle.refused_in_command(), None);

    let refusal = submission.bind(Some(binding("upper", 2))).unwrap_err();
    assert_eq!(refusal, refused(BudgetRefusal::AlreadyBound));
    assert_eq!(wallet.handle.refused_in_command(), Some(refusal));
}

#[test]
fn an_attempt_that_cannot_be_tied_is_refused_for_the_command() {
    for (kind, attempt, expected) in [
        (
            LogicalKind::General,
            Some(binding("upper", 1)),
            refused(BudgetRefusal::BindingStageDiffers),
        ),
        (
            stage_kind("lower"),
            Some(binding("upper", 1)),
            refused(BudgetRefusal::BindingStageDiffers),
        ),
        (stage_kind("upper"), None, SendRefusal::Unavailable),
    ] {
        let wallet = TestWallet::open(|_| {});
        let submission = call(&wallet, None, 0).record(kind).unwrap();
        assert_eq!(submission.bind(attempt), Err(expected));
        assert_eq!(wallet.handle.refused_in_command(), Some(expected));
        let state = wallet.handle.wallet().unwrap();
        assert_eq!(state.logicals[0].attempt, None);
    }
}

/// The word a stage call the wallet could not count ends with survives a scrub as itself.
#[test]
fn the_word_of_a_stage_call_the_wallet_refused_is_a_fixed_identifier() {
    assert!(crate::sensitive_data::is_fixed_identifier(
        "run_budget_refused"
    ));
}

/// Off: no wallet, no budget for a call, and nothing of a wallet is read.
#[test]
fn with_no_wallet_a_call_has_no_budget() {
    let wallet = TestWallet::open(|_| {});
    let response = settled(&wallet);
    let before = wallet.handle.wallet().unwrap();
    assert!(for_call(&config_on(&wallet.workspace, None), Some(response), 0).is_none());
    assert!(for_call(&config_on(&wallet.workspace, None), None, 0).is_none());
    assert_eq!(wallet.handle.wallet().unwrap(), before);
}
