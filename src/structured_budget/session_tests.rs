//! A final repair is the host's to say, and a session opens one. Each send here goes through the
//! wallet as a client's does and reaches no network: a send that is let out is settled with a known
//! report in place of a response.

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::super::identity::{FinalRepairClaim, FinalRepairId, SendPurpose};
use super::super::ledger::{BudgetLedger, CommandWrites, LedgerEntry, LedgerFailure, LedgerOpen};
use super::super::policy::RunBudgetPolicy;
use super::super::reservation::{BudgetRefusal, ReserveRequest};
use super::super::test_support::{BODY, FakeClock, WALLET, policy_value};
use super::super::transport::{self, BudgetedBody, RunBudgetHandle, SandboxView, SendRefusal};
use super::FinalRepairMarker;
use crate::config::Config;
use crate::provider_call::{
    self, ProviderCallScope, ProviderChatRequest, current_final_repair as current,
    enter_final_repair as enter,
};
use crate::providers::{AssistantReply, ChatClient};
use crate::state::ConversationMessage;
use crate::tools::os_sandbox::SandboxPolicy;
use crate::tools::registry::ToolSpec;

const KNOWN: &str = r#"{"usage":{"prompt_tokens":10,"completion_tokens":5,"cost":0.00001}}"#;

struct Wallet {
    _root: tempfile::TempDir,
    handle: Arc<RunBudgetHandle>,
}

fn policy_at(base: &Path, change: impl FnOnce(&mut Value)) -> Arc<RunBudgetPolicy> {
    let mut value = policy_value();
    value["ledger_dir"] = json!(base.join("ledgers").to_str().unwrap());
    change(&mut value);
    Arc::new(RunBudgetPolicy::decode(value.to_string().as_bytes(), WALLET).unwrap())
}

/// A wallet on `<base>/workspace`, judged by a sandbox a test holds.
fn open_at<'a>(
    base: &Path,
    workspace: &'a Path,
    change: impl FnOnce(&mut Value),
) -> LedgerOpen<'a> {
    std::fs::create_dir_all(workspace).unwrap();
    let run_tmp = base.join("run-tmp");
    std::fs::create_dir_all(&run_tmp).unwrap();
    LedgerOpen {
        policy: policy_at(base, change),
        workspace,
        explicit_profile: None,
        entry: LedgerEntry::Fresh,
        clock: FakeClock::at(7_000),
        command_writes: Some(CommandWrites::of(SandboxPolicy::with_home(
            workspace,
            &run_tmp,
            base.join("home"),
        ))),
    }
}

impl Wallet {
    fn with(change: impl FnOnce(&mut Value)) -> Self {
        let root = tempfile::tempdir().unwrap();
        let base = std::fs::canonicalize(root.path()).unwrap();
        let workspace = base.join("workspace");
        let handle = RunBudgetHandle::open(open_at(&base, &workspace, change))
            .unwrap()
            .with_sandbox_view(SandboxView {
                applied_on: Some(base.join("workspace")),
            });
        Self {
            _root: root,
            handle: Arc::new(handle),
        }
    }

    fn final_repairs(limit: u64) -> Self {
        Self::with(|value| value["limits"]["final_repair_attempts"] = json!(limit))
    }

    /// One send of a client of `role`, in the session `marker` marks, settled when it is let out.
    fn send(
        &self,
        role: SendPurpose,
        marker: Option<&FinalRepairMarker>,
    ) -> Result<(), SendRefusal> {
        let body: Value = serde_json::from_slice(BODY).unwrap();
        let budgeted = BudgetedBody::new(&self.handle, &body, role)
            .unwrap()
            .of_session(marker.cloned());
        let (_request, send) = budgeted
            .begin(reqwest::Client::new().post("http://127.0.0.1:9/v1/chat/completions"))?;
        transport::settle_body(&mut Some(send), KNOWN)
    }

    fn sends(&self) -> u64 {
        self.handle.wallet().unwrap().physical_used()
    }

    /// Each send's purpose and final repair, in order.
    fn attempts(&self) -> Vec<(SendPurpose, Option<u64>)> {
        self.handle
            .wallet()
            .unwrap()
            .attempts
            .iter()
            .map(|attempt| (attempt.purpose, attempt.final_repair.map(|id| id.0)))
            .collect()
    }
}

fn refused(refusal: BudgetRefusal) -> Result<(), SendRefusal> {
    Err(SendRefusal::Wallet(LedgerFailure::Budget(refusal)))
}

