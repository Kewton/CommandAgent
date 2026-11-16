//! Issue #1015: the wallet's end-of-run record, by the wallet's own reducers.

use std::sync::{Arc, Barrier, mpsc};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::super::identity::{PhysicalId, SendPurpose};
use super::super::ledger_v2::{
    AttemptRecordV2, BudgetCharge, InputBoundMethod, ProviderBilled, UsageCounts, WalletStateV2,
};
use super::super::policy_v2::test_support::{decode_v2, policy_v2_value};
use super::super::reservation::{AttemptState, Money};
use super::super::responses::AccountingBasis;
use super::super::test_support::TestWallet;
use super::super::usage_tariff::charge;
use super::{END_OF_RUN_WAIT, EVENT, RunBudgetSummary, SummaryRecord, event, of_v2, snapshot};

fn usage(input: u64, cached: u64, written: (u64, u64), output: u64) -> UsageCounts {
    UsageCounts {
        input_tokens: input,
        output_tokens: output,
        cached_tokens: cached,
        reasoning_tokens: None,
        total_tokens: Some(input + output),
        cache_write_tokens: Some(written.0 + written.1),
        cache_write_30m_tokens: Some(written.0),
        cache_write_12h_tokens: Some(written.1),
    }
}

fn settled_v2(physical: u64, usage: UsageCounts, charged: Money) -> AttemptRecordV2 {
    AttemptRecordV2 {
        physical: PhysicalId(physical),
        purpose: SendPurpose::Executor,
        final_repair: None,
        request_sha256: "0".repeat(64),
        input_bound_method: InputBoundMethod::BodyBytesPlusOverhead,
        input_bound: usage.input_tokens,
        output_bound: usage.output_tokens,
        reserved: charged,
        state: AttemptState::Settled,
        unknown: None,
        observed: None,
        charged: Some(BudgetCharge {
            usage,
            tier: 0,
            charged,
            late: false,
        }),
        provider_billed: ProviderBilled::Unreported,
    }
}

/// Acceptance 2: the v2 total is the declared tariff applied to synthetic usage, summed with no
/// float, and it equals what the wallet's own reducer says it spent.
#[test]
fn the_v2_tariff_total_matches_a_hand_computed_sum_and_the_wallet_reducer() {
    let policy = decode_v2(&policy_v2_value());
    let mut wallet = WalletStateV2::new(&policy, "w".repeat(64), None);
    // Tier one at 2 / 0.2 / 2.5 / 4 / 8 per million dollars; tier two (above 272000 input
    // tokens) at 4 for input and 12 for output.
    //   100 in, 20 cached, 10 + 5 written, 30 out: 65*2 + 20*0.2 + 10*2.5 + 5*4 + 30*8 = 419
    //   1000 in, 7 out: 1000*2 + 7*8 = 2056
    //   300000 in, 1 out, in tier two: 300000*4 + 1*12 = 1200012
    // In all 1202487 per million dollars.
    for (physical, counts) in [
        usage(100, 20, (10, 5), 30),
        usage(1000, 0, (0, 0), 7),
        usage(300_000, 0, (0, 0), 1),
    ]
    .into_iter()
    .enumerate()
    {
        let charged = charge(&policy.tariff, &counts.usage()).unwrap().charge;
        wallet
            .attempts
            .push(settled_v2(physical as u64 + 1, counts, charged));
    }
    let limit = policy.limits.cost;
    let observed = of_v2(&wallet, limit).unwrap();
    assert_eq!(
        observed.declared_tariff_charge_usd_total.as_deref(),
        Some("1.202487")
    );
    assert_eq!(observed.spent_usd, "1.202487");
    assert_eq!(observed.spent_usd, wallet.spent().unwrap().decimal_text());
    assert_eq!(observed.held_usd, "0");
    assert_eq!(observed.limit_usd, "1000");
    assert_eq!(observed.sends.settled, 3);
    assert!(observed.unknown.is_empty());
    assert_eq!(observed.latch, None);
}

fn observed_json(summary: &RunBudgetSummary) -> Value {
    serde_json::to_value(summary).unwrap()
}

/// One end-of-run deadline from now.
fn soon() -> Instant {
    Instant::now() + END_OF_RUN_WAIT
}

