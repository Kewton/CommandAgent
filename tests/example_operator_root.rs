//! Issue #1014: the operator-root example under `examples/operator-root/`, run the way the user
//! guide (`docs/guide/*/operator-root-tutorial.md`) tells an operator to run it.
//!
//! On every OS: the templates decode as the host's own types, the shipped schema pins are real,
//! `setup.sh` lays out a root whose run budget the binary accepts, the bash helpers give the same
//! SHA-256 and base64 as `sha2`, the fake model answers as documented, and neither the sample nor
//! the tutorial names a user or a machine.
//!
//! On macOS, where the pinned process can run: the success path (check, copy the pins, check
//! again, rehearse against the fake) and the refusal path (one byte appended to a pinned script is
//! refused before the start, with no connection to the fake). An isolation probe that is
//! `unavailable` fails these tests; nothing here is skipped or ignored.
//!
//! Every binary runs as the guide's `ca` wrapper runs it: an empty environment, `HOME` and
//! `TMPDIR` at the `home/` and `tmp/` that `setup.sh` makes, the three `COMMANDAGENT_*` variables,
//! and `PATH=/usr/bin:/bin`. A test holds the guide's wrapper to the same list. No model, key, or
//! external provider is used: the only provider is `examples/operator-root/rehearse/fake_model.py`
//! on 127.0.0.1.

#![cfg(unix)]

use std::io::{Read as _, Write as _};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use commandagent::tools::profile_tools::descriptor::Descriptor;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const PROFILE: &str = "two-stage-example";
const WALLET: &str = "example-wallet";
const TOOLS: [&str; 2] = ["example-parse", "example-check"];
const UNUSED_URL: &str = "http://127.0.0.1:9/v1";

/// What stage `parse` makes of the shipped `workspace/input/items.txt`.
const ITEMS: &[u8] =
    b"{\"items\":[{\"name\":\"apple\",\"quantity\":3},{\"name\":\"pear\",\"quantity\":0},{\"name\":\"plum\",\"quantity\":12}]}\n";

/// The environment the guide's `ca` wrapper gives `commandagent` (besides the caller's `PATH`),
/// with `$ROOT` for the laid-out root. The tests run the binary with exactly these.
const RUN_ENV: [(&str, &str); 5] = [
    ("HOME", "$ROOT/home"),
    ("TMPDIR", "$ROOT/tmp"),
    ("COMMANDAGENT_ADAPTER_CONFIG_ROOT", "$ROOT/operator"),
    ("COMMANDAGENT_RUN_BUDGET", WALLET),
    ("COMMANDAGENT_COMPATIBLE_REASONING_EFFORT", "low"),
];

fn tutorial(language: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("docs/guide/{language}/operator-root-tutorial.md"))
}

fn sample() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/operator-root")
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn encode_base64(bytes: &[u8]) -> String {
    const SYMBOLS: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut text = String::new();
    for chunk in bytes.chunks(3) {
        let word = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for index in 0..4 {
            if index <= chunk.len() {
                text.push(SYMBOLS[(word >> (18 - 6 * index) & 63) as usize] as char);
            } else {
                text.push('=');
            }
        }
    }
    text
}

