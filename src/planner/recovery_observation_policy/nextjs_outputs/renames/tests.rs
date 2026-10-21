use super::*;
use crate::planner::recovery_observation_policy::RecoveryObservationPolicy;
use crate::planner::recovery_snapshot::current_preflight_source_sha256;

fn workspace(source: &str) -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("src/app/api/projects")).unwrap();
    std::fs::write(root.path().join("package.json"), "{}").unwrap();
    std::fs::write(root.path().join("src/app/api/projects/route.ts"), source).unwrap();
    root
}

fn policy(root: &Path, protected: &[&str]) -> Vec<String> {
    let contract = serde_json::from_value(
        serde_json::json!({"profile":crate::planner::profiles::nextjs::PROFILE_ID, "protected_paths":protected}),
    )
    .unwrap();
    RecoveryObservationPolicy::for_contract_at_workspace(&contract, root).allowed_generated_paths
}

#[test]
fn issue459_rename_registers_only_proven_destination_under_existing_filters() {
    for source in [
        "import fs from 'node:fs'; fs.renameSync(temp, 'data/tasks.json');",
        "import { renameSync as move } from 'fs'; const dest = 'data/tasks.json'; move(temp, dest);",
        "import { rename } from 'node:fs/promises'; rename(temp, 'data/tasks.json');",
        "import fs from 'fs'; import path from 'path'; const dest = path.join(process.cwd(), 'data', 'tasks.json'); fs.promises.rename(temp, dest);",
        "import fs from 'fs'; fs.rename(temp, 'data/tasks.json', () => {});",
    ] {
        let root = workspace(source);
        assert_eq!(policy(root.path(), &[]), ["data/tasks.json"], "{source}");
        assert!(policy(root.path(), &["data"]).is_empty());
        std::fs::create_dir_all(root.path().join("data")).unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("../package.json", root.path().join("data/tasks.json"))
                .unwrap();
            assert!(policy(root.path(), &[]).is_empty());
        }
    }
    for dest in [
        "../outside.json",
        "/tmp/outside.json",
        ".env",
        "credentials.json",
        "data/.private.json",
        "src/config.json",
        "tsconfig.json",
        "package.json",
    ] {
        let root = workspace(&format!(
            "import fs from 'fs'; fs.renameSync(temp, '{dest}');"
        ));
        assert!(policy(root.path(), &[]).is_empty(), "{dest}");
    }
}

#[test]
fn issue459_rename_unknown_or_shadowed_bindings_never_grant_outputs() {
    for source in [
        "import fs from 'fs'; fs.renameSync('data/tasks.json', destination);",
        "import fs from 'fs'; fs.renameSync(temp, 'data/tasks.json' + suffix);",
        "import fs from 'fs'; fs.renameSync(temp, choose('data/tasks.json'));",
        "import fs from 'fs'; function move(destination: string) { fs.renameSync(temp, destination); } move('data/tasks.json');",
        "import fs from 'fs'; function save(fs) { fs.renameSync(temp, 'data/tasks.json'); }",
        "import { renameSync } from 'untrusted'; renameSync(temp, 'data/tasks.json');",
        "import fs from 'fs'; fs.renameSync = fake; fs.renameSync(temp, 'data/tasks.json');",
        "import fs from 'fs'; let dest = 'data/tasks.json'; fs.renameSync(temp, dest);",
        "import fs from 'fs'; const dest = 'data/tasks.json'; dest = other; fs.renameSync(temp, dest);",
        "import fs from 'fs'; const dest = 'data/tasks.json'; `${dest = other}`; fs.renameSync(temp, dest);",
        "import fs from 'fs'; process.chdir('nested'); fs.renameSync(temp, 'data/tasks.json');",
        "const text = \"fs.renameSync(temp, 'data/tasks.json')\";",
        "// fs.renameSync(temp, 'data/tasks.json');\nexport const value = 1;",
    ] {
        let root = workspace(source);
        assert!(policy(root.path(), &[]).is_empty(), "{source}");
    }
}

#[test]
fn issue459_rename_source_deletion_and_extra_mutation_still_fail_snapshot() {
    let root = workspace("import fs from 'fs'; fs.renameSync('source.json', 'data/tasks.json');");
    std::fs::create_dir(root.path().join("data")).unwrap();
    std::fs::write(root.path().join("source.json"), "[]").unwrap();
    let allowed = policy(root.path(), &[]);
    assert_eq!(allowed, ["data/tasks.json"]);
    let before = current_preflight_source_sha256(root.path(), &allowed).unwrap();
    std::fs::rename(
        root.path().join("source.json"),
        root.path().join("data/tasks.json"),
    )
    .unwrap();
    assert_ne!(
        current_preflight_source_sha256(root.path(), &allowed).unwrap(),
        before
    );
    std::fs::write(root.path().join("source.json"), "[]").unwrap();
    assert_eq!(
        current_preflight_source_sha256(root.path(), &allowed).unwrap(),
        before
    );
    std::fs::write(root.path().join("data/unregistered.json"), "[]").unwrap();
    assert_ne!(
        current_preflight_source_sha256(root.path(), &allowed).unwrap(),
        before
    );
}

#[test]
fn issue459_rename_corpus_preserves_opaque_atomic_negative_and_proves_explicit_sinks() {
    let raw = include_str!(
        "../../../../../tests/corpus/apps/issue457-nextjs-contracts/overlays/members/src/lib/store.ts"
    );
    let explicit = include_str!(
        "../../../../../tests/corpus/apps/issue459-create-recovery-integration/overlays/observable/src/lib/store.ts"
    );
    assert!(policy(workspace(raw).path(), &[]).is_empty());
    assert_eq!(
        policy(workspace(explicit).path(), &[]),
        ["data/projects.json", "data/tasks.json"]
    );
    assert_eq!(
        policy(workspace(explicit).path(), &["data/projects.json"]),
        ["data/tasks.json"]
    );
}