#[test]
fn a_v1_wallet_reports_its_reducers_and_no_tariff_total() {
    let wallet = TestWallet::open(|_| {});
    wallet.send(50, None).unwrap();
    wallet.send(50, None).unwrap();
    let summary = snapshot(&wallet.handle, soon());
    assert_eq!(summary.record, SummaryRecord::Observed);
    assert_eq!(
        summary.accounting_basis,
        Some(AccountingBasis::ProviderBilled)
    );
    let state = wallet.handle.wallet().unwrap();
    let observed = summary.observed.clone().unwrap();
    assert_eq!(observed.spent_usd, state.spent().unwrap().decimal_text());
    assert_eq!(observed.held_usd, "0");
    assert_eq!(observed.limit_usd, "1000");
    assert_eq!(observed.budget_id, state.budget_id);
    assert_eq!(observed.policy_sha256, state.policy_sha256);
    assert_eq!(observed.sends.settled, 2);
    assert_eq!(summary.stop, None);
    let value = observed_json(&summary);
    assert_eq!(value["record"], "observed");
    assert_eq!(value["accounting_basis"], "provider_billed");
    assert!(value.get("declared_tariff_charge_usd_total").is_none());
    assert!(value["latch"].is_null());
    assert!(value["stop"].is_null());
    // The event carries the same record and reads back as it.
    let emitted = event(&summary);
    assert_eq!(emitted["event"], EVENT);
    let back: RunBudgetSummary = serde_json::from_value(emitted).unwrap();
    assert_eq!(back, summary);
}

#[test]
fn an_unknown_send_is_reported_with_its_cause_and_the_latch() {
    let wallet = TestWallet::open(|_| {});
    let _ = wallet.send_reporting(
        50,
        None,
        r#"{"usage":{"prompt_tokens":10,"completion_tokens":5}}"#,
    );
    let summary = snapshot(&wallet.handle, soon());
    let value = observed_json(&summary);
    assert_eq!(value["sends"]["unknown"], 1);
    assert_eq!(
        value["unknown"],
        json!([{"physical": 1, "cause": "cost_missing"}])
    );
    assert_eq!(value["latch"]["cause"], "unknown_billing");
    // An unknown send holds its reservation: it is held, never spent and never zero.
    let state = wallet.handle.wallet().unwrap();
    assert_eq!(value["held_usd"], state.held().unwrap().decimal_text());
    assert_ne!(value["held_usd"], "0");
}

#[test]
fn a_cap_refusal_is_the_stop_and_nothing_else_is() {
    let wallet = TestWallet::open(|value| value["limits"]["cost_usd"] = json!("0.001"));
    let before = snapshot(&wallet.handle, soon());
    assert_eq!(before.stop, None);
    let mut refused = None;
    for _ in 0..50 {
        if let Err(refusal) = wallet.send(50, None) {
            refused = Some(refusal);
            break;
        }
    }
    assert!(refused.is_some(), "the cap never refused a send");
    let value = observed_json(&snapshot(&wallet.handle, soon()));
    assert_eq!(value["stop"]["kind"], "limit");
    assert_eq!(value["stop"]["limit"], "cost_usd");
    assert_eq!(value["stop"]["refusal"], "Wallet(Budget(CostExceeded))");
    assert_eq!(value["limit_usd"], "0.001");
}

/// The record is unavailable, with no amount and no zero, when the lock is not free in time.
#[test]
fn a_held_lock_gives_an_unavailable_record_with_no_amount() {
    let wallet = TestWallet::open(|_| {});
    wallet.send(50, None).unwrap();
    let handle = Arc::clone(&wallet.handle);
    let held = Arc::new(Barrier::new(2));
    let (release, released) = mpsc::channel::<()>();
    let holder = {
        let held = Arc::clone(&held);
        std::thread::spawn(move || {
            handle.with_ledger_locked(|| {
                held.wait();
                released.recv().unwrap();
            });
        })
    };
    held.wait();
    let summary = snapshot(&wallet.handle, Instant::now() + Duration::from_millis(50));
    release.send(()).unwrap();
    holder.join().unwrap();
    assert_eq!(summary.record, SummaryRecord::Unavailable);
    assert_eq!(summary.observed, None);
    let value = observed_json(&summary);
    assert_eq!(
        value,
        json!({"record": "unavailable", "accounting_basis": "provider_billed", "stop": null})
    );
    // Taken again with the lock free, the same wallet is observed.
    assert_eq!(
        snapshot(&wallet.handle, soon()).record,
        SummaryRecord::Observed
    );
}

