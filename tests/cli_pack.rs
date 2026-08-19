use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const ASSIST: &str = r#"schema_version: commandagent.pack.assist/v0
pack:
  id: nextjs-acme
  version: 1.0.0
  profile: nextjs
  intent: create
inject:
  - point: build-verification
    source: browser_interaction
    required: true
    params:
      fields:
        - dispatched_inputs
        - observed_state
        - hook_status
        - surface
        - outcome
"#;

fn write_pack(root: &Path, layout_has_packs: bool) -> (PathBuf, String) {
    let base = if layout_has_packs {
        root.join("packs")
    } else {
        root.to_path_buf()
    };
    let directory = base.join("nextjs-acme/1.0.0");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("assist.yaml"), ASSIST).unwrap();
    let hash = commandagent::planner::pack::exact_byte_hash(Some(ASSIST.as_bytes()), None);
    std::fs::write(directory.join("pack.sha256"), format!("{hash}\n")).unwrap();
    (directory, hash)
}

fn command(workspace: &Path, args: &[&str]) -> Output {
    let home = workspace.join("home");
    let state = workspace.join("state");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&state).unwrap();
    Command::new(env!("CARGO_BIN_EXE_commandagent"))
        .args(args)
        .args([
            "--cwd",
            workspace.to_str().unwrap(),
            "--state-dir",
            state.to_str().unwrap(),
        ])
        .env("HOME", home)
        .output()
        .unwrap()
}

#[test]
fn extension_root_wins_and_summary_projects_the_exact_pack_identity() {
    let workspace = tempfile::tempdir().unwrap();
    let extension = workspace.path().join("extensions");
    let (_, hash) = write_pack(&extension, false);
    let (repository_pack, _) = write_pack(workspace.path(), true);
    std::fs::write(repository_pack.join("pack.sha256"), "sha256:stale\n").unwrap();

    let output = command(
        workspace.path(),
        &[
            "--runs",
            "--profile",
            "nextjs",
            "--extension-root",
            extension.to_str().unwrap(),
            "--pack",
            "nextjs-acme@1.0.0",
            "--pack-hash",
            &hash,
            "--summary-json",
        ],
    );

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let summary: serde_json::Value = serde_json::from_slice(
        String::from_utf8(output.stdout)
            .unwrap()
            .lines()
            .last()
            .unwrap()
            .as_bytes(),
    )
    .unwrap();
    assert_eq!(summary["pack"]["id"], "nextjs-acme");
    assert_eq!(summary["pack"]["version"], "1.0.0");
    assert_eq!(summary["pack"]["hash"], hash);
    assert_eq!(summary["pack"]["source"], "extension_root");
}

#[test]
fn preset_pack_and_top_level_extension_root_activate_without_pack_flags() {
    let workspace = tempfile::tempdir().unwrap();
    let extension = workspace.path().join("extensions");
    let (_, hash) = write_pack(&extension, false);
    let config_dir = workspace.path().join(".commandagent");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(
        config_dir.join("config.toml"),
        "extension_root = \"extensions\"\n\n[preset.nextjs_acme_cagentpack]\npack = \"nextjs-acme@1.0.0\"\nprofile = \"nextjs\"\n",
    )
    .unwrap();

    let output = command(
        workspace.path(),
        &[
            "--runs",
            "--preset",
            "nextjs_acme_cagentpack",
            "--summary-json",
        ],
    );

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let summary: serde_json::Value = serde_json::from_str(stdout.lines().last().unwrap()).unwrap();
    assert_eq!(summary["pack"]["hash"], hash);
    assert_eq!(summary["pack"]["source"], "extension_root");
}

#[test]
fn invalid_pins_hashes_profiles_and_preset_conflicts_exit_two() {
    let cases: &[(&str, &[&str])] = &[
        ("unpinned selector", &["--pack", "nextjs-acme"]),
        (
            "explicit hash",
            &["--pack", "nextjs-acme@1.0.0", "--pack-hash", "sha256:wrong"],
        ),
        (
            "profile",
            &["--pack", "nextjs-acme@1.0.0", "--profile", "data"],
        ),
    ];
    for (name, extra) in cases {
        let workspace = tempfile::tempdir().unwrap();
        write_pack(workspace.path(), true);
        let mut args = vec!["--runs", "--profile", "nextjs"];
        if *name == "profile" {
            args = vec!["--runs"];
        }
        args.extend_from_slice(extra);
        let output = command(workspace.path(), &args);
        assert_eq!(
            output.status.code(),
            Some(2),
            "{name}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let workspace = tempfile::tempdir().unwrap();
    let (directory, _) = write_pack(workspace.path(), true);
    std::fs::write(directory.join("pack.sha256"), "sha256:wrong\n").unwrap();
    let output = command(
        workspace.path(),
        &[
            "--runs",
            "--profile",
            "nextjs",
            "--pack",
            "nextjs-acme@1.0.0",
        ],
    );
    assert_eq!(output.status.code(), Some(2));

    let workspace = tempfile::tempdir().unwrap();
    let (directory, _) = write_pack(workspace.path(), true);
    std::fs::remove_file(directory.join("pack.sha256")).unwrap();
    let output = command(
        workspace.path(),
        &[
            "--runs",
            "--profile",
            "nextjs",
            "--pack",
            "nextjs-acme@1.0.0",
        ],
    );
    assert_eq!(output.status.code(), Some(2));

    let workspace = tempfile::tempdir().unwrap();
    write_pack(workspace.path(), true);
    std::fs::create_dir_all(workspace.path().join(".commandagent")).unwrap();
    std::fs::write(
        workspace.path().join(".commandagent/config.toml"),
        "[preset.nextjs_acme_cagentpack]\npack = \"other@1.0.0\"\nprofile = \"nextjs\"\n",
    )
    .unwrap();
    let output = command(
        workspace.path(),
        &[
            "--runs",
            "--preset",
            "nextjs_acme_cagentpack",
            "--pack",
            "nextjs-acme@1.0.0",
        ],
    );
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn doctor_json_contains_pack_selection_check() {
    let workspace = tempfile::tempdir().unwrap();
    write_pack(workspace.path(), true);
    let output = command(
        workspace.path(),
        &[
            "--doctor",
            "--json",
            "--profile",
            "nextjs",
            "--pack",
            "nextjs-acme@1.0.0",
        ],
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let check = report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["id"] == "pack.selection")
        .unwrap();
    assert_eq!(check["status"], "pass");
    assert_eq!(check["details"]["selection"]["id"], "nextjs-acme");
}
