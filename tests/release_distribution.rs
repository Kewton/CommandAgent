#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const VERSION: &str = "1.2.3";
const TARGET: &str = "x86_64-apple-darwin";

struct InstallFixture {
    _temp: tempfile::TempDir,
    script: PathBuf,
    fake_bin: PathBuf,
    assets: PathBuf,
    home: PathBuf,
    temp_root: PathBuf,
    log: PathBuf,
}

impl InstallFixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let script = temp.path().join("install.sh");
        let fake_bin = temp.path().join("bin");
        let assets = temp.path().join("assets");
        let home = temp.path().join("home");
        let temp_root = temp.path().join("tmp");
        let log = temp.path().join("curl.log");

        fs::create_dir_all(&fake_bin).unwrap();
        fs::create_dir_all(&assets).unwrap();
        fs::create_dir_all(&home).unwrap();
        fs::create_dir_all(&temp_root).unwrap();
        fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/install.sh"),
            &script,
        )
        .unwrap();

        write_executable(
            &fake_bin.join("uname"),
            r#"#!/bin/sh
case "${1-}" in
  -s) printf '%s\n' "$INSTALL_TEST_OS" ;;
  -m) printf '%s\n' "$INSTALL_TEST_ARCH" ;;
  *) exit 2 ;;
esac
"#,
        );
        write_executable(
            &fake_bin.join("curl"),
            r#"#!/bin/sh
output=""
url=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    -o)
      shift
      output=${1-}
      ;;
    -H)
      shift
      ;;
    -*) ;;
    *) url=$1 ;;
  esac
  shift
done
printf '%s\n' "$url" >> "$INSTALL_TEST_LOG"
case "$url" in
  */releases/latest)
    printf '{"tag_name":"%s"}\n' "$INSTALL_TEST_LATEST_TAG"
    ;;
  */releases/download/*)
    [ -n "$output" ] || exit 2
    cp "$INSTALL_TEST_ASSETS/${url##*/}" "$output"
    ;;
  *) exit 22 ;;
esac
"#,
        );

        let fixture = Self {
            _temp: temp,
            script,
            fake_bin,
            assets,
            home,
            temp_root,
            log,
        };
        fixture.add_release(VERSION, TARGET);
        fixture
    }

    fn add_release(&self, version: &str, target: &str) {
        let payload = self.assets.join("payload");
        fs::create_dir_all(&payload).unwrap();
        write_executable(
            &payload.join("commandagent"),
            &format!("#!/bin/sh\nprintf 'commandagent {version} fixture\\n'\n"),
        );

        let archive_name = format!("commandagent-{version}-{target}.tar.gz");
        let archive = self.assets.join(&archive_name);
        let output = Command::new("tar")
            .args(["-czf"])
            .arg(&archive)
            .arg("commandagent")
            .current_dir(&payload)
            .output()
            .unwrap();
        assert!(output.status.success(), "{}", combined(&output));

        let hash = sha256(&archive);
        fs::write(
            self.assets.join(format!("{archive_name}.sha256")),
            format!("{hash}  {archive_name}\n"),
        )
        .unwrap();
    }

    fn run(&self, args: &[&str], os: &str, arch: &str) -> Output {
        let path = format!("{}:/usr/bin:/bin", self.fake_bin.display());
        Command::new("/bin/sh")
            .arg(&self.script)
            .args(args)
            .env("PATH", path)
            .env("HOME", &self.home)
            .env("TMPDIR", &self.temp_root)
            .env("INSTALL_TEST_ASSETS", &self.assets)
            .env("INSTALL_TEST_LOG", &self.log)
            .env("INSTALL_TEST_LATEST_TAG", format!("v{VERSION}"))
            .env("INSTALL_TEST_OS", os)
            .env("INSTALL_TEST_ARCH", arch)
            .env_remove("GITHUB_TOKEN")
            .output()
            .unwrap()
    }

    fn log_text(&self) -> String {
        fs::read_to_string(&self.log).unwrap_or_default()
    }
}

fn write_executable(path: &Path, contents: &str) {
    fs::write(path, contents).unwrap();
    let mut permissions = fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).unwrap();
}

fn sha256(path: &Path) -> String {
    let output = Command::new("sha256sum").arg(path).output().or_else(|_| {
        Command::new("shasum")
            .args(["-a", "256"])
            .arg(path)
            .output()
    });
    let output = output.expect("sha256sum or shasum must be available for the test");
    assert!(output.status.success(), "{}", combined(&output));
    String::from_utf8(output.stdout)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .to_owned()
}

