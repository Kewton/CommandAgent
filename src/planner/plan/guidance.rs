use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanFileKind {
    Step,
    Ultra,
}

impl PlanFileKind {
    pub const fn run_flag(self) -> &'static str {
        match self {
            Self::Step => "--run-plan",
            Self::Ultra => "--run-ultra-plan",
        }
    }
}

pub fn saved_plan_guidance(path: &Path, kind: PlanFileKind) -> String {
    let path = shell_path(path);
    format!(
        "Edit the saved plan if needed.\nNext: commandagent --validate-plan {path}\nRun after successful validation: commandagent {} {path}",
        kind.run_flag()
    )
}

pub fn next_command(path: &Path, kind: PlanFileKind) -> String {
    format!("commandagent {} {}", kind.run_flag(), shell_path(path))
}

fn shell_path(path: &Path) -> String {
    let display = path.to_string_lossy().replace('\\', "/");
    if display
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '/'))
    {
        display
    } else {
        format!("{display:?}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guidance_quotes_paths_with_spaces() {
        let guidance = saved_plan_guidance(Path::new("plans/my plan.yaml"), PlanFileKind::Step);
        assert!(guidance.starts_with("Edit the saved plan if needed.\nNext:"));
        assert!(guidance.contains("--validate-plan \"plans/my plan.yaml\""));
        assert!(
            guidance.contains(
                "Run after successful validation: commandagent --run-plan \"plans/my plan.yaml\""
            ),
            "{guidance}"
        );
    }

    #[test]
    fn ultra_plan_guidance_names_the_matching_runner_only_after_validation() {
        let guidance = saved_plan_guidance(Path::new("plans/ultra.yaml"), PlanFileKind::Ultra);

        assert!(guidance.contains("Next: commandagent --validate-plan plans/ultra.yaml"));
        assert!(guidance.contains(
            "Run after successful validation: commandagent --run-ultra-plan plans/ultra.yaml"
        ));
        assert!(!guidance.contains("commandagent --run-plan plans/ultra.yaml"));
    }
}
