//! Once its wallet refused anything of a command, nothing more of the command starts; a late
//! worker's refusal is its own command's; a later profile and `/resume` go on in the wallet held.
//! Every send goes through the wallet as a client's does and reaches no network.

use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc;

use serde_json::{Value, json};

use super::super::entry::EntryRefusal;
use super::super::identity::SendPurpose;
use super::super::ledger::{CommandWrites, LedgerEntry, LedgerFailure, LedgerOpen, LedgerRefusal};
use super::super::outcome::{CommandRefusal, begin_command, refusal};
use super::super::policy::RunBudgetPolicy;
use super::super::reservation::BudgetRefusal;
use super::super::test_support::{FakeClock, TestWallet, WALLET, body_asking_for, policy_value};
use super::super::transport::{self, BudgetedBody, RunBudgetHandle, SandboxView, SendRefusal};
use super::{guard, pin_profile, refuse_resume, refused};
use crate::config::Config;
use crate::planner::ultra_plan::{UltraPhase, UltraPlan};
use crate::providers::{AssistantReply, ChatClient};
use crate::state::ConversationMessage;
use crate::tools::os_sandbox::SandboxPolicy;
use crate::tools::registry::ToolSpec;
use crate::tui::NOOP_UI;

/// Under [`cheap`], a send asking for this much is refused for its cost, and one of [`SMALL`] goes.
const BIG: u64 = 99;
const SMALL: u64 = 50;

fn cheap(value: &mut Value) {
    value["max_output_tokens"] = json!(BIG);
    value["limits"]["cost_usd"] = json!("0.001");
}

fn cost_refused() -> SendRefusal {
    SendRefusal::Wallet(LedgerFailure::Budget(BudgetRefusal::CostExceeded))
}

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
    config.yes = true;
    config.max_iterations = 4;
    config.eval_events_path = None;
    config.field_sources.run_budget = handle;
    config
}

/// Every call is one send of the wallet asking for `asks`; when it goes, the model says nothing
/// a run can use.
#[derive(Clone)]
struct Sending {
    handle: Arc<RunBudgetHandle>,
    asks: u64,
}

impl ChatClient for Sending {
    fn label(&self) -> &str {
        "sending-976-s3e"
    }

    fn boxed_clone(&self) -> Box<dyn ChatClient> {
        Box::new(self.clone())
    }

    fn chat(
        &mut self,
        _model: &str,
        _messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        let body: Value = serde_json::from_slice(&body_asking_for(self.asks)).unwrap();
        let budgeted = BudgetedBody::new(&self.handle, &body, SendPurpose::Executor)?;
        let (_request, send) = budgeted
            .begin(reqwest::Client::new().post("http://127.0.0.1:9/v1/chat/completions"))
            .map_err(transport::refused)?;
        transport::settle_body(&mut Some(send), TestWallet::KNOWN).map_err(transport::refused)?;
        anyhow::bail!("sending-976-s3e: nothing to say")
    }
}

fn plan() -> UltraPlan {
    UltraPlan {
        goal: "Write notes".to_string(),
        profile: "generic".to_string(),
        style: "default".to_string(),
        intent: "create".to_string(),
        phases: vec![
            UltraPhase {
                id: "scaffold".to_string(),
                prompt: "Create the notes.".to_string(),
            },
            UltraPhase {
                id: "finish".to_string(),
                prompt: "Finish the notes.".to_string(),
            },
        ],
    }
}

fn events_of(path: &Path) -> Vec<Value> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn names(events: &[Value]) -> Vec<String> {
    events
        .iter()
        .map(|event| event["event"].as_str().unwrap_or_default().to_string())
        .collect()
}

