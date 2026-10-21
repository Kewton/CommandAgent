use super::*;
use crate::planner::recovery_snapshot::{current_source_sha256, source_file_sha256};
use serde_json::Value;

const FIXTURE: &str = "tests/corpus/apps/issue467-json-control";

fn workspace(mode: &str) -> (tempfile::TempDir, Config) {
    let root = tempfile::tempdir().unwrap();
    for path in [
        "package.json",
        "src/lib/store.js",
        "src/app/api/inquiries/route.js",
        "probe.mjs",
    ] {
        let dest = root.path().join(path);
        std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
        std::fs::copy(Path::new(FIXTURE).join(path), dest).unwrap();
    }
    std::fs::write(root.path().join("control.txt"), "original").unwrap();
    let mut config = config(root.path(), 1);
    let contract = root.path().join("completion-contract.json");
    std::fs::write(
        &contract,
        serde_json::to_vec(&json!({
            "profile": crate::planner::profiles::nextjs::PROFILE_ID,
            "required_paths":["src/lib/store.js", "src/app/api/inquiries/route.js", "probe.mjs"],
            "verify_commands":[format!("node probe.mjs {mode}")]
        }))
        .unwrap(),
    )
    .unwrap();
    config.completion_contract_path = Some(contract);
    config.offline = true;
    (root, config)
}

