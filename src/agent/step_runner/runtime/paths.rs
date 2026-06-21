use crate::agent::minimal_loop::guards::is_file_change_tool;
use crate::agent::minimal_loop::loop_run::RunResult;
use crate::safety::path_guard::PathGuard;
use std::path::Path;

pub(super) fn missing_paths(cwd: &Path, paths: &[String]) -> Vec<String> {
    let Ok(guard) = PathGuard::new(cwd) else {
        return paths.to_vec();
    };
    paths
        .iter()
        .filter(|path| match guard.resolve(path.as_str()) {
            Ok(resolved) => !resolved.exists(),
            Err(_) => true,
        })
        .cloned()
        .collect()
}

pub(super) fn result_changed_files(result: &RunResult) -> bool {
    result
        .tool_results
        .iter()
        .any(|record| record.ok && is_file_change_tool(&record.name))
}

pub(super) fn changed_file_markers(result: &RunResult) -> Vec<String> {
    result
        .tool_results
        .iter()
        .filter(|record| record.ok && is_file_change_tool(&record.name))
        .flat_map(|record| {
            if record.target_paths.is_empty() {
                vec![record.name.clone()]
            } else {
                record.target_paths.clone()
            }
        })
        .collect()
}

pub(super) fn display_path(cwd: &Path, path: &Path) -> String {
    path.strip_prefix(cwd).unwrap_or(path).display().to_string()
}
