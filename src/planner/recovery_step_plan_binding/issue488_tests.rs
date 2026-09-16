use super::*;
use crate::minimal_loop::evidence::{command_diagnosis, verify_runtime_acceptance_with_hints};
use sha2::{Digest, Sha256};

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/corpus/apps/issue488-d4-inline")
            .join(name),
    )
    .unwrap()
}

fn command(name: &str) -> String {
    let data: serde_json::Value = serde_json::from_str(&fixture("commands.json")).unwrap();
    let command = data[name]["command"].as_str().unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(command.as_bytes())),
        data[name]["sha256"]
    );
    command.into()
}

fn setup() -> (
    tempfile::TempDir,
    Config,
    authority::RunAuthorityGuard,
    std::path::PathBuf,
) {
    let root = tempfile::tempdir().unwrap();
    let c = super::issue478::nextjs_config(root.path());
    std::fs::write(
        root.path().join("src/app/page.tsx"),
        fixture("fixtures/normal.txt"),
    )
    .unwrap();
    std::fs::write(root.path().join("README.md"), "D4 marker fixture").unwrap();
    let guard = authority::begin_run(&c);
    let path = generated_contract(&c);
    std::fs::write(&path, fixture("contract.json")).unwrap();
    (root, c, guard, path)
}

fn proposal_plan(name: &str) -> StepPlan {
    let mut check = step("check-markers", "verify", &[]);
    check.instruction = "Check the saved source markers.".into();
    check.verify = vec![command(name)];
    StepPlan {
        goal: "Verify saved source markers".into(),
        steps: vec![check],
    }
}

fn acceptance(
    c: &Config,
    commands: &[String],
) -> crate::minimal_loop::evidence::RuntimeAcceptanceReport {
    let contract: CompletionContract = serde_json::from_str(&fixture("contract.json")).unwrap();
    verify_runtime_acceptance_with_hints(
        &c.workspace_root,
        &contract.required_paths,
        commands,
        &contract.required_capabilities,
        &contract.required_evidence,
        &contract.required_obligations,
        &[],
        &contract.evidence_hint_tokens,
    )
}

#[test]
fn issue488_real_formation_registration_and_final_boundary() {
    for name in ["O", "V1", "V2", "SWALLOW"] {
        let (_root, c, _guard, path) = setup();
        let original = proposal_plan(name);
        let mut admission = admission::Admission::default();
        let decision = admission
            .check(&c, None, &original, &mut original.clone(), 1)
            .unwrap();
        assert_eq!(
            matches!(decision, admission::Decision::Retry(_)),
            name != "V2"
        );
        if name != "V2" {
            let fixed = proposal_plan("V2");
            assert!(matches!(
                admission
                    .check(&c, None, &fixed, &mut fixed.clone(), 2)
                    .unwrap(),
                admission::Decision::Ready(false)
            ));
        }
        let registration = scope::register(&c, &original);
        assert_eq!(registration.is_ok(), name == "V2");
        let contract: CompletionContract =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(
            contract.verify_commands.contains(&command(name)),
            name == "V2"
        );
        let report = acceptance(&c, &[command(name)]);
        assert_eq!(report.passed, name == "V2", "{name}: {report:?}");
        let diagnosis = command_diagnosis::collect(&c.workspace_root, &[command(name)]);
        eprintln!(
            "current {name}: formation_retry={} registration_ok={registration:?} final_pass={} kind={}",
            name != "V2",
            report.passed,
            diagnosis[0].kind
        );
    }
}

#[path = "issue488_runtime_tests.rs"]
mod runtime;

#[path = "issue488_planner_tests.rs"]
mod planner;
#[path = "issue488_registration_tests.rs"]
mod registration;
