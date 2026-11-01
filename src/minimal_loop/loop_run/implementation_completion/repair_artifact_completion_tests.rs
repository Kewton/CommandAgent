//! Issue #956 leaf tests: the options table, the raw-byte decision, the
//! bounds, and the refusals of the host-bound final-repair obligation. Every
//! file lives in a temporary directory; no provider, model, or network.

use std::cell::Cell;
use std::path::Path;

use clap::Parser;

use super::super::ImplementationCompletion;
use super::{Fingerprint, MAX_FILE_BYTES, MAX_TARGETS, MAX_TOTAL_BYTES, RepairArtifactCompletion};
use crate::config::Config;
use crate::minimal_loop::loop_run::{RunSessionOptions, RunSessionScope, RunSessionStepKind};
use crate::minimal_loop::stagnation_carryover::{EscalationCarryoverHandle, attach_to_options};

thread_local! {
    /// Targets the bounded reader visited on this thread.
    pub(super) static TARGET_VISITS: Cell<usize> = const { Cell::new(0) };
}

fn visits() -> usize {
    TARGET_VISITS.with(Cell::get)
}

fn reset_visits() {
    TARGET_VISITS.with(|visits| visits.set(0));
}

const TARGET: &str = "contract/terms.json";
const OTHER: &str = "contract/schema.yaml";
const BROKEN: &[u8] = b"{\"state\": \"draft\"\n";
const FIXED: &[u8] = b"{\"state\": \"ready\"}\n";

fn workspace() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), TARGET, BROKEN);
    write(dir.path(), OTHER, b"version: 1\n");
    dir
}

fn write(root: &Path, rel: &str, bytes: &[u8]) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

fn bound(options: RunSessionOptions, targets: &[&str]) -> RunSessionOptions {
    let handle = EscalationCarryoverHandle::new();
    handle.bind_repair_targets(&targets.iter().map(|t| t.to_string()).collect::<Vec<_>>());
    attach_to_options(options, handle)
}

fn final_repair(targets: &[&str]) -> RunSessionOptions {
    bound(RunSessionOptions::final_acceptance_repair(), targets)
}

fn capture(root: &Path, options: &RunSessionOptions) -> RepairArtifactCompletion {
    RepairArtifactCompletion::capture(root, options)
}

fn config(root: &Path) -> Config {
    let mut config =
        Config::from_cli(crate::cli::Cli::parse_from(["commandagent", "--ux-demo"])).unwrap();
    config.workspace_root = root.to_path_buf();
    config.eval_events_path = Some(root.join(".commandagent/events.jsonl"));
    config
}

fn held(root: &Path, completion: &RepairArtifactCompletion) -> bool {
    completion.held(root, false).is_some()
}

// --- options table -------------------------------------------------------

/// Row 1 and 2: the canonical final repair (with its carryover, as the
/// repair loop runs it) and a profile repair that reaches the same
/// constructor are active only with a host binding, which the session takes
/// once.
#[test]
fn a_host_bound_final_repair_is_active_and_takes_the_binding_once() {
    let dir = workspace();
    let options = final_repair(&[TARGET]);
    let completion = capture(dir.path(), &options);
    assert!(completion.active());
    assert!(held(dir.path(), &completion));
    let carryover = options.escalation_carryover.as_ref().unwrap();
    assert_eq!(carryover.take_repair_targets(), None);
    reset_visits();
    assert!(!capture(dir.path(), &options).active());
    assert_eq!(visits(), 0);
}

/// The same constructor with no binding, with an empty binding, or with no
/// carryover at all keeps today's behavior and reads nothing.
#[test]
fn a_final_repair_without_a_host_binding_is_inactive_and_reads_nothing() {
    let dir = workspace();
    for options in [
        RunSessionOptions::final_acceptance_repair(),
        attach_to_options(
            RunSessionOptions::final_acceptance_repair(),
            EscalationCarryoverHandle::new(),
        ),
        final_repair(&[]),
    ] {
        reset_visits();
        let completion = capture(dir.path(), &options);
        assert!(!completion.active());
        assert!(!held(dir.path(), &completion));
        assert_eq!(visits(), 0);
    }
}

