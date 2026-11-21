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
/// session sends nothing: its send is refused with the command's refusal. The command is no
/// success, nor is the plan. (3e turned the send count around on purpose, to close H3.)
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
    assert_eq!(command.wallet.sends(), 0, "its send did not go");
    let expected = CommandRefusal::SendRefused(limit(BudgetRefusal::CostExceeded));
    assert_eq!(
        after.as_ref().unwrap_err().downcast_ref::<SendRefusal>(),
        Some(&limit(BudgetRefusal::CostExceeded))
    );
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
/// in the first is still the command's in the second, which starts nothing, sends nothing, and is
/// no success, with no completion said. (3e turned the send count around on purpose, to close H3.)
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
    assert_eq!(command.wallet.sends(), 0, "the second sent nothing");
    assert_eq!(completions(&events), 0);
}

/// Issue #1006 (T5, additions only): sessions of a run under a `usage_tariff` wallet. Every send
/// goes through the wallet as the direct Responses client's does, `BudgetedBody::for_responses`
/// with its endpoint, and is charged from a Responses reply in place of a response; no network.
mod responses_1006 {
    use std::sync::{Arc, Mutex};

    use serde_json::{Value, json};

    use super::super::super::identity::SendPurpose;
    use super::super::super::ledger::{CommandWrites, LedgerEntry};
    use super::super::super::ledger_v2::{LedgerOpenV2, ProviderBilled, UnknownCause};
    use super::super::super::policy_v2::RunBudgetPolicyV2;
    use super::super::super::policy_v2::test_support::{WALLET, policy_v2_value};
    use super::super::super::reservation::{AttemptState, BudgetRefusal};
    use super::super::super::test_support::FakeClock;
    use super::super::super::transport::{self, BudgetedBody, RunBudgetHandle, SandboxView};
    use super::super::{CommandRefusal, refusal};
    use super::{config_on, limit, write};
    use crate::config::Config;
    use crate::minimal_loop::loop_run::{
        RunSessionOptions, RunSessionOutcome, run_session_with_outcome_with_options,
    };
    use crate::providers::{AssistantReply, ChatClient};
    use crate::state::{ConversationMessage, SessionSnapshot, ToolCall};
    use crate::tools::os_sandbox::SandboxPolicy;
    use crate::tools::registry::ToolSpec;
    use crate::tui::NOOP_UI;

    const ENDPOINT: &str = "https://api.example.test/v1/responses";
    const UNIT: u128 = 1_000_000_000_000_000_000;

    /// 100 in, 20 cached, 10 written for 30m and 5 for 12h, 30 out: at 2 / 0.2 / 2.5 / 4 / 8 per
    /// million, 65*2 + 20*0.2 + 10*2.5 + 5*4 + 30*8 = 419 micro-dollars.
    const USAGE: &str = r#"{"input_tokens":100,"input_tokens_details":{"cached_tokens":20,"cache_write_tokens":15,"cache_write_tokens_details":{"ttl_30m_tokens":10,"ttl_12h_tokens":5}},"output_tokens":30,"output_tokens_details":{"reasoning_tokens":10},"total_tokens":130}"#;
    const CHARGED: u128 = 419 * UNIT / 1_000_000;

    fn reply_text(model: Option<&str>, tier: Option<&str>) -> String {
        let mut value = json!({"id": "resp-1006", "output": [], "usage": serde_json::from_str::<Value>(USAGE).unwrap()});
        if let Some(model) = model {
            value["model"] = json!(model);
        }
        if let Some(tier) = tier {
            value["service_tier"] = json!(tier);
        }
        value.to_string()
    }

    fn charged_reply() -> String {
        reply_text(Some("m-1-snapshot-a"), Some("default"))
    }

    struct World {
        _root: tempfile::TempDir,
        workspace: std::path::PathBuf,
        base: std::path::PathBuf,
        handle: Arc<RunBudgetHandle>,
        config: Config,
    }

    impl World {
        fn new(change: impl FnOnce(&mut Value)) -> Self {
            let root = tempfile::tempdir().unwrap();
            let base = std::fs::canonicalize(root.path()).unwrap();
            let workspace = base.join("workspace");
            std::fs::create_dir_all(&workspace).unwrap();
            std::fs::create_dir_all(base.join("run-tmp")).unwrap();
            let mut value = policy_v2_value();
            value["ledger_dir"] = json!(base.join("ledgers").to_str().unwrap());
            value["route"]["endpoint"] = json!(ENDPOINT);
            change(&mut value);
            let handle = Self::handle_of(&base, &workspace, &value, LedgerEntry::Fresh);
            let config = config_on(&workspace, Some(handle.clone()));
            Self {
                _root: root,
                workspace,
                base,
                handle,
                config,
            }
        }

