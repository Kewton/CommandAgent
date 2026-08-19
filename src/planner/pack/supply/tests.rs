use std::path::{Path, PathBuf};

use super::*;

fn extension_root() -> (tempfile::TempDir, SupplyRoot) {
    let temp = tempfile::tempdir().unwrap();
    private_directory(temp.path());
    let root = SupplyRoot::open(temp.path()).unwrap();
    (temp, root)
}

#[cfg(unix)]
fn private_directory(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
}

#[cfg(not(unix))]
fn private_directory(_: &Path) {}

fn repository_pack(id: &str) -> Vec<StagedFile> {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("packs/cli-assist/1.0.0");
    let assist = std::fs::read_to_string(source.join("assist.yaml"))
        .unwrap()
        .replace("id: cli-assist", &format!("id: {id}"));
    vec![StagedFile {
        name: "assist.yaml".to_string(),
        bytes: assist.into_bytes(),
    }]
}

fn journal_lines(root: &Path) -> Vec<serde_json::Value> {
    let text = std::fs::read_to_string(root.join(journal::JOURNAL_FILE)).unwrap_or_default();
    text.lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

#[test]
fn stage_verify_pin_and_retire_write_only_below_the_extension_root() {
    let (temp, root) = extension_root();
    let files = repository_pack("local-supply");

    let report = root
        .stage("local-supply", "1.0.0", &files, Actor::Gui)
        .unwrap();
    assert_eq!(report.status, PackStatus::Staged);
    assert_eq!(report.conformance.status, "conformant");
    assert_eq!(report.scrub.status, "clean");
    assert_eq!(report.scrub.scanned, vec!["assist.yaml".to_string()]);
    assert_eq!(
        report.directory,
        temp.path()
            .canonicalize()
            .unwrap()
            .join("packs/local-supply/1.0.0")
    );

    let verified = root
        .verify_recorded("local-supply", "1.0.0", Actor::Gui)
        .unwrap();
    assert_eq!(verified.hash, report.hash);

    root.pin("local-supply", "1.0.0", &report.hash, Actor::Gui)
        .unwrap();
    let pin = std::fs::read_to_string(report.directory.join(PACK_PIN_FILE)).unwrap();
    assert_eq!(pin.trim(), report.hash);
    assert_eq!(
        super::super::catalog::status(&report.directory),
        PackStatus::Pinned
    );

    root.retire("local-supply", "1.0.0", Actor::Gui).unwrap();
    assert_eq!(
        super::super::catalog::status(&report.directory),
        PackStatus::Retired
    );
    // Retirement keeps every byte, so the pack stays auditable.
    assert!(report.directory.join("assist.yaml").is_file());
    assert_eq!(
        std::fs::read_to_string(report.directory.join(PACK_PIN_FILE)).unwrap(),
        pin
    );

    let lines = journal_lines(temp.path());
    let actions = lines
        .iter()
        .map(|line| line["action"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(actions, ["stage", "verify", "pin", "retire"]);
    for line in &lines {
        assert_eq!(line["actor"], "gui");
        assert_eq!(line["result"], "ok");
        assert_eq!(line["pack"]["id"], "local-supply");
        assert_eq!(line["pack"]["version"], "1.0.0");
        assert_eq!(line["pack"]["hash"], report.hash);
        assert!(line["ts"].as_str().unwrap().ends_with('Z'));
        assert!(line["detail"].is_string());
    }
}

#[test]
fn pinned_and_retired_packs_reject_every_further_mutation() {
    let (temp, root) = extension_root();
    let files = repository_pack("local-supply");
    let report = root
        .stage("local-supply", "1.0.0", &files, Actor::Gui)
        .unwrap();

    assert!(matches!(
        root.pin("local-supply", "1.0.0", "sha256:not-the-hash", Actor::Gui),
        Err(SupplyError::Verification { .. })
    ));
    root.pin("local-supply", "1.0.0", &report.hash, Actor::Gui)
        .unwrap();
    assert!(matches!(
        root.pin("local-supply", "1.0.0", &report.hash, Actor::Gui),
        Err(SupplyError::Conflict(_))
    ));
    assert!(matches!(
        root.stage("local-supply", "1.0.0", &files, Actor::Gui),
        Err(SupplyError::Conflict(_))
    ));

    root.retire("local-supply", "1.0.0", Actor::Gui).unwrap();
    assert!(matches!(
        root.retire("local-supply", "1.0.0", Actor::Gui),
        Err(SupplyError::Conflict(_))
    ));
    assert!(matches!(
        root.stage("local-supply", "1.0.0", &files, Actor::Gui),
        Err(SupplyError::Conflict(_))
    ));

    // Every refusal is recorded; nothing rewrites an earlier line.
    let lines = journal_lines(temp.path());
    let errors = lines
        .iter()
        .filter(|line| line["result"] == "error")
        .count();
    assert_eq!(errors, 5, "{lines:#?}");
    assert!(lines.iter().all(|line| line["pack"]["hash"] == report.hash));
}

#[test]
fn restaging_an_unpinned_pack_replaces_the_tree_and_leaves_no_staging_residue() {
    let (temp, root) = extension_root();
    let first = root
        .stage(
            "local-supply",
            "1.0.0",
            &repository_pack("local-supply"),
            Actor::Cli,
        )
        .unwrap();
    let mut files = repository_pack("local-supply");
    files.push(StagedFile {
        name: "materials/CONVENTIONS.md".to_string(),
        bytes: b"# conventions\n".to_vec(),
    });
    let second = root
        .stage("local-supply", "1.0.0", &files, Actor::Cli)
        .unwrap();

    assert_ne!(first.hash, second.hash);
    assert!(second.directory.join("materials/CONVENTIONS.md").is_file());
    assert!(!temp.path().join("packs/.staging").exists());
    assert_eq!(
        root.bundle("local-supply", "1.0.0")
            .unwrap()
            .into_iter()
            .map(|file| file.name)
            .collect::<Vec<_>>(),
        ["assist.yaml", "materials/CONVENTIONS.md"]
    );
}

#[test]
fn staged_members_are_bounded_named_and_credential_scrubbed_before_any_write() {
    let (temp, root) = extension_root();
    let base = repository_pack("local-supply");
    let target = temp.path().join("packs/local-supply/1.0.0");

    for rejected in [
        vec![],
        vec![StagedFile {
            name: "../escape.yaml".to_string(),
            bytes: b"x".to_vec(),
        }],
        vec![StagedFile {
            name: "materials/nested/deep.md".to_string(),
            bytes: b"x".to_vec(),
        }],
        vec![StagedFile {
            name: "materials/notes.txt".to_string(),
            bytes: b"x".to_vec(),
        }],
        vec![StagedFile {
            name: "README.md".to_string(),
            bytes: b"x".to_vec(),
        }],
        vec![StagedFile {
            name: "assist.yaml".to_string(),
            bytes: vec![b'a'; (MAX_FILE_BYTES + 1) as usize],
        }],
        vec![StagedFile {
            name: "assist.yaml".to_string(),
            bytes: vec![0xff, 0xfe],
        }],
    ] {
        assert!(
            matches!(
                root.stage("local-supply", "1.0.0", &rejected, Actor::Gui),
                Err(SupplyError::Invalid(_))
            ),
            "accepted {:?}",
            rejected.iter().map(|file| &file.name).collect::<Vec<_>>()
        );
        assert!(!target.exists(), "a rejected stage wrote bytes");
    }

    let mut credential = base.clone();
    credential.push(StagedFile {
        name: "materials/CONVENTIONS.md".to_string(),
        bytes: b"api_key: sk-abcdefghijklmnopqrstuvwxyz012345\n".to_vec(),
    });
    assert!(matches!(
        root.stage("local-supply", "1.0.0", &credential, Actor::Gui),
        Err(SupplyError::Invalid(_))
    ));
    assert!(!target.exists(), "credential-bearing bytes were written");

    let mut oversize = base.clone();
    oversize.push(StagedFile {
        name: "materials/BIG.md".to_string(),
        bytes: vec![b'#'; (MAX_MATERIAL_BYTES + 1) as usize],
    });
    assert!(matches!(
        root.stage("local-supply", "1.0.0", &oversize, Actor::Gui),
        Err(SupplyError::Invalid(_))
    ));
    assert!(!temp.path().join("journal.jsonl").exists());
}

#[test]
fn identifiers_outside_the_contract_regex_never_become_paths() {
    let (_temp, root) = extension_root();
    for (id, version) in [
        ("..", "1.0.0"),
        ("local/supply", "1.0.0"),
        ("Local", "1.0.0"),
        ("local-", "1.0.0"),
        ("local-supply", "1.0"),
        ("local-supply", "01.0.0"),
        ("local-supply", "../1.0.0"),
    ] {
        assert!(
            matches!(root.directory(id, version), Err(SupplyError::Invalid(_))),
            "accepted {id}@{version}"
        );
    }
}

#[test]
fn a_failed_verification_keeps_staged_bytes_but_never_pins_them() {
    let (temp, root) = extension_root();
    let broken = vec![StagedFile {
        name: "assist.yaml".to_string(),
        bytes: b"schema_version: commandagent.pack.assist/v0\npack:\n  id: local-supply\n  version: 1.0.0\n  profile: python-cli\n  intent: create\ninject: []\n".to_vec(),
    }];
    let error = root
        .stage("local-supply", "1.0.0", &broken, Actor::Gui)
        .unwrap_err();
    assert!(matches!(error, SupplyError::Verification { .. }));
    assert!(
        temp.path()
            .join("packs/local-supply/1.0.0/assist.yaml")
            .is_file()
    );

    let rows = root.list().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, PackStatus::Staged);
    assert!(!rows[0].conformance_ok);
    assert!(rows[0].detail.is_some());

    assert!(
        root.pin("local-supply", "1.0.0", "sha256:x", Actor::Gui)
            .is_err()
    );
    assert!(
        !temp
            .path()
            .join("packs/local-supply/1.0.0/pack.sha256")
            .exists()
    );
    assert_eq!(
        root.bundle("local-supply", "1.0.0")
            .unwrap()
            .into_iter()
            .map(|file| file.name)
            .collect::<Vec<_>>(),
        ["assist.yaml"]
    );
    assert!(matches!(
        root.retire("local-supply", "1.0.0", Actor::Gui),
        Err(SupplyError::Conflict(_))
    ));
}

#[test]
fn journal_detail_is_credential_scrubbed_and_bounded() {
    let entry = JournalEntry::new(
        Actor::Cli,
        Action::Verify,
        JournalPack {
            id: "local-supply".to_string(),
            version: "1.0.0".to_string(),
            hash: format!("sha256:{}", "0".repeat(64)),
        },
        Outcome::Error,
        format!(
            "token: sk-abcdefghijklmnopqrstuvwxyz012345\n{}",
            "x".repeat(journal::MAX_DETAIL_BYTES * 2)
        ),
    );
    assert!(!entry.detail.contains("sk-abcdefghijklmnopqrstuvwxyz012345"));
    assert!(entry.detail.contains("[redacted]"));
    assert!(entry.detail.len() <= journal::MAX_DETAIL_BYTES);
    assert!(!entry.detail.contains('\n'));
}

#[cfg(unix)]
#[test]
fn a_group_readable_or_missing_root_is_not_a_supply_root() {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempfile::tempdir().unwrap();
    std::fs::set_permissions(temp.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(matches!(
        SupplyRoot::open(temp.path()),
        Err(SupplyError::Root { .. })
    ));

    std::fs::set_permissions(temp.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let file = temp.path().join("not-a-directory");
    std::fs::write(&file, b"x").unwrap();
    assert!(matches!(
        SupplyRoot::open(&file),
        Err(SupplyError::Root { .. })
    ));

    let link = temp.path().join("link");
    std::os::unix::fs::symlink(temp.path(), &link).unwrap();
    assert!(matches!(
        SupplyRoot::open(&link),
        Err(SupplyError::Root { .. })
    ));

    assert!(matches!(
        SupplyRoot::open(&temp.path().join("absent")),
        Err(SupplyError::Root { .. })
    ));
}

#[cfg(unix)]
#[test]
fn managed_symlinks_and_a_symlinked_journal_never_redirect_supply_writes() {
    use std::os::unix::fs::{PermissionsExt, symlink};

    let (temp, root) = extension_root();
    let outside = tempfile::tempdir().unwrap();
    let packs = temp.path().join("packs");
    std::fs::create_dir(&packs).unwrap();
    std::fs::set_permissions(&packs, std::fs::Permissions::from_mode(0o700)).unwrap();
    symlink(outside.path(), packs.join("local-supply")).unwrap();
    assert!(matches!(
        root.stage(
            "local-supply",
            "1.0.0",
            &repository_pack("local-supply"),
            Actor::Gui
        ),
        Err(SupplyError::Invalid(_))
    ));
    assert!(std::fs::read_dir(outside.path()).unwrap().next().is_none());

    std::fs::remove_file(packs.join("local-supply")).unwrap();
    let outside_journal = outside.path().join("journal.jsonl");
    std::fs::write(&outside_journal, b"sentinel\n").unwrap();
    symlink(&outside_journal, temp.path().join(journal::JOURNAL_FILE)).unwrap();
    assert!(
        root.stage(
            "local-supply",
            "1.0.0",
            &repository_pack("local-supply"),
            Actor::Gui
        )
        .is_err()
    );
    assert_eq!(
        std::fs::read_to_string(outside_journal).unwrap(),
        "sentinel\n"
    );
}

#[cfg(unix)]
#[test]
fn symlinked_lifecycle_markers_cannot_bypass_immutability_or_selection() {
    use std::os::unix::fs::symlink;

    for marker in [PACK_PIN_FILE, RETIRED_MARKER_FILE] {
        let (_temp, root) = extension_root();
        let report = root
            .stage(
                "local-supply",
                "1.0.0",
                &repository_pack("local-supply"),
                Actor::Gui,
            )
            .unwrap();
        let outside = tempfile::NamedTempFile::new().unwrap();
        symlink(outside.path(), report.directory.join(marker)).unwrap();

        assert!(matches!(
            root.stage(
                "local-supply",
                "1.0.0",
                &repository_pack("local-supply"),
                Actor::Gui
            ),
            Err(SupplyError::Invalid(_))
        ));
        assert!(super::super::load_directory(&report.directory).is_err());
    }
}
