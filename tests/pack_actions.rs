use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn commandagent(arguments: &[&std::ffi::OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_commandagent"))
        .args(arguments)
        .current_dir(repository_root())
        .output()
        .unwrap()
}

fn copy_pack(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).unwrap();
    for name in ["assist.yaml", "eval.yaml"] {
        let source_file = source.join(name);
        if source_file.is_file() {
            fs::copy(source_file, destination.join(name)).unwrap();
        }
    }
}

fn write_named_cli_pack(destination: &Path, id: &str) {
    fs::create_dir_all(destination).unwrap();
    let assist = fs::read_to_string(repository_root().join("packs/cli-assist/1.0.0/assist.yaml"))
        .unwrap()
        .replacen("id: cli-assist", &format!("id: {id}"), 1);
    fs::write(destination.join("assist.yaml"), &assist).unwrap();
    let hash = commandagent::planner::pack::exact_byte_hash(Some(assist.as_bytes()), None);
    fs::write(destination.join("pack.sha256"), format!("{hash}\n")).unwrap();
}

#[test]
fn packs_lists_two_admitted_entries_and_a_local_pack_with_sources() {
    let temp = tempfile::tempdir().unwrap();
    let local = temp.path().join("cli-assist/1.0.0");
    copy_pack(&repository_root().join("packs/cli-assist/1.0.0"), &local);

    let output = commandagent(&[
        "--extension-root".as_ref(),
        temp.path().as_os_str(),
        "--profile".as_ref(),
        "python-cli".as_ref(),
        "--intent".as_ref(),
        "create".as_ref(),
        "--packs".as_ref(),
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        stdout
            .lines()
            .filter(|line| line.ends_with("\tadmitted"))
            .count(),
        2
    );
    assert_eq!(
        stdout
            .lines()
            .filter(|line| line.ends_with("\tlocal"))
            .count(),
        1
    );
    assert!(stdout.contains("cli-assist@1.0.0"));
    assert!(stdout.contains("cli-assist@1.1.0"));
}

#[test]
fn co_located_profiles_and_packs_remain_independently_usable() {
    let temp = tempfile::tempdir().unwrap();
    let extension = temp.path().join("extensions");
    let profile = extension.join("profiles/static-site");
    fs::create_dir_all(&profile).unwrap();
    fs::copy(
        repository_root()
            .join("tests/corpus/apps/issue117-draft-profile/extension-root/profiles/static-site/manifest.toml"),
        profile.join("manifest.toml"),
    )
    .unwrap();
    write_named_cli_pack(&extension.join("packs/my-cli-pack/1.0.0"), "my-cli-pack");

    let listed = commandagent(&[
        "--extension-root".as_ref(),
        extension.as_os_str(),
        "--profile".as_ref(),
        "python-cli".as_ref(),
        "--intent".as_ref(),
        "create".as_ref(),
        "--packs".as_ref(),
    ]);
    assert!(
        listed.status.success(),
        "{}",
        String::from_utf8_lossy(&listed.stderr)
    );
    assert!(
        String::from_utf8_lossy(&listed.stdout)
            .lines()
            .any(|line| line.starts_with("my-cli-pack@1.0.0\t") && line.ends_with("\tlocal"))
    );

    let profile_run = commandagent(&[
        "--extension-root".as_ref(),
        extension.as_os_str(),
        "--profile".as_ref(),
        "static-site".as_ref(),
        "--runs".as_ref(),
    ]);
    assert!(
        profile_run.status.success(),
        "{}",
        String::from_utf8_lossy(&profile_run.stderr)
    );

    let pack_run = commandagent(&[
        "--extension-root".as_ref(),
        extension.as_os_str(),
        "--profile".as_ref(),
        "python-cli".as_ref(),
        "--pack".as_ref(),
        "my-cli-pack@1.0.0".as_ref(),
        "--runs".as_ref(),
    ]);
    assert!(
        pack_run.status.success(),
        "{}",
        String::from_utf8_lossy(&pack_run.stderr)
    );
}

#[test]
fn packs_warns_for_invalid_candidates_and_keeps_listing_valid_local_packs() {
    let temp = tempfile::tempdir().unwrap();
    let extension = temp.path().join("extensions");
    let valid = extension.join("packs/valid-local/1.0.0");
    let invalid = extension.join("packs/broken/1.0.0");
    let memo = extension.join("notes/2026-08-21");
    write_named_cli_pack(&valid, "valid-local");
    fs::create_dir_all(&invalid).unwrap();
    fs::create_dir_all(&memo).unwrap();
    fs::write(memo.join("memo.txt"), "not a pack\n").unwrap();

    let output = commandagent(&[
        "--extension-root".as_ref(),
        extension.as_os_str(),
        "--profile".as_ref(),
        "python-cli".as_ref(),
        "--intent".as_ref(),
        "create".as_ref(),
        "--packs".as_ref(),
    ]);

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout
            .lines()
            .any(|line| line.starts_with("valid-local@1.0.0\t") && line.ends_with("\tlocal")),
        "{stdout}"
    );
    assert!(!stdout.contains("broken@1.0.0"), "{stdout}");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert_eq!(
        stderr
            .lines()
            .filter(|line| line.starts_with("warning: skipping invalid local pack"))
            .count(),
        2,
        "{stderr}"
    );
    for skipped in [invalid, memo] {
        assert!(stderr.contains(&skipped.display().to_string()), "{stderr}");
    }
}

