use commandagent::planner::verify::{
    VerifyCommandViolationKind, diagnose_verify_command, normalize_planner_verify_command,
    validate_verify_command,
};

const RUN6_REJECTION: &str =
    include_str!("corpus/apps/test0716_data7b_quoted_lint/fixtures/run6-runtime-rejection.jsonl");

#[test]
fn measured_run6_quoted_python_payload_passes_the_shared_verify_boundary() {
    let event: serde_json::Value = serde_json::from_str(RUN6_REJECTION.trim()).unwrap();
    let command = event["original_command"].as_str().unwrap();

    let normalized = normalize_planner_verify_command(command).unwrap();
    assert_eq!(normalized.len(), 1);
    assert!(validate_verify_command(command).is_ok());
    assert_eq!(diagnose_verify_command(command).violation, None);
}

#[test]
fn unquoted_semicolon_remains_shell_control_syntax() {
    let command = "python -c \"print('ok')\"; echo bad";
    assert!(normalize_planner_verify_command(command).is_err());
    assert_eq!(
        diagnose_verify_command(command).violation,
        Some(VerifyCommandViolationKind::ShellControlSyntax)
    );
}