        fn handle_of(
            base: &std::path::Path,
            workspace: &std::path::Path,
            value: &Value,
            entry: LedgerEntry,
        ) -> Arc<RunBudgetHandle> {
            let policy = RunBudgetPolicyV2::decode(value.to_string().as_bytes(), WALLET).unwrap();
            Arc::new(
                RunBudgetHandle::open_responses(LedgerOpenV2 {
                    policy: Arc::new(policy),
                    workspace,
                    explicit_profile: None,
                    entry,
                    clock: FakeClock::at(7_000),
                    command_writes: Some(CommandWrites::of(SandboxPolicy::with_home(
                        workspace,
                        &base.join("run-tmp"),
                        base.join("home"),
                    ))),
                })
                .unwrap()
                .with_sandbox_view(SandboxView {
                    applied_on: Some(workspace.to_path_buf()),
                }),
            )
        }

        fn session(&self, client: &mut Responses) -> anyhow::Result<RunSessionOutcome> {
            run_session_with_outcome_with_options(
                client,
                &mut SessionSnapshot::new(),
                "Repair final acceptance.\n\nRequired final artifacts:\n- target.txt",
                &["target.txt".to_string()],
                &self.config,
                &NOOP_UI,
                RunSessionOptions::final_acceptance_repair(),
            )
        }
    }

    /// A client whose every call is one send of the wallet, charged from `reply`.
    #[derive(Clone)]
    struct Responses {
        handle: Arc<RunBudgetHandle>,
        reply: String,
        answer: Arc<Mutex<Vec<AssistantReply>>>,
    }

    impl Responses {
        fn of(world: &World, reply: String, answers: Vec<AssistantReply>) -> Self {
            Self {
                handle: world.handle.clone(),
                reply,
                answer: Arc::new(Mutex::new(answers)),
            }
        }
    }

    fn body() -> Value {
        json!({
            "model": "m-1",
            "input": [{"role": "user", "content": [{"type": "input_text", "text": "repair"}]}],
            "max_output_tokens": 50,
            "reasoning": {"effort": "high"},
            "store": false,
            "include": ["reasoning.encrypted_content"],
            "service_tier": "default"
        })
    }

    impl ChatClient for Responses {
        fn label(&self) -> &str {
            "responses-1006"
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
            let budgeted = BudgetedBody::for_responses(
                &self.handle,
                &body(),
                SendPurpose::Executor,
                ENDPOINT,
            )?
            .of_session(crate::provider_call::current_final_repair());
            let (_request, send) = budgeted
                .begin(reqwest::Client::new().post(ENDPOINT))
                .map_err(transport::refused)?;
            if let Some(response) = transport::settle_body_identified(&mut Some(send), &self.reply)
                .map_err(transport::refused)?
            {
                crate::provider_call::note_settled_response(response);
            }
            let mut answers = self.answer.lock().unwrap();
            anyhow::ensure!(!answers.is_empty(), "the script ran out");
            Ok(answers.remove(0))
        }
    }

    fn two_writes() -> AssistantReply {
        AssistantReply {
            content: String::new(),
            tool_calls: vec![
                ToolCall::new("Write", json!({"path": "target.txt", "content": "one\n"})),
                ToolCall::new("Write", json!({"path": "other.txt", "content": "two\n"})),
            ],
            prompt_tokens: None,
            completion_tokens: None,
        }
    }

    /// AC3, AC4, AC6: a final repair's reply is charged once by the tariff, nothing is billed,
    /// and the command is a success.
    #[test]
    fn a_final_repair_is_charged_once_by_the_tariff_and_nothing_is_billed() {
        let world = World::new(|_| {});
        let mut client = Responses::of(&world, charged_reply(), vec![two_writes()]);
        world.session(&mut client).unwrap();
        assert_eq!(refusal(&world.config), None);
        let wallet = world.handle.responses_wallet().unwrap();
        assert_eq!(wallet.attempts.len(), 1, "one physical send for two calls");
        let attempt = &wallet.attempts[0];
        assert_eq!(attempt.purpose, SendPurpose::FinalRepair);
        assert_eq!(attempt.state, AttemptState::Settled);
        assert_eq!(attempt.provider_billed, ProviderBilled::Unreported);
        assert_eq!(attempt.charged.unwrap().charged.units(), CHARGED);
        assert_eq!(wallet.spent().unwrap().units(), CHARGED);
        assert!(world.workspace.join("target.txt").exists());
    }

