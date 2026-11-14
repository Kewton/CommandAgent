//! Issue #1006: a run under a `usage_tariff` wallet, through the binary as an operator starts it.
//!
//! The direct OpenAI client posts only to `https://api.openai.com`, and no production path lets
//! a test point it elsewhere. So every run here has its proxy variables set to a fake on
//! 127.0.0.1 that sees each attempt as a `CONNECT` and never lets it reach the network: what it
//! counts is what the wallet let out. The fake reads the ledger on disk at the moment each
//! attempt arrives. A reply of the provider cannot be faked over TLS here, so what a charged
//! reply does is tested in the crate (`structured_budget::{responses,outcome_tests}`,
//! `providers::openai::run_budget_tests`); this file is the run path up to the send, the send
//! that fails, the process that ends, and the wallet's files.
//!
//! The per-call cache-write events (AC8) are tested here with the wallet off, on the compatible
//! clients, whose base URL a test may set.
//!
//! All prices are synthetic.

#![cfg(unix)]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

const UNIT: u128 = 1_000_000_000_000_000_000;
const KEY: &str = "sk-test-not-a-real-key-1006";
const ENDPOINT: &str = "https://api.openai.com/v1/responses";

/// What the fake proxy does with an attempt.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Proxy {
    /// Closes the connection: the send fails.
    Close,
    /// Keeps it open and answers nothing, until the test is done.
    Hold,
}

/// One attempt as it arrived: its first line and the wallet's last attempt on disk then.
#[derive(Debug, Clone)]
struct Arrival {
    line: String,
    attempt: Option<Value>,
}

struct Fake {
    url: String,
    stop: Arc<AtomicBool>,
    thread: JoinHandle<()>,
    seen: Arc<Mutex<Vec<Arrival>>>,
}

fn read_head(stream: &mut TcpStream) -> Vec<u8> {
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut data = Vec::new();
    let mut buffer = [0_u8; 8192];
    while !data.windows(4).any(|window| window == b"\r\n\r\n") {
        match stream.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(read) => data.extend_from_slice(&buffer[..read]),
        }
    }
    data
}

fn read_request(stream: &mut TcpStream) -> (String, String) {
    let mut data = read_head(stream);
    let end = data
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .unwrap_or(data.len());
    let head = String::from_utf8_lossy(&data[..end]).to_string();
    let length = head
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().unwrap_or(0))
        })
        .unwrap_or(0);
    let mut buffer = [0_u8; 8192];
    while data.len() < end + 4 + length {
        match stream.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(read) => data.extend_from_slice(&buffer[..read]),
        }
    }
    let body = String::from_utf8_lossy(&data[(end + 4).min(data.len())..]).to_string();
    (head.lines().next().unwrap_or("").to_string(), body)
}

impl Fake {
    /// A proxy that counts each `CONNECT` and reads `ledger` as it arrives.
    fn proxy(mode: Proxy, ledger: Option<PathBuf>) -> Self {
        Self::serve(move |stream, seen| {
            let head = read_head(stream);
            let line = String::from_utf8_lossy(&head)
                .lines()
                .next()
                .unwrap_or("")
                .to_string();
            let attempt = ledger.as_deref().and_then(last_attempt);
            seen.lock().unwrap().push(Arrival { line, attempt });
            mode == Proxy::Hold
        })
    }

    /// A compatible provider that answers every request with `reply`.
    fn provider(reply: String) -> Self {
        Self::serve(move |stream, seen| {
            let (line, _body) = read_request(stream);
            seen.lock().unwrap().push(Arrival {
                line,
                attempt: None,
            });
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}",
                reply.len()
            );
            let _ = stream.write_all(response.as_bytes());
            false
        })
    }

    /// `handle` returns whether to keep the connection open.
    fn serve(
        handle: impl Fn(&mut TcpStream, &Mutex<Vec<Arrival>>) -> bool + Send + 'static,
    ) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        listener.set_nonblocking(true).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let seen = Arc::new(Mutex::new(Vec::new()));
        let (thread_stop, thread_seen) = (stop.clone(), seen.clone());
        let thread = std::thread::spawn(move || {
            let mut held = Vec::new();
            while !thread_stop.load(Ordering::SeqCst) {
                let Ok((mut stream, _)) = listener.accept() else {
                    std::thread::sleep(Duration::from_millis(5));
                    continue;
                };
                stream.set_nonblocking(false).unwrap();
                if handle(&mut stream, &thread_seen) {
                    held.push(stream);
                }
            }
        });
        Self {
            url,
            stop,
            thread,
            seen,
        }
    }

    fn count(&self) -> usize {
        self.seen.lock().unwrap().len()
    }

    fn finish(self) -> Vec<Arrival> {
        self.stop.store(true, Ordering::SeqCst);
        self.thread.join().unwrap();
        self.seen.lock().unwrap().clone()
    }
}