/// A wallet on `<base>/workspace` opened `entry`, judged by a sandbox the test holds.
fn open_at(base: &Path, entry: LedgerEntry, profile: Option<&str>) -> Arc<RunBudgetHandle> {
    let workspace = base.join("workspace");
    let run_tmp = base.join("run-tmp");
    std::fs::create_dir_all(&workspace).unwrap();
    std::fs::create_dir_all(&run_tmp).unwrap();
    let mut value = policy_value();
    value["ledger_dir"] = json!(base.join("ledgers").to_str().unwrap());
    let handle = RunBudgetHandle::open(LedgerOpen {
        policy: Arc::new(RunBudgetPolicy::decode(value.to_string().as_bytes(), WALLET).unwrap()),
        workspace: &workspace,
        explicit_profile: profile,
        entry,
        clock: FakeClock::at(7_000),
        command_writes: Some(CommandWrites::of(SandboxPolicy::with_home(
            &workspace,
            &run_tmp,
            base.join("home"),
        ))),
    })
    .unwrap()
    .with_sandbox_view(SandboxView {
        applied_on: Some(workspace),
    });
    Arc::new(handle)
}

// ---- off ----

/// With no wallet nothing here refuses, pins, or reads anything; a configuration whose slot was
/// emptied is off, while the handle it held goes on refusing for the one that still holds it.
#[test]
fn with_no_wallet_nothing_here_refuses_and_an_emptied_slot_is_off() {
    let wallet = TestWallet::open(cheap);
    let on = config_on(&wallet.workspace, Some(wallet.handle.clone()));
    wallet.send(BIG, None).unwrap_err();
    let mut off = on.clone();
    off.field_sources.run_budget = None;
    off.profile_explicit = true;
    off.profile = " ".to_string();
    assert_eq!(refused(&off), None);
    assert!(guard(&off).is_ok());
    assert!(refuse_resume(&off).is_ok());
    assert!(pin_profile(&off).is_ok());
    assert_eq!(
        refused(&on),
        Some(cost_refused()),
        "the handle still holds it"
    );
}

// ---- H3: nothing more of a refused command ----

/// After a refusal, no send of the command goes, whatever it asks for and whatever it is for, and
/// the refusal stays the first. The next command sends from what is left.
#[test]
fn a_command_the_wallet_refused_sends_nothing_more() {
    let wallet = TestWallet::open(cheap);
    let config = config_on(&wallet.workspace, Some(wallet.handle.clone()));
    assert_eq!(wallet.send(SMALL, None), Ok(()));
    assert_eq!(wallet.send(BIG, None), Err(cost_refused()));
    let marker = super::super::session::FinalRepairMarker::default();
    for (asks, final_repair) in [(SMALL, None), (SMALL, Some(&marker)), (BIG, None)] {
        assert_eq!(wallet.send(asks, final_repair), Err(cost_refused()));
    }
    assert_eq!(
        wallet.sends(),
        1,
        "nothing reserved or sent after the refusal"
    );
    assert_eq!(refused(&config), Some(cost_refused()));
    let error = guard(&config).unwrap_err();
    assert_eq!(
        error.downcast_ref::<CommandRefusal>(),
        Some(&CommandRefusal::SendRefused(cost_refused()))
    );

    begin_command(&config);
    assert_eq!(refused(&config), None);
    assert!(guard(&config).is_ok());
    assert_eq!(wallet.send(SMALL, None), Ok(()), "the next command sends");
    assert_eq!(wallet.sends(), 2, "from the same wallet");
}

/// A plan run in a command the wallet refused starts nothing: no event, no send.
#[test]
fn a_plan_run_in_a_refused_command_starts_nothing() {
    let wallet = TestWallet::open(cheap);
    let mut config = config_on(&wallet.workspace, Some(wallet.handle.clone()));
    let events = wallet.workspace.parent().unwrap().join("events.jsonl");
    config.eval_events_path = Some(events.clone());
    wallet.send(BIG, None).unwrap_err();
    let mut planner = Sending {
        handle: wallet.handle.clone(),
        asks: SMALL,
    };
    let mut execution = planner.clone();
    let error = crate::planner::run_ultra_plan_with_ui(
        &mut planner,
        &mut execution,
        &plan(),
        &config,
        &NOOP_UI,
    )
    .unwrap_err();
    assert_eq!(
        error.downcast_ref::<CommandRefusal>(),
        Some(&CommandRefusal::SendRefused(cost_refused()))
    );
    assert_eq!(names(&events_of(&events)), Vec::<String>::new());
    assert_eq!(wallet.sends(), 0);
}