#[test]
fn pack_verify_matches_the_pack_conformance_binary_report() {
    let directory = repository_root().join("packs/cli-assist/1.0.0");
    let commandagent_output = commandagent(&["--pack-verify".as_ref(), directory.as_os_str()]);
    let conformance_output = Command::new(env!("CARGO_BIN_EXE_pack_conformance"))
        .arg(&directory)
        .current_dir(repository_root())
        .output()
        .unwrap();

    assert!(commandagent_output.status.success());
    assert!(conformance_output.status.success());
    let direct: serde_json::Value = serde_json::from_slice(&commandagent_output.stdout).unwrap();
    let standalone: serde_json::Value = serde_json::from_slice(&conformance_output.stdout).unwrap();
    assert_eq!(direct, standalone);
    assert_eq!(direct["status"], "conformant");
    assert_eq!(direct["exact_byte_hash"], standalone["exact_byte_hash"]);
}

#[test]
fn pack_pin_creates_then_preserves_a_pin_and_rejects_tampering_with_exit_one() {
    let temp = tempfile::tempdir().unwrap();
    let pack = temp.path().join("cli-assist/1.0.0");
    copy_pack(&repository_root().join("packs/cli-assist/1.0.0"), &pack);
    let pin_path = pack.join("pack.sha256");

    let first = commandagent(&["--pack-pin".as_ref(), pack.as_os_str()]);
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(String::from_utf8_lossy(&first.stdout).starts_with("created "));
    let pinned = fs::read(&pin_path).unwrap();

    let second = commandagent(&["--pack-pin".as_ref(), pack.as_os_str()]);
    assert!(
        second.status.success(),
        "{}",
        String::from_utf8_lossy(&second.stderr)
    );
    assert!(String::from_utf8_lossy(&second.stdout).starts_with("unchanged "));
    assert_eq!(fs::read(&pin_path).unwrap(), pinned);

    let assist_path = pack.join("assist.yaml");
    let mut assist = fs::read(&assist_path).unwrap();
    assist.push(b'\n');
    fs::write(assist_path, assist).unwrap();
    let tampered = commandagent(&["--pack-pin".as_ref(), pack.as_os_str()]);
    assert_eq!(tampered.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&tampered.stderr).contains("pack hash mismatch"));
    assert_eq!(fs::read(&pin_path).unwrap(), pinned);
}
