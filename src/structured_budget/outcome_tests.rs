//! A command under a wallet is no success once a send of it was refused, however the run takes the
//! refusal, and a REPL's next command is judged by its own. Every send goes through the wallet as a
//! client's does and reaches no network: a send let out is settled with a known report in place of
//! a response, and the reply is a script's.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use super::super::identity::SendPurpose;
use super::super::ledger::LedgerFailure;
use super::super::reservation::BudgetRefusal;
use super::super::session::FinalRepairMarker;
use super::super::test_support::{FakeClock, TestWallet, body_asking_for};
use super::super::transport::{self, BudgetedBody, RunBudgetHandle, SendRefusal};
use super::{CommandRefusal, begin_command, gate, gate_session, refusal, refusal_text};
use crate::config::Config;
use crate::minimal_loop::loop_run::{
    RunSessionOptions, RunSessionOutcome, run_session_with_outcome_with_options,
};
use crate::planner::ultra_plan::{UltraPhase, UltraPlan};
use crate::providers::{AssistantReply, ChatClient};
use crate::state::{ConversationMessage, SessionSnapshot, ToolCall};
use crate::tools::registry::ToolSpec;
use crate::tui::NOOP_UI;

/// What a send under these limits costs is set by what it asks for: 50 tokens of output reserve
/// 720 per million dollars, 99 reserve 1,210, and each is settled at 10 per million. So under a
/// limit of 1,000 a send of 99 is refused while one of 50 goes, again and again.
const SMALL: u64 = 50;
const BIG: u64 = 99;

fn cheap(value: &mut Value) {
    value["max_output_tokens"] = json!(BIG);
    value["limits"]["cost_usd"] = json!("0.001");
}

/// One turn of a script: what its send asks for, and what the model says when it goes.
#[derive(Clone)]
struct Turn {
    asks: u64,
    /// The body that settles the send.
    report: &'static str,
    reply: AssistantReply,
}

fn write(path: &str, content: &str) -> AssistantReply {
    AssistantReply {
        content: String::new(),
        tool_calls: vec![ToolCall::new(
            "Write",
            json!({"path": path, "content": content}),
        )],
        prompt_tokens: None,
        completion_tokens: None,
    }
}

fn turn(asks: u64, reply: AssistantReply) -> Turn {
    Turn {
        asks,
        report: TestWallet::KNOWN,
        reply,
    }
}

/// A client whose every call is one send of the wallet. A refused send leaves its turn for the
/// next call, as a client would send the same bytes again.
#[derive(Clone)]
struct Script {
    handle: Arc<RunBudgetHandle>,
    turns: Arc<Mutex<VecDeque<Turn>>>,
}

impl Script {
    fn of(handle: &Arc<RunBudgetHandle>, turns: Vec<Turn>) -> Self {
        Self {
            handle: handle.clone(),
            turns: Arc::new(Mutex::new(turns.into())),
        }
    }
}

impl ChatClient for Script {
    fn label(&self) -> &str {
        "script-976"
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
        let mut turns = self.turns.lock().unwrap();
        let Some(next) = turns.front().cloned() else {
            anyhow::bail!("the script ran out");
        };
        let body: Value = serde_json::from_slice(&body_asking_for(next.asks)).unwrap();
        let budgeted = BudgetedBody::new(&self.handle, &body, SendPurpose::Executor)?
            .of_session(crate::provider_call::current_final_repair());
        let (_request, send) = budgeted
            .begin(reqwest::Client::new().post("http://127.0.0.1:9/v1/chat/completions"))
            .map_err(transport::refused)?;
        transport::settle_body(&mut Some(send), next.report).map_err(transport::refused)?;
        turns.pop_front();
        Ok(next.reply)
    }
}

struct Command {
    wallet: TestWallet,
    config: Config,
}

impl Command {
    fn under(change: impl FnOnce(&mut Value)) -> Self {
        let wallet = TestWallet::open(change);
        let config = config_on(&wallet.workspace, Some(wallet.handle.clone()));
        Self { wallet, config }
    }

