//! Issue #852: the compile last-known-good snapshot never copies, restores or
//! rolls back credential-named files. Values below are fixed fake strings.

use std::path::Path;

use clap::Parser;

use crate::cli::Cli;
use crate::planner::runner::final_acceptance::{
    compile_rollback_snapshot_available, final_acceptance_source_snapshot,
    snapshot_last_known_good_sources, try_compile_rollback_after_repair_exhaustion,
};
use crate::planner::runner::{CompileError, Config, VerificationReport};

const FAKE_SECRET: &str = "FAKE_SECRET_VALUE_852\n";
const PAGE: &str = "export default function Page(){return <main>ok</main>;}\n";
const SECRET_PATHS: [&str; 3] = [".env", ".env.js", "config/.env.local.js"];

fn config_for(root: &Path) -> Config {
    let mut config = Config::from_cli(Cli::parse_from([
        "commandagent",
        "--cwd",
        root.to_str().unwrap(),
        "--profile",
        "python",
    ]))
    .unwrap();
    config.eval_events_path = Some(root.join("run/events.jsonl"));
    config
}

fn write(root: &Path, rel: &str, text: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn events(root: &Path) -> String {
    std::fs::read_to_string(root.join("run/events.jsonl")).unwrap_or_default()
}

fn failing_report(path: &str) -> VerificationReport {
    let mut report = VerificationReport::pass();
    report.push_compile_errors(
        "npm run build",
        vec![CompileError {
            path: path.to_string(),
            line: 1,
            column: 1,
            message: "Expected ';'".to_string(),
            excerpt: "1 | BROKEN".to_string(),
            symbol: None,
            route_bound: Some(true),
        }],
    );
    report
}

#[test]
fn snapshot_skips_secret_named_files_and_keeps_ordinary_source() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    for rel in SECRET_PATHS {
        write(root, rel, FAKE_SECRET);
    }
    write(root, "src/app/page.tsx", PAGE);
    let extra = SECRET_PATHS
        .iter()
        .map(|rel| rel.to_string())
        .chain(["src/app/page.tsx".to_string()])
        .collect::<Vec<_>>();
    let config = config_for(root);

    snapshot_last_known_good_sources(&config, "phase", Some("p1"), "python", "goal", &extra);

    let latest = root.join("run/snapshots/latest");
    assert!(latest.join("src/app/page.tsx").is_file());
    for rel in SECRET_PATHS {
        assert!(!latest.join(rel).exists(), "{rel} was copied");
    }
    let text = events(root);
    assert!(text.contains("compile_snapshot_saved"));
    assert!(text.contains("src/app/page.tsx"));
    assert!(!text.contains(".env"), "secret name leaked into event");
    assert!(!text.contains("FAKE_SECRET_VALUE_852"));
    assert!(text.contains("\"snapshot_sensitive_skipped\":2"));
}

#[test]
fn restore_ignores_secret_paths_in_an_old_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(root, "run/snapshots/latest/.env.js", FAKE_SECRET);
    write(
        root,
        "run/snapshots/latest/config/.env.local.js",
        FAKE_SECRET,
    );
    let config = config_for(root);

    for rel in [".env.js", "config/.env.local.js"] {
        let report = failing_report(rel);
        assert!(!compile_rollback_snapshot_available(&config, &report));
        let outcome = try_compile_rollback_after_repair_exhaustion(
            &config,
            "python",
            "goal",
            "phase-one",
            "prompt",
            &report,
            "bounded_repair_exhausted",
        )
        .unwrap();
        assert!(outcome.is_none());
        assert!(!root.join(rel).exists(), "{rel} was written back");
    }
}

#[test]
fn repair_workspace_snapshot_does_not_fingerprint_secret_files() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    for rel in SECRET_PATHS {
        write(root, rel, FAKE_SECRET);
    }
    write(root, "src/app/page.tsx", PAGE);
    let extra = SECRET_PATHS
        .iter()
        .map(|rel| rel.to_string())
        .collect::<Vec<_>>();

    let captured = final_acceptance_source_snapshot(root, "python", "goal", &[], &extra, &[]);

    assert!(captured.contains_key("src/app/page.tsx"));
    for rel in SECRET_PATHS {
        assert!(!captured.contains_key(rel), "{rel} was fingerprinted");
    }
}
