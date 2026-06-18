use crate::agent::minimal_loop::guards::is_file_change_tool;
use crate::agent::minimal_loop::loop_run::RunResult;
use std::path::Path;

pub(super) fn missing_paths(cwd: &Path, paths: &[String]) -> Vec<String> {
    paths
        .iter()
        .filter(|path| !cwd.join(path).exists())
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
        .map(|record| record.name.clone())
        .collect()
}

pub(super) fn display_path(cwd: &Path, path: &Path) -> String {
    path.strip_prefix(cwd).unwrap_or(path).display().to_string()
}
