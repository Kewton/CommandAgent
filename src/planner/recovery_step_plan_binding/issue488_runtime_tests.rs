use super::*;

fn raw(root: &Path, command: &str) -> crate::bounded_process::BoundedProcessOutput {
    crate::bounded_process::run_with_timeout(
        std::process::Command::new("sh")
            .args(["-c", command])
            .current_dir(root)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped()),
        std::time::Duration::from_secs(5),
    )
    .unwrap()
}

#[test]
fn issue488_real_node_matrix_preserves_classification_and_failure_observations() {
    let cases: Vec<serde_json::Value> = serde_json::from_str(&fixture("matrix.json")).unwrap();
    assert_eq!(cases.len(), 24);
    let mut observations = Vec::new();
    for case in cases {
        let name = case["command"].as_str().unwrap();
        let variant = case["fixture"].as_str().unwrap();
        let (root, c, _guard, _) = setup();
        let page = root.path().join("src/app/page.tsx");
        if variant == "file_missing" {
            std::fs::remove_file(page).unwrap();
        } else {
            std::fs::write(page, fixture(&format!("fixtures/{variant}.txt"))).unwrap();
        }
        let p = proposal_plan(name);
        let command = &p.steps[0].verify[0];
        let output = raw(root.path(), command);
        // Use the classifier's own literal parser, not shlex or a recorder's
        // imitation. The exact argv below is also supplied to a real process.
        let source = crate::minimal_loop::evidence::import_check::source(command).unwrap();
        let direct = crate::bounded_process::run_with_timeout(
            std::process::Command::new("node")
                .args(["-e", &source])
                .current_dir(root.path())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped()),
            std::time::Duration::from_secs(5),
        )
        .unwrap();
        assert_eq!(direct.status, output.status);
        assert_eq!(direct.stdout, output.stdout);
        assert_eq!(direct.stderr, output.stderr);
        let expected = case["execution_pass"].as_bool().unwrap();
        assert_eq!(output.success(), expected, "{name}/{variant}: {output:?}");
        if expected {
            assert_eq!(output.stdout, b"ok\n");
            assert!(output.stderr.is_empty());
        } else {
            assert!(output.stdout.is_empty());
            assert!(!output.stderr.is_empty());
        }
        let report = crate::planner::verify::verify_step(root.path(), &p.steps[0]);
        assert_eq!(report.is_pass(), expected, "{name}/{variant}: {report:?}");
        let diagnosis = command_diagnosis::collect(root.path(), std::slice::from_ref(command));
        assert_eq!(diagnosis[0].kind, case["kind"]);
        let accepted = acceptance(&c, std::slice::from_ref(command));
        assert_eq!(
            accepted
                .weak_evidence
                .iter()
                .any(|w| w == "node_smoke_without_assertion"),
            case["static_weak_evidence"].as_bool().unwrap()
        );
        let decision = admission::Admission::default()
            .check(&c, None, &p, &mut p.clone(), 1)
            .unwrap();
        assert_eq!(
            matches!(decision, admission::Decision::Retry(_)),
            case["formation"] == "retry"
        );
        assert_eq!(
            scope::register(&c, &p).is_ok(),
            case["registration"] == "accepted"
        );
        observations.push(json!({"name":name,"fixture":variant,"command":command,
            "classifier_parser_argv":["node", "-e", source], "shell_and_direct_outcomes_match":true,
            "exit_code":output.status.and_then(|s|s.code()),"success":output.success(),
            "stdout":String::from_utf8_lossy(&output.stdout),
            "stderr_sha256":format!("{:x}",Sha256::digest(&output.stderr)),
            "stderr_failure":String::from_utf8_lossy(&output.stderr).lines().find(|l| l.starts_with("Error:") || l.starts_with("AssertionError")),
            "classification":diagnosis[0].kind, "execution_pass":report.is_pass(),
            "static_acceptance":accepted.passed,"formation":case["formation"],"registration":case["registration"]}));
    }
    if let Ok(path) = std::env::var("ISSUE488_NODE_EVIDENCE") {
        std::fs::write(path, serde_json::to_vec_pretty(&observations).unwrap()).unwrap();
    }
}