/// A file that is no longer what the handle wrote is not vouched for, and the snapshot keeps no
/// refusal of its own: it changes nothing of the wallet.
#[test]
fn a_changed_ledger_file_gives_an_unavailable_record() {
    let wallet = TestWallet::open(|_| {});
    wallet.send(50, None).unwrap();
    let file = wallet
        .workspace
        .parent()
        .unwrap()
        .join("ledgers/wallet-alpha/ledger.json");
    let original = std::fs::read(&file).unwrap();
    std::fs::write(&file, b"{}").unwrap();
    assert_eq!(
        snapshot(&wallet.handle, soon()).record,
        SummaryRecord::Unavailable
    );
    std::fs::write(&file, &original).unwrap();
    assert_eq!(
        snapshot(&wallet.handle, soon()).record,
        SummaryRecord::Observed
    );
    assert!(wallet.handle.success_gate().is_ok());
}

/// A v2 wallet through its handle: one send settled by the tariff, the total under its own name.
#[test]
fn a_v2_handle_reports_the_tariff_charge_of_its_settled_send() {
    use super::super::ledger::{CommandWrites, LedgerEntry};
    use super::super::ledger_v2::LedgerOpenV2;
    use super::super::policy_v2::RunBudgetPolicyV2;
    use super::super::policy_v2::test_support::WALLET;
    use super::super::test_support::FakeClock;
    use super::super::transport::{BudgetedBody, RunBudgetHandle, SandboxView, settle_body};
    use crate::tools::os_sandbox::SandboxPolicy;

    const ENDPOINT: &str = "https://api.example.test/v1/responses";
    let root = tempfile::tempdir().unwrap();
    let base = std::fs::canonicalize(root.path()).unwrap();
    let workspace = base.join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    std::fs::create_dir_all(base.join("run-tmp")).unwrap();
    let mut value = policy_v2_value();
    value["ledger_dir"] = json!(base.join("ledgers").to_str().unwrap());
    value["route"]["endpoint"] = json!(ENDPOINT);
    let policy = RunBudgetPolicyV2::decode(value.to_string().as_bytes(), WALLET).unwrap();
    let handle = Arc::new(
        RunBudgetHandle::open_responses(LedgerOpenV2 {
            policy: Arc::new(policy),
            workspace: &workspace,
            explicit_profile: None,
            entry: LedgerEntry::Fresh,
            clock: FakeClock::at(7_000),
            command_writes: Some(CommandWrites::of(SandboxPolicy::with_home(
                &workspace,
                &base.join("run-tmp"),
                base.join("home"),
            ))),
        })
        .unwrap()
        .with_sandbox_view(SandboxView {
            applied_on: Some(workspace.clone()),
        }),
    );
    let body = json!({
        "model": "m-1",
        "input": [{"role": "user", "content": [{"type": "input_text", "text": "go"}]}],
        "max_output_tokens": 50,
        "reasoning": {"effort": "high"},
        "store": false,
        "include": ["reasoning.encrypted_content"],
        "service_tier": "default"
    });
    let budgeted =
        BudgetedBody::for_responses(&handle, &body, SendPurpose::Executor, ENDPOINT).unwrap();
    let (_request, send) = budgeted
        .begin(reqwest::Client::new().post(ENDPOINT))
        .unwrap();
    let reply = json!({
        "id": "resp-1015",
        "model": "m-1-snapshot-a",
        "service_tier": "default",
        "output": [],
        "usage": {
            "input_tokens": 100,
            "input_tokens_details": {"cached_tokens": 20, "cache_write_tokens": 15,
                "cache_write_tokens_details": {"ttl_30m_tokens": 10, "ttl_12h_tokens": 5}},
            "output_tokens": 30,
            "output_tokens_details": {"reasoning_tokens": 10},
            "total_tokens": 130
        }
    });
    settle_body(&mut Some(send), &reply.to_string()).unwrap();
    let summary = snapshot(&handle, soon());
    let value = observed_json(&summary);
    assert_eq!(value["record"], "observed");
    assert_eq!(value["accounting_basis"], "usage_tariff");
    // 65*2 + 20*0.2 + 10*2.5 + 5*4 + 30*8 = 419 per million dollars.
    assert_eq!(value["declared_tariff_charge_usd_total"], "0.000419");
    assert_eq!(value["spent_usd"], "0.000419");
    assert_eq!(
        value["spent_usd"],
        handle
            .responses_wallet()
            .unwrap()
            .spent()
            .unwrap()
            .decimal_text()
    );
    assert_eq!(value["sends"]["settled"], 1);
}

