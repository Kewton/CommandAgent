//! Candidate boundary, guards, and raw-byte deltas of the Issue #949 artifact
//! candidates. Every workspace is a temporary directory; contents are
//! synthetic markers, never a real project's files or credentials.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde_json::json;
use sha2::{Digest, Sha256};

use super::super::changed_snapshot_paths;
use super::super::repair_workspace_snapshot::{capture, capture_with_artifacts};
use super::{artifact_candidates, artifact_repair_candidates};
use crate::planner::artifact_contract_checks::{bound_workspace_paths, run_check};
use crate::planner::capability_catalog::artifact::ArtifactContractCheck;

type Snapshot = BTreeMap<String, Option<Vec<u8>>>;

const REQUEST: &str = "check/request.json";

/// An operator registry with one adapter that exits 1 at once; the input
/// `terms` is pinned to `pin` when given.
struct Registry {
    _dir: tempfile::TempDir,
    config: PathBuf,
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn registry(pin: Option<&str>) -> Registry {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let tooling = root.join("tooling");
    let config = root.join("config");
    std::fs::create_dir_all(&tooling).unwrap();
    std::fs::create_dir_all(config.join("adapters")).unwrap();
    std::fs::write(tooling.join("gate.sh"), "exit 1\n").unwrap();
    let shell = std::fs::canonicalize("/bin/sh").unwrap();
    let mut input = json!({"input_id": "terms"});
    if let Some(pin) = pin {
        input["sha256"] = json!(pin);
    }
    let descriptor = json!({
        "protocol": "artifact-contract.adapter/v1",
        "adapter_id": "gate",
        "executable": {"path": shell, "sha256": sha256_hex(&std::fs::read(&shell).unwrap())},
        "tooling_root": tooling,
        "entrypoint": "gate.sh",
        "members": {"gate.sh": sha256_hex(b"exit 1\n")},
        "required": [{"check_id": "terms/state", "targets": []}],
        "inputs": [input],
    });
    std::fs::write(
        config.join("adapters/gate.json"),
        serde_json::to_vec(&descriptor).unwrap(),
    )
    .unwrap();
    Registry { _dir: dir, config }
}

fn request(input_id: &str, path: &str) -> String {
    json!({
        "protocol": "artifact-contract.request/v1",
        "request_id": "r-1",
        "inputs": [{"input_id": input_id, "path": path}],
    })
    .to_string()
}

/// Run the check once in `root` and return the names it recorded there.
fn recorded(registry: &Registry, adapter: &str, root: &Path) -> Vec<String> {
    let check = ArtifactContractCheck {
        adapter_id: adapter.to_string(),
        request: REQUEST.to_string(),
    };
    let receipt = run_check(root, &check, Some(&registry.config), || false);
    assert!(!receipt.native_passed);
    bound_workspace_paths(root)
}

fn write(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

fn strings(paths: &[&str]) -> Vec<String> {
    paths.iter().map(|path| (*path).to_string()).collect()
}

/// A snapshot whose artifact candidates come from `declared` and `bound`
/// exactly as `artifact_repair_candidates` combines them.
fn snap(root: &Path, declared: &[&str], bound: &[&str], extra: &[&str]) -> Snapshot {
    let artifacts = artifact_candidates(&strings(declared), &strings(bound));
    capture_with_artifacts(root, "generic", "goal", &[], &strings(extra), &artifacts)
}

#[test]
fn declared_and_bound_json_yaml_byte_changes_are_reported() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(root, "contract/terms.json", "{\"state\": \"draft\"}\n");
    write(root, "contract/terms.yaml", "state: draft\n");
    write(root, "data/input.json", "{\"rows\": 1}\n");
    write(root, "check/request.json", "{}\n");
    let declared = ["contract/terms.json", "contract/terms.yaml"];
    let bound = ["check/request.json", "data/input.json"];
    let before = snap(root, &declared, &bound, &[]);
    write(root, "contract/terms.json", "{\"state\": \"ready\"}\n");
    write(root, "contract/terms.yaml", "state: ready\n");
    write(root, "data/input.json", "{\"rows\": 2}\n");
    assert_eq!(
        changed_snapshot_paths(&before, &snap(root, &declared, &bound, &[])),
        [
            "contract/terms.json",
            "contract/terms.yaml",
            "data/input.json"
        ]
    );
}

#[test]
fn a_declared_artifact_outside_the_bound_inputs_is_compared() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(root, "contract/schema.yaml", "version: 1\n");
    let declared = ["contract/schema.yaml"];
    let bound = ["check/request.json"];
    let before = snap(root, &declared, &bound, &[]);
    assert!(before.contains_key("contract/schema.yaml"), "{before:?}");
    write(root, "contract/schema.yaml", "version: 2\n");
    assert_eq!(
        changed_snapshot_paths(&before, &snap(root, &declared, &bound, &[])),
        ["contract/schema.yaml"]
    );
}