fn ledger_body(path: &Path) -> Option<Value> {
    let bytes = std::fs::read(path).ok()?;
    let envelope: Value = serde_json::from_slice(&bytes).ok()?;
    Some(envelope["body"].clone())
}

fn last_attempt(path: &Path) -> Option<Value> {
    ledger_body(path)?["attempts"].as_array()?.last().cloned()
}

/// What one start of the binary did.
#[derive(Debug)]
struct Ran {
    code: Option<i32>,
    error: String,
    arrivals: Vec<Arrival>,
}

/// The directories of one operator and one workspace, kept apart: the declarations and the
/// ledgers are out of the reach of the run's commands.
struct World {
    _root: tempfile::TempDir,
    base: PathBuf,
}

/// A v2 declaration of `id`, writes priced by TTL, one tier unless changed.
fn v2_policy(id: &str, ledgers: &Path) -> Value {
    json!({
        "protocol": "run-budget.policy/v2",
        "accounting_basis": "usage_tariff",
        "budget_id": id,
        "limits": {
            "cost_usd": "1000",
            "physical_sends": 100,
            "logical_global": 100,
            "logical_per_stage": 100,
            "tokens": 100_000_000,
            "wall_seconds": 10_000,
            "final_repair_attempts": 10,
            "final_repair_seconds": 10_000
        },
        "route": {
            "api": "responses",
            "endpoint": ENDPOINT,
            "model": "m-1",
            "response_models": ["m-1-snapshot"],
            "effort": "high",
            "service_tier": "default"
        },
        "tariff": {
            "revision": "synthetic-1006",
            "price_conditions": {"region": "global", "regional_surcharge": "none"},
            "cache_write_mode": "ttl_30m_12h",
            "tiers": [{
                "above_input_tokens": 0,
                "input_per_million": "2",
                "cached_input_per_million": "0.2",
                "cache_write_30m_per_million": "2.5",
                "cache_write_12h_per_million": "4",
                "output_per_million": "8",
                "request_fee": "0"
            }]
        },
        "input_bound": {
            "method": "body_bytes_plus_overhead_or_provider_max",
            "input_overhead": 4,
            "provider_max_input_tokens": 1_050_000
        },
        "max_output_tokens": 500,
        "max_input_tokens": 2_000_000,
        "ledger_dir": ledgers.to_str().unwrap()
    })
}

/// A v1 declaration of `id` in the same ledger directory.
fn v1_policy(id: &str, ledgers: &Path) -> Value {
    json!({
        "protocol": "run-budget.policy/v1",
        "budget_id": id,
        "limits": {
            "cost_usd": "1000",
            "physical_sends": 100,
            "logical_global": 100,
            "logical_per_stage": 100,
            "tokens": 100_000_000,
            "wall_seconds": 10_000,
            "final_repair_attempts": 10,
            "final_repair_seconds": 10_000
        },
        "tariff": {
            "revision": "synthetic-976",
            "input_per_million": "2",
            "cached_input_per_million": "1",
            "cache_write_per_million": "2",
            "output_per_million": "10",
            "request_fee": "0"
        },
        "route": {
            "api": "chat_completions",
            "model": "m-1",
            "upstream_only": ["u"],
            "allow_fallbacks": false,
            "effort": "low",
            "reasoning_in_output": true
        },
        "max_output_tokens": 500,
        "max_input_tokens": 2_000_000,
        "input_overhead": 4,
        "ledger_dir": ledgers.to_str().unwrap()
    })
}

