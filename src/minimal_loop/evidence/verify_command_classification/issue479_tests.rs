use super::super::{collect_workspace_evidence, verify_command_kind};
use super::*;
use crate::minimal_loop::completion::CompletionContract;
use serde_json::json;
use std::path::Path;

fn commands() -> Vec<String> {
    serde_json::from_str(include_str!(
        "../../../../tests/corpus/apps/issue479-verifier-formation/original-commands.json"
    ))
    .unwrap()
}

fn write(root: &Path, path: &str, text: &str) {
    let path = root.join(path);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn formed() -> Vec<String> {
    serde_json::from_str(include_str!(
        "../../../../tests/corpus/apps/issue479-verifier-formation/formed-commands.json"
    ))
    .unwrap()
}

fn saved_modules(root: &Path) {
    write(
        root,
        "package.json",
        include_str!("../../../../tests/corpus/apps/issue479-verifier-formation/package.json"),
    );
    std::fs::create_dir_all(root.join("node_modules")).unwrap();
    write(
        root,
        "src/lib/types.ts",
        include_str!("../../../../tests/corpus/apps/issue479-verifier-formation/src/lib/types.ts"),
    );
    write(
        root,
        "src/lib/store.ts",
        include_str!("../../../../tests/corpus/apps/issue479-verifier-formation/src/lib/store.ts"),
    );
}

fn execute(root: &Path, command: &str) -> crate::planner::verify::VerificationReport {
    crate::planner::verify::verify_step(
        root,
        &crate::planner::step_plan::PlanStep {
            id: "check".into(),
            kind: "verify".into(),
            expected_result: "pass".into(),
            instruction: "Run the declared target check".into(),
            expected_paths: vec![],
            verify: vec![command.into()],
        },
    )
}

#[test]
fn issue479_inventory_classifies_original_and_host_commands() {
    let root = tempfile::tempdir().unwrap();
    let mut commands = commands();
    let package: Vec<String> = serde_json::from_str(include_str!(
        "../../../../tests/corpus/apps/issue478-model-host-obligations/package-checks.json"
    ))
    .unwrap();
    commands.extend(package);
    let before = commands
        .iter()
        .map(|c| {
            format!(
                "{:?}",
                verify_command_kind(c, &collect_workspace_evidence(root.path()))
            )
        })
        .collect::<Vec<_>>();
    write(root.path(), "src/lib/types.ts", "export const value = 42;");
    write(root.path(), "src/lib/store.ts", "export const value = 42;");
    write(
        root.path(),
        "src/app/api/projects/route.ts",
        "export function GET(){ return Response.json([]); }",
    );
    let after = commands
        .iter()
        .map(|c| {
            format!(
                "{:?}",
                verify_command_kind(c, &collect_workspace_evidence(root.path()))
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(before, after, "inline evidence must remain command-local");
    if let Ok(path) = std::env::var("ISSUE479_INVENTORY_EVIDENCE") {
        std::fs::write(path, serde_json::to_vec_pretty(&json!({
            "source":"actual verify_command_kind before/after app changes",
            "inventory":commands.iter().zip(&before).map(|(command, kind)| json!({"command":command,"kind":kind})).collect::<Vec<_>>(),
            "app_change_independent":before == after
        })).unwrap()).unwrap();
    }
    assert_eq!(before[0], "Build");
    for kind in &before[1..3] {
        assert_eq!(kind, "Weak(\"node_smoke_without_assertion\")");
    }
    for kind in &before[3..] {
        assert_eq!(kind, "StaticSyntax");
    }
}

#[test]
fn issue479_original_import_remains_weak_at_real_final_acceptance() {
    let root = tempfile::tempdir().unwrap();
    for command in &commands()[1..3] {
        write(root.path(), "src/lib/types.ts", "export const value = 42;");
        write(root.path(), "src/lib/store.ts", "export const value = 42;");
        let contract: CompletionContract = serde_json::from_value(json!({
            "required_paths":["src/lib/types.ts","src/lib/store.ts"],
            "verify_commands":[command], "required_evidence":["implementation_artifact"]
        }))
        .unwrap();
        let before = contract.verify_with_goal(root.path(), "Check the shared application modules");
        assert!(!before.is_pass(), "{before:?}");
        assert!(
            before
                .primary_reason()
                .contains("node_smoke_without_assertion")
        );
        write(root.path(), "src/lib/types.ts", "export const value = 43;");
        write(
            root.path(),
            "src/app/api/projects/route.ts",
            "export function GET(){ return Response.json([]); }",
        );
        let after = contract.verify_with_goal(root.path(), "Check the shared application modules");
        assert!(!after.is_pass());
        assert!(
            after
                .primary_reason()
                .contains("node_smoke_without_assertion")
        );
    }
}

#[test]
fn issue479_saved_modules_strengthen_loadability_and_explicit_export_boundary() {
    use sha2::{Digest, Sha256};
    let root = tempfile::tempdir().unwrap();
    saved_modules(root.path());
    assert_eq!(
        format!(
            "{:x}",
            Sha256::digest(std::fs::read(root.path().join("src/lib/types.ts")).unwrap())
        ),
        "cbc67d6d031f3c029e3379e622471e0b1b1f47ac769a561e3bbc287a31530793"
    );
    for (index, target) in [(1, "src/lib/types.ts"), (2, "src/lib/store.ts")] {
        let check = &formed()[index];
        assert_eq!(
            verify_command_kind(check, &collect_workspace_evidence(root.path())),
            VerifyCommandKind::StaticSyntax
        );
        assert!(!super::super::has_bound_verify_command(
            std::slice::from_ref(check),
            &collect_workspace_evidence(root.path())
        ));
        assert!(execute(root.path(), &commands()[index]).is_pass());
        let report = execute(root.path(), check);
        assert!(report.is_pass(), "{report:?}");
        let source = std::fs::read_to_string(root.path().join(target)).unwrap();
        // The original import succeeds with an unexpected export; refinement fails.
        write(
            root.path(),
            target,
            &format!("{source}\nexport const unexpected = 1;\n"),
        );
        assert!(execute(root.path(), &commands()[index]).is_pass());
        let failed = execute(root.path(), check);
        assert!(!failed.is_pass(), "{failed:?}");
        assert!(failed.command_failures.iter().any(|f| f.command == *check));
        // Both commands propagate original module-evaluation failure.
        write(
            root.path(),
            target,
            &format!("{source}\nthrow new Error('original import failure');\n"),
        );
        for command in [&commands()[index], check] {
            let failed = execute(root.path(), command);
            assert!(!failed.is_pass());
            assert!(
                failed
                    .command_failures
                    .iter()
                    .any(|f| f.command == *command),
                "{failed:?}"
            );
            // The bounded product diagnostic may summarize an ESM stack as a
            // compile error. Separately measure raw rejection propagation.
            let raw = crate::bounded_process::run_with_timeout(
                std::process::Command::new("sh")
                    .args(["-c", command])
                    .current_dir(root.path())
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped()),
                std::time::Duration::from_secs(5),
            )
            .unwrap();
            assert!(!raw.success());
            assert!(String::from_utf8_lossy(&raw.stderr).contains("original import failure"));
        }
        std::fs::remove_file(root.path().join(target)).unwrap();
        assert!(!execute(root.path(), check).is_pass());
        write(root.path(), target, &source);
    }
    // Separately changed context: explicit ESM has an empty namespace for the
    // same type-only bytes. This does not alter the saved positive context.
    write(root.path(), "package.json", "{\"type\":\"module\"}");
    let empty = "node -e \"import('./src/lib/types.ts').then(actual=>{require('node:assert/strict').deepStrictEqual(Object.keys(actual).sort(),[])})\"";
    assert!(execute(root.path(), empty).is_pass());
    assert!(!execute(root.path(), &formed()[1]).is_pass());
}

#[test]
fn issue479_real_target_assertion_retains_import_and_additional_failure() {
    let root = tempfile::tempdir().unwrap();
    let check = "node -e \"import('./src/calculate.mjs').then(actual=>{require('node:assert/strict').deepStrictEqual(actual.double(...[21]),42)})\"";
    write(
        root.path(),
        "src/calculate.mjs",
        "export function double(n){return n*2;}",
    );
    assert_eq!(
        verify_command_kind(check, &collect_workspace_evidence(root.path())),
        VerifyCommandKind::Test
    );
    assert!(execute(root.path(), check).is_pass());
    write(
        root.path(),
        "src/calculate.mjs",
        "export function double(n){return n*3;}",
    );
    assert!(!execute(root.path(), check).is_pass());
    write(
        root.path(),
        "src/calculate.mjs",
        "throw new Error('import failed');",
    );
    assert!(!execute(root.path(), check).is_pass());
}

#[test]
fn issue479_structure_predicates_execute_every_original_negative() {
    let root = tempfile::tempdir().unwrap();
    let paths = [
        "src/app/api/projects/route.ts",
        "src/app/api/projects/[id]/route.ts",
        "src/app/api/tasks/route.ts",
        "src/app/api/tasks/[id]/route.ts",
        "src/app/api/members/route.ts",
    ];
    for path in paths {
        write(
            root.path(),
            path,
            "export function GET() { return Response.json([]); }",
        );
    }
    let page = "'use client'; export default function Page(){return <div data-anvil-action='primary' data-anvil-state='{}'/>;}";
    write(root.path(), "src/app/page.tsx", page);
    for command in &commands()[3..] {
        assert_eq!(
            verify_command_kind(command, &collect_workspace_evidence(root.path())),
            VerifyCommandKind::StaticSyntax
        );
        assert!(execute(root.path(), command).is_pass());
    }
    for path in paths {
        std::fs::remove_file(root.path().join(path)).unwrap();
        let failed = execute(root.path(), &commands()[3]);
        assert!(!failed.is_pass());
        assert!(
            failed
                .command_failures
                .iter()
                .any(|f| f.reason.contains(path)),
            "{failed:?}"
        );
        write(
            root.path(),
            path,
            "export function GET() { return Response.json([]); }",
        );
    }
    for predicate in ["use client", "data-anvil-action", "data-anvil-state"] {
        write(
            root.path(),
            "src/app/page.tsx",
            &page.replace(predicate, "removed"),
        );
        assert!(!execute(root.path(), &commands()[4]).is_pass());
    }
}

#[test]
fn issue479_closed_grammars_reject_swallowing_vacuity_and_spoofing() {
    let original = formed()[1].clone();
    let workspace = WorkspaceEvidence::default();
    for command in [
        original.replace(".then(actual", ".catch(()=>{}).then(actual"),
        original.replace("Object.keys(actual).sort()", "[]"),
        original.replace("Object.keys(actual).sort()", "Object.keys({}).sort()"),
        original.replace("deepStrictEqual", "notDeepStrictEqual"),
        original.replace("})\"", "}).catch(()=>{})\""),
        original.replace("})\"", "});process.exit(0)\""),
        format!("{original} || true"),
        commands()[3].replace("throw new Error('missing '+f)", "return"),
        commands()[4].replace("throw new Error('missing anvil')", "console.log('ok')"),
    ] {
        assert!(
            matches!(
                verify_command_kind(&command, &workspace),
                VerifyCommandKind::Weak(_)
            ),
            "{command}"
        );
    }
}

#[test]
fn issue479_missing_file_verifier_is_mutable_evidence_not_immutable_inline() {
    use crate::minimal_loop::evidence::command_diagnosis;
    let root = tempfile::tempdir().unwrap();
    let command = "node checks/target.cjs".to_string();
    let before = command_diagnosis::collect(root.path(), std::slice::from_ref(&command));
    assert_eq!(
        before[0].repairability,
        "file_backed_verifier_requires_owner"
    );
    assert!(command_diagnosis::immutable_failure(&before).is_none());
    write(
        root.path(),
        "checks/target.cjs",
        "const assert=require('node:assert/strict');assert.equal(require('../value.json').value,42);",
    );
    write(root.path(), "value.json", "{\"value\":41}");
    let after = command_diagnosis::collect(root.path(), std::slice::from_ref(&command));
    assert_eq!(after[0].kind, "test");
    assert_eq!(after[0].repairability, "runtime_input_dependent");
    assert!(!execute(root.path(), &command).is_pass());
    write(root.path(), "value.json", "{\"value\":42}");
    assert!(execute(root.path(), &command).is_pass());
}

#[test]
fn issue479_attached_inline_flags_share_classification_and_repairability() {
    use crate::minimal_loop::evidence::command_diagnosis;
    let root = tempfile::tempdir().unwrap();
    for command in [
        "node --eval=\"console.log(1)\"",
        "node -e\"console.log(1)\"",
        "node --print=\"1\"",
        "node -p\"1\"",
        "node --eval \"console.log(1)\"",
    ] {
        let normalized = crate::planner::verify::normalize_verify_command(command).unwrap();
        assert_eq!(normalized.as_str(), command);
        assert!(matches!(
            verify_command_kind(command, &collect_workspace_evidence(root.path())),
            VerifyCommandKind::Weak(_)
        ));
        let report = execute(root.path(), command);
        let glued_short = command.starts_with("node -e\"") || command.starts_with("node -p\"");
        assert_eq!(report.is_pass(), !glued_short, "{command}: {report:?}");
        let diagnoses = command_diagnosis::collect(root.path(), &[command.into()]);
        assert_eq!(diagnoses[0].repairability, "immutable_inline_evidence");
        write(root.path(), "src/unrelated.js", "export const value=42;");
        assert!(
            command_diagnosis::immutable_failure(&command_diagnosis::collect(
                root.path(),
                &[command.into()]
            ))
            .unwrap()
            .contains(command)
        );
    }
    let attached = formed()[1].replace("node -e ", "node --eval=");
    assert_eq!(
        verify_command_kind(&attached, &collect_workspace_evidence(root.path())),
        VerifyCommandKind::StaticSyntax
    );
    for prefix in ["node -e", "node -p", "node --print="] {
        let invalid =
            format!("{prefix}\"const assert=require('node:assert/strict');assert(false)\"");
        assert!(
            matches!(
                verify_command_kind(&invalid, &collect_workspace_evidence(root.path())),
                VerifyCommandKind::Weak(_)
            ),
            "{invalid}"
        );
    }
    for command in ["node --eval=\"console.log(1)\" | cat", "node -e \"$CODE\""] {
        assert!(matches!(
            verify_command_kind(command, &collect_workspace_evidence(root.path())),
            VerifyCommandKind::Weak(_)
        ));
        assert_eq!(
            command_diagnosis::collect(root.path(), &[command.into()])[0].repairability,
            "unknown"
        );
    }
}