#[test]
fn a_session_opens_one_final_repair_and_every_later_send_of_it_continues_it() {
    let wallet = Wallet::final_repairs(1);
    let session = FinalRepairMarker::default();
    for _ in 0..5 {
        assert_eq!(
            wallet.send(SendPurpose::Executor, Some(&session.clone())),
            Ok(())
        );
    }
    assert_eq!(
        wallet.attempts(),
        vec![(SendPurpose::FinalRepair, Some(1)); 5]
    );
    assert_eq!(wallet.handle.wallet().unwrap().final_repair_used(), 1);
    assert_eq!(
        session.claim(),
        FinalRepairClaim::Continue(FinalRepairId(1))
    );
}

#[test]
fn another_session_opens_another_and_one_past_the_limit_sends_nothing() {
    let wallet = Wallet::final_repairs(2);
    let (first, second, third) = (
        FinalRepairMarker::default(),
        FinalRepairMarker::default(),
        FinalRepairMarker::default(),
    );
    assert_eq!(wallet.send(SendPurpose::Executor, Some(&first)), Ok(()));
    assert_eq!(wallet.send(SendPurpose::Executor, Some(&second)), Ok(()));
    assert_eq!(
        wallet.send(SendPurpose::Executor, Some(&third)),
        refused(BudgetRefusal::FinalRepairAttemptsExceeded)
    );
    assert_eq!(wallet.sends(), 2, "the refused one reserved nothing");
    assert_eq!(third.claim(), FinalRepairClaim::New, "it opened nothing");
    // The sessions already open go on in the next command (3e: the refused one sends nothing more,
    // on purpose, to close H3): continuing one counts nothing new.
    wallet.handle.forget_command_refusals();
    assert_eq!(wallet.send(SendPurpose::Executor, Some(&first)), Ok(()));
    assert_eq!(wallet.send(SendPurpose::Executor, Some(&second)), Ok(()));
    assert_eq!(
        wallet.attempts(),
        vec![
            (SendPurpose::FinalRepair, Some(1)),
            (SendPurpose::FinalRepair, Some(2)),
            (SendPurpose::FinalRepair, Some(1)),
            (SendPurpose::FinalRepair, Some(2)),
        ]
    );
    assert_eq!(wallet.handle.wallet().unwrap().final_repair_used(), 2);
}

/// In the command a final repair was refused in, no session open before it sends again: not a
/// continue, not another purpose. The refusal stays the first one.
#[test]
fn after_a_refused_final_repair_no_session_of_the_command_sends() {
    let wallet = Wallet::final_repairs(2);
    let (first, second, third) = (
        FinalRepairMarker::default(),
        FinalRepairMarker::default(),
        FinalRepairMarker::default(),
    );
    assert_eq!(wallet.send(SendPurpose::Executor, Some(&first)), Ok(()));
    assert_eq!(wallet.send(SendPurpose::Executor, Some(&second)), Ok(()));
    let limit = refused(BudgetRefusal::FinalRepairAttemptsExceeded);
    assert_eq!(wallet.send(SendPurpose::Executor, Some(&third)), limit);
    for marker in [Some(&first), Some(&second), None] {
        assert_eq!(wallet.send(SendPurpose::Executor, marker), limit);
    }
    assert_eq!(wallet.sends(), 2, "nothing more reserved or sent");
    assert_eq!(
        wallet.handle.refused_in_command(),
        limit.err(),
        "the first refusal stays"
    );
}

#[test]
fn a_continue_of_a_final_repair_the_ledger_never_opened_sends_nothing() {
    let wallet = Wallet::final_repairs(5);
    for id in [0, 1, 7] {
        let forged = FinalRepairMarker::holding(FinalRepairId(id));
        assert_eq!(
            wallet.send(SendPurpose::Executor, Some(&forged)),
            refused(BudgetRefusal::NoSuchFinalRepair),
            "{id}"
        );
    }
    assert_eq!(wallet.sends(), 0);
}

/// A send of no final-repair session is what its client's role says, and opens none.
#[test]
fn a_send_of_no_final_repair_session_is_as_its_role_says() {
    let wallet = Wallet::final_repairs(1);
    for role in [
        SendPurpose::Executor,
        SendPurpose::Planner,
        SendPurpose::Classifier,
    ] {
        assert_eq!(wallet.send(role, None), Ok(()));
    }
    assert_eq!(
        wallet.attempts(),
        vec![
            (SendPurpose::Executor, None),
            (SendPurpose::Planner, None),
            (SendPurpose::Classifier, None),
        ]
    );
    assert_eq!(wallet.handle.wallet().unwrap().final_repair_used(), 0);
}