impl World {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let base = std::fs::canonicalize(root.path()).unwrap();
        for directory in [
            "workspace",
            "state",
            "home",
            "operator/run-budgets",
            "ledgers",
        ] {
            std::fs::create_dir_all(base.join(directory)).unwrap();
        }
        Self { _root: root, base }
    }

    fn ledgers(&self) -> PathBuf {
        self.base.join("ledgers")
    }

    fn ledger(&self, id: &str) -> PathBuf {
        self.ledgers().join(id).join("ledger.json")
    }

    fn declare(&self, id: &str, policy: &Value) {
        std::fs::write(
            self.base.join(format!("operator/run-budgets/{id}.json")),
            policy.to_string(),
        )
        .unwrap();
    }

    fn v2(&self, id: &str, change: impl FnOnce(&mut Value)) {
        let mut policy = v2_policy(id, &self.ledgers());
        change(&mut policy);
        self.declare(id, &policy);
    }

    /// The binary through `main`, its whole environment `env` beside HOME, PATH, the key, the
    /// effort, the operator root and the proxy, against the direct Responses route unless
    /// `arguments` say otherwise. A `Hold` proxy's first attempt stops the child there.
    fn run(
        &self,
        wallet: Option<&str>,
        proxy: Proxy,
        env: &[(&str, &str)],
        arguments: &[&str],
    ) -> Ran {
        let fake = Fake::proxy(proxy, wallet.map(|id| self.ledger(id)));
        let mut command = Command::new(env!("CARGO_BIN_EXE_commandagent"));
        command
            .env_clear()
            .env("HOME", self.base.join("home"))
            .env("PATH", "/usr/bin:/bin")
            .env(
                "COMMANDAGENT_ADAPTER_CONFIG_ROOT",
                self.base.join("operator"),
            )
            .env("OPENAI_API_KEY", KEY)
            .env("COMMANDAGENT_OPENAI_REASONING_EFFORT", "high")
            .env("HTTPS_PROXY", &fake.url)
            .env("HTTP_PROXY", &fake.url)
            .env("ALL_PROXY", &fake.url)
            // The compatible fake is on 127.0.0.1 itself and is reached directly.
            .env("NO_PROXY", "127.0.0.1,localhost")
            .env("COMMANDAGENT_EVAL_EVENTS", self.base.join("events.jsonl"));
        if let Some(id) = wallet {
            command.env("COMMANDAGENT_RUN_BUDGET", id);
        }
        command.envs(env.iter().copied());
        let mut all = vec![
            "--cwd".to_string(),
            self.base.join("workspace").to_str().unwrap().to_string(),
            "--state-dir".to_string(),
            self.base.join("state").to_str().unwrap().to_string(),
        ];
        let defaults = [
            "--provider",
            "openai",
            "--api",
            "responses",
            "--model",
            "m-1",
            "--chat-retries",
            "3",
        ];
        if !arguments.contains(&"--provider") {
            all.extend(defaults.iter().map(ToString::to_string));
        }
        all.extend(arguments.iter().map(ToString::to_string));
        all.extend(["--prompt".to_string(), "say hi".to_string()]);
        let mut child = command
            .args(&all)
            .current_dir(self.base.join("workspace"))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(120);
        let code = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status.code();
            }
            if proxy == Proxy::Hold && fake.count() > 0 {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
            assert!(Instant::now() < deadline, "neither an exit nor an attempt");
            std::thread::sleep(Duration::from_millis(10));
        };
        let mut stderr = String::new();
        child
            .stderr
            .take()
            .unwrap()
            .read_to_string(&mut stderr)
            .unwrap();
        let error = stderr
            .lines()
            .rfind(|line| line.starts_with("error"))
            .unwrap_or("")
            .to_string();
        Ran {
            code,
            error,
            arrivals: fake.finish(),
        }
    }

    fn direct(&self, id: &str) -> Ran {
        self.run(Some(id), Proxy::Close, &[], &[])
    }

    fn events(&self) -> Vec<Value> {
        std::fs::read_to_string(self.base.join("events.jsonl"))
            .unwrap_or_default()
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect()
    }
}

/// The reservation the oracle gives: the bound at the highest input rate of the tiers it can
/// reach, the output at the highest output rate, per million, rounded up once.
fn oracle_reserved(
    input_bound: u64,
    output_bound: u64,
    input_rate: u128,
    output_rate: u128,
) -> u128 {
    (u128::from(input_bound) * input_rate * UNIT + u128::from(output_bound) * output_rate * UNIT)
        .div_ceil(1_000_000)
}

fn units(attempt: &Value, key: &str) -> u128 {
    attempt[key].as_str().unwrap().parse().unwrap()
}

fn decimal(units: u128) -> String {
    format!("{}.{:018}", units / UNIT, units % UNIT)
}

const SEND_REFUSED: &str = "error: the run budget refused the send: Wallet(Budget(";

// ---- AC1: the route and what is refused before a send ----

#[test]
fn ac1_the_direct_route_reserves_its_text_bound_on_the_one_wallet() {
    let world = World::new();
    world.v2("w-ac1", |_| {});
    let ran = world.direct("w-ac1");
    assert_eq!(ran.arrivals.len(), 1, "{ran:?}");
    assert_eq!(ran.arrivals[0].line, "CONNECT api.openai.com:443 HTTP/1.1");
    assert_ne!(ran.code, Some(0));
    let ledger = ledger_body(&world.ledger("w-ac1")).unwrap();
    assert_eq!(ledger["schema"], json!("run-budget.ledger/v2"));
    assert_eq!(ledger["accounting_basis"], json!("usage_tariff"));
    let attempt = &ledger["attempts"][0];
    assert_eq!(attempt["purpose"], json!("executor"));
    assert_eq!(
        attempt["input_bound_method"],
        json!("body_bytes_plus_overhead")
    );
    let bound = attempt["input_bound"].as_u64().unwrap();
    // The bytes of a run's request, and the overhead of 4.
    assert!(bound > 1_000, "{bound}");
    // The client asked for the policy's 500, not its own 8192.
    assert_eq!(attempt["output_bound"], json!(500));
    assert_eq!(
        units(attempt, "reserved"),
        oracle_reserved(bound, 500, 4, 8),
        "the highest input rate is the 12h write's"
    );
}