/// The recovery events that come only once a recovery run has begun to start: its preflight, the
/// commands bound to its candidate, and its start.
const RECOVERY_STARTED: [&str; 7] = [
    "recovery_preflight_observation",
    "recovery_candidate_verify_commands_bound",
    "recovery_handoff_fidelity_bound",
    "recovery_candidate_completion_requirements_bound",
    "recovery_plan_auto_run_start",
    "recovery_treatment_delta",
    "recovery_plan_auto_run_complete",
];

/// One automatic recovery after a first run whose planner asks for `asks`.
fn recovered(planner_asks: u64) -> (TestWallet, Vec<Value>, anyhow::Result<String>) {
    let wallet = TestWallet::open(cheap);
    let mut config = config_on(&wallet.workspace, Some(wallet.handle.clone()));
    let events = wallet.workspace.parent().unwrap().join("events.jsonl");
    config.eval_events_path = Some(events.clone());
    config.recovery_plan_auto_runs = 1;
    let mut planner = Sending {
        handle: wallet.handle.clone(),
        asks: planner_asks,
    };
    let mut execution = Sending {
        handle: wallet.handle.clone(),
        asks: SMALL,
    };
    let result = crate::planner::auto_recovery::run_plan_with_ui(
        &mut planner,
        &mut execution,
        &plan(),
        &config,
        &NOOP_UI,
    );
    (wallet, events_of(&events), result)
}

/// H3: a first run the wallet refused is not run again. Nothing of a recovery run starts: no
/// preflight (no snapshot, observation, or verify command), no command bound to its candidate, no
/// preparation, no start, and no send. The stop is the one a failure that cannot be recovered
/// has, with the error of the first run.
#[test]
fn a_run_the_wallet_refused_is_not_run_again() {
    let (wallet, events, result) = recovered(BIG);
    let error = format!("{:#}", result.unwrap_err());
    assert!(error.contains(&cost_refused().to_string()), "{error}");
    let names = names(&events);
    for started in RECOVERY_STARTED {
        assert!(
            !names.iter().any(|name| name == started),
            "{started}: {names:?}"
        );
    }
    let stops = events
        .iter()
        .filter(|event| event["event"] == "recovery_plan_auto_run_stopped")
        .map(|event| event["recovery_plan_auto_run_stop_reason"].clone())
        .collect::<Vec<_>>();
    assert_eq!(stops, vec![json!("not_recoverable")]);
    assert_eq!(wallet.sends(), 0, "no send, then or after");
    assert!(
        !wallet
            .workspace
            .join(".commandagent/recovery-boundaries")
            .exists(),
        "no snapshot"
    );
}

/// The control: the same first run failing with nothing refused goes on to the recovery run's
/// preflight, as before.
#[test]
fn a_run_that_failed_with_nothing_refused_goes_on_to_recovery_as_before() {
    let (wallet, events, result) = recovered(SMALL);
    assert!(result.is_err());
    assert!(
        names(&events)
            .iter()
            .any(|name| name == "recovery_preflight_observation"),
        "{:?}",
        names(&events)
    );
    assert!(wallet.sends() > 0);
}

// ---- H1: a late worker's refusal is its own command's ----

/// A worker of command N begins its send only after the next command began, in that order, by a
/// channel and not a length of time. The wallet refuses it, and the refusal is N's: the next
/// command has none, and sends.
#[test]
fn a_late_worker_of_the_last_command_leaves_the_next_with_no_refusal() {
    let wallet = TestWallet::open(cheap);
    let config = config_on(&wallet.workspace, Some(wallet.handle.clone()));
    let body: Value = serde_json::from_slice(&body_asking_for(BIG)).unwrap();
    let late = BudgetedBody::new(&wallet.handle, &body, SendPurpose::Executor).unwrap();
    let (go, wait) = mpsc::channel::<()>();
    let (done, finished) = mpsc::channel::<Option<SendRefusal>>();
    let worker = std::thread::spawn(move || {
        wait.recv().unwrap();
        let refused = late
            .begin(reqwest::Client::new().post("http://127.0.0.1:9/v1/chat/completions"))
            .err();
        done.send(refused).unwrap();
    });
    begin_command(&config);
    go.send(()).unwrap();
    assert_eq!(
        finished.recv().unwrap(),
        Some(cost_refused()),
        "the wallet refused it"
    );
    worker.join().unwrap();
    assert_eq!(refusal(&config), None, "not the next command's");
    assert_eq!(refused(&config), None);
    assert_eq!(wallet.send(SMALL, None), Ok(()), "the next command sends");
}