/// A claim with another purpose, and the purpose with no claim, are refused before a send: no way
/// of saying it lets a final repair out uncounted.
#[test]
fn a_final_repair_cannot_go_around_its_count_by_another_purpose_or_none() {
    let root = tempfile::tempdir().unwrap();
    let base = std::fs::canonicalize(root.path()).unwrap();
    let workspace = base.join("workspace");
    let mut ledger = BudgetLedger::open(open_at(&base, &workspace, |_| {})).unwrap();
    for (purpose, claim) in [
        (SendPurpose::Repair, Some(FinalRepairClaim::New)),
        (SendPurpose::Executor, Some(FinalRepairClaim::New)),
        (SendPurpose::FinalRepair, None),
    ] {
        let got = ledger.reserve(&ReserveRequest {
            body: BODY,
            purpose,
            final_repair: claim,
        });
        assert_eq!(
            got.err(),
            Some(LedgerFailure::Budget(BudgetRefusal::PurposeDiffers)),
            "{purpose:?} {claim:?}"
        );
    }
    assert_eq!(ledger.wallet().physical_used(), 0);
}

#[test]
fn a_session_puts_back_the_marker_it_found_when_it_ends() {
    assert!(current().is_none());
    let outer = FinalRepairMarker::default();
    {
        let _outer = enter(Some(&outer));
        assert!(current().unwrap().same_session(&outer));
        {
            let _inner = enter(None);
            assert!(current().is_none(), "a session of no final repair has none");
        }
        assert!(current().unwrap().same_session(&outer));
    }
    assert!(
        current().is_none(),
        "nothing of the session stays on the thread"
    );
}

/// Which client's worker held which marker.
type Seen = Arc<Mutex<Vec<(&'static str, Option<FinalRepairMarker>)>>>;

/// A client that tells what marker its call's worker holds, once `open` lets it go on.
#[derive(Clone)]
struct Observing {
    name: &'static str,
    open: Option<Arc<(Mutex<bool>, Condvar)>>,
    seen: Seen,
}

impl ChatClient for Observing {
    fn label(&self) -> &str {
        "observing-976"
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
        if let Some(open) = &self.open {
            let (lock, ready) = &**open;
            let mut opened = lock.lock().unwrap();
            while !*opened {
                opened = ready.wait(opened).unwrap();
            }
        }
        self.seen.lock().unwrap().push((self.name, current()));
        Ok(AssistantReply::text("done"))
    }
}

fn call_config(base: &Path) -> Config {
    let workspace = base.join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let parsed = crate::provider_cli::parse_from([
        "commandagent",
        "--cwd",
        workspace.to_str().unwrap(),
        "--state-dir",
        base.join("state").to_str().unwrap(),
    ])
    .unwrap();
    Config::from_cli_with_provider_options(parsed.cli, parsed.provider_options).unwrap()
}

fn call(client: &mut Observing, config: &Config, is_cancelled: impl Fn() -> bool) {
    let _ = provider_call::chat_with_cancel(
        client,
        config,
        ProviderChatRequest {
            scope: ProviderCallScope::Executor,
            model: "m-1",
            messages: &[ConversationMessage::user("hello-976")],
            tools: &[],
            native_tools_enabled: false,
        },
        is_cancelled,
    );
}

/// A call given up on leaves its worker out. That worker holds its own call's marker, and the next
/// call, of another session, holds that session's: nothing is shared between them to be swapped.
/// The order is the gate's, not a length of time.
#[test]
fn a_worker_out_after_its_call_keeps_that_calls_marker_and_the_next_call_its_own() {
    let root = tempfile::tempdir().unwrap();
    let config = call_config(root.path());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let open = Arc::new((Mutex::new(false), Condvar::new()));
    let repair = FinalRepairMarker::default();
    {
        let _session = enter(Some(&repair));
        let mut late = Observing {
            name: "late",
            open: Some(open.clone()),
            seen: seen.clone(),
        };
        // Not cancelled when asked before the worker starts, cancelled after: the call returns
        // with its worker still waiting.
        let asked = AtomicUsize::new(0);
        call(&mut late, &config, || {
            asked.fetch_add(1, Ordering::SeqCst) > 0
        });
    }
    let next = FinalRepairMarker::default();
    {
        let _session = enter(Some(&next));
        let mut client = Observing {
            name: "next",
            open: None,
            seen: seen.clone(),
        };
        call(&mut client, &config, || false);
    }
    {
        let _session = enter(None);
        let mut client = Observing {
            name: "plain",
            open: None,
            seen: seen.clone(),
        };
        call(&mut client, &config, || false);
    }
    let (lock, ready) = &*open;
    *lock.lock().unwrap() = true;
    ready.notify_all();
    let started = Instant::now();
    while seen.lock().unwrap().len() < 3 {
        assert!(
            started.elapsed() < Duration::from_secs(30),
            "the late worker never went on"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    let seen = seen.lock().unwrap();
    let marker_of = |name: &str| {
        seen.iter()
            .find(|(seen_name, _)| *seen_name == name)
            .map(|(_, marker)| marker.clone())
            .unwrap()
    };
    assert!(marker_of("late").unwrap().same_session(&repair));
    assert!(marker_of("next").unwrap().same_session(&next));
    assert!(marker_of("plain").is_none());
}