/// Model-probe product options (`src/model_probe.rs` probe_run_options):
/// plan-run step, no mutation flag, no carryover, no Recovery obligation.
fn model_probe_options(step_kind: RunSessionStepKind) -> RunSessionOptions {
    let mut options = RunSessionOptions::plan_step(step_kind);
    options.phase_scope = Some("model-probe".to_string());
    options.require_mutation_before_contract_short_circuit = false;
    options
}

/// Row 5 and 6: an ordinary Implement step, a direct prompt (the generic
/// minimal loop of the CA bench: scope MinimalLoop, no step kind),
/// model-probe options, and Verify/Inspect/Setup steps (also with a
/// carryover and the mutation flag) never take on the obligation, even when
/// a binding is present, and read no target.
#[test]
fn options_alone_never_enable_it_and_read_nothing() {
    let dir = workspace();
    let direct = RunSessionOptions::default();
    assert_eq!(direct.scope, RunSessionScope::MinimalLoop);
    assert_eq!(direct.step_kind, None);
    let mut cases = vec![
        RunSessionOptions::plan_step(RunSessionStepKind::Implement),
        direct,
        model_probe_options(RunSessionStepKind::Implement),
        model_probe_options(RunSessionStepKind::Verify),
    ];
    for kind in [
        RunSessionStepKind::Verify,
        RunSessionStepKind::Inspect,
        RunSessionStepKind::Setup,
    ] {
        cases.push(
            RunSessionOptions::plan_step(kind).with_required_mutation_before_short_circuit(true),
        );
    }
    let mut minimal_implement = RunSessionOptions::final_acceptance_repair();
    minimal_implement.scope = RunSessionScope::MinimalLoop;
    cases.push(minimal_implement);
    for options in cases {
        for options in [options.clone(), bound(options, &[TARGET])] {
            reset_visits();
            let completion = capture(dir.path(), &options);
            assert!(!completion.active(), "{options:?}");
            assert!(!held(dir.path(), &completion));
            assert_eq!(visits(), 0, "{options:?}");
        }
    }
}

/// Row 4 (Issue #465): with a Recovery obligation the existing Recovery
/// decision has priority even with the mutation flag and a binding: a
/// pending defect stays unresolved, and the confirmed no-edit positive
/// (the defect resolved elsewhere, proven by the latest host check) passes
/// with no new raw-byte demand and no new read.
#[test]
fn a_recovery_obligation_keeps_the_issue465_decision() {
    use crate::planner::recovery_repair_obligation::tests::{options, plan, setup};
    let (_root, config) = setup();
    let options = bound(
        options(&config, &plan(), 0).with_required_mutation_before_short_circuit(true),
        &["src/api.js"],
    );
    let paths = vec!["src/api.js".to_string()];
    reset_visits();
    let implementation = ImplementationCompletion::capture(&config, &options, &paths);
    assert_eq!(visits(), 0);
    let pending = implementation.feedback(&config, &options, false).unwrap();
    assert_eq!(
        ImplementationCompletion::blocking_reason(&pending),
        "recovery_repair_unresolved"
    );
    let path = config.workspace_root.join("src/api.js");
    let source = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        source.replace("validateShift(input)", "validateShift(input, policy)"),
    )
    .unwrap();
    assert_eq!(implementation.feedback(&config, &options, false), None);
    assert_eq!(visits(), 0);
}

// --- what clears the obligation -------------------------------------------

/// Reading, a passing check, a self-report, a same-bytes rewrite, an
/// mtime-only touch, a change undone, a deletion, and a change of another
/// file never clear it.
#[test]
fn no_change_of_the_bound_bytes_keeps_it_held() {
    let dir = workspace();
    let root = dir.path();
    let completion = capture(root, &final_repair(&[TARGET]));
    assert!(held(root, &completion), "read only");
    write(root, TARGET, BROKEN);
    assert!(held(root, &completion), "same bytes");
    let file = std::fs::File::options()
        .write(true)
        .open(root.join(TARGET))
        .unwrap();
    file.set_modified(std::time::SystemTime::UNIX_EPOCH)
        .unwrap();
    drop(file);
    assert!(held(root, &completion), "mtime only");
    write(root, OTHER, b"version: 2\n");
    write(root, "helper/dummy.py", b"print(1)\n");
    assert!(held(root, &completion), "another file");
    write(root, TARGET, FIXED);
    write(root, TARGET, BROKEN);
    assert!(held(root, &completion), "changed and undone");
    std::fs::remove_file(root.join(TARGET)).unwrap();
    assert!(held(root, &completion), "deleted");
}

