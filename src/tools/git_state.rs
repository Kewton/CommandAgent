use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use crate::bounded_process::{self, BoundedProcessOutcomeKind};

const GIT_TIMEOUT: Duration = Duration::from_secs(3);
pub const DIRTY_WARNING: &str = "warning: workspace has uncommitted Git changes before the run; the exit report will include pre-existing changes";
pub const UNMANAGED_WARNING: &str =
    "warning: workspace is not Git-managed; CommandAgent cannot report workspace changes at exit";
pub const EXIT_REPORT_HEADING: &str = "Workspace Git changes at exit:";

#[derive(Debug)]
enum Inspection {
    Managed(Status),
    Unmanaged,
    Unavailable(String),
}

#[derive(Debug, Default)]
struct Status {
    dirty: bool,
    tracked_changes: bool,
    untracked: Vec<String>,
}

pub(crate) struct RunReporter {
    root: PathBuf,
}

impl RunReporter {
    pub(crate) fn start(root: &Path) -> Self {
        match inspect(root) {
            Inspection::Managed(status) if status.dirty => eprintln!("{DIRTY_WARNING}"),
            Inspection::Managed(_) => {}
            Inspection::Unmanaged => eprintln!("{UNMANAGED_WARNING}"),
            Inspection::Unavailable(reason) => {
                eprintln!("warning: workspace Git state could not be inspected: {reason}")
            }
        }
        Self {
            root: root.to_path_buf(),
        }
    }
}

impl Drop for RunReporter {
    fn drop(&mut self) {
        if let Some(report) = render_exit_report(&self.root) {
            eprintln!("{report}");
        }
    }
}

fn inspect(root: &Path) -> Inspection {
    let inside = match run_git(root, &["rev-parse", "--is-inside-work-tree"]) {
        Ok(output) if output.success => output,
        Ok(output) if output.stderr.contains("not a git repository") => {
            return Inspection::Unmanaged;
        }
        Ok(output) => {
            return Inspection::Unavailable(nonempty_error(&output));
        }
        Err(error) => return Inspection::Unavailable(error),
    };
    if inside.stdout.trim() != "true" {
        return Inspection::Unmanaged;
    }
    match run_git(
        root,
        &[
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
            "--",
            ".",
        ],
    ) {
        Ok(output) if output.success => Inspection::Managed(parse_status(&output.stdout)),
        Ok(output) => Inspection::Unavailable(nonempty_error(&output)),
        Err(error) => Inspection::Unavailable(error),
    }
}

fn render_exit_report(root: &Path) -> Option<String> {
    let Inspection::Managed(status) = inspect(root) else {
        return None;
    };
    if !status.dirty {
        return None;
    }

    let mut lines = vec![EXIT_REPORT_HEADING.to_string()];
    if status.tracked_changes {
        let stat = diff_stat(root);
        lines.push("Tracked/staged diff stat:".to_string());
        if stat.is_empty() {
            lines.push(
                "  (tracked changes reported by git status; diff stat unavailable)".to_string(),
            );
        } else {
            lines.extend(stat.lines().map(|line| format!("  {line}")));
        }
    }
    if !status.untracked.is_empty() {
        lines.push("Untracked files:".to_string());
        lines.extend(status.untracked.iter().map(|path| format!("  - {path}")));
    }
    Some(lines.join("\n"))
}

fn diff_stat(root: &Path) -> String {
    let has_head =
        run_git(root, &["rev-parse", "--verify", "HEAD"]).is_ok_and(|output| output.success);
    let argument_sets: &[&[&str]] = if has_head {
        &[&["diff", "--stat", "HEAD", "--", "."]]
    } else {
        &[
            &["diff", "--stat", "--", "."],
            &["diff", "--cached", "--stat", "--", "."],
        ]
    };
    argument_sets
        .iter()
        .filter_map(|arguments| run_git(root, arguments).ok())
        .filter(|output| output.success)
        .map(|output| output.stdout.trim().to_string())
        .filter(|output| !output.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn parse_status(output: &str) -> Status {
    let entries = output
        .split('\0')
        .filter(|entry| !entry.is_empty())
        .collect::<Vec<_>>();
    let untracked = entries
        .iter()
        .filter_map(|entry| entry.strip_prefix("?? ").map(ToOwned::to_owned))
        .collect::<Vec<_>>();
    Status {
        dirty: !entries.is_empty(),
        tracked_changes: entries.iter().any(|entry| !entry.starts_with("?? ")),
        untracked,
    }
}

struct GitOutput {
    success: bool,
    stdout: String,
    stderr: String,
}

fn run_git(root: &Path, arguments: &[&str]) -> Result<GitOutput, String> {
    let mut command = Command::new("git");
    command.arg("-C").arg(root).args(arguments);
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let output = bounded_process::run_with_timeout(&mut command, GIT_TIMEOUT)
        .map_err(|error| format!("failed to launch git: {error}"))?;
    if output.kind != BoundedProcessOutcomeKind::Exited {
        return Err(format!("git inspection ended with {:?}", output.kind));
    }
    Ok(GitOutput {
        success: output.success(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

fn nonempty_error(output: &GitOutput) -> String {
    let message = output.stderr.trim();
    if message.is_empty() {
        "git command failed without diagnostic output".to_string()
    } else {
        crate::eval_events::body_snippet(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn git(root: &Path, arguments: &[&str]) {
        let status = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(arguments)
            .status()
            .unwrap();
        assert!(status.success(), "git {arguments:?}");
    }

    #[test]
    fn unmanaged_workspace_has_an_explicit_warning() {
        let root = tempfile::tempdir().unwrap();
        assert!(matches!(inspect(root.path()), Inspection::Unmanaged));
    }

    #[test]
    fn dirty_workspace_reports_tracked_stat_and_untracked_files() {
        let root = tempfile::tempdir().unwrap();
        git(root.path(), &["init", "-q"]);
        fs::write(root.path().join("tracked.txt"), "before\n").unwrap();
        git(root.path(), &["add", "tracked.txt"]);
        git(
            root.path(),
            &[
                "-c",
                "user.name=CommandAgent Test",
                "-c",
                "user.email=test@example.invalid",
                "commit",
                "-q",
                "-m",
                "initial",
            ],
        );
        fs::write(root.path().join("tracked.txt"), "after\n").unwrap();
        fs::write(root.path().join("new.txt"), "new\n").unwrap();

        let inspection = inspect(root.path());
        let Inspection::Managed(status) = inspection else {
            panic!("expected managed workspace, got {inspection:?}");
        };
        assert!(status.dirty);
        assert!(status.tracked_changes);
        assert_eq!(status.untracked, ["new.txt"]);

        let report = render_exit_report(root.path()).unwrap();
        assert!(report.contains(EXIT_REPORT_HEADING), "{report}");
        assert!(report.contains("tracked.txt"), "{report}");
        assert!(report.contains("Untracked files:\n  - new.txt"), "{report}");
    }
}