/// The control: the same send refused in its own command is that command's.
#[test]
fn a_refusal_in_its_own_command_is_that_commands() {
    let wallet = TestWallet::open(cheap);
    let config = config_on(&wallet.workspace, Some(wallet.handle.clone()));
    begin_command(&config);
    assert_eq!(wallet.send(BIG, None), Err(cost_refused()));
    assert_eq!(
        refusal(&config),
        Some(CommandRefusal::SendRefused(cost_refused()))
    );
}

// ---- A: the REPL's commands, `/resume`, and a later profile ----

/// Two commands of a REPL spend one wallet: the second goes on from what the first left.
#[test]
fn the_commands_of_a_repl_spend_one_wallet() {
    let wallet = TestWallet::open(|value| value["limits"]["physical_sends"] = json!(2));
    let config = config_on(&wallet.workspace, Some(wallet.handle.clone()));
    begin_command(&config);
    assert_eq!(wallet.send(SMALL, None), Ok(()));
    begin_command(&config);
    assert_eq!(wallet.send(SMALL, None), Ok(()));
    begin_command(&config);
    assert_eq!(
        transport::budget_refusal(&transport::refused(wallet.send(SMALL, None).unwrap_err())),
        Some(BudgetRefusal::PhysicalExceeded)
    );
    assert_eq!(wallet.sends(), 2);
}

fn resume_line(config: &Config, handle: &Arc<RunBudgetHandle>) -> String {
    let mut planner = Sending {
        handle: handle.clone(),
        asks: SMALL,
    };
    let mut execution = planner.clone();
    let error = crate::tui::slash::handle_command(
        "/resume 018f0000-no-such-run",
        config,
        &mut planner,
        &mut execution,
        &NOOP_UI,
    )
    .unwrap_err();
    format!("{error:#}")
}

/// E1: `/resume` in a wallet this process made is refused as a resume with no record is, before the
/// run is even looked for. One the process found on disk goes on to look for the run.
#[test]
fn resume_in_a_wallet_this_process_made_is_refused() {
    let root = tempfile::tempdir().unwrap();
    let base = std::fs::canonicalize(root.path()).unwrap();
    let made = open_at(&base, LedgerEntry::Fresh, None);
    let config = config_on(&base.join("workspace"), Some(made.clone()));
    let refusal = EntryRefusal::Wallet(LedgerRefusal::Missing).to_string();
    let error = resume_line(&config, &made);
    assert!(error.contains(&refusal), "{error}");
    assert!(!error.contains("no resumable run"), "{error}");
    assert_eq!(
        refuse_resume(&config)
            .unwrap_err()
            .downcast_ref::<EntryRefusal>(),
        Some(&EntryRefusal::Wallet(LedgerRefusal::Missing))
    );
    assert_eq!(made.wallet().unwrap().physical_used(), 0, "nothing sent");
    drop(config);
    drop(made);

    for entry in [LedgerEntry::Fresh, LedgerEntry::Resume] {
        let found = open_at(&base, entry, None);
        let config = config_on(&base.join("workspace"), Some(found.clone()));
        assert!(refuse_resume(&config).is_ok(), "{entry:?}");
        let error = resume_line(&config, &found);
        assert!(error.contains("no resumable run"), "{entry:?}: {error}");
        assert!(!error.contains(&refusal), "{entry:?}: {error}");
    }
}