    /// A repair session, which ends at the tool result that writes its artifact: one send.
    fn session(&self, turns: Vec<Turn>) -> anyhow::Result<RunSessionOutcome> {
        let mut client = Script::of(&self.wallet.handle, turns);
        run_session_with_outcome_with_options(
            &mut client,
            &mut SessionSnapshot::new(),
            "Repair final acceptance.\n\nRequired final artifacts:\n- target.txt",
            &["target.txt".to_string()],
            &self.config,
            &NOOP_UI,
            RunSessionOptions::final_acceptance_repair(),
        )
    }
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

fn command_refusal(error: &anyhow::Error) -> Option<CommandRefusal> {
    error.downcast_ref::<CommandRefusal>().copied()
}

fn limit(refusal: BudgetRefusal) -> SendRefusal {
    SendRefusal::Wallet(LedgerFailure::Budget(refusal))
}

/// C4 (b), mended: the refused session's caller passes the refusal over and goes on, and the next
/// session, whose send goes through, is still no success, nor is the plan.
#[test]
fn a_refusal_passed_over_leaves_the_command_no_success() {
    let command = Command::under(cheap);
    let refused = command.session(vec![turn(BIG, write("target.txt", "big\n"))]);
    assert_eq!(
        transport::budget_refusal(refused.as_ref().unwrap_err()),
        Some(BudgetRefusal::CostExceeded)
    );
    assert_eq!(command.wallet.sends(), 0);
    let after = command.session(vec![turn(SMALL, write("target.txt", "small\n"))]);
    assert_eq!(command.wallet.sends(), 1, "its send went out");
    let expected = CommandRefusal::SendRefused(limit(BudgetRefusal::CostExceeded));
    assert_eq!(command_refusal(after.as_ref().unwrap_err()), Some(expected));
    let plan = crate::planner::structured_stages::refuse_unmet_plan(&command.config).unwrap_err();
    assert_eq!(command_refusal(&plan), Some(expected), "G5 answers first");
    assert_eq!(
        crate::planner::structured_stages::final_gate_failure(&command.config),
        Some(expected.to_string()),
        "with no stage declared, G5 still answers"
    );
}

/// C4 (a), mended: a wallet that stopped on a bill it could not read lets no command succeed,
/// even one whose session needed no more.
#[test]
fn a_command_whose_wallet_cannot_be_called_settled_is_no_success() {
    let command = Command::under(cheap);
    let unread = Turn {
        report: r#"{"id":"no usage"}"#,
        ..turn(SMALL, write("target.txt", "done\n"))
    };
    let after = command.session(vec![unread]);
    assert_eq!(command.wallet.sends(), 1);
    assert_eq!(
        command_refusal(after.as_ref().unwrap_err()),
        Some(CommandRefusal::WalletUnsettled(limit(
            BudgetRefusal::Latched
        )))
    );
}

/// Every limit's refusal, each in a command of its own, and what each gate then says.
#[test]
fn every_limit_that_refuses_a_send_ends_its_command_as_no_success() {
    type Row = (
        &'static str,
        fn(&mut Value),
        fn(&TestWallet, &Arc<FakeClock>),
        BudgetRefusal,
    );
    let rows: [Row; 6] = [
        (
            "physical",
            |value| value["limits"]["physical_sends"] = json!(1),
            |wallet, _| {
                wallet.send(SMALL, None).unwrap();
                wallet.send(SMALL, None).unwrap_err();
            },
            BudgetRefusal::PhysicalExceeded,
        ),
        (
            "cost",
            cheap,
            |wallet, _| {
                wallet.send(BIG, None).unwrap_err();
            },
            BudgetRefusal::CostExceeded,
        ),
        (
            "tokens",
            // 160 a send reserves, 15 each is settled at.
            |value| value["limits"]["tokens"] = json!(170),
            |wallet, _| {
                wallet.send(SMALL, None).unwrap();
                wallet.send(SMALL, None).unwrap_err();
            },
            BudgetRefusal::TokensExceeded,
        ),
        (
            "wall",
            |value| value["limits"]["wall_seconds"] = json!(10),
            |wallet, clock| {
                wallet.send(SMALL, None).unwrap();
                clock.advance(10_001);
                wallet.send(SMALL, None).unwrap_err();
            },
            BudgetRefusal::WallExceeded,
        ),
        (
            "final repair attempts",
            |value| value["limits"]["final_repair_attempts"] = json!(1),
            |wallet, _| {
                wallet
                    .send(SMALL, Some(&FinalRepairMarker::default()))
                    .unwrap();
                wallet
                    .send(SMALL, Some(&FinalRepairMarker::default()))
                    .unwrap_err();
            },
            BudgetRefusal::FinalRepairAttemptsExceeded,
        ),
        (
            "final repair wall",
            |value| value["limits"]["final_repair_seconds"] = json!(10),
            |wallet, clock| {
                let marker = FinalRepairMarker::default();
                wallet.send(SMALL, Some(&marker)).unwrap();
                clock.advance(10_001);
                wallet.send(SMALL, Some(&marker)).unwrap_err();
            },
            BudgetRefusal::FinalRepairWallExceeded,
        ),
    ];
    for (name, change, refuse, expected) in rows {
        let command = Command::under(change);
        assert_eq!(refusal(&command.config), None, "{name}: none before");
        refuse(&command.wallet, &command.wallet.clock);
        let want = CommandRefusal::SendRefused(limit(expected));
        assert_eq!(refusal(&command.config), Some(want), "{name}");
        assert_eq!(
            command_refusal(&gate(&command.config).unwrap_err()),
            Some(want),
            "{name}"
        );
        assert_eq!(
            command_refusal(&gate_session(Ok(()), &command.config).unwrap_err()),
            Some(want),
            "{name}"
        );
        assert_eq!(
            refusal_text(&command.config),
            Some(want.to_string()),
            "{name}"
        );
    }
}

/// G5 makes no error a success, and lets every error stand as it was.
#[test]
fn an_error_stays_the_error_it_was() {
    let command = Command::under(cheap);
    command.wallet.send(BIG, None).unwrap_err();
    let failed: anyhow::Result<()> = Err(anyhow::anyhow!("as it was"));
    assert_eq!(
        gate_session(failed, &command.config)
            .unwrap_err()
            .to_string(),
        "as it was"
    );
}

/// The control: under the limits a command succeeds as before, and with no wallet nothing here
/// says anything.
#[test]
fn a_command_within_its_limits_and_one_with_no_wallet_succeed_as_before() {
    let command = Command::under(cheap);
    let outcome = command.session(vec![turn(SMALL, write("target.txt", "done\n"))]);
    assert!(outcome.is_ok(), "{:?}", outcome.err());
    assert_eq!(refusal(&command.config), None);
    crate::planner::structured_stages::refuse_unmet_plan(&command.config).unwrap();

    let off = config_on(&command.wallet.workspace, None);
    assert_eq!(refusal(&off), None);
    assert_eq!(gate_session(Ok(7), &off).unwrap(), 7);
    begin_command(&off);
}

/// T-REPL: each command of a REPL starts with no refusal of its own. The first is refused and is
/// no success; the second, with money left, succeeds; the third, with no send left, is refused
/// again.
#[test]
fn each_command_of_a_repl_is_judged_by_its_own_refusals() {
    let command = Command::under(|value| {
        cheap(value);
        value["limits"]["physical_sends"] = json!(1);
    });
    begin_command(&command.config);
    let first = command.session(vec![turn(BIG, write("target.txt", "big\n"))]);
    assert!(first.is_err());
    assert!(refusal(&command.config).is_some());

    begin_command(&command.config);
    let second = command.session(vec![turn(SMALL, write("target.txt", "small\n"))]);
    assert!(second.is_ok(), "{:?}", second.err());
    assert_eq!(refusal(&command.config), None);

    begin_command(&command.config);
    let third = command.session(vec![turn(SMALL, write("target.txt", "again\n"))]);
    assert_eq!(
        transport::budget_refusal(third.as_ref().unwrap_err()),
        Some(BudgetRefusal::PhysicalExceeded)
    );
    assert_eq!(
        refusal(&command.config),
        Some(CommandRefusal::SendRefused(limit(
            BudgetRefusal::PhysicalExceeded
        )))
    );
}

/// The REPL begins each command where the line it read is one: once in its loop, after an empty
/// line goes back to the prompt and before anything the line asks for runs. The loop reads a
/// terminal, which no test has, so this is a reading of its source.
#[test]
fn the_repl_begins_each_command_with_no_refusal_of_the_last() {
    let source = include_str!("../tui/repl.rs");
    let call = "crate::structured_budget::outcome::begin_command(&config);";
    assert_eq!(source.matches(call).count(), 1, "one beginning");
    let at = source.find(call).unwrap();
    let loop_at = source.find("\n    loop {\n").expect("the command loop");
    let empty_at = source[loop_at..]
        .find("if line.is_empty() {")
        .map(|offset| loop_at + offset)
        .expect("the empty line");
    let dispatch_at = source[loop_at..]
        .find("let words = crate::tui::slash::parse_words(line);")
        .map(|offset| loop_at + offset)
        .expect("the dispatch");
    assert!(
        empty_at < at && at < dispatch_at,
        "inside the loop, before the line runs"
    );
}

// ---- a flow run again within one command ----

const GOAL: &str = "Build an interactive memo app with add and delete actions";

fn memo_source() -> &'static str {
    r#"import { useState } from "react";
export default function Memo(){
  const [items, setItems] = useState([]);
  return <form onSubmit={(event) => { event.preventDefault(); setItems([...items, "note"]); }}>
<input onChange={() => setItems([...items, "draft"])} />
<button type="submit">Add</button>
<ul>{items.map((item, index) => <li key={index}>{item}</li>)}</ul>
  </form>;
}
"#
}

