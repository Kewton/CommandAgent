use std::path::Path;

use super::{VerifyCommandOracleRepair, shell_words_with_spans};

pub(super) fn normalize(
    command: &str,
    root: &Path,
    profile: Option<&str>,
) -> Option<VerifyCommandOracleRepair> {
    if profile != Some("python-cli") {
        return None;
    }
    let words = shell_words_with_spans(command)?;
    if words.len() < 2
        || !matches!(words[0].value.as_str(), "python" | "python3")
        || words[1].value.trim_start_matches("./") != "cli/main.py"
    {
        return None;
    }
    let documented =
        crate::planner::profiles::python_cli::argv_probe::documented_normal_args(root).ok()?;
    let (documented_index, documented_path) = documented
        .iter()
        .enumerate()
        .find(|(_, value)| root.join(value).is_file())?;
    if documented_path.chars().any(char::is_whitespace) {
        return None;
    }
    let required_flag = documented_index
        .checked_sub(1)
        .and_then(|index| documented.get(index))
        .filter(|value| value.starts_with("--"));
    let command_path = words
        .iter()
        .enumerate()
        .skip(2)
        .find(|(_, word)| root.join(&word.value).is_file());

    let normalized = if let Some((index, word)) = command_path {
        let flag = required_flag?;
        if index > 1 && words[index - 1].value == *flag {
            return None;
        }
        format!(
            "{}{} {}",
            &command[..word.start],
            flag,
            &command[word.start..]
        )
    } else {
        let suffix = required_flag
            .map(|flag| format!(" {flag} {documented_path}"))
            .unwrap_or_else(|| format!(" {documented_path}"));
        format!("{}{suffix}", command.trim_end())
    };
    Some(VerifyCommandOracleRepair {
        normalized,
        reason: "CLI verify input binding restored from the concrete documented invocation; planner verification semantics were preserved".to_string(),
        kind: "cli_documented_input_binding",
    })
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;
    use crate::minimal_loop::dependency_setup::NodeDependencySetupAuthority;
    use crate::planner::step_plan::PlanStep;

    #[derive(Deserialize)]
    struct MeasuredFixture {
        source: String,
        planner_verify: String,
        documented_usage: String,
        expected_normalized: String,
    }

    #[test]
    fn restores_measured_cli_input_binding_without_dropping_semantics() {
        let fixture: MeasuredFixture = serde_json::from_str(include_str!(
            "../../../tests/corpus/apps/test0725_cli_elev_004/fixtures/uat-test0801-cli-luna-007/verify-input-binding.json"
        ))
        .unwrap();
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("cli")).unwrap();
        std::fs::create_dir_all(dir.path().join("data")).unwrap();
        std::fs::write(dir.path().join("data/sample.txt"), "error: measured\n").unwrap();
        std::fs::write(
            dir.path().join("cli/main.py"),
            "import argparse\np=argparse.ArgumentParser()\np.add_argument('--file', required=True)\np.add_argument('--pattern', required=True)\na=p.parse_args()\nprint(a.pattern)\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("README.md"),
            format!("## Usage\n\n```bash\n{}\n```\n", fixture.documented_usage),
        )
        .unwrap();
        let events = dir.path().join("events.jsonl");
        let step = PlanStep {
            id: "verify-basic-extraction".to_string(),
            kind: "verify".to_string(),
            expected_result: "pass".to_string(),
            instruction: format!("Measured fixture: {}", fixture.source),
            expected_paths: Vec::new(),
            verify: vec![fixture.planner_verify],
        };

        let report = super::super::verify_step_with_profile_setup_observed_with_offline_and_events(
            dir.path(),
            &step,
            Some("python-cli"),
            NodeDependencySetupAuthority::None,
            false,
            Some(&events),
        )
        .0;

        assert!(report.is_pass(), "{report:?}");
        assert_eq!(report.runtime_command_normalizations.len(), 1);
        assert_eq!(
            report.runtime_command_normalizations[0].repaired,
            fixture.expected_normalized
        );
        assert!(
            std::fs::read_to_string(events)
                .unwrap()
                .contains("cli_documented_input_binding")
        );
    }

    #[test]
    fn preserves_a_required_positional_sample() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("cli")).unwrap();
        std::fs::create_dir_all(dir.path().join("data")).unwrap();
        std::fs::write(dir.path().join("cli/main.py"), "print('ok')\n").unwrap();
        std::fs::write(dir.path().join("data/sample.csv"), "amount\n1\n").unwrap();
        std::fs::write(
            dir.path().join("README.md"),
            "## Usage\n\n```bash\npython3 cli/main.py data/sample.csv --column amount\n```\n",
        )
        .unwrap();

        let repair = normalize(
            "python3 cli/main.py --column amount",
            dir.path(),
            Some("python-cli"),
        )
        .unwrap();

        assert_eq!(
            repair.normalized,
            "python3 cli/main.py --column amount data/sample.csv"
        );
        assert_eq!(repair.kind, "cli_documented_input_binding");
    }
}