/// A raw-byte change of a bound target by any tool, a reformat included,
/// and creating a missing target hand the completion back.
#[test]
fn a_raw_byte_change_of_a_bound_target_hands_it_back() {
    let dir = workspace();
    let root = dir.path();
    let completion = capture(root, &final_repair(&[OTHER, TARGET]));
    write(root, TARGET, b"{\n  \"state\": \"draft\"\n");
    assert!(!held(root, &completion), "reformat is a change");
    let completion = capture(root, &final_repair(&["contract/missing.json"]));
    assert!(held(root, &completion));
    write(root, "contract/missing.json", FIXED);
    assert!(!held(root, &completion), "created");
}

/// A successful Write or Edit hands the candidate to the existing checks
/// whatever it wrote: the repair loop's raw-byte snapshot judges it, so the
/// bounded repair spends no extra provider call.
#[test]
fn a_write_or_edit_hands_it_back_to_the_existing_checks() {
    let dir = workspace();
    let completion = capture(dir.path(), &final_repair(&[TARGET]));
    assert!(completion.held(dir.path(), true).is_none());
    assert!(completion.held(dir.path(), false).is_some());
}

// --- bounds ---------------------------------------------------------------

fn names(count: usize) -> Vec<String> {
    (0..count)
        .map(|index| format!("t/{index:03}.json"))
        .collect()
}

fn bind_names(names: &[String]) -> RunSessionOptions {
    let handle = EscalationCarryoverHandle::new();
    handle.bind_repair_targets(names);
    attach_to_options(RunSessionOptions::final_acceptance_repair(), handle)
}

#[test]
fn the_target_count_is_bounded_at_128() {
    assert_eq!(MAX_TARGETS, 128);
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    for count in [127, 128, 129] {
        let names = names(count);
        for name in &names {
            write(root, name, b"{}\n");
        }
        reset_visits();
        let completion = capture(root, &bind_names(&names));
        assert!(completion.active());
        if count <= MAX_TARGETS {
            assert_eq!(visits(), count);
            write(root, &names[0], b"{\"a\":1}\n");
            assert!(!held(root, &completion), "{count}");
            write(root, &names[0], b"{}\n");
        } else {
            // Over the limit nothing is read, and no change confirms it.
            assert_eq!(visits(), 0);
            write(root, &names[0], b"{\"a\":1}\n");
            assert!(held(root, &completion), "{count}");
            assert_eq!(visits(), 0);
            write(root, &names[0], b"{}\n");
        }
    }
}

#[test]
fn one_file_is_adopted_up_to_1048576_bytes() {
    assert_eq!(MAX_FILE_BYTES, 1_048_576);
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    for (size, adopted) in [
        (1_048_575usize, true),
        (1_048_576, true),
        (1_048_577, false),
    ] {
        write(root, "big.json", &vec![b'a'; size]);
        let read = super::read_set(root, &["big.json".to_string()], false);
        assert_eq!(
            matches!(read.items[0].fingerprint, Fingerprint::Digest(_)),
            adopted,
            "{size}"
        );
        assert!(read.bytes_read <= 1_048_577, "{size}: {}", read.bytes_read);
        assert_eq!(read.bytes_read as usize, size.min(1_048_577));
    }
    // A file grown past the bound since the start is unknown, never changed.
    write(root, "big.json", &vec![b'a'; 1_048_576]);
    let completion = capture(root, &final_repair(&["big.json"]));
    write(root, "big.json", &vec![b'b'; 1_048_577]);
    assert!(held(root, &completion));
}

