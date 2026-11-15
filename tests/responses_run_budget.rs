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
/// The byte length of an off-control world's canonical root on every host (see
/// [`World::fixed_length`]).
const FIXED_ROOT_LENGTH: usize = 128;

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
        Self::at(root, base)
    }

    /// A world whose root, as the product sees it after canonicalization, is exactly
    /// [`FIXED_ROOT_LENGTH`] bytes long on every host: the temporary directory with one padded
    /// directory under it. The prompt-size estimates of a run count the raw workspace path, so
    /// the off control is recorded and compared at one length. A temporary directory too long
    /// for the target fails here, not as a difference later.
    fn fixed_length() -> Self {
        let root = tempfile::tempdir().unwrap();
        let canonical = std::fs::canonicalize(root.path()).unwrap();
        let length = canonical.as_os_str().len();
        assert!(
            length + 2 <= FIXED_ROOT_LENGTH,
            "the temporary directory {} is {length} bytes; the off control needs it at most {} \
             (set TMPDIR to a shorter directory)",
            canonical.display(),
            FIXED_ROOT_LENGTH - 2
        );
        let base = canonical.join("p".repeat(FIXED_ROOT_LENGTH - length - 1));
        std::fs::create_dir(&base).unwrap();
        let base = std::fs::canonicalize(base).unwrap();
        assert_eq!(base.as_os_str().len(), FIXED_ROOT_LENGTH);
        Self::at(root, base)
    }

    fn at(root: tempfile::TempDir, base: PathBuf) -> Self {
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

// ---- AC7: wallet off, against a recording of the unchanged base ----

/// Issue #1006, AC7: with no wallet, what a run sends, the events it writes, the files it leaves
/// and how it ends are what the unchanged base ff920681 gave for the same runs.
///
/// The base's record is `tests/corpus/apps/responses-run-budget/fixtures/off-golden.json`, written
/// by [`off_control::records_the_off_golden`] in a git-archive copy of ff920681 (see that corpus's
/// `source-manifest.json`). The only differences allowed are the values named in
/// [`off_control::NORMALIZED`], each shown to vary between two runs of the base, and the three new
/// event fields, removed by their exact names before an event line is compared.
mod off_control {
    use std::collections::{BTreeMap, VecDeque};

    use super::*;

    pub(super) const GOLDEN: &str =
        "tests/corpus/apps/responses-run-budget/fixtures/off-golden.json";
    pub(super) const NEW_FIELDS: [&str; 3] = [
        "provider_cache_write_input_tokens",
        "provider_cache_write_input_tokens_30m",
        "provider_cache_write_input_tokens_12h",
    ];
    /// What differs between two runs of the base itself, and the build's own identity, and only
    /// that. Each is listed with its evidence in the corpus's `source-manifest.json`.
    pub(super) const NORMALIZED: [&str; 8] = [
        "the run's temporary root, wherever it appears in a string -> <ROOT>",
        "the fake's loopback address http://127.0.0.1:<port> and 127.0.0.1:<port> -> <LOOPBACK>",
        "a non-negative integer, at any depth of an event, under one of TIMING_KEYS -> 0; any other type is kept",
        "a string under build_commit or build_timestamp, a boolean under build_dirty -> \"<BUILD>\"; any other type is kept",
        "in the workspace lock file, the values of pid and run_id -> \"<PID>\", \"<RUN>\"",
        "in a bash_sandbox_status event, a backend that is one of SANDBOX_BACKENDS -> \"<SANDBOX_BACKEND>\"; any other value or type is kept",
        "not normalized: the run's root is exactly FIXED_ROOT_LENGTH bytes on every host, so the prompt-size estimates are compared as they are",
        "nothing else: every other byte of a request, an event line, a file, and the outcome is compared",
    ];
    /// The wall-clock values two runs of the base differ in.
    pub(super) const TIMING_KEYS: [&str; 7] = [
        "duration_ms",
        "occurred_at_epoch_ms",
        "executor_ms",
        "provider_ms",
        "total_ms",
        "time_profile_provider_ms",
        "time_profile_total_ms",
    ];
    /// What says which binary ran: the base and this code are two builds.
    pub(super) const BUILD_KEYS: [&str; 3] = ["build_commit", "build_dirty", "build_timestamp"];
    /// The host's OS sandbox, as `src/tools/os_sandbox.rs` names it: macOS, Linux, or neither.
    pub(super) const SANDBOX_BACKENDS: [&str; 3] = ["seatbelt", "landlock", "none"];

    /// A provider on 127.0.0.1 that answers each request with the next of `replies` (the last one
    /// again once they run out) and keeps each request's line and body.
    struct Script {
        url: String,
        stop: Arc<AtomicBool>,
        thread: JoinHandle<()>,
        seen: Arc<Mutex<Vec<(String, String)>>>,
    }

    impl Script {
        fn spawn(replies: Vec<(u16, String)>) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let url = format!("http://{}", listener.local_addr().unwrap());
            listener.set_nonblocking(true).unwrap();
            let stop = Arc::new(AtomicBool::new(false));
            let seen = Arc::new(Mutex::new(Vec::new()));
            let (thread_stop, thread_seen) = (stop.clone(), seen.clone());
            let thread = std::thread::spawn(move || {
                let mut replies: VecDeque<(u16, String)> = replies.into();
                while !thread_stop.load(Ordering::SeqCst) {
                    let Ok((mut stream, _)) = listener.accept() else {
                        std::thread::sleep(Duration::from_millis(5));
                        continue;
                    };
                    stream.set_nonblocking(false).unwrap();
                    let request = read_request(&mut stream);
                    thread_seen.lock().unwrap().push(request);
                    let (status, body) = if replies.len() > 1 {
                        replies.pop_front().unwrap()
                    } else {
                        replies.front().cloned().unwrap()
                    };
                    let response = format!(
                        "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = stream.write_all(response.as_bytes());
                }
            });
            Self {
                url,
                stop,
                thread,
                seen,
            }
        }

        fn finish(self) -> Vec<(String, String)> {
            self.stop.store(true, Ordering::SeqCst);
            self.thread.join().unwrap();
            self.seen.lock().unwrap().clone()
        }
    }

    fn usage(cache_write: bool) -> Value {
        let mut usage = json!({
            "input_tokens": 50,
            "input_tokens_details": {"cached_tokens": 10},
            "output_tokens": 5,
            "output_tokens_details": {"reasoning_tokens": 2},
            "total_tokens": 55
        });
        if cache_write {
            usage["input_tokens_details"]["cache_write_tokens"] = json!(4);
            usage["input_tokens_details"]["cache_write_tokens_details"] =
                json!({"ttl_30m_tokens": 3, "ttl_12h_tokens": 1});
        }
        usage
    }

    fn responses_reply(id: &str, output: Value, cache_write: bool) -> String {
        json!({
            "id": id, "object": "response", "model": "m-1-snapshot", "status": "completed",
            "service_tier": "default", "output": output, "usage": usage(cache_write)
        })
        .to_string()
    }

    fn text(id: &str, text: &str) -> Value {
        json!({"type": "message", "id": id, "status": "completed", "role": "assistant",
               "content": [{"type": "output_text", "text": text, "annotations": [], "logprobs": []}]})
    }

    fn write_call(call_id: &str) -> Value {
        json!({"type": "function_call", "id": format!("fc_{call_id}"), "call_id": call_id,
               "name": "Write", "status": "completed",
               "arguments": "{\"path\":\"notes.txt\",\"content\":\"synthetic notes\\n\"}"})
    }

    fn reasoning() -> Value {
        json!({"type": "reasoning", "id": "rs_off_1",
               "summary": [{"type": "summary_text", "text": "synthetic summary"}],
               "encrypted_content": "synthetic-encrypted"})
    }

    fn chat_reply(content: &str) -> String {
        json!({
            "id": "chat-off", "object": "chat.completion", "model": "m-1-snapshot",
            "choices": [{"index": 0, "message": {"role": "assistant", "content": content}, "finish_reason": "stop"}],
            "usage": {"prompt_tokens": 50, "completion_tokens": 5,
                      "prompt_tokens_details": {"cached_tokens": 10, "cache_write_tokens": 4}}
        })
        .to_string()
    }

    /// One run of a path: its id, the API on the compatible entry (or `None` for the direct
    /// client behind the proxy), and the replies.
    struct OffPath {
        id: &'static str,
        api: Option<&'static str>,
        replies: Vec<(u16, String)>,
        retries: &'static str,
    }

    fn paths() -> Vec<OffPath> {
        vec![
            OffPath {
                id: "responses-ordinary-reply",
                api: Some("responses"),
                replies: vec![(
                    200,
                    responses_reply("resp-o1", json!([text("msg_o1", "done")]), true),
                )],
                retries: "0",
            },
            OffPath {
                id: "responses-tool-call",
                api: Some("responses"),
                replies: vec![
                    (
                        200,
                        responses_reply("resp-t1", json!([write_call("call_t1")]), false),
                    ),
                    (
                        200,
                        responses_reply("resp-t2", json!([text("msg_t2", "done")]), true),
                    ),
                ],
                retries: "0",
            },
            OffPath {
                id: "responses-reasoning-replay",
                api: Some("responses"),
                replies: vec![
                    (
                        200,
                        responses_reply(
                            "resp-r1",
                            json!([reasoning(), write_call("call_r1")]),
                            false,
                        ),
                    ),
                    (
                        200,
                        responses_reply("resp-r2", json!([text("msg_r2", "done")]), false),
                    ),
                ],
                retries: "0",
            },
            OffPath {
                id: "responses-retry",
                api: Some("responses"),
                replies: vec![
                    (500, "{}".to_string()),
                    (
                        200,
                        responses_reply("resp-x1", json!([text("msg_x1", "done")]), false),
                    ),
                ],
                retries: "1",
            },
            OffPath {
                id: "chat-ordinary-reply",
                api: Some("chat-completions"),
                replies: vec![(200, chat_reply("done"))],
                retries: "0",
            },
            OffPath {
                id: "direct-responses-retry",
                api: None,
                replies: Vec::new(),
                retries: "1",
            },
        ]
    }

    fn files_under(root: &Path, at: &Path, out: &mut BTreeMap<String, String>) {
        let Ok(entries) = std::fs::read_dir(at) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                files_under(root, &path, out);
            } else {
                let name = path
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .to_string();
                let content = std::fs::read(&path).unwrap_or_default();
                out.insert(name, String::from_utf8_lossy(&content).to_string());
            }
        }
    }

    /// With `RESPONSES_RUN_BUDGET_OFF_RAW` set, the recorder keeps every value as the run wrote it:
    /// the evidence that each normalized value varies between two runs of the base.
    fn raw() -> bool {
        std::env::var_os("RESPONSES_RUN_BUDGET_OFF_RAW").is_some()
    }

    fn normalize_text(text: &str, root: &str, loopback: Option<&str>) -> String {
        if raw() {
            return text.to_string();
        }
        let mut text = text.replace(root, "<ROOT>");
        if let Some(url) = loopback {
            text = text.replace(url, "<LOOPBACK>");
            text = text.replace(url.trim_start_matches("http://"), "<LOOPBACK>");
        }
        text
    }

    fn normalize_value(value: &mut Value) {
        match value {
            Value::Object(map) => {
                for (key, item) in map.iter_mut() {
                    // A value is normalized only when it has the type it is known to have: a
                    // duration a non-negative integer, a commit and a time strings, a dirty flag a
                    // boolean. Anything else is kept, and so is a difference.
                    let build_typed = match key.as_str() {
                        "build_dirty" => item.is_boolean(),
                        _ => item.is_string(),
                    };
                    if TIMING_KEYS.contains(&key.as_str()) && item.is_u64() {
                        *item = json!(0);
                    } else if BUILD_KEYS.contains(&key.as_str()) && build_typed {
                        *item = json!("<BUILD>");
                    } else {
                        normalize_value(item);
                    }
                }
            }
            Value::Array(items) => items.iter_mut().for_each(normalize_value),
            _ => {}
        }
    }

    /// The sandbox backend of a `bash_sandbox_status` event names the host, not the run. Only a
    /// known backend's name is normalized; the rest of the event is compared.
    fn normalize_backend(value: &mut Value) {
        if value.get("event").and_then(Value::as_str) != Some("bash_sandbox_status") {
            return;
        }
        if let Some(backend) = value.get_mut("backend")
            && backend
                .as_str()
                .is_some_and(|name| SANDBOX_BACKENDS.contains(&name))
        {
            *backend = json!("<SANDBOX_BACKEND>");
        }
    }

    /// One event line, normalized as a recording does, with no root or loopback in it.
    pub(super) fn normalize_line(line: &str) -> String {
        normalize_event(line, "\u{0}", None)
    }

    /// A line that is not JSON is kept as it came, marked, so it is a difference and not dropped.
    fn normalize_event(line: &str, root: &str, loopback: Option<&str>) -> String {
        match serde_json::from_str::<Value>(line) {
            Ok(mut value) => {
                if !raw() {
                    normalize_value(&mut value);
                    normalize_backend(&mut value);
                }
                normalize_text(&value.to_string(), root, loopback)
            }
            Err(_) => format!("<NOT JSON> {}", normalize_text(line, root, loopback)),
        }
    }

    fn normalize_lock(content: &str) -> String {
        match serde_json::from_str::<Value>(content) {
            Ok(mut value) if value.get("pid").is_some() => {
                value["pid"] = json!("<PID>");
                value["run_id"] = json!("<RUN>");
                value.to_string()
            }
            _ => content.to_string(),
        }
    }

    /// Runs one path with no wallet and returns its record.
    fn record(path: &OffPath) -> Value {
        let world = World::fixed_length();
        let root = world.base.to_str().unwrap().to_string();
        let script = path.api.map(|_| Script::spawn(path.replies.clone()));
        let base_url = script.as_ref().map(|script| format!("{}/v1", script.url));
        let mut arguments: Vec<&str> = match (path.api, base_url.as_deref()) {
            (Some(api), Some(url)) => vec![
                "--provider",
                "openai-compatible",
                "--base-url",
                url,
                "--api",
                api,
            ],
            _ => vec!["--provider", "openai", "--api", "responses"],
        };
        arguments.extend(["--model", "m-1", "--chat-retries", path.retries, "--yes"]);
        let ran = world.run(None, Proxy::Close, &[], &arguments);
        let loopback = script.as_ref().map(|script| script.url.clone());
        let requests: Vec<Value> = script
            .map(Script::finish)
            .unwrap_or_default()
            .into_iter()
            .map(|(line, body)| {
                json!({"line": line, "body": normalize_text(&body, &root, loopback.as_deref())})
            })
            .collect();
        let events: Vec<String> = std::fs::read_to_string(world.base.join("events.jsonl"))
            .unwrap_or_default()
            .lines()
            .map(|line| normalize_event(line, &root, loopback.as_deref()))
            .collect();
        let mut artifacts = BTreeMap::new();
        let workspace = world.base.join("workspace");
        files_under(&workspace, &workspace, &mut artifacts);
        let artifacts: BTreeMap<String, String> = artifacts
            .into_iter()
            .map(|(name, content)| {
                let content = if name == ".commandagent/lock" && !raw() {
                    normalize_lock(&content)
                } else {
                    content
                };
                (name, normalize_text(&content, &root, loopback.as_deref()))
            })
            .collect();
        let mut ledgers = BTreeMap::new();
        files_under(&world.ledgers(), &world.ledgers(), &mut ledgers);
        json!({
            "id": path.id,
            "requests": requests,
            "connects": ran.arrivals.len(),
            "events": events,
            "artifacts": artifacts,
            "outcome": {"code": ran.code, "error": normalize_text(&ran.error, &root, loopback.as_deref())},
            "ledger_files": ledgers.len()
        })
    }

    pub(super) fn record_all() -> Vec<Value> {
        paths().iter().map(record).collect()
    }

    /// An event line with the three new names taken out, by those exact names only.
    pub(super) fn without_new_fields(line: &str) -> String {
        let mut value: Value = serde_json::from_str(line).unwrap();
        if let Some(object) = value.as_object_mut() {
            for key in NEW_FIELDS {
                object.remove(key);
            }
        }
        value.to_string()
    }

    /// The first place, in key order, where `now` is not `base`: its path from `at` and both
    /// values (`<absent>` for a missing one). The fixtures hold no secret value.
    fn first_difference(base: &Value, now: &Value, at: &str) -> Option<(String, String, String)> {
        if base == now {
            return None;
        }
        let shown = |value: Option<&Value>| value.map_or("<absent>".to_string(), Value::to_string);
        match (base, now) {
            (Value::Object(left), Value::Object(right)) => {
                let mut keys: Vec<&String> = left.keys().chain(right.keys()).collect();
                keys.sort();
                keys.dedup();
                keys.into_iter().find_map(|key| {
                    let path = format!("{at}.{key}");
                    match (left.get(key), right.get(key)) {
                        (Some(l), Some(r)) => first_difference(l, r, &path),
                        (l, r) if l != r => Some((path, shown(l), shown(r))),
                        _ => None,
                    }
                })
            }
            (Value::Array(left), Value::Array(right)) => {
                (0..left.len().max(right.len())).find_map(|index| {
                    let path = format!("{at}[{index}]");
                    match (left.get(index), right.get(index)) {
                        (Some(l), Some(r)) => first_difference(l, r, &path),
                        (l, r) => Some((path, shown(l), shown(r))),
                    }
                })
            }
            _ => Some((at.to_string(), base.to_string(), now.to_string())),
        }
    }

    /// Where two event lines differ: `(<event>): field <name>: base <v> now <v>`, or both lines
    /// whole when one is not JSON.
    fn line_difference(base: &str, now: &str) -> String {
        match (
            serde_json::from_str::<Value>(base),
            serde_json::from_str::<Value>(now),
        ) {
            (Ok(left), Ok(right)) => {
                let event = left["event"].as_str().unwrap_or("?").to_string();
                match first_difference(&left, &right, "") {
                    Some((field, l, r)) => format!(
                        "({event}): field {}: base {l} now {r}",
                        field.trim_start_matches('.')
                    ),
                    None => format!("({event}): same value, other bytes: base {base} now {now}"),
                }
            }
            _ => format!("(not JSON): base {base} now {now}"),
        }
    }

    /// Every way `current` is not what the base recorded in `golden`.
    pub(super) fn differences(golden: &Value, current: &Value) -> Vec<String> {
        let mut found = Vec::new();
        let id = golden["id"].as_str().unwrap_or("?");
        for key in [
            "id",
            "requests",
            "connects",
            "artifacts",
            "outcome",
            "ledger_files",
        ] {
            if let Some((at, base, now)) = first_difference(&golden[key], &current[key], key) {
                found.push(format!("{id}: {at}: base {base} now {now}"));
            }
        }
        let (golden_events, current_events) = (
            golden["events"].as_array().cloned().unwrap_or_default(),
            current["events"].as_array().cloned().unwrap_or_default(),
        );
        if golden_events.len() != current_events.len() {
            found.push(format!(
                "{id}: {} event lines, the base had {}",
                current_events.len(),
                golden_events.len()
            ));
        }
        for (index, (base, now)) in golden_events.iter().zip(&current_events).enumerate() {
            let (base, now) = (base.as_str().unwrap(), now.as_str().unwrap());
            let has_new = NEW_FIELDS
                .iter()
                .any(|key| now.contains(&format!("\"{key}\"")));
            let comparable = if has_new {
                without_new_fields(now)
            } else {
                now.to_string()
            };
            if comparable != base {
                found.push(format!(
                    "{id}: event line {index} {}",
                    line_difference(base, &comparable)
                ));
            }
        }
        found
    }

    /// The recorder: run on the base copy with `RESPONSES_RUN_BUDGET_OFF_OUT` set; elsewhere it
    /// does nothing.
    #[test]
    fn records_the_off_golden() {
        let Some(out) = std::env::var_os("RESPONSES_RUN_BUDGET_OFF_OUT") else {
            return;
        };
        let version = std::process::Command::new(env!("CARGO_BIN_EXE_commandagent"))
            .arg("--version")
            .output()
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
            .unwrap_or_default();
        let mut golden = json!({
            "issue": "#1006",
            "kind": "responses-run-budget-off",
            "source": "ff920681",
            "normalization": NORMALIZED,
            "paths": record_all()
        });
        if raw() {
            golden["raw"] = json!(true);
            golden["binary_version"] = json!(version);
        }
        std::fs::write(out, serde_json::to_string_pretty(&golden).unwrap() + "\n").unwrap();
    }
}