fn said(output: &Output) -> String {
    format!(
        "{}\n--- stdout\n{}\n--- stderr\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// An operator's machine in a temporary directory: the sample laid out by its own `setup.sh`,
/// with the empty `home/` and `tmp/` it makes for the run's `HOME` and `TMPDIR`.
struct Operator {
    _temp: tempfile::TempDir,
    root: PathBuf,
}

impl Operator {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(temp.path()).unwrap().join("example");
        let output = Command::new("/bin/bash")
            .arg(sample().join("setup.sh"))
            .arg(&root)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .output()
            .unwrap();
        assert!(output.status.success(), "setup.sh: {}", said(&output));
        let printed = String::from_utf8_lossy(&output.stdout);
        assert!(printed.contains("--operator-check --json"), "{printed}");
        assert!(
            printed.contains("This command is for the sample only.")
                && printed.contains("every file under tools/ is part of the directory pin"),
            "{printed}"
        );
        for directory in ["home", "tmp"] {
            let made = root.join(directory);
            assert_eq!(
                std::fs::read_dir(&made).map(|entries| entries.count()).ok(),
                Some(0),
                "setup.sh makes an empty {made:?}"
            );
        }
        Self { _temp: temp, root }
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    /// The command the guide gives, through its `ca` wrapper, against `url`.
    fn command(&self, url: &str) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_commandagent"));
        command
            .env_clear()
            .current_dir(self.path("workspace"))
            .env("PATH", "/usr/bin:/bin");
        let root = self.root.to_str().unwrap();
        for (name, value) in RUN_ENV {
            command.env(name, value.replace("$ROOT", root));
        }
        command
            .arg("--cwd")
            .arg(self.path("workspace"))
            .arg("--extension-root")
            .arg(self.path("extension"))
            .args(["--profile", PROFILE, "--preset", "example"])
            .args(["--base-url", url, "--allow", "read,write"])
            .stdin(Stdio::null());
        command
    }

    fn check(&self, url: &str) -> (Option<i32>, Value) {
        let output = self
            .command(url)
            .args(["--operator-check", "--json"])
            .output()
            .unwrap();
        let report: Value = serde_json::from_slice(&output.stdout)
            .unwrap_or_else(|error| panic!("{error}: {}", said(&output)));
        assert_eq!(report["schema_version"], "commandagent.operator-check/v1");
        (output.status.code(), report)
    }
}

fn check_of<'a>(report: &'a Value, id: &str) -> &'a Value {
    report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["id"] == id)
        .unwrap_or_else(|| panic!("no check {id} in {report:#}"))
}

fn template(tool: &str) -> String {
    std::fs::read_to_string(sample().join(format!("operator/profile-tools/{tool}.json"))).unwrap()
}

/// The guide runs every command through `ca`, which gives `commandagent` the caller's `PATH` and
/// exactly `RUN_ENV`, so a user's own `~/.commandagent/config.toml` and stray `COMMANDAGENT_*`
/// variables stay out of the rehearsal, as they stay out of these tests. Both languages show the
/// same wrapper, and every command in the guide goes through it.
#[test]
fn the_tutorial_runs_commandagent_the_way_these_tests_do() {
    let mut wrapper = String::from("ca() {\n  env -i PATH=\"$PATH\" \\\n");
    for (name, value) in RUN_ENV {
        wrapper.push_str(&format!("    {name}=\"{value}\" \\\n"));
    }
    wrapper.push_str("    commandagent \"$@\"\n}\n");
    for language in ["en", "ja"] {
        let text = std::fs::read_to_string(tutorial(language)).unwrap();
        assert!(
            text.contains(&wrapper),
            "{language}: the tutorial's wrapper differs from\n{wrapper}"
        );
        let mut in_bash = false;
        for line in text.lines() {
            match line.trim_start() {
                "```bash" => in_bash = true,
                "```" => in_bash = false,
                "commandagent \"$@\"" => {}
                command if in_bash && command.starts_with("commandagent") => {
                    panic!("{language}: `{command}` runs commandagent without `ca`")
                }
                _ => {}
            }
        }
    }
}

/// The one `bash` block of the tutorial in `language` that holds `needle`, as the reader copies it.
fn bash_block(language: &str, needle: &str) -> String {
    let text = std::fs::read_to_string(tutorial(language)).unwrap();
    let mut blocks = Vec::new();
    let mut current: Option<String> = None;
    for line in text.lines() {
        match (line.trim_start(), current.as_mut()) {
            ("```bash", None) => current = Some(String::new()),
            ("```", Some(_)) => blocks.push(current.take().unwrap()),
            (_, Some(block)) => {
                block.push_str(line);
                block.push('\n');
            }
            _ => {}
        }
    }
    let found: Vec<String> = blocks
        .into_iter()
        .filter(|block| block.contains(needle))
        .collect();
    assert_eq!(found.len(), 1, "{language}: blocks holding `{needle}`");
    found.into_iter().next().unwrap()
}

/// "Try a script by hand": the guide's hand-made request line, run outside the host against a
/// fresh layout, gets the stage's result with its check passed and the expected file.
#[test]
fn the_tutorials_hand_request_runs_the_stage_script() {
    let operator = Operator::new();
    for language in ["en", "ja"] {
        let block = bash_block(language, "\"outputs\":[\"items\"]");
        let output = Command::new("/bin/bash")
            .arg("-c")
            .arg(&block)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("LC_ALL", "C")
            .env("ROOT", &operator.root)
            .output()
            .unwrap();
        assert!(output.status.success(), "{language}: {}", said(&output));
        let result: Value = serde_json::from_slice(&output.stdout)
            .unwrap_or_else(|error| panic!("{language}: {error}: {}", said(&output)));
        assert_eq!(result["protocol"], "profile-tool.result/v1");
        assert_eq!(
            result["checks"],
            json!([{"id": "lines", "status": "passed"}])
        );
        assert_eq!(result["outputs"][0]["sha256"], sha256(ITEMS), "{language}");
        assert_eq!(
            result["outputs"][0]["bytes_base64"],
            encode_base64(ITEMS),
            "{language}"
        );
    }
}