fn events(config: &Config) -> Vec<Value> {
    std::fs::read_to_string(config.eval_events_path.as_ref().unwrap())
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn audit(events: &[Value]) -> &Value {
    let audits: Vec<_> = events
        .iter()
        .filter(|e| e["event"] == "recovery_preflight_control_audit")
        .collect();
    assert_eq!(audits.len(), 1, "exactly one audit per preflight");
    audits[0]
}

#[test]
fn issue467_real_preflight_audits_every_exit_and_actual_restore() {
    let cases: Value = serde_json::from_str(include_str!(
        "../../../tests/corpus/apps/issue467-json-control/audit-cases.json"
    ))
    .unwrap();
    for case in cases.as_array().unwrap() {
        if case["unix"] == true && !cfg!(unix) {
            continue;
        }
        let name = case["name"].as_str().unwrap();
        let (root, config) = workspace(case["mode"].as_str().unwrap());
        let probe = root.path().join("probe.mjs");
        let extra = format!(
            "\nconst control = {};\n{}\n",
            serde_json::to_string(root.path()).unwrap(),
            case["script"].as_str().unwrap()
        );
        std::fs::write(&probe, std::fs::read_to_string(&probe).unwrap() + &extra).unwrap();
        let before = current_source_sha256(root.path()).unwrap();
        let observed = recovery_preflight(&config, &candidate(name), 0);
        match case["outcome"].as_str().unwrap() {
            "pass" => assert!(
                matches!(&observed, RecoveryPreflight::CurrentSuccess { .. }),
                "{name}: {observed:?}"
            ),
            "fail" => assert!(
                matches!(&observed, RecoveryPreflight::Failed { .. }),
                "{name}: {observed:?}"
            ),
            _ => assert!(
                matches!(&observed, RecoveryPreflight::Unavailable { reason } if reason.starts_with(case["reason"].as_str().unwrap())),
                "{name}: {observed:?}"
            ),
        }
        let events = events(&config);
        let audit = audit(&events);
        assert_eq!(audit["control_audited"], true, "{name}");
        assert_eq!(audit["control_status"], case["control"], "{name}");
        assert_eq!(audit["restore_invoked"], case["restore"], "{name}");
        assert_eq!(audit["control_retained"], case["retained"], "{name}");
        assert_eq!(
            audit["restore_succeeded"],
            case["restore"] == true && case["retained"] == true,
            "{name}"
        );
        assert_eq!(audit["control_before_sha256"], before, "{name}");
        if case["retained"] == true {
            assert_eq!(
                current_source_sha256(root.path()).unwrap(),
                before,
                "{name}"
            );
            assert_eq!(
                std::fs::read_to_string(root.path().join("control.txt")).unwrap(),
                "original"
            );
        }
        if let Some(expected) = case["control_bytes_after"].as_str() {
            assert_eq!(
                std::fs::read_to_string(root.path().join("control.txt")).unwrap(),
                expected,
                "{name}: invalid restoration source must not overwrite control"
            );
            assert!(audit.get("restore_error").is_some());
        }
        if case["restore"] == false {
            assert!(
                audit.get("control_after_restore_sha256").is_none(),
                "{name}"
            );
            assert!(audit.get("restored_file_count").is_none(), "{name}");
        }
        if case["mode"] == "extra" {
            assert_eq!(
                audit["isolated_reason"],
                "preflight_source_mutation_rejected_and_restored"
            );
        }
        if name.starts_with("observation-error") {
            assert!(
                audit["isolated_reason"]
                    .as_str()
                    .unwrap()
                    .starts_with("preflight_source_observation_failed:")
            );
        }
        assert!(!root.path().join("data/inquiries.json").exists(), "{name}");
    }
}

#[test]
fn issue467_generated_json_delta_is_exact_and_business_failure_stays_failed() {
    for existing in [false, true] {
        for mode in ["business-failure", "extra"] {
            let (root, config) = workspace(mode);
            if existing {
                std::fs::create_dir(root.path().join("data")).unwrap();
                // Malformed shape is not the focus; the minimum reader returns
                // existing valid business data without an initialization write.
                std::fs::write(
                    root.path().join("data/inquiries.json"),
                    r#"{"inquiries":[{"id":"existing"}]}"#,
                )
                .unwrap();
            }
            let before = source_file_sha256(root.path()).unwrap();
            let result = recovery_preflight(&config, &candidate("T3"), 0);
            assert!(!matches!(result, RecoveryPreflight::CurrentSuccess { .. }));
            if mode == "business-failure" {
                assert!(matches!(result, RecoveryPreflight::Failed { .. }));
            }
            let events = events(&config);
            let effect = events
                .iter()
                .find(|e| {
                    e["event"] == "recovery_preflight_effect_observation"
                        && e["stage"] == "registered_verification"
                })
                .unwrap();
            assert_eq!(effect["scope"], "isolated_workspace");
            assert_eq!(
                effect["allowed_generated_paths"],
                json!(["data/inquiries.json"])
            );
            assert_eq!(effect["first_write_within_stage"], "unknown");
            let changes = effect["changes"].as_array().unwrap();
            let observation = root
                .path()
                .join(".commandagent/recovery-observations/attempt-0/workspace");
            let after = source_file_sha256(&observation).unwrap();
            assert_eq!(
                changes.len(),
                usize::from(!existing) + usize::from(mode == "extra")
            );
            for change in changes {
                let path = change["path"].as_str().unwrap();
                assert_eq!(change["before_sha256"], Value::Null);
                assert_eq!(change["after_sha256"], after[path]);
                assert_eq!(change["allowed_generated"], path == "data/inquiries.json");
            }
            assert_eq!(source_file_sha256(root.path()).unwrap(), before);
            assert_eq!(audit(&events)["restore_invoked"], false);
        }
    }
}

#[test]
fn issue467_prepare_failure_still_audits_control() {
    let (root, config) = workspace("pass");
    std::fs::create_dir_all(
        root.path()
            .join(".commandagent/recovery-observations/attempt-0/workspace"),
    )
    .unwrap();
    let observed = recovery_preflight(&config, &candidate("prepare-error"), 0);
    assert!(
        matches!(observed, RecoveryPreflight::Unavailable { reason } if reason.starts_with("preflight_observation_prepare_failed:"))
    );
    let events = events(&config);
    let audit = audit(&events);
    assert_eq!(audit["control_status"], "unchanged");
    assert_eq!(audit["restore_invoked"], false);
}

#[test]
fn issue467_browser_observer_error_cannot_skip_control_audit_or_grant_success() {
    let (_root, mut config) = workspace("pass");
    let path = config.completion_contract_path.as_ref().unwrap();
    let mut contract: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    contract["required_capabilities"] = json!(["persistence"]);
    contract["goal"] = json!("Observe inquiries on port 61347");
    std::fs::write(path, serde_json::to_vec(&contract).unwrap()).unwrap();
    config.offline = true;
    let result = recovery_preflight(&config, &candidate("browser-error"), 0);
    assert!(
        matches!(
            result,
            RecoveryPreflight::Unavailable { .. } | RecoveryPreflight::Failed { .. }
        ),
        "{result:?}"
    );
    let events = events(&config);
    assert_eq!(audit(&events)["control_status"], "unchanged");
    assert_eq!(audit(&events)["restore_invoked"], false);
    assert!(
        events
            .iter()
            .any(|e| e["event"] == "recovery_preflight_effect_observation"
                && e["stage"] == "nextjs_capabilities")
    );
}

#[test]
fn issue467_equal_control_hashes_keep_pre_continuation_and_post_observations_distinct() {
    let (root, config) = workspace("business-failure");
    let before = current_source_sha256(root.path()).unwrap();
    let cases: Value = serde_json::from_str(include_str!(
        "../../../tests/corpus/apps/issue467-json-control/observation-ids.json"
    ))
    .unwrap();
    for case in cases.as_array().unwrap() {
        let attempt = u8::try_from(case["checkpoint_attempt"].as_u64().unwrap()).unwrap();
        assert!(matches!(
            recovery_preflight(&config, &candidate("T3"), attempt),
            RecoveryPreflight::Failed { .. }
        ));
    }
    let events = events(&config);
    for case in cases.as_array().unwrap() {
        let matched: Vec<_> = events
            .iter()
            .filter(|e| e["observation_id"] == case["observation_id"])
            .cloned()
            .collect();
        assert_eq!(matched.len(), 3, "one audit and two observed boundaries");
        assert_eq!(audit(&matched)["control_before_sha256"], before);
        for stage in ["before_observation", "registered_verification"] {
            assert_eq!(
                matched
                    .iter()
                    .filter(|e| e["event"] == "recovery_preflight_effect_observation"
                        && e["stage"] == stage)
                    .count(),
                1
            );
        }
    }
    assert_eq!(current_source_sha256(root.path()).unwrap(), before);
}
