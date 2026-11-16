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
        json!({"record": "unavailable", "accounting_basis": "usage_tariff", "stop": null})
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

// --- M1: only the current run counts --------------------------------------------------------

/// One events file holding the fixtures' runs in this order, as a reused
/// `COMMANDAGENT_EVAL_EVENTS` holds them.
fn stream(lines: &[Value]) -> (tempfile::TempDir, PathBuf, Vec<Value>) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("run/events.jsonl");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let body = lines.iter().map(Value::to_string).collect::<Vec<_>>();
    std::fs::write(&path, format!("{}\n", body.join("\n"))).unwrap();
    (dir, path, lines.to_vec())
}

fn runs(cases: &[&str]) -> Vec<Value> {
    cases.iter().flat_map(|case| events(case)).collect()
}

fn summary_of_stream(path: &Path) -> Value {
    serde_json::from_str(&render(&Source::from_events_path(path))).unwrap()
}

#[test]
fn a_run_without_a_declaration_after_one_with_both_shows_neither() {
    let (_dir, path, lines) = stream(&runs(&["wallet-v2", "stages-unsealed", "no-declaration"]));
    let value = summary_of_stream(&path);
    assert!(value.get("run_budget").is_none(), "{value}");
    assert!(value.get("structured_stages").is_none(), "{value}");
    assert_eq!(super::markdown_sections(&lines), None);
    let markdown = crate::time_profile::render_summary_sections(&lines).unwrap();
    assert!(!markdown.contains("Run budget"), "{markdown}");
    assert!(!markdown.contains("Structured stages"), "{markdown}");
}

#[test]
fn a_run_after_another_declared_one_shows_only_its_own_values() {
    let (_dir, path, lines) = stream(&runs(&["wallet-v1", "stages-unsealed", "wallet-v2"]));
    let value = summary_of_stream(&path);
    assert_eq!(value["run_budget"], summary("wallet-v2")["run_budget"]);
    assert!(value.get("structured_stages").is_none(), "{value}");
    let markdown = super::markdown_sections(&lines).unwrap();
    assert!(markdown.contains("usage_tariff"), "{markdown}");
    assert!(!markdown.contains("provider_billed"), "{markdown}");

    let (_dir, path, _) = stream(&runs(&["stages-unsealed", "stages-sealed"]));
    let value = summary_of_stream(&path);
    assert_eq!(
        value["structured_stages"],
        summary("stages-sealed")["structured_stages"]
    );
}

/// The failed checks of one run never attach to another's, even under the same run nonce.
#[test]
fn failed_checks_of_an_earlier_run_never_attach_to_this_one() {
    let earlier = events("stages-unsealed");
    let same_record = earlier
        .iter()
        .find(|event| event["event"] == "structured_stages_summary")
        .unwrap()
        .clone();
    let mut lines = earlier.clone();
    lines.push(json!({"event": "run_start", "workspace_root": "/tmp/cm0/again"}));
    lines.push(same_record);
    let (_dir, path, lines) = stream(&lines);
    let stages = summary_of_stream(&path)["structured_stages"].clone();
    assert_eq!(stages["record"], "observed");
    for stage in stages["stages"].as_array().unwrap() {
        assert!(stage.get("failed_checks").is_none(), "{stage}");
    }
    let markdown = super::markdown_sections(&lines).unwrap();
    assert!(!markdown.contains("attempt 2"), "{markdown}");
}

/// The same through the real hook: a run with a wallet, then one without, on one events path.
#[test]
fn the_real_hook_does_not_carry_a_wallet_into_the_next_run_on_the_same_path() {
    let wallet = crate::structured_budget::test_support::TestWallet::open(|_| {});
    wallet.send(50, None).unwrap();
    let root = tempfile::tempdir().unwrap();
    let mut with_wallet = plain_config(root.path());
    with_wallet.field_sources.run_budget = Some(wallet.handle.clone());
    crate::emit_run_start(&with_wallet);
    super::emit_end_of_run(&with_wallet);
    let path = root.path().join("run/events.jsonl");
    assert_eq!(summary_of_stream(&path)["run_budget"]["record"], "observed");

    let without = plain_config(root.path());
    crate::emit_run_start(&without);
    super::emit_end_of_run(&without);
    let value = summary_of_stream(&path);
    assert!(value.get("run_budget").is_none(), "{value}");
}

// --- M2: every emitted word is reserved, and a scrubbed record is not `observed` -------------