#[test]
fn ac1_a_request_the_policy_does_not_allow_sends_nothing() {
    type Change = fn(&mut Value);
    type Row = (
        &'static str,
        Change,
        &'static [(&'static str, &'static str)],
        &'static [&'static str],
        &'static str,
    );
    let rows: Vec<Row> = vec![
        (
            "another endpoint",
            |v| v["route"]["endpoint"] = json!("https://api.example.test/v1/responses"),
            &[],
            &[],
            "RouteNotAdmitted",
        ),
        (
            "another model",
            |v| v["route"]["model"] = json!("m-2"),
            &[],
            &[],
            "RouteNotAdmitted",
        ),
        (
            "another effort",
            |_| {},
            &[("COMMANDAGENT_OPENAI_REASONING_EFFORT", "low")],
            &[],
            "RouteNotAdmitted",
        ),
        (
            "a bound past the most input",
            |v| v["max_input_tokens"] = json!(100),
            &[],
            &[],
            "InputBoundExceeded",
        ),
        (
            "a bound past counting",
            |v| v["input_bound"]["input_overhead"] = json!(u64::MAX),
            &[],
            &[],
            "Overflow",
        ),
        (
            "a money limit below the reservation",
            |v| v["limits"]["cost_usd"] = json!("0.000000000000000001"),
            &[],
            &[],
            "CostExceeded",
        ),
        (
            "no physical send",
            |v| v["limits"]["physical_sends"] = json!(0),
            &[],
            &[],
            "PhysicalExceeded",
        ),
        (
            "no token",
            |v| v["limits"]["tokens"] = json!(1),
            &[],
            &[],
            "TokensExceeded",
        ),
    ];
    for (name, change, env, arguments, refusal) in rows {
        let world = World::new();
        world.v2("w-refused", change);
        let ran = world.run(Some("w-refused"), Proxy::Close, env, arguments);
        assert_eq!(ran.arrivals.len(), 0, "{name}: {ran:?}");
        assert!(ran.error.starts_with(SEND_REFUSED), "{name}: {ran:?}");
        assert!(ran.error.contains(refusal), "{name}: {ran:?}");
        let ledger = ledger_body(&world.ledger("w-refused")).unwrap();
        assert_eq!(ledger["attempts"], json!([]), "{name}");
    }
}

#[test]
fn ac1_a_client_or_declaration_the_wallet_cannot_count_is_refused_before_it_exists() {
    let refused_client = "error: the run budget refused the client: RouteNotCountable";
    // Direct Chat Completions under a v2 wallet.
    let world = World::new();
    world.v2("w-chat", |_| {});
    let ran = world.run(
        Some("w-chat"),
        Proxy::Close,
        &[],
        &[
            "--provider",
            "openai",
            "--api",
            "chat-completions",
            "--model",
            "m-1",
        ],
    );
    assert_eq!(ran.arrivals.len(), 0, "{ran:?}");
    assert_eq!(ran.error, refused_client, "{ran:?}");
    // A compatible endpoint, Responses or Chat, under it: nothing reaches the fake provider.
    for api in ["responses", "chat-completions"] {
        let provider = Fake::provider("{}".to_string());
        let base_url = format!("{}/v1", provider.url);
        let world = World::new();
        world.v2("w-compat", |_| {});
        let ran = world.run(
            Some("w-compat"),
            Proxy::Close,
            &[],
            &[
                "--provider",
                "openai-compatible",
                "--base-url",
                &base_url,
                "--api",
                api,
                "--model",
                "m-1",
            ],
        );
        assert_eq!(provider.finish().len(), 0, "{api}: {ran:?}");
        assert_eq!(ran.error, refused_client, "{api}: {ran:?}");
    }
    // A declaration of v2 the decoder refuses (a tier other than default), and no declaration.
    let world = World::new();
    world.v2("w-flex", |v| v["route"]["service_tier"] = json!("flex"));
    let ran = world.direct("w-flex");
    assert_eq!(ran.arrivals.len(), 0);
    assert_eq!(
        ran.error,
        "error: the run budget refused the entry: PolicyV2(UnsupportedServiceTier)"
    );
    let ran = world.direct("w-none");
    assert_eq!(ran.arrivals.len(), 0);
    assert_eq!(
        ran.error,
        "error: the run budget refused the entry: PolicyUnreadable"
    );
}