fn slash(config: &Config, handle: &Arc<RunBudgetHandle>, line: &str) -> anyhow::Result<String> {
    let mut planner = Sending {
        handle: handle.clone(),
        asks: SMALL,
    };
    let mut execution = planner.clone();
    crate::tui::slash::handle_command(line, config, &mut planner, &mut execution, &NOOP_UI)
}

fn wallet_refusal(error: &anyhow::Error) -> Option<EntryRefusal> {
    error.downcast_ref::<EntryRefusal>().copied()
}

/// E2: a profile named with a slash command after the wallet opened is pinned in it once; the same
/// one again goes on, another is refused before anything is sent, and the pin is on disk.
#[test]
fn a_later_profile_is_pinned_once_and_another_is_refused() {
    let wallet = TestWallet::open(|_| {});
    let config = config_on(&wallet.workspace, Some(wallet.handle.clone()));
    assert_eq!(wallet.handle.wallet().unwrap().profile, None);
    let first = slash(
        &config,
        &wallet.handle,
        "/ultra-plan --profile python-cli Build a tool",
    );
    assert!(first.as_ref().err().and_then(wallet_refusal).is_none());
    assert_eq!(
        wallet.handle.wallet().unwrap().profile.as_deref(),
        Some("python-cli")
    );
    let again = slash(
        &config,
        &wallet.handle,
        "/ultra-plan --profile python-cli Build more",
    );
    assert!(again.as_ref().err().and_then(wallet_refusal).is_none());
    let sends = wallet.sends();
    let other = slash(
        &config,
        &wallet.handle,
        "/ultra-plan --profile data Build a report",
    )
    .unwrap_err();
    assert_eq!(
        wallet_refusal(&other),
        Some(EntryRefusal::Wallet(LedgerRefusal::ProfileChanged))
    );
    assert_eq!(wallet.sends(), sends, "nothing sent");
    assert_eq!(
        wallet.handle.wallet().unwrap().profile.as_deref(),
        Some("python-cli")
    );
    let on_disk = std::fs::read_to_string(
        wallet
            .workspace
            .parent()
            .unwrap()
            .join("ledgers")
            .join(WALLET)
            .join("ledger.json"),
    )
    .unwrap();
    assert!(on_disk.contains(r#""profile":"python-cli""#), "{on_disk}");
}

/// A profile named at the start is the one pinned, and a later one is held to it.
#[test]
fn a_profile_named_at_the_start_holds_a_later_one_to_it() {
    let root = tempfile::tempdir().unwrap();
    let base = std::fs::canonicalize(root.path()).unwrap();
    let handle = open_at(&base, LedgerEntry::Fresh, Some("python-cli"));
    let mut config = config_on(&base.join("workspace"), Some(handle.clone()));
    config.profile_explicit = true;
    config.profile = "python-cli".to_string();
    assert!(pin_profile(&config).is_ok());
    config.profile = "data".to_string();
    assert_eq!(
        wallet_refusal(&pin_profile(&config).unwrap_err()),
        Some(EntryRefusal::Wallet(LedgerRefusal::ProfileChanged))
    );
}

/// L9: a profile with nothing in its name is refused, never taken for none; one that was only
/// inferred is not pinned.
#[test]
fn a_blank_profile_is_refused_and_an_inferred_one_is_not_pinned() {
    let wallet = TestWallet::open(|_| {});
    let mut config = config_on(&wallet.workspace, Some(wallet.handle.clone()));
    config.profile = "python-cli".to_string();
    config.profile_explicit = false;
    assert!(pin_profile(&config).is_ok());
    assert_eq!(wallet.handle.wallet().unwrap().profile, None, "inferred");
    for blank in ["", "   ", "\t"] {
        config.profile = blank.to_string();
        config.profile_explicit = true;
        assert_eq!(
            wallet_refusal(&pin_profile(&config).unwrap_err()),
            Some(EntryRefusal::Wallet(LedgerRefusal::ProfileBlank)),
            "{blank:?}"
        );
    }
    assert_eq!(wallet.handle.wallet().unwrap().profile, None);
}