use crate::planner::structured_stages::receipt::{CheckResult, CheckStatus};
use crate::planner::structured_stages::state::{AttemptTicket, CheckFailureKind, Phase};
use crate::planner::structured_stages::summary::{
    ArtifactSummary, AttemptsSummary, FailedChecks, ObservedStages, StageEntry, StagesSummary,
    failed_checks_event,
};
use crate::providers::usage_observation::UsageStop;
use crate::structured_budget::ledger_v2::{ResponseStop, UnknownCause};
use crate::structured_budget::reservation::LatchReason;
use crate::structured_budget::responses::AccountingBasis;
use crate::structured_budget::settlement::UnknownReason;
use crate::structured_budget::summary::{
    LatchSummary, ObservedBudget, RunBudgetSummary, SendCounts, StopSummary, SummaryRecord,
    UnknownSend,
};
use crate::structured_budget::usage_tariff::ChargeStop;

/// Each list names every variant, and each `match` has no wildcard: a variant added later does
/// not compile until it is listed here, and then its word is checked below.
fn latch_reasons() -> Vec<LatchReason> {
    use LatchReason::*;
    let all = vec![
        UnknownBilling,
        OverReservation,
        InputOverBound,
        OutputOverBound,
        InterruptedAfterStart,
        ResponseAfterDeadline,
        ResponseAfterFinalRepairWindow,
    ];
    for reason in &all {
        match reason {
            UnknownBilling
            | OverReservation
            | InputOverBound
            | OutputOverBound
            | InterruptedAfterStart
            | ResponseAfterDeadline
            | ResponseAfterFinalRepairWindow => {}
        }
    }
    all
}

fn unknown_reasons() -> Vec<UnknownReason> {
    use UnknownReason::*;
    let all = vec![
        CostMissing,
        UsageMissing,
        UsageInconsistent,
        BodyTruncated,
        ParseFailed,
        HttpError,
        Timeout,
        Cancelled,
        SendError,
        InterruptedAfterStart,
    ];
    for reason in &all {
        match reason {
            CostMissing
            | UsageMissing
            | UsageInconsistent
            | BodyTruncated
            | ParseFailed
            | HttpError
            | Timeout
            | Cancelled
            | SendError
            | InterruptedAfterStart => {}
        }
    }
    all
}

fn usage_stops() -> Vec<UsageStop> {
    use UsageStop::*;
    let all = vec![
        ResponseMalformed,
        UsageMissing,
        UsageNotObject,
        DuplicateKey,
        UnknownField,
        InvalidValue,
        RequiredFieldMissing,
        CacheWriteSplitMismatch,
        CachedAndWrittenOverInput,
        ReasoningOverOutput,
        TotalMismatch,
    ];
    for stop in &all {
        match stop {
            ResponseMalformed
            | UsageMissing
            | UsageNotObject
            | DuplicateKey
            | UnknownField
            | InvalidValue
            | RequiredFieldMissing
            | CacheWriteSplitMismatch
            | CachedAndWrittenOverInput
            | ReasoningOverOutput
            | TotalMismatch => {}
        }
    }
    all
}

fn response_stops() -> Vec<ResponseStop> {
    use ResponseStop::*;
    let all = vec![
        DuplicateKey,
        ModelMissing,
        ModelNotDeclared,
        ServiceTierMissing,
        ServiceTierNotDefault,
    ];
    for stop in &all {
        match stop {
            DuplicateKey
            | ModelMissing
            | ModelNotDeclared
            | ServiceTierMissing
            | ServiceTierNotDefault => {}
        }
    }
    all
}

fn charge_stops() -> Vec<ChargeStop> {
    use ChargeStop::*;
    let all = vec![
        CacheWriteFieldMissing,
        CacheWriteInNoneMode,
        UsageInconsistent,
        InputZero,
        ZeroCharge,
        Overflow,
        TariffInconsistent,
    ];
    for stop in &all {
        match stop {
            CacheWriteFieldMissing
            | CacheWriteInNoneMode
            | UsageInconsistent
            | InputZero
            | ZeroCharge
            | Overflow
            | TariffInconsistent => {}
        }
    }
    all
}

