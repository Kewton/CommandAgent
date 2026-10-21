use super::*;
use crate::planner::recovery_observation_policy::RecoveryObservationPolicy;

fn policy_for(source: &str) -> Vec<String> {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("src/app/api/items")).unwrap();
    std::fs::write(root.path().join("package.json"), "{}").unwrap();
    std::fs::write(root.path().join("src/app/api/items/route.ts"), source).unwrap();
    let contract = serde_json::from_value(
        serde_json::json!({"profile": crate::planner::profiles::nextjs::PROFILE_ID}),
    )
    .unwrap();
    RecoveryObservationPolicy::for_contract_at_workspace(&contract, root.path())
        .allowed_generated_paths
}

#[test]
fn issue429_real_campaign_sources_register_exact_outputs_before_and_after_creation() {
    let fixture = Path::new("tests/corpus/apps/issue429-lazy-json-preflight/fixtures/campaign");
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(fixture.join("source-sha256.json")).unwrap())
            .unwrap();
    for (run, expected, writers) in [
        (
            "S1",
            ["data/shifts.json", "data/staff.json"],
            vec!["src/app/api/staff/route.ts", "src/app/api/shifts/route.ts"],
        ),
        (
            "S3",
            ["data/shifts.json", "data/staff.json"],
            vec!["src/lib/storage.ts"],
        ),
        (
            "E3",
            ["data/departments.json", "data/expenses.json"],
            vec!["src/lib/db.ts"],
        ),
    ] {
        use sha2::{Digest, Sha256};
        let root = tempfile::tempdir().unwrap();
        for (path, hash) in manifest[run].as_object().unwrap() {
            let bytes = std::fs::read(fixture.join(run).join(path)).unwrap();
            assert_eq!(
                format!("{:x}", Sha256::digest(&bytes)),
                hash.as_str().unwrap(),
                "{run}/{path}"
            );
            let destination = root.path().join(path);
            std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
            std::fs::write(destination, bytes).unwrap();
        }
        for writer in writers {
            let text = std::fs::read_to_string(root.path().join(writer)).unwrap();
            assert!(Source::parse(&text).is_some(), "{run}/{writer}");
        }
        std::fs::write(root.path().join("package.json"), "{}").unwrap();
        let mut contract: CompletionContract = serde_json::from_value(
            serde_json::json!({"profile": crate::planner::profiles::nextjs::PROFILE_ID}),
        )
        .unwrap();
        for existing in [false, true] {
            if existing {
                std::fs::create_dir(root.path().join("data")).unwrap();
                for path in expected {
                    std::fs::write(root.path().join(path), "[]").unwrap();
                }
                std::fs::write(root.path().join("data/unregistered.json"), "[]").unwrap();
            }
            let actual =
                RecoveryObservationPolicy::for_contract_at_workspace(&contract, root.path());
            assert_eq!(
                actual.allowed_generated_paths, expected,
                "{run}, existing={existing}"
            );
        }
        contract.protected_paths = vec![expected[0].to_string()];
        contract.required_paths = vec![expected[1].to_string()];
        assert!(
            RecoveryObservationPolicy::for_contract_at_workspace(&contract, root.path())
                .allowed_generated_paths
                .is_empty()
        );
    }
}

#[test]
fn issue429_harmless_templates_regex_and_division_preserve_independent_writers() {
    for expression in [
        r#"`message ${name} ${`nested ${({a: "}", b: /[}]/}).a}`} escaped \` end`"#,
        r#"/^[a-z\/{}]+$/gi"#,
        "12 / 3 / 2",
        "(12 + 6) / 3",
        "values[0] / 2",
        r#"`${(() => { /* } */ return /[}]/.test("}") ? `yes ${name}` : 'no'; })()}`"#,
    ] {
        let source = format!(
            "import fs from 'fs'; const unrelated = {expression}; fs.writeFileSync('data/real.json', '[]');"
        );
        assert!(Source::parse(&source).is_some(), "{expression}");
        assert_eq!(policy_for(&source), ["data/real.json"], "{expression}");
    }
    assert_eq!(
        policy_for(
            "import fs from 'fs'; if (ok) /[{}]/.test(text); fs.writeFileSync('data/real.json', '[]');"
        ),
        ["data/real.json"]
    );
}

