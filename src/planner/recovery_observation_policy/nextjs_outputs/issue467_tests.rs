use super::*;

const T3: &str =
    include_str!("../../../../tests/corpus/apps/issue467-json-control/historical/store.ts");

#[test]
fn issue467_t3_pid_causal_control() {
    use sha2::{Digest, Sha256};
    let evidence: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tests/corpus/apps/issue467-json-control/recognition-and-stage.json"
    ))
    .unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(T3.as_bytes())),
        evidence["recognizer_before_fix"]["source_sha256"]
    );
    assert_eq!(evidence["historical_first_write_stage"], "unknown");
    assert_eq!(evidence["historical_interaction_success"], false);
    for (label, text, expected) in [
        (
            "actual",
            T3.to_string(),
            vec!["data/inquiries.json".to_string()],
        ),
        (
            "pid_literal",
            T3.replace("${process.pid}", "123"),
            vec!["data/inquiries.json".to_string()],
        ),
    ] {
        let source = Source::parse(&text).expect("T3 parses");
        let paths = source.writer_paths();
        assert!(!source.unsafe_names.contains("process"));
        assert_eq!(paths, expected, "{label}");
    }
}

const MINIMUM: &str =
    include_str!("../../../../tests/corpus/apps/issue467-json-control/src/lib/store.js");

fn workspace(writer: &str) -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("src/app/api/inquiries")).unwrap();
    std::fs::create_dir_all(root.path().join("src/lib")).unwrap();
    std::fs::write(root.path().join("package.json"), "{}").unwrap();
    std::fs::write(root.path().join("src/app/api/inquiries/route.ts"),
        "import { readData } from '../../../lib/store'; export async function GET(){ return Response.json(await readData()); }").unwrap();
    std::fs::write(root.path().join("src/lib/store.ts"), writer).unwrap();
    root
}

fn policy(root: &Path, additions: serde_json::Value) -> Vec<String> {
    let mut value = serde_json::json!({"profile": crate::planner::profiles::nextjs::PROFILE_ID});
    value
        .as_object_mut()
        .unwrap()
        .extend(additions.as_object().unwrap().clone());
    super::super::RecoveryObservationPolicy::for_contract_at_workspace(
        &serde_json::from_value(value).unwrap(),
        root,
    )
    .allowed_generated_paths
}

#[test]
fn issue467_real_entry_accepts_only_exact_existing_or_lazy_json() {
    for writer in [T3, MINIMUM] {
        for existing in [false, true] {
            let root = workspace(writer);
            if existing {
                std::fs::create_dir(root.path().join("data")).unwrap();
                std::fs::write(root.path().join("data/inquiries.json"), "[]").unwrap();
            }
            assert_eq!(
                policy(root.path(), serde_json::json!({})),
                ["data/inquiries.json"]
            );
            for additions in [
                serde_json::json!({"protected_paths":["data"]}),
                serde_json::json!({"required_paths":["data/inquiries.json"]}),
                serde_json::json!({"verify_commands":["cd nested && npm run build"]}),
            ] {
                assert!(policy(root.path(), additions).is_empty());
            }
        }
    }
}

#[test]
fn issue467_pid_exception_keeps_opaque_mutation_shadowing_and_dynamic_sinks_denied() {
    let mutations = [
        MINIMUM.replace("${process.pid}", "${process.cwd = () => '/tmp'}"),
        MINIMUM.replace("${process.pid}", "${process.pid = 123}"),
        MINIMUM.replace("${process.pid}", "${process.pid++}"),
        MINIMUM.replace("${process.pid}", "${process['pid']}"),
        MINIMUM.replace("${process.pid}", "${process.pid()}"),
        MINIMUM.replace(
            "${process.pid}",
            "${process.pid || process.chdir('nested')}",
        ),
        MINIMUM.replace("${process.pid}", "${`${process.cwd = other}`}"),
        format!("const process = fake;\n{MINIMUM}"),
        format!("function unknown(process) {{}}\n{MINIMUM}"),
        format!("import process from 'untrusted';\n{MINIMUM}"),
        format!("{MINIMUM}\nprocess.cwd = fake;"),
        format!("{MINIMUM}\nprocess.chdir('nested');"),
        MINIMUM.replace(
            "fs.rename(tmp, DATA_FILE)",
            "fs.rename(tmp, DATA_FILE + suffix)",
        ),
        MINIMUM.replace("fs.rename(tmp, DATA_FILE)", "fs.rename(tmp, unknown)"),
        MINIMUM.replace(
            "fs.rename(tmp, DATA_FILE)",
            "fs.rename(tmp, `${process.pid}.json`)",
        ),
        MINIMUM.replace("fs.rename(tmp, DATA_FILE)", "fs.rename(tmp, tmp)"),
        MINIMUM.replace("writeData(data)", "writeData(fs)"),
    ];
    for writer in mutations {
        assert!(
            policy(workspace(&writer).path(), serde_json::json!({})).is_empty(),
            "{writer}"
        );
    }
}

#[test]
fn issue467_pid_does_not_expand_filesystem_or_config_authority() {
    for destination in [
        "../outside.json",
        "/tmp/outside.json",
        "src/private.json",
        "app/model.json",
        "config/settings.json",
        "data/credentials.json",
        "data/.private.json",
        "data/package.json",
        "data/tsconfig.json",
        "data/tsconfig.build.json",
        "data/build.config.json",
        ".commandagent/state.json",
        ".anvil/state.json",
    ] {
        let writer = MINIMUM.replace(
            "join(DATA_DIR, \"inquiries.json\")",
            &format!("{destination:?}"),
        );
        assert!(
            policy(workspace(&writer).path(), serde_json::json!({})).is_empty(),
            "{destination}"
        );
    }
    let root = workspace(MINIMUM);
    std::fs::write(
        root.path().join("tsconfig.json"),
        r#"{"extends":"./data/inquiries.json"}"#,
    )
    .unwrap();
    assert!(policy(root.path(), serde_json::json!({})).is_empty());
    #[cfg(unix)]
    for leaf in [false, true] {
        let root = workspace(MINIMUM);
        if leaf {
            std::fs::create_dir(root.path().join("data")).unwrap();
            std::os::unix::fs::symlink("missing", root.path().join("data/inquiries.json")).unwrap();
        } else {
            std::os::unix::fs::symlink("missing", root.path().join("data")).unwrap();
        }
        assert!(policy(root.path(), serde_json::json!({})).is_empty());
    }
}
