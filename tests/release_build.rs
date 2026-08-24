#![cfg(unix)]

use std::ffi::OsStr;
use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

const RELEASE_SCRIPT: &str = include_str!("../scripts/build-release.sh");
const BUILD_SCRIPT: &str = include_str!("../build.rs");
const ENV_COMPAT: &str = include_str!("../src/env_compat.rs");

const FAKE_CARGO: &str = r#"#!/usr/bin/env bash
set -euo pipefail

mkdir -p "$CARGO_TARGET_DIR/release/deps"
printf 'temporary link artifact\n' > "$CARGO_TARGET_DIR/release/deps/libtemporary.rlib"

if [[ "${FAKE_CARGO_MODE:-success}" == "build-failure" ]]; then
    exit 17
fi

candidate="$CARGO_TARGET_DIR/release/commandagent"
{
    printf '%s\n' '#!/usr/bin/env bash'
    printf 'printf "%%s\\n" "%s"\n' "$FAKE_BINARY_OUTPUT"
} > "$candidate"
chmod 0755 "$candidate"
"#;

struct Fixture {
    _temp: TempDir,
    root: PathBuf,
    fake_bin: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().expect("create fixture tempdir");
        let root = temp.path().join("repo");
        let fake_bin = temp.path().join("fake-bin");
        fs::create_dir_all(root.join("scripts")).expect("create scripts directory");
        fs::create_dir_all(&fake_bin).expect("create fake bin directory");

        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"commandagent\"\nversion = \"0.1.0\"\n",
        )
        .expect("write Cargo.toml");
        fs::write(root.join(".gitignore"), "/target/\n").expect("write .gitignore");
        write_executable(&root.join("scripts/build-release.sh"), RELEASE_SCRIPT);
        write_executable(&fake_bin.join("cargo"), FAKE_CARGO);

        run_checked(&root, "git", ["init", "-q"]);
        run_checked(&root, "git", ["config", "user.name", "Release Test"]);
        run_checked(
            &root,
            "git",
            ["config", "user.email", "release-test@example.invalid"],
        );
        run_checked(&root, "git", ["add", "."]);
        run_checked(
            &root,
            "git",
            ["-c", "commit.gpgsign=false", "commit", "-qm", "fixture"],
        );

        Self {
            _temp: temp,
            root,
            fake_bin,
        }
    }

    fn commit(&self) -> String {
        let output = run_checked(&self.root, "git", ["rev-parse", "--short", "HEAD"]);
        String::from_utf8(output.stdout)
            .expect("commit is UTF-8")
            .trim()
            .to_string()
    }

    fn expected_version(&self) -> String {
        format!("commandagent 0.1.0 {} 2026-07-20T00:00:00Z", self.commit())
    }

    fn run_release(&self, mode: &str, binary_output: &str) -> Output {
        let path = format!(
            "{}:{}",
            self.fake_bin.display(),
            std::env::var("PATH").unwrap_or_default()
        );
        Command::new("bash")
            .arg("scripts/build-release.sh")
            .current_dir(&self.root)
            .env("PATH", path)
            .env("FAKE_CARGO_MODE", mode)
            .env("FAKE_BINARY_OUTPUT", binary_output)
            .output()
            .expect("run release script")
    }

    fn seed_previous_release(&self) -> Vec<u8> {
        let release = self.root.join("target/release");
        fs::create_dir_all(release.join("deps")).expect("create stale deps");
        fs::create_dir_all(release.join("build")).expect("create stale build directory");
        fs::write(release.join("deps/libstale.rlib"), "stale").expect("write stale library");
        fs::write(release.join("build/stale-metadata"), "stale").expect("write stale metadata");
        let previous = b"previous published executable\n".to_vec();
        fs::write(release.join("commandagent"), &previous).expect("write previous executable");
        previous
    }

    fn assert_previous_preserved(&self, expected: &[u8]) {
        let actual = fs::read(self.root.join("target/release/commandagent"))
            .expect("read published executable");
        assert_eq!(actual, expected);
    }

    fn assert_no_temporary_build_artifacts(&self) {
        let target = self.root.join("target");
        let leftovers: Vec<_> = fs::read_dir(target)
            .expect("read target directory")
            .map(|entry| entry.expect("read target entry").file_name())
            .filter(|name| name.to_string_lossy().starts_with(".commandagent-release-"))
            .collect();
        assert!(
            leftovers.is_empty(),
            "temporary artifacts remain: {leftovers:?}"
        );
    }
}

#[test]
fn successful_build_publishes_only_the_executable_and_supports_launcher_symlink() {
    let fixture = Fixture::new();
    fixture.seed_previous_release();
    let expected_version = fixture.expected_version();

    let output = fixture.run_release("success", &expected_version);

    assert_success(&output);
    let release = fixture.root.join("target/release");
    let entries: Vec<_> = fs::read_dir(&release)
        .expect("read release directory")
        .map(|entry| entry.expect("read release entry").file_name())
        .collect();
    assert_eq!(entries, [OsStr::new("commandagent")]);
    assert!(!release.join("deps").exists());
    fixture.assert_no_temporary_build_artifacts();

    let launcher_dir = fixture.root.join("launcher-bin");
    fs::create_dir(&launcher_dir).expect("create launcher directory");
    symlink(
        release.join("commandagent"),
        launcher_dir.join("commandagentdev"),
    )
    .expect("create commandagentdev symlink");
    let launcher = Command::new(launcher_dir.join("commandagentdev"))
        .arg("--version")
        .output()
        .expect("run commandagentdev symlink");
    assert_success(&launcher);
    assert_eq!(
        String::from_utf8(launcher.stdout)
            .expect("launcher output is UTF-8")
            .trim(),
        expected_version
    );
}

