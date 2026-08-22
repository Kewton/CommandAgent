use std::path::{Path, PathBuf};

use anyhow::Context;
use sha2::{Digest, Sha256};

const HISTORY_DIRECTORY: &str = "workspace-history";

pub fn workspace_history_path(state_dir: &Path, workspace_root: &Path) -> anyhow::Result<PathBuf> {
    let workspace = workspace_root
        .canonicalize()
        .with_context(|| format!("canonicalize workspace {}", workspace_root.display()))?;
    let digest = Sha256::digest(workspace.as_os_str().as_encoded_bytes());
    Ok(state_dir
        .join(HISTORY_DIRECTORY)
        .join(format!("{digest:x}.txt")))
}

pub fn prepare_workspace_history_path(
    state_dir: &Path,
    workspace_root: &Path,
) -> anyhow::Result<PathBuf> {
    let path = workspace_history_path(state_dir, workspace_root)?;
    let parent = path
        .parent()
        .expect("workspace history path always has a parent");
    std::fs::create_dir_all(parent)
        .with_context(|| format!("create workspace history directory {}", parent.display()))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distinct_workspaces_use_distinct_non_revealing_leaves() {
        let root = tempfile::tempdir().unwrap();
        let state = root.path().join("state");
        let first = root.path().join("customer-alpha");
        let second = root.path().join("customer-beta");
        std::fs::create_dir_all(&first).unwrap();
        std::fs::create_dir_all(&second).unwrap();

        let first_path = workspace_history_path(&state, &first).unwrap();
        let second_path = workspace_history_path(&state, &second).unwrap();

        assert_ne!(first_path, second_path);
        assert_eq!(first_path.parent(), second_path.parent());
        let first_name = first_path.file_name().unwrap().to_string_lossy();
        assert_eq!(first_name.len(), 68);
        assert!(!first_name.contains("customer-alpha"));
    }

    #[test]
    fn preparing_workspace_history_preserves_legacy_shared_file() {
        let root = tempfile::tempdir().unwrap();
        let state = root.path().join("state");
        let workspace = root.path().join("workspace");
        std::fs::create_dir_all(&state).unwrap();
        std::fs::create_dir_all(&workspace).unwrap();
        let legacy = state.join("history.txt");
        std::fs::write(&legacy, "private legacy request\n").unwrap();

        let path = prepare_workspace_history_path(&state, &workspace).unwrap();

        assert_ne!(path, legacy);
        assert!(!path.exists());
        assert_eq!(
            std::fs::read_to_string(legacy).unwrap(),
            "private legacy request\n"
        );
    }
}
