use std::fs;

use clap::Parser;
use commandagent::cli::Cli;
use commandagent::config::Config;
use commandagent::planner::extension_profiles;

// Issue #516 originally pinned that a broken higher-priority config file was
// skipped and the lower-priority `extension_root` applied. Issue #782
// (user decision, 2026-10-21) replaced that: an unknown top-level key in any
// config layer is a startup error naming file, line and key, and a lower layer
// is never used in its place.
#[test]
fn broken_higher_priority_config_is_an_error_and_does_not_fall_back_to_lower_priority() {
    let workspace = tempfile::tempdir().unwrap();
    let workspace_root = workspace.path().canonicalize().unwrap();
    let extension_root = workspace_root.join("extensions");
    fs::create_dir_all(&extension_root).unwrap();

    fs::create_dir_all(workspace_root.join(".commandagent")).unwrap();
    fs::write(
        workspace_root.join(".commandagent/config.toml"),
        "extension_roots = \"broken\"\n",
    )
    .unwrap();

    fs::create_dir_all(workspace_root.join(".anvil")).unwrap();
    fs::write(
        workspace_root.join(".anvil/config.toml"),
        "extension_root = \"extensions\"\n",
    )
    .unwrap();

    let error = Config::from_cli(Cli::parse_from([
        "commandagent",
        "--cwd",
        workspace_root.to_str().unwrap(),
    ]))
    .map(|_| ())
    .expect_err("a broken higher-priority config must fail startup");
    let message = format!("{error:#}");
    assert!(
        message.contains(".commandagent/config.toml:1 unknown config key 'extension_roots'"),
        "{message}"
    );
    assert!(!message.contains("broken"), "{message}");
    assert_ne!(
        extension_profiles::registered_root().as_deref(),
        Some(extension_root.as_path()),
        "the lower-priority extension_root must not be used"
    );
}
