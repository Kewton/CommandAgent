use super::super::*;
use crate::planner::runner::final_acceptance::*;
use clap::Parser;
use serde_json::Value;

fn commands(formed: bool) -> Vec<String> {
    serde_json::from_str(if formed {
        include_str!("../../../tests/corpus/apps/issue479-verifier-formation/formed-commands.json")
    } else {
        include_str!(
            "../../../tests/corpus/apps/issue479-verifier-formation/original-commands.json"
        )
    })
    .unwrap()
}

fn write(root: &Path, path: &str, text: &str) {
    let path = root.join(path);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn setup(root: &Path, formed: bool) -> (Config, UltraPlan) {
    for path in ["package.json", "src/lib/types.ts", "src/lib/store.ts"] {
        let source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/corpus/apps/issue479-verifier-formation")
            .join(path);
        write(root, path, &std::fs::read_to_string(source).unwrap());
    }
    std::fs::create_dir_all(root.join("node_modules")).unwrap();
    write(
        root,
        "src/app/page.tsx",
        "'use client'; export default function Page(){ return <main data-anvil-action='primary' data-anvil-state='{}'/>; }",
    );
    for path in [
        "projects",
        "projects/[id]",
        "tasks",
        "tasks/[id]",
        "members",
    ] {
        write(
            root,
            &format!("src/app/api/{path}/route.ts"),
            "export function GET(){return Response.json([]);}",
        );
    }
    // These structural controls execute the four source checks. The build and
    // full Next.js Profile remain separate required plan/parent execution gates.
    write(
        root,
        "contract.json",
        &json!({"profile":"generic",
            "required_paths":["src/lib/types.ts","src/lib/store.ts","src/app/page.tsx"],
            "required_evidence":["implementation_artifact"],"verify_commands":commands(formed)[1..]
        })
        .to_string(),
    );
    let mut config =
        Config::from_cli(crate::cli::Cli::parse_from(["commandagent", "--ux-demo"])).unwrap();
    config.workspace_root = root.into();
    config.profile = "generic".into();
    config.profile_explicit = true;
    config.completion_contract_path = Some(root.join("contract.json"));
    config.eval_events_path = Some(root.join("events.jsonl"));
    let plan = UltraPlan {
        goal: "Check the declared module and source structure".into(),
        profile: "generic".into(),
        intent: "create".into(),
        style: "balanced".into(),
        phases: vec![],
    };
    (config, plan)
}

fn final_event(c: &Config) -> Value {
    std::fs::read_to_string(c.eval_events_path.as_ref().unwrap())
        .unwrap()
        .lines()
        .filter_map(|s| serde_json::from_str::<Value>(s).ok())
        .rfind(|e| e["event"] == "ultra_final_acceptance")
        .unwrap()
}

#[test]
fn issue479_final_acceptance_diagnoses_immutable_imports_without_changing_the_contract() {
    let root = tempfile::tempdir().unwrap();
    let (c, plan) = setup(root.path(), false);
    let before = std::fs::read(root.path().join("contract.json")).unwrap();
    for cycle in 0..2 {
        let failed = ultra_final_acceptance_report_with_cycle(&plan, &c, cycle).unwrap();
        assert!(!failed.is_pass(), "{failed:?}");
        let event = final_event(&c);
        assert_eq!(event["weak_evidence_sources"].as_array().unwrap().len(), 2);
        let diagnoses = event["command_diagnoses"].as_array().unwrap();
        for (d, command) in diagnoses[..2].iter().zip(&commands(false)[1..3]) {
            assert_eq!(d["command"], *command);
            assert_eq!(d["reason"], "node_smoke_without_assertion");
            assert_eq!(d["repairability"], "immutable_inline_evidence");
        }
        write(
            root.path(),
            "src/app/api/projects/route.ts",
            "export function GET(){return Response.json({items:[]});}",
        );
    }
    assert_eq!(
        std::fs::read(root.path().join("contract.json")).unwrap(),
        before
    );
}

#[test]
fn issue479_refined_final_acceptance_executes_target_failures_without_business_credit() {
    let root = tempfile::tempdir().unwrap();
    let (c, plan) = setup(root.path(), true);
    let before = std::fs::read(root.path().join("contract.json")).unwrap();
    let passed = ultra_final_acceptance_report(&plan, &c).unwrap();
    assert!(passed.is_pass(), "{passed:?}");
    assert_eq!(final_event(&c)["weak_evidence"], json!([]));
    assert!(
        final_event(&c)["command_diagnoses"]
            .as_array()
            .unwrap()
            .iter()
            .all(|d| d["kind"] == "static_syntax")
    );
    let target = root.path().join("src/lib/types.ts");
    let source = std::fs::read_to_string(&target).unwrap();
    for changed in [
        format!("{source}\nexport const unexpected=1;"),
        format!("{source}\nthrow new Error('original import failure');"),
    ] {
        std::fs::write(&target, changed).unwrap();
        let failed = ultra_final_acceptance_report_with_cycle(&plan, &c, 1).unwrap();
        assert!(!failed.is_pass());
        let command = &commands(true)[1];
        assert!(
            failed
                .command_failures
                .iter()
                .any(|f| f.command == *command),
            "{failed:?}"
        );
        let event = final_event(&c);
        let diagnosis = event["command_diagnoses"]
            .as_array()
            .unwrap()
            .iter()
            .find(|d| d["command"] == *command)
            .unwrap();
        assert_eq!(diagnosis["repairability"], "runtime_input_dependent");
        assert!(diagnosis["execution_failure"].as_str().is_some());
    }
    std::fs::write(target, source).unwrap();
    assert!(
        ultra_final_acceptance_report_with_cycle(&plan, &c, 2)
            .unwrap()
            .is_pass()
    );
    assert_eq!(
        std::fs::read(root.path().join("contract.json")).unwrap(),
        before
    );
}
