//! Fake-process tests for `artifact_contract_check`.
//!
//! Every adapter here is a synthetic POSIX shell script run by the
//! operator-pinned `/bin/sh`. Two distinct adapters share the host path:
//! `tally` counts text lines under its own opaque IDs, and `ledger` reports
//! canned multi-target rows for the truth table and the failure modes. Every
//! file lives in a temporary directory; nothing touches the real workspace,
//! network, model, or credentials.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::protocol::{
    AdapterResult, Counts, DomainVerdict, Expectation, RowFacts, RowKey, RowStatus, cap_domain,
    counts_agree, parse_request, parse_result, validate_result,
};
use super::{Receipt, admit, run_check};
use crate::planner::capability_catalog::artifact::ArtifactContractCheck;

const SHARED_PRELUDE: &str = r#"IFS= read -r envelope || :
field() { value=${envelope#*\"$1\":\"}; printf '%s' "${value%%\"*}"; }
request_id=$(field request_id)
nonce=$(field nonce)
workspace=$(field workspace)
digests=""
rest=$envelope
last_id=""
last_path=""
while :; do
  case $rest in *'"input_id":"'*) ;; *) break ;; esac
  rest=${rest#*\"input_id\":\"}; last_id=${rest%%\"*}
  rest=${rest#*\"path\":\"}; last_path=${rest%%\"*}
  rest=${rest#*\"sha256\":\"}; sha=${rest%%\"*}
  digests="$digests${digests:+,}{\"input_id\":\"$last_id\",\"sha256\":\"$sha\"}"
done
"#;

const LEDGER_BODY: &str = r#"mode=$1
rows_file=$2
code=$3
observe=$4
rows=""
while IFS= read -r line || [ -n "$line" ]; do rows="$rows$line"; done < "$rows_file"
case $mode in
  env) export -p > "$observe/env.txt" ;;
  mutate) printf 'drift\n' >> "$workspace/$last_path" ;;
  tamper) printf 'tampered\n' >> "$rows_file" ;;
  fifo) /bin/rm -f "$workspace/$last_path"; /usr/bin/mkfifo "$workspace/$last_path" ;;
  relink) /bin/rm -f "$workspace/$last_path"; /bin/ln -s "$workspace/input/spec.txt" "$workspace/$last_path" ;;
  relink_same) /bin/rm -f "$workspace/$last_path"; /bin/ln -s "$observe/same-decl.txt" "$workspace/$last_path" ;;
  relink_dir) /bin/mv "$workspace/input" "$observe/moved-input"; /bin/ln -s "$observe/moved-input" "$workspace/input" ;;
  sleep) /bin/sleep 30 ;;
  grandchild) ( while :; do printf x >> "$observe/beat"; /bin/sleep 0.05; done ) & /bin/sleep 30 ;;
  flood) while :; do printf '%s' 'xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx'; done ;;
  garbage) printf 'PASS all checks\n'; exit "$code" ;;
  stale_nonce) nonce=0190aaaa-0000-7000-8000-000000000000 ;;
  stale_request) request_id=previous-request ;;
  stale_digest) digests="{\"input_id\":\"spec\",\"sha256\":\"0000000000000000000000000000000000000000000000000000000000000000\"},{\"input_id\":\"decl\",\"sha256\":\"0000000000000000000000000000000000000000000000000000000000000000\"}" ;;
  *) ;;
esac
printf '{"protocol":"artifact-contract.result/v1","request_id":"%s","nonce":"%s","input_digests":[%s],"checks":%s}\n' "$request_id" "$nonce" "$digests" "$rows"
exit "$code"
"#;

const TALLY_BODY: &str = r#"expected=$1
count=0
while IFS= read -r line || [ -n "$line" ]; do count=$((count + 1)); done < "$workspace/$last_path"
if [ "$count" -eq "$expected" ]; then status=passed; ok=1; bad=0; code=0; else status=failed; ok=0; bad=1; code=1; fi
printf '{"protocol":"artifact-contract.result/v1","request_id":"%s","nonce":"%s","input_digests":[%s],"checks":[{"check_id":"q9:lines","target_id":"doc#1","status":"%s","counts":{"expected":1,"executed":1,"passed":%s,"failed":%s,"unresolved":0},"diagnostics":[{"code":"line-total","message":"%s lines"}]}]}\n' "$request_id" "$nonce" "$digests" "$status" "$ok" "$bad" "$count"
exit "$code"
"#;

const PASS_ALL: &str = r#"[{"check_id":"k1","target_id":"t-a","status":"passed","counts":{"expected":1,"executed":1,"passed":1,"failed":0,"unresolved":0}},{"check_id":"k1","target_id":"t-b","status":"passed","counts":{"expected":2,"executed":2,"passed":2,"failed":0,"unresolved":0}},{"check_id":"k2","target_id":null,"status":"passed","counts":{"expected":3,"executed":3,"passed":3,"failed":0,"unresolved":0}},{"check_id":"rv","target_id":null,"status":"passed","counts":{"expected":1,"executed":1,"passed":1,"failed":0,"unresolved":0}}]"#;
const REVIEW_NOT_PROVIDED: &str = r#"[{"check_id":"k1","target_id":"t-a","status":"passed","counts":{"expected":1,"executed":1,"passed":1,"failed":0,"unresolved":0}},{"check_id":"k1","target_id":"t-b","status":"passed","counts":{"expected":2,"executed":2,"passed":2,"failed":0,"unresolved":0}},{"check_id":"k2","target_id":null,"status":"passed","counts":{"expected":3,"executed":3,"passed":3,"failed":0,"unresolved":0}},{"check_id":"rv","target_id":null,"status":"not_provided","counts":{"expected":null,"executed":0,"passed":0,"failed":0,"unresolved":0}}]"#;
const REQUIRED_UNRESOLVED: &str = r#"[{"check_id":"k1","target_id":"t-a","status":"passed","counts":{"expected":1,"executed":1,"passed":1,"failed":0,"unresolved":0}},{"check_id":"k1","target_id":"t-b","status":"passed","counts":{"expected":2,"executed":2,"passed":2,"failed":0,"unresolved":0}},{"check_id":"k2","target_id":null,"status":"unresolved","counts":{"expected":3,"executed":3,"passed":2,"failed":0,"unresolved":1}}]"#;
const MISMATCH_AND_UNRESOLVED: &str = r#"[{"check_id":"k1","target_id":"t-a","status":"failed","counts":{"expected":1,"executed":1,"passed":0,"failed":1,"unresolved":0}},{"check_id":"k1","target_id":"t-b","status":"passed","counts":{"expected":2,"executed":2,"passed":2,"failed":0,"unresolved":0}},{"check_id":"k2","target_id":null,"status":"unresolved","counts":{"expected":3,"executed":3,"passed":2,"failed":0,"unresolved":1}}]"#;
const STATIC_FAILED: &str = r#"[{"check_id":"k1","target_id":"t-a","status":"failed","counts":{"expected":1,"executed":1,"passed":0,"failed":1,"unresolved":0}},{"check_id":"k1","target_id":"t-b","status":"not_run","counts":{"expected":null,"executed":0,"passed":0,"failed":0,"unresolved":0}},{"check_id":"k2","target_id":null,"status":"not_run","counts":{"expected":null,"executed":0,"passed":0,"failed":0,"unresolved":0}}]"#;
const REQUIRED_NOT_RUN: &str = r#"[{"check_id":"k1","target_id":"t-a","status":"passed","counts":{"expected":1,"executed":1,"passed":1,"failed":0,"unresolved":0}},{"check_id":"k1","target_id":"t-b","status":"passed","counts":{"expected":2,"executed":2,"passed":2,"failed":0,"unresolved":0}},{"check_id":"k2","target_id":null,"status":"not_run","counts":{"expected":null,"executed":0,"passed":0,"failed":0,"unresolved":0}}]"#;
const MISSING_TARGET: &str = r#"[{"check_id":"k1","target_id":"t-a","status":"passed","counts":{"expected":1,"executed":1,"passed":1,"failed":0,"unresolved":0}},{"check_id":"k2","target_id":null,"status":"passed","counts":{"expected":3,"executed":3,"passed":3,"failed":0,"unresolved":0}}]"#;
const DUPLICATE_ROW: &str = r#"[{"check_id":"k1","target_id":"t-a","status":"passed","counts":{"expected":1,"executed":1,"passed":1,"failed":0,"unresolved":0}},{"check_id":"k1","target_id":"t-b","status":"passed","counts":{"expected":2,"executed":2,"passed":2,"failed":0,"unresolved":0}},{"check_id":"k2","target_id":null,"status":"passed","counts":{"expected":3,"executed":3,"passed":3,"failed":0,"unresolved":0}},{"check_id":"k2","target_id":null,"status":"passed","counts":{"expected":3,"executed":3,"passed":3,"failed":0,"unresolved":0}}]"#;
const COUNT_MISMATCH: &str = r#"[{"check_id":"k1","target_id":"t-a","status":"passed","counts":{"expected":1,"executed":1,"passed":1,"failed":0,"unresolved":0}},{"check_id":"k1","target_id":"t-b","status":"passed","counts":{"expected":2,"executed":2,"passed":2,"failed":0,"unresolved":0}},{"check_id":"k2","target_id":null,"status":"passed","counts":{"expected":3,"executed":3,"passed":2,"failed":0,"unresolved":0}}]"#;
const ZERO_AS_SUCCESS: &str = r#"[{"check_id":"k1","target_id":"t-a","status":"passed","counts":{"expected":1,"executed":1,"passed":1,"failed":0,"unresolved":0}},{"check_id":"k1","target_id":"t-b","status":"passed","counts":{"expected":2,"executed":2,"passed":2,"failed":0,"unresolved":0}},{"check_id":"k2","target_id":null,"status":"passed","counts":{"expected":null,"executed":0,"passed":0,"failed":0,"unresolved":0}}]"#;
const UNKNOWN_ROW: &str = r#"[{"check_id":"k1","target_id":"t-a","status":"passed","counts":{"expected":1,"executed":1,"passed":1,"failed":0,"unresolved":0}},{"check_id":"k1","target_id":"t-b","status":"passed","counts":{"expected":2,"executed":2,"passed":2,"failed":0,"unresolved":0}},{"check_id":"k2","target_id":null,"status":"passed","counts":{"expected":3,"executed":3,"passed":3,"failed":0,"unresolved":0}},{"check_id":"zz","target_id":null,"status":"passed","counts":{"expected":1,"executed":1,"passed":1,"failed":0,"unresolved":0}}]"#;