fn plan() -> UltraPlan {
    UltraPlan {
        goal: GOAL.to_string(),
        profile: "generic".to_string(),
        style: "default".to_string(),
        intent: "create".to_string(),
        phases: vec![
            UltraPhase {
                id: "scaffold".to_string(),
                prompt: "Create the initial scaffold.".to_string(),
            },
            UltraPhase {
                id: "finish".to_string(),
                prompt: "Complete the final behavior and verification evidence.".to_string(),
            },
        ],
    }
}

fn step_plan(goal: &str, path: &str) -> AssistantReply {
    AssistantReply::text(
        json!({
            "goal": goal,
            "steps": [{
                "id": "write-artifact",
                "kind": "implement",
                "expected_result": "pass",
                "instruction": format!("Create {path}."),
                "expected_paths": [path],
                "verify": []
            }]
        })
        .to_string(),
    )
}

struct Flow {
    planner: Script,
    execution: Script,
}

/// The whole of one ultra run, each of its sends asking for `asks`.
fn flow(handle: &Arc<RunBudgetHandle>, planner_asks: u64) -> Flow {
    Flow {
        planner: Script::of(
            handle,
            vec![
                turn(planner_asks, step_plan("Write notes", "notes.txt")),
                turn(SMALL, step_plan("Create generic app source", "memo.jsx")),
            ],
        ),
        execution: Script::of(
            handle,
            vec![
                turn(SMALL, write("notes.txt", "scaffold notes")),
                turn(SMALL, write("memo.jsx", memo_source())),
            ],
        ),
    }
}