fn unknown_causes() -> Vec<UnknownCause> {
    let mut all = Vec::new();
    all.extend(
        unknown_reasons()
            .into_iter()
            .map(|reason| UnknownCause::Transport { reason }),
    );
    all.extend(
        usage_stops()
            .into_iter()
            .map(|stop| UnknownCause::Usage { stop }),
    );
    all.extend(
        response_stops()
            .into_iter()
            .map(|stop| UnknownCause::Response { stop }),
    );
    all.extend(
        charge_stops()
            .into_iter()
            .map(|stop| UnknownCause::Charge { stop }),
    );
    for cause in &all {
        match cause {
            UnknownCause::Transport { .. }
            | UnknownCause::Usage { .. }
            | UnknownCause::Response { .. }
            | UnknownCause::Charge { .. } => {}
        }
    }
    all
}

fn phases() -> Vec<Phase> {
    use Phase::*;
    let all = vec![NotReady, Proposed, LocallyValid, Sealed, Unverified];
    for phase in &all {
        match phase {
            NotReady | Proposed | LocallyValid | Sealed | Unverified => {}
        }
    }
    all
}

fn check_statuses() -> Vec<CheckStatus> {
    use CheckStatus::*;
    let all = vec![Passed, Failed, NotRun, Unknown];
    for status in &all {
        match status {
            Passed | Failed | NotRun | Unknown => {}
        }
    }
    all
}

fn check_codes() -> Vec<&'static str> {
    use CheckFailureKind::*;
    [Failed, NotRun, Unknown]
        .into_iter()
        .map(|kind| match kind {
            Failed | NotRun | Unknown => {
                crate::tools::profile_tools::effects::check_failure_code(kind)
            }
        })
        .collect()
}

/// Every record the three events and the two summary objects can hold, with every enum word,
/// every optional part present, and each operator-chosen or dynamic value (IDs, digests, paths,
/// amounts, stage names, the refusal's Debug text) a placeholder shorter than any secret.
fn every_emitted_shape() -> Vec<Value> {
    const SHORT: &str = "x";
    let mut shapes = Vec::new();
    let budget = |accounting_basis, record, cause: Value, latch, stop| RunBudgetSummary {
        record,
        accounting_basis: Some(accounting_basis),
        stop: Some(StopSummary {
            kind: stop,
            limit: Some(SHORT.to_string()),
            refusal: SHORT.to_string(),
        }),
        observed: Some(ObservedBudget {
            budget_id: SHORT.to_string(),
            policy_sha256: SHORT.to_string(),
            workspace_sha256: SHORT.to_string(),
            limit_usd: "1".to_string(),
            spent_usd: "1".to_string(),
            held_usd: "1".to_string(),
            declared_tariff_charge_usd_total: Some("1".to_string()),
            sends: SendCounts::default(),
            unknown: vec![UnknownSend { physical: 1, cause }],
            latch: Some(LatchSummary {
                cause: latch,
                physical: 1,
            }),
        }),
    };
    let latches = latch_reasons();
    let causes = unknown_reasons()
        .into_iter()
        .map(|reason| serde_json::to_value(reason).unwrap())
        .chain(
            unknown_causes()
                .into_iter()
                .map(|cause| serde_json::to_value(cause).unwrap()),
        )
        .collect::<Vec<_>>();
    let stops = crate::structured_budget::summary::limit_names();
    let longest = causes.len().max(latches.len()).max(stops.len());
    for index in 0..longest {
        for basis in [
            AccountingBasis::ProviderBilled,
            AccountingBasis::UsageTariff,
        ] {
            for record in [SummaryRecord::Observed, SummaryRecord::Unavailable] {
                let summary = budget(
                    basis,
                    record,
                    causes[index % causes.len()].clone(),
                    latches[index % latches.len()],
                    stops[index % stops.len()].to_string(),
                );
                shapes.push(crate::structured_budget::summary::event(&summary));
            }
        }
    }
    for limit in &stops {
        shapes.push(json!({ "limit": limit }));
    }
    let checks = check_statuses()
        .into_iter()
        .map(
            |status| crate::planner::structured_stages::summary::CheckEntry {
                id: SHORT.to_string(),
                status,
            },
        )
        .collect::<Vec<_>>();
    for (phase, code) in phases().into_iter().zip(check_codes().into_iter().cycle()) {
        let stages = StagesSummary {
            record: SummaryRecord::Observed,
            observed: Some(ObservedStages {
                complete: false,
                ledger_intact: true,
                contract_sha256: SHORT.to_string(),
                run_nonce: SHORT.to_string(),
                stages: vec![StageEntry {
                    stage: SHORT.to_string(),
                    target: SHORT.to_string(),
                    phase,
                    sealed: false,
                    attempts: AttemptsSummary { used: 1, limit: 1 },
                    artifact: ArtifactSummary {
                        path: SHORT.to_string(),
                        committed_sha256: Some(SHORT.to_string()),
                    },
                    failed_checks: Some(FailedChecks {
                        attempt: 1,
                        code: code.to_string(),
                        checks: checks.clone(),
                        missing: vec![SHORT.to_string()],
                        unrequested: 1,
                    }),
                }],
            }),
        };
        shapes.push(crate::planner::structured_stages::summary::event(&stages));
    }
    let ticket = AttemptTicket {
        run_nonce: SHORT.to_string(),
        stage: SHORT.to_string(),
        target: SHORT.to_string(),
        attempt: 1,
        generation: 0,
        turn: 1,
    };
    let required = [SHORT.to_string()].into_iter().collect();
    for code in check_codes() {
        let results = check_statuses()
            .into_iter()
            .map(|status| CheckResult {
                id: SHORT.to_string(),
                status,
            })
            .collect::<Vec<_>>();
        shapes.push(failed_checks_event(&ticket, &required, &results, code));
    }
    shapes
}