const SPEC_V1: &str = "spec version one\n";
const SPEC_V2: &str = "spec version two\n";
const DECL: &str = "alpha\nbeta\ngamma\n";

struct Harness {
    _temp: tempfile::TempDir,
    config: PathBuf,
    tooling: PathBuf,
    workspace: PathBuf,
    observe: PathBuf,
}

impl Harness {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let base = fs::canonicalize(temp.path()).unwrap();
        let config = base.join("config");
        let tooling = base.join("tooling");
        let workspace = base.join("workspace");
        let observe = base.join("observe");
        for dir in [&config, &tooling, &workspace, &observe] {
            fs::create_dir_all(dir).unwrap();
        }
        fs::create_dir_all(config.join("adapters")).unwrap();
        fs::write(
            tooling.join("ledger.sh"),
            format!("{SHARED_PRELUDE}{LEDGER_BODY}"),
        )
        .unwrap();
        fs::write(
            tooling.join("tally.sh"),
            format!("{SHARED_PRELUDE}{TALLY_BODY}"),
        )
        .unwrap();
        for (name, rows) in [
            ("pass-all", PASS_ALL),
            ("review-not-provided", REVIEW_NOT_PROVIDED),
            ("required-unresolved", REQUIRED_UNRESOLVED),
            ("mismatch-and-unresolved", MISMATCH_AND_UNRESOLVED),
            ("static-failed", STATIC_FAILED),
            ("required-not-run", REQUIRED_NOT_RUN),
            ("missing-target", MISSING_TARGET),
            ("duplicate-row", DUPLICATE_ROW),
            ("count-mismatch", COUNT_MISMATCH),
            ("zero-as-success", ZERO_AS_SUCCESS),
            ("unknown-row", UNKNOWN_ROW),
            ("optional-omitted", REQUIRED_NOT_RUN.replace(
                r#"{"check_id":"k2","target_id":null,"status":"not_run","counts":{"expected":null,"executed":0,"passed":0,"failed":0,"unresolved":0}}"#,
                r#"{"check_id":"k2","target_id":null,"status":"passed","counts":{"expected":3,"executed":3,"passed":3,"failed":0,"unresolved":0}}"#,
            ).as_str()),
        ] {
            fs::create_dir_all(tooling.join("rows")).unwrap();
            fs::write(tooling.join(format!("rows/{name}.json")), rows).unwrap();
        }
        fs::create_dir_all(workspace.join("input")).unwrap();
        fs::write(workspace.join("input/spec.txt"), SPEC_V1).unwrap();
        fs::write(workspace.join("input/decl.txt"), DECL).unwrap();
        fs::write(
            workspace.join("input/request.json"),
            request_json("req-1", "input/spec.txt"),
        )
        .unwrap();
        Self {
            _temp: temp,
            config,
            tooling,
            workspace,
            observe,
        }
    }

    fn members(&self) -> BTreeMap<String, String> {
        let mut members = BTreeMap::new();
        let mut pending = vec![self.tooling.clone()];
        while let Some(dir) = pending.pop() {
            for entry in fs::read_dir(dir).unwrap() {
                let entry = entry.unwrap();
                let file_type = entry.file_type().unwrap();
                if file_type.is_dir() {
                    pending.push(entry.path());
                } else if file_type.is_file() {
                    let relative = entry
                        .path()
                        .strip_prefix(&self.tooling)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/");
                    members.insert(relative, sha256_file(&entry.path()));
                }
            }
        }
        members
    }

    /// Register the `ledger` adapter under `id` with fixed `args`.
    fn ledger(&self, id: &str, mode: &str, rows: &str, code: i32) -> ArtifactContractCheck {
        let observe = self.observe.to_string_lossy().to_string();
        // Only the timeout scenario uses a short bound; every other run keeps
        // the fixed 30 s ceiling so a busy machine cannot fake a failure.
        let timeout_seconds = if mode == "grandchild" { 1 } else { 30 };
        let descriptor = json!({
            "protocol": super::DESCRIPTOR_PROTOCOL,
            "adapter_id": id,
            "executable": {"path": shell(), "sha256": sha256_file(&shell())},
            "tooling_root": self.tooling,
            "entrypoint": "ledger.sh",
            "args": [mode, format!("rows/{rows}.json"), code.to_string(), observe],
            "members": self.members(),
            "required": [{"check_id": "k1", "targets": ["t-a", "t-b"]}, {"check_id": "k2"}],
            "optional": [{"check_id": "rv"}],
            "inputs": [
                {"input_id": "spec", "sha256": sha256_bytes(SPEC_V1.as_bytes())},
                {"input_id": "decl"}
            ],
            "limits": {"timeout_seconds": timeout_seconds, "max_stream_bytes": 24000}
        });
        self.register(id, &descriptor)
    }

    fn tally(&self, id: &str, expected_lines: usize) -> ArtifactContractCheck {
        let descriptor = json!({
            "protocol": super::DESCRIPTOR_PROTOCOL,
            "adapter_id": id,
            "executable": {"path": shell(), "sha256": sha256_file(&shell())},
            "tooling_root": self.tooling,
            "entrypoint": "tally.sh",
            "args": [expected_lines.to_string()],
            "members": self.members(),
            "required": [{"check_id": "q9:lines", "targets": ["doc#1"]}],
            "inputs": [{"input_id": "text"}]
        });
        fs::write(
            self.workspace.join("input/tally-request.json"),
            r#"{"protocol":"artifact-contract.request/v1","request_id":"tally-1","inputs":[{"input_id":"text","path":"input/decl.txt"}]}"#,
        )
        .unwrap();
        self.register(id, &descriptor);
        ArtifactContractCheck {
            adapter_id: id.to_string(),
            request: "input/tally-request.json".to_string(),
        }
    }

    fn register(&self, id: &str, descriptor: &Value) -> ArtifactContractCheck {
        fs::write(
            self.config.join(format!("adapters/{id}.json")),
            serde_json::to_vec_pretty(descriptor).unwrap(),
        )
        .unwrap();
        ArtifactContractCheck {
            adapter_id: id.to_string(),
            request: "input/request.json".to_string(),
        }
    }

    /// Register an edited copy of another adapter's descriptor under `id`.
    fn relabel(&self, id: &str, mut descriptor: Value) -> ArtifactContractCheck {
        descriptor["adapter_id"] = json!(id);
        self.register(id, &descriptor)
    }

    fn descriptor(&self, id: &str) -> Value {
        serde_json::from_slice(&fs::read(self.config.join(format!("adapters/{id}.json"))).unwrap())
            .unwrap()
    }

    fn run(&self, check: &ArtifactContractCheck) -> Receipt {
        run_check(&self.workspace, check, Some(&self.config), || false)
    }
}

