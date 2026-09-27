use std::fs;

use clap::Parser;
use commandagent::cli::Cli;
use commandagent::config::Config;
use commandagent::planner::extension_profiles;

#[test]
fn broken_higher_priority_config_falls_back_to_lower_priority_extension_root() {
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

    Config::from_cli(Cli::parse_from([
        "commandagent",
        "--cwd",
        workspace_root.to_str().unwrap(),
    ]))
    .expect("a broken higher-priority config must not abort configuration resolution");

    assert_eq!(
        extension_profiles::registered_root().as_deref(),
        Some(extension_root.as_path()),
        "extension_root must fall back to the lower-priority config file"
    );
}
