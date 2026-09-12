#[cfg(test)]
mod tests {
    use super::super::super::RecoveryObservationPolicy;
    use super::super::*;
    use serde_json::{Value as Json, json};
    use sha2::{Digest, Sha256};

    const STORE: &str = include_str!(
        "../../../../tests/corpus/apps/issue475-store-preflight/historical/src/lib/store.ts"
    );
    const FIXTURE: &str = "tests/corpus/apps/issue475-store-preflight/historical";

    fn workspace(writer: &str) -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        for p in [
            "src/lib/types.ts",
            "src/app/api/projects/route.ts",
            "src/app/api/tasks/route.ts",
        ] {
            let dest = root.path().join(p);
            std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
            std::fs::copy(Path::new(FIXTURE).join(p), dest).unwrap();
        }
        std::fs::write(root.path().join("src/lib/store.ts"), writer).unwrap();
        std::fs::write(root.path().join("package.json"), r#"{"type":"module"}"#).unwrap();
        root
    }

    fn policy(root: &Path, additions: Json) -> Vec<String> {
        let mut contract = json!({"profile":"nextjs"});
        contract
            .as_object_mut()
            .unwrap()
            .extend(additions.as_object().unwrap().clone());
        RecoveryObservationPolicy::for_contract_at_workspace(
            &serde_json::from_value(contract).unwrap(),
            root,
        )
        .allowed_generated_paths
    }

    #[test]
    fn issue475_saved_store_and_causal_variants_use_real_entry() {
        let provenance: Json = serde_json::from_str(include_str!(
            "../../../../tests/corpus/apps/issue475-store-preflight/provenance.json"
        ))
        .unwrap();
        assert_eq!(
            format!("{:x}", Sha256::digest(STORE.as_bytes())),
            provenance["sources"]["src/lib/store.ts"]
        );
        assert_eq!(provenance["historical_operation"], "unknown");
        let baseline: Json = serde_json::from_str(include_str!(
            "../../../../tests/corpus/apps/issue475-store-preflight/baseline.json"
        ))
        .unwrap();
        assert_eq!(baseline["results"].as_array().unwrap().len(), 5);
        for text in [
            STORE.to_string(),
            STORE.replace("${path.basename(filePath)}", "file"),
            STORE.replace("${process.pid}", "123"),
        ] {
            let source = Source::parse(&text).unwrap();
            assert!(!source.unsafe_names.contains("path"));
            assert_eq!(
                source.writer_paths(),
                ["data/projects.json", "data/tasks.json"]
            );
            for existing in [false, true] {
                let root = workspace(&text);
                if existing {
                    std::fs::create_dir(root.path().join("data")).unwrap();
                    for p in ["projects", "tasks"] {
                        std::fs::write(root.path().join(format!("data/{p}.json")), "[]").unwrap();
                    }
                }
                assert_eq!(
                    policy(root.path(), json!({})),
                    ["data/projects.json", "data/tasks.json"]
                );
            }
        }
        // Renaming identifiers and concrete output names preserves the same proof.
        let renamed = STORE
            .replace("atomicWrite", "commitFile")
            .replace("loadCollection", "readArray")
            .replace("saveCollection", "persistArray")
            .replace("filePath", "destination")
            .replace("projects.json", "boards.json");
        assert_eq!(
            policy(workspace(&renamed).path(), json!({})),
            ["data/boards.json", "data/tasks.json"]
        );
    }

    #[test]
    fn issue475_unknown_calls_shadowing_mutation_and_ambiguous_sinks_are_denied() {
        let variants = [
            format!("{STORE}\natomicWrite(process.env.OUTPUT, '[]');"),
            format!("{STORE}\nconst alias = atomicWrite;"),
            format!(
                "{STORE}\nfunction unrelated(atomicWrite) {{ atomicWrite(PROJECTS_FILE, '[]'); }}"
            ),
            format!(
                "{STORE}\nfunction unrelated() {{ const PROJECTS_FILE = 'other.json'; saveCollection(PROJECTS_FILE, []); }}"
            ),
            format!("{STORE}\nsaveCollection('other.json', []);"),
            format!("{STORE}\nloadCollection<Project>(unknown);"),
            STORE.replace(
                "fs.rename(tmpPath, filePath)",
                "fs.rename(tmpPath, filePath + suffix)",
            ),
            STORE.replace(
                "fs.rename(tmpPath, filePath)",
                "fs.rename(filePath, tmpPath)",
            ),
            STORE.replace(
                "fs.rename(tmpPath, filePath)",
                "fs.rename(tmpPath, `${process.pid}.json`)",
            ),
            STORE.replace(
                "await ensureDataDir();\n  const dir",
                "filePath = 'other.json';\n  await ensureDataDir();\n  const dir",
            ),
            STORE.replace(
                "await ensureDataDir();\n  const dir",
                "unknown(filePath);\n  await ensureDataDir();\n  const dir",
            ),
            STORE.replace(
                "${path.basename(filePath)}",
                "${path.basename(filePath = 'other.json')}",
            ),
            STORE.replace(
                "${path.basename(filePath)}",
                "${path['basename'](filePath)}",
            ),
            STORE.replace(
                "${path.basename(filePath)}",
                "${path.basename(filePath) || (path.join = other)}",
            ),
            format!("{STORE}\npath.basename = unknown;"),
            format!("{STORE}\nfunction other(path) {{}}"),
            format!("const path = fake;\n{STORE}"),
            STORE.replace(
                "import * as path from 'path'",
                "import * as path from 'untrusted'",
            ),
            format!("{STORE}\nprocess.chdir('nested');"),
            STORE.replace(
                "loadCollection<Project>(PROJECTS_FILE)",
                "loadCollection<Project>(PROJECTS_FILE + suffix)",
            ),
            STORE.replace(
                "await fs.rename(tmpPath, filePath);",
                "await fs.rename(tmpPath, filePath); await atomicWrite(filePath, contents);",
            ),
        ];
        for text in variants {
            assert!(
                policy(workspace(&text).path(), json!({})).is_empty(),
                "{text}"
            );
        }
        for route in [
            "import { atomicWrite as write } from '@/lib/store'; export async function POST(req) { await write(req.url, '[]'); }",
            "import * as store from '@/lib/store'; export async function POST(req) { await store['atomic' + 'Write'](req.url, '[]'); }",
            "import /* comment */ * as store from '@/lib/store'; export async function POST(req) { await store['atomic' + 'Write'](req.url, '[]'); }",
            "export async function POST(req) { const store = await import('@/lib/store'); await store['atomic' + 'Write'](req.url, '[]'); }",
            r"import { ato\u006dicWrite as write } from '@/lib/store'; export async function POST(req) { await write(req.url, '[]'); }",
        ] {
            let root = workspace(STORE);
            std::fs::write(root.path().join("src/app/api/projects/route.ts"), route).unwrap();
            assert!(policy(root.path(), json!({})).is_empty(), "{route}");
        }
    }