fn request_json(request_id: &str, spec_path: &str) -> String {
    format!(
        r#"{{"protocol":"artifact-contract.request/v1","request_id":"{request_id}","inputs":[{{"input_id":"spec","path":"{spec_path}"}},{{"input_id":"decl","path":"input/decl.txt"}}]}}"#
    )
}

fn shell() -> PathBuf {
    fs::canonicalize("/bin/sh").unwrap()
}

fn sha256_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn sha256_file(path: &Path) -> String {
    sha256_bytes(&fs::read(path).unwrap())
}

fn assert_verified(receipt: &Receipt, native_passed: bool, domain: DomainVerdict) {
    assert_eq!(receipt.stopped_at, "verified", "{receipt:#?}");
    assert_eq!(receipt.native_passed, native_passed, "{receipt:#?}");
    assert_eq!(receipt.domain, domain, "{receipt:#?}");
    assert_ne!(
        receipt.domain,
        DomainVerdict::Full,
        "false full: {receipt:#?}"
    );
}

fn assert_failed_at(receipt: &Receipt, stopped_at: &str, reason: &str) {
    assert_eq!(receipt.stopped_at, stopped_at, "{receipt:#?}");
    assert!(!receipt.native_passed, "{receipt:#?}");
    assert_eq!(receipt.domain, DomainVerdict::None, "{receipt:#?}");
    assert!(
        receipt.reasons.iter().any(|actual| actual.contains(reason)),
        "expected reason containing {reason:?}: {receipt:#?}"
    );
    assert!(!receipt.failures().is_empty());
}

type Mutation = Box<dyn Fn(&mut Value)>;

fn rows(keys: &[(&str, Option<&str>)]) -> BTreeSet<RowKey> {
    keys.iter()
        .map(|(check, target)| (check.to_string(), target.map(str::to_string)))
        .collect()
}

fn expectation() -> Expectation {
    Expectation {
        request_id: "req-1".to_string(),
        nonce: "nonce-1".to_string(),
        input_digests: BTreeMap::from([("spec".to_string(), "a".repeat(64))]),
        required: rows(&[("k1", Some("t-a")), ("k2", None)]),
        optional: rows(&[("rv", None)]),
    }
}

fn result_json(rows: &str) -> String {
    format!(
        r#"{{"protocol":"artifact-contract.result/v1","request_id":"req-1","nonce":"nonce-1","input_digests":[{{"input_id":"spec","sha256":"{}"}}],"checks":{rows}}}"#,
        "a".repeat(64)
    )
}

fn row(check: &str, target: Option<&str>, status: &str, counts: [u64; 4]) -> String {
    let target = target.map_or("null".to_string(), |target| format!("\"{target}\""));
    format!(
        r#"{{"check_id":"{check}","target_id":{target},"status":"{status}","counts":{{"expected":{},"executed":{},"passed":{},"failed":{},"unresolved":{}}}}}"#,
        counts[0], counts[0], counts[1], counts[2], counts[3]
    )
}

fn validate(rows: &[String], exit_code: i32) -> Result<RowFacts, Vec<String>> {
    let result: AdapterResult =
        parse_result(result_json(&format!("[{}]", rows.join(","))).as_bytes())
            .expect("well-formed result");
    validate_result(&result, &expectation(), exit_code)
}

#[test]
fn count_identity_rejects_inconsistent_rows_and_zero_as_success() {
    let counts = |expected: Option<u64>, executed, passed, failed, unresolved| Counts {
        expected,
        executed,
        passed,
        failed,
        unresolved,
    };
    let accepted = [
        (RowStatus::Passed, counts(Some(2), 2, 2, 0, 0)),
        (RowStatus::Passed, counts(None, 1, 1, 0, 0)),
        (RowStatus::Failed, counts(Some(3), 3, 1, 1, 1)),
        (RowStatus::Unresolved, counts(Some(3), 3, 2, 0, 1)),
        (RowStatus::Unresolved, counts(Some(3), 2, 2, 0, 0)),
        (RowStatus::NotRun, counts(None, 0, 0, 0, 0)),
        (RowStatus::NotProvided, counts(Some(4), 0, 0, 0, 0)),
    ];
    for (status, counts) in accepted {
        assert_eq!(
            counts_agree(status, counts),
            Ok(()),
            "{status:?} {counts:?}"
        );
    }
    let rejected = [
        // An unexecuted check is never a success.
        (RowStatus::Passed, counts(None, 0, 0, 0, 0)),
        (RowStatus::Passed, counts(Some(0), 0, 0, 0, 0)),
        (RowStatus::Passed, counts(Some(3), 2, 2, 0, 0)),
        (RowStatus::Passed, counts(Some(3), 3, 2, 0, 0)),
        (RowStatus::Passed, counts(Some(3), 3, 2, 0, 1)),
        (RowStatus::Failed, counts(Some(3), 3, 3, 0, 0)),
        (RowStatus::Unresolved, counts(Some(3), 3, 3, 0, 0)),
        (RowStatus::Unresolved, counts(Some(3), 0, 0, 0, 0)),
        (RowStatus::Unresolved, counts(Some(3), 3, 1, 1, 1)),
        (RowStatus::NotRun, counts(None, 1, 1, 0, 0)),
        (RowStatus::Passed, counts(Some(1), 2, 2, 0, 0)),
        (RowStatus::Passed, counts(None, u64::MAX, u64::MAX, 1, 0)),
    ];
    for (status, counts) in rejected {
        assert!(
            counts_agree(status, counts).is_err(),
            "{status:?} {counts:?}"
        );
    }
}

#[test]
fn result_validation_rejects_identity_digest_coverage_and_exit_violations() {
    let k1 = row("k1", Some("t-a"), "passed", [1, 1, 0, 0]);
    let k2 = row("k2", None, "passed", [2, 2, 0, 0]);
    assert!(validate(&[k1.clone(), k2.clone()], 0).is_ok());

    let cases: Vec<(&str, Vec<String>, i32)> = vec![
        (
            "is reported more than once",
            vec![k1.clone(), k2.clone(), k2.clone()],
            0,
        ),
        ("required row `k2` is missing", vec![k1.clone()], 0),
        (
            "is not registered for this adapter",
            vec![
                k1.clone(),
                k2.clone(),
                row("zz", None, "passed", [1, 1, 0, 0]),
            ],
            0,
        ),
        (
            "required row `k2` may not be not_provided",
            vec![k1.clone(), row("k2", None, "not_provided", [0, 0, 0, 0])],
            1,
        ),
        (
            "disagrees with its reported rows",
            vec![k1.clone(), k2.clone()],
            1,
        ),
        (
            "disagrees with its reported rows",
            vec![k1.clone(), row("k2", None, "failed", [2, 1, 1, 0])],
            0,
        ),
        (
            "status disagrees with its counts",
            vec![k1.clone(), row("k2", None, "passed", [2, 1, 0, 1])],
            0,
        ),
    ];
    for (reason, rows, exit) in cases {
        let violations = validate(&rows, exit).expect_err(reason);
        assert!(
            violations
                .iter()
                .any(|violation| violation.contains(reason)),
            "{reason}: {violations:?}"
        );
    }

    let good = result_json(&format!("[{k1},{k2}]"));
    for (label, mutated, reason) in [
        (
            "stale request",
            good.replace("\"req-1\"", "\"req-0\""),
            "different request_id",
        ),
        (
            "stale nonce",
            good.replace("nonce-1", "nonce-0"),
            "different run nonce",
        ),
        (
            "stale digest",
            good.replace(&"a".repeat(64), &"b".repeat(64)),
            "input digests differ",
        ),
        (
            "protocol",
            good.replace("artifact-contract.result/v1", "artifact-contract.result/v0"),
            "result protocol",
        ),
    ] {
        let result = parse_result(mutated.as_bytes()).unwrap();
        let violations = validate_result(&result, &expectation(), 0).expect_err(label);
        assert!(
            violations
                .iter()
                .any(|violation| violation.contains(reason)),
            "{label}: {violations:?}"
        );
    }
}