// ---- AC2: each attempt is on disk before it leaves ----

#[test]
fn ac2_the_attempt_is_started_on_disk_when_it_arrives_and_a_retry_sends_nothing() {
    let world = World::new();
    world.v2("w-ac2", |_| {});
    let ran = world.direct("w-ac2");
    // Retries were 3; the first send was unknown, so no other went.
    assert_eq!(ran.arrivals.len(), 1, "{ran:?}");
    let seen = ran.arrivals[0]
        .attempt
        .clone()
        .expect("a ledger at the attempt");
    assert_eq!(seen["state"], json!("started"));
    assert_eq!(seen["request_sha256"].as_str().unwrap().len(), 64);
    assert!(units(&seen, "reserved") > 0);
    assert!(ran.error.contains("Latched"), "{ran:?}");
}

#[test]
fn ac2_exactly_the_cost_or_token_limit_passes_and_one_less_sends_nothing() {
    // The same workspace and request for each wallet: its reservation, read off the first.
    let world = World::new();
    world.v2("w-probe", |_| {});
    world.direct("w-probe");
    let probe = last_attempt(&world.ledger("w-probe")).unwrap();
    let reserved = units(&probe, "reserved");
    let tokens = probe["input_bound"].as_u64().unwrap() + probe["output_bound"].as_u64().unwrap();
    for (id, cost, token_limit, passes) in [
        ("w-cost-exact", decimal(reserved), tokens, true),
        ("w-cost-short", decimal(reserved - 1), tokens, false),
        ("w-tokens-short", decimal(reserved), tokens - 1, false),
    ] {
        world.v2(id, |v| {
            v["limits"]["cost_usd"] = json!(cost);
            v["limits"]["tokens"] = json!(token_limit);
        });
        let ran = world.direct(id);
        assert_eq!(ran.arrivals.len(), usize::from(passes), "{id}: {ran:?}");
        let attempt = last_attempt(&world.ledger(id));
        if passes {
            assert_eq!(
                units(attempt.as_ref().unwrap(), "reserved"),
                reserved,
                "{id}"
            );
        } else {
            assert!(attempt.is_none(), "{id}");
            assert!(ran.error.starts_with(SEND_REFUSED), "{id}: {ran:?}");
        }
    }
}

#[test]
fn ac2_a_ledger_that_cannot_be_written_sends_nothing() {
    // The wallet's directory is there with a directory where its temporary goes: the first write
    // of the wallet fails, and nothing is sent.
    let world = World::new();
    world.v2("w-disk", |_| {});
    std::fs::create_dir_all(world.ledgers().join("w-disk/ledger.json.tmp")).unwrap();
    let ran = world.direct("w-disk");
    assert_eq!(ran.arrivals.len(), 0, "{ran:?}");
    assert_eq!(
        ran.error, "error: the run budget refused the entry: Wallet(Unavailable)",
        "{ran:?}"
    );
    assert!(!world.ledger("w-disk").exists());
}

// ---- AC3: the reservation's tier through the path ----

#[test]
fn ac3_a_bound_that_only_crosses_the_boundary_reserves_at_the_high_tier() {
    let world = World::new();
    world.v2("w-low", |_| {});
    world.direct("w-low");
    let bytes = last_attempt(&world.ledger("w-low")).unwrap()["input_bound"]
        .as_u64()
        .unwrap()
        - 4;
    // An overhead that puts the bound one token past 272000, with a tier above it at 4/8/12.
    let overhead = 272_001 - bytes;
    world.v2("w-high", |v| {
        v["input_bound"]["input_overhead"] = json!(overhead);
        v["tariff"]["tiers"].as_array_mut().unwrap().push(json!({
            "above_input_tokens": 272_000,
            "input_per_million": "4",
            "cached_input_per_million": "0.4",
            "cache_write_30m_per_million": "5",
            "cache_write_12h_per_million": "8",
            "output_per_million": "12",
            "request_fee": "0"
        }));
    });
    world.direct("w-high");
    let attempt = last_attempt(&world.ledger("w-high")).unwrap();
    assert_eq!(attempt["input_bound"], json!(272_001));
    assert_eq!(
        units(&attempt, "reserved"),
        oracle_reserved(272_001, 500, 8, 12)
    );
    // One token less is the low tier alone.
    world.v2("w-at", |v| {
        v["input_bound"]["input_overhead"] = json!(overhead - 1);
        v["tariff"]["tiers"].as_array_mut().unwrap().push(json!({
            "above_input_tokens": 272_000,
            "input_per_million": "4",
            "cached_input_per_million": "0.4",
            "cache_write_30m_per_million": "5",
            "cache_write_12h_per_million": "8",
            "output_per_million": "12",
            "request_fee": "0"
        }));
    });
    world.direct("w-at");
    let attempt = last_attempt(&world.ledger("w-at")).unwrap();
    assert_eq!(attempt["input_bound"], json!(272_000));
    assert_eq!(
        units(&attempt, "reserved"),
        oracle_reserved(272_000, 500, 4, 8)
    );
}