#[test]
fn the_set_is_adopted_up_to_8388608_bytes_and_reads_at_most_8388736() {
    assert_eq!(MAX_TOTAL_BYTES, 8_388_608);
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let mib = vec![b'a'; 1_048_576];
    let names = names(9);
    for name in &names[..7] {
        write(root, name, &mib);
    }
    // 7 MiB + (1 MiB - 1) = 8,388,607: all adopted.
    write(root, &names[7], &mib[..1_048_575]);
    let read = super::read_set(root, &names[..8], false);
    assert!(
        read.items
            .iter()
            .all(|item| matches!(item.fingerprint, Fingerprint::Digest(_)))
    );
    assert_eq!(read.bytes_read, 8_388_607);
    // Exactly 8 MiB: all adopted.
    write(root, &names[7], &mib);
    let read = super::read_set(root, &names[..8], false);
    assert!(
        read.items
            .iter()
            .all(|item| matches!(item.fingerprint, Fingerprint::Digest(_)))
    );
    assert_eq!(read.bytes_read, 8_388_608);
    // One byte over the set: the ninth file is unknown after one probe byte.
    write(root, &names[8], b"x");
    let read = super::read_set(root, &names, false);
    assert_eq!(
        read.items[8].fingerprint,
        Fingerprint::Unknown("over_limit")
    );
    assert_eq!(read.bytes_read, 8_388_609);
    // The worst case of 128 targets reads exactly the stated maximum: eight
    // files each one byte over the file bound use the whole set budget, and
    // each of the other 120 then costs its one probe byte.
    let all = self::names(128);
    let over = vec![b'a'; 1_048_577];
    for (index, name) in all.iter().enumerate() {
        write(root, name, if index < 8 { &over } else { b"x" });
    }
    let read = super::read_set(root, &all, false);
    assert!(
        read.items
            .iter()
            .all(|item| item.fingerprint == Fingerprint::Unknown("over_limit"))
    );
    assert_eq!(read.bytes_read, 8_388_736);
}

// --- refusals ---------------------------------------------------------------

fn fingerprint(root: &Path, rel: &str) -> Fingerprint {
    super::read_set(root, &[rel.to_string()], false).items[0]
        .fingerprint
        .clone()
}

#[cfg(unix)]
#[test]
fn escapes_credentials_symlinks_and_special_files_are_refused_unread() {
    use std::os::unix::fs::symlink;
    let dir = workspace();
    let outside = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(outside.path(), "secret.json", b"{}\n");
    write(root, ".env", b"TOKEN=not-a-real-value\n");
    symlink(root.join(TARGET), root.join("inside-link.json")).unwrap();
    symlink(
        outside.path().join("secret.json"),
        root.join("outside-link.json"),
    )
    .unwrap();
    symlink(root.join("contract"), root.join("linked-dir")).unwrap();
    std::fs::create_dir_all(root.join("a-dir")).unwrap();
    let fifo = root.join("pipe.json");
    let path = std::ffi::CString::new(fifo.to_string_lossy().as_bytes()).unwrap();
    // SAFETY: a valid NUL-terminated path inside the temporary directory.
    assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
    for (rel, reason) in [
        ("../outside.json", "escape"),
        ("/etc/hosts", "escape"),
        ("", "escape"),
        ("./contract/terms.json", "escape"),
        (".env", "sensitive"),
        ("inside-link.json", "symlink"),
        ("outside-link.json", "escape"),
        ("linked-dir/terms.json", "symlink"),
        ("a-dir", "special_file"),
        ("pipe.json", "special_file"),
    ] {
        let started = std::time::Instant::now();
        assert_eq!(
            fingerprint(root, rel),
            Fingerprint::Unknown(reason),
            "{rel}"
        );
        assert!(
            started.elapsed() < std::time::Duration::from_secs(5),
            "{rel}"
        );
    }
    assert_eq!(
        fingerprint(root, "contract/absent.json"),
        Fingerprint::Absent
    );
    // A refused target never confirms a change and is not named.
    let completion = capture(root, &final_repair(&[".env", "inside-link.json"]));
    write(root, ".env", b"TOKEN=changed\n");
    write(root, TARGET, FIXED);
    let held = completion.held(root, false).unwrap();
    assert!(!held.feedback.contains(".env"), "{}", held.feedback);
    assert!(!held.feedback.contains("TOKEN"), "{}", held.feedback);
    assert_eq!(held.unknown, 2);
}