#[test]
fn an_unavailable_event_reads_back_without_amounts() {
    let value = json!({
        "event": EVENT,
        "record": "unavailable",
        "accounting_basis": "usage_tariff",
        "schema_version": "1",
    });
    let back: RunBudgetSummary = serde_json::from_value(value).unwrap();
    assert_eq!(back.record, SummaryRecord::Unavailable);
    assert_eq!(back.observed, None);
}

/// N3: the typed cap refusal is the handle's, behind no ledger lock, so an `unavailable` record
/// still says the wallet refused a send, and nothing else.
#[test]
fn an_unavailable_record_keeps_the_typed_refusal() {
    let wallet = TestWallet::open(|value| value["limits"]["cost_usd"] = json!("0.001"));
    while wallet.send(50, None).is_ok() {}
    let handle = Arc::clone(&wallet.handle);
    let held = Arc::new(Barrier::new(2));
    let (release, released) = mpsc::channel::<()>();
    let holder = {
        let held = Arc::clone(&held);
        std::thread::spawn(move || {
            handle.with_ledger_locked(|| {
                held.wait();
                released.recv().unwrap();
            });
        })
    };
    held.wait();
    let summary = snapshot(&wallet.handle, Instant::now() + Duration::from_millis(50));
    release.send(()).unwrap();
    holder.join().unwrap();
    assert_eq!(
        observed_json(&summary),
        json!({
            "record": "unavailable",
            "accounting_basis": "provider_billed",
            "stop": {"kind": "limit", "limit": "cost_usd", "refusal": "Wallet(Budget(CostExceeded))"},
        })
    );
}

/// M2.3, N4: a record is read back by what it says and what decodes. `observed` with a body that
/// no longer decodes is `unavailable`; `unavailable` never carries amounts; an event that does not
/// decode at all is `unavailable` with nothing else.
#[test]
fn a_record_is_read_back_as_unavailable_when_its_body_does_not_decode() {
    let wallet = TestWallet::open(|_| {});
    wallet.send(50, None).unwrap();
    let recorded = event(&snapshot(&wallet.handle, soon()));
    assert_eq!(
        RunBudgetSummary::read_back(&recorded).record,
        SummaryRecord::Observed
    );

    // What the scrub leaves of a key or a word that was registered as a secret.
    let mut scrubbed = recorded.clone();
    let sends = scrubbed["sends"].as_object_mut().unwrap();
    let count = sends.remove("reserved").unwrap();
    sends.insert("<redacted>".to_string(), count);
    let back = RunBudgetSummary::read_back(&scrubbed);
    assert_eq!(back.record, SummaryRecord::Unavailable);
    assert_eq!(back.observed, None);
    assert_eq!(back.accounting_basis, Some(AccountingBasis::ProviderBilled));

    let mut unavailable_with_amounts = recorded.clone();
    unavailable_with_amounts["record"] = json!("unavailable");
    let back = RunBudgetSummary::read_back(&unavailable_with_amounts);
    assert_eq!(back.observed, None);
    assert!(!observed_json(&back).to_string().contains("spent_usd"));

    let mut no_record = recorded;
    no_record["record"] = json!("<redacted>");
    let back = RunBudgetSummary::read_back(&no_record);
    assert_eq!(
        observed_json(&back),
        json!({"record": "unavailable", "stop": null})
    );
}