// ---- AC4: the basis a wallet was made with ----

#[test]
fn ac4_a_v1_wallet_is_not_continued_as_v2_nor_a_v2_one_as_v1() {
    // v1 first: its open makes the record, whatever its send does.
    let world = World::new();
    world.declare("w-mix", &v1_policy("w-mix", &world.ledgers()));
    let provider = Fake::provider("{}".to_string());
    let base_url = format!("{}/v1", provider.url);
    world.run(
        Some("w-mix"),
        Proxy::Close,
        &[],
        &[
            "--provider",
            "openai-compatible",
            "--base-url",
            &base_url,
            "--api",
            "chat-completions",
            "--model",
            "m-1",
        ],
    );
    drop(provider.finish());
    let v1_bytes = std::fs::read(world.ledger("w-mix")).unwrap();
    assert_eq!(
        ledger_body(&world.ledger("w-mix")).unwrap()["schema"],
        json!("run-budget.ledger/v1")
    );
    world.v2("w-mix", |_| {});
    let ran = world.direct("w-mix");
    assert_eq!(ran.arrivals.len(), 0);
    assert_eq!(
        ran.error,
        "error: the run budget refused the entry: Wallet(AccountingBasisChanged)"
    );
    assert_eq!(std::fs::read(world.ledger("w-mix")).unwrap(), v1_bytes);

    // v2 first, with an attempt held: v1 is refused and nothing comes back.
    let world = World::new();
    world.v2("w-mix2", |_| {});
    world.direct("w-mix2");
    let v2_bytes = std::fs::read(world.ledger("w-mix2")).unwrap();
    world.declare("w-mix2", &v1_policy("w-mix2", &world.ledgers()));
    let provider = Fake::provider("{}".to_string());
    let base_url = format!("{}/v1", provider.url);
    let ran = world.run(
        Some("w-mix2"),
        Proxy::Close,
        &[],
        &[
            "--provider",
            "openai-compatible",
            "--base-url",
            &base_url,
            "--api",
            "chat-completions",
            "--model",
            "m-1",
        ],
    );
    assert_eq!(provider.finish().len(), 0);
    assert_eq!(
        ran.error,
        "error: the run budget refused the entry: Wallet(AccountingBasisChanged)"
    );
    assert_eq!(std::fs::read(world.ledger("w-mix2")).unwrap(), v2_bytes);
}

#[test]
fn ac4_a_changed_mode_or_declaration_is_refused_and_the_held_send_stays() {
    let world = World::new();
    world.v2("w-mode", |_| {});
    world.direct("w-mode");
    let before = std::fs::read(world.ledger("w-mode")).unwrap();
    type Change = fn(&mut Value);
    let changes: [(&str, Change); 2] = [
        ("none mode", |v| {
            v["tariff"]["cache_write_mode"] = json!("none");
            let tier = v["tariff"]["tiers"][0].as_object_mut().unwrap();
            tier.remove("cache_write_30m_per_million");
            tier.remove("cache_write_12h_per_million");
        }),
        ("more money", |v| v["limits"]["cost_usd"] = json!("2000")),
    ];
    for (name, change) in changes {
        world.v2("w-mode", change);
        let ran = world.direct("w-mode");
        assert_eq!(ran.arrivals.len(), 0, "{name}");
        assert_eq!(
            ran.error, "error: the run budget refused the entry: Wallet(PolicyChanged)",
            "{name}"
        );
        assert_eq!(
            std::fs::read(world.ledger("w-mode")).unwrap(),
            before,
            "{name}"
        );
    }
    let ledger = ledger_body(&world.ledger("w-mode")).unwrap();
    assert_eq!(
        ledger["attempts"][0]["provider_billed"],
        json!("unreported")
    );
    assert!(!ledger.to_string().contains("billed_usd"));
}

// ---- AC5: unknown, interrupted, never a success ----