#[test]
fn result_parser_rejects_non_json_depth_unknown_and_duplicate_fields() {
    assert!(parse_result(b"PASS").is_err());
    assert!(parse_result(b"").is_err());
    let deep = format!("{}{}", "[".repeat(17), "]".repeat(17));
    assert!(parse_result(deep.as_bytes()).unwrap_err().contains("depth"));
    let with_verdict = result_json("[]").replacen('{', r#"{"domain_verdict":"full","#, 1);
    assert!(parse_result(with_verdict.as_bytes()).is_err());
    let duplicate = result_json("[]").replacen('{', r#"{"nonce":"nonce-1","#, 1);
    assert!(parse_result(duplicate.as_bytes()).is_err());
    let oversized = format!("{}{}", result_json("[]"), " ".repeat(24_001));
    assert!(parse_result(oversized.as_bytes()).is_err());
}

#[test]
fn truth_table_is_fixed_and_never_yields_full() {
    let k1 = row("k1", Some("t-a"), "passed", [1, 1, 0, 0]);
    let k2 = row("k2", None, "passed", [2, 2, 0, 0]);
    let rv_passed = row("rv", None, "passed", [1, 1, 0, 0]);
    let rv_absent = row("rv", None, "not_provided", [0, 0, 0, 0]);
    let k2_unresolved = row("k2", None, "unresolved", [2, 1, 0, 1]);
    let k1_failed = row("k1", Some("t-a"), "failed", [1, 0, 1, 0]);
    let k2_not_run = r#"{"check_id":"k2","target_id":null,"status":"not_run","counts":{"expected":null,"executed":0,"passed":0,"failed":0,"unresolved":0}}"#.to_string();
    let table = [
        // E=true, M=false, U=false: uncapped full, host cap partial.
        (
            vec![k1.clone(), k2.clone(), rv_passed.clone()],
            0,
            true,
            DomainVerdict::Full,
        ),
        // Intentionally unprovided review: partial, native passes.
        (
            vec![k1.clone(), k2.clone(), rv_absent.clone()],
            0,
            true,
            DomainVerdict::Partial,
        ),
        // Required unresolved: partial domain, native failure kept.
        (
            vec![k1.clone(), k2_unresolved.clone()],
            1,
            false,
            DomainVerdict::Partial,
        ),
        // M + U: none even though unresolved coexists.
        (
            vec![k1_failed.clone(), k2_unresolved.clone()],
            1,
            false,
            DomainVerdict::None,
        ),
        // S=false: later rows not_run, never zero-as-success.
        (
            vec![k1_failed.clone(), k2_not_run.clone()],
            1,
            false,
            DomainVerdict::None,
        ),
        // Required not_run alone: none.
        (
            vec![k1.clone(), k2_not_run.clone()],
            1,
            false,
            DomainVerdict::None,
        ),
    ];
    for (rows, exit, native, uncapped) in table {
        let facts = validate(&rows, exit).unwrap_or_else(|error| panic!("{rows:?}: {error:?}"));
        assert_eq!(facts.native_passed(), native, "{rows:?}");
        assert_eq!(facts.uncapped_verdict(), uncapped, "{rows:?}");
        let capped = cap_domain(facts.uncapped_verdict());
        assert_ne!(capped, DomainVerdict::Full, "false full: {rows:?}");
        assert_eq!(capped, uncapped.min(DomainVerdict::Partial));
    }
    assert_eq!(cap_domain(DomainVerdict::Full), DomainVerdict::Partial);
    assert_eq!(cap_domain(DomainVerdict::None), DomainVerdict::None);
}

#[test]
fn registry_admission_refuses_unset_unregistered_invalid_and_self_approved_descriptors() {
    let harness = Harness::new();
    harness.ledger("ok-adapter", "normal", "pass-all", 0);
    assert!(admit(Some(&harness.config), "ok-adapter").is_ok());

    assert!(
        admit(None, "ok-adapter")
            .unwrap_err()
            .contains("is not set")
    );
    assert!(
        admit(Some(Path::new("relative/config")), "ok-adapter")
            .unwrap_err()
            .contains("absolute")
    );
    assert!(
        admit(Some(&harness.config), "unknown-adapter")
            .unwrap_err()
            .contains("not registered")
    );
    assert!(admit(Some(&harness.config), "Bad Id").is_err());

    let base = harness.descriptor("ok-adapter");
    let linked_shell = harness.observe.join("sh-link");
    #[cfg(unix)]
    std::os::unix::fs::symlink(shell(), &linked_shell).unwrap();
    let invalid: Vec<(&str, Mutation)> = vec![
        (
            "unknown field `argv`",
            Box::new(|value| value["argv"] = json!(["/bin/sh", "-c", "true"])),
        ),
        (
            "unknown field `model`",
            Box::new(|value| value["model"] = json!("any-model")),
        ),
        (
            "protocol must be `artifact-contract.adapter/v1`",
            Box::new(|value| value["protocol"] = json!("artifact-contract.adapter/v0")),
        ),
        (
            "adapter_id differs from its registry file name",
            Box::new(|value| value["adapter_id"] = json!("other")),
        ),
        (
            "executable.path must be the operator-resolved realpath",
            Box::new(move |value| value["executable"]["path"] = json!(linked_shell)),
        ),
        (
            "executable.sha256 must be 64 lowercase hex digits",
            Box::new(|value| value["executable"]["sha256"] = json!("ABC")),
        ),
        (
            "entrypoint must be a pinned member",
            Box::new(|value| value["entrypoint"] = json!("other.sh")),
        ),
        (
            "required must name at least one check",
            Box::new(|value| value["required"] = json!([])),
        ),
        (
            "limits must stay within 1..30 seconds and 1..24000 stream bytes",
            Box::new(|value| value["limits"]["timeout_seconds"] = json!(31)),
        ),
        (
            "limits must stay within 1..30 seconds and 1..24000 stream bytes",
            Box::new(|value| value["limits"]["max_stream_bytes"] = json!(24_001)),
        ),
        (
            "row `k2` is registered twice",
            Box::new(|value| value["optional"] = json!([{"check_id": "k2"}])),
        ),
        (
            "member `./x` must use plain path components",
            Box::new(|value| value["members"]["./x"] = json!("a".repeat(64))),
        ),
    ];
    // Every case but the ID-mismatch one keeps the descriptor ID equal to its
    // file name, so each reaches the condition it names.
    for (reason, mutate) in invalid {
        let mut descriptor = base.clone();
        descriptor["adapter_id"] = json!("bad-adapter");
        mutate(&mut descriptor);
        harness.register("bad-adapter", &descriptor);
        let error = admit(Some(&harness.config), "bad-adapter").unwrap_err();
        assert!(error.contains(reason), "expected {reason:?}, got {error}");
    }

    // A repeated member key is refused instead of keeping the last value.
    let mut descriptor = base.clone();
    descriptor["adapter_id"] = json!("dup-adapter");
    let compact = String::from_utf8(serde_json::to_vec(&descriptor).unwrap()).unwrap();
    let repeated = compact.replacen(
        "\"members\":{",
        &format!("\"members\":{{\"ledger.sh\":\"{}\",", "a".repeat(64)),
        1,
    );
    assert_ne!(repeated, compact);
    fs::write(harness.config.join("adapters/dup-adapter.json"), repeated).unwrap();
    let error = admit(Some(&harness.config), "dup-adapter").unwrap_err();
    assert!(error.contains("repeated key `ledger.sh`"), "{error}");

    // A descriptor that is a symlink (for example into a model-written place)
    // is refused even when its target is a valid descriptor.
    #[cfg(unix)]
    {
        let target = harness.observe.join("planted.json");
        fs::write(&target, serde_json::to_vec(&base).unwrap()).unwrap();
        std::os::unix::fs::symlink(&target, harness.config.join("adapters/linked.json")).unwrap();
        assert!(
            admit(Some(&harness.config), "linked")
                .unwrap_err()
                .contains("not a symlink")
        );
    }
}

#[test]
fn request_cannot_carry_executable_argv_model_or_expected_hashes() {
    let good = request_json("req-1", "input/spec.txt");
    assert!(parse_request(good.as_bytes()).is_ok());
    for field in [
        r#""executable":"/bin/sh""#,
        r#""argv":["node","x.mjs"]"#,
        r#""module":"file:///x.mjs""#,
        r#""model":"any""#,
        r#""expected_sha256":{"spec":"00"}"#,
    ] {
        let request = good.replacen('{', &format!("{{{field},"), 1);
        assert!(parse_request(request.as_bytes()).is_err(), "{field}");
    }
    let input_hash = good.replace(
        r#""path":"input/decl.txt""#,
        r#""path":"input/decl.txt","sha256":"00""#,
    );
    assert!(parse_request(input_hash.as_bytes()).is_err());
    let oversized = format!("{good}{}", " ".repeat(16_384));
    assert!(parse_request(oversized.as_bytes()).is_err());
}

#[test]
fn two_distinct_synthetic_adapters_pass_through_the_same_host_path() {
    let harness = Harness::new();
    let ledger = harness.ledger("ledger", "normal", "pass-all", 0);
    let receipt = harness.run(&ledger);
    assert_verified(&receipt, true, DomainVerdict::Partial);
    assert_eq!(receipt.uncapped, DomainVerdict::Full);
    assert_eq!(receipt.request_id.as_deref(), Some("req-1"));
    assert_eq!(
        receipt.input_digests.get("decl").map(String::as_str),
        Some(sha256_bytes(DECL.as_bytes()).as_str())
    );
    let process = receipt.process.as_ref().unwrap();
    assert_eq!(process.exit_code, Some(0));
    assert!(process.reaped);

    let tally = harness.tally("tally", 3);
    let receipt = harness.run(&tally);
    assert_verified(&receipt, true, DomainVerdict::Partial);
    assert_eq!(receipt.facts.unwrap().required_passed, 1);

    let wrong = harness.tally("tally-wrong", 4);
    let receipt = harness.run(&wrong);
    assert_verified(&receipt, false, DomainVerdict::None);
    assert!(receipt.reasons[0].contains("q9:lines"));

    // The host leaves no report behind in the workspace.
    let mut entries = fs::read_dir(harness.workspace.join("input"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().to_string())
        .collect::<Vec<_>>();
    entries.sort();
    assert_eq!(
        entries,
        ["decl.txt", "request.json", "spec.txt", "tally-request.json"]
    );
    assert_eq!(fs::read_dir(&harness.workspace).unwrap().count(), 1);
}

#[test]
fn truth_table_through_the_host_path() {
    let harness = Harness::new();
    let cases = [
        ("review-not-provided", 0, true, DomainVerdict::Partial),
        ("required-unresolved", 1, false, DomainVerdict::Partial),
        ("mismatch-and-unresolved", 1, false, DomainVerdict::None),
        ("static-failed", 1, false, DomainVerdict::None),
        ("required-not-run", 1, false, DomainVerdict::None),
    ];
    for (index, (rows, code, native, domain)) in cases.into_iter().enumerate() {
        let check = harness.ledger(&format!("truth-{index}"), "normal", rows, code);
        let receipt = harness.run(&check);
        assert_verified(&receipt, native, domain);
        assert_eq!(receipt.failures().is_empty(), native, "{rows}");
    }
}

#[test]
fn malformed_stale_and_inconsistent_results_fail() {
    let harness = Harness::new();
    let cases = [
        ("garbage", "pass-all", 0, "not a valid envelope"),
        ("stale_nonce", "pass-all", 0, "different run nonce"),
        ("stale_request", "pass-all", 0, "different request_id"),
        ("stale_digest", "pass-all", 0, "input digests differ"),
        ("normal", "missing-target", 0, "`k1`/`t-b` is missing"),
        ("normal", "duplicate-row", 0, "reported more than once"),
        ("normal", "count-mismatch", 0, "executed does not equal"),
        (
            "normal",
            "zero-as-success",
            0,
            "status disagrees with its counts",
        ),
        (
            "normal",
            "unknown-row",
            0,
            "not registered for this adapter",
        ),
        (
            "normal",
            "static-failed",
            0,
            "disagrees with its reported rows",
        ),
        ("normal", "pass-all", 2, "exited with code 2"),
    ];
    for (index, (mode, rows, code, reason)) in cases.into_iter().enumerate() {
        let check = harness.ledger(&format!("bad-{index}"), mode, rows, code);
        let receipt = harness.run(&check);
        let stage = if code == 2 { "process" } else { "envelope" };
        assert_failed_at(&receipt, stage, reason);
    }
}

#[test]
fn child_starts_from_an_empty_environment() {
    let harness = Harness::new();
    let check = harness.ledger("env-probe", "env", "pass-all", 0);
    let receipt = harness.run(&check);
    assert_verified(&receipt, true, DomainVerdict::Partial);
    let dump = fs::read_to_string(harness.observe.join("env.txt")).unwrap();
    let exported = exported_env(&dump);
    for key in ["LANG", "TZ", super::process::TOOL_ROOT_ENV] {
        assert!(exported.contains_key(key), "{key} missing: {dump}");
    }
    assert_eq!(
        exported
            .get(super::process::TOOL_ROOT_ENV)
            .map(String::as_str),
        Some(harness.tooling.to_string_lossy().as_ref())
    );
    for key in ["HOME", "NODE_OPTIONS", "NODE_PATH", "USER", "TMPDIR"] {
        assert!(!exported.contains_key(key), "{key} leaked: {dump}");
    }
    if let Some(parent_path) = std::env::var_os("PATH") {
        assert_ne!(
            exported.get("PATH").map(String::as_str),
            parent_path.to_str(),
            "parent PATH leaked: {dump}"
        );
    }
}

/// Keys and values of `export -p` output from dash or bash.
pub(super) fn exported_env(dump: &str) -> BTreeMap<String, String> {
    dump.lines()
        .filter_map(|line| line.strip_prefix("export "))
        .map(|line| line.strip_prefix("-x ").unwrap_or(line))
        .map(|entry| match entry.split_once('=') {
            Some((key, value)) => (
                key.to_string(),
                value.trim_matches(|ch| ch == '"' || ch == '\'').to_string(),
            ),
            None => (entry.to_string(), String::new()),
        })
        .collect()
}

#[test]
fn workspace_node_modules_and_self_approved_registry_cannot_create_a_pass() {
    let harness = Harness::new();
    let marker = harness.observe.join("workspace-node-ran");
    let fake = format!("#!/bin/sh\nprintf ran > '{}'\n", marker.display());
    fs::create_dir_all(harness.workspace.join("node_modules/.bin")).unwrap();
    for path in ["node", "node_modules/.bin/node", "node_modules/.bin/sh"] {
        fs::write(harness.workspace.join(path), &fake).unwrap();
        make_executable(&harness.workspace.join(path));
    }
    let check = harness.ledger("ledger", "normal", "pass-all", 0);
    assert_verified(&harness.run(&check), true, DomainVerdict::Partial);
    assert!(!marker.exists(), "a workspace executable was used");

    // A descriptor whose executable is the workspace fake is refused.
    let mut descriptor = harness.descriptor("ledger");
    let fake_node = harness.workspace.join("node");
    descriptor["executable"] = json!({"path": fake_node, "sha256": sha256_file(&fake_node)});
    let check = harness.relabel("ws-node", descriptor);
    assert_failed_at(&harness.run(&check), "trust", "inside the workspace");
    assert!(!marker.exists());

    // A dependency link into the workspace is not an allowed package root.
    #[cfg(unix)]
    {
        fs::create_dir_all(harness.workspace.join("dep")).unwrap();
        fs::create_dir_all(harness.tooling.join("node_modules")).unwrap();
        std::os::unix::fs::symlink(
            harness.workspace.join("dep"),
            harness.tooling.join("node_modules/dep"),
        )
        .unwrap();
        let mut descriptor = harness.descriptor("ledger");
        descriptor["links"] = json!({"node_modules/dep": "rows"});
        let check = harness.relabel("ws-module", descriptor);
        assert_failed_at(&harness.run(&check), "pin", "expected package root");
        let check = harness.ledger("ws-module-unlisted", "normal", "pass-all", 0);
        assert_failed_at(&harness.run(&check), "pin", "unregistered symlink");
        fs::remove_file(harness.tooling.join("node_modules/dep")).unwrap();
    }

    // A registry inside the workspace is self-approval.
    let planted = harness.workspace.join("registry");
    fs::create_dir_all(planted.join("adapters")).unwrap();
    fs::copy(
        harness.config.join("adapters/ledger.json"),
        planted.join("adapters/ledger.json"),
    )
    .unwrap();
    let receipt = run_check(
        &harness.workspace,
        &ArtifactContractCheck {
            adapter_id: "ledger".to_string(),
            request: "input/request.json".to_string(),
        },
        Some(&planted),
        || false,
    );
    assert_failed_at(&receipt, "trust", "self-approved");
}

#[test]
fn tooling_symlink_or_replacement_cannot_create_a_pass() {
    let harness = Harness::new();
    let check = harness.ledger("ledger", "normal", "pass-all", 0);
    assert_verified(&harness.run(&check), true, DomainVerdict::Partial);

    // Replaced member.
    let original = fs::read(harness.tooling.join("rows/pass-all.json")).unwrap();
    fs::write(
        harness.tooling.join("rows/pass-all.json"),
        REVIEW_NOT_PROVIDED,
    )
    .unwrap();
    assert_failed_at(&harness.run(&check), "pin", "does not match its pin");
    fs::write(harness.tooling.join("rows/pass-all.json"), &original).unwrap();

    // Replaced executable pin.
    let mut descriptor = harness.descriptor("ledger");
    descriptor["executable"]["sha256"] = json!("0".repeat(64));
    let pinned = harness.relabel("bad-exe", descriptor);
    assert_failed_at(&harness.run(&pinned), "pin", "executable does not match");

    // A member swapped for a symlink to identical bytes elsewhere.
    #[cfg(unix)]
    {
        let elsewhere = harness.observe.join("pass-all.json");
        fs::write(&elsewhere, &original).unwrap();
        fs::remove_file(harness.tooling.join("rows/pass-all.json")).unwrap();
        std::os::unix::fs::symlink(&elsewhere, harness.tooling.join("rows/pass-all.json")).unwrap();
        assert_failed_at(&harness.run(&check), "pin", "unregistered symlink");
        fs::remove_file(harness.tooling.join("rows/pass-all.json")).unwrap();
        fs::write(harness.tooling.join("rows/pass-all.json"), &original).unwrap();
    }

    // A member replaced while the adapter runs is detected afterwards.
    let tamper = harness.ledger("tamper", "tamper", "pass-all", 0);
    assert_failed_at(&harness.run(&tamper), "binding", "tooling changed");
}

#[test]
fn pinned_input_mismatch_and_midrun_input_change_fail() {
    let harness = Harness::new();
    let check = harness.ledger("ledger", "normal", "pass-all", 0);
    fs::write(harness.workspace.join("input/spec.txt"), SPEC_V2).unwrap();
    assert_failed_at(
        &harness.run(&check),
        "input",
        "does not match its operator pin",
    );
    fs::write(harness.workspace.join("input/spec.txt"), SPEC_V1).unwrap();

    let mutate = harness.ledger("mutate", "mutate", "pass-all", 0);
    assert_failed_at(
        &harness.run(&mutate),
        "binding",
        "changed while the adapter ran",
    );

    // Inputs must be safe workspace files and exactly the registered IDs.
    for (request, reason) in [
        (request_json("req-1", "../spec.txt"), "invalid"),
        (request_json("req-1", ".env"), "credential"),
        (request_json("req-1", "input/absent.txt"), "missing"),
        (
            r#"{"protocol":"artifact-contract.request/v1","request_id":"req-1","inputs":[{"input_id":"spec","path":"input/spec.txt"}]}"#.to_string(),
            "exactly the operator-registered input IDs",
        ),
    ] {
        fs::write(harness.workspace.join("input/request.json"), request).unwrap();
        let receipt = harness.run(&check);
        assert!(!receipt.native_passed, "{receipt:#?}");
        assert!(
            receipt.reasons.iter().any(|actual| actual.contains(reason)),
            "{reason}: {receipt:#?}"
        );
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(
            harness.workspace.join("input/spec.txt"),
            harness.workspace.join("input/spec-link.txt"),
        )
        .unwrap();
        fs::write(
            harness.workspace.join("input/request.json"),
            request_json("req-1", "input/spec-link.txt"),
        )
        .unwrap();
        assert_failed_at(&harness.run(&check), "input", "symlink");
    }
}

#[test]
fn timeout_overflow_and_cancel_stop_and_reap_the_process_group() {
    let harness = Harness::new();

    let timeout = harness.ledger("timeout", "grandchild", "pass-all", 0);
    let started = Instant::now();
    let receipt = harness.run(&timeout);
    assert!(started.elapsed() < Duration::from_secs(10));
    assert_failed_at(&receipt, "process", "timed_out");
    let process = receipt.process.as_ref().unwrap();
    assert!(process.reaped && process.group_stopped, "{process:?}");
    assert_heartbeat_stopped(&harness.observe.join("beat"));

    let flood = harness.ledger("flood", "flood", "pass-all", 0);
    let receipt = harness.run(&flood);
    assert_failed_at(&receipt, "process", "output_exceeded");
    let process = receipt.process.as_ref().unwrap();
    assert!(process.reaped && process.group_stopped, "{process:?}");
    assert!(process.stdout_bytes <= 24_000);

    let cancel = harness.ledger("cancel", "sleep", "pass-all", 0);
    let started = Instant::now();
    let receipt = run_check(&harness.workspace, &cancel, Some(&harness.config), || {
        started.elapsed() > Duration::from_millis(200)
    });
    assert_failed_at(&receipt, "process", "cancelled");
    assert!(started.elapsed() < Duration::from_secs(2));
    assert!(receipt.process.as_ref().unwrap().reaped);
}

fn assert_heartbeat_stopped(beat: &Path) {
    let first = fs::metadata(beat)
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    assert!(first > 0, "the grandchild never ran");
    std::thread::sleep(Duration::from_millis(400));
    let second = fs::metadata(beat)
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    assert_eq!(first, second, "a grandchild survived the stopped group");
}

#[test]
fn old_results_are_never_reused_and_an_operator_selected_old_version_is_rechecked() {
    let harness = Harness::new();
    let check = harness.ledger("ledger", "normal", "pass-all", 0);
    let first = harness.run(&check);
    assert_verified(&first, true, DomainVerdict::Partial);

    // The newer candidate fails; the earlier pass is not carried over.
    fs::write(harness.workspace.join("input/decl.txt"), "alpha\n").unwrap();
    let failing = harness.ledger("ledger", "normal", "static-failed", 1);
    let second = harness.run(&failing);
    assert_verified(&second, false, DomainVerdict::None);
    assert_ne!(first.input_digests, second.input_digests);

    // A replayed result from an earlier run carries a stale nonce.
    let replay = harness.ledger("replay", "stale_nonce", "pass-all", 0);
    assert_failed_at(&harness.run(&replay), "envelope", "different run nonce");

    // The operator explicitly selects the old version in a new request; it is
    // executed afresh and bound to the selected bytes.
    fs::write(harness.workspace.join("input/decl.txt"), DECL).unwrap();
    fs::write(harness.workspace.join("input/spec-v1.txt"), SPEC_V1).unwrap();
    fs::write(
        harness.workspace.join("input/request.json"),
        request_json("req-old-selected", "input/spec-v1.txt"),
    )
    .unwrap();
    let selected = harness.ledger("selected", "normal", "pass-all", 0);
    let third = harness.run(&selected);
    assert_verified(&third, true, DomainVerdict::Partial);
    assert_eq!(third.request_id.as_deref(), Some("req-old-selected"));
    assert_eq!(third.input_digests, first.input_digests);
}

#[test]
fn receipt_failures_never_turn_a_domain_outcome_into_native_success() {
    let harness = Harness::new();
    let check = harness.ledger("ledger", "normal", "required-unresolved", 1);
    let receipt = harness.run(&check);
    assert_verified(&receipt, false, DomainVerdict::Partial);
    let failures = receipt.failures();
    assert_eq!(failures.len(), 1);
    assert!(
        failures[0].contains("`ledger` failed at verified"),
        "{failures:?}"
    );

    // The registry is checked again at run time; an unset root fails honestly.
    let receipt = run_check(&harness.workspace, &check, None, || false);
    assert_failed_at(&receipt, "registry", "is not set");
}

fn make_executable(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
}

/// `CatalogError` Display moved from a hand-written `impl Display` to
/// `thiserror` attributes. The expected strings are literals produced by the
/// pre-change implementation (be989685) for these representative values, not
/// derived from the current one. The match has no wildcard, so a new variant
/// fails to compile until it is listed here.
#[test]
fn catalog_error_display_matches_the_pre_thiserror_strings() {
    use crate::planner::capability_catalog::CatalogError;

    fn pre_change_display(error: &CatalogError) -> &'static str {
        match error {
            CatalogError::UnknownId(_) => "unknown capability id `cap-x`",
            CatalogError::UnknownParameter { .. } => {
                "unknown parameter `param-y` for capability `cap-x`"
            }
            CatalogError::MissingParameter { .. } => {
                "missing parameter `param-y` for capability `cap-x`"
            }
            CatalogError::TypeMismatch { .. } => {
                "parameter `param-y` for capability `cap-x` must be [path]"
            }
            CatalogError::InvalidParameter { .. } => {
                "invalid parameter `param-y` for capability `cap-x`: path may not contain .."
            }
            CatalogError::ProbeBindingUnimplemented { .. } => {
                "probe capability `cap-x` is registered but not bindable yet"
            }
        }
    }

    let id = || "cap-x".to_string();
    let parameter = || "param-y".to_string();
    let errors = [
        CatalogError::UnknownId(id()),
        CatalogError::UnknownParameter {
            id: id(),
            parameter: parameter(),
        },
        CatalogError::MissingParameter {
            id: id(),
            parameter: parameter(),
        },
        CatalogError::TypeMismatch {
            id: id(),
            parameter: parameter(),
            expected: "[path]".to_string(),
        },
        CatalogError::InvalidParameter {
            id: id(),
            parameter: parameter(),
            reason: "path may not contain ..".to_string(),
        },
        CatalogError::ProbeBindingUnimplemented { id: id() },
    ];
    let variants = errors
        .iter()
        .map(std::mem::discriminant)
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(variants.len(), errors.len(), "every variant exactly once");
    for error in &errors {
        assert_eq!(error.to_string(), pre_change_display(error), "{error:?}");
    }
}

/// B1: an input swapped for a FIFO or a symlink after the adapter read it must
/// fail promptly; the host never waits on the special file.
#[test]
fn input_swapped_for_a_fifo_or_symlink_fails_within_a_bound() {
    for mode in ["fifo", "relink"] {
        let harness = Harness::new();
        let check = harness.ledger(&format!("swap-{mode}"), mode, "pass-all", 0);
        let (sender, receiver) = std::sync::mpsc::channel();
        let workspace = harness.workspace.clone();
        let config = harness.config.clone();
        std::thread::spawn(move || {
            let _ = sender.send(run_check(&workspace, &check, Some(&config), || false));
        });
        let receipt = receiver
            .recv_timeout(Duration::from_secs(10))
            .unwrap_or_else(|_| panic!("{mode}: the host did not return within 10 s"));
        assert_failed_at(&receipt, "binding", "changed while the adapter ran");
        assert!(
            receipt.failures()[0].contains("failed at binding"),
            "{receipt:#?}"
        );
    }
}

/// B3: an optional row the adapter leaves out is unverified, like an explicit
/// `not_provided`; the uncapped verdict is partial, not full.
#[test]
fn an_omitted_optional_row_is_recorded_as_unverified() {
    let harness = Harness::new();
    let check = harness.ledger("omitted", "normal", "optional-omitted", 0);
    let receipt = harness.run(&check);
    assert_verified(&receipt, true, DomainVerdict::Partial);
    let facts = receipt.facts.unwrap();
    assert!(facts.optional_unverified);
    assert_eq!(facts.optional_missing, 1);
    assert_eq!(receipt.uncapped, DomainVerdict::Partial);

    let events = harness.observe.join("run/events.jsonl");
    super::record(&receipt, Some(&events), "draft_profile", "omitted");
    let event: Value =
        serde_json::from_str(fs::read_to_string(&events).unwrap().lines().next().unwrap()).unwrap();
    assert_eq!(event["optional_unverified"], true);
    assert_eq!(event["optional_missing"], 1);
    assert_eq!(event["domain_verdict"], "partial");
    assert_eq!(event["domain_cap_applied"], false);
    assert_eq!(event["status"], "passed");
}

/// R2: a profile that is not a registered external profile declaring
/// `artifact_contract_check` contributes no adapter and reads no manifest.
/// (The pack side reads the process's pack selection; it is covered by the
/// integration tests that own that environment.)
#[test]
fn an_undeclared_profile_contributes_no_adapter() {
    for profile in ["generic", "not-a-registered-profile"] {
        assert_eq!(
            super::profile_adapter_ids(profile),
            Ok(BTreeSet::new()),
            "{profile}"
        );
    }
}

/// B2: only `artifact_contract_check` entries under the top-level `checks`
/// bindings are declarations; a same-shaped object elsewhere is ignored, and
/// a malformed declaration is an error instead of an empty set.
#[test]
fn declared_adapter_ids_are_read_only_from_check_bindings() {
    let manifest: toml::Value = toml::from_str(
        r#"
[metadata]
id = "x"
[notes.example]
id = "artifact_contract_check"
params = { adapter_id = "in-notes", request = "r.json" }
[[checks.final]]
id = "artifact_contract_check"
params = { adapter_id = "first", request = "r.json" }
[[checks.other]]
id = "command_check"
params = { adapter_id = "not-this-one" }
"#,
    )
    .unwrap();
    assert_eq!(
        super::check_binding_adapter_ids(&manifest),
        Ok(BTreeSet::from(["first".to_string()]))
    );
    for broken in [
        "checks = \"x\"\n",
        "[checks]\nfinal = \"x\"\n",
        "[[checks.final]]\nid = \"artifact_contract_check\"\nparams = { adapter_id = 3 }\n",
    ] {
        let manifest: toml::Value = toml::from_str(broken).unwrap();
        assert!(
            super::check_binding_adapter_ids(&manifest).is_err(),
            "{broken}"
        );
    }
}

/// B1/B2: the registered manifest is re-read only as the exact bytes that
/// were loaded. A check removed or replaced after loading, a FIFO, a trailing
/// symlink, and an oversized file are all refused, and a FIFO never blocks.
#[cfg(unix)]
#[test]
fn a_registered_manifest_is_re_read_only_as_its_loaded_bytes() {
    use crate::planner::profile_manifest::source::{EXTENSION_MANIFEST_FILE, exact_byte_hash};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(EXTENSION_MANIFEST_FILE);
    let loaded = "[[checks.final]]\nid = \"artifact_contract_check\"\nparams = { adapter_id = \"a\", request = \"r.json\" }\n";
    fs::write(&path, loaded).unwrap();
    let hash = exact_byte_hash(EXTENSION_MANIFEST_FILE, loaded.as_bytes());
    assert_eq!(
        super::registered_manifest_adapter_ids(&path, &hash),
        Ok(BTreeSet::from(["a".to_string()]))
    );
    for changed in ["", &loaded.replace("\"a\"", "\"b\"")] {
        fs::write(&path, changed).unwrap();
        let error = super::registered_manifest_adapter_ids(&path, &hash).unwrap_err();
        assert!(error.contains("changed since it was loaded"), "{error}");
    }
    fs::write(&path, vec![b'#'; 256 * 1024 + 1]).unwrap();
    assert!(super::registered_manifest_adapter_ids(&path, &hash).is_err());

    fs::remove_file(&path).unwrap();
    let elsewhere = dir.path().join("elsewhere.toml");
    fs::write(&elsewhere, loaded).unwrap();
    std::os::unix::fs::symlink(&elsewhere, &path).unwrap();
    assert!(super::registered_manifest_adapter_ids(&path, &hash).is_err());

    fs::remove_file(&path).unwrap();
    make_fifo(&path);
    let (sender, receiver) = std::sync::mpsc::channel();
    let fifo = path.clone();
    let expected = hash.clone();
    std::thread::spawn(move || {
        let _ = sender.send(super::registered_manifest_adapter_ids(&fifo, &expected));
    });
    let result = receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("reading a FIFO manifest did not block");
    assert!(result.unwrap_err().contains("not a regular file"));
}

/// Red team `parse-overlay-checks-match`, `parse-v2-manifest-match`,
/// `parse-manifest-phases-on-artifact` (preflight side): an overlay file
/// hashed under its own name, a v2 manifest, and a binding that carries
/// `phases` all yield their declared adapter, the same set final acceptance
/// binds (it then refuses the phased binding natively).
#[test]
fn overlay_v2_and_phased_declarations_are_collected() {
    use crate::planner::profile_manifest::source::{
        EXTENSION_MANIFEST_FILE, EXTENSION_OVERLAY_FILE, exact_byte_hash,
    };
    let dir = tempfile::tempdir().unwrap();
    for (file, text, expected) in [
        (
            EXTENSION_OVERLAY_FILE,
            "[overlay]\nbase_profile = \"nextjs\"\nmode = \"additive\"\n[[checks.extra]]\nid = \"artifact_contract_check\"\nparams = { adapter_id = \"overlay-a\", request = \"r.json\" }\n",
            "overlay-a",
        ),
        (
            EXTENSION_MANIFEST_FILE,
            "[metadata]\nid = \"v2\"\nschema_version = \"v2\"\n[[checks.final]]\nid = \"artifact_contract_check\"\nparams = { adapter_id = \"v2-a\", request = \"r.json\" }\n",
            "v2-a",
        ),
        (
            EXTENSION_MANIFEST_FILE,
            "[[checks.final]]\nid = \"artifact_contract_check\"\nphases = [\"build\"]\nparams = { adapter_id = \"phased-a\", request = \"r.json\" }\n",
            "phased-a",
        ),
    ] {
        let path = dir.path().join(file);
        fs::write(&path, text).unwrap();
        let hash = exact_byte_hash(file, text.as_bytes());
        assert_eq!(
            super::registered_manifest_adapter_ids(&path, &hash),
            Ok(BTreeSet::from([expected.to_string()])),
            "{expected}"
        );
    }
}

/// The safe open itself: a FIFO with no writer, a trailing symlink, and a
/// device are refused without blocking; a regular file opens.
#[cfg(unix)]
#[test]
fn open_regular_refuses_fifo_symlink_and_device_without_blocking() {
    let dir = tempfile::tempdir().unwrap();
    let regular = dir.path().join("regular");
    fs::write(&regular, b"x").unwrap();
    assert!(super::open_regular(&regular).is_ok());

    let link = dir.path().join("link");
    std::os::unix::fs::symlink(&regular, &link).unwrap();
    assert!(super::open_regular(&link).is_err());

    let fifo = dir.path().join("fifo");
    make_fifo(&fifo);
    let (sender, receiver) = std::sync::mpsc::channel();
    let target = fifo.clone();
    std::thread::spawn(move || {
        let _ = sender.send(super::open_regular(&target).map(|_| ()));
    });
    let result = receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("opening a FIFO without a writer did not block");
    assert_eq!(result, Err("is not a regular file".to_string()));

    assert_eq!(
        super::open_regular(Path::new("/dev/null")).map(|_| ()),
        Err("is not a regular file".to_string())
    );
}

/// R2 (recheck 3): the shared safe read that the pack loader now uses after
/// its own `lstat` check. A member swapped for a writerless FIFO or a symlink
/// after that check, or grown past the limit after the size check, fails
/// without blocking with the kinds the loader maps onto `NotRegularFile`,
/// `NotRegularFile`, and `TooLarge`/`MaterialTooLarge`; a regular file within
/// the limit reads unchanged.
#[cfg(unix)]
#[test]
fn shared_safe_read_refuses_swapped_or_grown_members_without_blocking() {
    use super::SafeReadError;
    let dir = tempfile::tempdir().unwrap();
    for member in ["eval.yaml", "assist.yaml", "guide.md"] {
        let path = dir.path().join(member);
        fs::write(&path, b"ok: true\n").unwrap();
        assert_eq!(
            super::read_regular_bounded(&path, 64).unwrap(),
            b"ok: true\n"
        );

        // Grown past the limit after a size check.
        fs::write(&path, vec![b'#'; 65]).unwrap();
        assert!(matches!(
            super::read_regular_bounded(&path, 64),
            Err(SafeReadError::TooLarge(64))
        ));

        // Swapped for a symlink to an acceptable file.
        let elsewhere = dir.path().join(format!("{member}.real"));
        fs::write(&elsewhere, b"ok: true\n").unwrap();
        fs::remove_file(&path).unwrap();
        std::os::unix::fs::symlink(&elsewhere, &path).unwrap();
        let error = super::read_regular_bounded(&path, 64).unwrap_err();
        assert!(error.is_symlink_refusal(), "{member}: {error}");

        // Swapped for a writerless FIFO: refused without blocking.
        fs::remove_file(&path).unwrap();
        make_fifo(&path);
        let (sender, receiver) = std::sync::mpsc::channel();
        let target = path.clone();
        std::thread::spawn(move || {
            let _ = sender.send(super::read_regular_bounded(&target, 64).map(|_| ()));
        });
        let result = receiver
            .recv_timeout(Duration::from_secs(5))
            .unwrap_or_else(|_| panic!("{member}: reading a FIFO blocked"));
        assert!(
            matches!(result, Err(SafeReadError::NotRegular)),
            "{member}: {result:?}"
        );
        fs::remove_file(&path).unwrap();
    }
}

#[cfg(unix)]
fn make_fifo(path: &Path) {
    use std::os::unix::ffi::OsStrExt;
    let name = std::ffi::CString::new(path.as_os_str().as_bytes()).unwrap();
    // SAFETY: `name` is a valid NUL-terminated path in a test-owned tempdir.
    let status = unsafe { libc::mkfifo(name.as_ptr(), 0o600) };
    assert_eq!(status, 0, "mkfifo failed");
}

/// R3: a descendant that leaves the process group and keeps stdin open
/// without reading it cannot hold the host. The run returns within its bound,
/// reports the request as incomplete, and the host has closed its stdin end:
/// the descendant later reads to end-of-file.
#[cfg(unix)]
#[test]
fn a_descendant_holding_stdin_cannot_hold_the_host() {
    let perl = Path::new("/usr/bin/perl");
    assert!(perl.is_file(), "/usr/bin/perl is required for this test");
    let observe = tempfile::tempdir().unwrap();
    let pid_file = observe.path().join("escaped.pid");
    let eof_file = observe.path().join("escaped.eof");
    let script = r#"/usr/bin/perl -e 'use POSIX; POSIX::setsid(); open(my $p, ">", $ARGV[0]); print $p $$; close $p; sleep 2; my $n = 0; while (my $r = sysread(STDIN, my $b, 65536)) { $n += $r } open(my $m, ">", $ARGV[1]); print $m $n; close $m;' "$1" "$2" <&0 >/dev/null 2>&1 &
while [ ! -s "$1" ]; do /bin/sleep 0.05; done
exit 0
"#;
    let payload = vec![b'x'; 1 << 20];
    let launch = super::process::Launch {
        executable: shell(),
        args: vec![
            "-c".into(),
            script.into(),
            "adapter".into(),
            pid_file.clone().into(),
            eof_file.clone().into(),
        ],
        cwd: observe.path().to_path_buf(),
        tool_root: observe.path().to_path_buf(),
        stdin: payload.clone(),
        timeout: Duration::from_secs(5),
        max_stream_bytes: 24_000,
    };
    // Stops the escaped descendant's own process group (its setsid made its
    // pid the group id) on every exit, including a failed assertion.
    let _cleanup = EscapedGroup(pid_file.clone());
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = sender.send(super::process::run(&launch, || false));
    });
    let observation = receiver
        .recv_timeout(Duration::from_secs(10))
        .expect("the host returned within 10 s")
        .unwrap();
    assert!(!observation.stdin_complete, "{observation:?}");
    assert!(observation.reaped, "{observation:?}");

    let deadline = Instant::now() + Duration::from_secs(10);
    while !eof_file.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    let read = fs::read_to_string(&eof_file)
        .expect("the escaped descendant reached end-of-file, so the host closed stdin");
    let read: usize = read.trim().parse().unwrap();
    assert!(read < payload.len(), "{read}");
}

