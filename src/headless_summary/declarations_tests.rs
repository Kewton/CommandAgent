//! Issue #1015: the summary's `run_budget` and `structured_stages`, read from the run's events.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::super::{Source, render};
use crate::config::Config;

const NEW_CASES: [&str; 9] = [
    "wallet-v1",
    "wallet-v2",
    "wallet-v2-unknown",
    "wallet-v2-cap",
    "wallet-unavailable",
    "stages-sealed",
    "stages-unsealed",
    "stages-attempt-limit",
    "no-declaration",
];

fn fixture_dir(case: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/corpus/apps/cm0-headless-summary/fixtures")
        .join(case)
}

fn events(case: &str) -> Vec<Value> {
    std::fs::read_to_string(fixture_dir(case).join("events.jsonl"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn summary(case: &str) -> Value {
    serde_json::from_str(&render(&Source::from_events_path(
        fixture_dir(case).join("events.jsonl"),
    )))
    .unwrap()
}

#[test]
fn a_v1_wallet_is_projected_as_recorded_without_a_tariff_total() {
    let value = summary("wallet-v1");
    assert_eq!(
        value["run_budget"],
        json!({
            "record": "observed",
            "accounting_basis": "provider_billed",
            "budget_id": "wallet-example",
            "policy_sha256": "a".repeat(64),
            "workspace_sha256": "b".repeat(64),
            "limit_usd": "5",
            "spent_usd": "0.0009",
            "held_usd": "0",
            "sends": {"settled": 2, "reserved": 0, "started": 0, "unknown": 0},
            "unknown": [],
            "latch": null,
            "stop": null,
        })
    );
    assert!(value.get("structured_stages").is_none());
    // #1003's billed fields stay as they were, apart from the wallet.
    assert_eq!(value["provider_billed_cost_usd_total"], "0.0009");
}

#[test]
fn a_v2_wallet_shows_its_tariff_total_under_its_own_name() {
    let value = summary("wallet-v2");
    let budget = &value["run_budget"];
    assert_eq!(budget["accounting_basis"], "usage_tariff");
    assert_eq!(budget["limit_usd"], "5");
    assert_eq!(budget["spent_usd"], "0.002475");
    assert_eq!(budget["held_usd"], "0");
    assert_eq!(budget["declared_tariff_charge_usd_total"], "0.002475");
    // A Responses run reports no billed amount: the billed fields stay unreported.
    assert!(value["provider_billed_cost_usd_total"].is_null());
    assert_eq!(value["provider_billed_cost_complete"], false);
    assert_eq!(value["provider_billed_cost_calls"], 0);
}

#[test]
fn an_unknown_send_and_the_latch_are_projected_with_their_words() {
    let budget = summary("wallet-v2-unknown")["run_budget"].clone();
    assert_eq!(budget["sends"]["unknown"], 1);
    assert_eq!(
        budget["unknown"],
        json!([{"physical": 2, "cause": {"kind": "transport", "reason": "timeout"}}])
    );
    assert_eq!(
        budget["latch"],
        json!({"cause": "unknown_billing", "physical": 2})
    );
    assert_eq!(budget["held_usd"], "0.0412");
    assert!(budget["stop"].is_null());
}

#[test]
fn a_cap_refusal_is_projected_as_the_stop() {
    let budget = summary("wallet-v2-cap")["run_budget"].clone();
    assert_eq!(
        budget["stop"],
        json!({"kind": "limit", "limit": "cost_usd", "refusal": "Wallet(Budget(CostExceeded))"})
    );
}

#[test]
fn an_unavailable_wallet_record_has_no_amount_and_no_zero() {
    let value = summary("wallet-unavailable");
    assert_eq!(
        value["run_budget"],
        json!({"record": "unavailable", "accounting_basis": "usage_tariff"})
    );
}

#[test]
fn sealed_stages_are_projected_as_recorded_and_assurance_stays_static() {
    let value = summary("stages-sealed");
    let stages = &value["structured_stages"];
    assert_eq!(stages["record"], "observed");
    assert_eq!(stages["complete"], true);
    assert_eq!(stages["ledger_intact"], true);
    assert_eq!(stages["run_nonce"], "run-nonce-sealed");
    assert_eq!(stages["stages"][0]["phase"], "sealed");
    assert_eq!(stages["stages"][0]["sealed"], true);
    assert_eq!(stages["stages"][0]["artifact"]["path"], "out/alpha.json");
    assert!(stages["stages"][0].get("failed_checks").is_none());
    assert_eq!(value["assurance"], "static");
    assert!(value.get("run_budget").is_none());
}

#[test]
fn an_unsealed_target_carries_the_checks_of_its_latest_failed_attempt_in_this_run() {
    let stages = summary("stages-unsealed")["structured_stages"].clone();
    assert_eq!(stages["complete"], false);
    assert!(stages["stages"][0].get("failed_checks").is_none());
    assert_eq!(
        stages["stages"][1]["failed_checks"],
        json!({
            "attempt": 2,
            "code": "check_not_run",
            "checks": [{"id": "schema", "status": "passed"}],
            "missing": ["lint"],
            "unrequested": 1,
        })
    );
}

#[test]
fn a_target_out_of_attempts_is_projected_with_its_counts() {
    let stages = summary("stages-attempt-limit")["structured_stages"].clone();
    assert_eq!(
        stages["stages"][0]["attempts"],
        json!({"used": 2, "limit": 2})
    );
    assert_eq!(stages["stages"][0]["sealed"], false);
    assert_eq!(stages["stages"][0]["failed_checks"]["code"], "check_failed");
}

#[test]
fn a_run_without_a_declaration_has_neither_key() {
    let value = summary("no-declaration");
    assert!(value.get("run_budget").is_none());
    assert!(value.get("structured_stages").is_none());
}

/// Each new fixture's `summary.md` is exactly the time-profile section rendered from its events.
#[test]
fn summary_md_sections_match_the_fixtures() {
    for case in NEW_CASES {
        let expected = std::fs::read_to_string(fixture_dir(case).join("summary.md")).unwrap();
        let rendered = crate::time_profile::render_summary_sections(&events(case)).unwrap();
        assert_eq!(format!("{rendered}\n"), expected, "{case}");
    }
}

#[test]
fn sections_are_only_for_what_was_declared() {
    assert_eq!(super::markdown_sections(&events("no-declaration")), None);
    let unavailable = super::markdown_sections(&events("wallet-unavailable")).unwrap();
    assert!(!unavailable.contains('$'), "{unavailable}");
    assert!(!unavailable.contains(" 0"), "{unavailable}");
}

/// A declaration alone still gets its section on a run whose measured time is 0 ms.
#[test]
fn a_declaration_section_survives_a_zero_millisecond_run() {
    let events = vec![
        json!({"event": "run_budget_summary", "record": "unavailable", "accounting_basis": "provider_billed"}),
    ];
    assert_eq!(
        crate::time_profile::render_summary_sections(&events).as_deref(),
        Some("Run budget (provider_billed): record unavailable, no amount recorded")
    );
}

fn plain_config(root: &Path) -> Config {
    let workspace = root.to_string_lossy().into_owned();
    let state = root.join("state").to_string_lossy().into_owned();
    let parsed = crate::provider_cli::parse_from([
        "commandagent",
        "--cwd",
        &workspace,
        "--state-dir",
        &state,
    ])
    .unwrap();
    let mut config =
        Config::from_cli_with_provider_options(parsed.cli, parsed.provider_options).unwrap();
    config.eval_events_path = Some(root.join("run/events.jsonl"));
    config
}

/// Acceptance 4: a run that declared neither records none of the three events.
#[test]
fn a_run_without_a_declaration_emits_none_of_the_new_events() {
    let root = tempfile::tempdir().unwrap();
    let config = plain_config(root.path());
    assert!(config.field_sources.run_budget.is_none());
    super::emit_end_of_run(&config);
    let written = std::fs::read_to_string(root.path().join("run/events.jsonl")).unwrap_or_default();
    for name in [
        "run_budget_summary",
        "structured_stages_summary",
        "structured_stage_checks",
    ] {
        assert!(!written.contains(name), "{name}: {written}");
    }
}

/// A run with a wallet records it at its end, through the same function the stop calls.
#[test]
fn a_run_with_a_wallet_records_it_once() {
    let wallet = crate::structured_budget::test_support::TestWallet::open(|_| {});
    wallet.send(50, None).unwrap();
    let root = tempfile::tempdir().unwrap();
    let mut config = plain_config(root.path());
    config.field_sources.run_budget = Some(wallet.handle.clone());
    super::emit_end_of_run(&config);
    let written = std::fs::read_to_string(root.path().join("run/events.jsonl")).unwrap();
    assert_eq!(
        written.matches("\"event\":\"run_budget_summary\"").count(),
        1
    );
    let source = Source::from_events_path(root.path().join("run/events.jsonl"));
    let value: Value = serde_json::from_str(&render(&source)).unwrap();
    assert_eq!(value["run_budget"]["record"], "observed");
    assert_eq!(value["run_budget"]["sends"]["settled"], 1);
    assert_eq!(
        value["run_budget"]["spent_usd"],
        wallet
            .handle
            .wallet()
            .unwrap()
            .spent()
            .unwrap()
            .decimal_text()
    );
}
