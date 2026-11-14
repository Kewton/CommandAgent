//! Issue #1006, T3: the direct OpenAI Responses client under a `usage_tariff` wallet, against a
//! fake on 127.0.0.1 that reads the ledger at the moment each POST arrives.

use std::io::{Read as _, Write as _};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use super::*;
use crate::planner::structured_stages::receipt::sha256_hex;
use crate::structured_budget::ledger::{CommandWrites, LedgerEntry};
use crate::structured_budget::ledger_v2::{LedgerOpenV2, UnknownCause};
use crate::structured_budget::policy_v2::RunBudgetPolicyV2;
use crate::structured_budget::policy_v2::test_support::{WALLET, policy_v2_value};
use crate::structured_budget::reservation::AttemptState;
use crate::structured_budget::settlement::UnknownReason;
use crate::structured_budget::test_support::FakeClock;
use crate::structured_budget::transport::SandboxView;
use crate::tools::os_sandbox::SandboxPolicy;

/// What the base ff920681 sent for [`off_messages`] with effort `high` and 128 tokens of output,
/// captured from that code by a fake (`results/L-196/T3/off-capture.txt`), not made by this one.
const OFF_BODY_AT_BASE: &str = r#"{"include":["reasoning.encrypted_content"],"input":[{"content":[{"text":"be brief","type":"input_text"}],"role":"system"},{"content":[{"text":"hello","type":"input_text"}],"role":"user"}],"max_output_tokens":128,"model":"m-1","reasoning":{"effort":"high"},"store":false}"#;
const OFF_LINE_AT_BASE: &str = "POST /v1/responses HTTP/1.1";

fn off_messages() -> Vec<ConversationMessage> {
    vec![
        ConversationMessage::system("be brief"),
        ConversationMessage::user("hello"),
    ]
}

/// A reply of a declared model at the default tier: 10 in, 5 out, writes reported at zero.
const CHARGED: &str = r#"{"id":"resp_1","model":"m-1-snapshot-a","service_tier":"default","output":[{"type":"message","content":[{"type":"output_text","text":"ok"}]}],"usage":{"input_tokens":10,"input_tokens_details":{"cached_tokens":0,"cache_write_tokens":0,"cache_write_tokens_details":{"ttl_30m_tokens":0,"ttl_12h_tokens":0}},"output_tokens":5,"total_tokens":15}}"#;

/// What the fake does with one POST.
#[derive(Debug, Clone)]
enum Reply {
    Status(u16, String),
    /// A body shorter than its Content-Length, then the connection closed.
    Truncated,
    /// No answer for longer than the client waits.
    Silent(Duration),
    /// A redirect to `/elsewhere` on the same fake.
    Redirect,
    /// The client's token is cancelled while the send is out, then nothing is answered.
    CancelThenSilent(CancelToken),
}

/// One POST as it arrived, and the ledger as it was then.
#[derive(Debug)]
struct Received {
    line: String,
    body: String,
    /// The last attempt on disk when the POST arrived, if a wallet is watched.
    attempt: Option<Value>,
}

fn read_request(stream: &mut TcpStream) -> Option<(String, String)> {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 4096];
    loop {
        let read = stream.read(&mut chunk).ok()?;
        if read == 0 {
            return None;
        }
        buffer.extend_from_slice(&chunk[..read]);
        if let Some(end) = buffer.windows(4).position(|w| w == b"\r\n\r\n") {
            let head = String::from_utf8_lossy(&buffer[..end]).to_string();
            let length = head
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().unwrap())
                })
                .unwrap_or(0);
            while buffer.len() < end + 4 + length {
                let read = stream.read(&mut chunk).ok()?;
                if read == 0 {
                    return None;
                }
                buffer.extend_from_slice(&chunk[..read]);
            }
            let line = head.lines().next().unwrap_or_default().to_string();
            let body = String::from_utf8_lossy(&buffer[end + 4..end + 4 + length]).to_string();
            return Some((line, body));
        }
    }
}

