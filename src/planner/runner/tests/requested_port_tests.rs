use super::*;

#[test]
fn requested_port_telemetry_labels_nextjs_default_and_explicit_goal() {
    let runtime = resolve_profile_runtime(crate::planner::profiles::nextjs::PROFILE_ID);
    let default = effective_requested_port(runtime, "ブラウザで使えるメモアプリ", None)
        .expect("nextjs default port");
    assert_eq!(default.port, 3011);
    assert_eq!(default.telemetry, "3011 (default)");

    let explicit =
        effective_requested_port(runtime, "4000番ポートで起動", None).expect("explicit port");
    assert_eq!(explicit.port, 4000);
    assert_eq!(explicit.telemetry, "4000 (goal)");
}

#[test]
fn requested_port_telemetry_leaves_python_cli_without_default_port() {
    assert_eq!(
        effective_requested_port(
            resolve_profile_runtime("python-cli"),
            "CSVを集計するCLI",
            None,
        ),
        None
    );
}