#[test]
fn a_bound_input_outside_the_declared_artifacts_is_compared() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(root, "data/extra.yaml", "a: 1\n");
    let bound = ["check/request.json", "data/extra.yaml"];
    let before = snap(root, &[], &bound, &[]);
    write(root, "data/extra.yaml", "a: 2\n");
    assert_eq!(
        changed_snapshot_paths(&before, &snap(root, &[], &bound, &[])),
        ["data/extra.yaml"]
    );
}

#[test]
fn same_bytes_rewrite_and_mtime_only_touch_are_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(root, "contract/terms.json", "{\"state\": \"draft\"}\n");
    write(root, "contract/terms.yaml", "state: draft\n");
    let declared = ["contract/terms.json", "contract/terms.yaml"];
    let before = snap(root, &declared, &["check/request.json"], &[]);
    write(root, "contract/terms.json", "{\"state\": \"draft\"}\n");
    std::fs::File::options()
        .write(true)
        .open(root.join("contract/terms.yaml"))
        .unwrap()
        .set_modified(SystemTime::now() + Duration::from_secs(120))
        .unwrap();
    assert_eq!(
        changed_snapshot_paths(
            &before,
            &snap(root, &declared, &["check/request.json"], &[])
        ),
        Vec::<String>::new()
    );
}

#[test]
fn a_reformat_with_the_same_meaning_is_a_raw_byte_change() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(root, "contract/terms.json", "{\"state\":\"draft\"}");
    let declared = ["contract/terms.json"];
    let before = snap(root, &declared, &["check/request.json"], &[]);
    write(
        root,
        "contract/terms.json",
        "{\n  \"state\": \"draft\"\n}\n",
    );
    assert_eq!(
        changed_snapshot_paths(
            &before,
            &snap(root, &declared, &["check/request.json"], &[])
        ),
        ["contract/terms.json"]
    );
}

#[test]
fn creation_deletion_and_move_of_candidates_are_reported() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(root, "contract/old.json", "{}\n");
    write(root, "contract/gone.yaml", "a: 1\n");
    let bound = [
        "check/request.json",
        "contract/old.json",
        "contract/new.json",
        "contract/gone.yaml",
        "contract/created.yaml",
    ];
    let before = snap(root, &[], &bound, &[]);
    assert_eq!(before["contract/new.json"], None);
    std::fs::rename(
        root.join("contract/old.json"),
        root.join("contract/new.json"),
    )
    .unwrap();
    std::fs::remove_file(root.join("contract/gone.yaml")).unwrap();
    write(root, "contract/created.yaml", "b: 2\n");
    assert_eq!(
        changed_snapshot_paths(&before, &snap(root, &[], &bound, &[])),
        [
            "contract/created.yaml",
            "contract/gone.yaml",
            "contract/new.json",
            "contract/old.json",
        ]
    );
}

