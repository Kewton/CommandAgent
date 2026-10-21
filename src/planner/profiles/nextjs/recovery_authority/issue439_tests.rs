use super::*;
use crate::planner::verify::normalize_verify_command;

#[test]
fn issue439_generated_hook_identity_is_exact() {
    for path in [
        "src/app/page.tsx",
        "app/page.tsx",
        "src/components/Form.tsx",
    ] {
        for pattern in [
            "data-anvil-state",
            "data-anvil-action=\"primary\"",
            "data-anvil-action=\"input\"",
            "data-anvil-action=\"restart\"",
        ] {
            let command = normalize_verify_command(&format!("grep -q '{pattern}' {path}")).unwrap();
            assert!(is_generated_hook_check(command.as_str()), "{command}");
            assert!(!is_generated_hook_check(&format!("{command} || true")));
            assert!(!is_generated_hook_check(
                &command
                    .as_str()
                    .replace("process.exit(1)", "process.exit(0)")
            ));
        }
    }
    for command in [
        "node -p 'console.log(\"data-anvil-state\")'",
        "node -p 'require(\"fs\").readFileSync(\"src/app/page.tsx\"); process.exit(0)'",
        "grep -q 'use client' src/app/page.tsx",
    ] {
        assert!(!is_generated_hook_check(command), "{command}");
    }
}

#[test]
fn issue439_equivalence_does_not_drop_path_or_conditional_requirements() {
    let goal = "Create an interactive Next.js app on port 60302";
    assert!(final_verifier_covers_command(
        goal,
        &super::super::package_build_script_verify_command()
    ));
    for command in [
        "npm test",
        "node --test",
        "node smoke-check.js",
        "grep -q 'use client' src/app/page.tsx",
        "grep -q 'use client' src/components/Form.tsx",
        "grep -q 'data-anvil-action' src/app/page.tsx",
    ] {
        assert!(!final_verifier_covers_command(goal, command), "{command}");
    }
    for check in super::super::package_script_port_verify_commands(60303) {
        assert!(!final_verifier_covers_command(goal, &check), "{check}");
    }
}

#[test]
fn issue439_client_directive_coverage_is_conditional() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("src/app")).unwrap();
    let page = root.path().join("src/app/page.tsx");
    std::fs::write(
        &page,
        "export default function Page() { return <main>Server page</main>; }",
    )
    .unwrap();
    assert!(super::super::client_component_contract_failure(root.path()).is_none());
    std::fs::write(&page, "export default function Page() { const [n, setN] = useState(0); return <button onClick={() => setN(n + 1)}>Add</button>; }").unwrap();
    assert!(super::super::client_component_contract_failure(root.path()).is_some());
    assert!(!final_verifier_covers_command(
        "Create an app",
        "grep -q 'use client' src/app/page.tsx"
    ));
}
