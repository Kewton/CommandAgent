use std::path::{Path, PathBuf};

pub const WORKSPACE_DIR: &str = ".commandagent";
pub const LEGACY_WORKSPACE_DIR: &str = ".anvil";
pub const STATE_DIR: &str = "commandagent";
pub const LEGACY_STATE_DIR: &str = "anvilminimal";

pub fn workspace_dir(root: &Path) -> PathBuf {
    root.join(WORKSPACE_DIR)
}

pub fn legacy_workspace_dir(root: &Path) -> PathBuf {
    root.join(LEGACY_WORKSPACE_DIR)
}

pub fn runs_dir(root: &Path) -> PathBuf {
    workspace_dir(root).join("runs")
}

pub fn run_read_dirs(root: &Path) -> [PathBuf; 2] {
    [runs_dir(root), legacy_workspace_dir(root).join("runs")]
}

pub fn plans_dir(root: &Path) -> PathBuf {
    workspace_dir(root).join("plans")
}

pub fn repairs_dir(root: &Path) -> PathBuf {
    workspace_dir(root).join("repairs")
}

pub fn evidence_dir(root: &Path) -> PathBuf {
    workspace_dir(root).join("evidence")
}

pub fn default_state_dir() -> PathBuf {
    state_base_dir().join(STATE_DIR)
}

pub fn legacy_state_dir() -> PathBuf {
    state_base_dir().join(LEGACY_STATE_DIR)
}

fn state_base_dir() -> PathBuf {
    if let Some(xdg) = std::env::var_os("XDG_STATE_HOME") {
        return PathBuf::from(xdg);
    }
    let home = std::env::var_os("HOME").unwrap_or_else(|| ".".into());
    PathBuf::from(home).join(".local").join("state")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_writes_are_canonical_and_reads_keep_legacy_second() {
        let root = Path::new("/workspace");

        assert_eq!(runs_dir(root), root.join(".commandagent/runs"));
        assert_eq!(plans_dir(root), root.join(".commandagent/plans"));
        assert_eq!(repairs_dir(root), root.join(".commandagent/repairs"));
        assert_eq!(evidence_dir(root), root.join(".commandagent/evidence"));
        assert_eq!(
            run_read_dirs(root),
            [root.join(".commandagent/runs"), root.join(".anvil/runs")]
        );
    }

    #[test]
    fn state_names_freeze_new_write_and_legacy_read_contract() {
        assert!(default_state_dir().ends_with("commandagent"));
        assert!(legacy_state_dir().ends_with("anvilminimal"));
        assert_eq!(default_state_dir().parent(), legacy_state_dir().parent());
    }
}