#[test]
fn reported_and_unrelated_json_yaml_never_become_candidates() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(root, "contract/terms.json", "{}\n");
    write(root, "contract/other.json", "{}\n");
    write(root, "notes/config.yaml", "a: 1\n");
    let declared = ["contract/terms.json"];
    let reported = ["contract/other.json", "notes/config.yaml"];
    let before = snap(root, &declared, &["check/request.json"], &[]);
    assert!(!before.contains_key("contract/other.json"), "{before:?}");
    assert!(!before.contains_key("notes/config.yaml"), "{before:?}");
    write(root, "contract/other.json", "{\"changed\": true}\n");
    write(root, "notes/config.yaml", "a: 2\n");
    let after = snap(root, &declared, &["check/request.json"], &reported);
    assert!(!after.contains_key("contract/other.json"), "{after:?}");
    assert_eq!(
        changed_snapshot_paths(&before, &after),
        Vec::<String>::new()
    );
}

#[test]
fn without_an_admitted_check_the_snapshot_is_the_legacy_one() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(root, "src/app/main.py", "print(1)\n");
    write(root, "web/page.tsx", "export {}\n");
    write(root, "contract/terms.json", "{}\n");
    let expected = strings(&["contract/terms.json"]);
    assert!(artifact_repair_candidates(root, "generic", "goal").is_empty());
    let captured = capture(root, "generic", "goal", &expected, &[]);
    let legacy = capture_with_artifacts(root, "generic", "goal", &expected, &[], &[]);
    assert_eq!(format!("{captured:?}"), format!("{legacy:?}"));
    assert!(
        !captured.contains_key("contract/terms.json"),
        "{captured:?}"
    );
}

#[test]
fn the_boundary_refuses_escape_absolute_hidden_and_runtime_paths() {
    let declared = strings(&[
        "../outside.json",
        "/etc/terms.json",
        "contract/../../x.json",
        ".env",
        "config/.secret.yaml",
        ".anvil/runs/r.json",
        ".commandagent/e.json",
        "node_modules/pkg/package.json",
        "contract/terms.json",
        "./contract/terms.json",
        "",
    ]);
    assert_eq!(
        artifact_candidates(
            &declared,
            &strings(&["check/request.json", "contract/terms.json"])
        ),
        ["contract/terms.json", "check/request.json"]
    );
}

#[test]
fn credential_artifacts_are_dropped_unread() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(root, "keys/service-account-x.json", "FAKE_SECRET_VALUE");
    let bound = ["check/request.json", "keys/service-account-x.json"];
    let before = snap(root, &[], &bound, &[]);
    assert!(
        !before.contains_key("keys/service-account-x.json"),
        "{before:?}"
    );
    assert!(!format!("{before:?}").contains("FAKE_SECRET_VALUE"));
    write(root, "keys/service-account-x.json", "FAKE_SECRET_CHANGED");
    assert_eq!(
        changed_snapshot_paths(&before, &snap(root, &[], &bound, &[])),
        Vec::<String>::new()
    );
}

#[test]
fn fingerprints_are_digests_never_artifact_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(root, "contract/terms.yaml", "marker: FAKE_BODY_MARKER\n");
    let snapshot = snap(root, &["contract/terms.yaml"], &["check/request.json"], &[]);
    let value = String::from_utf8(snapshot["contract/terms.yaml"].clone().unwrap()).unwrap();
    assert!(value.starts_with("sha256:"), "{value}");
    assert!(!format!("{snapshot:?}").contains("FAKE_BODY_MARKER"));
}

#[test]
fn oversized_artifact_is_unknown_and_never_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(root, "contract/big.json", &"a".repeat(1024 * 1024 + 1));
    let bound = ["check/request.json", "contract/big.json"];
    let before = snap(root, &[], &bound, &[]);
    assert_eq!(
        changed_snapshot_paths(&before, &snap(root, &[], &bound, &[])),
        ["contract/big.json"]
    );
}

