use std::path::Path;

use crate::planner::step_plan::{PlanStep, StepPlan};

pub(crate) fn nextjs_setup_fallback(root: &Path, goal: &str) -> Option<StepPlan> {
    fallback_setup_plan(
        goal,
        crate::planner::profiles::nextjs::setup_scaffold_paths(root),
        nextjs_instruction,
    )
}

pub(crate) fn python_cli_setup_fallback(root: &Path, goal: &str) -> Option<StepPlan> {
    fallback_setup_plan(
        goal,
        crate::planner::profiles::python_cli::contract_scaffold_paths(root, goal),
        python_cli_instruction,
    )
}

fn fallback_setup_plan(
    goal: &str,
    expected_paths: Vec<String>,
    instruction: fn(&str, &[String]) -> String,
) -> Option<StepPlan> {
    if !looks_like_setup_phase_goal(goal) || expected_paths.is_empty() {
        return None;
    }
    let verify = expected_paths
        .iter()
        .filter_map(|path| {
            crate::planner::verify::normalize_verify_command(&format!("test -f {path}"))
                .ok()
                .map(|normalized| normalized.into_string())
        })
        .collect();
    let plan = StepPlan {
        goal: goal.to_string(),
        steps: vec![PlanStep {
            id: "fallback-setup".to_string(),
            kind: "setup".to_string(),
            expected_result: "pass".to_string(),
            instruction: instruction(goal, &expected_paths),
            expected_paths,
            verify,
        }],
    };
    Some(plan)
}

fn python_cli_instruction(goal: &str, paths: &[String]) -> String {
    format!(
        "Create one coherent python-cli package scaffold for this setup phase: {goal}. \
         Required files: {paths}. \
         Coherence requirements: pyproject.toml declares the package metadata; \
         src/<package>/main.py implements a CLI that reads stdin or argv and prints non-empty output that changes when input changes; \
         keep dependency setup separate from verification and verify syntax with python3 -m compileall -q src.",
        goal = compact_single_line(goal),
        paths = paths.join(", "),
    )
}

fn nextjs_instruction(goal: &str, paths: &[String]) -> String {
    format!(
        "Create one coherent nextjs App Router scaffold for this setup phase: {goal}. \
         Required files: {paths}. \
         Coherence requirements: src/app/globals.css contains @tailwind base, @tailwind components, and @tailwind utilities; \
         src/app/layout.tsx imports ./globals.css; \
         create exactly one Tailwind config file, preferring tailwind.config.ts and never creating multiple tailwind.config.* files; \
         package.json scripts.dev and scripts.start run next dev and next start on the goal's port when one is mentioned.",
        goal = compact_single_line(goal),
        paths = paths.join(", "),
    )
}

fn looks_like_setup_phase_goal(goal: &str) -> bool {
    let phase_text = phase_id_and_task_text(goal).unwrap_or_else(|| goal.into());
    let lower = phase_text.to_ascii_lowercase();
    crate::planner::signals::contains_setup_token(&phase_text)
        || lower.contains("set up")
        || lower.contains("scaffold")
        || lower.contains("init")
        || lower.contains("initialize")
        || lower.contains("initialise")
}

fn phase_id_and_task_text(goal: &str) -> Option<String> {
    let lines = goal
        .lines()
        .map(str::trim_start)
        .filter(|line| line.starts_with("Phase id:") || line.starts_with("Phase task:"))
        .map(str::to_string)
        .collect::<Vec<_>>();
    (!lines.is_empty()).then(|| lines.join("\n"))
}

fn compact_single_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::python_cli_setup_fallback;
    use crate::planner::profiles::python_cli::complete_scaffold;

    #[test]
    fn python_cli_setup_fallback_uses_goal_package_not_default_app() {
        let dir = tempfile::tempdir().unwrap();
        let goal = "Set up the project scaffold for csv_stats.py";

        let plan = python_cli_setup_fallback(dir.path(), goal).expect("fallback plan");
        let paths = &plan.steps[0].expected_paths;
        assert!(
            paths.contains(&"src/csv_stats/main.py".to_string()),
            "{paths:?}"
        );
        assert!(!paths.contains(&"src/app/main.py".to_string()), "{paths:?}");

        complete_scaffold(dir.path(), paths).unwrap();
        assert!(dir.path().join("src/csv_stats/main.py").is_file());
        assert!(!dir.path().join("src/app").exists());
        let pyproject = std::fs::read_to_string(dir.path().join("pyproject.toml")).unwrap();
        assert!(pyproject.contains("csv_stats.main:main"), "{pyproject}");
    }

    #[test]
    fn python_cli_complete_scaffold_keeps_existing_goal_package() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src/csv_stats")).unwrap();
        std::fs::write(dir.path().join("src/csv_stats/main.py"), "print('x')\n").unwrap();

        let goal = "Set up the project scaffold for csv_stats.py";
        let plan = python_cli_setup_fallback(dir.path(), goal).expect("fallback plan");
        complete_scaffold(dir.path(), &plan.steps[0].expected_paths).unwrap();

        assert!(!dir.path().join("src/app").exists());
    }
}