fn words(value: &Value, out: &mut std::collections::BTreeSet<String>) {
    match value {
        Value::String(word) => {
            out.insert(word.clone());
        }
        Value::Array(items) => items.iter().for_each(|item| words(item, out)),
        Value::Object(object) => {
            for (key, item) in object {
                out.insert(key.clone());
                words(item, out);
            }
        }
        _ => {}
    }
}

/// M2: every key and fixed word the three events and the two summary objects can carry, long
/// enough to be registered as a secret, is one no secret can be (a fixed identifier or a part of
/// one), so the scrub can never rewrite it.
#[test]
fn every_emitted_word_long_enough_to_be_a_secret_is_reserved() {
    let mut found = std::collections::BTreeSet::new();
    for shape in every_emitted_shape() {
        words(&shape, &mut found);
    }
    let missing = found
        .iter()
        .filter(|word| word.chars().count() >= crate::sensitive_data::MIN_GENERIC_SECRET_LEN)
        .filter(|word| !crate::sensitive_data::collides_with_fixed_identifier(word))
        .cloned()
        .collect::<Vec<_>>();
    assert!(missing.is_empty(), "not reserved: {missing:?}");
    // The set is the real one: a sample of the words the reproduction found unreserved.
    for word in [
        "reserved",
        "physical",
        "attempts",
        "unknown_billing",
        "not_ready",
        "locally_valid",
        "unverified",
        "cost_missing",
        "transport",
        "physical_sends",
        "check_not_run",
    ] {
        assert!(found.contains(word), "{word}");
    }
}

/// M2.3: an event whose words the scrub rewrote is not `observed` in the summary, and the JSON
/// and `summary.md` say the same.
#[test]
fn a_scrubbed_record_is_unavailable_in_the_json_and_the_markdown_alike() {
    // What the scrub leaves of a key that was registered as a secret, as the reproduction saw it
    // for `reserved` before it was reserved. Any word the list could miss would read the same.
    let mut lines = events("wallet-v2");
    for line in &mut lines {
        if line["event"] == "run_budget_summary" {
            let sends = line["sends"].as_object_mut().unwrap();
            let count = sends.remove("reserved").unwrap();
            sends.insert("<redacted>".to_string(), count);
        }
    }
    let (_dir, path, lines) = stream(&lines);
    let value = summary_of_stream(&path);
    assert_eq!(
        value["run_budget"],
        json!({"record": "unavailable", "accounting_basis": "usage_tariff", "stop": null})
    );
    let markdown = super::markdown_sections(&lines).unwrap();
    assert_eq!(
        markdown,
        "Run budget (usage_tariff): record unavailable, no amount recorded"
    );

    let mut lines = events("stages-unsealed");
    for line in &mut lines {
        if line["event"] == "structured_stages_summary" {
            line["stages"][1]["phase"] = json!("<redacted>");
        }
    }
    let (_dir, path, lines) = stream(&lines);
    assert_eq!(
        summary_of_stream(&path)["structured_stages"],
        json!({"record": "unavailable"})
    );
    assert_eq!(
        super::markdown_sections(&lines).as_deref(),
        Some("Structured stages: record unavailable")
    );
}