#[test]
fn artifacts_share_the_total_byte_budget() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let mut bound = vec!["check/request.json".to_string()];
    for index in 0..17 {
        let rel = format!("contract/m{index:02}.json");
        write(root, &rel, &"a".repeat(1024 * 1024));
        bound.push(rel);
    }
    let bound = bound.iter().map(String::as_str).collect::<Vec<_>>();
    let before = snap(root, &[], &bound, &[]);
    assert_eq!(
        changed_snapshot_paths(&before, &snap(root, &[], &bound, &[])),
        ["contract/m16.json"]
    );
}

#[test]
fn the_candidate_cap_still_bounds_the_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let mut bound = Vec::new();
    for index in 0..140 {
        let rel = format!("contract/c{index:03}.json");
        write(root, &rel, "{}\n");
        bound.push(rel);
    }
    let bound = bound.iter().map(String::as_str).collect::<Vec<_>>();
    let before = snap(root, &[], &bound, &[]);
    assert_eq!(before.len(), 128);
    assert!(before.contains_key("contract/c000.json"));
    assert!(!before.contains_key("contract/c139.json"));
}

#[cfg(unix)]
#[test]
fn a_symlink_escape_is_unknown_and_its_target_is_never_read() {
    let outside = tempfile::tempdir().unwrap();
    write(outside.path(), "terms.json", "FAKE_OUTSIDE_BODY");
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir_all(root.join("contract")).unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("terms.json"),
        root.join("contract/terms.json"),
    )
    .unwrap();
    let declared = ["contract/terms.json"];
    let before = snap(root, &declared, &["check/request.json"], &[]);
    assert!(!format!("{before:?}").contains("FAKE_OUTSIDE_BODY"));
    assert_eq!(
        changed_snapshot_paths(
            &before,
            &snap(root, &declared, &["check/request.json"], &[])
        ),
        ["contract/terms.json"]
    );
}

#[cfg(unix)]
#[test]
fn a_special_file_is_unknown_and_never_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir_all(root.join("contract")).unwrap();
    let fifo = std::ffi::CString::new(
        root.join("contract/terms.json")
            .to_string_lossy()
            .as_bytes(),
    )
    .unwrap();
    // SAFETY: `fifo` is a NUL-terminated path inside a temporary directory.
    assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
    let declared = ["contract/terms.json"];
    let before = snap(root, &declared, &["check/request.json"], &[]);
    assert_eq!(
        changed_snapshot_paths(
            &before,
            &snap(root, &declared, &["check/request.json"], &[])
        ),
        ["contract/terms.json"]
    );
}

#[cfg(unix)]
#[test]
fn an_unreadable_artifact_is_unknown() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(root, "contract/terms.yaml", "state: draft\n");
    let path = root.join("contract/terms.yaml");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();
    if std::fs::File::open(&path).is_ok() {
        return;
    }
    let declared = ["contract/terms.yaml"];
    let before = snap(root, &declared, &["check/request.json"], &[]);
    assert_eq!(
        changed_snapshot_paths(
            &before,
            &snap(root, &declared, &["check/request.json"], &[])
        ),
        ["contract/terms.yaml"]
    );
}

/// B3: a bound name is compared verbatim. With a trailing-space input and a
/// look-alike file without the space side by side, only the real input's
/// change counts.
#[test]
fn a_trailing_space_bound_name_is_compared_verbatim() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(root, "data/terms.json ", "{\"state\": \"draft\"}\n");
    write(root, "data/terms.json", "{\"state\": \"draft\"}\n");
    let bound = [REQUEST, "data/terms.json "];
    let before = snap(root, &[], &bound, &[]);
    assert!(before.contains_key("data/terms.json "), "{before:?}");
    assert!(!before.contains_key("data/terms.json"), "{before:?}");
    write(root, "data/terms.json", "{\"state\": \"ready\"}\n");
    assert_eq!(
        changed_snapshot_paths(&before, &snap(root, &[], &bound, &[])),
        Vec::<String>::new()
    );
    write(root, "data/terms.json ", "{\"state\": \"ready\"}\n");
    assert_eq!(
        changed_snapshot_paths(&before, &snap(root, &[], &bound, &[])),
        ["data/terms.json "]
    );
}