#[test]
fn ac5_a_send_that_failed_holds_its_reservation_and_stops_every_later_run() {
    let world = World::new();
    world.v2("w-unknown", |_| {});
    let first = world.direct("w-unknown");
    assert_eq!(first.arrivals.len(), 1);
    assert_ne!(first.code, Some(0));
    let ledger = ledger_body(&world.ledger("w-unknown")).unwrap();
    let attempt = &ledger["attempts"][0];
    assert_eq!(attempt["state"], json!("unknown"));
    assert_eq!(
        attempt["unknown"],
        json!({"kind": "transport", "reason": "send_error"})
    );
    assert_eq!(ledger["latch"]["reason"], json!("unknown_billing"));
    for arguments in [&[][..], &["--resume", "latest"][..]] {
        let ran = world.run(Some("w-unknown"), Proxy::Close, &[], arguments);
        assert_eq!(ran.arrivals.len(), 0, "{arguments:?}: {ran:?}");
        assert_ne!(ran.code, Some(0), "{arguments:?}");
    }
    let after = ledger_body(&world.ledger("w-unknown")).unwrap();
    assert_eq!(after["attempts"].as_array().unwrap().len(), 1);
    assert_eq!(after["attempts"][0]["reserved"], attempt["reserved"]);
}

#[test]
fn ac5_a_process_that_ended_with_its_send_started_leaves_it_unknown_for_the_next() {
    let world = World::new();
    world.v2("w-cut", |_| {});
    // The proxy holds the attempt, and the process is ended while it is out.
    let first = world.run(Some("w-cut"), Proxy::Hold, &[], &[]);
    assert_eq!(first.arrivals.len(), 1);
    assert_eq!(first.code, None);
    let attempt = last_attempt(&world.ledger("w-cut")).unwrap();
    assert_eq!(attempt["state"], json!("started"));
    let reserved = attempt["reserved"].clone();
    // The next run recovers it as unknown, keeps it, and sends nothing.
    let second = world.direct("w-cut");
    assert_eq!(second.arrivals.len(), 0, "{second:?}");
    assert_ne!(second.code, Some(0));
    let ledger = ledger_body(&world.ledger("w-cut")).unwrap();
    assert_eq!(ledger["attempts"][0]["state"], json!("unknown"));
    assert_eq!(
        ledger["attempts"][0]["unknown"],
        json!({"kind": "transport", "reason": "interrupted_after_start"})
    );
    assert_eq!(ledger["attempts"][0]["reserved"], reserved);
    assert_eq!(ledger["latch"]["reason"], json!("interrupted_after_start"));
}

// ---- AC6: one wallet for every role ----

#[test]
fn ac6_every_role_of_the_run_draws_on_the_one_wallet() {
    // A planner of the same direct route goes first; its send is the wallet's, and after it was
    // unknown no role sends again.
    let world = World::new();
    world.v2("w-roles", |_| {});
    let ran = world.run(
        Some("w-roles"),
        Proxy::Close,
        &[],
        &[
            "--provider",
            "openai",
            "--api",
            "responses",
            "--model",
            "m-1",
            "--planner-provider",
            "openai",
            "--planner-model",
            "m-1",
            "--chat-retries",
            "3",
        ],
    );
    let ledger = ledger_body(&world.ledger("w-roles")).unwrap();
    let attempts = ledger["attempts"].as_array().unwrap();
    assert_eq!(attempts.len(), ran.arrivals.len(), "{ran:?}");
    assert_eq!(attempts.len(), 1, "{ran:?}");
    assert!(["planner", "executor"].contains(&attempts[0]["purpose"].as_str().unwrap()));
    assert_eq!(ledger["logicals"], json!([]));
}

// ---- AC8: per-call cache-write events, wallet off, compatible clients ----

fn responses_reply(details: &Value) -> String {
    json!({
        "id": "resp-1006", "model": "m-1", "service_tier": "default",
        "output": [{"type": "message", "content": [{"type": "output_text", "text": "done"}]}],
        "usage": {"input_tokens": 100, "input_tokens_details": details, "output_tokens": 6}
    })
    .to_string()
}

fn chat_reply(usage: &Value) -> String {
    json!({
        "id": "chat-1006", "object": "chat.completion", "model": "m-1",
        "choices": [{"index": 0, "message": {"role": "assistant", "content": "done"}, "finish_reason": "stop"}],
        "usage": usage
    })
    .to_string()
}

fn turn_fields(world: &World) -> Vec<Value> {
    world
        .events()
        .into_iter()
        .filter(|event| event["event"] == "provider_turn_duration")
        .map(|event| {
            let mut fields = serde_json::Map::new();
            for key in [
                "provider_cache_write_input_tokens",
                "provider_cache_write_input_tokens_30m",
                "provider_cache_write_input_tokens_12h",
            ] {
                if let Some(value) = event.get(key) {
                    fields.insert(key.to_string(), value.clone());
                }
            }
            Value::Object(fields)
        })
        .collect()
}