fn combined(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn latest_release_is_verified_and_installed_into_default_directory() {
    let fixture = InstallFixture::new();
    let output = fixture.run(&[], "Darwin", "x86_64");
    let text = combined(&output);
    let installed = fixture.home.join(".local/bin/commandagent");

    assert!(output.status.success(), "{text}");
    assert!(installed.is_file());
    assert_ne!(
        fs::metadata(&installed).unwrap().permissions().mode() & 0o111,
        0
    );
    assert!(text.contains("Verified SHA-256 checksum"), "{text}");
    assert!(text.contains("Add CommandAgent to PATH"), "{text}");
    assert!(text.contains("export PATH="), "{text}");
    assert!(fixture.log_text().contains("/releases/latest"));
    assert!(
        fixture
            .log_text()
            .contains("commandagent-1.2.3-x86_64-apple-darwin.tar.gz")
    );

    let installed_output = Command::new(installed).output().unwrap();
    assert!(installed_output.status.success());
    assert_eq!(
        String::from_utf8(installed_output.stdout).unwrap(),
        "commandagent 1.2.3 fixture\n"
    );
}

#[test]
fn explicit_version_and_prefix_skip_latest_release_lookup() {
    let fixture = InstallFixture::new();
    let prefix = fixture.home.join("tools");
    let prefix_arg = prefix.to_str().unwrap();
    let output = fixture.run(
        &["--version", "v1.2.3", "--prefix", prefix_arg],
        "Darwin",
        "amd64",
    );
    let text = combined(&output);

    assert!(output.status.success(), "{text}");
    assert!(prefix.join("commandagent").is_file());
    assert!(!fixture.log_text().contains("/releases/latest"));
}

#[test]
fn checksum_mismatch_fails_without_installing_a_binary() {
    let fixture = InstallFixture::new();
    let checksum = fixture
        .assets
        .join("commandagent-1.2.3-x86_64-apple-darwin.tar.gz.sha256");
    fs::write(checksum, format!("{}  archive.tar.gz\n", "0".repeat(64))).unwrap();

    let output = fixture.run(&["--version", VERSION], "Darwin", "x86_64");
    let text = combined(&output);

    assert!(!output.status.success(), "{text}");
    assert!(text.contains("checksum verification failed"), "{text}");
    assert!(!fixture.home.join(".local/bin/commandagent").exists());
}

#[test]
fn unsupported_platform_fails_before_downloading_assets() {
    let fixture = InstallFixture::new();
    let output = fixture.run(&["--version", VERSION], "Linux", "aarch64");
    let text = combined(&output);

    assert!(!output.status.success(), "{text}");
    assert!(
        text.contains("unsupported operating system or architecture: Linux/aarch64"),
        "{text}"
    );
    assert!(fixture.log_text().is_empty());
}

#[test]
fn release_workflow_preserves_the_distribution_contract() {
    let workflow = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows/release.yml"),
    )
    .unwrap();

    for required in [
        "tags:\n      - \"v*\"",
        "actions/checkout@v7",
        "actions/upload-artifact@v7",
        "actions/download-artifact@v8",
        "softprops/action-gh-release@v3",
        "cargo test --locked",
        "aarch64-apple-darwin",
        "x86_64-apple-darwin",
        "runner: macos-15-intel",
        "x86_64-unknown-linux-gnu",
        "x86_64-unknown-linux-musl",
        "cargo build --release --locked --target",
        "commandagent-${version}-${{ matrix.target }}.tar.gz",
        "shasum -a 256 -c",
        "sha256sum \"$archive\"",
        "sha256sum -c",
        "generate_release_notes: true",
        "prerelease: ${{ contains(github.ref_name, '-') }}",
    ] {
        assert!(
            workflow.contains(required),
            "missing workflow contract: {required}"
        );
    }

    for obsolete in [
        "runner: macos-13",
        "generate-notes:",
        "actions/checkout@v4",
        "actions/upload-artifact@v4",
        "actions/download-artifact@v4",
        "softprops/action-gh-release@v2",
    ] {
        assert!(
            !workflow.contains(obsolete),
            "obsolete workflow configuration remains: {obsolete}"
        );
    }
}