/// B3: on POSIX `\` is part of a file name, never a separator.
#[cfg(unix)]
#[test]
fn a_backslash_bound_name_is_not_turned_into_a_separator() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(root, "data\\terms.json", "{\"state\": \"draft\"}\n");
    write(root, "data/terms.json", "{\"state\": \"draft\"}\n");
    let bound = [REQUEST, "data\\terms.json"];
    let before = snap(root, &[], &bound, &[]);
    assert!(before.contains_key("data\\terms.json"), "{before:?}");
    assert!(!before.contains_key("data/terms.json"), "{before:?}");
    write(root, "data/terms.json", "{\"state\": \"ready\"}\n");
    assert_eq!(
        changed_snapshot_paths(&before, &snap(root, &[], &bound, &[])),
        Vec::<String>::new()
    );
    write(root, "data\\terms.json", "{\"state\": \"ready\"}\n");
    assert_eq!(
        changed_snapshot_paths(&before, &snap(root, &[], &bound, &[])),
        ["data\\terms.json"]
    );
}

#[test]
fn verbatim_bound_names_still_refuse_escape_hidden_runtime_and_credentials() {
    let bound = strings(&[
        "../outside.json",
        "/etc/terms.json",
        "./data/terms.json",
        "data/../terms.json",
        "data/.hidden.json",
        ".anvil/runs/r.json",
        "pkg/node_modules/x.json",
        "keys/service-account-x.json",
        "",
        "data/terms.json ",
    ]);
    assert_eq!(artifact_candidates(&[], &bound), ["data/terms.json "]);
}

/// N1: what one admitted run records, by how far the request got.
#[test]
fn recorded_names_follow_the_request_and_operator_input_ids() {
    let registry = registry(None);
    let cases = [
        (
            "unparsable request",
            Some("{not json".to_string()),
            vec![REQUEST],
        ),
        ("missing request", None, vec![REQUEST]),
        (
            "input ID differs from the operator's",
            Some(request("other", "data/terms.json")),
            vec![REQUEST],
        ),
        (
            "missing input",
            Some(request("terms", "data/terms.json")),
            vec![REQUEST, "data/terms.json"],
        ),
    ];
    for (label, body, expected) in cases {
        let dir = tempfile::tempdir().unwrap();
        if let Some(body) = body {
            write(dir.path(), REQUEST, &body);
        }
        assert_eq!(recorded(&registry, "gate", dir.path()), expected, "{label}");
    }
}

/// N1: an input that fails its operator pin is still a named repair
/// candidate, so repairing it reruns the check, which keeps failing until the
/// bytes match the pin.
#[test]
fn an_input_failing_its_operator_pin_stays_a_candidate() {
    let registry = registry(Some(&"0".repeat(64)));
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), REQUEST, &request("terms", "data/terms.json"));
    write(dir.path(), "data/terms.json", "{}\n");
    let check = ArtifactContractCheck {
        adapter_id: "gate".to_string(),
        request: REQUEST.to_string(),
    };
    let receipt = run_check(dir.path(), &check, Some(&registry.config), || false);
    assert_eq!(receipt.stopped_at, "input");
    assert_eq!(
        bound_workspace_paths(dir.path()),
        [REQUEST, "data/terms.json"]
    );
}

/// N1: a registry stop records nothing, so the snapshot stays legacy.
#[test]
fn an_unregistered_adapter_records_nothing() {
    let registry = registry(None);
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), REQUEST, &request("terms", "data/terms.json"));
    assert!(recorded(&registry, "absent", dir.path()).is_empty());
    assert!(artifact_repair_candidates(dir.path(), "generic", "goal").is_empty());
}

/// N1: an input the binding refuses lexically (a credential name) is never
/// recorded, even though the request names the operator's input ID.
#[test]
fn a_credential_input_name_is_never_recorded() {
    let registry = registry(None);
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        REQUEST,
        &request("terms", "keys/service-account-x.json"),
    );
    assert_eq!(recorded(&registry, "gate", dir.path()), [REQUEST]);
}
