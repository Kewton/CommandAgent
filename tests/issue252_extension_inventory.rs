use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/corpus/apps/issue252-extension-inventory/extension-root")
}

fn commandagent(arguments: &[&std::ffi::OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_commandagent"))
        .args(arguments)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap()
}

fn inventory(json: bool) -> Output {
    let root = fixture_root();
    let mut arguments = vec![
        "--extensions".as_ref(),
        "--extension-root".as_ref(),
        root.as_os_str(),
    ];
    if json {
        arguments.push("--json".as_ref());
    }
    commandagent(&arguments)
}

#[test]
fn text_inventory_keeps_one_reason_line_per_bad_entry() {
    let output = inventory(false);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let lines = stdout.lines().collect::<Vec<_>>();

    let malformed = lines
        .iter()
        .find(|line| line.starts_with("PROFILE\tbroken\tprofile\tinvalid\t"))
        .expect("missing malformed profile row");
    assert!(malformed.contains("duplicate key"), "{malformed}");

    let overlay = lines
        .iter()
        .find(|line| line.starts_with("PROFILE\tacme-nextjs\toverlay\tdraft\tnextjs\t"))
        .expect("missing overlay row");
    assert!(overlay.contains("\tusable\t-"), "{overlay}");

    let unpinned = lines
        .iter()
        .find(|line| line.starts_with("PACK\tunpinned-cli@1.0.0\t"))
        .expect("missing unpinned pack row");
    assert!(unpinned.contains("\tstaged\tpassed\tpython-cli×create\tunusable\t"));
    assert!(unpinned.contains("pack is not pinned"), "{unpinned}");

    let unregistered = lines
        .iter()
        .find(|line| line.starts_with("PACK\tunregistered-source@1.0.0\t"))
        .expect("missing unregistered-source row");
    assert!(
        unregistered.contains("unregistered AssistSource id"),
        "{unregistered}"
    );
    assert!(unregistered.contains("not_registered"), "{unregistered}");

    let journal = lines
        .iter()
        .find(|line| line.starts_with("JOURNAL\tpresent\t"))
        .expect("missing journal row");
    assert!(journal.contains("\"action\":\"pin\""), "{journal}");
    assert!(journal.contains("\"detail\":\"final record\""), "{journal}");
    assert!(!stdout.contains("earlier record"), "{stdout}");
}

#[test]
fn json_inventory_projects_profiles_packs_and_latest_journal() {
    let output = inventory(true);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["schema_version"], "commandagent.extensions/v1");
    assert!(report["profile_catalog_error"].as_str().is_some());

    let profiles = report["profiles"].as_array().unwrap();
    let broken = profiles.iter().find(|row| row["id"] == "broken").unwrap();
    assert_eq!(broken["usable"], false);
    assert!(broken["reason"].as_str().unwrap().contains("duplicate key"));
    let overlay = profiles
        .iter()
        .find(|row| row["id"] == "acme-nextjs")
        .unwrap();
    assert_eq!(overlay["kind"], "overlay");
    assert_eq!(overlay["status"], "draft");
    assert_eq!(overlay["base_profile"], "nextjs");
    assert_eq!(overlay["usable"], true);
    assert!(overlay["hash"].as_str().unwrap().starts_with("sha256:"));

    let packs = report["packs"].as_array().unwrap();
    let unpinned = packs
        .iter()
        .find(|row| row["id"] == "unpinned-cli")
        .unwrap();
    assert_eq!(unpinned["status"], "staged");
    assert_eq!(unpinned["conformance"], "passed");
    assert_eq!(unpinned["profile"], "python-cli");
    assert_eq!(unpinned["intent"], "create");
    assert_eq!(unpinned["usable"], false);
    assert!(unpinned["reason"].as_str().unwrap().contains("not pinned"));
    let unregistered = packs
        .iter()
        .find(|row| row["id"] == "unregistered-source")
        .unwrap();
    assert_eq!(unregistered["conformance"], "not_run");
    assert!(
        unregistered["reason"]
            .as_str()
            .unwrap()
            .contains("unregistered AssistSource id")
    );

    assert_eq!(report["journal"]["status"], "present");
    assert_eq!(report["journal"]["latest"]["action"], "pin");
    assert_eq!(report["journal"]["latest"]["detail"], "final record");
}

#[test]
fn extensions_uses_the_workspace_configured_root_when_flag_is_omitted() {
    let workspace = tempfile::tempdir().unwrap();
    let config_dir = workspace.path().join(".commandagent");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(
        config_dir.join("config.toml"),
        format!("extension_root = {:?}\n", fixture_root().to_string_lossy()),
    )
    .unwrap();

    let output = commandagent(&[
        "--extensions".as_ref(),
        "--json".as_ref(),
        "--cwd".as_ref(),
        workspace.path().as_os_str(),
    ]);

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["schema_version"], "commandagent.extensions/v1");
    assert_eq!(report["packs"].as_array().unwrap().len(), 2);
}
