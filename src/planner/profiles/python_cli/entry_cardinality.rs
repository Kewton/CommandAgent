//! Final-acceptance check that a python-cli workspace has exactly one
//! `src/<package>/main.py` and that `[project.scripts]` points at it.

use std::path::Path;

use crate::planner::profile::profile_failure;
use crate::planner::verify::VerificationReport;

use super::{display_path, entrypoint_path};

/// Returns a profile failure when the workspace holds more than one
/// `src/<dir>/main.py`, or when a console script targets another package than
/// the resolved entrypoint. Returns `None` when the entrypoint is unambiguous.
pub(super) fn check(root: &Path, goal: Option<&str>) -> Option<VerificationReport> {
    let mains = src_main_files(root);
    if mains.len() >= 2 {
        return Some(profile_failure(format!(
            "Python CLI must have exactly one src/<package>/main.py; found {}: {}",
            mains.len(),
            mains.join(", ")
        )));
    }
    let entrypoint = entrypoint_path(root, goal)?;
    let package = entrypoint.parent()?.file_name()?.to_str()?.to_string();
    let mismatched = script_packages(root)
        .into_iter()
        .filter(|(_, target_package)| *target_package != package)
        .collect::<Vec<_>>();
    if mismatched.is_empty() {
        return None;
    }
    let listed = mismatched
        .iter()
        .map(|(name, target)| format!("{name} -> {target}"))
        .collect::<Vec<_>>()
        .join(", ");
    Some(profile_failure(format!(
        "[project.scripts] must point at the entrypoint package `{package}` ({}): {listed}",
        display_path(root, &entrypoint)
    )))
}

fn src_main_files(root: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(root.join("src")) else {
        return Vec::new();
    };
    let mut paths = entries
        .flatten()
        .map(|entry| entry.path().join("main.py"))
        .filter(|path| path.is_file())
        .map(|path| display_path(root, &path))
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

/// `(script name, top-level package of its target)` for each
/// `[project.scripts]` entry such as `tool = "pkg.main:main"`.
fn script_packages(root: &Path) -> Vec<(String, String)> {
    let Ok(raw) = std::fs::read_to_string(root.join("pyproject.toml")) else {
        return Vec::new();
    };
    let mut in_scripts = false;
    let mut scripts = Vec::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_scripts = trimmed.split('#').next().unwrap_or("").trim() == "[project.scripts]";
            continue;
        }
        if !in_scripts {
            continue;
        }
        let Some((name, value)) = trimmed.split_once('=') else {
            continue;
        };
        let target = value
            .split('#')
            .next()
            .unwrap_or("")
            .trim()
            .trim_matches(|ch| ch == '"' || ch == '\'');
        let module = target.split(':').next().unwrap_or("");
        let package = module.split('.').next().unwrap_or("");
        if !package.is_empty() {
            scripts.push((
                name.trim()
                    .trim_matches(|ch| ch == '"' || ch == '\'')
                    .to_string(),
                package.to_string(),
            ));
        }
    }
    scripts
}

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use crate::planner::profile::DomainProfile;
    use crate::planner::profiles::python_cli::PythonCliProfile;

    fn write_package(root: &std::path::Path, package: &str) {
        let dir = root.join("src").join(package);
        std::fs::create_dir_all(&dir).unwrap();
        let main = dir.join("main.py");
        std::fs::write(&main, "print('ok')\n").unwrap();
        std::fs::set_permissions(&main, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn write_pyproject(root: &std::path::Path, name: &str, script_target: &str) {
        std::fs::write(
            root.join("pyproject.toml"),
            format!(
                "[project]\nname = \"{name}\"\nversion = \"0.1.0\"\n\n[project.scripts]\n{name} = \"{script_target}\"\n"
            ),
        )
        .unwrap();
    }

    fn failures(root: &std::path::Path, goal: &str) -> Vec<String> {
        PythonCliProfile.verify_final(root, goal).profile_failures
    }

    #[test]
    fn single_package_passes_final_acceptance() {
        let dir = tempfile::tempdir().unwrap();
        write_package(dir.path(), "csv_summarizer");
        write_pyproject(dir.path(), "csv-summarizer", "csv_summarizer.main:main");

        let report = PythonCliProfile.verify_final(dir.path(), "Create csv_summarizer.py");
        assert!(report.is_pass(), "{report:?}");
    }

    #[test]
    fn two_packages_fail_and_list_both_paths() {
        for (first, second) in [("csv_summarizer", "app"), ("alpha", "beta")] {
            let dir = tempfile::tempdir().unwrap();
            write_package(dir.path(), first);
            write_package(dir.path(), second);
            write_pyproject(dir.path(), first, &format!("{first}.main:main"));

            let reasons = failures(dir.path(), &format!("Create {first}.py"));
            assert_eq!(reasons.len(), 1, "{reasons:?}");
            assert!(
                reasons[0].contains(&format!("src/{first}/main.py")),
                "{reasons:?}"
            );
            assert!(
                reasons[0].contains(&format!("src/{second}/main.py")),
                "{reasons:?}"
            );
        }
    }

    #[test]
    fn console_script_pointing_at_another_package_fails() {
        let dir = tempfile::tempdir().unwrap();
        write_package(dir.path(), "csv_summarizer");
        write_pyproject(dir.path(), "csv-summarizer", "app.main:main");

        let reasons = failures(dir.path(), "Create csv_summarizer.py");
        assert_eq!(reasons.len(), 1, "{reasons:?}");
        assert!(reasons[0].contains("app"), "{reasons:?}");
        assert!(reasons[0].contains("csv_summarizer"), "{reasons:?}");
    }

    #[test]
    fn issue811_corpus_cases_match_final_acceptance() {
        let raw = include_str!(
            "../../../../tests/corpus/apps/issue811-python-cli-single-entrypoint/fixtures/cases.json"
        );
        let cases: serde_json::Value = serde_json::from_str(raw).unwrap();
        for case in cases["cases"].as_array().unwrap() {
            let dir = tempfile::tempdir().unwrap();
            for file in case["files"].as_array().unwrap() {
                let rel = file.as_str().unwrap();
                if rel.ends_with("main.py") {
                    let package = rel.split('/').nth(1).unwrap();
                    write_package(dir.path(), package);
                }
            }
            let scripts = case["scripts"].as_object().unwrap();
            let (name, target) = scripts.iter().next().unwrap();
            write_pyproject(dir.path(), name, target.as_str().unwrap());

            let reasons = failures(dir.path(), case["goal"].as_str().unwrap());
            let id = case["id"].as_str().unwrap();
            match case["expect"].as_str().unwrap() {
                "pass" => assert!(reasons.is_empty(), "{id}: {reasons:?}"),
                _ => {
                    assert_eq!(reasons.len(), 1, "{id}: {reasons:?}");
                    for needle in case["reason_contains"].as_array().unwrap() {
                        assert!(
                            reasons[0].contains(needle.as_str().unwrap()),
                            "{id}: {reasons:?}"
                        );
                    }
                }
            }
        }
    }
}