fn compatible_run(api: &str, reply: String) -> (World, usize) {
    let world = World::new();
    let provider = Fake::provider(reply);
    let base_url = format!("{}/v1", provider.url);
    world.run(
        None,
        Proxy::Close,
        &[],
        &[
            "--provider",
            "openai-compatible",
            "--base-url",
            &base_url,
            "--api",
            api,
            "--model",
            "m-1",
            "--chat-retries",
            "0",
        ],
    );
    let sent = provider.finish().len();
    (world, sent)
}

#[test]
fn ac8_each_turn_carries_the_cache_write_its_reply_reported_and_nothing_else() {
    let rows = [
        (
            "ttl",
            json!({"cached_tokens": 10, "cache_write_tokens": 9,
            "cache_write_tokens_details": {"ttl_30m_tokens": 4, "ttl_12h_tokens": 5}}),
            json!({"provider_cache_write_input_tokens": 9, "provider_cache_write_input_tokens_30m": 4,
                "provider_cache_write_input_tokens_12h": 5}),
        ),
        (
            "explicit zero",
            json!({"cached_tokens": 0, "cache_write_tokens": 0,
            "cache_write_tokens_details": {"ttl_30m_tokens": 0, "ttl_12h_tokens": 0}}),
            json!({"provider_cache_write_input_tokens": 0, "provider_cache_write_input_tokens_30m": 0,
                "provider_cache_write_input_tokens_12h": 0}),
        ),
        (
            "total only",
            json!({"cached_tokens": 0, "cache_write_tokens": 7}),
            json!({"provider_cache_write_input_tokens": 7}),
        ),
        ("missing", json!({"cached_tokens": 0}), json!({})),
        (
            "null",
            json!({"cached_tokens": 0, "cache_write_tokens": null}),
            json!({}),
        ),
        (
            "bad type",
            json!({"cached_tokens": 0, "cache_write_tokens": "7"}),
            json!({}),
        ),
        (
            "mismatch",
            json!({"cached_tokens": 0, "cache_write_tokens": 7,
            "cache_write_tokens_details": {"ttl_30m_tokens": 4, "ttl_12h_tokens": 4}}),
            json!({}),
        ),
    ];
    for (name, details, expected) in rows {
        let (world, sent) = compatible_run("responses", responses_reply(&details));
        assert!(sent >= 1, "{name}");
        let turns = turn_fields(&world);
        assert_eq!(turns.len(), sent, "{name}: one turn event per call");
        for turn in turns {
            assert_eq!(turn, expected, "{name}");
        }
    }
    // Chat, at its own paths only; a name the table does not list is not read.
    for (name, usage, expected) in [
        (
            "chat ttl",
            json!({"prompt_tokens": 100, "completion_tokens": 6,
            "prompt_tokens_details": {"cached_tokens": 0, "cache_write_tokens": 3,
                "cache_write_tokens_details": {"ttl_30m_tokens": 3, "ttl_12h_tokens": 0}}}),
            json!({"provider_cache_write_input_tokens": 3, "provider_cache_write_input_tokens_30m": 3,
                "provider_cache_write_input_tokens_12h": 0}),
        ),
        (
            "chat alias",
            json!({"prompt_tokens": 100, "completion_tokens": 6,
            "prompt_tokens_details": {"cache_creation_input_tokens": 3}}),
            json!({}),
        ),
    ] {
        let (world, sent) = compatible_run("chat-completions", chat_reply(&usage));
        assert!(sent >= 1, "{name}");
        for turn in turn_fields(&world) {
            assert_eq!(turn, expected, "{name}");
        }
    }
}

#[test]
fn ac8_a_sum_over_turn_events_alone_counts_each_reported_write_once() {
    let (world, sent) = compatible_run(
        "responses",
        responses_reply(&json!({"cached_tokens": 0, "cache_write_tokens": 9,
            "cache_write_tokens_details": {"ttl_30m_tokens": 4, "ttl_12h_tokens": 5}})),
    );
    assert!(sent >= 1);
    let events = world.events();
    let sum = |name: &str| -> u64 {
        events
            .iter()
            .filter(|event| event["event"] == name)
            .filter_map(|event| event["provider_cache_write_input_tokens"].as_u64())
            .sum()
    };
    // The oracle: what the fake reported, once for each call it answered.
    assert_eq!(sum("provider_turn_duration"), 9 * sent as u64);
    // The response events repeat it; an aggregator reading both would count it twice.
    assert_eq!(sum("provider_response"), sum("provider_turn_duration"));
}
