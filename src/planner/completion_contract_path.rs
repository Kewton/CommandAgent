use std::path::{Path, PathBuf};

pub(crate) fn generated_path(
    workspace_root: &Path,
    eval_events_path: Option<&Path>,
    filename: &str,
) -> PathBuf {
    if let Some(event_directory) = eval_events_path.and_then(Path::parent)
        && let Some(event_directory) =
            canonical_workspace_directory(workspace_root, event_directory)
    {
        return event_directory.join(filename);
    }
    crate::runtime_paths::workspace_dir(workspace_root).join(filename)
}

fn canonical_workspace_directory(workspace_root: &Path, candidate: &Path) -> Option<PathBuf> {
    let workspace = workspace_root.canonicalize().ok()?;
    let candidate = if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        workspace_root.join(candidate)
    };
    candidate
        .canonicalize()
        .ok()?
        .starts_with(&workspace)
        .then_some(candidate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_an_event_directory_inside_the_workspace() {
        let root = tempfile::tempdir().unwrap();
        let run = root.path().join(".commandagent/runs/session");
        std::fs::create_dir_all(&run).unwrap();

        assert_eq!(
            generated_path(
                root.path(),
                Some(&run.join("events.jsonl")),
                "contract.json"
            ),
            run.join("contract.json")
        );
    }

    #[test]
    fn isolated_gui_run_contracts_fall_back_to_the_session_workspace() {
        let root = tempfile::tempdir().unwrap();
        let workspace = root.path().join("sessions/session");
        let run = root.path().join(".commandagent/runs/session");
        std::fs::create_dir_all(&workspace).unwrap();
        std::fs::create_dir_all(&run).unwrap();

        for filename in [
            "completion-contract-plan-run.json",
            "completion-contract-ultra-plan-run.json",
        ] {
            assert_eq!(
                generated_path(&workspace, Some(&run.join("events.jsonl")), filename),
                workspace.join(".commandagent").join(filename)
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn event_directory_symlink_cannot_escape_the_workspace() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        let workspace = root.path().join("workspace");
        let outside = root.path().join("outside");
        std::fs::create_dir_all(&workspace).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        symlink(&outside, workspace.join("run-link")).unwrap();

        assert_eq!(
            generated_path(
                &workspace,
                Some(&workspace.join("run-link/events.jsonl")),
                "contract.json"
            ),
            workspace.join(".commandagent/contract.json")
        );
    }
}