    #[test]
    fn issue475_concrete_outputs_keep_filesystem_config_and_contract_boundaries() {
        for destination in [
            "../outside.json",
            "/tmp/outside.json",
            "data/../other.json",
            "src/private.json",
            "config/app.json",
            "data/package.json",
            "data/tsconfig.json",
            "data/build.config.json",
            "data/credentials.json",
            ".anvil/state.json",
            ".commandagent/state.json",
        ] {
            let text = STORE
                .replace(
                    "path.join(DATA_DIR, 'projects.json')",
                    &format!("{destination:?}"),
                )
                .replace(
                    "path.join(DATA_DIR, 'tasks.json')",
                    &format!("{destination:?}"),
                );
            assert!(
                policy(workspace(&text).path(), json!({})).is_empty(),
                "{destination}"
            );
        }
        for additions in [
            json!({"protected_paths":["data"]}),
            json!({"required_paths":["data/projects.json","data/tasks.json"]}),
            json!({"verify_commands":["cd nested && npm run build"]}),
        ] {
            assert!(policy(workspace(STORE).path(), additions).is_empty());
        }
        for config in [
            r#"{"extends":"./data/projects.json","references":[{"path":"./data/tasks.json"}]}"#,
            r#"{"extends":["./data/projects.json","./data/tasks.json"]}"#,
        ] {
            let root = workspace(STORE);
            std::fs::write(root.path().join("tsconfig.json"), config).unwrap();
            assert!(policy(root.path(), json!({})).is_empty());
        }
        let root = workspace(STORE);
        std::fs::write(root.path().join("data"), "not a directory").unwrap();
        assert!(policy(root.path(), json!({})).is_empty());
        #[cfg(unix)]
        for leaf in [false, true] {
            let root = workspace(STORE);
            if leaf {
                std::fs::create_dir(root.path().join("data")).unwrap();
                for p in ["projects", "tasks"] {
                    std::os::unix::fs::symlink(
                        "missing",
                        root.path().join(format!("data/{p}.json")),
                    )
                    .unwrap();
                }
            } else {
                std::os::unix::fs::symlink("missing", root.path().join("data")).unwrap();
            }
            assert!(policy(root.path(), json!({})).is_empty());
        }
    }

    #[test]
    fn issue475_escaped_foreign_import_refuses_generic_grant() {
        let root = workspace(STORE);
        let route = include_str!(
            "../../../../tests/corpus/apps/issue475-store-preflight/escaped-import-route.ts"
        );
        std::fs::write(root.path().join("src/app/api/projects/route.ts"), route).unwrap();
        let closure = crate::minimal_loop::import_scan::nextjs_route_bound_closure(root.path());
        assert!(
            closure.contains(Path::new("src/lib/store.ts")),
            "ordinary tasks route retains the store"
        );
        assert!(
            Source::parse(route).is_none(),
            "escaped importer is outside the supported language"
        );
        let paths = policy(root.path(), json!({}));
        println!("ISSUE475_ESCAPED_IMPORT allowed_generated_paths={paths:?}");
        assert!(paths.is_empty());
    }

    #[test]
    fn issue475_parameter_depth_bound_and_cycles_do_not_grant_partial_sets() {
        for count in [2, 9] {
            let mut source = STORE.to_string();
            source.push_str(
                "\nfunction forward0(filePath: string) { return atomicWrite(filePath, '[]'); }\n",
            );
            for i in 1..count {
                source.push_str(&format!(
                    "function forward{i}(filePath: string) {{ return forward{}(filePath); }}\n",
                    i - 1
                ));
            }
            source.push_str(&format!("forward{}(PROJECTS_FILE);", count - 1));
            let paths = policy(workspace(&source).path(), json!({}));
            if count == 2 {
                assert_eq!(paths, ["data/projects.json", "data/tasks.json"]);
            } else {
                assert!(paths.is_empty());
            }
        }
    }
}