#[test]
fn ac7_with_no_wallet_a_run_is_what_the_base_recorded() {
    let golden: Value =
        serde_json::from_str(&std::fs::read_to_string(off_control::GOLDEN).unwrap()).unwrap();
    let paths = golden["paths"].as_array().unwrap();
    let current = off_control::record_all();
    assert_eq!(current.len(), paths.len());
    let mut found = Vec::new();
    let mut new_field_lines = 0;
    for (base, now) in paths.iter().zip(&current) {
        found.extend(off_control::differences(base, now));
        assert_eq!(now["ledger_files"], json!(0), "no ledger with no wallet");
        new_field_lines += now["events"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|line| {
                off_control::NEW_FIELDS
                    .iter()
                    .any(|key| line.as_str().unwrap().contains(&format!("\"{key}\"")))
            })
            .count();
    }
    assert_eq!(found, Vec::<String>::new());
    // The comparison did meet the new fields: the replies that report a cache write.
    assert!(new_field_lines > 0);
}

#[test]
fn ac7_the_off_comparison_sees_every_other_change() {
    let golden: Value =
        serde_json::from_str(&std::fs::read_to_string(off_control::GOLDEN).unwrap()).unwrap();
    let base = golden["paths"]
        .as_array()
        .unwrap()
        .iter()
        .find(|path| path["id"] == "responses-ordinary-reply")
        .unwrap()
        .clone();
    assert!(off_control::differences(&base, &base).is_empty());
    let line = |value: &Value, index: usize| value["events"][index].as_str().unwrap().to_string();
    let last = base["events"].as_array().unwrap().len() - 1;
    type Change = Box<dyn Fn(&mut Value)>;
    let changes: Vec<(&str, Change)> = vec![
        (
            "another field",
            Box::new(move |v: &mut Value| {
                let mut event: Value = serde_json::from_str(&line(v, last)).unwrap();
                event["provider_cached_input_tokens"] = json!(999);
                v["events"][last] = json!(event.to_string());
            }),
        ),
        (
            "a field like the new ones",
            Box::new(move |v: &mut Value| {
                let mut event: Value = serde_json::from_str(&line(v, last)).unwrap();
                event["provider_cache_write_input_tokens_1h"] = json!(1);
                v["events"][last] = json!(event.to_string());
            }),
        ),
        (
            "a line more",
            Box::new(|v: &mut Value| {
                v["events"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!("{\"event\":\"extra\"}"));
            }),
        ),
        (
            "a line less",
            Box::new(|v: &mut Value| {
                v["events"].as_array_mut().unwrap().pop();
            }),
        ),
        (
            "a request byte",
            Box::new(|v: &mut Value| {
                let body = v["requests"][0]["body"]
                    .as_str()
                    .unwrap()
                    .replacen("m-1", "m-2", 1);
                v["requests"][0]["body"] = json!(body);
            }),
        ),
        (
            "an artifact",
            Box::new(|v: &mut Value| v["artifacts"]["x.txt"] = json!("x")),
        ),
        (
            "the outcome",
            Box::new(|v: &mut Value| v["outcome"]["code"] = json!(99)),
        ),
        (
            "a ledger file",
            Box::new(|v: &mut Value| v["ledger_files"] = json!(1)),
        ),
    ];
    for (name, change) in changes {
        let mut current = base.clone();
        change(&mut current);
        assert!(
            !off_control::differences(&base, &current).is_empty(),
            "{name}"
        );
    }
    // Only the three exact names come out.
    let mut current = base.clone();
    let mut event: Value = serde_json::from_str(&line(&current, last)).unwrap();
    for key in off_control::NEW_FIELDS {
        event[key] = json!(7);
    }
    current["events"][last] = json!(event.to_string());
    assert!(off_control::differences(&base, &current).is_empty());
}

