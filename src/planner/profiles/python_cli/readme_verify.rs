use std::path::Path;

use serde_json::json;

use crate::eval_events;
use crate::planner::step_plan::{PlanStep, StepKind, StepPlan};
use crate::planner::verify::VerificationReport;

pub(crate) const CHECK_COMMAND: &str = "anvil-cli-check:readme_structure";
const VERIFY_INSTRUCTION: &str =
    "Verify that README.md contains a structurally bound cli/main.py usage example.";

pub(crate) fn canonicalize_step_plan(
    plan: &mut StepPlan,
    profile: &str,
    create_intent: bool,
    eval_events_path: Option<&Path>,
) -> usize {
    if !create_intent || !is_cli_profile(profile) {
        return 0;
    }
    plan.steps
        .iter_mut()
        .map(|step| canonicalize_step(step, eval_events_path))
        .sum()
}

pub(crate) fn is_check_command(command: &str) -> bool {
    command.trim() == CHECK_COMMAND
}

pub(crate) fn is_internal_check_command(command: &str) -> bool {
    crate::planner::profiles::data::step_policy::catalog_check_id(command).is_some()
        || is_check_command(command)
}

pub(crate) fn run_step_checks(
    root: &Path,
    profile: Option<&str>,
    goal: Option<&str>,
    step: &PlanStep,
    eval_events_path: Option<&Path>,
    report: &mut VerificationReport,
) {
    crate::planner::profiles::data::step_policy::run_step_catalog_checks(
        root,
        profile,
        goal,
        step,
        eval_events_path,
        report,
    );
    run_step_check(root, profile, step, report);
}

pub(crate) fn run_step_check(
    root: &Path,
    profile: Option<&str>,
    step: &PlanStep,
    report: &mut VerificationReport,
) {
    if !step.verify.iter().any(|command| is_check_command(command)) {
        return;
    }
    if !profile.is_some_and(is_cli_profile) {
        report.push_command_failure(
            CHECK_COMMAND,
            "CLI README structure check is invalid outside the active CLI profile",
        );
        return;
    }
    for reason in verify_readme_structure(root) {
        report.push_profile_failure(reason);
    }
}

fn canonicalize_step(step: &mut PlanStep, eval_events_path: Option<&Path>) -> usize {
    let verifier_artifact = step
        .expected_paths
        .iter()
        .any(|path| is_readme_verifier_path(path));
    let dedicated_verify = step.step_kind() == StepKind::Verify
        && mentions_readme(&format!(
            "{}\n{}\n{}",
            step.id,
            step.instruction,
            step.verify.join("\n")
        ));
    let mut changes = 0;

    if verifier_artifact {
        let original = format!(
            "kind={}; expected_paths={}",
            step.kind,
            step.expected_paths.join(",")
        );
        step.kind = "verify".to_string();
        step.expected_result = "pass".to_string();
        step.instruction = VERIFY_INSTRUCTION.to_string();
        step.expected_paths
            .retain(|path| !is_readme_verifier_path(path));
        emit_canonicalized(
            eval_events_path,
            &step.id,
            "readme_verifier_artifact",
            &original,
            CHECK_COMMAND,
        );
        changes += 1;
    }

    if dedicated_verify || verifier_artifact {
        if step.verify != [CHECK_COMMAND] {
            let original = step.verify.join("\n");
            step.verify = vec![CHECK_COMMAND.to_string()];
            emit_canonicalized(
                eval_events_path,
                &step.id,
                "verify",
                &original,
                CHECK_COMMAND,
            );
            changes += 1;
        }
        step.instruction = VERIFY_INSTRUCTION.to_string();
        return changes;
    }

    let original = std::mem::take(&mut step.verify);
    for command in original {
        let replacement = if mentions_readme(&command) {
            changes += 1;
            emit_canonicalized(
                eval_events_path,
                &step.id,
                "verify",
                &command,
                CHECK_COMMAND,
            );
            CHECK_COMMAND.to_string()
        } else {
            command
        };
        if !step.verify.contains(&replacement) {
            step.verify.push(replacement);
        }
    }
    changes
}

// This phase gate is intentionally structural: goal-derived natural-language
// literals and README output values are forbidden here. Acceptance C3 owns
// comparison of documented output claims with observed CLI output.
fn verify_readme_structure(root: &Path) -> Vec<String> {
    let Ok(readme) = std::fs::read_to_string(root.join("README.md")) else {
        return vec!["cli_readme_structure:readme_missing".to_string()];
    };
    let mut fenced = false;
    let mut in_section = false;
    let mut invocation_found = false;
    let mut section_invocation_found = false;

    for line in readme.lines() {
        let trimmed = line.trim();
        if let Some(level) = markdown_heading_level(trimmed) {
            in_section = level >= 2;
        }
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fenced = !fenced;
            continue;
        }
        if cli_invocation(trimmed, fenced) {
            invocation_found = true;
            section_invocation_found |= in_section;
        }
    }

    if !invocation_found {
        vec!["cli_readme_structure:cli_invocation_missing".to_string()]
    } else if !section_invocation_found {
        vec!["cli_readme_structure:usage_section_missing".to_string()]
    } else {
        Vec::new()
    }
}

fn markdown_heading_level(line: &str) -> Option<usize> {
    let level = line.chars().take_while(|ch| *ch == '#').count();
    (level > 0 && level <= 6 && line.chars().nth(level).is_some_and(char::is_whitespace))
        .then_some(level)
}