/// The descriptors decode as the host's own type, ship a real schema pin, pin `/bin/bash` and the
/// one tools directory, and leave the two machine-specific pins as zeros for the operator.
#[test]
fn the_descriptor_templates_decode_and_ship_a_real_schema_pin() {
    for (tool, script, output) in [
        ("example-parse", "parse.sh", "work/items.json"),
        ("example-check", "check.sh", "out/report.json"),
    ] {
        let text = template(tool);
        let descriptor: Descriptor =
            serde_json::from_str(&text).unwrap_or_else(|error| panic!("{tool}: {error}"));
        assert_eq!(descriptor.protocol, "profile-tool.descriptor/v1");
        assert_eq!(descriptor.tool_id, tool);
        assert_eq!(descriptor.effect, "propose-artifacts");
        assert_eq!(descriptor.runtime.kind, "pinned_process");
        let schema = descriptor.schema.as_ref().expect("a schema");
        assert_eq!(
            descriptor.schema_sha256,
            sha256(serde_json::to_string(schema).unwrap().as_bytes()),
            "{tool}: the shipped schema pin is the digest of the compact schema"
        );
        assert_eq!(
            schema["root"]["required"],
            json!(["expected_revision"]),
            "{tool}"
        );
        let process = descriptor.process.as_ref().expect("a process");
        assert_eq!(process.executable.path, Path::new("/bin/bash"));
        assert_eq!(process.executable.sha256, "0".repeat(64));
        assert_eq!(process.dependency_directories.len(), 1);
        assert_eq!(
            process.dependency_directories[0].path,
            Path::new("@ROOT@/tools")
        );
        assert_eq!(process.dependency_directories[0].sha256, "0".repeat(64));
        assert_eq!(process.argv, [format!("@ROOT@/tools/{script}")]);
        assert_eq!(process.cwd, Path::new("@ROOT@/tools"));
        assert_eq!(process.outputs.len(), 1);
        assert_eq!(process.outputs[0].path, output);
        let floor = descriptor.stage.as_ref().expect("a stage floor");
        assert!(!floor.required_checks.is_empty(), "{tool}");
    }
}