/// Review 3, should 2: the off comparison's preprocessing normalizes a value only when it has the
/// expected type, and a line that is not JSON fails the comparison.
#[test]
fn ac7_the_preprocessing_does_not_hide_a_changed_type_or_a_bad_line() {
    let golden: Value =
        serde_json::from_str(&std::fs::read_to_string(off_control::GOLDEN).unwrap()).unwrap();
    let base = golden["paths"]
        .as_array()
        .unwrap()
        .iter()
        .find(|path| path["id"] == "responses-ordinary-reply")
        .unwrap()
        .clone();
    let events = base["events"].as_array().unwrap();
    let find = |name: &str| {
        events
            .iter()
            .position(|line| {
                line.as_str()
                    .unwrap()
                    .contains(&format!("\"event\":\"{name}\""))
            })
            .unwrap()
    };
    let (turn, stop) = (find("provider_turn_duration"), find("tui_command_stop"));
    // A raw line as a run writes it, normalized the way the recorder does.
    let renormalized = |index: usize, change: &dyn Fn(&mut Value)| {
        let mut raw: Value = serde_json::from_str(events[index].as_str().unwrap()).unwrap();
        change(&mut raw);
        let mut current = base.clone();
        current["events"][index] = json!(off_control::normalize_line(&raw.to_string()));
        off_control::differences(&base, &current)
    };
    // The control: real values of the right type normalize to the base's.
    assert!(renormalized(turn, &|v| v["duration_ms"] = json!(1234)).is_empty());
    assert!(
        renormalized(stop, &|v| {
            v["build_dirty"] = json!(false);
            v["build_commit"] = json!("0123abcd");
            v["build_timestamp"] = json!("2026-11-14T00:00:00+09:00");
        })
        .is_empty()
    );
    // Another type is not normalized away.
    for (name, index, change) in [
        (
            "duration null",
            turn,
            Box::new(|v: &mut Value| v["duration_ms"] = json!(null)) as Box<dyn Fn(&mut Value)>,
        ),
        (
            "duration negative",
            turn,
            Box::new(|v: &mut Value| v["duration_ms"] = json!(-1)),
        ),
        (
            "dirty a number",
            stop,
            Box::new(|v: &mut Value| v["build_dirty"] = json!(0)),
        ),
        (
            "commit a number",
            stop,
            Box::new(|v: &mut Value| v["build_commit"] = json!(1)),
        ),
        (
            "timestamp null",
            stop,
            Box::new(|v: &mut Value| v["build_timestamp"] = json!(null)),
        ),
    ] {
        assert!(!renormalized(index, &*change).is_empty(), "{name}");
    }
    // A line that is not JSON is kept as a line, and so is a difference.
    let mut current = base.clone();
    current["events"]
        .as_array_mut()
        .unwrap()
        .push(json!(off_control::normalize_line("{not json")));
    assert!(!off_control::differences(&base, &current).is_empty());
    let mut current = base.clone();
    current["events"][turn] = json!(off_control::normalize_line("{\"event\":"));
    assert!(!off_control::differences(&base, &current).is_empty());
}

