use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn commandagent(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_commandagent"))
        .args(args)
        .output()
        .unwrap()
}

fn doctor(extension_root: &Path) -> Output {
    let workspace = tempfile::tempdir().unwrap();
    let state = workspace.path().join("state");
    let home = workspace.path().join("home");
    fs::create_dir_all(&state).unwrap();
    fs::create_dir_all(&home).unwrap();
    Command::new(env!("CARGO_BIN_EXE_commandagent"))
        .args([
            "--doctor",
            "--provider",
            "openai",
            "--planner-provider",
            "openai",
            "--model",
            "executor-test-model",
            "--planner-model",
            "planner-test-model",
            "--cwd",
        ])
        .arg(workspace.path())
        .arg("--state-dir")
        .arg(&state)
        .arg("--extension-root")
        .arg(extension_root)
        .env("HOME", home)
        .env("PATH", "")
        .env("OPENAI_API_KEY", "integration-secret")
        .output()
        .unwrap()
}

fn v2_extension_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/corpus/apps/issue248-manifest-v2/extension-root")
}

#[test]
fn v1_external_manifest_remains_readable() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/corpus/apps/issue117-draft-profile/extension-root");
    let profiles =
        commandagent::planner::profile_manifest::source::load_extension_manifests(&root).unwrap();

    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].id(), "static-site");
}

#[test]
fn compact_v2_external_manifest_passes_doctor() {
    let path = v2_extension_root().join("profiles/static-site/manifest.toml");
    let body = fs::read_to_string(&path).unwrap();
    assert!(
        body.lines().count() <= 20,
        "{} lines\n{body}",
        body.lines().count()
    );

    let output = doctor(&v2_extension_root());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        output.status.success(),
        "{stdout}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout.contains("static-site"), "{stdout}");
    assert!(stdout.contains("sha256:"), "{stdout}");
}

#[test]
fn doctor_reports_one_toml_cause_with_file_line_and_column() {
    let extension = tempfile::tempdir().unwrap();
    let profile = extension.path().join("profiles/broken");
    fs::create_dir_all(&profile).unwrap();
    let path = profile.join("manifest.toml");
    fs::write(
        &path,
        "[metadata]\nid = \"broken\"\ndisplay_name = \"Broken\"\nschema_version = \"v2\"\ntask_family = \"unknown\"\n[metadata]\n",
    )
    .unwrap();

    let output = doctor(extension.path());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(!output.status.success(), "{stdout}");
    assert_eq!(
        stdout.matches("duplicate key `metadata`").count(),
        1,
        "{stdout}"
    );
    assert!(stdout.contains(&path.display().to_string()), "{stdout}");
    assert!(stdout.contains(":6:1:"), "{stdout}");
}

#[test]
fn external_located_error_is_terminal_and_renders_one_toml_cause() {
    let extension = tempfile::tempdir().unwrap();
    let profile = extension.path().join("profiles/broken");
    fs::create_dir_all(&profile).unwrap();
    let path = profile.join("manifest.toml");
    fs::write(&path, "[metadata]\nid = \"broken\"\n[metadata]\n").unwrap();

    let error =
        commandagent::planner::profile_manifest::source::load_extension_manifests(extension.path())
            .unwrap_err();
    assert!(std::error::Error::source(&error).is_none());
    let rendered = format!("{:#}", anyhow::Error::new(error));
    assert_eq!(
        rendered.matches("duplicate key `metadata`").count(),
        1,
        "{rendered}"
    );
    assert!(rendered.contains(&path.display().to_string()), "{rendered}");
    assert!(rendered.contains(":3:1:"), "{rendered}");
}

#[test]
fn validate_manifest_accepts_v2_and_rejects_one_located_toml_cause() {
    let valid = v2_extension_root().join("profiles/static-site/manifest.toml");
    let output = Command::new(env!("CARGO_BIN_EXE_commandagent"))
        .arg("--validate-manifest")
        .arg(&valid)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("valid external profile manifest"));

    let extension = tempfile::tempdir().unwrap();
    let profile = extension.path().join("profiles/broken");
    fs::create_dir_all(&profile).unwrap();
    let invalid = profile.join("manifest.toml");
    fs::write(&invalid, "[metadata]\nid = \"broken\"\n[metadata]\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_commandagent"))
        .arg("--validate-manifest")
        .arg(&invalid)
        .output()
        .unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(!output.status.success(), "{stderr}");
    assert_eq!(
        stderr.matches("duplicate key `metadata`").count(),
        1,
        "{stderr}"
    );
    assert!(stderr.contains(":3:1:"), "{stderr}");
}

#[test]
fn validate_manifest_rejects_an_overlay_without_an_existing_base() {
    let extension = tempfile::tempdir().unwrap();
    let profile = extension.path().join("profiles/missing-base");
    fs::create_dir_all(&profile).unwrap();
    let path = profile.join("overlay.toml");
    fs::write(
        &path,
        "[metadata]\nid = \"custom-profile\"\ndisplay_name = \"Custom\"\nschema_version = \"v1\"\nstatus = \"draft\"\n[overlay]\nbase_profile = \"missing-base\"\nmode = \"additive\"\n[artifacts]\nrequired = [\"README.md\"]\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_commandagent"))
        .arg("--validate-manifest")
        .arg(&path)
        .output()
        .unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(!output.status.success(), "{stderr}");
    assert!(
        stderr.contains("must be the canonical id of an admitted"),
        "{stderr}"
    );
    assert!(stderr.contains(":7:17:"), "{stderr}");
}

#[test]
fn init_profile_creates_a_bounded_valid_v2_template_once() {
    let extension = tempfile::tempdir().unwrap();
    let root = extension.path().to_str().unwrap();
    let first = commandagent(&[
        "--extension-root",
        root,
        "--init-profile",
        "neutral-profile",
    ]);
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );

    let path = extension
        .path()
        .join("profiles/neutral-profile/manifest.toml");
    let body = fs::read_to_string(&path).unwrap();
    assert!(
        body.lines().count() <= 20,
        "{} lines\n{body}",
        body.lines().count()
    );
    assert!(body.contains("schema_version = \"v2\""), "{body}");
    assert!(!body.contains("tailwind"), "{body}");

    let validate = Command::new(env!("CARGO_BIN_EXE_commandagent"))
        .arg("--validate-manifest")
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        validate.status.success(),
        "{}",
        String::from_utf8_lossy(&validate.stderr)
    );

    let second = commandagent(&[
        "--extension-root",
        root,
        "--init-profile",
        "neutral-profile",
    ]);
    let stderr = String::from_utf8(second.stderr).unwrap();
    assert!(!second.status.success(), "{stderr}");
    assert!(stderr.contains("refusing to overwrite"), "{stderr}");
    assert_eq!(fs::read_to_string(path).unwrap(), body);
}