#[test]
fn issue429_hidden_executable_mutations_and_opaque_lookalikes_grant_no_authority() {
    for body in [
        r#"const t = `${fs.writeFile = replacement}`; fs.writeFile('data/fake.json', '[]');"#,
        r#"const t = `${`nested ${fs['writeFileSync'] = replacement}`}`; fs.writeFileSync('data/fake.json', '[]');"#,
        r#"const t = `${(() => { const fs = fake; return fs; })()}`; fs.writeFile('data/fake.json', '[]');"#,
        r#"const t = `${path.join = fake}`; fs.writeFile(path.join('data', 'fake.json'), '[]');"#,
        r#"const t = `${process.cwd = fake}`; fs.writeFile(path.join(process.cwd(), 'data', 'fake.json'), '[]');"#,
        r#"const t = `${writer = fake}`; writer('data/fake.json', '[]');"#,
        r#"const t = `writeFile('data/fake.json', '[]') ${fs.writeFile('data/inner.json', '[]')}`;"#,
        r#"const r = /fs.writeFile('data/fake.json', '[]')/;"#,
        r#"const x = {} / (fs.writeFile = fake) / 2; fs.writeFile('data/fake.json', '[]');"#,
        r#"const x = (12) / (fs.writeFile = fake) / 2; fs.writeFile('data/fake.json', '[]');"#,
        r#"const x = obj.return / (fs.writeFile = fake) / 2; fs.writeFile('data/fake.json', '[]');"#,
        r#"const x = counter++ / (fs.writeFile = fake) / 2; fs.writeFile('data/fake.json', '[]');"#,
        r#"const x = obj.if(1) / (fs.writeFile = fake) / 2; fs.writeFile('data/fake.json', '[]');"#,
        r#"const x = yield / (f\u0073.writeFile = fake) / 2; fs.writeFile('data/fake.json', '[]');"#,
        r#"const x = 1 + + /fs.writeFile('data/fake.json', '[]')/;"#,
        r#"const t = `${[...fs] = replacement}`; fs.writeFile('data/fake.json', '[]');"#,
        r#"const t = `${f\u0073.writeFile = replacement}`; fs.writeFile('data/fake.json', '[]');"#,
        r#"const x = value! / (f\u0073.writeFile = fake) / 2; fs.writeFile('data/fake.json', '[]');"#,
        r#"const x = fn<T> / (f\u0073.writeFile = fake) / 2; fs.writeFile('data/fake.json', '[]');"#,
        r#"const page = <div>fs.writeFile('data/fake.json', '[]')<br/></div>;"#,
        r#"if (ok) run(); else /fs.writeFile('fake.json', '[]')/.test(text);"#,
        r#"do /fs.writeFile('fake.json', '[]')/.test(text); while (ok);"#,
        r#"for await (const x of values) /fs.writeFile('fake.json', '[]')/.test(text);"#,
        "while (ok) { break\n /fs.writeFile('fake.json', '[]')/.test(text); }",
        "label: while (ok) { break label\n /fs.writeFile('fake.json', '[]')/.test(text); }",
        "import fs from 'fs'\n /fs.writeFile('fake.json', '[]')/.test(text);",
        "let x\n /fs.writeFile('fake.json', '[]')/.test(text);",
        r#"const x = /unterminated; fs.writeFile('data/fake.json', '[]');"#,
        r#"const x = `unterminated ${name}; fs.writeFile('data/fake.json', '[]');"#,
    ] {
        let source = format!(
            "import fs from 'fs'; import path from 'path'; import {{ writeFile as writer }} from 'fs/promises'; {body}"
        );
        assert!(policy_for(&source).is_empty(), "{body}");
    }
}

