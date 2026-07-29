use std::path::Path;

use crate::planner::profile::DomainProfile;
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
        crate::planner::profiles::python_cli::PythonCliProfile.setup_scaffold_paths(root),
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
         keep dependency setup separate from verification and verify syntax with python -m compileall -q src.",
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
