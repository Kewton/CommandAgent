use std::collections::BTreeSet;
use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::argv_probe::{self, Observation};

pub const EVIDENCE_PATH: &str = "evidence/help-binding.json";
pub const IMPLEMENTATION_TO_HELP_SCOPE: &str = "unknown_option_rejection_with_bound_normal_argv_v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NearestMiss {
    pub candidate: String,
    pub edit_distance: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptionBinding {
    pub direction: String,
    pub option: String,
    pub ok: bool,
    pub observation: Observation,
    pub nearest_miss: Option<NearestMiss>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    pub capability_id: String,
    pub status: String,
    pub ok: bool,
    pub help_observation: Observation,
    pub help_options: Vec<String>,
    pub bindings: Vec<OptionBinding>,
    pub implementation_to_help_scope: String,
    pub failure_kinds: Vec<String>,
}

pub fn run(
    root: &Path,
    entry: &Path,
    normal_args: &[String],
    timeout: Duration,
) -> anyhow::Result<Report> {
    let help = argv_probe::observe(root, entry, &["--help".to_string()], timeout, "help")?;
    let help_text = format!("{}\n{}", help.stdout.text, help.stderr.text);
    let help_options = extract_options(&help_text);
    let mut bindings = help_options
        .iter()
        .map(|option| observe_help_option(root, entry, option, timeout))
        .collect::<anyhow::Result<Vec<_>>>()?;
    let mut unknown_args = normal_args.to_vec();
    unknown_args.push(argv_probe::INVALID_OPTION.to_string());
    let unknown = argv_probe::observe(
        root,
        entry,
        &unknown_args,
        timeout,
        "implementation-to-help",
    )?;
    let unknown_error = is_unrecognized(&unknown.stderr.text);
    bindings.push(OptionBinding {
        direction: "implementation_to_help".to_string(),
        option: argv_probe::INVALID_OPTION.to_string(),
        ok: unknown.exit_code.is_some_and(|code| code != 0) && unknown_error,
        nearest_miss: (!unknown_error)
            .then(|| nearest_miss(argv_probe::INVALID_OPTION, &unknown.stderr.text))
            .flatten(),
        observation: unknown,
    });
    let mut failure_kinds = Vec::new();
    if help.exit_code != Some(0) {
        failure_kinds.push("help_binding:help_exit_nonzero".to_string());
    }
    for binding in bindings.iter().filter(|binding| !binding.ok) {
        failure_kinds.push(format!(
            "help_binding:{}:option={}",
            binding.direction, binding.option
        ));
    }
    let ok = failure_kinds.is_empty();
    let status = if !ok {
        "failed"
    } else if help_options.is_empty() {
        "claims_absent"
    } else {
        "pass"
    };
    let report = Report {
        capability_id: "help_binding".to_string(),
        status: status.to_string(),
        ok,
        help_observation: help,
        help_options,
        bindings,
        // v0 cannot enumerate parser-accepted options hidden from runtime help.
        // The reverse direction is therefore limited to proving real unknown rejection.
        implementation_to_help_scope: IMPLEMENTATION_TO_HELP_SCOPE.to_string(),
        failure_kinds,
    };
    argv_probe::write_json(root, EVIDENCE_PATH, &report)?;
    Ok(report)
}

fn observe_help_option(
    root: &Path,
    entry: &Path,
    option: &str,
    timeout: Duration,
) -> anyhow::Result<OptionBinding> {
    let observation = argv_probe::observe(
        root,
        entry,
        &[option.to_string()],
        timeout,
        "help-to-implementation",
    )?;
    let rejected = is_unrecognized(&observation.stderr.text);
    Ok(OptionBinding {
        direction: "help_to_implementation".to_string(),
        option: option.to_string(),
        ok: !rejected,
        nearest_miss: rejected
            .then(|| nearest_miss(option, &observation.stderr.text))
            .flatten(),
        observation,
    })
}