fn cli_invocation(line: &str, fenced: bool) -> bool {
    let command = line.strip_prefix("$ ").unwrap_or(line);
    let words = command.split_whitespace().collect::<Vec<_>>();
    let command_line = fenced
        || line.starts_with("$ ")
        || words
            .first()
            .is_some_and(|program| matches!(*program, "python" | "python3"));
    command_line
        && words
            .first()
            .is_some_and(|program| matches!(*program, "python" | "python3"))
        && words.iter().skip(1).any(|word| {
            word.trim_matches(['`', '\'', '"']).trim_start_matches("./") == "cli/main.py"
        })
}

fn is_cli_profile(profile: &str) -> bool {
    matches!(
        profile.trim().to_ascii_lowercase().as_str(),
        "cli" | "python-cli" | "python_cli"
    )
}

fn is_readme_verifier_path(path: &str) -> bool {
    let lower = path.replace('\\', "/").to_ascii_lowercase();
    lower
        .rsplit('/')
        .next()
        .is_some_and(|name| name.contains("verify_readme"))
}

fn mentions_readme(text: &str) -> bool {
    text.to_ascii_lowercase().contains("readme")
}

fn emit_canonicalized(
    eval_events_path: Option<&Path>,
    step_id: &str,
    field: &str,
    original: &str,
    replacement: &str,
) {
    eval_events::emit(
        eval_events_path,
        json!({
            "event": "verify_canonicalized",
            "step_id": step_id,
            "field": field,
            "original": original,
            "replacement": replacement,
            "disposition": "canonical",
        }),
    );
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    const ENGLISH_README: &str =
        include_str!("../../../../tests/corpus/apps/cli-readme-structural/README.md");

    fn write_readme(root: &Path, text: &str) {
        std::fs::write(root.join("README.md"), text).unwrap();
    }

    fn verify_step(command: &str) -> PlanStep {
        PlanStep {
            id: "verify-readme-content".to_string(),
            kind: "verify".to_string(),
            expected_result: "pass".to_string(),
            instruction: "Check README content.".to_string(),
            expected_paths: Vec::new(),
            verify: vec![command.to_string()],
        }
    }

    #[test]
    fn english_readme_without_goal_literal_passes_structural_check() {
        let dir = tempdir().unwrap();
        write_readme(dir.path(), ENGLISH_README);
        assert!(!ENGLISH_README.contains("合計"));
        assert!(verify_readme_structure(dir.path()).is_empty());
        let mut report = VerificationReport::pass();
        run_step_check(
            dir.path(),
            Some("cli"),
            &verify_step(CHECK_COMMAND),
            &mut report,
        );
        assert!(report.is_pass());
    }

    #[test]
    fn missing_readme_and_missing_invocation_fail() {
        let dir = tempdir().unwrap();
        assert_eq!(
            verify_readme_structure(dir.path()),
            ["cli_readme_structure:readme_missing"]
        );
        write_readme(dir.path(), "# Tool\n\n## Usage\n\nSee the source.\n");
        assert_eq!(
            verify_readme_structure(dir.path()),
            ["cli_readme_structure:cli_invocation_missing"]
        );
    }

    #[test]
    fn usage_section_is_structural_and_accepts_a_bare_command_line() {
        let dir = tempdir().unwrap();
        write_readme(
            dir.path(),
            "# Tool\n\n## Beispiel\n\npython3 cli/main.py --help\n",
        );
        assert!(verify_readme_structure(dir.path()).is_empty());

        write_readme(
            dir.path(),
            "# Tool\n\n```bash\npython3 cli/main.py --help\n```\n",
        );
        assert_eq!(
            verify_readme_structure(dir.path()),
            ["cli_readme_structure:usage_section_missing"]
        );
    }

    #[test]
    fn local_goal_literal_assert_is_replaced_by_bound_structure_check() {
        let mut plan = StepPlan {
            goal: "Create README.md".to_string(),
            steps: vec![verify_step(
                "python -c \"content=open('README.md').read(); assert '合計' in content\"",
            )],
        };
        assert_eq!(canonicalize_step_plan(&mut plan, "cli", true, None), 1);
        assert_eq!(plan.steps[0].verify, [CHECK_COMMAND]);
        assert!(!plan.steps[0].instruction.contains("合計"));
    }

    #[test]
    fn generated_readme_verifier_artifact_is_not_created_or_executed() {
        let mut plan = StepPlan {
            goal: "Document the CLI".to_string(),
            steps: vec![PlanStep {
                id: "create-smoke-test".to_string(),
                kind: "implement".to_string(),
                expected_result: "pass".to_string(),
                instruction: "Create tests/verify_readme.py with content assertions.".to_string(),
                expected_paths: vec!["tests/verify_readme.py".to_string()],
                verify: Vec::new(),
            }],
        };
        assert_eq!(canonicalize_step_plan(&mut plan, "cli", true, None), 2);
        assert_eq!(plan.steps[0].step_kind(), StepKind::Verify);
        assert!(plan.steps[0].expected_paths.is_empty());
        assert_eq!(plan.steps[0].verify, [CHECK_COMMAND]);
    }

    #[test]
    fn non_cli_and_non_create_plans_are_byte_unchanged() {
        let original = StepPlan {
            goal: "Check README.md".to_string(),
            steps: vec![verify_step("grep -q Usage README.md")],
        };
        for (profile, create) in [("data", true), ("nextjs", true), ("cli", false)] {
            let mut candidate = original.clone();
            assert_eq!(
                canonicalize_step_plan(&mut candidate, profile, create, None),
                0
            );
            assert_eq!(candidate, original);
        }
    }
}
