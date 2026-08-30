//! Test-runner command recognition used by structural acceptance evidence.

pub(crate) fn missing_artifact_reason(command: &str) -> Option<&'static str> {
    if command.starts_with("python3 -m unittest") || command.starts_with("python -m unittest") {
        return Some("unittest_without_test_artifact");
    }
    ["pytest", "python3 -m pytest", "python -m pytest"]
        .iter()
        .any(|prefix| command == *prefix || command.starts_with(&format!("{prefix} ")))
        .then_some("pytest_without_test_artifact")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_supported_python_test_runners_without_prefix_confusion() {
        for command in [
            "python3 -m unittest discover",
            "python -m unittest tests",
            "pytest",
            "pytest -q tests",
            "python3 -m pytest -q tests",
            "python -m pytest -q tests",
        ] {
            assert!(missing_artifact_reason(command).is_some(), "{command}");
        }
        for command in ["pytester", "python3 -m pytester", "python3 cli.py"] {
            assert_eq!(missing_artifact_reason(command), None, "{command}");
        }
    }
}
