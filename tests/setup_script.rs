#![cfg(unix)]

use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

struct SetupFixture {
    _temp: tempfile::TempDir,
    root: PathBuf,
    fake_bin: PathBuf,
    home: PathBuf,
    state: PathBuf,
    log: PathBuf,
}

impl SetupFixture {
    fn new(rust_version: &str) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("repo");
        let fake_bin = temp.path().join("bin");
        let home = temp.path().join("home");
        let state = temp.path().join("state");
        let log = temp.path().join("commands.log");
        fs::create_dir_all(root.join("scripts")).unwrap();
        fs::create_dir_all(&fake_bin).unwrap();
        fs::create_dir_all(&home).unwrap();
        fs::create_dir_all(&state).unwrap();
        fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/setup.sh"),
            root.join("scripts/setup.sh"),
        )
        .unwrap();
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"commandagent\"\nversion = \"0.1.0\"\nrust-version = \"1.88\"\n",
        )
        .unwrap();

        write_executable(
            &fake_bin.join("rustc"),
            &format!("#!/bin/sh\nprintf 'rustc {rust_version} (fixture)\\n'\n"),
        );
        write_executable(
            &fake_bin.join("cargo"),
            r#"#!/bin/sh
if [ "${1-}" = "--version" ]; then
  printf 'cargo 1.94.0 (fixture)\n'
  exit 0
fi
printf 'cargo|%s\n' "$*" >> "$SETUP_TEST_LOG"
if [ "${1-}" = "install" ]; then
  : > "$SETUP_TEST_STATE/installed"
  exit 0
fi
if [ "${1-}" = "build" ] && [ "${2-}" = "--release" ]; then
  : > "$SETUP_TEST_STATE/installed"
  mkdir -p target/release
  cp "$SETUP_TEST_COMMANDAGENT" target/release/commandagent
  chmod 0755 target/release/commandagent
  exit 0
fi
exit 2
"#,
        );
        write_executable(
            &fake_bin.join("git"),
            r#"#!/bin/sh
if [ "${1-}" = "--version" ]; then
  printf 'git version 2.50.0\n'
  exit 0
fi
if [ "${1-}" = "-C" ]; then
  shift 2
fi
case "${1-}" in
  rev-parse) printf 'abc1234\n' ;;
  status) ;;
  *) exit 2 ;;
esac
"#,
        );
        write_executable(&fake_bin.join("curl"), "#!/bin/sh\nexit 0\n");
        write_executable(&fake_bin.join("node"), "#!/bin/sh\nprintf 'v24.0.0\\n'\n");
        write_executable(&fake_bin.join("npm"), "#!/bin/sh\nprintf '11.0.0\\n'\n");
        write_executable(
            &fake_bin.join("python3"),
            "#!/bin/sh\nprintf 'Python 3.13.0\\n'\n",
        );
        write_executable(
            &fake_bin.join("ollama"),
            r#"#!/bin/sh
if [ "${1-}" = "list" ]; then
  printf 'NAME ID SIZE MODIFIED\n'
  if [ ! -f "$SETUP_TEST_STATE/no-models" ]; then
    printf 'fixture:latest abc 1 GB now\n'
  fi
  exit 0
fi
printf 'ollama|%s\n' "$*" >> "$SETUP_TEST_LOG"
if [ "${1-}" = "pull" ]; then
  rm -f "$SETUP_TEST_STATE/no-models"
fi
exit 0
"#,
        );
        write_executable(
            &fake_bin.join("commandagent"),
            r#"#!/bin/sh
case "${1-}" in
  --version)
    if [ -f "$SETUP_TEST_STATE/installed" ]; then
      printf 'commandagent 0.1.0 abc1234 2026-07-20T00:00:00Z\n'
    else
      printf 'commandagent 0.0.9 old0000 2026-01-01T00:00:00Z\n'
    fi
    ;;
  --setup-interaction-probe)
    printf 'commandagent|--setup-interaction-probe\n' >> "$SETUP_TEST_LOG"
    printf 'interaction probe setup: existing playwright 1.0 resolved from fixture\n'
    ;;
  --completions)
    printf 'commandagent|--completions|%s\n' "${2-}" >> "$SETUP_TEST_LOG"
    case "${2-}" in
      bash) printf '_commandagent() {}\n' ;;
      fish) printf 'complete -c commandagent\n' ;;
      zsh) printf '#compdef commandagent\n' ;;
      *) exit 2 ;;
    esac
    ;;
  --model-probe)
    printf 'commandagent|--model-probe\n' >> "$SETUP_TEST_LOG"
    printf 'model probe fixture passed\n'
    ;;
  *) exit 2 ;;