fn last_attempt(ledger_file: &Path) -> Option<Value> {
    let bytes = std::fs::read(ledger_file).ok()?;
    let envelope: Value = serde_json::from_slice(&bytes).ok()?;
    envelope["body"]["attempts"].as_array()?.last().cloned()
}

struct Fake {
    server: std::thread::JoinHandle<Vec<Received>>,
}

impl Fake {
    /// A port to serve on, taken before anything that has to know it.
    fn bind() -> TcpListener {
        TcpListener::bind("127.0.0.1:0").unwrap()
    }

    /// Answers each POST in turn with `replies`, reading `ledger_file` as each arrives, and stops
    /// once nobody connects for a while.
    fn serve(listener: TcpListener, replies: Vec<Reply>, ledger_file: Option<PathBuf>) -> Self {
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let mut received = Vec::new();
            let mut replies = replies.into_iter();
            let mut idle = Duration::ZERO;
            while idle < Duration::from_millis(2_500) {
                let (mut stream, _) = match listener.accept() {
                    Ok(accepted) => accepted,
                    Err(_) => {
                        std::thread::sleep(Duration::from_millis(10));
                        idle += Duration::from_millis(10);
                        continue;
                    }
                };
                idle = Duration::ZERO;
                stream.set_nonblocking(false).unwrap();
                let Some((line, body)) = read_request(&mut stream) else {
                    continue;
                };
                let attempt = ledger_file.as_deref().and_then(last_attempt);
                received.push(Received {
                    line,
                    body,
                    attempt,
                });
                let Some(reply) = replies.next() else {
                    let _ = write!(
                        stream,
                        "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    );
                    continue;
                };
                match reply {
                    Reply::Status(status, body) => {
                        let _ = write!(
                            stream,
                            "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        );
                    }
                    Reply::Truncated => {
                        let _ = write!(
                            stream,
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 400\r\nConnection: close\r\n\r\n{{\"id\":\"resp_1\""
                        );
                    }
                    Reply::Silent(wait) => std::thread::sleep(wait),
                    Reply::Redirect => {
                        let _ = write!(
                            stream,
                            "HTTP/1.1 307 Temporary Redirect\r\nLocation: http://{address}/elsewhere\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                        );
                    }
                    Reply::CancelThenSilent(token) => {
                        token.cancel();
                        std::thread::sleep(Duration::from_millis(300));
                    }
                }
            }
            received
        });
        Self { server }
    }

    fn finish(self) -> Vec<Received> {
        self.server.join().unwrap()
    }
}

/// A `usage_tariff` wallet whose policy's endpoint is the fake's. The decode allows only https;
/// only a test sets a plain one, on the decoded value.
struct World {
    _root: tempfile::TempDir,
    workspace: PathBuf,
    ledger_file: PathBuf,
    wallet_dir: PathBuf,
    handle: Arc<RunBudgetHandle>,
}

impl World {
    fn new(endpoint: &str, change: impl FnOnce(&mut Value)) -> Self {
        let root = tempfile::tempdir().unwrap();
        let base = std::fs::canonicalize(root.path()).unwrap();
        let workspace = base.join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        std::fs::create_dir_all(base.join("run-tmp")).unwrap();
        let ledgers = base.join("ledgers");
        let mut value = policy_v2_value();
        value["ledger_dir"] = json!(ledgers.to_str().unwrap());
        change(&mut value);
        let mut policy = RunBudgetPolicyV2::decode(value.to_string().as_bytes(), WALLET).unwrap();
        policy.route.endpoint = endpoint.to_string();
        let handle = RunBudgetHandle::open_responses(LedgerOpenV2 {
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
        });
        Self {
            _root: root,
            workspace,
            ledger_file: ledgers.join(WALLET).join("ledger.json"),
            wallet_dir: ledgers.join(WALLET),
            handle: Arc::new(handle),
        }
    }

    /// The direct client as the factory makes it for a wallet, at the fake, with `retries`.
    fn client(&self, base_url: &str, retries: usize) -> OpenAiClient {
        let mut client = OpenAiClient::for_test_responses("sk-test-not-real", base_url, None);
        client.retries = retries;
        client.reasoning_effort = Some("high".to_string());
        client.under_run_budget(&self.handle, 1).unwrap()
    }

    fn attempts(&self) -> Vec<crate::structured_budget::ledger_v2::AttemptRecordV2> {
        self.handle.responses_wallet().unwrap().attempts
    }
}

fn say_hello(client: &mut OpenAiClient) -> anyhow::Result<AssistantReply> {
    client.dispatch("m-1", &off_messages(), &[], false)
}

/// A wallet whose policy's endpoint is a fake's, the fake answering `replies`, and a client of
/// `retries` at it.
fn wallet_and_fake(
    replies: impl FnOnce(&OpenAiClient) -> Vec<Reply>,
    retries: usize,
    change: impl FnOnce(&mut Value),
) -> (World, OpenAiClient, Fake) {
    let listener = Fake::bind();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    let world = World::new(&format!("{base_url}/v1/responses"), change);
    let client = world.client(&base_url, retries);
    let fake = Fake::serve(listener, replies(&client), Some(world.ledger_file.clone()));
    (world, client, fake)
}

fn the_started_send_is_on_disk_for(received: &Received) {
    let attempt = received.attempt.as_ref().expect("a ledger with an attempt");
    assert_eq!(attempt["state"], json!("started"), "{attempt}");
    assert_eq!(
        attempt["request_sha256"],
        json!(sha256_hex(received.body.as_bytes())),
        "the digest on disk is of the bytes that arrived"
    );
    let reserved: u128 = attempt["reserved"].as_str().unwrap().parse().unwrap();
    assert!(reserved > 0);
}

#[test]
fn every_post_arrives_with_its_reservation_and_start_on_disk() {
    let (world, mut client, fake) = wallet_and_fake(
        |_| {
            vec![
                Reply::Status(200, CHARGED.into()),
                Reply::Status(200, CHARGED.into()),
            ]
        },
        2,
        |_| {},
    );
    assert_eq!(say_hello(&mut client).unwrap().content, "ok");
    assert_eq!(say_hello(&mut client).unwrap().content, "ok");
    let received = fake.finish();
    assert_eq!(received.len(), 2);
    for one in &received {
        assert_eq!(one.line, OFF_LINE_AT_BASE);
        the_started_send_is_on_disk_for(one);
    }
    let attempts = world.attempts();
    assert_eq!(attempts.len(), 2);
    assert!(attempts.iter().all(|a| a.state == AttemptState::Settled));
    assert_eq!(world.handle.success_gate(), Ok(()));
}

#[test]
fn a_wallet_request_carries_the_default_tier_and_the_policy_s_output_and_off_is_as_the_base_sent() {
    // The wallet's: service_tier and the output clamped to the policy's 50 before the send.
    let (_world, mut client, fake) =
        wallet_and_fake(|_| vec![Reply::Status(200, CHARGED.into())], 0, |_| {});
    say_hello(&mut client).unwrap();
    let received = fake.finish();
    let body: Value = serde_json::from_str(&received[0].body).unwrap();
    assert_eq!(body["service_tier"], json!("default"));
    assert_eq!(body["store"], json!(false));
    assert_eq!(body["include"], json!(["reasoning.encrypted_content"]));
    assert_eq!(body["max_output_tokens"], json!(50));
    let mut base: Value = serde_json::from_str(OFF_BODY_AT_BASE).unwrap();
    base["service_tier"] = json!("default");
    base["max_output_tokens"] = json!(50);
    assert_eq!(
        body, base,
        "only the wallet's two controls differ from the base"
    );

    // Off: no wallet, the bytes and the retry the base made, byte for byte.
    let listener = Fake::bind();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    let fake = Fake::serve(
        listener,
        vec![
            Reply::Status(500, "{}".into()),
            Reply::Status(200, r#"{"id":"resp_1","model":"m-1","output":[{"type":"message","content":[{"type":"output_text","text":"ok"}]}],"usage":{"input_tokens":5,"output_tokens":1}}"#.into()),
        ],
        None,
    );
    let mut client = OpenAiClient::for_test_responses("sk-test-not-real", base_url, None);
    client.retries = 1;
    client.reasoning_effort = Some("high".to_string());
    assert_eq!(say_hello(&mut client).unwrap().content, "ok");
    let received = fake.finish();
    assert_eq!(received.len(), 2);
    for one in &received {
        assert_eq!(one.line, OFF_LINE_AT_BASE);
        assert_eq!(one.body, OFF_BODY_AT_BASE);
    }
}

#[test]
fn a_refused_reservation_or_a_ledger_that_cannot_be_written_sends_nothing() {
    // The money limit below the reservation.
    let (world, mut client, fake) = wallet_and_fake(
        |_| vec![Reply::Status(200, CHARGED.into())],
        3,
        |v| v["limits"]["cost_usd"] = json!("0.000000000000000001"),
    );
    assert!(say_hello(&mut client).is_err());
    assert_eq!(fake.finish().len(), 0);
    assert!(world.attempts().is_empty());

    // The ledger cannot take the reservation: no start, no permit, no POST, and nothing after.
    let (world, mut client, fake) =
        wallet_and_fake(|_| vec![Reply::Status(200, CHARGED.into())], 3, |_| {});
    std::fs::create_dir(world.wallet_dir.join("ledger.json.tmp")).unwrap();
    assert!(say_hello(&mut client).is_err());
    std::fs::remove_dir(world.wallet_dir.join("ledger.json.tmp")).unwrap();
    assert!(say_hello(&mut client).is_err());
    assert_eq!(fake.finish().len(), 0);
    assert!(world.attempts().is_empty());
    assert!(world.handle.success_gate().is_err());
}

#[test]
fn a_send_that_ends_unknown_is_never_followed_by_another_post() {
    let rows: Vec<(&str, Reply, UnknownReason)> = vec![
        (
            "http error",
            Reply::Status(500, "{}".into()),
            UnknownReason::HttpError,
        ),
        (
            "rate limit",
            Reply::Status(429, "{}".into()),
            UnknownReason::HttpError,
        ),
        ("redirect", Reply::Redirect, UnknownReason::HttpError),
        ("truncated", Reply::Truncated, UnknownReason::BodyTruncated),
        (
            "timeout",
            Reply::Silent(Duration::from_millis(1_800)),
            UnknownReason::Timeout,
        ),
    ];
    for (name, reply, reason) in rows {
        let (world, mut client, fake) = wallet_and_fake(
            |_| vec![reply, Reply::Status(200, CHARGED.into())],
            3,
            |_| {},
        );
        assert!(say_hello(&mut client).is_err(), "{name}");
        let received = fake.finish();
        assert_eq!(received.len(), 1, "{name}: one POST, retries left or not");
        assert!(
            received.iter().all(|r| r.line == OFF_LINE_AT_BASE),
            "{name}: no redirect followed"
        );
        the_started_send_is_on_disk_for(&received[0]);
        let attempts = world.attempts();
        assert_eq!(attempts.len(), 1, "{name}");
        assert_eq!(attempts[0].state, AttemptState::Unknown, "{name}");
        assert_eq!(
            attempts[0].unknown,
            Some(UnknownCause::Transport { reason }),
            "{name}"
        );
        // Held, and the run is no success.
        assert_eq!(
            world.handle.responses_wallet().unwrap().held().unwrap(),
            attempts[0].reserved,
            "{name}"
        );
        assert!(world.handle.success_gate().is_err(), "{name}");
    }
}

#[test]
fn a_reply_whose_usage_is_unknown_stops_the_wallet_before_anything_reads_it() {
    // A reply with no usage: the reply itself is fine to read, and the wallet is stopped first.
    let reply = r#"{"id":"resp_1","model":"m-1-snapshot-a","service_tier":"default","output":[{"type":"message","content":[{"type":"output_text","text":"ok"}]}]}"#;
    let (world, mut client, fake) = wallet_and_fake(
        |_| {
            vec![
                Reply::Status(200, reply.into()),
                Reply::Status(200, CHARGED.into()),
            ]
        },
        3,
        |_| {},
    );
    assert_eq!(say_hello(&mut client).unwrap().content, "ok");
    assert!(say_hello(&mut client).is_err());
    assert_eq!(fake.finish().len(), 1);
    assert_eq!(world.attempts()[0].state, AttemptState::Unknown);
    // A reply the parser cannot take is still charged: its usage was read first.
    let broken = CHARGED.replace(
        r#""output":[{"type":"message""#,
        r#""output":[{"type":"function_call","call_id":"c","arguments":"{}"},{"type":"message""#,
    );
    let (world, mut client, fake) =
        wallet_and_fake(|_| vec![Reply::Status(200, broken.clone())], 0, |_| {});
    assert!(
        say_hello(&mut client).is_err(),
        "a function_call with no name does not parse"
    );
    assert_eq!(fake.finish().len(), 1);
    assert_eq!(world.attempts()[0].state, AttemptState::Settled);
}

#[test]
fn a_cancel_before_the_send_takes_nothing_and_one_after_it_keeps_the_reservation() {
    // Before: the token is cancelled when the call begins.
    let (world, mut client, fake) =
        wallet_and_fake(|_| vec![Reply::Status(200, CHARGED.into())], 3, |_| {});
    let token = CancelToken::default();
    token.cancel();
    client.set_cancel_token(token);
    assert!(say_hello(&mut client).is_err());
    assert_eq!(fake.finish().len(), 0);
    assert!(world.attempts().is_empty());
    assert_eq!(world.handle.success_gate(), Ok(()));

    // After: the fake cancels once the POST has arrived.
    let token = CancelToken::default();
    let watched = token.clone();
    let (world, mut client, fake) =
        wallet_and_fake(move |_| vec![Reply::CancelThenSilent(watched)], 3, |_| {});
    client.set_cancel_token(token);
    assert!(say_hello(&mut client).is_err());
    let received = fake.finish();
    assert_eq!(received.len(), 1);
    the_started_send_is_on_disk_for(&received[0]);
    let attempts = world.attempts();
    assert_eq!(attempts[0].state, AttemptState::Unknown);
    assert_eq!(
        attempts[0].unknown,
        Some(UnknownCause::Transport {
            reason: UnknownReason::Cancelled
        })
    );
    assert_eq!(
        world.handle.responses_wallet().unwrap().held().unwrap(),
        attempts[0].reserved
    );
}

#[test]
fn a_wallet_whose_endpoint_is_not_where_the_client_posts_sends_nothing() {
    // The client posts to the fake; the policy names another endpoint.
    let listener = Fake::bind();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    let world = World::new("https://api.example.test/v1/responses", |_| {});
    let mut client = world.client(&base_url, 3);
    let fake = Fake::serve(listener, vec![Reply::Status(200, CHARGED.into())], None);
    assert!(say_hello(&mut client).is_err());
    assert_eq!(fake.finish().len(), 0);
    assert!(world.attempts().is_empty());
    // And a Chat Completions request of the same client is not this wallet's either.
    let (world, client, fake) =
        wallet_and_fake(|_| vec![Reply::Status(200, CHARGED.into())], 3, |_| {});
    let mut chat = client.clone();
    chat.api = OpenAiApi::ChatCompletions;
    assert!(say_hello(&mut chat).is_err());
    assert_eq!(fake.finish().len(), 0);
    assert!(world.attempts().is_empty());
    let _ = world.workspace;
}

/// The factory, in a child whose environment holds a key and the selection.
mod factory {
    use super::*;
    use crate::config::Config;

    const CHILD: &str = "COMMANDAGENT_TEST_OPENAI_V2_FACTORY_CHILD";
    const RAN: &str = "openai-v2-factory-child-ran";

    fn config(root: &Path) -> Config {
        let (cwd, state) = (root.join("workspace"), root.join("state"));
        std::fs::create_dir_all(&cwd).unwrap();
        let arguments = [
            "commandagent",
            "--cwd",
            cwd.to_str().unwrap(),
            "--state-dir",
            state.to_str().unwrap(),
            "--model",
            "m-1",
            "--provider",
            "openai-compatible",
            "--base-url",
            "http://127.0.0.1:9/v1",
        ];
        let parsed =
            crate::provider_cli::parse_from(arguments.iter().map(ToString::to_string)).unwrap();
        Config::from_cli_with_provider_options(parsed.cli, parsed.provider_options).unwrap()
    }

    fn direct(config: &mut Config) {
        config.openai_compatible = None;
        config.provider = Provider::Openai;
        config.planner_provider = Provider::Openai;
        config.classifier_provider = Provider::Openai;
        config.openai_api = OpenAiApi::Responses;
    }

    #[test]
    fn the_factory_builds_the_direct_client_with_the_wallet_and_nothing_else_child() {
        if std::env::var_os(CHILD).is_none() {
            return;
        }
        let root = tempfile::tempdir().unwrap();
        let base = std::fs::canonicalize(root.path()).unwrap();
        let world = World::new("https://api.openai.com/v1/responses", |_| {});
        // A compatible endpoint, under a v2 wallet: refused before a client exists.
        let mut compatible = config(&base);
        compatible.openai_api = OpenAiApi::Responses;
        compatible.field_sources.run_budget = Some(world.handle.clone());
        let error =
            crate::providers::client_from_config_for_role(&compatible, ProviderRole::Executor)
                .map(|_| ())
                .unwrap_err();
        assert_eq!(
            format!("{error:#}"),
            "the run budget refused the client: RouteNotCountable"
        );
        // Direct, under it: the OpenAI client, built.
        let mut config = config(&base);
        direct(&mut config);
        config.field_sources.run_budget = Some(world.handle.clone());
        for role in [
            ProviderRole::Executor,
            ProviderRole::Planner,
            ProviderRole::Classifier,
        ] {
            let client = crate::providers::client_from_config_for_role(&config, role).unwrap();
            assert_eq!(client.label(), "openai", "{role:?}");
        }
        // Direct Chat Completions under it: refused.
        let mut chat = config.clone();
        chat.openai_api = OpenAiApi::ChatCompletions;
        assert!(
            crate::providers::client_from_config_for_role(&chat, ProviderRole::Executor).is_err()
        );
        // The client the factory builds for it carries the wallet; one built with none does not.
        let hooked =
            OpenAiClient::from_env_under_run_budget(&config, ProviderRole::Planner, &world.handle)
                .unwrap();
        assert!(
            hooked
                .run_budget
                .as_ref()
                .is_some_and(|held| Arc::ptr_eq(held, &world.handle))
        );
        assert_eq!(hooked.provider_role, ProviderRole::Planner);
        assert!(hooked.max_predict <= 50);
        let mut off = config.clone();
        off.field_sources.run_budget = None;
        assert!(OpenAiClient::from_env(&off).unwrap().run_budget.is_none());
        println!("{RAN}");
    }

    #[test]
    fn the_factory_builds_the_direct_client_with_the_wallet_and_nothing_else() {
        let home = tempfile::tempdir().unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "providers::openai::run_budget_tests::factory::the_factory_builds_the_direct_client_with_the_wallet_and_nothing_else_child",
                "--nocapture",
                "--test-threads=1",
            ])
            .env_clear()
            .env("HOME", home.path())
            .env(CHILD, "1")
            .env("OPENAI_API_KEY", "sk-test-not-real")
            .env(crate::structured_budget::admission::RUN_BUDGET_ENV, WALLET)
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{stdout}\n{stderr}");
        assert!(stdout.contains(RAN), "{stdout}\n{stderr}");
    }
}
