use std::path::Path;

use crate::minimal_loop::build_verifier;
use crate::tools::path_guard::resolve_existing;

use super::shell_words_with_spans;

pub(super) fn is_dependency_missing_for_command(root: &Path, command: &str, output: &str) -> bool {
    if is_existing_workspace_python_script(root, command) {
        return false;
    }
    build_verifier::is_dependency_missing_output(output)
}

fn is_existing_workspace_python_script(root: &Path, command: &str) -> bool {
    let Some(words) = shell_words_with_spans(command) else {
        return false;
    };
    let [program, script, ..] = words.as_slice() else {
        return false;
    };
    if !matches!(program.value.as_str(), "python" | "python3")
        || !script.value.to_ascii_lowercase().ends_with(".py")
    {
        return false;
    }
    resolve_existing(root, &script.value).is_ok_and(|path| path.is_file())
}

#[cfg(test)]
mod tests {
    use crate::minimal_loop::dependency_setup::NodeDependencySetupAuthority;
    use crate::planner::step_plan::PlanStep;

    #[test]
    fn measured_workspace_python_script_failure_is_not_dependency_setup() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("verify_pipeline.py"),
            "import sys\nsys.stderr.write('records.json not found at /workspace/output/records.json\\n')\nsys.exit(1)\n",
        )
        .unwrap();
        let step = PlanStep {
            id: "verify-results".to_string(),
            kind: "verify".to_string(),
            expected_result: "pass".to_string(),
            instruction: "Run the measured ingest verification command".to_string(),
            expected_paths: Vec::new(),
            verify: vec!["python3 verify_pipeline.py".to_string()],
        };

        let report = super::super::verify_step_with_profile_setup_observed_with_offline(
            dir.path(),
            &step,
            Some("ingest"),
            NodeDependencySetupAuthority::None,
            false,
        )
        .0;

        assert!(report.dependency_missing.is_empty(), "{report:?}");
        assert_eq!(report.command_failures.len(), 1, "{report:?}");
        assert_eq!(
            report.command_failures[0].command,
            "python3 verify_pipeline.py"
        );
    }

    #[test]
    fn actual_python_dependency_install_still_requires_authority() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("pyproject.toml"),
            "[project]\nname = \"fixture\"\nversion = \"0.1.0\"\ndependencies = [\"anvil-missing-fixture\"]\n",
        )
        .unwrap();
        let step = PlanStep {
            id: "install".to_string(),
            kind: "verify".to_string(),
            expected_result: "pass".to_string(),
            instruction: "Install a dependency".to_string(),
            expected_paths: Vec::new(),
            verify: vec!["pip install -e .".to_string()],
        };

        let report = super::super::verify_step(dir.path(), &step);

        assert_eq!(
            report.dependency_missing,
            ["dependency_setup_authority_required: pip install -e ."]
        );
        assert!(report.command_failures.is_empty(), "{report:?}");
    }
}
