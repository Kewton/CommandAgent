#[cfg(test)]
mod cases {
    use super::super::*;
    use crate::minimal_loop::completion::CompletionContract;
    use crate::planner::recovery_observation_policy::RecoveryObservationPolicy;
    use crate::planner::recovery_snapshot::current_preflight_source_sha256;

    const STORAGE: &str = include_str!(
        "../../../../tests/corpus/apps/issue429-lazy-json-preflight/fixtures/storage-shape.ts"
    );

    fn workspace(source: &str) -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("src/app/api/staff")).unwrap();
        std::fs::create_dir_all(root.path().join("src/lib")).unwrap();
        std::fs::write(root.path().join("package.json"), "{}").unwrap();
        std::fs::write(root.path().join("src/app/api/staff/route.ts"),
            "import { save } from '../../../lib/storage'; export async function GET() { return save(); }").unwrap();
        std::fs::write(root.path().join("src/lib/storage.ts"), source).unwrap();
        root
    }

    fn policy(root: &Path, protected: &[&str], required: &[&str]) -> Vec<String> {
        let contract: CompletionContract = serde_json::from_value(serde_json::json!({
            "profile": crate::planner::profiles::nextjs::PROFILE_ID,
            "protected_paths": protected, "required_paths": required,
        }))
        .unwrap();
        RecoveryObservationPolicy::for_contract_at_workspace(&contract, root)
            .allowed_generated_paths
    }

    #[test]
    fn lexical_writers_register_missing_and_existing_outputs_without_siblings() {
        let root = workspace(STORAGE);
        assert!(!root.path().join("data").exists());
        assert_eq!(
            policy(root.path(), &[], &[]),
            ["data/shifts.json", "data/staff.json"]
        );
        std::fs::create_dir(root.path().join("data")).unwrap();
        for file in ["staff.json", "shifts.json", "unregistered.json"] {
            std::fs::write(root.path().join("data").join(file), "[]").unwrap();
        }
        std::fs::create_dir(root.path().join("sibling")).unwrap();
        std::fs::write(root.path().join("sibling/staff.json"), "[]").unwrap();
        assert_eq!(
            policy(root.path(), &[], &[]),
            ["data/shifts.json", "data/staff.json"]
        );
        assert_eq!(
            policy(root.path(), &["data/staff.json"], &[]),
            ["data/shifts.json"]
        );
        assert!(policy(root.path(), &["data"], &[]).is_empty());
        assert!(policy(root.path(), &[], &["data"]).is_empty());
        assert_eq!(
            policy(root.path(), &["data/staff"], &[]),
            ["data/shifts.json", "data/staff.json"]
        );
    }

    #[test]
    fn nested_projects_and_cwd_overrides_never_grant_workspace_root_outputs() {
        let root = workspace(STORAGE);
        let nested = root.path().join("nested");
        std::fs::create_dir(&nested).unwrap();
        std::fs::rename(root.path().join("src"), nested.join("src")).unwrap();
        std::fs::rename(
            root.path().join("package.json"),
            nested.join("package.json"),
        )
        .unwrap();
        assert!(policy(root.path(), &[], &[]).is_empty());
        let before = current_preflight_source_sha256(root.path(), &[]).unwrap();
        std::fs::create_dir(root.path().join("data")).unwrap();
        std::fs::write(root.path().join("data/staff.json"), "[]").unwrap();
        assert_ne!(
            current_preflight_source_sha256(root.path(), &[]).unwrap(),
            before
        );

        let root = workspace(STORAGE);
        for command in [
            "cd nested && npm run build",
            "npm --prefix nested run build",
            "npm --prefix=nested run build",
            "pnpm -C nested run build",
            "yarn --cwd nested build",
            "bash -c 'cd nested && npm run build'",
            "node -e \"process.chdir('nested')\"",
        ] {
            let contract: CompletionContract = serde_json::from_value(serde_json::json!({
                "profile": crate::planner::profiles::nextjs::PROFILE_ID, "verify_commands": [command],
            })).unwrap();
            let allowed =
                RecoveryObservationPolicy::for_contract_at_workspace(&contract, root.path())
                    .allowed_generated_paths;
            assert!(allowed.is_empty(), "ambiguous execution cwd: {command}");
        }
        // A writer helper's subdirectory is never taken as its execution cwd.
        assert_eq!(
            policy(root.path(), &[], &[]),
            ["data/shifts.json", "data/staff.json"]
        );
        std::fs::write(root.path().join("src/app/api/staff/route.ts"),
            "import { ensureSeeded } from '../../../lib/storage'; process.chdir('nested'); export async function GET() { await ensureSeeded(); }").unwrap();
        assert!(policy(root.path(), &[], &[]).is_empty());
    }

    #[test]
    fn package_script_cwd_changes_disable_root_output_registration() {
        let root = workspace(STORAGE);
        let contract: CompletionContract = serde_json::from_value(serde_json::json!({
            "profile": crate::planner::profiles::nextjs::PROFILE_ID,
            "verify_commands": ["npm run build"],
        }))
        .unwrap();
        for scripts in [
            serde_json::json!({"build": "cd nested && next build"}),
            serde_json::json!({"dev": "npm --prefix nested run dev"}),
            serde_json::json!({"start": "node -e \"process.chdir('nested')\""}),
            serde_json::json!({"build": "npm run nested-build", "nested-build": "cd nested && next build"}),
        ] {
            std::fs::write(
                root.path().join("package.json"),
                serde_json::to_vec(&serde_json::json!({"scripts": scripts})).unwrap(),
            )
            .unwrap();
            assert!(
                RecoveryObservationPolicy::for_contract_at_workspace(&contract, root.path())
                    .allowed_generated_paths
                    .is_empty()
            );
        }
        std::fs::write(
            root.path().join("package.json"),
            r#"{"scripts":{"build":"next build","dev":"next dev","start":"next start"}}"#,
        )
        .unwrap();
        assert_eq!(
            RecoveryObservationPolicy::for_contract_at_workspace(&contract, root.path())
                .allowed_generated_paths,
            ["data/shifts.json", "data/staff.json"]
        );
    }

    #[test]
    fn lexical_scope_does_not_leak_between_writers_or_through_shadowing() {
        let root = workspace(
            r#"
            import { writeFile as writer } from 'node:fs/promises';
            const filePath = 'data/outer.json';
            function first() { const filePath = 'data/first.json'; writer(filePath, '[]'); }
            function second() { const filePath = unknown(); writer(filePath, '[]'); }
            function third() { writer(filePath, '[]'); const filePath = 'data/tdz.json'; }
            function fourth() { writer(filePath, '[]'); }
            function fifth() { { const filePath = 'data/inner.json'; } writer(filePath, '[]'); }
        "#,
        );
        assert_eq!(
            policy(root.path(), &[], &[]),
            ["data/first.json", "data/outer.json"]
        );
    }

    #[test]
    fn literal_and_comment_lookalikes_and_unbound_imports_grant_no_authority() {
        let root = workspace(
            r#"
            import { writeFile } from 'fs/promises';
            // writeFile('data/comment.json', '[]');
            /* writeFile('data/block.json', '[]'); */
            const quote = "writeFile('data/quoted.json', '[]')";
            const quote2 = 'writeFile("data/double.json", "[]")';
            // import { decoy } from './decoy';
            // import { fakePage } from './app/page';
            const fakeImport = "import { decoy } from './decoy'";
            writeFile('data/real.json', '[]');
        "#,
        );
        std::fs::write(
            root.path().join("src/lib/decoy.ts"),
            "import { writeFile } from 'fs/promises'; writeFile('data/decoy.json', '[]');",
        )
        .unwrap();
        std::fs::create_dir_all(root.path().join("src/lib/app")).unwrap();
        std::fs::write(
            root.path().join("src/lib/app/page.ts"),
            "import { writeFile } from 'fs/promises'; writeFile('data/fake-page.json', '[]');",
        )
        .unwrap();
        assert_eq!(policy(root.path(), &[], &[]), ["data/real.json"]);
    }

    #[test]
    fn unsupported_syntax_and_false_bindings_grant_no_outputs() {
        for source in [
            r#"import { writeFile } from 'fs/promises'; const r = /writeFile('data/regex.json', '[]')/;"#,
            r#"import { writeFile } from 'fs/promises'; const t = `writeFile('data/template.json', '[]')`;"#,
            r#"import { writeFile } from 'fs/promises'; const t = `${writeFile('data/template.json', '[]')}`;"#,
            r#"const fs = fake(); fs.writeFile('data/fake.json', '[]');"#,
            r#"import fs from 'fs'; function save(fs) { fs.writeFile('data/fake.json', '[]'); }"#,
            r#"import fs from 'fs'; fs = fake(); fs.writeFile('data/fake.json', '[]');"#,
            r#"import fs from 'fs'; fs.writeFile = fake; fs.writeFile('data/fake.json', '[]');"#,
            r#"import fs from 'fs'; fs['writeFile'] = fake; fs.writeFile('data/fake.json', '[]');"#,
            r#"import fs from 'fs'; { const fs = fake(); fs.writeFile('data/fake.json', '[]'); }"#,
            r#"import fs from 'fs'; import path from 'path'; function save(path) { fs.writeFile(path.join('data', 'fake.json'), '[]'); }"#,
            r#"import fs from 'fs'; import path from 'path'; path.join = fake; fs.writeFile(path.join('data', 'fake.json'), '[]');"#,
            r#"import fs from 'fs'; import path from 'path'; { const path = fake(); fs.writeFile(path.join('data', 'fake.json'), '[]'); }"#,
            r#"import fs from 'fs'; import path from 'path'; function save(process) { fs.writeFile(path.join(process.cwd(), 'fake.json'), '[]'); }"#,
            r#"import fs from 'fs'; import path from 'path'; process.cwd = fake; fs.writeFile(path.join(process.cwd(), 'fake.json'), '[]');"#,
            r#"import fs from 'fs'; import path from 'path'; const process = fake(); fs.writeFile(path.join(process.cwd(), 'fake.json'), '[]');"#,
            r#"import { writeFile as writer } from 'fs/promises'; function save(writer) { writer('data/fake.json', '[]'); }"#,
            r#"import { writeFile as writer } from 'fs/promises'; writer = fake; writer('data/fake.json', '[]');"#,
            r#"import { writeFile as writer } from 'fs/promises'; { const writer = fake; writer('data/fake.json', '[]'); }"#,
            r#"import { writeFile as writer } from 'fs/promises'; const save = writer => writer('data/fake.json', '[]');"#,
            r#"import { writeFile } from 'fs/promises'; writeFile('data/prefix.json' + suffix, '[]');"#,
            r#"import { writeFile } from 'fs/promises'; const p = 'data/prefix.json' + suffix; writeFile(p, '[]');"#,
            r#"import { writeFile } from 'fs/promises'; import path from 'path'; writeFile(path.join('data', 'prefix.json') + suffix, '[]');"#,
            r#"import { writeFile } from 'fs/promises'; let p = 'data/mutable.json'; writeFile(p, '[]');"#,
            r#"import { writeFile } from 'fs/promises'; writeFile('data/escape\u002ejson', '[]');"#,
        ] {
            let root = workspace(source);
            assert!(
                policy(root.path(), &[], &[]).is_empty(),
                "false authority: {source}"
            );
        }
    }

    #[test]
    fn builtin_aliases_and_normalized_literal_paths_are_supported() {
        let root = workspace(
            r#"
            import * as fs from 'node:fs';
            import { join as joinPath } from 'node:path';
            const file = joinPath(process.cwd(), './data//', 'staff.json');
            fs.promises.writeFile(file, '[]');
            fs.writeFileSync('./data/shifts.json', '[]');
        "#,
        );
        assert_eq!(
            policy(root.path(), &[], &[]),
            ["data/shifts.json", "data/staff.json"]
        );
    }

    #[test]
    fn source_config_runtime_and_traversal_paths_remain_protected() {
        for path in [
            "src/state.json",
            "app/state.json",
            "config/state.json",
            "package.json",
            "data/package-lock.json",
            "tsconfig.json",
            "tsconfig.app.json",
            "tsconfig.build.json",
            "jsconfig.base.json",
            "vercel.json",
            "data/app.config.json",
            ".anvil/state.json",
            ".commandagent/state.json",
            "node_modules/state.json",
            "../escape.json",
            "data/../../escape.json",
            "/tmp/escape.json",
            "C:/escape.json",
        ] {
            let root = workspace(&format!(
                "import {{ writeFile }} from 'fs/promises'; writeFile('{path}', '[]');"
            ));
            assert!(
                policy(root.path(), &[], &[]).is_empty(),
                "unprotected: {path}"
            );
        }
        let root = workspace(
            "import fs from 'fs'; import path from 'path'; fs.writeFile(path.join(process.cwd(), 'data', '..', 'escape.json'), '[]');",
        );
        assert!(policy(root.path(), &[], &[]).is_empty());
    }

    #[test]
    fn imported_and_transitively_referenced_custom_json_remain_hashed() {
        let root = workspace(
            r#"
            import fs from 'fs';
            import settings from '../../data/imported.json' with { type: 'json' };
            fs.writeFile('data/imported.json', '[]');
            fs.writeFile('data/custom.json', '[]');
            fs.writeFile('data/base.json', '[]');
            fs.writeFile('data/runtime.json', '[]');
        "#,
        );
        std::fs::create_dir(root.path().join("data")).unwrap();
        std::fs::write(
            root.path().join("tsconfig.build.json"),
            r#"{"extends":"./data/custom.json"}"#,
        )
        .unwrap();
        std::fs::write(
            root.path().join("data/custom.json"),
            r#"{"extends":"./base"}"#,
        )
        .unwrap();
        for name in ["base", "imported", "runtime"] {
            std::fs::write(root.path().join(format!("data/{name}.json")), "{}").unwrap();
        }
        let allowed = policy(root.path(), &[], &[]);
        assert_eq!(allowed, ["data/runtime.json"]);
        for file in [
            "data/imported.json",
            "data/custom.json",
            "data/base.json",
            "tsconfig.build.json",
        ] {
            let before = current_preflight_source_sha256(root.path(), &allowed).unwrap();
            std::fs::write(root.path().join(file), "changed").unwrap();
            assert_ne!(
                current_preflight_source_sha256(root.path(), &allowed).unwrap(),
                before,
                "{file}"
            );
        }
    }

    #[test]
    fn jsonc_and_non_route_config_sources_exclude_custom_json_outputs() {
        for (config_path, config_source) in [
            (
                "tsconfig.json",
                "{ // JSONC comment\n \"extends\": \"./data/custom.json\",\n}",
            ),
            (
                "tsconfig.app.json",
                "{ /* JSONC */ \"extends\": \"./data/custom\", }",
            ),
            (
                "next.config.js",
                "const settings = require('./data/custom.json'); module.exports = settings;",
            ),
            (
                "next.config.ts",
                "import settings from './data/custom.json'; export default settings;",
            ),
        ] {
            let root = workspace(
                "import fs from 'fs'; fs.writeFile('data/custom.json', '[]'); fs.writeFile('data/runtime.json', '[]');",
            );
            std::fs::write(root.path().join(config_path), config_source).unwrap();
            assert_eq!(
                policy(root.path(), &[], &[]),
                ["data/runtime.json"],
                "missing: {config_path}"
            );
            std::fs::create_dir(root.path().join("data")).unwrap();
            std::fs::write(root.path().join("data/custom.json"), "{}").unwrap();
            let allowed = policy(root.path(), &[], &[]);
            assert_eq!(allowed, ["data/runtime.json"], "existing: {config_path}");
            let before = current_preflight_source_sha256(root.path(), &allowed).unwrap();
            std::fs::write(root.path().join("data/custom.json"), "changed").unwrap();
            assert_ne!(
                current_preflight_source_sha256(root.path(), &allowed).unwrap(),
                before
            );
        }
    }

    #[test]
    fn snapshot_excludes_exact_files_and_rejects_directory_masquerading() {
        let root = workspace(STORAGE);
        let allowed = policy(root.path(), &[], &[]);
        let before = current_preflight_source_sha256(root.path(), &allowed).unwrap();
        std::fs::create_dir(root.path().join("data")).unwrap();
        std::fs::write(root.path().join("data/staff.json"), "[]").unwrap();
        assert_eq!(
            current_preflight_source_sha256(root.path(), &allowed).unwrap(),
            before
        );
        std::fs::write(root.path().join("data/staff.json.backup"), "[]").unwrap();
        assert_ne!(
            current_preflight_source_sha256(root.path(), &allowed).unwrap(),
            before
        );
        let root = workspace(STORAGE);
        std::fs::create_dir_all(root.path().join("data/staff.json")).unwrap();
        assert_eq!(policy(root.path(), &[], &[]), ["data/shifts.json"]);
        assert!(current_preflight_source_sha256(root.path(), &allowed).is_err());
        std::fs::write(root.path().join("data/staff.json/nested.json"), "[]").unwrap();
        assert!(current_preflight_source_sha256(root.path(), &allowed).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn symlink_ancestors_leaves_and_new_links_are_rejected_without_touching_targets() {
        use std::os::unix::fs::symlink;
        let outside = tempfile::tempdir().unwrap();
        let sentinel = outside.path().join("sentinel.json");
        std::fs::write(&sentinel, "original").unwrap();
        for leaf in [false, true] {
            let root = workspace(STORAGE);
            let allowed = policy(root.path(), &[], &[]);
            if leaf {
                std::fs::create_dir(root.path().join("data")).unwrap();
                symlink(&sentinel, root.path().join("data/staff.json")).unwrap();
            } else {
                symlink(outside.path(), root.path().join("data")).unwrap();
            }
            assert!(!policy(root.path(), &[], &[]).contains(&"data/staff.json".to_string()));
            assert!(current_preflight_source_sha256(root.path(), &allowed).is_err());
            assert!(
                crate::planner::recovery_snapshot::capture_for_transaction(root.path(), 0).is_err()
            );
        }
        let root = workspace(STORAGE);
        let allowed = policy(root.path(), &[], &[]);
        symlink(outside.path().join("absent"), root.path().join("data")).unwrap();
        assert!(policy(root.path(), &[], &[]).is_empty());
        assert!(current_preflight_source_sha256(root.path(), &allowed).is_err());
        assert_eq!(std::fs::read_to_string(sentinel).unwrap(), "original");
        assert!(!outside.path().join("staff.json").exists());
        let root = workspace(STORAGE);
        let before = current_preflight_source_sha256(root.path(), &[]).unwrap();
        symlink(outside.path(), root.path().join(".goal-verify-tools")).unwrap();
        assert_eq!(
            current_preflight_source_sha256(root.path(), &[]).unwrap(),
            before
        );
    }
}