/// B1 (direct path refusal): after the run an input replaced by a symlink to
/// identical bytes, or reached through a directory replaced by a link, is
/// refused by the path checks themselves; the digest alone would still match.
#[cfg(unix)]
#[test]
fn identical_bytes_behind_a_symlink_or_linked_directory_are_refused() {
    for mode in ["relink_same", "relink_dir"] {
        let harness = Harness::new();
        fs::write(harness.observe.join("same-decl.txt"), DECL).unwrap();
        let check = harness.ledger(
            &format!("swap-{}", mode.replace('_', "-")),
            mode,
            "pass-all",
            0,
        );
        let receipt = harness.run(&check);
        assert_failed_at(&receipt, "binding", "changed while the adapter ran");
        assert_eq!(
            sha256_file(&harness.observe.join("same-decl.txt")),
            sha256_bytes(DECL.as_bytes()),
            "{mode}: the bytes behind the link are unchanged"
        );
    }
}

#[cfg(unix)]
struct EscapedGroup(PathBuf);

#[cfg(unix)]
impl Drop for EscapedGroup {
    fn drop(&mut self) {
        if let Some(pid) = fs::read_to_string(&self.0)
            .ok()
            .and_then(|pid| pid.trim().parse::<u32>().ok())
        {
            let _ = crate::bounded_process::terminate_process_group_by_pid(pid);
        }
    }
}