#[test]
fn failed_build_preserves_previous_executable_and_removes_staging() {
    let fixture = Fixture::new();
    let previous = fixture.seed_previous_release();

    let output = fixture.run_release("build-failure", "unused");

    assert!(!output.status.success());
    fixture.assert_previous_preserved(&previous);
    fixture.assert_no_temporary_build_artifacts();
}

#[test]
fn failed_provenance_verification_preserves_previous_executable_and_removes_staging() {
    let fixture = Fixture::new();
    let previous = fixture.seed_previous_release();

    let output = fixture.run_release(
        "success",
        "commandagent 0.1.0 incorrect 2026-07-20T00:00:00Z",
    );

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("unexpected version provenance"),
        "stderr was: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    fixture.assert_previous_preserved(&previous);
    fixture.assert_no_temporary_build_artifacts();
}

#[test]
fn same_commit_rebuilds_have_identical_version_strings() {
    let temp = tempfile::tempdir().expect("create deterministic build fixture");
    let root = temp.path().join("repo");
    fs::create_dir_all(root.join("src")).expect("create fixture source directory");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"build-info-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("write fixture manifest");
    fs::write(
        root.join("Cargo.lock"),
        "# This file is automatically @generated by Cargo.\n# It is not intended for manual editing.\nversion = 4\n\n[[package]]\nname = \"build-info-fixture\"\nversion = \"0.1.0\"\n",
    )
    .expect("write fixture lockfile");
    fs::write(root.join("build.rs"), BUILD_SCRIPT).expect("write fixture build script");
    fs::write(root.join("src/env_compat.rs"), ENV_COMPAT).expect("write env compatibility");
    fs::write(
        root.join("src/main.rs"),
        "fn main() { println!(\"{}\", env!(\"COMMANDAGENT_VERSION\")); }\n",
    )
    .expect("write fixture binary");

    run_checked(&root, "git", ["init", "-q"]);
    run_checked(&root, "git", ["config", "user.name", "Build Info Test"]);
    run_checked(
        &root,
        "git",
        ["config", "user.email", "build-info@example.invalid"],
    );
    run_checked(&root, "git", ["add", "."]);
    let commit = Command::new("git")
        .args(["-c", "commit.gpgsign=false", "commit", "-qm", "fixture"])
        .current_dir(&root)
        .env("GIT_AUTHOR_DATE", "2026-08-03T00:00:00Z")
        .env("GIT_COMMITTER_DATE", "2026-08-03T00:00:00Z")
        .output()
        .expect("commit fixture");
    assert_success(&commit);

    let first = build_fixture_version(&root, &temp.path().join("target-one"));
    let second = build_fixture_version(&root, &temp.path().join("target-two"));
    assert_eq!(first, second);

    let short_commit =
        String::from_utf8(run_checked(&root, "git", ["rev-parse", "--short", "HEAD"]).stdout)
            .expect("commit is UTF-8");
    let commit_time =
        String::from_utf8(run_checked(&root, "git", ["show", "-s", "--format=%cI", "HEAD"]).stdout)
            .expect("commit time is UTF-8");
    assert_eq!(
        first,
        format!("0.1.0 {} {}", short_commit.trim(), commit_time.trim())
    );
}

fn build_fixture_version(root: &Path, target: &Path) -> String {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let build = Command::new(cargo)
        .args(["build", "--release", "--quiet"])
        .current_dir(root)
        .env("CARGO_TARGET_DIR", target)
        .env_remove("SOURCE_DATE_EPOCH")
        .env_remove("COMMANDAGENT_FORCE_BUILD_INFO")
        .env_remove("ANVIL_FORCE_BUILD_INFO")
        .output()
        .expect("rebuild fixture");
    assert_success(&build);
    let output = Command::new(target.join("release/build-info-fixture"))
        .output()
        .expect("run rebuilt fixture");
    assert_success(&output);
    String::from_utf8(output.stdout)
        .expect("version is UTF-8")
        .trim()
        .to_string()
}

fn write_executable(path: &Path, contents: &str) {
    fs::write(path, contents).expect("write executable fixture");
    let mut permissions = fs::metadata(path)
        .expect("read fixture metadata")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("set executable fixture permissions");
}

fn run_checked<const N: usize>(root: &Path, program: &str, args: [&str; N]) -> Output {
    let output = Command::new(program)
        .args(args)
        .current_dir(root)
        .output()
        .unwrap_or_else(|error| panic!("run {program}: {error}"));
    assert_success(&output);
    output
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "command failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