esac
"#,
        );

        Self {
            _temp: temp,
            root,
            fake_bin,
            home,
            state,
            log,
        }
    }

    fn mark_current_binary(&self) {
        fs::write(self.state.join("installed"), "").unwrap();
    }

    fn run(&self, args: &[&str], input: &str) -> Output {
        self.run_with_shell(args, input, "/bin/zsh")
    }

    fn run_with_shell(&self, args: &[&str], input: &str, shell: &str) -> Output {
        let path = format!("{}:/usr/bin:/bin", self.fake_bin.display());
        let commandagent = self.fake_bin.join("commandagent");
        let mut child = Command::new("/bin/bash")
            .arg(self.root.join("scripts/setup.sh"))
            .args(args)
            .current_dir(&self.root)
            .env("PATH", path)
            .env("HOME", &self.home)
            .env("SHELL", shell)
            .env("CARGO_HOME", self.home.join(".cargo"))
            .env("SETUP_TEST_STATE", &self.state)
            .env("SETUP_TEST_LOG", &self.log)
            .env("SETUP_TEST_COMMANDAGENT", commandagent)
            .env_remove("GEMINI_API_KEY")
            .env_remove("OPENAI_API_KEY")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
        child.wait_with_output().unwrap()
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

fn combined(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn yes_mode_installs_bash_and_fish_completions_in_user_paths() {
    let cases = [
        (
            "/bin/bash",
            ".local/share/bash-completion/completions/commandagent",
            "_commandagent() {}\n",
        ),
        (
            "/usr/local/bin/fish",
            ".config/fish/completions/commandagent.fish",
            "complete -c commandagent\n",
        ),
    ];

    for (shell, relative_path, expected) in cases {
        let fixture = SetupFixture::new("1.94.0");
        fixture.mark_current_binary();
        let output = fixture.run_with_shell(&["--yes"], "", shell);
        let text = combined(&output);

        assert!(output.status.success(), "{shell}: {text}");
        assert_eq!(
            fs::read_to_string(fixture.home.join(relative_path)).unwrap(),
            expected
        );
    }
}

#[test]
fn check_only_reports_prerequisites_without_mutation() {
    let fixture = SetupFixture::new("1.88.0");
    let output = fixture.run(&["--check-only"], "");
    let text = combined(&output);

    assert!(output.status.success(), "{text}");
    assert!(
        text.contains("rustc 1.88.0 satisfies rust-version 1.88"),
        "{text}"
    );
    assert!(
        text.contains("Changes                  skipped --check-only made no changes"),
        "{text}"
    );
    assert!(!fixture.root.join(".env").exists());
    assert!(
        !fixture
            .home
            .join(".local/share/zsh/site-functions/_commandagent")
            .is_file()
    );
    assert!(!fixture.state.join("installed").exists());
    assert!(fixture.log_text().is_empty());
}

#[test]
fn check_only_rejects_outdated_rust_with_remediation() {
    let fixture = SetupFixture::new("1.87.9");
    let output = fixture.run(&["--check-only"], "");
    let text = combined(&output);

    assert!(!output.status.success(), "{text}");
    assert!(
        text.contains("rustc 1.87.9 is older than Cargo.toml rust-version 1.88"),
        "{text}"
    );
    assert!(
        text.contains("https://www.rust-lang.org/tools/install"),
        "{text}"
    );
    assert!(
        text.contains("install or repair the required tools above"),
        "{text}"
    );
    assert!(!fixture.state.join("installed").exists());
}

#[test]
fn yes_mode_installs_once_and_runs_safe_probe_defaults() {
    let fixture = SetupFixture::new("1.94.0");
    let first = fixture.run(&["--yes"], "");
    let first_text = combined(&first);
    assert!(first.status.success(), "{first_text}");
    assert!(
        first_text.contains("non-interactive mode never writes secrets"),
        "{first_text}"
    );
    assert!(
        first_text.contains("existing managed Playwright installation reused"),
        "{first_text}"
    );
    assert!(
        first_text.contains("model smoke test completed"),
        "{first_text}"
    );
    assert!(!fixture.root.join(".env").exists());
    let completion = fixture
        .home
        .join(".local/share/zsh/site-functions/_commandagent");
    assert_eq!(
        fs::read_to_string(completion).unwrap(),
        "#compdef commandagent\n"
    );

    let second = fixture.run(&["--yes"], "");
    let second_text = combined(&second);
    assert!(second.status.success(), "{second_text}");
    assert!(
        second_text.contains("matching source build already available"),
        "{second_text}"
    );

    let log = fixture.log_text();
    assert_eq!(
        log.matches("cargo|install --path . --locked").count(),
        1,
        "{log}"
    );
    assert_eq!(
        log.matches("commandagent|--setup-interaction-probe")
            .count(),
        2,
        "{log}"
    );
    assert_eq!(
        log.matches("commandagent|--model-probe").count(),
        2,
        "{log}"
    );
    assert_eq!(
        log.matches("commandagent|--completions|zsh").count(),
        2,
        "{log}"
    );
}

#[test]
fn declined_install_uses_release_build_fallback() {
    let fixture = SetupFixture::new("1.94.0");
    let output = fixture.run(&[], "n\nn\nn\nn\nn\nn\n");
    let text = combined(&output);

    assert!(output.status.success(), "{text}");
    assert!(
        text.contains("Install declined; building target/release/commandagent instead."),
        "{text}"
    );
    assert!(text.contains("PATH suggestion: export PATH="), "{text}");
    assert!(text.contains("commandagent 0.1.0 abc1234"), "{text}");
    assert!(fixture.log_text().contains("cargo|build --release"));
}

#[test]
fn empty_ollama_list_uses_only_the_model_name_entered_by_the_user() {
    let fixture = SetupFixture::new("1.94.0");
    fixture.mark_current_binary();
    fs::write(fixture.state.join("no-models"), "").unwrap();
    let output = fixture.run(&[], "n\nn\nn\ny\nchosen-model:latest\nn\nn\n");
    let text = combined(&output);

    assert!(output.status.success(), "{text}");
    assert!(text.contains("Model name (no default):"), "{text}");
    assert!(text.contains("requested model pulled"), "{text}");
    let log = fixture.log_text();
    assert!(log.contains("ollama|pull chosen-model:latest"), "{log}");
    assert!(!log.contains("ollama|pull fixture:latest"), "{log}");
}

#[test]
fn interactive_secret_input_creates_private_env_and_is_idempotent() {
    let fixture = SetupFixture::new("1.94.0");
    fixture.mark_current_binary();
    let secret = "fixture-secret-must-not-be-printed";
    let first = fixture.run(&[], &format!("n\ny\n{secret}\nn\nn\nn\n"));
    let first_text = combined(&first);

    assert!(first.status.success(), "{first_text}");
    assert!(!first_text.contains(secret), "{first_text}");
    let env_path = fixture.root.join(".env");
    let env_text = fs::read_to_string(&env_path).unwrap();
    assert_eq!(env_text, format!("GEMINI_API_KEY={secret}\n"));
    assert_eq!(
        fs::metadata(&env_path).unwrap().permissions().mode() & 0o777,
        0o600
    );

    let second = fixture.run(&[], "n\nn\nn\nn\n");
    let second_text = combined(&second);
    assert!(second.status.success(), "{second_text}");
    assert!(
        second_text.contains("GEMINI_API_KEY is already configured"),
        "{second_text}"
    );
    let second_env = fs::read_to_string(&env_path).unwrap();
    assert_eq!(second_env.matches("GEMINI_API_KEY=").count(), 1);
    assert_eq!(second_env, env_text);
}