fn extract_options(text: &str) -> Vec<String> {
    text.split_whitespace()
        .filter_map(|word| {
            let option = word
                .trim_matches(|ch: char| "[](){},:".contains(ch))
                .split('=')
                .next()?;
            (option.starts_with('-')
                && option.len() > 1
                && option[1..].chars().any(|ch| ch.is_ascii_alphanumeric()))
            .then(|| option.to_string())
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn is_unrecognized(stderr: &str) -> bool {
    let lower = stderr.to_ascii_lowercase();
    [
        "unrecognized argument",
        "unrecognized option",
        "unknown argument",
        "unknown option",
        "no such option",
        "unexpected argument",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

fn nearest_miss(expected: &str, observed: &str) -> Option<NearestMiss> {
    extract_options(observed)
        .into_iter()
        .map(|candidate| NearestMiss {
            edit_distance: edit_distance(expected, &candidate),
            candidate,
        })
        .min_by_key(|miss| miss.edit_distance)
}

fn edit_distance(left: &str, right: &str) -> usize {
    let mut costs = (0..=right.len()).collect::<Vec<_>>();
    for (i, a) in left.bytes().enumerate() {
        let mut diagonal = i;
        costs[0] = i + 1;
        for (j, b) in right.bytes().enumerate() {
            let above = costs[j + 1];
            costs[j + 1] = if a == b {
                diagonal
            } else {
                1 + diagonal.min(above).min(costs[j])
            };
            diagonal = above;
        }
    }
    costs[right.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    const MEASURED_FIXTURE: &str = "tests/corpus/apps/test0725_cli_elev_003/fixtures";
    const MEASURED_HELP_BINDING: &str = include_str!(
        "../../../../tests/corpus/apps/test0725_cli_elev_003/fixtures/evidence/help-binding.json"
    );

    fn fixture(script: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("cli")).unwrap();
        std::fs::write(dir.path().join("cli/main.py"), script).unwrap();
        dir
    }

    #[test]
    fn help_listed_but_unimplemented_option_is_rejected() {
        let dir = fixture(
            "import argparse, sys\nif '--help' in sys.argv:\n print('usage: tool [--ghost]')\n raise SystemExit(0)\nargparse.ArgumentParser(add_help=False).parse_args()\n",
        );
        let report = run(
            dir.path(),
            Path::new("cli/main.py"),
            &[],
            Duration::from_secs(2),
        )
        .unwrap();
        assert!(!report.ok);
        let ghost = report
            .bindings
            .iter()
            .find(|binding| binding.option == "--ghost")
            .unwrap();
        assert!(!ghost.ok);
        assert!(ghost.nearest_miss.is_some());
    }

    #[test]
    fn hidden_implemented_option_is_outside_v0_enumeration_scope() {
        let dir = fixture(
            "import argparse\np=argparse.ArgumentParser()\np.add_argument('--hidden', help=argparse.SUPPRESS)\np.parse_args()\n",
        );
        let report = run(
            dir.path(),
            Path::new("cli/main.py"),
            &[],
            Duration::from_secs(2),
        )
        .unwrap();
        assert!(report.ok, "{report:?}");
        assert!(!report.help_options.contains(&"--hidden".to_string()));
        assert_eq!(
            report.implementation_to_help_scope,
            IMPLEMENTATION_TO_HELP_SCOPE
        );
    }

    #[test]
    fn argparse_help_and_parser_are_execution_bound_in_both_directions() {
        let dir = fixture(
            "import argparse\np=argparse.ArgumentParser()\np.add_argument('--name')\np.parse_args()\n",
        );
        let report = run(
            dir.path(),
            Path::new("cli/main.py"),
            &[],
            Duration::from_secs(2),
        )
        .unwrap();
        assert!(report.ok, "{report:?}");
        assert!(report.help_options.contains(&"--name".to_string()));
        assert!(
            report.bindings.iter().all(|binding| binding.ok),
            "{report:?}"
        );
        assert!(dir.path().join(EVIDENCE_PATH).is_file());
    }

    #[test]
    fn measured_required_args_masked_the_v0_unknown_option_probe() {
        let measured: Report = serde_json::from_str(MEASURED_HELP_BINDING).unwrap();
        let binding = measured
            .bindings
            .iter()
            .find(|binding| binding.direction == "implementation_to_help")
            .unwrap();

        assert_eq!(binding.observation.args, ["--anvil-invalid-probe"]);
        assert_eq!(binding.observation.exit_code, Some(2));
        assert!(
            binding
                .observation
                .stderr
                .text
                .contains("the following arguments are required: file, --pattern")
        );
        assert!(!binding.ok);
    }

    #[test]
    fn measured_cli_rejects_unknown_option_after_bound_normal_argv() {
        let dir = tempfile::tempdir().unwrap();
        for relative in ["cli/main.py", "data/sample.txt"] {
            let target = dir.path().join(relative);
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            std::fs::copy(Path::new(MEASURED_FIXTURE).join(relative), target).unwrap();
        }
        let normal_args = [
            "data/sample.txt".to_string(),
            "--pattern".to_string(),
            "Apple".to_string(),
        ];

        let report = run(
            dir.path(),
            Path::new("cli/main.py"),
            &normal_args,
            Duration::from_secs(2),
        )
        .unwrap();

        let binding = report
            .bindings
            .iter()
            .find(|binding| binding.direction == "implementation_to_help")
            .unwrap();
        assert_eq!(
            binding.observation.args,
            [
                "data/sample.txt",
                "--pattern",
                "Apple",
                "--anvil-invalid-probe"
            ]
        );
        assert_eq!(binding.observation.exit_code, Some(2));
        assert!(is_unrecognized(&binding.observation.stderr.text));
        assert!(binding.ok, "{binding:?}");
        assert!(report.ok, "{report:?}");
    }
}
