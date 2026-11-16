//! Issue #1015: the stages' end-of-run record and the failed-checks event.

use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::super::receipt::{CheckResult, CheckStatus};
use super::super::state::AttemptTicket;
use super::{CHECKS_EVENT, EVENT, StagesSummary, event, failed_checks_event, snapshot};
use crate::config::Config;
use crate::structured_budget::summary::SummaryRecord;

fn ticket() -> AttemptTicket {
    AttemptTicket {
        run_nonce: "run-1015".to_string(),
        stage: "alpha".to_string(),
        target: "main".to_string(),
        attempt: 2,
        generation: 0,
        turn: 3,
    }
}

fn result(id: &str, status: CheckStatus) -> CheckResult {
    CheckResult {
        id: id.to_string(),
        status,
    }
}

#[test]
fn the_checks_event_names_only_required_checks() {
    let required = ["lint", "schema"]
        .into_iter()
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    let results = [
        result("lint", CheckStatus::Failed),
        result("operator-did-not-ask-this", CheckStatus::Passed),
    ];
    let value = failed_checks_event(&ticket(), &required, &results, "check_failed");
    assert_eq!(
        value,
        json!({
            "event": CHECKS_EVENT,
            "run_nonce": "run-1015",
            "stage": "alpha",
            "target": "main",
            "attempt": 2,
            "code": "check_failed",
            "checks": [{"id": "lint", "status": "failed"}],
            "missing": ["schema"],
            "unrequested": 1,
        })
    );
    assert!(!value.to_string().contains("operator-did-not-ask-this"));
}

fn plain_config(root: &std::path::Path) -> Config {
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
    Config::from_cli_with_provider_options(parsed.cli, parsed.provider_options).unwrap()
}

/// A run with no stage declaration has no record, and nothing is written for it.
#[test]
fn a_run_without_a_declaration_has_no_record() {
    let root = tempfile::tempdir().unwrap();
    let config = plain_config(root.path());
    assert_eq!(
        snapshot(&config, Instant::now() + Duration::from_millis(10)),
        None
    );
    assert!(!root.path().join("state").exists());
}

#[test]
fn an_unavailable_record_carries_nothing_but_its_word() {
    let summary = StagesSummary {
        record: SummaryRecord::Unavailable,
        observed: None,
    };
    let value = event(&summary);
    assert_eq!(value, json!({"event": EVENT, "record": "unavailable"}));
    let back: StagesSummary = serde_json::from_value(value).unwrap();
    assert_eq!(back, summary);
}

#[test]
fn an_observed_record_reads_back_with_no_failed_checks_key() {
    let value: Value = json!({
        "event": EVENT,
        "record": "observed",
        "complete": false,
        "ledger_intact": true,
        "contract_sha256": "c",
        "run_nonce": "n",
        "stages": [{
            "stage": "alpha", "target": "main", "phase": "proposed", "sealed": false,
            "attempts": {"used": 1, "limit": 8},
            "artifact": {"path": "out/alpha.json", "committed_sha256": null}
        }],
        "schema_version": "1"
    });
    let back: StagesSummary = serde_json::from_value(value).unwrap();
    let observed = back.observed.clone().unwrap();
    assert_eq!(observed.stages[0].failed_checks, None);
    let again = serde_json::to_value(&back).unwrap();
    assert!(again["stages"][0].get("failed_checks").is_none());

    // M2.3: the same record with a word the scrub rewrote reads back as unavailable.
    assert_eq!(
        StagesSummary::read_back(&again).record,
        SummaryRecord::Observed
    );
    let mut scrubbed = again.clone();
    scrubbed["stages"][0]["phase"] = json!("<redacted>");
    assert_eq!(
        StagesSummary::read_back(&scrubbed),
        StagesSummary {
            record: SummaryRecord::Unavailable,
            observed: None,
        }
    );
    let mut keyless = again;
    let attempts = keyless["stages"][0]
        .as_object_mut()
        .unwrap()
        .remove("attempts")
        .unwrap();
    keyless["stages"][0]["<redacted>"] = attempts;
    assert_eq!(
        StagesSummary::read_back(&keyless).record,
        SummaryRecord::Unavailable
    );
}

/// Claude-4 S1: a bound stage set whose ledger this process never opened (the run ended before
/// its first stage call and before any gate) read nothing, so its record is `unavailable`. Once a
/// judgment opened the ledger, the same run is observed.
#[test]
fn a_ledger_never_opened_is_recorded_as_unavailable() {
    use std::collections::BTreeMap;
    use std::sync::Arc;

    use super::super::contract::{
        HostToolFacts, NarrowedStageLimits, StageContractDraft, StageDeclaration, StageLimits,
        StageRunFacts, StageSetDeclaration, TargetDeclaration,
    };
    use super::super::ledger::{LedgerEntry, LedgerRegistry};
    use super::super::{StageRun, StageScope};

    let temp = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(temp.path()).unwrap();
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let draft = StageContractDraft::new(
        &StageSetDeclaration {
            stages: vec![StageDeclaration {
                id: "only".to_string(),
                targets: vec![TargetDeclaration {
                    id: "main".to_string(),
                    tool_id: "t0".to_string(),
                }],
                depends_on: Vec::new(),
                required_checks: Vec::new(),
            }],
            limits: NarrowedStageLimits::default(),
        },
        &[HostToolFacts {
            tool_id: "t0".to_string(),
            output_id: "artifact".to_string(),
            output_path: "out/only.json".to_string(),
            schema_inputs: Vec::new(),
            has_revision_argument: false,
            has_compiler: false,
            required_checks: vec!["shape".to_string()],
            limits: StageLimits {
                attempts_per_target: 2,
                attempts_per_run: 32,
                deadline_seconds: 60,
            },
        }],
    )
    .unwrap();
    let ledgers = Arc::new(LedgerRegistry::default());
    let run = StageRun::new(
        draft,
        BTreeMap::new(),
        workspace.clone(),
        "a-profile".to_string(),
        ledgers.clone(),
    );
    run.bind_session(
        StageRunFacts::for_test(true, false),
        &root.join("state"),
        LedgerEntry::Fresh,
    )
    .unwrap();
    let deadline = || Instant::now() + Duration::from_secs(1);

    assert!(
        ledgers
            .held_within(&workspace, "a-profile", deadline())
            .unwrap()
            .is_none()
    );
    assert_eq!(super::observe(&run, deadline()), None);

    // A judgment opens the ledger; the record is then the copy's verdict, with its own nonce.
    assert!(run.judge(StageScope::All).is_some(), "nothing is sealed");
    let observed = super::observe(&run, deadline()).expect("the ledger is held now");
    assert!(!observed.complete);
    assert!(observed.ledger_intact);
    assert!(!observed.run_nonce.is_empty());
    assert_eq!(observed.stages.len(), 1);
}
