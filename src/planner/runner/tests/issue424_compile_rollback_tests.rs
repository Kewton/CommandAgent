#[test]
#[cfg(unix)]
fn compile_rollback_rejects_a_noncompiling_snapshot_and_accounts_build_time() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join(".anvil/runs/broken-rollback/events.jsonl");
    write_static_compile_repair_workspace(dir.path(), static_broken_page_source());
    let mut cfg = config(dir.path().to_path_buf());
    cfg.profile = "nextjs".to_string();
    cfg.eval_events_path = Some(events.clone());
    snapshot_last_known_good_sources(
        &cfg,
        "test",
        Some("inspection-only"),
        "nextjs",
        "Create a static Next.js page",
        &["src/app/page.tsx".to_string()],
    );
    let mut report = VerificationReport::pass();
    report.push_compile_errors(
        "npm run build",
        vec![CompileError {
            path: "src/app/page.tsx".to_string(),
            line: 12,
            column: 1,
            message: "Expected ';', '}' or <eof>".to_string(),
            excerpt: "12 | BROKEN_SYNTAX\n   | ^".to_string(),
            symbol: None,
            route_bound: Some(true),
        }],
    );

    let rollback = try_compile_rollback_after_repair_exhaustion(
        &cfg,
        "nextjs",
        "Create a static Next.js page",
        "core-implementation",
        "Run the production build.",
        &report,
        "bounded_repair_exhausted",
    )
    .unwrap();

    assert!(rollback.is_none());
    let event_text = std::fs::read_to_string(&events).unwrap();
    assert!(
        event_text.contains("\"event\":\"compile_rollback_failed\""),
        "{event_text}"
    );
    assert!(
        event_text.contains("\"build_reverified\":false"),
        "{event_text}"
    );
    assert!(
        event_text.contains("\"build_reverification_status\":\"failed\""),
        "{event_text}"
    );
    assert!(
        event_text.contains("\"build_duration_ms\":"),
        "{event_text}"
    );
    assert!(
        !event_text.contains("\"event\":\"compile_rollback_applied\""),
        "{event_text}"
    );
    assert!(
        crate::time_profile::aggregate_event_path(Some(&events)).builds_ms > 0,
        "{event_text}"
    );
}
