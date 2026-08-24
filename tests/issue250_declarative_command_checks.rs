use std::path::{Path, PathBuf};

use commandagent::planner::pack::catalog::{PackLocator, PackSource};
use commandagent::planner::profile::{ProfileId, ProfileRuntimeRegistry};

fn case_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/corpus/apps/issue250-declarative-command-checks")
}

#[test]
fn draft_manifest_runs_registered_command_check_and_stays_static() {
    let case = case_root();
    let extension = case.join("extension-root");
    commandagent::planner::extension_profiles::register(&extension).unwrap();

    let workspace = tempfile::tempdir().unwrap();
    std::fs::write(
        workspace.path().join("index.html"),
        "<!doctype html><title>Green Tea</title>",
    )
    .unwrap();
    let events = workspace.path().join("run/events.jsonl");
    let runtime = ProfileRuntimeRegistry::resolve(&ProfileId::parse("static-site"));
    let report = runtime.verify_final_with_events(
        workspace.path(),
        "Create a Green Tea static site",
        Some(&events),
    );
    assert!(report.is_pass(), "{}", report.primary_reason());

    let emitted = std::fs::read_to_string(&events).unwrap();
    assert!(emitted.contains(r#""event":"declarative_command_check_result""#));
    assert!(emitted.contains(r#""source":"draft_profile""#));
    assert!(emitted.contains(r#""status":"passed""#));
    let summary = std::fs::read_to_string(events.parent().unwrap().join("summary.md")).unwrap();
    assert!(summary.contains("draft_profile `static-site`"));
    assert!(summary.contains("1/1 passed"));

    let located = PackLocator::with_extension_root(env!("CARGO_MANIFEST_DIR"), Some(extension))
        .locate_pinned("static-validation", "1.0.0", None)
        .unwrap();
    assert_eq!(located.source, PackSource::Local);
    assert_eq!(
        located.hash,
        "sha256:29aa7ba26cadb78a6a3fee4772bc6a089bc33c4034981dd422e32494247c98c4"
    );

    let terminal = commandagent::eval_events::latest_completion_snapshot(Some(
        &case.join("fixtures/terminal.jsonl"),
    ));
    assert_eq!(terminal.effective_profile, "static-site");
    assert_eq!(terminal.assurance_level, "static");
    assert_eq!(terminal.assurance_reason, "profile_not_admitted");
}