    /// AC6: the calls of one charged reply are submissions of its one send, each once, and the
    /// host's logical counts run apart from the physical one.
    #[test]
    fn the_calls_of_one_charged_reply_are_submissions_of_its_one_send() {
        let world = World::new(|v| v["limits"]["logical_global"] = json!(3));
        let budgeted =
            BudgetedBody::for_responses(&world.handle, &body(), SendPurpose::Executor, ENDPOINT)
                .unwrap();
        let (_request, send) = budgeted
            .begin(reqwest::Client::new().post(ENDPOINT))
            .unwrap();
        let response = transport::settle_body_identified(&mut Some(send), &charged_reply())
            .unwrap()
            .expect("charged");
        for index in 0..2 {
            crate::structured_budget::submission::for_call(&world.config, Some(response), index)
                .unwrap()
                .record(super::super::super::identity::LogicalKind::General)
                .unwrap();
        }
        // The same call again is refused for what it is, with the logical limit not yet reached.
        let again =
            crate::structured_budget::submission::for_call(&world.config, Some(response), 1)
                .unwrap()
                .record(super::super::super::identity::LogicalKind::General)
                .unwrap_err();
        assert_eq!(again, limit(BudgetRefusal::DuplicateSubmission));
        let wallet = world.handle.responses_wallet().unwrap();
        assert_eq!(wallet.physical_used(), 1);
        assert_eq!(wallet.logical_global_used(), 2);
        assert_eq!(wallet.spent().unwrap().units(), CHARGED);
    }

    /// AC5: a reply whose service tier or model is not the route's is not charged: the send is
    /// unknown, its reservation held, and the command is no success, whatever the session did.
    #[test]
    fn a_reply_of_another_tier_or_model_leaves_the_command_no_success() {
        for (name, text) in [
            ("no tier", reply_text(Some("m-1-snapshot-a"), None)),
            ("flex", reply_text(Some("m-1-snapshot-a"), Some("flex"))),
            ("alias", reply_text(Some("m-1"), Some("default"))),
            (
                "prefix",
                reply_text(Some("m-1-snapshot-a-2"), Some("default")),
            ),
        ] {
            let world = World::new(|_| {});
            let mut client = Responses::of(&world, text, vec![write("target.txt", "x\n")]);
            let _ = world.session(&mut client);
            let wallet = world.handle.responses_wallet().unwrap();
            assert_eq!(wallet.attempts[0].state, AttemptState::Unknown, "{name}");
            assert!(
                matches!(
                    wallet.attempts[0].unknown,
                    Some(UnknownCause::Response { .. })
                ),
                "{name}"
            );
            assert_eq!(
                wallet.held().unwrap(),
                wallet.attempts[0].reserved,
                "{name}"
            );
            assert_eq!(
                wallet.logicals.len(),
                0,
                "{name}: no call of it is a submission"
            );
            assert!(
                matches!(
                    refusal(&world.config),
                    Some(CommandRefusal::WalletUnsettled(_) | CommandRefusal::SendRefused(_))
                ),
                "{name}"
            );
        }
    }

    /// AC5: a late valid reply of a send already unknown is charged once, and the latch stays.
    #[test]
    fn a_late_reply_is_charged_once_and_the_command_stays_no_success() {
        let world = World::new(|_| {});
        let budgeted =
            BudgetedBody::for_responses(&world.handle, &body(), SendPurpose::Executor, ENDPOINT)
                .unwrap();
        let (_request, send) = budgeted
            .begin(reqwest::Client::new().post(ENDPOINT))
            .unwrap();
        drop(send);
        let wallet = world.handle.responses_wallet().unwrap();
        let physical = wallet.attempts[0].physical;
        assert_eq!(wallet.attempts[0].state, AttemptState::Unknown);
        world
            .handle
            .settle_response(physical, &charged_reply())
            .unwrap();
        assert!(
            world
                .handle
                .settle_response(physical, &charged_reply())
                .is_err()
        );
        let wallet = world.handle.responses_wallet().unwrap();
        assert_eq!(wallet.spent().unwrap().units(), CHARGED);
        assert!(wallet.attempts[0].charged.unwrap().late);
        assert_eq!(
            refusal(&world.config),
            Some(CommandRefusal::WalletUnsettled(limit(
                BudgetRefusal::Latched
            )))
        );
    }

    /// AC6: what one session spent is spent for the next session and for a resumed run.
    #[test]
    fn another_session_or_a_resume_gets_no_budget_back() {
        let world = World::new(|_| {});
        let mut client = Responses::of(&world, charged_reply(), vec![write("target.txt", "x\n")]);
        world.session(&mut client).unwrap();
        let spent = world.handle.responses_wallet().unwrap().spent().unwrap();
        let mut again = Responses::of(&world, charged_reply(), vec![write("target.txt", "y\n")]);
        world.session(&mut again).unwrap();
        let wallet = world.handle.responses_wallet().unwrap();
        assert_eq!(wallet.spent().unwrap().units(), 2 * spent.units());
        assert_eq!(wallet.attempts.len(), 2);
        // A resume of the same wallet, in a new process.
        let mut value = policy_v2_value();
        value["ledger_dir"] = json!(world.base.join("ledgers").to_str().unwrap());
        value["route"]["endpoint"] = json!(ENDPOINT);
        let World {
            _root: root,
            workspace,
            base,
            handle,
            config,
        } = world;
        drop((client, again, handle, config));
        let resumed = World::handle_of(&base, &workspace, &value, LedgerEntry::Resume);
        assert_eq!(
            resumed.responses_wallet().unwrap().spent().unwrap().units(),
            2 * spent.units()
        );
        drop(root);
    }
}