// --- wiring of ImplementationCompletion ---------------------------------------

/// A bound target is read by the bounded reader for the step hashes too: a
/// symlinked target is never followed by the legacy reader, and the
/// blocking event carries counts, not content.
#[cfg(unix)]
#[test]
fn step_hashes_and_events_of_a_bound_target_use_the_bounded_reader() {
    let dir = workspace();
    let root = dir.path();
    std::os::unix::fs::symlink(root.join(TARGET), root.join("link.json")).unwrap();
    let config = config(root);
    let options = final_repair(&["link.json", TARGET]);
    let paths = vec![
        "link.json".to_string(),
        TARGET.to_string(),
        OTHER.to_string(),
    ];
    let implementation = ImplementationCompletion::capture(&config, &options, &paths);
    let feedback = implementation.feedback(&config, &options, false).unwrap();
    assert_eq!(
        ImplementationCompletion::blocking_reason(&feedback),
        "final_repair_artifact_unchanged"
    );
    let event = implementation.event(&config, &options, false);
    let hashes = event["expected_path_hashes"].as_array().unwrap();
    assert_eq!(hashes[0]["path"], "contract/schema.yaml");
    assert_eq!(hashes[1]["path"], "contract/terms.json");
    assert_eq!(hashes[2]["path"], "link.json");
    assert_eq!(hashes[2]["before_sha256"], "unknown:symlink");
    assert_eq!(hashes[2]["after_sha256"], "unknown:symlink");
    let events = std::fs::read_to_string(config.eval_events_path.as_ref().unwrap()).unwrap();
    let blocked = events
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .find(|event| event["reason"] == "final_repair_artifact_unchanged")
        .unwrap();
    assert_eq!(blocked["event"], "step_completion_blocked");
    assert_eq!(
        blocked["repair_binding"],
        serde_json::json!({"targets": 2, "unchanged": 1, "absent": 0, "unknown": 1})
    );
    assert!(!events.contains("draft"), "no content in events");
    write(root, TARGET, FIXED);
    assert_eq!(implementation.feedback(&config, &options, false), None);
    let mut changed = Vec::new();
    implementation.merge_changes(root, &mut changed);
    assert_eq!(changed, vec![TARGET.to_string()]);
}

/// Without a binding the step hashes are byte-for-byte the legacy ones and
/// the bounded reader is never used.
#[test]
fn an_unbound_step_keeps_the_legacy_hashes_and_reads() {
    let dir = workspace();
    let root = dir.path();
    let config = config(root);
    let options = RunSessionOptions::final_acceptance_repair();
    let paths = vec![TARGET.to_string()];
    reset_visits();
    let implementation = ImplementationCompletion::capture(&config, &options, &paths);
    let event = implementation.event(&config, &options, false);
    assert_eq!(implementation.feedback(&config, &options, false), None);
    assert_eq!(visits(), 0);
    let legacy = {
        use sha2::Digest;
        format!("{:x}", sha2::Sha256::digest(BROKEN))
    };
    assert_eq!(event["expected_path_hashes"][0]["before_sha256"], legacy);
    assert_eq!(event["expected_path_hashes"][0]["after_sha256"], legacy);
    assert!(event.get("repair_binding").is_none());
}

/// A binding over the target limit is refused as a whole: none of its
/// names is read by any path of the step, and it is held.
#[test]
fn a_refused_binding_never_falls_back_to_the_legacy_reader() {
    let dir = workspace();
    let root = dir.path();
    let config = config(root);
    let mut names = names(MAX_TARGETS);
    names.push(TARGET.to_string());
    let options = bind_names(&names);
    let paths = vec![TARGET.to_string()];
    reset_visits();
    let implementation = ImplementationCompletion::capture(&config, &options, &paths);
    write(root, TARGET, FIXED);
    let feedback = implementation.feedback(&config, &options, false).unwrap();
    assert!(feedback.contains("too_many_targets"), "{feedback}");
    let event = implementation.event(&config, &options, false);
    assert_eq!(
        event["expected_path_hashes"][0]["before_sha256"],
        "unknown:too_many_targets"
    );
    assert_eq!(visits(), 0);
}