fn run(config: &Config, mut flow: Flow) -> anyhow::Result<String> {
    crate::planner::run_ultra_plan_with_ui(
        &mut flow.planner,
        &mut flow.execution,
        &plan(),
        config,
        &NOOP_UI,
    )
}

fn completions(events: &Path) -> usize {
    std::fs::read_to_string(events)
        .unwrap_or_default()
        .lines()
        .filter(|line| line.contains("\"event\":\"ultra_plan_complete\""))
        .count()
}

fn flow_command(change: impl FnOnce(&mut Value)) -> (Command, PathBuf) {
    let mut command = Command::under(change);
    let events = command
        .wallet
        .workspace
        .parent()
        .unwrap()
        .join("events.jsonl");
    command.config.eval_events_path = Some(events.clone());
    (command, events)
}

/// The control: an ultra run under its limits completes, and says so once.
#[test]
fn an_ultra_run_within_its_limits_completes_as_before() {
    let (command, events) = flow_command(cheap);
    let result = run(&command.config, flow(&command.wallet.handle, SMALL));
    assert_eq!(result.unwrap(), "ultra-plan-run complete: 2 phases");
    assert_eq!(completions(&events), 1);
}

/// T-RERUN: auto recovery runs a flow again within its command when the first failed. A refusal
/// in the first is still the command's in the second, which goes all the way and is no success,
/// with no completion said.
#[test]
fn a_flow_run_again_within_its_command_keeps_the_refusal_of_the_first() {
    let (command, events) = flow_command(cheap);
    let first = run(&command.config, flow(&command.wallet.handle, BIG));
    assert!(first.is_err());
    assert_eq!(command.wallet.sends(), 0, "the planner's send was refused");
    let second = run(&command.config, flow(&command.wallet.handle, SMALL));
    let error = second.unwrap_err();
    let want = CommandRefusal::SendRefused(limit(BudgetRefusal::CostExceeded));
    // The phase that ends the run puts its own words around the gate's; they are still the gate's.
    assert!(
        format!("{error:#}").contains(&want.to_string()),
        "{error:#}"
    );
    assert_eq!(refusal(&command.config), Some(want));
    assert!(command.wallet.sends() > 0, "the second went on and sent");
    assert_eq!(completions(&events), 0);
}