/// The manifest is a valid external profile, and declares the two tools as two stages.
#[test]
fn the_manifest_declares_the_two_tools_as_two_stages() {
    let manifest = sample().join("extension/profiles/two-stage-example/manifest.toml");
    let output = Command::new(env!("CARGO_BIN_EXE_commandagent"))
        .arg("--validate-manifest")
        .arg(&manifest)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", said(&output));
    let value: toml::Value = toml::from_str(&std::fs::read_to_string(&manifest).unwrap()).unwrap();
    let tools = &value["profile_tools"];
    assert_eq!(
        tools["schema_version"].as_str(),
        Some("profile-tool.declaration/v2")
    );
    let declared: Vec<&str> = tools["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["id"].as_str().unwrap())
        .collect();
    assert_eq!(declared, TOOLS);
    let staged: Vec<&str> = tools["stages"]["stages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|stage| stage["targets"][0]["tool_id"].as_str().unwrap())
        .collect();
    assert_eq!(staged, TOOLS);
}

/// `setup.sh` writes the root's canonical path wherever a template says `@ROOT@`, and the binary
/// reads the run budget it laid out and can count the preset's route. The pins are still zeros,
/// so the profile tools fail (on macOS as `invalid_pin`; elsewhere the pinned process cannot run
/// at all), and the check does not pass.
#[test]
fn setup_lays_out_a_root_whose_run_budget_the_binary_accepts() {
    let operator = Operator::new();
    for file in [
        "operator/profile-tools/example-parse.json",
        "operator/profile-tools/example-check.json",
        "operator/run-budgets/example-wallet.json",
    ] {
        let text = std::fs::read_to_string(operator.path(file)).unwrap();
        assert!(!text.contains("@ROOT@"), "{file}");
    }
    let policy: Value = serde_json::from_slice(
        &std::fs::read(operator.path("operator/run-budgets/example-wallet.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        policy["ledger_dir"].as_str().map(PathBuf::from),
        Some(operator.path("ledgers"))
    );
    let (code, report) = operator.check(UNUSED_URL);
    assert_eq!(code, Some(1), "{report:#}");
    assert_eq!(report["status"], "fail");
    for id in ["run_budget.declaration", "run_budget.route.executor"] {
        assert_eq!(check_of(&report, id)["status"], "pass", "{id}: {report:#}");
    }
    assert_eq!(check_of(&report, "profile_tools")["status"], "fail");
    if cfg!(target_os = "macos") {
        assert_eq!(check_of(&report, "profile_tools")["code"], "invalid_pin");
    }
}

/// The bash helpers the tools use, against `sha2` and a plain encoder: the edge sizes of SHA-256
/// padding (0, 55, 56, 64 bytes) and a longer input, every byte value included, plus the known
/// vectors of FIPS 180-2.
#[test]
fn the_bash_helpers_match_sha2_and_base64() {
    let library = sample().join("tools/lib.sh");
    let run = |bytes: &[u8]| -> (String, String) {
        let output = Command::new("/bin/bash")
            .arg("-c")
            .arg(
                "source \"$1\"; b64_decode \"$2\" || exit 9; sha256_bytes; b64_encode; \
                 printf '%s %s' \"$DIGEST\" \"$B64\"",
            )
            .arg("lib-test")
            .arg(&library)
            .arg(encode_base64(bytes))
            .env_clear()
            .env("LC_ALL", "C")
            .output()
            .unwrap();
        assert!(output.status.success(), "{}", said(&output));
        let text = String::from_utf8(output.stdout).unwrap();
        let (digest, encoded) = text.split_once(' ').unwrap_or((text.as_str(), ""));
        (digest.to_string(), encoded.to_string())
    };
    for size in [0usize, 1, 2, 3, 55, 56, 63, 64, 65, 1000] {
        let bytes: Vec<u8> = (0..size).map(|index| (index * 37 + 11) as u8).collect();
        let (digest, encoded) = run(&bytes);
        assert_eq!(digest, sha256(&bytes), "{size} bytes");
        assert_eq!(encoded, encode_base64(&bytes), "{size} bytes");
    }
    for (text, digest) in [
        (
            "",
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        ),
        (
            "abc",
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        ),
        (
            "abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq",
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1",
        ),
    ] {
        assert_eq!(run(text.as_bytes()).0, digest, "{text:?}");
    }

    // Bytes to text and back, as the tools read their input and write their output: every byte
    // value but NUL survives, and NUL is refused rather than dropped.
    let round_trip = |bytes: &[u8]| {
        Command::new("/bin/bash")
            .arg("-c")
            .arg(
                "source \"$1\"; b64_decode \"$2\" || exit 9; bytes_to_text || exit 8; \
                 text_to_bytes \"$TEXT\"; b64_encode; printf '%s' \"$B64\"",
            )
            .arg("lib-test")
            .arg(&library)
            .arg(encode_base64(bytes))
            .env_clear()
            .env("LC_ALL", "C")
            .output()
            .unwrap()
    };
    let every: Vec<u8> = (1..=255).collect();
    let output = round_trip(&every);
    assert!(output.status.success(), "{}", said(&output));
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        encode_base64(&every)
    );
    assert_eq!(round_trip(b"a\0b").status.code(), Some(8));
}

/// One fake model, started as the guide starts it. Killed when dropped.
struct Fake {
    child: std::process::Child,
    url: String,
    port: u16,
    record: PathBuf,
}

impl Fake {
    fn start(rehearse: &Path, record: PathBuf) -> Self {
        let mut child = Command::new("python3")
            .arg("-I")
            .arg(rehearse.join("fake_model.py"))
            .arg("--replies")
            .arg(rehearse.join("replies.json"))
            .args(["--port", "0", "--record"])
            .arg(&record)
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .stdin(Stdio::null())
            .spawn()
            .expect("python3 runs the fake model");
        let mut stdout = child.stdout.take().unwrap();
        let mut line = Vec::new();
        let mut byte = [0u8; 1];
        while stdout.read(&mut byte).unwrap() == 1 && byte[0] != b'\n' {
            line.push(byte[0]);
        }
        let line = String::from_utf8(line).unwrap();
        let url = line
            .strip_prefix("listening ")
            .unwrap_or_else(|| panic!("the fake said {line:?}"))
            .to_string();
        let port = url
            .trim_start_matches("http://127.0.0.1:")
            .trim_end_matches("/v1")
            .parse()
            .unwrap();
        Self {
            child,
            url,
            port,
            record,
        }
    }

    fn entries(&self) -> Vec<Value> {
        std::fs::read_to_string(&self.record)
            .unwrap_or_default()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    fn connections(&self) -> usize {
        self.entries()
            .iter()
            .filter(|entry| entry["event"] == "connection")
            .count()
    }

    fn bodies(&self) -> Vec<Value> {
        self.entries()
            .into_iter()
            .filter(|entry| entry["event"] == "request")
            .map(|entry| entry["body"].clone())
            .collect()
    }

    /// One raw HTTP exchange with the fake.
    fn exchange(&self, request: &str) -> String {
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).unwrap();
        stream.write_all(request.as_bytes()).unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        response
    }

    fn chat(&self, body: &Value) -> Value {
        let body = body.to_string();
        let response = self.exchange(&format!(
            "POST /v1/chat/completions HTTP/1.1\r\nhost: 127.0.0.1\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{body}",
            body.len()
        ));
        let (_, json) = response.split_once("\r\n\r\n").unwrap();
        serde_json::from_str(json).unwrap()
    }

    /// The fake takes connections one at a time, in order, so once this one is recorded every
    /// connection made before it is recorded too. Returns the count before this one.
    fn connections_before_a_sentinel(&self) -> usize {
        let response = self.exchange("GET /sentinel HTTP/1.1\r\nhost: 127.0.0.1\r\n\r\n");
        assert!(response.starts_with("HTTP/1.1 404"), "{response}");
        self.connections() - 1
    }
}

impl Drop for Fake {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The fake answers as its documentation says: the script in order, `{revision}` from the host's
/// last answer, a `stale` answer repeated with its revision, and a billed cost on every reply.
#[test]
fn the_fake_model_follows_its_script_and_the_hosts_revisions() {
    let temp = tempfile::tempdir().unwrap();
    let fake = Fake::start(&sample().join("rehearse"), temp.path().join("record.jsonl"));
    // The URL it prints is the one `--base-url` takes.
    assert_eq!(fake.url, format!("http://127.0.0.1:{}/v1", fake.port));
    let tool_answer = |answer: Value| {
        json!({"model": "example-model", "messages": [
            {"role": "user", "content": "go"},
            {"role": "tool", "tool_call_id": "call-1", "content": answer.to_string()},
        ]})
    };
    let call = |reply: &Value| {
        let call = &reply["choices"][0]["message"]["tool_calls"][0]["function"];
        let arguments: Value = serde_json::from_str(call["arguments"].as_str().unwrap()).unwrap();
        (call["name"].as_str().unwrap().to_string(), arguments)
    };

    let first = fake
        .chat(&json!({"model": "example-model", "messages": [{"role": "user", "content": "go"}]}));
    assert_eq!(first["usage"]["cost"], json!(0.0001), "{first}");
    assert!(first["usage"]["prompt_tokens"].as_u64().unwrap() > 0);
    assert_eq!(
        call(&first),
        (
            "ExampleParse".to_string(),
            json!({"expected_revision": "initial"})
        )
    );
    let stale = fake.chat(&tool_answer(
        json!({"status": "stale", "revision": "rev-from-stale"}),
    ));
    assert_eq!(
        call(&stale),
        (
            "ExampleParse".to_string(),
            json!({"expected_revision": "rev-from-stale"})
        )
    );
    let second = fake.chat(&tool_answer(json!({"stage": {
        "status": "sealed", "downstream": [{"revision": "rev-below"}]
    }})));
    assert_eq!(
        call(&second),
        (
            "ExampleCheck".to_string(),
            json!({"expected_revision": "rev-below"})
        )
    );
    let last = fake.chat(&tool_answer(json!({"stage": {"status": "sealed"}})));
    assert!(
        last["choices"][0]["message"]["content"]
            .as_str()
            .unwrap()
            .contains("sealed"),
        "{last}"
    );
    assert_eq!(fake.bodies().len(), 4);
    assert_eq!(fake.connections_before_a_sentinel(), 4);
}

/// Words and values that belong to one user, one machine, or one vendor's product, and absolute
/// paths of a machine. Matched without regard to case, as whole words.
const DENIED: &[&str] = &[
    r"musubi",
    r"musunest",
    r"kewton",
    r"maenokota",
    r"nextjs",
    r"acme",
    r"cagentpack",
    r"commandmate",
    r"jev",
    r"glm",
    r"gemma",
    r"luna",
    r"terra",
    r"relace",
    r"openinference",
    r"open-inference",
    r"openrouter\.ai",
    r"claude",
    r"anthropic",
    r"codex",
    r"antigravity",
    r"gpt",
    r"qwen",
    r"deepseek",
    r"gmail",
    r"fable",
    r"opus",
    r"sonnet",
    r"haiku",
    r"gemini",
    r"mistral",
    r"llama",
    r"ollama",
    r"kimi",
    r"minimax",
    r"grok",
    r"groq",
    r"fireworks",
    r"together\.ai",
    r"togetherai",
    r"deepinfra",
    r"novita",
    r"cerebras",
    r"sk-[A-Za-z0-9]",
];

const DENIED_PATHS: &[&str] = &["/Users/", "/Volumes/", "/home/", "/private/", "/opt/"];

fn walk(directory: &Path, files: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            walk(&path, files);
        } else {
            files.push(path);
        }
    }
}

/// The sections this example added to the reference page, from `start` up to `end`. The rest of
/// the page belongs to other features and may name a real endpoint.
fn reference_sections(language: &str, start: &str, end: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("docs/guide/{language}/profile-tools.md"));
    let text = std::fs::read_to_string(&path).unwrap();
    let from = text
        .find(start)
        .unwrap_or_else(|| panic!("{path:?} has no `{start}`"));
    let to = text[from..]
        .find(end)
        .unwrap_or_else(|| panic!("{path:?} has no `{end}` after `{start}`"));
    let sections = text[from..from + to].to_string();
    assert!(sections.contains("profile-tool.result/v1"), "{sections}");
    sections
}

/// Acceptance 2: nothing in the sample, the tutorial, or the reference sections it added names a
/// user, a machine, or a real model, vendor, or upstream. `openrouter_provider` is CommandAgent's
/// own configuration key and stays allowed.
#[test]
fn the_sample_and_the_tutorial_name_no_user_and_no_machine() {
    let mut files = Vec::new();
    walk(&sample(), &mut files);
    assert!(files.len() >= 13, "{files:?}");
    let mut texts: Vec<(String, String)> = files
        .iter()
        .map(|file| {
            let text =
                std::fs::read_to_string(file).unwrap_or_else(|error| panic!("{file:?}: {error}"));
            (format!("{file:?}"), text)
        })
        .collect();
    for language in ["en", "ja"] {
        let path = tutorial(language);
        texts.push((format!("{path:?}"), std::fs::read_to_string(&path).unwrap()));
    }
    for (language, start, end) in [
        ("en", "## Structured Stages", "## Run Budget"),
        ("ja", "## structured stages", "## run budget"),
    ] {
        texts.push((
            format!("{language}/profile-tools.md `{start}` to `{end}`"),
            reference_sections(language, start, end),
        ));
    }
    let words = regex::Regex::new(&format!(r"(?i)\b(?:{})", DENIED.join("|"))).unwrap();
    for (name, text) in &texts {
        if let Some(found) = words.find(text) {
            panic!("{name} names `{}`", found.as_str());
        }
        for path in DENIED_PATHS {
            assert!(
                !text.contains(path),
                "{name} holds the absolute path {path}"
            );
        }
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::*;

    const REPORT: &[u8] = b"{\"items\":3,\"total\":15}\n";

    impl Operator {
        /// The documented step: copy each `calculated` pin the check shows into the descriptor.
        fn copy_pins(&self, report: &Value) {
            for tool in TOOLS {
                let path = self.path(&format!("operator/profile-tools/{tool}.json"));
                let mut descriptor: Value =
                    serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
                let mut copied = 0;
                for pin in report["pins"].as_array().unwrap() {
                    if pin["tool_id"] != tool {
                        continue;
                    }
                    let calculated = pin["calculated"].clone();
                    assert!(calculated.is_string(), "{pin:#}");
                    match pin["pin"].as_str().unwrap() {
                        "schema_sha256" => assert_eq!(pin["declared"], calculated, "{pin:#}"),
                        "executable" => {
                            assert_eq!(pin["declared"], "0".repeat(64));
                            descriptor["process"]["executable"]["sha256"] = calculated;
                            copied += 1;
                        }
                        "dependency_directory" => {
                            assert_eq!(pin["declared"], "0".repeat(64));
                            descriptor["process"]["dependency_directories"][0]["sha256"] =
                                calculated;
                            copied += 1;
                        }
                        _ => {}
                    }
                }
                assert_eq!(copied, 2, "{tool}: {report:#}");
                std::fs::write(&path, serde_json::to_string_pretty(&descriptor).unwrap()).unwrap();
            }
        }

        /// Step 5's "check the values yourself": `shasum -a 256 /bin/bash` and the guide's
        /// directory recipe, in both languages, give the `calculated` pins the check shows.
        fn pins_match_the_guides_recipes(&self, report: &Value) {
            let calculated = |kind: &str| -> Vec<String> {
                let mut values: Vec<String> = report["pins"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|pin| pin["pin"] == kind)
                    .map(|pin| pin["calculated"].as_str().unwrap().to_string())
                    .collect();
                values.dedup();
                values
            };
            let run = |script: &str| -> String {
                let output = Command::new("/bin/bash")
                    .arg("-c")
                    .arg(script)
                    .env_clear()
                    .env("PATH", "/usr/bin:/bin")
                    .env("ROOT", &self.root)
                    .output()
                    .unwrap();
                assert!(output.status.success(), "{}", said(&output));
                String::from_utf8(output.stdout).unwrap()[..64].to_string()
            };
            assert_eq!(
                calculated("executable"),
                [run("shasum -a 256 /bin/bash")],
                "{report:#}"
            );
            for language in ["en", "ja"] {
                let recipe = bash_block(language, "shopt -s dotglob");
                assert_eq!(
                    calculated("dependency_directory"),
                    [run(&recipe)],
                    "{language}: {report:#}"
                );
            }
        }

        /// From a fresh layout to a root the check passes, with no connection to the fake.
        fn pinned(&self, fake: &Fake) {
            let (code, report) = self.check(&fake.url);
            assert_eq!(code, Some(1), "{report:#}");
            let tools = check_of(&report, "profile_tools");
            assert_eq!(
                (&tools["status"], &tools["code"]),
                (&json!("fail"), &json!("invalid_pin")),
                "{report:#}"
            );
            self.pins_match_the_guides_recipes(&report);
            self.copy_pins(&report);
            let (code, report) = self.check(&fake.url);
            assert_eq!(code, Some(0), "{report:#}");
            assert_eq!(report["status"], "pass");
            for id in [
                "profile_tools",
                "isolation.probe_cleanup",
                "structured_stages",
                "run_budget.declaration",
                "run_budget.ledger",
                "run_budget.route.executor",
            ] {
                assert_eq!(check_of(&report, id)["status"], "pass", "{id}: {report:#}");
            }
            assert_eq!(fake.connections(), 0, "the check connected to the fake");
        }

        fn rehearse(&self, url: &str) -> Output {
            self.command(url)
                .args(["--summary-json", "--prompt", "Produce out/report.json."])
                .output()
                .unwrap()
        }
    }

    fn summary(output: &Output) -> Value {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let line = stdout
            .lines()
            .rfind(|line| line.starts_with('{'))
            .unwrap_or_else(|| panic!("no summary: {}", said(output)));
        serde_json::from_str(line).unwrap()
    }

    /// Acceptance 1, success: the documented path from a fresh layout to a rehearsal in which both
    /// stages are sealed and the wallet observed every send.
    #[test]
    fn the_example_passes_the_check_and_seals_both_stages_under_its_wallet() {
        let operator = Operator::new();
        let fake = Fake::start(&operator.path("rehearse"), operator.path("requests.jsonl"));
        operator.pinned(&fake);

        let output = operator.rehearse(&fake.url);
        let said = said(&output);
        assert_eq!(output.status.code(), Some(0), "{said}");
        assert_eq!(
            std::fs::read(operator.path("workspace/work/items.json")).unwrap(),
            ITEMS,
            "{said}"
        );
        assert_eq!(
            std::fs::read(operator.path("workspace/out/report.json")).unwrap(),
            REPORT,
            "{said}"
        );

        let summary = summary(&output);
        let stages = &summary["structured_stages"];
        assert_eq!(stages["record"], "observed", "{summary:#}");
        assert_eq!(stages["complete"], true, "{summary:#}");
        assert_eq!(stages["ledger_intact"], true, "{summary:#}");
        let stages = stages["stages"].as_array().unwrap();
        assert_eq!(stages.len(), 2, "{summary:#}");
        for (stage, (name, path, bytes)) in stages.iter().zip([
            ("parse", "work/items.json", ITEMS),
            ("check", "out/report.json", REPORT),
        ]) {
            assert_eq!(stage["stage"], name, "{summary:#}");
            assert_eq!(stage["sealed"], true, "{summary:#}");
            assert_eq!(stage["phase"], "sealed", "{summary:#}");
            assert_eq!(stage["artifact"]["path"], path, "{summary:#}");
            assert_eq!(
                stage["artifact"]["committed_sha256"],
                sha256(bytes),
                "{summary:#}"
            );
        }

        let bodies = fake.bodies();
        let budget = &summary["run_budget"];
        assert_eq!(budget["record"], "observed", "{summary:#}");
        assert_eq!(budget["accounting_basis"], "provider_billed", "{summary:#}");
        assert_eq!(budget["budget_id"], WALLET, "{summary:#}");
        assert_eq!(budget["stop"], Value::Null, "{summary:#}");
        assert_eq!(budget["latch"], Value::Null, "{summary:#}");
        assert_eq!(budget["sends"]["unknown"], 0, "{summary:#}");
        assert_eq!(budget["sends"]["settled"], bodies.len(), "{summary:#}");
        assert_eq!(bodies.len(), 3, "{bodies:#?}");
        for body in &bodies {
            assert_eq!(body["model"], "example-model");
            assert_eq!(body["reasoning_effort"], "low");
            assert_eq!(
                body["provider"],
                json!({"only": ["example-upstream"], "allow_fallbacks": false})
            );
        }

        // The guide's way to rehearse again: stop the fake and run step 6 from its start, in the
        // same directory. The run exits 0 again and the wallet keeps counting.
        drop(fake);
        let again = Fake::start(
            &operator.path("rehearse"),
            operator.path("requests-again.jsonl"),
        );
        let output = operator.rehearse(&again.url);
        let said_again = super::said(&output);
        assert_eq!(output.status.code(), Some(0), "{said_again}");
        assert_eq!(
            std::fs::read(operator.path("workspace/out/report.json")).unwrap(),
            REPORT,
            "{said_again}"
        );
        let second = self::summary(&output);
        assert_eq!(second["structured_stages"]["complete"], true, "{second:#}");
        let budget = &second["run_budget"];
        assert_eq!(budget["stop"], Value::Null, "{second:#}");
        assert_eq!(
            budget["sends"]["settled"],
            bodies.len() + again.bodies().len(),
            "{second:#}"
        );
    }

    /// Acceptance 1, refusal: one byte appended to a pinned script. The check fails as
    /// `invalid_pin`, and the run is refused before its start with exit 2 and no connection to the
    /// fake.
    #[test]
    fn a_changed_pinned_script_is_refused_before_the_start_with_no_connection() {
        let operator = Operator::new();
        let fake = Fake::start(&operator.path("rehearse"), operator.path("requests.jsonl"));
        operator.pinned(&fake);

        let script = operator.path("tools/check.sh");
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&script)
            .unwrap();
        file.write_all(b"\n").unwrap();
        drop(file);

        let (code, report) = operator.check(&fake.url);
        assert_eq!(code, Some(1), "{report:#}");
        let tools = check_of(&report, "profile_tools");
        assert_eq!(tools["status"], "fail", "{report:#}");
        assert_eq!(tools["code"], "invalid_pin", "{report:#}");

        let output = operator.rehearse(&fake.url);
        let said = said(&output);
        assert_eq!(output.status.code(), Some(2), "{said}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        let error = stderr
            .lines()
            .rfind(|line| line.starts_with("error"))
            .unwrap_or("");
        assert!(
            error.contains("profile_tool_refused: invalid_pin"),
            "{said}"
        );
        assert_eq!(fake.connections_before_a_sentinel(), 0, "{said}");
        assert!(!operator.path("workspace/work/items.json").exists());
        assert!(!operator.path("workspace/out/report.json").exists());
    }
}