#[test]
fn issue429_local_forwarding_is_one_level_and_requires_unambiguous_immutable_paths() {
    let prefix = "import fs from 'fs'; const FILE = './data//real.json';";
    assert_eq!(
        policy_for(&format!(
            "{prefix} async function save<T>(file: string, data: T[]): Promise<void> {{ await fs.writeFile(file, data); }} save(FILE, []);"
        )),
        ["data/real.json"]
    );
    for body in [
        "function save(file) { fs.writeFile(file, '[]'); } save(dynamic, []);",
        "function save(file) { fs.writeFile(file, '[]'); } save(FILE + suffix, []);",
        "function save(file) { fs.writeFile(file, '[]'); } save('data/literal.json', []);",
        "function save(file) { fs.writeFile(file, '[]'); } function outer(file) { save(file); } outer(FILE);",
        "function save(file) { file = 'data/other.json'; fs.writeFile(file, '[]'); } save(FILE);",
        "function save(file) { const file = 'data/other.json'; fs.writeFile(file, '[]'); } save(FILE);",
        "function save(file) { fs.writeFile(file, '[]'); } save = fake; save(FILE);",
        "function save(file) { fs.writeFile(file, '[]'); } function other(save) { save(FILE); }",
        "function save(file) { fs.writeFile(file, '[]'); } const save = fake; save(FILE);",
        "function save(file) { fs.writeFile(file, '[]'); } const alias = save; alias(FILE);",
        "function save(file) { fs.writeFile(file, '[]'); } function save(file) {} save(FILE);",
        "function save(file) { fs.writeFile(file, '[]'); } function other() { const FILE = dynamic; save(FILE); }",
        "function save(file) { fs.writeFile(file, '[]'); } const t = `${save = fake}`; save(FILE);",
        "function save(file) { const t = `${file = other}`; fs.writeFile(file, '[]'); } save(FILE);",
    ] {
        assert!(policy_for(&format!("{prefix} {body}")).is_empty(), "{body}");
    }
    for path in [
        "../escape.json",
        "src/state.json",
        ".anvil/state.json",
        "data/package.json",
    ] {
        assert!(policy_for(&format!("import fs from 'fs'; const FILE = '{path}'; function save(file) {{ fs.writeFile(file, '[]'); }} save(FILE);")).is_empty(), "{path}");
    }
}

#[test]
fn issue429_dynamic_evaluation_fixture_cannot_hide_identity_mutation() {
    let cases: Vec<String> = serde_json::from_str(include_str!(
        "../../../../tests/corpus/apps/issue429-lazy-json-preflight/fixtures/hidden-evaluation.json"
    ))
    .unwrap();
    assert_eq!(cases.len(), 6);
    for source in cases {
        assert!(Source::parse(&source).is_none(), "{source}");
        assert!(policy_for(&source).is_empty(), "{source}");
    }
}

#[test]
fn issue429_conditions_are_not_parameters_but_loop_bindings_remain_unknown() {
    assert_eq!(
        policy_for(
            "import fs from 'fs'; const FILE = 'data/real.json'; if (fs.existsSync(FILE)) { fs.writeFileSync(FILE, '[]'); }"
        ),
        ["data/real.json"]
    );
    for body in [
        "for (fs of values) { fs.writeFile('data/fake.json', '[]'); }",
        "for (fs in values) { fs.writeFile('data/fake.json', '[]'); }",
        "try {} catch (fs) { fs.writeFile('data/fake.json', '[]'); }",
        "if (fs = fake) { fs.writeFile('data/fake.json', '[]'); }",
    ] {
        assert!(
            policy_for(&format!("import fs from 'fs'; {body}")).is_empty(),
            "{body}"
        );
    }
}