/// The base's record of its ordinary Responses path, and the index of its first line of `event`.
fn off_ordinary_reply() -> Value {
    let golden: Value =
        serde_json::from_str(&std::fs::read_to_string(off_control::GOLDEN).unwrap()).unwrap();
    golden["paths"]
        .as_array()
        .unwrap()
        .iter()
        .find(|path| path["id"] == "responses-ordinary-reply")
        .unwrap()
        .clone()
}

fn off_line_of(base: &Value, event: &str) -> usize {
    base["events"]
        .as_array()
        .unwrap()
        .iter()
        .position(|line| {
            line.as_str()
                .unwrap()
                .contains(&format!("\"event\":\"{event}\""))
        })
        .unwrap()
}

/// The differences after line `index` of `base` is written raw with `change` and normalized
/// the way the recorder does.
fn off_renormalized(base: &Value, index: usize, change: &dyn Fn(&mut Value)) -> Vec<String> {
    let mut raw: Value = serde_json::from_str(base["events"][index].as_str().unwrap()).unwrap();
    change(&mut raw);
    let mut current = base.clone();
    current["events"][index] = json!(off_control::normalize_line(&raw.to_string()));
    off_control::differences(base, &current)
}

/// F6: the host's sandbox backend is normalized only when it is a known backend's name; any
/// other value or type, or the field gone, is still a difference.
#[test]
fn ac7_only_a_known_sandbox_backend_is_normalized() {
    let base = off_ordinary_reply();
    let status = off_line_of(&base, "bash_sandbox_status");
    assert!(
        base["events"][status]
            .as_str()
            .unwrap()
            .contains("\"backend\":\"<SANDBOX_BACKEND>\"")
    );
    for name in off_control::SANDBOX_BACKENDS {
        assert!(
            off_renormalized(&base, status, &|v| v["backend"] = json!(name)).is_empty(),
            "{name}"
        );
    }
    let others: Vec<(&str, Value)> = vec![
        ("an unknown name", json!("bubblewrap")),
        ("another case", json!("Seatbelt")),
        ("an empty string", json!("")),
        ("a number", json!(1)),
        ("null", json!(null)),
        ("a boolean", json!(true)),
        ("an object", json!({"name": "seatbelt"})),
    ];
    for (name, value) in others {
        let found = off_renormalized(&base, status, &|v| v["backend"] = value.clone());
        assert_eq!(found.len(), 1, "{name}");
        assert!(found[0].contains("field backend"), "{name}: {found:?}");
    }
    let gone = off_renormalized(&base, status, &|v| {
        v.as_object_mut().unwrap().remove("backend");
    });
    assert!(!gone.is_empty());
    // The rest of the event is still compared.
    assert!(!off_renormalized(&base, status, &|v| v["applied"] = json!(false)).is_empty());
    // A backend field in another event is not touched.
    let other = off_control::normalize_line(r#"{"backend":"seatbelt","event":"other"}"#);
    assert!(other.contains("\"seatbelt\""));
}

/// F6: a difference names the first differing field and both values, not only the line.
#[test]
fn ac7_a_difference_names_its_field_and_both_values() {
    let base = off_ordinary_reply();
    let turn = off_line_of(&base, "provider_turn_duration");
    let event: Value = serde_json::from_str(base["events"][turn].as_str().unwrap()).unwrap();
    let was = event["estimated_prompt_tokens_sent"].clone();
    assert!(was.is_u64());
    let found = off_renormalized(&base, turn, &|v| {
        v["estimated_prompt_tokens_sent"] = json!(was.as_u64().unwrap() + 13)
    });
    assert_eq!(
        found,
        vec![format!(
            "responses-ordinary-reply: event line {turn} (provider_turn_duration): field \
             estimated_prompt_tokens_sent: base {was} now {}",
            was.as_u64().unwrap() + 13
        )]
    );
    // A field gone, and one nested in an object.
    let found = off_renormalized(&base, turn, &|v| {
        v.as_object_mut()
            .unwrap()
            .remove("estimated_prompt_tokens_sent");
    });
    assert_eq!(
        found,
        vec![format!(
            "responses-ordinary-reply: event line {turn} (provider_turn_duration): field \
             estimated_prompt_tokens_sent: base {was} now <absent>"
        )]
    );
    let profile = off_line_of(&base, "time_profile");
    let found = off_renormalized(&base, profile, &|v| v["profile"]["extra"] = json!(1));
    assert_eq!(
        found,
        vec![format!(
            "responses-ordinary-reply: event line {profile} (time_profile): field \
             profile.extra: base <absent> now 1"
        )]
    );
    // Outside the event lines: the path into the record.
    let mut current = base.clone();
    current["outcome"]["code"] = json!(99);
    assert_eq!(
        off_control::differences(&base, &current),
        vec![format!(
            "responses-ordinary-reply: outcome.code: base {} now 99",
            base["outcome"]["code"]
        )]
    );
    // A line that is not JSON is shown whole.
    let mut current = base.clone();
    current["events"][turn] = json!(off_control::normalize_line("{\"event\":"));
    let found = off_control::differences(&base, &current);
    assert_eq!(found.len(), 1);
    assert!(
        found[0].starts_with(&format!(
            "responses-ordinary-reply: event line {turn} (not JSON): base "
        )),
        "{found:?}"
    );
}
